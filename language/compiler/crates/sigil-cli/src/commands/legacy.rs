//! Command implementations for CLI

use super::compile_support::{
    analyze_module_graph, build_world_runtime_prelude, collect_sigil_targets,
    compile_entry_files_with_cache, generate_module_graph_outputs, group_compile_targets,
    runner_prelude, topology_source_path, AnalyzedModule, CompiledGraphOutputs, CoverageTarget,
    GeneratedGraphOutputs, OutputFlavor,
};
use super::shared::{
    extract_error_code, format_validation_errors, output_inspect_error, output_json_error_to,
    output_json_value, project_error_json_details, type_error_json_details,
    validate_project_entrypoint_for_path, validate_project_entrypoints_for_files,
    SourcePoint as TestLocation,
};
use crate::hash::encode_lower_hex;
use crate::module_graph::{
    entry_module_key, load_project_effect_catalog_for, ModuleGraph, ModuleGraphError,
};
use crate::project::{get_project_config, ProjectConfig, ProjectConfigError};
use rayon::{prelude::*, ThreadPoolBuilder};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sigil_ast::{Declaration, Expr, Pattern, Program, SourceLocation, Type, TypeDef};
use sigil_codegen::{world_runtime_helpers_source, DebugSpanKind, DebugSpanRecord, ModuleSpanMap};
use sigil_diagnostics::codes;
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::types::InferenceType;
use sigil_typechecker::{TypeError, TypeScheme, TypedDeclaration};
use sigil_validator::{
    print_canonical_expr, print_canonical_program_with_effects, print_canonical_type_definition,
    validate_canonical_form_with_options, ValidationError, ValidationOptions,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const TEST_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lexer error: {0}")]
    Lexer(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Type error: {0}")]
    Type(#[from] TypeError),

    #[error("Codegen error: {0}")]
    Codegen(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Breakpoint error: {code}: {message}")]
    Breakpoint {
        code: String,
        message: String,
        details: serde_json::Value,
    },

    #[error("Module graph error: {0}")]
    ModuleGraph(#[from] ModuleGraphError),

    #[error("Project config error: {0}")]
    ProjectConfig(#[from] ProjectConfigError),

    #[error("reported")]
    Reported(i32),
}

impl CliError {
    pub fn reported_exit_code(&self) -> Option<i32> {
        match self {
            CliError::Reported(exit_code) => Some(*exit_code),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectMode {
    Types,
    Proof,
    Validate,
    Codegen,
    World,
}

impl InspectMode {
    fn command_name(self) -> &'static str {
        match self {
            InspectMode::Types => "sigilc inspect types",
            InspectMode::Proof => "sigilc inspect proof",
            InspectMode::Validate => "sigilc inspect validate",
            InspectMode::Codegen => "sigilc inspect codegen",
            InspectMode::World => "sigilc inspect world",
        }
    }

    fn phase(self) -> &'static str {
        match self {
            InspectMode::Types => "typecheck",
            InspectMode::Proof => "proof",
            InspectMode::Validate => "canonical",
            InspectMode::Codegen => "codegen",
            InspectMode::World => "topology",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            InspectMode::Types => "inspect types",
            InspectMode::Proof => "inspect proof",
            InspectMode::Validate => "inspect validate",
            InspectMode::Codegen => "inspect codegen",
            InspectMode::World => "inspect world",
        }
    }
}

/// Lex command: tokenize a Sigil file
pub fn lex_command(file: &Path) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| CliError::Lexer(format!("{}", e)))?;

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc lex",
        "ok": true,
        "phase": "lexer",
        "data": {
            "file": filename,
            "summary": {
                "tokens": tokens.len()
            },
            "tokens": tokens.iter().map(|t| {
                serde_json::json!({
                    "type": format!("{:?}", t.token_type),
                    "lexeme": &t.value,
                    "start": {
                        "line": t.location.start.line,
                        "column": t.location.start.column,
                        "offset": t.location.start.offset
                    },
                    "end": {
                        "line": t.location.end.line,
                        "column": t.location.end.column,
                        "offset": t.location.end.offset
                    },
                    "text": format!("{}({}) at {}:{}", format!("{:?}", t.token_type), &t.value, t.location.start.line, t.location.start.column)
                })
            }).collect::<Vec<_>>()
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());

    Ok(())
}

/// Parse command: parse a Sigil file to AST
pub fn parse_command(file: &Path) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| CliError::Lexer(format!("{}", e)))?;
    let token_count = tokens.len(); // Store token count for JSON output

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser
        .parse()
        .map_err(|e| CliError::Parser(format!("{}", e)))?;

    let effect_catalog = load_project_effect_catalog_for(file)?;

    // Validate canonical form (includes formatting)
    validate_canonical_form_with_options(
        &ast,
        Some(&filename),
        Some(&source),
        ValidationOptions { effect_catalog },
    )
    .map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format_validation_errors(&errors))
    })?;

    let ast_json = serde_json::to_value(&ast).unwrap_or_else(|e| {
        eprintln!("Warning: AST serialization failed: {}", e);
        serde_json::json!(format!("{:#?}", ast))
    });

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc parse",
        "ok": true,
        "phase": "parser",
        "data": {
            "file": filename,
            "summary": {
                "tokens": token_count,
                "declarations": ast.declarations.len()
            },
            "ast": ast_json
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());

    Ok(())
}

fn project_json(project: Option<&ProjectConfig>) -> Option<serde_json::Value> {
    project.map(|project| {
        serde_json::json!({
            "root": project.root.to_string_lossy(),
            "layout": serde_json::to_value(&project.layout).unwrap_or(serde_json::json!({}))
        })
    })
}

fn source_location_json(source_file: &str, location: SourceLocation) -> serde_json::Value {
    serde_json::json!({
        "file": source_file,
        "start": {
            "line": location.start.line,
            "column": location.start.column,
            "offset": location.start.offset
        },
        "end": {
            "line": location.end.line,
            "column": location.end.column,
            "offset": location.end.offset
        }
    })
}

fn ast_declaration_summary(program: &Program) -> serde_json::Value {
    let mut functions = 0usize;
    let mut types = 0usize;
    let mut effects = 0usize;
    let mut consts = 0usize;
    let mut feature_flags = 0usize;
    let mut tests = 0usize;
    let mut externs = 0usize;
    let mut labels = 0usize;
    let mut rules = 0usize;
    let mut transforms = 0usize;

    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(_) => functions += 1,
            Declaration::Type(_) => types += 1,
            Declaration::Protocol(_) => {}
            Declaration::Effect(_) => effects += 1,
            Declaration::Const(_) => consts += 1,
            Declaration::FeatureFlag(_) => feature_flags += 1,
            Declaration::Test(_) => tests += 1,
            Declaration::Extern(_) => externs += 1,
            Declaration::Label(_) => labels += 1,
            Declaration::Rule(_) => rules += 1,
            Declaration::Transform(_) => transforms += 1,
        }
    }

    serde_json::json!({
        "declarations": program.declarations.len(),
        "functions": functions,
        "types": types,
        "effects": effects,
        "consts": consts,
        "featureFlags": feature_flags,
        "tests": tests,
        "externs": externs,
        "labels": labels,
        "rules": rules,
        "transforms": transforms
    })
}

fn collect_quantified_var_names(
    typ: &InferenceType,
    quantified_vars: &HashSet<u32>,
    names: &mut HashMap<u32, String>,
) {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => {}
        InferenceType::Var(var) => {
            if quantified_vars.contains(&var.id) {
                names
                    .entry(var.id)
                    .or_insert_with(|| var.name.clone().unwrap_or_else(|| format!("α{}", var.id)));
            }
            if let Some(instance) = &var.instance {
                collect_quantified_var_names(instance, quantified_vars, names);
            }
        }
        InferenceType::Function(function) => {
            for param in &function.params {
                collect_quantified_var_names(param, quantified_vars, names);
            }
            collect_quantified_var_names(&function.return_type, quantified_vars, names);
        }
        InferenceType::List(list) => {
            collect_quantified_var_names(&list.element_type, quantified_vars, names);
        }
        InferenceType::Map(map) => {
            collect_quantified_var_names(&map.key_type, quantified_vars, names);
            collect_quantified_var_names(&map.value_type, quantified_vars, names);
        }
        InferenceType::Tuple(tuple) => {
            for item in &tuple.types {
                collect_quantified_var_names(item, quantified_vars, names);
            }
        }
        InferenceType::Record(record) => {
            for field_type in record.fields.values() {
                collect_quantified_var_names(field_type, quantified_vars, names);
            }
        }
        InferenceType::Constructor(constructor) => {
            for arg in &constructor.type_args {
                collect_quantified_var_names(arg, quantified_vars, names);
            }
        }
        InferenceType::Owned(inner) => {
            collect_quantified_var_names(inner, quantified_vars, names);
        }
        InferenceType::Borrowed(borrowed) => {
            collect_quantified_var_names(&borrowed.resource_type, quantified_vars, names);
        }
    }
}

fn format_type_scheme(scheme: &TypeScheme) -> String {
    let type_text = sigil_typechecker::format_type(&scheme.typ);
    if scheme.quantified_vars.is_empty() {
        return type_text;
    }

    let mut names = HashMap::new();
    collect_quantified_var_names(&scheme.typ, &scheme.quantified_vars, &mut names);

    let mut quantified = scheme
        .quantified_vars
        .iter()
        .map(|id| {
            (
                names.get(id).cloned().unwrap_or_else(|| format!("α{}", id)),
                *id,
            )
        })
        .collect::<Vec<_>>();
    quantified.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    format!(
        "∀{}. {}",
        quantified
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
            .join(", "),
        type_text
    )
}

fn format_test_signature(test_decl: &sigil_typechecker::typed_ir::TypedTestDecl) -> String {
    let mut signature = String::from("() =>");
    if let Some(effects) = &test_decl.effects {
        let mut sorted_effects = effects.iter().cloned().collect::<Vec<_>>();
        sorted_effects.sort();
        signature.push_str(
            &sorted_effects
                .into_iter()
                .map(|effect| format!("!{}", effect))
                .collect::<Vec<_>>()
                .join(""),
        );
        signature.push(' ');
    } else {
        signature.push(' ');
    }
    signature.push_str(&sigil_typechecker::format_type(&test_decl.body.typ));
    signature
}

fn inspect_named_type_id(module: &AnalyzedModule, name: &str) -> String {
    format!("{}.{}", module.module_id, name)
}

fn inspect_type_equality_mode(definition: &TypeDef, constrained: bool) -> &'static str {
    match definition {
        TypeDef::Sum(_) => "nominal",
        TypeDef::Alias(_) | TypeDef::Product(_) => {
            if constrained {
                "refinement"
            } else {
                "structural"
            }
        }
    }
}

fn inspect_literal_type_name(literal_type: sigil_ast::LiteralType) -> &'static str {
    match literal_type {
        sigil_ast::LiteralType::Int => "Int",
        sigil_ast::LiteralType::Float => "Float",
        sigil_ast::LiteralType::String => "String",
        sigil_ast::LiteralType::Char => "Char",
        sigil_ast::LiteralType::Bool => "Bool",
        sigil_ast::LiteralType::Unit => "Unit",
    }
}

fn inspect_pattern_literal_type_name(literal_type: sigil_ast::PatternLiteralType) -> &'static str {
    match literal_type {
        sigil_ast::PatternLiteralType::Int => "Int",
        sigil_ast::PatternLiteralType::Float => "Float",
        sigil_ast::PatternLiteralType::String => "String",
        sigil_ast::PatternLiteralType::Char => "Char",
        sigil_ast::PatternLiteralType::Bool => "Bool",
        sigil_ast::PatternLiteralType::Unit => "Unit",
    }
}

fn inspect_pipeline_operator_name(operator: sigil_ast::PipelineOperator) -> &'static str {
    match operator {
        sigil_ast::PipelineOperator::Pipe => "|>",
        sigil_ast::PipelineOperator::ComposeFwd => ">>",
        sigil_ast::PipelineOperator::ComposeBwd => "<<",
    }
}

fn inspect_literal_value_json(value: &sigil_ast::LiteralValue) -> Value {
    match value {
        sigil_ast::LiteralValue::Int(value) => json!(value),
        sigil_ast::LiteralValue::Float(value) => json!(value),
        sigil_ast::LiteralValue::String(value) => json!(value),
        sigil_ast::LiteralValue::Char(value) => json!(value.to_string()),
        sigil_ast::LiteralValue::Bool(value) => json!(value),
        sigil_ast::LiteralValue::Unit => Value::Null,
    }
}

fn inspect_pattern_literal_value_json(value: &sigil_ast::PatternLiteralValue) -> Value {
    match value {
        sigil_ast::PatternLiteralValue::Int(value) => json!(value),
        sigil_ast::PatternLiteralValue::Float(value) => json!(value),
        sigil_ast::PatternLiteralValue::String(value) => json!(value),
        sigil_ast::PatternLiteralValue::Char(value) => json!(value.to_string()),
        sigil_ast::PatternLiteralValue::Bool(value) => json!(value),
        sigil_ast::PatternLiteralValue::Unit => Value::Null,
    }
}

fn inspect_type_ast(typ: &Type) -> Value {
    match typ {
        Type::Primitive(primitive) => json!({
            "kind": "primitive",
            "name": primitive.name.to_string()
        }),
        Type::List(list) => json!({
            "kind": "list",
            "element": inspect_type_ast(&list.element_type)
        }),
        Type::Map(map) => json!({
            "kind": "map",
            "key": inspect_type_ast(&map.key_type),
            "value": inspect_type_ast(&map.value_type)
        }),
        Type::Function(function) => json!({
            "kind": "function",
            "params": function
                .param_types
                .iter()
                .map(inspect_type_ast)
                .collect::<Vec<_>>(),
            "effects": function.effects,
            "returns": inspect_type_ast(&function.return_type)
        }),
        Type::Constructor(constructor) => json!({
            "kind": "constructor",
            "name": constructor.name,
            "typeArgs": constructor
                .type_args
                .iter()
                .map(inspect_type_ast)
                .collect::<Vec<_>>()
        }),
        Type::Variable(variable) => json!({
            "kind": "variable",
            "name": variable.name
        }),
        Type::Tuple(tuple) => json!({
            "kind": "tuple",
            "items": tuple.types.iter().map(inspect_type_ast).collect::<Vec<_>>()
        }),
        Type::Qualified(qualified) => json!({
            "kind": "qualified",
            "modulePath": qualified.module_path,
            "name": qualified.type_name,
            "typeArgs": qualified
                .type_args
                .iter()
                .map(inspect_type_ast)
                .collect::<Vec<_>>()
        }),
    }
}

fn inspect_type_definition_ast(definition: &TypeDef) -> Value {
    match definition {
        TypeDef::Alias(alias) => json!({
            "kind": "alias",
            "target": inspect_type_ast(&alias.aliased_type)
        }),
        TypeDef::Product(product) => json!({
            "kind": "product",
            "fields": product
                .fields
                .iter()
                .map(|field| {
                    json!({
                        "name": field.name,
                        "type": inspect_type_ast(&field.field_type)
                    })
                })
                .collect::<Vec<_>>()
        }),
        TypeDef::Sum(sum) => json!({
            "kind": "sum",
            "variants": sum
                .variants
                .iter()
                .map(|variant| {
                    json!({
                        "name": variant.name,
                        "types": variant.types.iter().map(inspect_type_ast).collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>()
        }),
    }
}

fn inspect_pattern_ast(pattern: &Pattern) -> Value {
    match pattern {
        Pattern::Literal(literal) => json!({
            "kind": "literal",
            "literalType": inspect_pattern_literal_type_name(literal.literal_type),
            "value": inspect_pattern_literal_value_json(&literal.value)
        }),
        Pattern::Identifier(identifier) => json!({
            "kind": "identifier",
            "name": identifier.name
        }),
        Pattern::Wildcard(_) => json!({
            "kind": "wildcard"
        }),
        Pattern::Constructor(constructor) => json!({
            "kind": "constructor",
            "modulePath": constructor.module_path,
            "name": constructor.name,
            "patterns": constructor
                .patterns
                .iter()
                .map(inspect_pattern_ast)
                .collect::<Vec<_>>()
        }),
        Pattern::List(list) => json!({
            "kind": "list",
            "items": list.patterns.iter().map(inspect_pattern_ast).collect::<Vec<_>>(),
            "rest": list.rest
        }),
        Pattern::Record(record) => json!({
            "kind": "record",
            "fields": record
                .fields
                .iter()
                .map(|field| {
                    json!({
                        "name": field.name,
                        "pattern": field.pattern.as_ref().map(inspect_pattern_ast)
                    })
                })
                .collect::<Vec<_>>()
        }),
        Pattern::Tuple(tuple) => json!({
            "kind": "tuple",
            "items": tuple.patterns.iter().map(inspect_pattern_ast).collect::<Vec<_>>()
        }),
    }
}

fn inspect_expr_ast(expr: &Expr) -> Value {
    match expr {
        Expr::Literal(literal) => json!({
            "kind": "literal",
            "literalType": inspect_literal_type_name(literal.literal_type),
            "value": inspect_literal_value_json(&literal.value)
        }),
        Expr::Identifier(identifier) => {
            if identifier.name == "value" {
                json!({
                    "kind": "value"
                })
            } else {
                json!({
                    "kind": "name",
                    "name": identifier.name
                })
            }
        }
        Expr::Lambda(lambda) => json!({
            "kind": "lambda",
            "params": lambda
                .params
                .iter()
                .map(|param| {
                    json!({
                        "name": param.name,
                        "type": param.type_annotation.as_ref().map(inspect_type_ast),
                        "isMutable": param.is_mutable
                    })
                })
                .collect::<Vec<_>>(),
            "effects": lambda.effects,
            "returnType": inspect_type_ast(&lambda.return_type),
            "body": inspect_expr_ast(&lambda.body)
        }),
        Expr::Application(application) => json!({
            "kind": "call",
            "func": inspect_expr_ast(&application.func),
            "args": application.args.iter().map(inspect_expr_ast).collect::<Vec<_>>()
        }),
        Expr::Binary(binary) => json!({
            "kind": "binary",
            "operator": binary.operator.to_string(),
            "left": inspect_expr_ast(&binary.left),
            "right": inspect_expr_ast(&binary.right)
        }),
        Expr::Unary(unary) => json!({
            "kind": "unary",
            "operator": unary.operator.to_string(),
            "operand": inspect_expr_ast(&unary.operand)
        }),
        Expr::Match(match_expr) => json!({
            "kind": "match",
            "scrutinee": inspect_expr_ast(&match_expr.scrutinee),
            "arms": match_expr
                .arms
                .iter()
                .map(|arm| {
                    json!({
                        "pattern": inspect_pattern_ast(&arm.pattern),
                        "guard": arm.guard.as_ref().map(inspect_expr_ast),
                        "body": inspect_expr_ast(&arm.body)
                    })
                })
                .collect::<Vec<_>>()
        }),
        Expr::Let(let_expr) => json!({
            "kind": "let",
            "pattern": inspect_pattern_ast(&let_expr.pattern),
            "value": inspect_expr_ast(&let_expr.value),
            "body": inspect_expr_ast(&let_expr.body)
        }),
        Expr::Using(using_expr) => json!({
            "kind": "using",
            "name": using_expr.name,
            "value": inspect_expr_ast(&using_expr.value),
            "body": inspect_expr_ast(&using_expr.body)
        }),
        Expr::If(if_expr) => json!({
            "kind": "if",
            "condition": inspect_expr_ast(&if_expr.condition),
            "then": inspect_expr_ast(&if_expr.then_branch),
            "else": if_expr.else_branch.as_ref().map(inspect_expr_ast)
        }),
        Expr::List(list) => json!({
            "kind": "list",
            "items": list.elements.iter().map(inspect_expr_ast).collect::<Vec<_>>()
        }),
        Expr::Record(record) => json!({
            "kind": "record",
            "fields": record
                .fields
                .iter()
                .map(|field| {
                    json!({
                        "name": field.name,
                        "value": inspect_expr_ast(&field.value)
                    })
                })
                .collect::<Vec<_>>()
        }),
        Expr::MapLiteral(map_literal) => json!({
            "kind": "map",
            "entries": map_literal
                .entries
                .iter()
                .map(|entry| {
                    json!({
                        "key": inspect_expr_ast(&entry.key),
                        "value": inspect_expr_ast(&entry.value)
                    })
                })
                .collect::<Vec<_>>()
        }),
        Expr::Tuple(tuple) => json!({
            "kind": "tuple",
            "items": tuple.elements.iter().map(inspect_expr_ast).collect::<Vec<_>>()
        }),
        Expr::FieldAccess(field_access) => json!({
            "kind": "field",
            "object": inspect_expr_ast(&field_access.object),
            "field": field_access.field
        }),
        Expr::Index(index) => json!({
            "kind": "index",
            "object": inspect_expr_ast(&index.object),
            "index": inspect_expr_ast(&index.index)
        }),
        Expr::Pipeline(pipeline) => json!({
            "kind": "pipeline",
            "operator": inspect_pipeline_operator_name(pipeline.operator),
            "left": inspect_expr_ast(&pipeline.left),
            "right": inspect_expr_ast(&pipeline.right)
        }),
        Expr::Map(map_expr) => json!({
            "kind": "mapExpr",
            "list": inspect_expr_ast(&map_expr.list),
            "func": inspect_expr_ast(&map_expr.func)
        }),
        Expr::Filter(filter) => json!({
            "kind": "filter",
            "list": inspect_expr_ast(&filter.list),
            "predicate": inspect_expr_ast(&filter.predicate)
        }),
        Expr::Fold(fold) => json!({
            "kind": "fold",
            "list": inspect_expr_ast(&fold.list),
            "func": inspect_expr_ast(&fold.func),
            "init": inspect_expr_ast(&fold.init)
        }),
        Expr::Concurrent(concurrent) => json!({
            "kind": "concurrent",
            "name": concurrent.name,
            "width": inspect_expr_ast(&concurrent.width),
            "policy": concurrent.policy.as_ref().map(|policy| inspect_expr_ast(&Expr::Record(policy.clone()))),
            "steps": concurrent
                .steps
                .iter()
                .map(|step| match step {
                    sigil_ast::ConcurrentStep::Spawn(spawn) => json!({
                        "kind": "spawn",
                        "expr": inspect_expr_ast(&spawn.expr)
                    }),
                    sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => json!({
                        "kind": "spawnEach",
                        "func": inspect_expr_ast(&spawn_each.func),
                        "list": inspect_expr_ast(&spawn_each.list)
                    }),
                })
                .collect::<Vec<_>>()
        }),
        Expr::MemberAccess(member_access) => json!({
            "kind": "memberAccess",
            "namespace": member_access.namespace,
            "member": member_access.member
        }),
        Expr::TypeAscription(ascription) => json!({
            "kind": "ascribe",
            "expr": inspect_expr_ast(&ascription.expr),
            "type": inspect_type_ast(&ascription.ascribed_type)
        }),
    }
}

fn inspect_named_types(module: &AnalyzedModule) -> Vec<Value> {
    let source_file = module.file_path.to_string_lossy().to_string();

    module
        .typed_program
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| match declaration {
            TypedDeclaration::Type(type_decl) => {
                let constrained = type_decl.ast.constraint.is_some();
                let kind = match &type_decl.ast.definition {
                    TypeDef::Alias(_) => "alias",
                    TypeDef::Product(_) => "product",
                    TypeDef::Sum(_) => "sum",
                };
                Some(json!({
                    "typeId": inspect_named_type_id(module, &type_decl.ast.name),
                    "name": type_decl.ast.name,
                    "moduleId": module.module_id,
                    "kind": kind,
                    "typeParams": type_decl.ast.type_params,
                    "definitionSource": print_canonical_type_definition(&type_decl.ast.definition),
                    "definitionAst": inspect_type_definition_ast(&type_decl.ast.definition),
                    "constrained": constrained,
                    "constraintSource": type_decl.ast.constraint.as_ref().map(print_canonical_expr),
                    "constraintAst": type_decl.ast.constraint.as_ref().map(inspect_expr_ast),
                    "equalityMode": inspect_type_equality_mode(&type_decl.ast.definition, constrained),
                    "spanId": module
                        .declaration_span_ids
                        .get(index)
                        .and_then(|span_id| span_id.clone())
                        .unwrap_or_default(),
                    "location": source_location_json(&source_file, type_decl.ast.location)
                }))
            }
            TypedDeclaration::Function(_)
            | TypedDeclaration::Const(_)
            | TypedDeclaration::Test(_)
            | TypedDeclaration::Extern(_) => None,
        })
        .collect()
}

fn inspect_type_declarations(module: &AnalyzedModule) -> Vec<Value> {
    let source_file = module.file_path.to_string_lossy().to_string();

    module
        .typed_program
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| match declaration {
            TypedDeclaration::Function(function) => Some(serde_json::json!({
                "name": function.name,
                "kind": "function",
                "type": module
                    .declaration_schemes
                    .get(&function.name)
                    .map(format_type_scheme)
                    .or_else(|| {
                        module
                            .declaration_types
                            .get(&function.name)
                            .map(sigil_typechecker::format_type)
                    })
                    .unwrap_or_else(|| sigil_typechecker::format_type(&function.return_type)),
                "spanId": module.declaration_span_ids.get(index).and_then(|span_id| span_id.clone()).unwrap_or_default(),
                "location": source_location_json(&source_file, function.location)
            })),
            TypedDeclaration::Const(const_decl) => Some(serde_json::json!({
                "name": const_decl.name,
                "kind": "const",
                "type": module
                    .declaration_schemes
                    .get(&const_decl.name)
                    .map(format_type_scheme)
                    .unwrap_or_else(|| sigil_typechecker::format_type(&const_decl.typ)),
                "spanId": module.declaration_span_ids.get(index).and_then(|span_id| span_id.clone()).unwrap_or_default(),
                "location": source_location_json(&source_file, const_decl.location)
            })),
            TypedDeclaration::Test(test_decl) => Some(serde_json::json!({
                "name": test_decl.description,
                "kind": "test",
                "type": format_test_signature(test_decl),
                "spanId": module.declaration_span_ids.get(index).and_then(|span_id| span_id.clone()).unwrap_or_default(),
                "location": source_location_json(&source_file, test_decl.location)
            })),
            TypedDeclaration::Type(_) | TypedDeclaration::Extern(_) => None,
        })
        .collect()
}

fn inspect_type_summary(declarations: &[Value], types: &[Value]) -> Value {
    let mut functions = 0usize;
    let mut consts = 0usize;
    let mut tests = 0usize;
    let mut aliases = 0usize;
    let mut products = 0usize;
    let mut sums = 0usize;
    let mut constrained_types = 0usize;

    for declaration in declarations {
        match declaration["kind"].as_str() {
            Some("function") => functions += 1,
            Some("const") => consts += 1,
            Some("test") => tests += 1,
            _ => {}
        }
    }

    for type_entry in types {
        match type_entry["kind"].as_str() {
            Some("alias") => aliases += 1,
            Some("product") => products += 1,
            Some("sum") => sums += 1,
            _ => {}
        }
        if type_entry["constrained"].as_bool().unwrap_or(false) {
            constrained_types += 1;
        }
    }

    json!({
        "declarations": declarations.len(),
        "functions": functions,
        "consts": consts,
        "tests": tests,
        "types": types.len(),
        "aliases": aliases,
        "products": products,
        "sums": sums,
        "constrainedTypes": constrained_types
    })
}

fn inspect_types_file_result(input: &Path, module: &AnalyzedModule) -> Value {
    let declarations = inspect_type_declarations(module);
    let types = inspect_named_types(module);
    json!({
        "input": input.to_string_lossy(),
        "moduleId": module.module_id,
        "sourceFile": module.file_path.to_string_lossy(),
        "project": project_json(module.project.as_ref()),
        "summary": inspect_type_summary(&declarations, &types),
        "declarations": declarations,
        "types": types
    })
}

fn inspect_proof_site(
    kind: &str,
    source_file: &str,
    owner_kind: &str,
    owner_name: &str,
    location: SourceLocation,
    predicate: Option<&Expr>,
    pattern: Option<&Pattern>,
) -> Value {
    json!({
        "kind": kind,
        "ownerKind": owner_kind,
        "ownerName": owner_name,
        "location": source_location_json(source_file, location),
        "predicateSource": predicate.map(print_canonical_expr),
        "predicateAst": predicate.map(inspect_expr_ast),
        "patternSource": pattern.map(print_pattern_source),
        "patternAst": pattern.map(inspect_pattern_ast)
    })
}

fn print_pattern_source(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Literal(literal) => match &literal.value {
            sigil_ast::PatternLiteralValue::Int(value) => value.to_string(),
            sigil_ast::PatternLiteralValue::Float(value) => value.to_string(),
            sigil_ast::PatternLiteralValue::String(value) => format!("{:?}", value),
            sigil_ast::PatternLiteralValue::Char(value) => format!("'{}'", value),
            sigil_ast::PatternLiteralValue::Bool(value) => value.to_string(),
            sigil_ast::PatternLiteralValue::Unit => "()".to_string(),
        },
        Pattern::Identifier(identifier) => identifier.name.clone(),
        Pattern::Wildcard(_) => "_".to_string(),
        Pattern::Constructor(constructor) => {
            let name = if constructor.module_path.is_empty() {
                constructor.name.clone()
            } else {
                format!(
                    "{}::{}",
                    constructor.module_path.join("::"),
                    constructor.name
                )
            };
            if constructor.patterns.is_empty() {
                format!("{}()", name)
            } else {
                format!(
                    "{}({})",
                    name,
                    constructor
                        .patterns
                        .iter()
                        .map(print_pattern_source)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        }
        Pattern::List(list) => {
            let mut parts = list
                .patterns
                .iter()
                .map(print_pattern_source)
                .collect::<Vec<_>>();
            if let Some(rest) = &list.rest {
                parts.push(format!(".{}", rest));
            }
            format!("[{}]", parts.join(","))
        }
        Pattern::Record(record) => format!(
            "{{{}}}",
            record
                .fields
                .iter()
                .map(|field| match &field.pattern {
                    Some(pattern) => format!("{}:{}", field.name, print_pattern_source(pattern)),
                    None => field.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
        Pattern::Tuple(tuple) => format!(
            "({})",
            tuple
                .patterns
                .iter()
                .map(print_pattern_source)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn collect_expr_proof_sites(
    expr: &Expr,
    source_file: &str,
    owner_kind: &str,
    owner_name: &str,
    out: &mut Vec<Value>,
) {
    match expr {
        Expr::Match(match_expr) => {
            for arm in &match_expr.arms {
                out.push(inspect_proof_site(
                    "matchArm",
                    source_file,
                    owner_kind,
                    owner_name,
                    arm.location,
                    arm.guard.as_ref(),
                    Some(&arm.pattern),
                ));
                if let Some(guard) = &arm.guard {
                    collect_expr_proof_sites(guard, source_file, owner_kind, owner_name, out);
                }
                collect_expr_proof_sites(&arm.body, source_file, owner_kind, owner_name, out);
            }
            collect_expr_proof_sites(
                &match_expr.scrutinee,
                source_file,
                owner_kind,
                owner_name,
                out,
            );
        }
        Expr::If(if_expr) => {
            out.push(inspect_proof_site(
                "ifCondition",
                source_file,
                owner_kind,
                owner_name,
                if_expr.location,
                Some(&if_expr.condition),
                None,
            ));
            collect_expr_proof_sites(&if_expr.condition, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(
                &if_expr.then_branch,
                source_file,
                owner_kind,
                owner_name,
                out,
            );
            if let Some(else_branch) = &if_expr.else_branch {
                collect_expr_proof_sites(else_branch, source_file, owner_kind, owner_name, out);
            }
        }
        Expr::Lambda(lambda) => {
            collect_expr_proof_sites(&lambda.body, source_file, owner_kind, owner_name, out)
        }
        Expr::Application(application) => {
            collect_expr_proof_sites(&application.func, source_file, owner_kind, owner_name, out);
            for arg in &application.args {
                collect_expr_proof_sites(arg, source_file, owner_kind, owner_name, out);
            }
        }
        Expr::Binary(binary) => {
            collect_expr_proof_sites(&binary.left, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&binary.right, source_file, owner_kind, owner_name, out);
        }
        Expr::Unary(unary) => {
            collect_expr_proof_sites(&unary.operand, source_file, owner_kind, owner_name, out);
        }
        Expr::Let(let_expr) => {
            collect_expr_proof_sites(&let_expr.value, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&let_expr.body, source_file, owner_kind, owner_name, out);
        }
        Expr::Using(using_expr) => {
            collect_expr_proof_sites(&using_expr.value, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&using_expr.body, source_file, owner_kind, owner_name, out);
        }
        Expr::List(list) => {
            for item in &list.elements {
                collect_expr_proof_sites(item, source_file, owner_kind, owner_name, out);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                collect_expr_proof_sites(&field.value, source_file, owner_kind, owner_name, out);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                collect_expr_proof_sites(&entry.key, source_file, owner_kind, owner_name, out);
                collect_expr_proof_sites(&entry.value, source_file, owner_kind, owner_name, out);
            }
        }
        Expr::Tuple(tuple) => {
            for item in &tuple.elements {
                collect_expr_proof_sites(item, source_file, owner_kind, owner_name, out);
            }
        }
        Expr::FieldAccess(field_access) => collect_expr_proof_sites(
            &field_access.object,
            source_file,
            owner_kind,
            owner_name,
            out,
        ),
        Expr::Index(index) => {
            collect_expr_proof_sites(&index.object, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&index.index, source_file, owner_kind, owner_name, out);
        }
        Expr::Pipeline(pipeline) => {
            collect_expr_proof_sites(&pipeline.left, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&pipeline.right, source_file, owner_kind, owner_name, out);
        }
        Expr::Map(map_expr) => {
            collect_expr_proof_sites(&map_expr.list, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&map_expr.func, source_file, owner_kind, owner_name, out);
        }
        Expr::Filter(filter) => {
            collect_expr_proof_sites(&filter.list, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&filter.predicate, source_file, owner_kind, owner_name, out);
        }
        Expr::Fold(fold) => {
            collect_expr_proof_sites(&fold.list, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&fold.func, source_file, owner_kind, owner_name, out);
            collect_expr_proof_sites(&fold.init, source_file, owner_kind, owner_name, out);
        }
        Expr::Concurrent(concurrent) => {
            collect_expr_proof_sites(&concurrent.width, source_file, owner_kind, owner_name, out);
            if let Some(policy) = &concurrent.policy {
                for field in &policy.fields {
                    collect_expr_proof_sites(
                        &field.value,
                        source_file,
                        owner_kind,
                        owner_name,
                        out,
                    );
                }
            }
            for step in &concurrent.steps {
                match step {
                    sigil_ast::ConcurrentStep::Spawn(spawn) => {
                        collect_expr_proof_sites(
                            &spawn.expr,
                            source_file,
                            owner_kind,
                            owner_name,
                            out,
                        );
                    }
                    sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                        collect_expr_proof_sites(
                            &spawn_each.func,
                            source_file,
                            owner_kind,
                            owner_name,
                            out,
                        );
                        collect_expr_proof_sites(
                            &spawn_each.list,
                            source_file,
                            owner_kind,
                            owner_name,
                            out,
                        );
                    }
                }
            }
        }
        Expr::TypeAscription(ascription) => {
            collect_expr_proof_sites(&ascription.expr, source_file, owner_kind, owner_name, out);
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => {}
    }
}

fn inspect_proof_sites(module: &AnalyzedModule) -> Vec<Value> {
    let source_file = module.file_path.to_string_lossy().to_string();
    let mut sites = Vec::new();

    for declaration in &module.ast.declarations {
        match declaration {
            Declaration::Type(type_decl) => {
                if let Some(constraint) = &type_decl.constraint {
                    sites.push(inspect_proof_site(
                        "typeConstraint",
                        &source_file,
                        "type",
                        &type_decl.name,
                        type_decl.location,
                        Some(constraint),
                        None,
                    ));
                }
            }
            Declaration::Function(function) => {
                if let Some(requires) = &function.requires {
                    sites.push(inspect_proof_site(
                        "requires",
                        &source_file,
                        "function",
                        &function.name,
                        function.location,
                        Some(requires),
                        None,
                    ));
                }
                if let Some(ensures) = &function.ensures {
                    sites.push(inspect_proof_site(
                        "ensures",
                        &source_file,
                        "function",
                        &function.name,
                        function.location,
                        Some(ensures),
                        None,
                    ));
                }
                collect_expr_proof_sites(
                    &function.body,
                    &source_file,
                    "function",
                    &function.name,
                    &mut sites,
                );
            }
            Declaration::Test(test_decl) => {
                collect_expr_proof_sites(
                    &test_decl.body,
                    &source_file,
                    "test",
                    &test_decl.description,
                    &mut sites,
                );
            }
            Declaration::FeatureFlag(feature_flag_decl) => {
                collect_expr_proof_sites(
                    &feature_flag_decl.default,
                    &source_file,
                    "featureFlag",
                    &feature_flag_decl.name,
                    &mut sites,
                );
            }
            Declaration::Effect(_)
            | Declaration::Protocol(_)
            | Declaration::Const(_)
            | Declaration::Extern(_)
            | Declaration::Transform(_)
            | Declaration::Label(_)
            | Declaration::Rule(_) => {}
        }
    }

    sites
}

fn inspect_proof_summary(sites: &[Value]) -> Value {
    let mut type_constraints = 0usize;
    let mut requires = 0usize;
    let mut ensures = 0usize;
    let mut match_arms = 0usize;
    let mut if_conditions = 0usize;

    for site in sites {
        match site["kind"].as_str() {
            Some("typeConstraint") => type_constraints += 1,
            Some("requires") => requires += 1,
            Some("ensures") => ensures += 1,
            Some("matchArm") => match_arms += 1,
            Some("ifCondition") => if_conditions += 1,
            _ => {}
        }
    }

    json!({
        "sites": sites.len(),
        "typeConstraints": type_constraints,
        "requires": requires,
        "ensures": ensures,
        "matchArms": match_arms,
        "ifConditions": if_conditions
    })
}

fn inspect_proof_file_result(input: &Path, module: &AnalyzedModule) -> Value {
    let sites = inspect_proof_sites(module);
    json!({
        "input": input.to_string_lossy(),
        "moduleId": module.module_id,
        "sourceFile": module.file_path.to_string_lossy(),
        "project": project_json(module.project.as_ref()),
        "proofFragment": {
            "constructs": [
                "type_where_constraints",
                "function_requires",
                "function_ensures",
                "match_patterns",
                "match_guards",
                "if_conditions"
            ]
        },
        "summary": inspect_proof_summary(&sites),
        "sites": sites
    })
}

fn inspect_proof_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        inspect_proof_directory_command(path, selected_env, ignore_paths, ignore_from)
    } else {
        inspect_proof_single_file_command(path, selected_env)
    }
}

fn inspect_proof_single_file_command(
    file: &Path,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    let graph = match ModuleGraph::build_with_env(file, selected_env) {
        Ok(graph) => graph,
        Err(error) => {
            output_inspect_error(
                InspectMode::Proof.command_name(),
                file,
                &CliError::ModuleGraph(error),
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let module_id = match entry_module_key(file) {
        Ok(module_id) => module_id,
        Err(error) => {
            output_inspect_error(
                InspectMode::Proof.command_name(),
                file,
                &CliError::ModuleGraph(error),
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let analyzed = match analyze_module_graph(&graph) {
        Ok(analyzed) => analyzed,
        Err(error) => {
            output_inspect_error(
                InspectMode::Proof.command_name(),
                file,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let module = analyzed.modules.get(&module_id).ok_or_else(|| {
        CliError::Codegen(format!(
            "inspect proof could not resolve requested module '{}'",
            file.display()
        ))
    })?;

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Proof.command_name(),
        "ok": true,
        "phase": InspectMode::Proof.phase(),
        "data": inspect_proof_file_result(file, module)
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_proof_directory_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files =
        match collect_sigil_targets(InspectMode::Proof.verb(), path, ignore_paths, ignore_from) {
            Ok(files) => files,
            Err(error) => {
                output_inspect_error(
                    InspectMode::Proof.command_name(),
                    path,
                    &error,
                    serde_json::Map::new(),
                );
                return Err(CliError::Reported(1));
            }
        };
    let groups = match group_compile_targets(&files) {
        Ok(groups) => groups,
        Err(error) => {
            output_inspect_error(
                InspectMode::Proof.command_name(),
                path,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let group_count = groups.len();
    let file_order = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut inspected_file_count = 0usize;
    let mut compiled_module_count = 0usize;
    let mut file_results = Vec::new();

    for group in groups {
        let first_file = group
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| path.to_path_buf());
        let graph = match ModuleGraph::build_many_with_env(&group.files, selected_env) {
            Ok(graph) => graph,
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(
                    InspectMode::Proof.command_name(),
                    &first_file,
                    &CliError::ModuleGraph(error),
                    extra,
                );
                return Err(CliError::Reported(1));
            }
        };
        let analyzed = match analyze_module_graph(&graph) {
            Ok(analyzed) => analyzed,
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(
                    InspectMode::Proof.command_name(),
                    &first_file,
                    &error,
                    extra,
                );
                return Err(CliError::Reported(1));
            }
        };
        compiled_module_count += analyzed.compiled_modules;

        for file in &group.files {
            let module_id = match entry_module_key(file) {
                Ok(module_id) => module_id,
                Err(error) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert(
                        "input".to_string(),
                        json!(path.to_string_lossy().to_string()),
                    );
                    extra.insert("discovered".to_string(), json!(files.len()));
                    extra.insert("inspected".to_string(), json!(inspected_file_count));
                    extra.insert(
                        "durationMs".to_string(),
                        json!(start_time.elapsed().as_millis()),
                    );
                    output_inspect_error(
                        InspectMode::Proof.command_name(),
                        file,
                        &CliError::ModuleGraph(error),
                        extra,
                    );
                    return Err(CliError::Reported(1));
                }
            };
            let module = analyzed.modules.get(&module_id).ok_or_else(|| {
                CliError::Codegen(format!(
                    "inspect proof did not produce results for '{}'",
                    file.display()
                ))
            })?;
            file_results.push(inspect_proof_file_result(file, module));
            inspected_file_count += 1;
        }
    }

    file_results.sort_by_key(|result| {
        result["input"]
            .as_str()
            .and_then(|input| file_order.get(Path::new(input)).copied())
            .unwrap_or(usize::MAX)
    });

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Proof.command_name(),
        "ok": true,
        "phase": InspectMode::Proof.phase(),
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "inspected": inspected_file_count,
                "groups": group_count,
                "modules": compiled_module_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn read_and_parse_program(file: &Path) -> Result<(String, Program), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|error| CliError::Lexer(error.to_string()))?;
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser
        .parse()
        .map_err(|error| CliError::Parser(error.to_string()))?;
    Ok((source, ast))
}

fn inspect_validate_file_result(file: &Path) -> Result<serde_json::Value, CliError> {
    validate_project_entrypoint_for_path(file)?;
    let (source, ast) = read_and_parse_program(file)?;
    let effect_catalog = load_project_effect_catalog_for(file)?;
    let canonical_source = print_canonical_program_with_effects(&ast, effect_catalog.as_ref());
    let validation_errors = validate_canonical_form_with_options(
        &ast,
        Some(file.to_string_lossy().as_ref()),
        Some(&source),
        ValidationOptions { effect_catalog },
    )
    .err()
    .unwrap_or_default();
    let validation_ok = validation_errors.is_empty();

    Ok(serde_json::json!({
        "input": file.to_string_lossy(),
        "sourceFile": file.to_string_lossy(),
        "project": project_json(get_project_config(file)?.as_ref()),
        "alreadyCanonical": validation_ok && source == canonical_source,
        "canonicalSource": canonical_source,
        "summary": ast_declaration_summary(&ast),
        "validation": {
            "ok": validation_ok,
            "errors": validation_errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
        }
    }))
}

fn inspect_codegen_module_inventory(
    graph: &ModuleGraph,
    generated: &GeneratedGraphOutputs,
) -> Result<Vec<serde_json::Value>, CliError> {
    graph
        .topo_order
        .iter()
        .map(|module_id| {
            let module = graph.modules.get(module_id).ok_or_else(|| {
                CliError::Codegen(format!(
                    "inspect codegen could not resolve module '{}'",
                    module_id
                ))
            })?;
            let output = generated.module_outputs.get(module_id).ok_or_else(|| {
                CliError::Codegen(format!(
                    "inspect codegen did not produce output for '{}'",
                    module.file_path.display()
                ))
            })?;
            Ok(serde_json::json!({
                "moduleId": module_id,
                "sourceFile": module.file_path.to_string_lossy(),
                "outputFile": output.output_path.to_string_lossy(),
                "spanMapFile": output.span_map_path.to_string_lossy()
            }))
        })
        .collect()
}

fn span_map_generated_range_count(span_map: &ModuleSpanMap) -> usize {
    span_map
        .spans
        .iter()
        .filter(|span| span.generated_range.is_some())
        .count()
}

fn span_map_top_level_anchor_count(span_map: &ModuleSpanMap) -> usize {
    span_map
        .spans
        .iter()
        .filter(|span| span.parent_span_id.is_none() && span.generated_range.is_some())
        .count()
}

fn inspect_codegen_line_count(source: &str) -> usize {
    if source.is_empty() {
        0
    } else {
        source.lines().count()
    }
}

fn inspect_codegen_file_result(
    input: &Path,
    graph: &ModuleGraph,
    generated: &GeneratedGraphOutputs,
    module_id: &str,
) -> Result<serde_json::Value, CliError> {
    let module = graph.modules.get(module_id).ok_or_else(|| {
        CliError::Codegen(format!(
            "inspect codegen could not resolve requested module '{}'",
            input.display()
        ))
    })?;
    let output = generated.module_outputs.get(module_id).ok_or_else(|| {
        CliError::Codegen(format!(
            "inspect codegen did not produce output for '{}'",
            input.display()
        ))
    })?;
    let line_count = inspect_codegen_line_count(&output.ts_code);
    let span_map_summary = serde_json::json!({
        "formatVersion": output.span_map.format_version,
        "spans": output.span_map.spans.len(),
        "generatedRanges": span_map_generated_range_count(&output.span_map),
        "topLevelAnchors": span_map_top_level_anchor_count(&output.span_map)
    });
    let modules = inspect_codegen_module_inventory(graph, generated)?;

    Ok(serde_json::json!({
        "input": input.to_string_lossy(),
        "moduleId": module_id,
        "sourceFile": module.file_path.to_string_lossy(),
        "project": project_json(module.project.as_ref()),
        "summary": {
            "modules": modules.len(),
            "lineCount": line_count,
            "spans": output.span_map.spans.len(),
            "generatedRanges": span_map_generated_range_count(&output.span_map),
            "topLevelAnchors": span_map_top_level_anchor_count(&output.span_map)
        },
        "codegen": {
            "outputFile": output.output_path.to_string_lossy(),
            "spanMapFile": output.span_map_path.to_string_lossy(),
            "source": output.ts_code,
            "lineCount": line_count,
            "spanMapSummary": span_map_summary
        },
        "modules": modules
    }))
}

pub fn inspect_command(
    mode: InspectMode,
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    match mode {
        InspectMode::Types => inspect_types_command(path, selected_env, ignore_paths, ignore_from),
        InspectMode::Proof => inspect_proof_command(path, selected_env, ignore_paths, ignore_from),
        InspectMode::Validate => {
            inspect_validate_command(path, selected_env, ignore_paths, ignore_from)
        }
        InspectMode::Codegen => {
            inspect_codegen_command(path, selected_env, ignore_paths, ignore_from)
        }
        InspectMode::World => inspect_world_command(path, selected_env),
    }
}

fn inspect_codegen_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        inspect_codegen_directory_command(path, selected_env, ignore_paths, ignore_from)
    } else {
        inspect_codegen_single_file_command(path, selected_env)
    }
}

fn inspect_codegen_single_file_command(
    file: &Path,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    let graph = match ModuleGraph::build_with_env(file, selected_env) {
        Ok(graph) => graph,
        Err(error) => {
            output_inspect_error(
                InspectMode::Codegen.command_name(),
                file,
                &CliError::ModuleGraph(error),
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let module_id = match entry_module_key(file) {
        Ok(module_id) => module_id,
        Err(error) => {
            output_inspect_error(
                InspectMode::Codegen.command_name(),
                file,
                &CliError::ModuleGraph(error),
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let generated = match generate_module_graph_outputs(
        &graph,
        None,
        false,
        false,
        false,
        OutputFlavor::TypeScript,
    ) {
        Ok(generated) => generated,
        Err(error) => {
            output_inspect_error(
                InspectMode::Codegen.command_name(),
                file,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let data = match inspect_codegen_file_result(file, &graph, &generated, &module_id) {
        Ok(data) => data,
        Err(error) => {
            output_inspect_error(
                InspectMode::Codegen.command_name(),
                file,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Codegen.command_name(),
        "ok": true,
        "phase": InspectMode::Codegen.phase(),
        "data": data
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_codegen_directory_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files =
        match collect_sigil_targets(InspectMode::Codegen.verb(), path, ignore_paths, ignore_from) {
            Ok(files) => files,
            Err(error) => {
                output_inspect_error(
                    InspectMode::Codegen.command_name(),
                    path,
                    &error,
                    serde_json::Map::new(),
                );
                return Err(CliError::Reported(1));
            }
        };
    let groups = match group_compile_targets(&files) {
        Ok(groups) => groups,
        Err(error) => {
            output_inspect_error(
                InspectMode::Codegen.command_name(),
                path,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let group_count = groups.len();
    let file_order = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut inspected_file_count = 0usize;
    let mut compiled_module_count = 0usize;
    let mut file_results = Vec::new();

    for group in groups {
        let first_file = group
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| path.to_path_buf());
        let graph = match ModuleGraph::build_many_with_env(&group.files, selected_env) {
            Ok(graph) => graph,
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(
                    InspectMode::Codegen.command_name(),
                    &first_file,
                    &CliError::ModuleGraph(error),
                    extra,
                );
                return Err(CliError::Reported(1));
            }
        };
        let generated = match generate_module_graph_outputs(
            &graph,
            None,
            false,
            false,
            false,
            OutputFlavor::TypeScript,
        ) {
            Ok(generated) => generated,
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(
                    InspectMode::Codegen.command_name(),
                    &first_file,
                    &error,
                    extra,
                );
                return Err(CliError::Reported(1));
            }
        };
        compiled_module_count += generated.module_outputs.len();

        for file in &group.files {
            let module_id = match entry_module_key(file) {
                Ok(module_id) => module_id,
                Err(error) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert(
                        "input".to_string(),
                        json!(path.to_string_lossy().to_string()),
                    );
                    extra.insert("discovered".to_string(), json!(files.len()));
                    extra.insert("inspected".to_string(), json!(inspected_file_count));
                    extra.insert(
                        "durationMs".to_string(),
                        json!(start_time.elapsed().as_millis()),
                    );
                    output_inspect_error(
                        InspectMode::Codegen.command_name(),
                        file,
                        &CliError::ModuleGraph(error),
                        extra,
                    );
                    return Err(CliError::Reported(1));
                }
            };
            let result = match inspect_codegen_file_result(file, &graph, &generated, &module_id) {
                Ok(result) => result,
                Err(error) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert(
                        "input".to_string(),
                        json!(path.to_string_lossy().to_string()),
                    );
                    extra.insert("discovered".to_string(), json!(files.len()));
                    extra.insert("inspected".to_string(), json!(inspected_file_count));
                    extra.insert(
                        "durationMs".to_string(),
                        json!(start_time.elapsed().as_millis()),
                    );
                    output_inspect_error(InspectMode::Codegen.command_name(), file, &error, extra);
                    return Err(CliError::Reported(1));
                }
            };
            file_results.push(result);
            inspected_file_count += 1;
        }
    }

    file_results.sort_by_key(|result| {
        result["input"]
            .as_str()
            .and_then(|input| file_order.get(Path::new(input)).copied())
            .unwrap_or(usize::MAX)
    });

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Codegen.command_name(),
        "ok": true,
        "phase": InspectMode::Codegen.phase(),
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "inspected": inspected_file_count,
                "groups": group_count,
                "modules": compiled_module_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_types_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        inspect_types_directory_command(path, selected_env, ignore_paths, ignore_from)
    } else {
        inspect_types_single_file_command(path, selected_env)
    }
}

fn inspect_types_single_file_command(
    file: &Path,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    let graph = match ModuleGraph::build_with_env(file, selected_env) {
        Ok(graph) => graph,
        Err(error) => {
            output_inspect_error(
                InspectMode::Types.command_name(),
                file,
                &CliError::ModuleGraph(error),
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let module_id = match entry_module_key(file) {
        Ok(module_id) => module_id,
        Err(error) => {
            output_inspect_error(
                InspectMode::Types.command_name(),
                file,
                &CliError::ModuleGraph(error),
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let analyzed = match analyze_module_graph(&graph) {
        Ok(analyzed) => analyzed,
        Err(error) => {
            output_inspect_error(
                InspectMode::Types.command_name(),
                file,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let module = analyzed.modules.get(&module_id).ok_or_else(|| {
        CliError::Codegen(format!(
            "inspect types could not resolve requested module '{}'",
            file.display()
        ))
    })?;

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Types.command_name(),
        "ok": true,
        "phase": InspectMode::Types.phase(),
        "data": inspect_types_file_result(file, module)
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_types_directory_command(
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files =
        match collect_sigil_targets(InspectMode::Types.verb(), path, ignore_paths, ignore_from) {
            Ok(files) => files,
            Err(error) => {
                output_inspect_error(
                    InspectMode::Types.command_name(),
                    path,
                    &error,
                    serde_json::Map::new(),
                );
                return Err(CliError::Reported(1));
            }
        };
    let groups = match group_compile_targets(&files) {
        Ok(groups) => groups,
        Err(error) => {
            output_inspect_error(
                InspectMode::Types.command_name(),
                path,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };
    let group_count = groups.len();
    let file_order = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut inspected_file_count = 0usize;
    let mut compiled_module_count = 0usize;
    let mut file_results = Vec::new();

    for group in groups {
        let first_file = group
            .files
            .first()
            .cloned()
            .unwrap_or_else(|| path.to_path_buf());
        let graph = match ModuleGraph::build_many_with_env(&group.files, selected_env) {
            Ok(graph) => graph,
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(
                    InspectMode::Types.command_name(),
                    &first_file,
                    &CliError::ModuleGraph(error),
                    extra,
                );
                return Err(CliError::Reported(1));
            }
        };
        let analyzed = match analyze_module_graph(&graph) {
            Ok(analyzed) => analyzed,
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(
                    InspectMode::Types.command_name(),
                    &first_file,
                    &error,
                    extra,
                );
                return Err(CliError::Reported(1));
            }
        };
        compiled_module_count += analyzed.compiled_modules;

        for file in &group.files {
            let module_id = match entry_module_key(file) {
                Ok(module_id) => module_id,
                Err(error) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert(
                        "input".to_string(),
                        json!(path.to_string_lossy().to_string()),
                    );
                    extra.insert("discovered".to_string(), json!(files.len()));
                    extra.insert("inspected".to_string(), json!(inspected_file_count));
                    extra.insert(
                        "durationMs".to_string(),
                        json!(start_time.elapsed().as_millis()),
                    );
                    output_inspect_error(
                        InspectMode::Types.command_name(),
                        file,
                        &CliError::ModuleGraph(error),
                        extra,
                    );
                    return Err(CliError::Reported(1));
                }
            };
            let module = analyzed.modules.get(&module_id).ok_or_else(|| {
                CliError::Codegen(format!(
                    "inspect types did not produce results for '{}'",
                    file.display()
                ))
            })?;
            file_results.push(inspect_types_file_result(file, module));
            inspected_file_count += 1;
        }
    }

    file_results.sort_by_key(|result| {
        result["input"]
            .as_str()
            .and_then(|input| file_order.get(Path::new(input)).copied())
            .unwrap_or(usize::MAX)
    });

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Types.command_name(),
        "ok": true,
        "phase": InspectMode::Types.phase(),
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "inspected": inspected_file_count,
                "groups": group_count,
                "modules": compiled_module_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_validate_command(
    path: &Path,
    _selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        inspect_validate_directory_command(path, ignore_paths, ignore_from)
    } else {
        inspect_validate_single_file_command(path)
    }
}

fn inspect_validate_single_file_command(file: &Path) -> Result<(), CliError> {
    let data = match inspect_validate_file_result(file) {
        Ok(data) => data,
        Err(error) => {
            output_inspect_error(
                InspectMode::Validate.command_name(),
                file,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Validate.command_name(),
        "ok": true,
        "phase": InspectMode::Validate.phase(),
        "data": data
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_validate_directory_command(
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files = match collect_sigil_targets(
        InspectMode::Validate.verb(),
        path,
        ignore_paths,
        ignore_from,
    ) {
        Ok(files) => files,
        Err(error) => {
            output_inspect_error(
                InspectMode::Validate.command_name(),
                path,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };

    if let Err(error) = validate_project_entrypoints_for_files(&files) {
        output_inspect_error(
            InspectMode::Validate.command_name(),
            path,
            &error,
            serde_json::Map::from_iter([
                (
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                ),
                ("discovered".to_string(), json!(files.len())),
                ("inspected".to_string(), json!(0)),
                (
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                ),
            ]),
        );
        return Err(CliError::Reported(1));
    }

    let mut inspected_file_count = 0usize;
    let mut file_results = Vec::new();

    for file in &files {
        match inspect_validate_file_result(file) {
            Ok(result) => {
                file_results.push(result);
                inspected_file_count += 1;
            }
            Err(error) => {
                let mut extra = serde_json::Map::new();
                extra.insert(
                    "input".to_string(),
                    json!(path.to_string_lossy().to_string()),
                );
                extra.insert("discovered".to_string(), json!(files.len()));
                extra.insert("inspected".to_string(), json!(inspected_file_count));
                extra.insert(
                    "durationMs".to_string(),
                    json!(start_time.elapsed().as_millis()),
                );
                output_inspect_error(InspectMode::Validate.command_name(), file, &error, extra);
                return Err(CliError::Reported(1));
            }
        }
    }

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::Validate.command_name(),
        "ok": true,
        "phase": InspectMode::Validate.phase(),
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "inspected": inspected_file_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

pub fn inspect_world_command(path: &Path, env: Option<&str>) -> Result<(), CliError> {
    let data = match inspect_world_result(path, env) {
        Ok(data) => data,
        Err(error) => {
            output_inspect_error(
                InspectMode::World.command_name(),
                path,
                &error,
                serde_json::Map::new(),
            );
            return Err(CliError::Reported(1));
        }
    };

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": InspectMode::World.command_name(),
        "ok": true,
        "phase": InspectMode::World.phase(),
        "data": data
    });
    println!("{}", serde_json::to_string(&output).unwrap());
    Ok(())
}

fn inspect_world_result(path: &Path, env: Option<&str>) -> Result<serde_json::Value, CliError> {
    if let Some(project) = get_project_config(path)? {
        let env = env.ok_or_else(|| {
            CliError::Validation(format!(
                "{}: inspect world requires --env <name> for Sigil projects",
                codes::topology::ENV_REQUIRED
            ))
        })?;
        let topology_file = topology_source_path(&project.root);
        let topology_present = topology_file.exists();

        let prelude = build_world_runtime_prelude(&project.root, env, topology_present)?;
        let runner_path = unique_world_inspect_runner_path(&project.root);
        if let Some(parent) = runner_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(
            &runner_path,
            format!(
                r#"{world_helpers}
{prelude}
const __sigil_inspect_topology = __sigil_world_collect_topology(globalThis.__sigil_topology_exports ?? null);
const __sigil_inspect_world = __sigil_world_prepare_template(
  globalThis.__sigil_world_value,
  globalThis.__sigil_topology_exports ?? null,
  globalThis.__sigil_world_env_name ?? null
);
console.log(JSON.stringify({{
  "topology": {{
    "present": Boolean(globalThis.__sigil_topology_exports),
    "declaredEnvs": Array.from(__sigil_inspect_topology.envs).sort(),
    "httpDependencies": Array.from(__sigil_inspect_topology.http).sort(),
    "sqlHandles": Array.from(__sigil_inspect_topology.sqlHandles ?? []).sort(),
    "tcpDependencies": Array.from(__sigil_inspect_topology.tcp).sort()
  }},
  "summary": {{
    "clockKind": String(__sigil_inspect_world.clock?.kind ?? ""),
    "fsKind": String(__sigil_inspect_world.fs?.kind ?? ""),
    "fsWatchKind": String(__sigil_inspect_world.fsWatch?.kind ?? ""),
    "logKind": String(__sigil_inspect_world.log?.kind ?? ""),
    "ptyKind": String(__sigil_inspect_world.pty?.kind ?? ""),
    "processKind": String(__sigil_inspect_world.process?.kind ?? ""),
    "randomKind": String(__sigil_inspect_world.random?.kind ?? ""),
    "sqlKind": String(__sigil_inspect_world.sql?.kind ?? ""),
    "streamKind": String(__sigil_inspect_world.stream?.kind ?? ""),
    "timerKind": String(__sigil_inspect_world.timer?.kind ?? ""),
    "websocketKind": String(__sigil_inspect_world.websocket?.kind ?? ""),
    "httpBindings": Object.keys(__sigil_inspect_world.http ?? {{}}).length,
    "sqlBindings": Object.keys(__sigil_inspect_world.sqlHandles ?? {{}}).length,
    "tcpBindings": Object.keys(__sigil_inspect_world.tcp ?? {{}}).length
  }},
  "normalizedWorld": __sigil_inspect_world
}}));
"#,
                world_helpers = world_runtime_helpers_source(),
                prelude = prelude
            ),
        )?;

        let runner_json = run_world_inspect_runner(&runner_path, &project.root)?;

        return Ok(serde_json::json!({
            "input": path.to_string_lossy(),
            "project": project_json(Some(&project)),
            "projectRoot": project.root.to_string_lossy(),
            "environment": env,
            "topology": runner_json["topology"].clone(),
            "summary": runner_json["summary"].clone(),
            "normalizedWorld": runner_json["normalizedWorld"].clone()
        }));
    }

    if path.is_dir() {
        return Err(CliError::Validation(format!(
            "{}: inspect world on non-project paths expects a single .sigil file",
            codes::topology::MISSING_MODULE
        )));
    }

    if env.is_some() {
        return Err(CliError::Validation(format!(
            "{}: --env is only valid for Sigil projects when inspecting runtime world",
            codes::cli::USAGE
        )));
    }

    let compiled = compile_entry_files_with_cache(
        &[path.to_path_buf()],
        None,
        None,
        env,
        false,
        false,
        false,
        OutputFlavor::RuntimeEsm,
    )?;
    let module_url = format!(
        "file://{}",
        fs::canonicalize(&compiled.entry_output_path)?.display()
    );
    let runner_path =
        unique_world_inspect_runner_path(path.parent().unwrap_or_else(|| Path::new(".")));
    if let Some(parent) = runner_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &runner_path,
        format!(
            r#"{world_helpers}
const __sigil_module = await import("{module_url}");
const __sigil_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);
const __sigil_inspect_topology = __sigil_world_collect_topology(__sigil_exports);
const __sigil_has_world = Object.prototype.hasOwnProperty.call(__sigil_exports, "world");
const __sigil_has_topology =
  __sigil_inspect_topology.envs.size > 0 ||
  __sigil_inspect_topology.fsRoots.size > 0 ||
  __sigil_inspect_topology.http.size > 0 ||
  __sigil_inspect_topology.logSinks.size > 0 ||
  __sigil_inspect_topology.ptyHandles.size > 0 ||
  __sigil_inspect_topology.processHandles.size > 0 ||
  __sigil_inspect_topology.sqlHandles.size > 0 ||
  __sigil_inspect_topology.tcp.size > 0 ||
  __sigil_inspect_topology.websocketHandles.size > 0;
if (!__sigil_has_world && __sigil_has_topology) {{
  const error = new Error("{local_world_required}: standalone topology programs must export c world");
  error.sigilCode = "{local_world_required}";
  throw error;
}}
const __sigil_inspect_world = __sigil_has_world
  ? __sigil_world_prepare_template(__sigil_exports.world, __sigil_exports, null)
  : __sigil_world_host_template();
console.log(JSON.stringify({{
  "topology": {{
    "present": __sigil_has_world,
    "declaredEnvs": Array.from(__sigil_inspect_topology.envs).sort(),
    "httpDependencies": Array.from(__sigil_inspect_topology.http).sort(),
    "sqlHandles": Array.from(__sigil_inspect_topology.sqlHandles ?? []).sort(),
    "tcpDependencies": Array.from(__sigil_inspect_topology.tcp).sort()
  }},
  "summary": {{
    "clockKind": String(__sigil_inspect_world.clock?.kind ?? ""),
    "fsKind": String(__sigil_inspect_world.fs?.kind ?? ""),
    "fsWatchKind": String(__sigil_inspect_world.fsWatch?.kind ?? ""),
    "logKind": String(__sigil_inspect_world.log?.kind ?? ""),
    "ptyKind": String(__sigil_inspect_world.pty?.kind ?? ""),
    "processKind": String(__sigil_inspect_world.process?.kind ?? ""),
    "randomKind": String(__sigil_inspect_world.random?.kind ?? ""),
    "sqlKind": String(__sigil_inspect_world.sql?.kind ?? ""),
    "streamKind": String(__sigil_inspect_world.stream?.kind ?? ""),
    "timerKind": String(__sigil_inspect_world.timer?.kind ?? ""),
    "websocketKind": String(__sigil_inspect_world.websocket?.kind ?? ""),
    "httpBindings": Object.keys(__sigil_inspect_world.http ?? {{}}).length,
    "sqlBindings": Object.keys(__sigil_inspect_world.sqlHandles ?? {{}}).length,
    "tcpBindings": Object.keys(__sigil_inspect_world.tcp ?? {{}}).length
  }},
  "normalizedWorld": __sigil_inspect_world
}}));
"#,
            world_helpers = world_runtime_helpers_source(),
            module_url = module_url,
            local_world_required = codes::topology::LOCAL_WORLD_REQUIRED
        ),
    )?;

    let runner_json = run_world_inspect_runner(
        &runner_path,
        path.parent().unwrap_or_else(|| Path::new(".")),
    )?;

    Ok(serde_json::json!({
        "input": path.to_string_lossy(),
        "environment": serde_json::Value::Null,
        "topology": runner_json["topology"].clone(),
        "summary": runner_json["summary"].clone(),
        "normalizedWorld": runner_json["normalizedWorld"].clone()
    }))
}

fn run_world_inspect_runner(
    runner_path: &Path,
    current_dir: &Path,
) -> Result<serde_json::Value, CliError> {
    let abs_runner = fs::canonicalize(runner_path)?;
    let output = Command::new("node")
        .current_dir(current_dir)
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime(
                    "node not found. Please install Node.js to inspect Sigil runtime worlds."
                        .to_string(),
                )
            } else {
                CliError::Runtime(format!("Failed to execute world inspection: {}", error))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(CliError::Validation(if message.is_empty() {
            "runtime world inspection failed".to_string()
        } else {
            message.to_string()
        }));
    }

    serde_json::from_slice::<serde_json::Value>(&output.stdout).map_err(|error| {
        CliError::Runtime(format!(
            "inspect world runner emitted invalid JSON: {}",
            error
        ))
    })
}

/// Run command: compile and execute a Sigil file
pub fn run_command(
    file: &Path,
    json_output: bool,
    trace_output: bool,
    trace_expr_output: bool,
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
    breakpoint_collect: bool,
    breakpoint_max_hits: usize,
    record_path: Option<&Path>,
    replay_path: Option<&Path>,
    selected_env: Option<&str>,
    args: &[String],
) -> Result<(), CliError> {
    let breakpoints_requested = !breakpoint_lines.is_empty()
        || !breakpoint_functions.is_empty()
        || !breakpoint_spans.is_empty();

    if trace_output && !json_output {
        output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::USAGE,
            "`--trace` requires `--json`",
            json!({
                "file": file.to_string_lossy(),
                "option": "--trace",
                "requires": "--json"
            }),
            true,
        );
        return Err(CliError::Reported(1));
    }

    if trace_expr_output && (!trace_output || !json_output) {
        output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::USAGE,
            "`--trace-expr` requires `--trace` and `--json`",
            json!({
                "file": file.to_string_lossy(),
                "option": "--trace-expr",
                "requires": ["--trace", "--json"]
            }),
            true,
        );
        return Err(CliError::Reported(1));
    }

    if breakpoints_requested && !json_output {
        output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::USAGE,
            "breakpoints require `--json`",
            json!({
                "file": file.to_string_lossy(),
                "option": "--break",
                "requires": "--json"
            }),
            true,
        );
        return Err(CliError::Reported(1));
    }

    if breakpoints_requested && breakpoint_max_hits == 0 {
        output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::USAGE,
            "`--break-max-hits` must be at least 1",
            json!({
                "file": file.to_string_lossy(),
                "option": "--break-max-hits",
                "minimum": 1
            }),
            !json_output,
        );
        return Err(CliError::Reported(1));
    }

    if replay_path.is_some() && selected_env.is_some() {
        output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::USAGE,
            "`--replay` cannot be combined with `--env`",
            json!({
                "file": file.to_string_lossy(),
                "option": "--replay",
                "conflictsWith": "--env"
            }),
            !json_output,
        );
        return Err(CliError::Reported(1));
    }

    let run_target = match build_run_target(
        file,
        json_output,
        selected_env,
        trace_output,
        trace_expr_output,
        breakpoints_requested,
        breakpoint_lines,
        breakpoint_functions,
        breakpoint_spans,
        if breakpoint_collect {
            BreakpointMode::Collect
        } else {
            BreakpointMode::Stop
        },
        breakpoint_max_hits,
        record_path,
        replay_path,
        args,
    ) {
        Ok(run_target) => run_target,
        Err(CliError::Breakpoint {
            code,
            message,
            details,
        }) => {
            output_json_error_to("sigilc run", "cli", &code, &message, details, !json_output);
            return Err(CliError::Reported(1));
        }
        Err(error) => {
            output_run_error(file, &error, !json_output);
            return Err(CliError::Reported(1));
        }
    };

    let runtime_output = match execute_runner(
        &run_target.runner_path,
        &run_target.runtime_error_path,
        run_target.runtime_trace_path.as_deref(),
        run_target.runtime_breakpoint_path.as_deref(),
        run_target.runtime_replay_path.as_deref(),
        None,
        args,
        !json_output,
    ) {
        Ok(runtime_output) => runtime_output,
        Err(error) => {
            output_run_error(file, &error, !json_output);
            return Err(CliError::Reported(1));
        }
    };

    if runtime_output.exit_code != 0 {
        let output_json = build_runtime_failure_output(file, &run_target, &runtime_output);
        output_json_value(&output_json, !json_output);
        return Err(CliError::Reported(1));
    }

    if json_output {
        let mut output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc run",
            "ok": true,
            "phase": "runtime",
            "data": {
                "compile": {
                    "input": file.to_string_lossy(),
                    "output": run_target.entry_output_path.to_string_lossy(),
                    "runnerFile": run_target.runner_path.to_string_lossy(),
                    "spanMapFile": run_target.entry_span_map_path.to_string_lossy()
                },
                "runtime": {
                    "engine": "node",
                    "exitCode": runtime_output.exit_code,
                    "durationMs": runtime_output.duration_ms,
                    "stdout": runtime_output.stdout,
                    "stderr": runtime_output.stderr
                },
                "trace": runtime_trace_json(runtime_output.trace_capture.as_ref()),
                "breakpoints": runtime_breakpoints_json(
                    run_target.breakpoint_config.as_ref(),
                    runtime_output.breakpoint_capture.as_ref(),
                    &run_target.module_debug_outputs
                ),
                "replay": runtime_replay_json(
                    run_target.replay_mode.as_deref(),
                    run_target.replay_file.as_deref(),
                    runtime_output.replay_capture.as_ref()
                )
            }
        });
        if !trace_output {
            if let Some(data) = output_json
                .get_mut("data")
                .and_then(|value| value.as_object_mut())
            {
                data.remove("trace");
            }
        }
        if run_target.replay_mode.is_none() {
            if let Some(data) = output_json
                .get_mut("data")
                .and_then(|value| value.as_object_mut())
            {
                data.remove("replay");
            }
        }
        if run_target.breakpoint_config.is_none() {
            if let Some(data) = output_json
                .get_mut("data")
                .and_then(|value| value.as_object_mut())
            {
                data.remove("breakpoints");
            }
        }
        output_json_value(&output_json, false);
    }

    Ok(())
}

struct RunTarget {
    entry_output_path: PathBuf,
    entry_span_map_path: PathBuf,
    runner_path: PathBuf,
    runtime_error_path: PathBuf,
    runtime_trace_path: Option<PathBuf>,
    runtime_breakpoint_path: Option<PathBuf>,
    runtime_replay_path: Option<PathBuf>,
    trace_enabled: bool,
    breakpoint_config: Option<ResolvedBreakpointConfig>,
    replay_mode: Option<String>,
    replay_file: Option<PathBuf>,
    module_debug_outputs: Vec<RuntimeModuleDebugOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakpointMode {
    Stop,
    Collect,
}

impl BreakpointMode {
    fn as_str(self) -> &'static str {
        match self {
            BreakpointMode::Stop => "stop",
            BreakpointMode::Collect => "collect",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeModuleDebugOutput {
    module_id: String,
    output_file: PathBuf,
    span_map: ModuleSpanMap,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedBreakpointSelector {
    kind: String,
    value: String,
}

#[derive(Debug, Clone)]
struct ResolvedBreakpointConfig {
    mode: BreakpointMode,
    max_hits: usize,
    spans: HashMap<String, Vec<ResolvedBreakpointSelector>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeExceptionCapture {
    name: String,
    message: String,
    stack: String,
    #[serde(default)]
    sigil_code: Option<String>,
    #[serde(default)]
    expression: Option<RuntimeExpressionCapture>,
}

struct RuntimeOutput {
    exit_code: i32,
    duration_ms: u128,
    stdout: String,
    stderr: String,
    exception_capture: Option<RuntimeExceptionCapture>,
    trace_capture: Option<RuntimeTraceCapture>,
    breakpoint_capture: Option<RuntimeBreakpointCapture>,
    replay_capture: Option<RuntimeReplayCapture>,
    step_capture: Option<RuntimeStepCapture>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTraceCapture {
    enabled: bool,
    truncated: bool,
    total_events: usize,
    returned_events: usize,
    dropped_events: usize,
    #[serde(default)]
    events: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeReplayCapture {
    mode: String,
    file: String,
    recorded_events: usize,
    consumed_events: usize,
    remaining_events: usize,
    partial: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStepCapture {
    state: String,
    pause_reason: String,
    event_kind: String,
    seq: usize,
    #[serde(default)]
    module_id: Option<String>,
    #[serde(default)]
    source_file: Option<String>,
    #[serde(default)]
    span_id: Option<String>,
    #[serde(default)]
    span_kind: Option<String>,
    #[serde(default)]
    declaration_kind: Option<String>,
    #[serde(default)]
    declaration_label: Option<String>,
    #[serde(default)]
    function_name: Option<String>,
    #[serde(default)]
    test_id: Option<String>,
    #[serde(default)]
    test_name: Option<String>,
    #[serde(default)]
    test_status: Option<String>,
    #[serde(default)]
    matched: Vec<ResolvedBreakpointSelector>,
    #[serde(default)]
    locals: Vec<RuntimeBreakpointLocalCapture>,
    #[serde(default)]
    stack: Vec<RuntimeBreakpointFrameCapture>,
    #[serde(default)]
    recent_trace: Vec<serde_json::Value>,
    #[serde(default)]
    frame_depth: usize,
    #[serde(default)]
    expression_depth: usize,
    #[serde(default)]
    last_completed: Option<serde_json::Value>,
    #[serde(default)]
    watches: Vec<RuntimeDebugWatchCapture>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeDebugWatchCapture {
    selector: String,
    status: String,
    #[serde(default)]
    value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBreakpointCapture {
    enabled: bool,
    mode: String,
    stopped: bool,
    truncated: bool,
    total_hits: usize,
    returned_hits: usize,
    dropped_hits: usize,
    max_hits: usize,
    #[serde(default)]
    hits: Vec<RuntimeBreakpointHitCapture>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBreakpointHitCapture {
    #[serde(default)]
    matched: Vec<ResolvedBreakpointSelector>,
    module_id: String,
    source_file: String,
    span_id: String,
    #[serde(default)]
    span_kind: Option<String>,
    #[serde(default)]
    declaration_kind: Option<String>,
    #[serde(default)]
    declaration_label: Option<String>,
    #[serde(default)]
    locals: Vec<RuntimeBreakpointLocalCapture>,
    #[serde(default)]
    stack: Vec<RuntimeBreakpointFrameCapture>,
    #[serde(default)]
    recent_trace: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeExpressionCapture {
    module_id: String,
    source_file: String,
    span_id: String,
    #[serde(default)]
    span_kind: Option<String>,
    #[serde(default)]
    declaration_kind: Option<String>,
    #[serde(default)]
    declaration_label: Option<String>,
    #[serde(default)]
    value: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
    #[serde(default)]
    locals: Vec<RuntimeBreakpointLocalCapture>,
    #[serde(default)]
    stack: Vec<RuntimeBreakpointFrameCapture>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBreakpointLocalCapture {
    name: String,
    origin: String,
    #[serde(default)]
    type_id: Option<String>,
    value: serde_json::Value,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBreakpointFrameCapture {
    module_id: String,
    source_file: String,
    span_id: String,
    #[serde(default)]
    declaration_kind: Option<String>,
    #[serde(default)]
    declaration_label: Option<String>,
    #[serde(default)]
    function_name: Option<String>,
}

fn parse_breakpoint_line_selector(selector: &str) -> Result<(PathBuf, usize), CliError> {
    let (raw_path, raw_line) = selector
        .rsplit_once(':')
        .ok_or_else(|| CliError::Breakpoint {
            code: codes::cli::USAGE.to_string(),
            message: format!("invalid breakpoint selector '{}'", selector),
            details: json!({
                "selector": selector,
                "expectedFormat": "FILE:LINE"
            }),
        })?;
    let line = raw_line
        .parse::<usize>()
        .map_err(|_| CliError::Breakpoint {
            code: codes::cli::USAGE.to_string(),
            message: format!("invalid breakpoint line '{}'", selector),
            details: json!({
                "selector": selector,
                "expectedFormat": "FILE:LINE"
            }),
        })?;
    if line == 0 {
        return Err(CliError::Breakpoint {
            code: codes::cli::USAGE.to_string(),
            message: format!("invalid breakpoint line '{}'", selector),
            details: json!({
                "selector": selector,
                "minimumLine": 1
            }),
        });
    }

    let path = Path::new(raw_path);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok((canonicalize_existing_path(&absolute), line))
}

fn breakpoint_selector_value(kind: &str, value: &str) -> ResolvedBreakpointSelector {
    ResolvedBreakpointSelector {
        kind: kind.to_string(),
        value: value.to_string(),
    }
}

fn breakpoint_span_matches_line(span: &DebugSpanRecord, source_file: &Path, line: usize) -> bool {
    canonicalize_existing_path(Path::new(&span.source_file)) == source_file
        && span.location.start.line <= line
        && span.location.end.line >= line
}

fn is_breakpoint_executable_span(span: &DebugSpanRecord) -> bool {
    matches!(
        span.kind,
        DebugSpanKind::FunctionDecl
            | DebugSpanKind::MatchArm
            | DebugSpanKind::ExprLiteral
            | DebugSpanKind::ExprIdentifier
            | DebugSpanKind::ExprNamespaceMember
            | DebugSpanKind::ExprLambda
            | DebugSpanKind::ExprCall
            | DebugSpanKind::ExprConstructorCall
            | DebugSpanKind::ExprExternCall
            | DebugSpanKind::ExprMethodCall
            | DebugSpanKind::ExprBinary
            | DebugSpanKind::ExprUnary
            | DebugSpanKind::ExprMatch
            | DebugSpanKind::ExprLet
            | DebugSpanKind::ExprIf
            | DebugSpanKind::ExprList
            | DebugSpanKind::ExprTuple
            | DebugSpanKind::ExprRecord
            | DebugSpanKind::ExprMapLiteral
            | DebugSpanKind::ExprFieldAccess
            | DebugSpanKind::ExprIndex
            | DebugSpanKind::ExprMap
            | DebugSpanKind::ExprFilter
            | DebugSpanKind::ExprFold
            | DebugSpanKind::ExprConcurrent
            | DebugSpanKind::ExprPipeline
    )
}

fn breakpoint_span_sort_key(span: &DebugSpanRecord) -> (usize, usize, usize, usize) {
    (
        span.location
            .end
            .line
            .saturating_sub(span.location.start.line),
        span.location
            .end
            .offset
            .saturating_sub(span.location.start.offset),
        span.location.start.line,
        span.location.start.column,
    )
}

fn resolved_breakpoint_config_json(config: &ResolvedBreakpointConfig) -> serde_json::Value {
    let spans = config
        .spans
        .iter()
        .map(|(span_id, selectors)| {
            (
                span_id.clone(),
                serde_json::to_value(selectors).unwrap_or_else(|_| json!([])),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "enabled": true,
        "mode": config.mode.as_str(),
        "maxHits": config.max_hits,
        "recentTraceLimit": 32,
        "spans": serde_json::Value::Object(spans)
    })
}

fn resolve_breakpoint_config(
    file: &Path,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
    mode: BreakpointMode,
    max_hits: usize,
) -> Result<Option<ResolvedBreakpointConfig>, CliError> {
    if breakpoint_lines.is_empty() && breakpoint_functions.is_empty() && breakpoint_spans.is_empty()
    {
        return Ok(None);
    }

    let mut spans = HashMap::<String, Vec<ResolvedBreakpointSelector>>::new();

    for selector in breakpoint_lines {
        let (source_file, line) = parse_breakpoint_line_selector(selector)?;
        let span = module_debug_outputs
            .iter()
            .flat_map(|module| module.span_map.spans.iter())
            .filter(|span| is_breakpoint_executable_span(span))
            .filter(|span| breakpoint_span_matches_line(span, &source_file, line))
            .min_by_key(|span| breakpoint_span_sort_key(span))
            .cloned()
            .ok_or_else(|| CliError::Breakpoint {
                code: codes::cli::BREAKPOINT_NOT_FOUND.to_string(),
                message: format!("no executable breakpoint found for '{}'", selector),
                details: json!({
                    "file": file.to_string_lossy(),
                    "selector": selector
                }),
            })?;
        spans
            .entry(span.span_id)
            .or_default()
            .push(breakpoint_selector_value("fileLine", selector));
    }

    for selector in breakpoint_functions {
        let matches = module_debug_outputs
            .iter()
            .flat_map(|module| module.span_map.spans.iter())
            .filter(|span| span.kind == DebugSpanKind::FunctionDecl)
            .filter(|span| span.label.as_deref() == Some(selector.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {
                return Err(CliError::Breakpoint {
                    code: codes::cli::BREAKPOINT_NOT_FOUND.to_string(),
                    message: format!("function breakpoint '{}' not found", selector),
                    details: json!({
                        "file": file.to_string_lossy(),
                        "selector": selector
                    }),
                });
            }
            [span] => {
                spans
                    .entry(span.span_id.clone())
                    .or_default()
                    .push(breakpoint_selector_value("function", selector));
            }
            _ => {
                return Err(CliError::Breakpoint {
                    code: codes::cli::BREAKPOINT_AMBIGUOUS.to_string(),
                    message: format!("function breakpoint '{}' is ambiguous", selector),
                    details: json!({
                        "file": file.to_string_lossy(),
                        "selector": selector,
                        "matches": matches
                            .iter()
                            .map(|span| json!({
                                "sourceFile": span.source_file,
                                "spanId": span.span_id,
                                "line": span.location.start.line
                            }))
                            .collect::<Vec<_>>()
                    }),
                });
            }
        }
    }

    for selector in breakpoint_spans {
        let span = module_debug_outputs
            .iter()
            .flat_map(|module| module.span_map.spans.iter())
            .find(|span| span.span_id == *selector && is_breakpoint_executable_span(span))
            .cloned()
            .ok_or_else(|| CliError::Breakpoint {
                code: codes::cli::BREAKPOINT_NOT_FOUND.to_string(),
                message: format!("breakpoint span '{}' not found", selector),
                details: json!({
                    "file": file.to_string_lossy(),
                    "selector": selector
                }),
            })?;
        spans
            .entry(span.span_id)
            .or_default()
            .push(breakpoint_selector_value("span", selector));
    }

    Ok(Some(ResolvedBreakpointConfig {
        mode,
        max_hits,
        spans,
    }))
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifact {
    format_version: u32,
    kind: String,
    entry: ReplayArtifactEntry,
    binding: ReplayArtifactBinding,
    world: ReplayArtifactWorld,
    summary: ReplayArtifactSummary,
    #[serde(default)]
    events: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<ReplayArtifactFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifactEntry {
    source_file: String,
    #[serde(default)]
    argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifactBinding {
    algorithm: String,
    fingerprint: String,
    modules: Vec<ReplayArtifactModule>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifactModule {
    module_id: String,
    source_file: String,
    source_hash: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifactWorld {
    normalized_world: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at_epoch_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifactSummary {
    failed: bool,
    recorded_events: usize,
    #[serde(default)]
    effect_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayArtifactFailure {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stack: Option<String>,
}

#[derive(Debug, Clone)]
enum PreparedReplayMode {
    Record {
        artifact_file: PathBuf,
        config: serde_json::Value,
    },
    Replay {
        artifact_file: PathBuf,
        config: serde_json::Value,
    },
}

fn build_run_target(
    file: &Path,
    json_output: bool,
    selected_env: Option<&str>,
    trace_enabled: bool,
    trace_expr_enabled: bool,
    breakpoints_requested: bool,
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
    breakpoint_mode: BreakpointMode,
    breakpoint_max_hits: usize,
    record_path: Option<&Path>,
    replay_path: Option<&Path>,
    args: &[String],
) -> Result<RunTarget, CliError> {
    let graph = ModuleGraph::build_with_env(file, selected_env)?;
    let replay_mode = prepare_replay_mode(file, &graph, record_path, replay_path, args)?;
    let trace_runtime_enabled = trace_enabled || breakpoints_requested;
    let compiled = compile_entry_files_with_cache(
        &[file.to_path_buf()],
        Some(graph),
        None,
        selected_env,
        trace_enabled,
        breakpoints_requested,
        json_output || trace_expr_enabled || breakpoints_requested,
        OutputFlavor::RuntimeEsm,
    )?;
    let module_debug_outputs = build_runtime_module_debug_outputs(&compiled)?;
    let topology_prelude = if matches!(
        replay_mode.as_ref(),
        Some(PreparedReplayMode::Replay { .. })
    ) {
        String::new()
    } else {
        runner_prelude(file, selected_env, &compiled.entry_output_path)?.unwrap_or_default()
    };
    let breakpoint_config = resolve_breakpoint_config(
        file,
        &module_debug_outputs,
        breakpoint_lines,
        breakpoint_functions,
        breakpoint_spans,
        breakpoint_mode,
        breakpoint_max_hits,
    )?;
    let entry_output_path = compiled.entry_output_path;
    let entry_span_map_path = compiled.entry_span_map_path;

    let runner_path = entry_output_path.with_extension("run.mjs");
    let runtime_error_path = unique_runtime_error_path(&entry_output_path);
    let runtime_trace_path = trace_enabled.then(|| unique_runtime_trace_path(&entry_output_path));
    let runtime_breakpoint_path = breakpoint_config
        .as_ref()
        .map(|_| unique_runtime_breakpoint_path(&entry_output_path));
    let runtime_replay_path = replay_mode
        .as_ref()
        .map(|_| unique_runtime_replay_path(&entry_output_path));
    let module_file_name = entry_output_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let module_specifier_json = serde_json::to_string(&format!("./{}", module_file_name)).unwrap();
    let filename_json = serde_json::to_string(&file.to_string_lossy().to_string()).unwrap();
    let runtime_error_path_json =
        serde_json::to_string(&runtime_error_path.to_string_lossy().to_string()).unwrap();
    let replay_enabled = replay_mode.is_some();
    let sync_capture_enabled =
        runtime_trace_path.is_some() || runtime_breakpoint_path.is_some() || replay_enabled;
    let sync_fs_import = if sync_capture_enabled {
        "import { writeFileSync } from 'node:fs';".to_string()
    } else {
        String::new()
    };
    let trace_config = if trace_runtime_enabled {
        format!(
            "globalThis.__sigil_trace_config = {{ enabled: true, maxEvents: 256, expressions: {} }};\nglobalThis.__sigil_trace_current = undefined;",
            if trace_expr_enabled { "true" } else { "false" }
        )
    } else {
        String::new()
    };
    let breakpoint_config_json = breakpoint_config
        .as_ref()
        .map(resolved_breakpoint_config_json)
        .map(|value| serde_json::to_string(&value).unwrap())
        .unwrap_or_else(|| "null".to_string());
    let breakpoint_config_setup = format!(
        "globalThis.__sigil_breakpoint_config = {breakpoint_config_json};\nglobalThis.__sigil_breakpoint_current = undefined;"
    );
    let trace_capture = if let Some(runtime_trace_path) = &runtime_trace_path {
        let runtime_trace_path_json =
            serde_json::to_string(&runtime_trace_path.to_string_lossy().to_string()).unwrap();
        format!(
            r#"
const __sigil_runtime_trace_file = {runtime_trace_path_json};

function __sigil_runtime_trace_payload() {{
  if (typeof globalThis.__sigil_trace_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_trace_snapshot();
    }} catch (_traceError) {{
      return {{ enabled: true, truncated: false, totalEvents: 0, returnedEvents: 0, droppedEvents: 0, events: [] }};
    }}
  }}
  return {{ enabled: true, truncated: false, totalEvents: 0, returnedEvents: 0, droppedEvents: 0, events: [] }};
}}

function __sigil_runtime_capture_trace_sync() {{
  try {{
    writeFileSync(__sigil_runtime_trace_file, JSON.stringify(__sigil_runtime_trace_payload()));
  }} catch (_captureTraceError) {{
    // Best-effort debug plumbing only.
  }}
}}

process.on('exit', () => {{
  __sigil_runtime_capture_trace_sync();
}});
"#
        )
    } else {
        String::new()
    };
    let breakpoint_capture = if let Some(runtime_breakpoint_path) = &runtime_breakpoint_path {
        let runtime_breakpoint_path_json =
            serde_json::to_string(&runtime_breakpoint_path.to_string_lossy().to_string()).unwrap();
        format!(
            r#"
const __sigil_runtime_breakpoint_file = {runtime_breakpoint_path_json};

function __sigil_runtime_breakpoint_payload() {{
  if (typeof globalThis.__sigil_breakpoint_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_breakpoint_snapshot();
    }} catch (_breakpointError) {{
      return {{
        enabled: true,
        mode: String(globalThis.__sigil_breakpoint_config?.mode ?? 'stop'),
        stopped: false,
        truncated: false,
        totalHits: 0,
        returnedHits: 0,
        droppedHits: 0,
        maxHits: Math.max(1, Number(globalThis.__sigil_breakpoint_config?.maxHits ?? 32)),
        hits: []
      }};
    }}
  }}
  return {{
    enabled: true,
    mode: String(globalThis.__sigil_breakpoint_config?.mode ?? 'stop'),
    stopped: false,
    truncated: false,
    totalHits: 0,
    returnedHits: 0,
    droppedHits: 0,
    maxHits: Math.max(1, Number(globalThis.__sigil_breakpoint_config?.maxHits ?? 32)),
    hits: []
  }};
}}

function __sigil_runtime_capture_breakpoints_sync() {{
  try {{
    writeFileSync(__sigil_runtime_breakpoint_file, JSON.stringify(__sigil_runtime_breakpoint_payload()));
  }} catch (_captureBreakpointError) {{
    // Best-effort debug plumbing only.
  }}
}}

process.on('exit', () => {{
  __sigil_runtime_capture_breakpoints_sync();
}});
"#
        )
    } else {
        String::new()
    };
    let replay_config_json = replay_mode
        .as_ref()
        .map(|mode| match mode {
            PreparedReplayMode::Record { config, .. } => serde_json::to_string(config).unwrap(),
            PreparedReplayMode::Replay { config, .. } => serde_json::to_string(config).unwrap(),
        })
        .unwrap_or_else(|| "null".to_string());
    let replay_capture = if let Some(runtime_replay_path) = &runtime_replay_path {
        let runtime_replay_path_json =
            serde_json::to_string(&runtime_replay_path.to_string_lossy().to_string()).unwrap();
        format!(
            r#"
const __sigil_runtime_replay_file = {runtime_replay_path_json};
globalThis.__sigil_replay_config = {replay_config_json};
globalThis.__sigil_replay_current = undefined;

function __sigil_runtime_replay_payload() {{
  if (typeof globalThis.__sigil_replay_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_replay_snapshot();
    }} catch (_replayError) {{
      return {{
        mode: String(globalThis.__sigil_replay_config?.mode ?? ''),
        file: String(globalThis.__sigil_replay_config?.file ?? ''),
        recordedEvents: 0,
        consumedEvents: 0,
        remainingEvents: 0,
        partial: false
      }};
    }}
  }}
  return {{
    mode: String(globalThis.__sigil_replay_config?.mode ?? ''),
    file: String(globalThis.__sigil_replay_config?.file ?? ''),
    recordedEvents: 0,
    consumedEvents: 0,
    remainingEvents: 0,
    partial: false
  }};
}}

function __sigil_runtime_capture_replay_sync() {{
  try {{
    writeFileSync(__sigil_runtime_replay_file, JSON.stringify(__sigil_runtime_replay_payload()));
  }} catch (_captureReplayError) {{
    // Best-effort debug plumbing only.
  }}
  if (globalThis.__sigil_replay_config?.mode === 'record' && typeof globalThis.__sigil_replay_artifact === 'function') {{
    try {{
      writeFileSync(
        String(globalThis.__sigil_replay_config.file),
        JSON.stringify(globalThis.__sigil_replay_artifact())
      );
    }} catch (_captureReplayArtifactError) {{
      // Best-effort debug plumbing only.
    }}
  }}
}}

process.on('exit', () => {{
  __sigil_runtime_capture_replay_sync();
}});
"#
        )
    } else {
        format!(
            r#"
globalThis.__sigil_replay_config = {replay_config_json};
globalThis.__sigil_replay_current = undefined;
"#
        )
    };
    let replay_failure_code_json =
        serde_json::to_string(codes::runtime::UNCAUGHT_EXCEPTION).unwrap();
    let replay_child_exit_code_json = serde_json::to_string(codes::runtime::CHILD_EXIT).unwrap();
    let replay_bootstrap_failure = if replay_enabled {
        format!(
            r#"
function __sigil_runtime_mark_replay_failure(code, message, stack) {{
  if (typeof globalThis.__sigil_replay_record_failure === 'function') {{
    try {{
      globalThis.__sigil_replay_record_failure(
        String(code ?? {replay_failure_code_json}),
        String(message ?? ''),
        typeof stack === 'string' ? stack : null
      );
    }} catch (_markReplayFailureError) {{
      // Best-effort debug plumbing only.
    }}
  }}
}}
"#
        )
    } else {
        String::new()
    };
    let replay_bootstrap_import = if replay_enabled {
        r#"
if (
  globalThis.__sigil_replay_config?.mode === 'replay' &&
  globalThis.__sigil_replay_config?.artifact?.failure &&
  globalThis.__sigil_replay_config?.artifact?.world?.normalizedWorld == null
) {
  const __sigil_recorded_failure = globalThis.__sigil_replay_config.artifact.failure;
  const __sigil_error = new Error(String(__sigil_recorded_failure.message ?? 'replayed runtime failure'));
  __sigil_error.sigilCode = String(__sigil_recorded_failure.code ?? 'SIGIL-RUNTIME-UNCAUGHT-EXCEPTION');
  if (typeof __sigil_recorded_failure.stack === 'string' && __sigil_recorded_failure.stack) {
    __sigil_error.stack = __sigil_recorded_failure.stack;
  }
  throw __sigil_error;
}
"#
        .to_string()
    } else {
        String::new()
    };

    let runner_code = format!(
        r#"import {{ writeFile }} from 'node:fs/promises';
{sync_fs_import}

const __sigil_runtime_error_file = {runtime_error_path_json};
{trace_capture}
{trace_config}
{breakpoint_capture}
{breakpoint_config_setup}
{replay_capture}
{replay_bootstrap_failure}

function __sigil_runtime_exception_name(error) {{
  if (error instanceof Error && error.name) {{
    return String(error.name);
  }}
  if (error && typeof error === 'object' && 'name' in error && error.name != null) {{
    return String(error.name);
  }}
  return 'Error';
}}

function __sigil_runtime_exception_message(error) {{
  if (error instanceof Error) {{
    return String(error.message ?? '');
  }}
  return String(error);
}}

function __sigil_runtime_exception_stack(error) {{
  if (error instanceof Error && typeof error.stack === 'string') {{
    return error.stack;
  }}
  return '';
}}

function __sigil_runtime_expression_payload() {{
  if (typeof globalThis.__sigil_expression_exception_payload === 'function') {{
    try {{
      return globalThis.__sigil_expression_exception_payload();
    }} catch (_captureExpressionError) {{
      return null;
    }}
  }}
  return null;
}}

async function __sigil_runtime_capture_error(error) {{
  const sigilCode =
    error && typeof error === 'object' && 'sigilCode' in error && error.sigilCode != null
      ? String(error.sigilCode)
      : null;
  const payload = {{
    message: __sigil_runtime_exception_message(error),
    name: __sigil_runtime_exception_name(error),
    sigilCode,
    expression: __sigil_runtime_expression_payload(),
    stack: __sigil_runtime_exception_stack(error)
  }};
  try {{
    await writeFile(__sigil_runtime_error_file, JSON.stringify(payload));
  }} catch (_captureError) {{
    // Best-effort debug plumbing only.
  }}
  return payload;
}}

function __sigil_runtime_is_breakpoint_stop(error) {{
  return typeof globalThis.__sigil_breakpoint_is_stop_signal === 'function'
    ? !!globalThis.__sigil_breakpoint_is_stop_signal(error)
    : false;
}}

try {{
{topology_prelude}
{replay_bootstrap_import}
  const __sigil_module = globalThis.__sigil_program_exports ?? await import({module_specifier_json});
  const main = __sigil_module.main;
  if (typeof main !== 'function') {{
    {missing_main_replay}
    console.error('Error: No main() function found in ' + {filename_json});
    console.error('Add a main() function to make this program runnable.');
    process.exit(1);
  }}

  const result = await main();
  if (result !== undefined) {{
    console.log(result);
  }}
  if (typeof __sigil_runtime_capture_trace_sync === 'function') {{
    __sigil_runtime_capture_trace_sync();
  }}
  if (typeof __sigil_runtime_capture_breakpoints_sync === 'function') {{
    __sigil_runtime_capture_breakpoints_sync();
  }}
  if (typeof __sigil_runtime_capture_replay_sync === 'function') {{
    __sigil_runtime_capture_replay_sync();
  }}
}} catch (error) {{
  if (__sigil_runtime_is_breakpoint_stop(error)) {{
    // Intentional early stop for machine-first breakpoint debugging.
  }} else {{
  const captured = await __sigil_runtime_capture_error(error);
  {catch_replay_mark}
  if (typeof __sigil_runtime_capture_trace_sync === 'function') {{
    __sigil_runtime_capture_trace_sync();
  }}
  if (typeof __sigil_runtime_capture_breakpoints_sync === 'function') {{
    __sigil_runtime_capture_breakpoints_sync();
  }}
  if (typeof __sigil_runtime_capture_replay_sync === 'function') {{
    __sigil_runtime_capture_replay_sync();
  }}
  const renderedStack = captured.stack;
  if (renderedStack) {{
    console.error(renderedStack);
  }} else {{
    console.error(`${{captured.name}}: ${{captured.message}}`);
  }}
  process.exit(1);
  }}
}}
"#,
        topology_prelude = topology_prelude,
        filename_json = filename_json,
        module_specifier_json = module_specifier_json,
        runtime_error_path_json = runtime_error_path_json,
        trace_capture = trace_capture,
        trace_config = trace_config,
        sync_fs_import = sync_fs_import,
        breakpoint_capture = breakpoint_capture,
        breakpoint_config_setup = breakpoint_config_setup,
        replay_capture = replay_capture,
        replay_bootstrap_failure = replay_bootstrap_failure,
        replay_bootstrap_import = replay_bootstrap_import,
        missing_main_replay = if replay_enabled {
            format!(
                "__sigil_runtime_mark_replay_failure({replay_child_exit_code_json}, 'No main() function found in ' + {filename_json}, null);"
            )
        } else {
            String::new()
        },
        catch_replay_mark = if replay_enabled {
            format!(
                "__sigil_runtime_mark_replay_failure(captured.sigilCode ?? {replay_failure_code_json}, captured.message, captured.stack);"
            )
        } else {
            String::new()
        }
    );

    fs::write(&runner_path, runner_code)?;
    Ok(RunTarget {
        entry_output_path,
        entry_span_map_path,
        runner_path,
        runtime_error_path,
        runtime_trace_path,
        runtime_breakpoint_path,
        runtime_replay_path,
        trace_enabled,
        breakpoint_config,
        replay_mode: replay_mode.as_ref().map(|mode| match mode {
            PreparedReplayMode::Record { .. } => "record".to_string(),
            PreparedReplayMode::Replay { .. } => "replay".to_string(),
        }),
        replay_file: replay_mode.map(|mode| match mode {
            PreparedReplayMode::Record { artifact_file, .. } => artifact_file,
            PreparedReplayMode::Replay { artifact_file, .. } => artifact_file,
        }),
        module_debug_outputs,
    })
}

fn execute_runner(
    runner_path: &Path,
    runtime_error_path: &Path,
    runtime_trace_path: Option<&Path>,
    runtime_breakpoint_path: Option<&Path>,
    runtime_replay_path: Option<&Path>,
    runtime_step_path: Option<&Path>,
    args: &[String],
    stream_output: bool,
) -> Result<RuntimeOutput, CliError> {
    let abs_runner_path = std::fs::canonicalize(runner_path)?;
    let _ = fs::remove_file(runtime_error_path);
    if let Some(runtime_trace_path) = runtime_trace_path {
        let _ = fs::remove_file(runtime_trace_path);
    }
    if let Some(runtime_breakpoint_path) = runtime_breakpoint_path {
        let _ = fs::remove_file(runtime_breakpoint_path);
    }
    if let Some(runtime_replay_path) = runtime_replay_path {
        let _ = fs::remove_file(runtime_replay_path);
    }
    if let Some(runtime_step_path) = runtime_step_path {
        let _ = fs::remove_file(runtime_step_path);
    }
    let start_time = Instant::now();
    if stream_output {
        if io::stdout().is_terminal() || io::stderr().is_terminal() {
            let status = Command::new("node")
                .arg(&abs_runner_path)
                .args(args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .map_err(map_runner_launch_error)?;

            return Ok(RuntimeOutput {
                exit_code: status.code().unwrap_or(-1),
                duration_ms: start_time.elapsed().as_millis(),
                stdout: String::new(),
                stderr: String::new(),
                exception_capture: read_runtime_exception_capture(runtime_error_path),
                trace_capture: read_runtime_trace_capture(runtime_trace_path),
                breakpoint_capture: read_runtime_breakpoint_capture(runtime_breakpoint_path),
                replay_capture: read_runtime_replay_capture(runtime_replay_path),
                step_capture: read_runtime_step_capture(runtime_step_path),
            });
        }

        let mut child = Command::new("node")
            .arg(&abs_runner_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(map_runner_launch_error)?;

        let stdout = child.stdout.take().ok_or_else(|| {
            CliError::Runtime(format!(
                "{}: failed to capture child stdout",
                codes::cli::UNEXPECTED
            ))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            CliError::Runtime(format!(
                "{}: failed to capture child stderr",
                codes::cli::UNEXPECTED
            ))
        })?;

        let stdout_handle = thread::spawn(move || tee_reader(stdout, io::stdout()));
        let stderr_handle = thread::spawn(move || tee_reader(stderr, io::stderr()));

        let status = child.wait()?;
        let stdout_bytes = join_tee_output(stdout_handle, "stdout")?;
        let stderr_bytes = join_tee_output(stderr_handle, "stderr")?;

        return Ok(RuntimeOutput {
            exit_code: status.code().unwrap_or(-1),
            duration_ms: start_time.elapsed().as_millis(),
            stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
            stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
            exception_capture: read_runtime_exception_capture(runtime_error_path),
            trace_capture: read_runtime_trace_capture(runtime_trace_path),
            breakpoint_capture: read_runtime_breakpoint_capture(runtime_breakpoint_path),
            replay_capture: read_runtime_replay_capture(runtime_replay_path),
            step_capture: read_runtime_step_capture(runtime_step_path),
        });
    }

    let output = Command::new("node")
        .arg(&abs_runner_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(map_runner_launch_error)?;

    Ok(RuntimeOutput {
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start_time.elapsed().as_millis(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exception_capture: read_runtime_exception_capture(runtime_error_path),
        trace_capture: read_runtime_trace_capture(runtime_trace_path),
        breakpoint_capture: read_runtime_breakpoint_capture(runtime_breakpoint_path),
        replay_capture: read_runtime_replay_capture(runtime_replay_path),
        step_capture: read_runtime_step_capture(runtime_step_path),
    })
}

#[derive(Debug, Clone)]
struct ParsedGeneratedFrame {
    file: String,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone)]
struct SourceExcerpt {
    start_line: usize,
    end_line: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct MappedSigilFrame {
    span: DebugSpanRecord,
    excerpt: Option<SourceExcerpt>,
}

#[derive(Debug, Clone)]
struct MappedSigilExpression {
    span: DebugSpanRecord,
    capture: RuntimeExpressionCapture,
}

#[derive(Debug, Clone)]
struct RuntimeExceptionAnalysis {
    generated_frame: Option<ParsedGeneratedFrame>,
    sigil_frame: Option<MappedSigilFrame>,
    sigil_expression: Option<MappedSigilExpression>,
}

fn build_runtime_module_debug_outputs(
    compiled: &CompiledGraphOutputs,
) -> Result<Vec<RuntimeModuleDebugOutput>, CliError> {
    let mut outputs = Vec::new();
    for (module_id, output_file) in &compiled.module_outputs {
        let span_map_file = compiled.span_map_outputs.get(module_id).ok_or_else(|| {
            CliError::Codegen(format!(
                "run target missing span map output for module '{}'",
                module_id
            ))
        })?;
        let span_map_contents = fs::read_to_string(span_map_file).map_err(|error| {
            CliError::Codegen(format!(
                "failed to read span map '{}': {}",
                span_map_file.display(),
                error
            ))
        })?;
        let span_map: ModuleSpanMap =
            serde_json::from_str(&span_map_contents).map_err(|error| {
                CliError::Codegen(format!(
                    "failed to parse span map '{}': {}",
                    span_map_file.display(),
                    error
                ))
            })?;
        outputs.push(RuntimeModuleDebugOutput {
            module_id: module_id.clone(),
            output_file: canonicalize_existing_path(output_file),
            span_map,
        });
    }
    Ok(outputs)
}

fn canonicalize_existing_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn unique_runtime_error_path(entry_output_path: &Path) -> PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let stem = entry_output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    entry_output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.{unique}.runtime-error.json"))
}

fn unique_world_inspect_runner_path(project_root: &Path) -> PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    project_root
        .join(".local")
        .join(format!("inspect-world.{unique}.run.mjs"))
}

fn unique_runtime_trace_path(entry_output_path: &Path) -> PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let stem = entry_output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    entry_output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.{unique}.runtime-trace.json"))
}

fn unique_runtime_breakpoint_path(entry_output_path: &Path) -> PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let stem = entry_output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    entry_output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.{unique}.runtime-breakpoints.json"))
}

fn unique_runtime_replay_path(entry_output_path: &Path) -> PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let stem = entry_output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    entry_output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.{unique}.runtime-replay.json"))
}

fn unique_runtime_step_path(entry_output_path: &Path) -> PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let stem = entry_output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    entry_output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}.{unique}.runtime-step.json"))
}

fn resolve_run_artifact_path(path: &Path, ensure_parent: bool) -> Result<PathBuf, CliError> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if ensure_parent {
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(resolved)
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode_lower_hex(Sha256::digest(bytes))
}

fn build_replay_binding(
    file: &Path,
    graph: &ModuleGraph,
    args: &[String],
) -> Result<(ReplayArtifactEntry, ReplayArtifactBinding), CliError> {
    let source_file = canonicalize_existing_path(file);
    let project_root = get_project_config(file)?.map(|project| {
        canonicalize_existing_path(&project.root)
            .to_string_lossy()
            .to_string()
    });
    let mut modules = graph
        .modules
        .values()
        .map(|module| ReplayArtifactModule {
            module_id: module.id.clone(),
            source_file: canonicalize_existing_path(&module.file_path)
                .to_string_lossy()
                .to_string(),
            source_hash: sha256_hex(module.source.as_bytes()),
        })
        .collect::<Vec<_>>();
    modules.sort_by(|left, right| left.module_id.cmp(&right.module_id));

    let mut fingerprint_hasher = Sha256::new();
    for module in &modules {
        fingerprint_hasher.update(module.module_id.as_bytes());
        fingerprint_hasher.update([0]);
        fingerprint_hasher.update(module.source_file.as_bytes());
        fingerprint_hasher.update([0]);
        fingerprint_hasher.update(module.source_hash.as_bytes());
        fingerprint_hasher.update([0]);
    }

    Ok((
        ReplayArtifactEntry {
            source_file: source_file.to_string_lossy().to_string(),
            argv: args.to_vec(),
            project_root,
        },
        ReplayArtifactBinding {
            algorithm: "sha256".to_string(),
            fingerprint: encode_lower_hex(fingerprint_hasher.finalize()),
            modules,
        },
    ))
}

fn read_replay_artifact(path: &Path) -> Result<ReplayArtifact, CliError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        CliError::Runtime(format!(
            "{}: failed to read replay artifact '{}': {}",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display(),
            error
        ))
    })?;
    let artifact: ReplayArtifact = serde_json::from_str(&contents).map_err(|error| {
        CliError::Runtime(format!(
            "{}: failed to parse replay artifact '{}': {}",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display(),
            error
        ))
    })?;
    if artifact.kind != "sigilRunReplay" || artifact.format_version != 2 {
        return Err(CliError::Runtime(format!(
            "{}: '{}' is not a supported Sigil replay artifact",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display()
        )));
    }
    if artifact.binding.algorithm != "sha256" {
        return Err(CliError::Runtime(format!(
            "{}: replay artifact '{}' uses unsupported fingerprint algorithm '{}'",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display(),
            artifact.binding.algorithm
        )));
    }
    Ok(artifact)
}

fn validate_replay_binding(
    file: &Path,
    args: &[String],
    expected_entry: &ReplayArtifactEntry,
    expected_binding: &ReplayArtifactBinding,
    artifact: &ReplayArtifact,
    artifact_path: &Path,
) -> Result<(), CliError> {
    let requested_file = canonicalize_existing_path(file)
        .to_string_lossy()
        .to_string();
    let artifact_file = canonicalize_existing_path(Path::new(&artifact.entry.source_file))
        .to_string_lossy()
        .to_string();
    if artifact_file != requested_file {
        return Err(CliError::Runtime(format!(
            "{}: replay artifact '{}' targets '{}' instead of '{}'",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display(),
            artifact.entry.source_file,
            requested_file
        )));
    }
    if artifact.entry.argv != args {
        return Err(CliError::Runtime(format!(
            "{}: replay artifact '{}' argv does not match this run",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display()
        )));
    }
    if artifact.binding.fingerprint != expected_binding.fingerprint
        || artifact.binding.modules != expected_binding.modules
    {
        return Err(CliError::Runtime(format!(
            "{}: replay artifact '{}' does not match the current source graph",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display()
        )));
    }
    if artifact.entry.source_file != expected_entry.source_file {
        return Err(CliError::Runtime(format!(
            "{}: replay artifact '{}' entry binding does not match the requested program",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display()
        )));
    }
    Ok(())
}

fn prepare_replay_mode(
    file: &Path,
    graph: &ModuleGraph,
    record_path: Option<&Path>,
    replay_path: Option<&Path>,
    args: &[String],
) -> Result<Option<PreparedReplayMode>, CliError> {
    let (entry, binding) = build_replay_binding(file, graph, args)?;

    if let Some(record_path) = record_path {
        let artifact_file = resolve_run_artifact_path(record_path, true)?;
        let config = json!({
            "mode": "record",
            "file": artifact_file.to_string_lossy(),
            "entry": entry,
            "binding": binding
        });
        return Ok(Some(PreparedReplayMode::Record {
            artifact_file,
            config,
        }));
    }

    if let Some(replay_path) = replay_path {
        let artifact_file = resolve_run_artifact_path(replay_path, false)?;
        let artifact = read_replay_artifact(&artifact_file)?;
        validate_replay_binding(file, args, &entry, &binding, &artifact, &artifact_file)?;
        let config = json!({
            "mode": "replay",
            "file": artifact_file.to_string_lossy(),
            "artifact": artifact
        });
        return Ok(Some(PreparedReplayMode::Replay {
            artifact_file,
            config,
        }));
    }

    Ok(None)
}

fn build_test_replay_binding(
    path: &Path,
    test_files: &[PathBuf],
    match_filter: Option<&str>,
    selected_env: Option<&str>,
) -> Result<(TestReplayArtifactRequest, ReplayArtifactBinding), CliError> {
    let requested_path = if path.exists() {
        canonicalize_existing_path(path)
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let project_root = get_project_config(path)?.map(|project| {
        canonicalize_existing_path(&project.root)
            .to_string_lossy()
            .to_string()
    });
    let mut modules_by_id = BTreeMap::<String, ReplayArtifactModule>::new();

    for test_file in test_files {
        let graph = ModuleGraph::build_with_env(test_file, selected_env)?;
        for module in graph.modules.values() {
            let replay_module = ReplayArtifactModule {
                module_id: module.id.clone(),
                source_file: canonicalize_existing_path(&module.file_path)
                    .to_string_lossy()
                    .to_string(),
                source_hash: sha256_hex(module.source.as_bytes()),
            };
            modules_by_id
                .entry(replay_module.module_id.clone())
                .or_insert(replay_module);
        }
    }

    let modules = modules_by_id.into_values().collect::<Vec<_>>();
    let mut fingerprint_hasher = Sha256::new();
    for module in &modules {
        fingerprint_hasher.update(module.module_id.as_bytes());
        fingerprint_hasher.update([0]);
        fingerprint_hasher.update(module.source_file.as_bytes());
        fingerprint_hasher.update([0]);
        fingerprint_hasher.update(module.source_hash.as_bytes());
        fingerprint_hasher.update([0]);
    }

    Ok((
        TestReplayArtifactRequest {
            path: requested_path.to_string_lossy().to_string(),
            match_filter: match_filter.map(str::to_string),
            project_root,
        },
        ReplayArtifactBinding {
            algorithm: "sha256".to_string(),
            fingerprint: encode_lower_hex(fingerprint_hasher.finalize()),
            modules,
        },
    ))
}

fn read_test_replay_artifact(path: &Path) -> Result<TestReplayArtifact, CliError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        CliError::Runtime(format!(
            "{}: failed to read test replay artifact '{}': {}",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display(),
            error
        ))
    })?;
    let artifact: TestReplayArtifact = serde_json::from_str(&contents).map_err(|error| {
        CliError::Runtime(format!(
            "{}: failed to parse test replay artifact '{}': {}",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display(),
            error
        ))
    })?;
    if artifact.kind != "sigilTestReplay" || artifact.format_version != 1 {
        return Err(CliError::Runtime(format!(
            "{}: '{}' is not a supported Sigil test replay artifact",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display()
        )));
    }
    if artifact.binding.algorithm != "sha256" {
        return Err(CliError::Runtime(format!(
            "{}: test replay artifact '{}' uses unsupported fingerprint algorithm '{}'",
            codes::runtime::REPLAY_INVALID_ARTIFACT,
            path.display(),
            artifact.binding.algorithm
        )));
    }
    Ok(artifact)
}

fn validate_test_replay_binding(
    path: &Path,
    expected_request: &TestReplayArtifactRequest,
    expected_binding: &ReplayArtifactBinding,
    artifact: &TestReplayArtifact,
    artifact_path: &Path,
) -> Result<(), CliError> {
    let requested_path = if path.exists() {
        canonicalize_existing_path(path)
            .to_string_lossy()
            .to_string()
    } else if path.is_absolute() {
        path.to_string_lossy().to_string()
    } else {
        std::env::current_dir()?
            .join(path)
            .to_string_lossy()
            .to_string()
    };
    if artifact.request.path != requested_path {
        return Err(CliError::Runtime(format!(
            "{}: test replay artifact '{}' targets '{}' instead of '{}'",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display(),
            artifact.request.path,
            requested_path
        )));
    }
    if artifact.request.match_filter != expected_request.match_filter {
        return Err(CliError::Runtime(format!(
            "{}: test replay artifact '{}' does not match the requested test filter",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display()
        )));
    }
    if artifact.binding.fingerprint != expected_binding.fingerprint
        || artifact.binding.modules != expected_binding.modules
    {
        return Err(CliError::Runtime(format!(
            "{}: test replay artifact '{}' does not match the current source graph",
            codes::runtime::REPLAY_BINDING_MISMATCH,
            artifact_path.display()
        )));
    }
    Ok(())
}

fn prepare_test_replay_mode(
    path: &Path,
    test_files: &[PathBuf],
    match_filter: Option<&str>,
    selected_env: Option<&str>,
    record_path: Option<&Path>,
    replay_path: Option<&Path>,
) -> Result<Option<PreparedTestReplayMode>, CliError> {
    let (request, binding) =
        build_test_replay_binding(path, test_files, match_filter, selected_env)?;

    if let Some(record_path) = record_path {
        let artifact_file = resolve_run_artifact_path(record_path, true)?;
        return Ok(Some(PreparedTestReplayMode::Record {
            artifact_file,
            request,
            binding,
        }));
    }

    if let Some(replay_path) = replay_path {
        let artifact_file = resolve_run_artifact_path(replay_path, false)?;
        let artifact = read_test_replay_artifact(&artifact_file)?;
        validate_test_replay_binding(path, &request, &binding, &artifact, &artifact_file)?;
        return Ok(Some(PreparedTestReplayMode::Replay {
            artifact_file,
            artifact,
        }));
    }

    Ok(None)
}

fn read_runtime_exception_capture(path: &Path) -> Option<RuntimeExceptionCapture> {
    let contents = fs::read_to_string(path).ok();
    let _ = fs::remove_file(path);
    let contents = contents?;
    let mut capture: RuntimeExceptionCapture = serde_json::from_str(&contents).ok()?;
    if capture
        .sigil_code
        .as_deref()
        .is_none_or(|code| code.is_empty())
    {
        capture.sigil_code = recover_runtime_exception_code(&capture.message, &capture.stack);
    }
    Some(capture)
}

fn read_runtime_trace_capture(path: Option<&Path>) -> Option<RuntimeTraceCapture> {
    let path = path?;
    let contents = fs::read_to_string(path).ok();
    let _ = fs::remove_file(path);
    let contents = contents?;
    serde_json::from_str(&contents).ok()
}

fn read_runtime_breakpoint_capture(path: Option<&Path>) -> Option<RuntimeBreakpointCapture> {
    let path = path?;
    let contents = fs::read_to_string(path).ok();
    let _ = fs::remove_file(path);
    let contents = contents?;
    serde_json::from_str(&contents).ok()
}

fn read_runtime_replay_capture(path: Option<&Path>) -> Option<RuntimeReplayCapture> {
    let path = path?;
    let contents = fs::read_to_string(path).ok();
    let _ = fs::remove_file(path);
    let contents = contents?;
    serde_json::from_str(&contents).ok()
}

fn read_runtime_step_capture(path: Option<&Path>) -> Option<RuntimeStepCapture> {
    let path = path?;
    let contents = fs::read_to_string(path).ok();
    let _ = fs::remove_file(path);
    let contents = contents?;
    serde_json::from_str(&contents).ok()
}

fn runtime_trace_json(trace_capture: Option<&RuntimeTraceCapture>) -> serde_json::Value {
    match trace_capture {
        Some(trace_capture) => serde_json::to_value(trace_capture).unwrap_or_else(|_| {
            json!({
                "enabled": true,
                "truncated": false,
                "totalEvents": 0,
                "returnedEvents": 0,
                "droppedEvents": 0,
                "events": []
            })
        }),
        None => json!({
            "enabled": true,
            "truncated": false,
            "totalEvents": 0,
            "returnedEvents": 0,
            "droppedEvents": 0,
            "events": []
        }),
    }
}

fn runtime_replay_json(
    mode: Option<&str>,
    artifact_file: Option<&Path>,
    replay_capture: Option<&RuntimeReplayCapture>,
) -> serde_json::Value {
    match (mode, artifact_file, replay_capture) {
        (Some(mode), Some(file), Some(capture)) => json!({
            "mode": mode,
            "file": file.to_string_lossy(),
            "recordedEvents": capture.recorded_events,
            "consumedEvents": capture.consumed_events,
            "remainingEvents": capture.remaining_events,
            "partial": capture.partial
        }),
        (Some(mode), Some(file), None) => json!({
            "mode": mode,
            "file": file.to_string_lossy(),
            "recordedEvents": 0,
            "consumedEvents": 0,
            "remainingEvents": 0,
            "partial": false
        }),
        _ => serde_json::Value::Null,
    }
}

fn find_debug_span<'a>(
    module_debug_outputs: &'a [RuntimeModuleDebugOutput],
    module_id: &str,
    span_id: &str,
) -> Option<&'a DebugSpanRecord> {
    module_debug_outputs
        .iter()
        .find(|module| module.module_id == module_id)?
        .span_map
        .spans
        .iter()
        .find(|span| span.span_id == span_id)
}

fn runtime_breakpoint_frame_json(
    frame: &RuntimeBreakpointFrameCapture,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> serde_json::Value {
    let location = find_debug_span(module_debug_outputs, &frame.module_id, &frame.span_id)
        .map(|span| serde_json::to_value(&span.location).unwrap());
    let mut value = serde_json::Map::new();
    value.insert("moduleId".to_string(), json!(frame.module_id));
    value.insert("sourceFile".to_string(), json!(frame.source_file));
    value.insert("spanId".to_string(), json!(frame.span_id));
    value.insert("declarationKind".to_string(), json!(frame.declaration_kind));
    value.insert(
        "declarationLabel".to_string(),
        json!(frame.declaration_label),
    );
    value.insert("functionName".to_string(), json!(frame.function_name));
    value.insert(
        "location".to_string(),
        location.unwrap_or(serde_json::Value::Null),
    );
    serde_json::Value::Object(value)
}

fn runtime_breakpoint_hit_json(
    hit: &RuntimeBreakpointHitCapture,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> serde_json::Value {
    let location = find_debug_span(module_debug_outputs, &hit.module_id, &hit.span_id)
        .map(|span| serde_json::to_value(&span.location).unwrap());
    json!({
        "matched": hit.matched,
        "moduleId": hit.module_id,
        "sourceFile": hit.source_file,
        "spanId": hit.span_id,
        "spanKind": hit.span_kind,
        "declarationKind": hit.declaration_kind,
        "declarationLabel": hit.declaration_label,
        "location": location,
        "locals": hit.locals,
        "stack": hit
            .stack
            .iter()
            .map(|frame| runtime_breakpoint_frame_json(frame, module_debug_outputs))
            .collect::<Vec<_>>(),
        "recentTrace": hit.recent_trace
    })
}

fn runtime_breakpoints_json(
    config: Option<&ResolvedBreakpointConfig>,
    breakpoint_capture: Option<&RuntimeBreakpointCapture>,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> serde_json::Value {
    match (config, breakpoint_capture) {
        (Some(_config), Some(capture)) => json!({
            "enabled": capture.enabled,
            "mode": capture.mode,
            "stopped": capture.stopped,
            "truncated": capture.truncated,
            "totalHits": capture.total_hits,
            "returnedHits": capture.returned_hits,
            "droppedHits": capture.dropped_hits,
            "maxHits": capture.max_hits,
            "hits": capture
                .hits
                .iter()
                .map(|hit| runtime_breakpoint_hit_json(hit, module_debug_outputs))
                .collect::<Vec<_>>()
        }),
        (Some(config), None) => json!({
            "enabled": true,
            "mode": config.mode.as_str(),
            "stopped": false,
            "truncated": false,
            "totalHits": 0,
            "returnedHits": 0,
            "droppedHits": 0,
            "maxHits": config.max_hits,
            "hits": []
        }),
        _ => serde_json::Value::Null,
    }
}

fn build_runtime_failure_output(
    file: &Path,
    run_target: &RunTarget,
    runtime_output: &RuntimeOutput,
) -> serde_json::Value {
    let compile = json!({
        "input": file.to_string_lossy(),
        "output": run_target.entry_output_path.to_string_lossy(),
        "runnerFile": run_target.runner_path.to_string_lossy(),
        "spanMapFile": run_target.entry_span_map_path.to_string_lossy()
    });
    let runtime = json!({
        "engine": "node",
        "exitCode": runtime_output.exit_code,
        "durationMs": runtime_output.duration_ms,
        "stdout": runtime_output.stdout,
        "stderr": runtime_output.stderr
    });
    let trace = run_target
        .trace_enabled
        .then(|| runtime_trace_json(runtime_output.trace_capture.as_ref()));
    let breakpoints = run_target.breakpoint_config.as_ref().map(|config| {
        runtime_breakpoints_json(
            Some(config),
            runtime_output.breakpoint_capture.as_ref(),
            &run_target.module_debug_outputs,
        )
    });
    let replay = run_target.replay_mode.as_ref().map(|mode| {
        runtime_replay_json(
            Some(mode.as_str()),
            run_target.replay_file.as_deref(),
            runtime_output.replay_capture.as_ref(),
        )
    });

    let exception_capture = runtime_output
        .exception_capture
        .clone()
        .or_else(|| runtime_exception_capture_from_stderr(&runtime_output.stderr));

    if let Some(capture) = &exception_capture {
        return build_runtime_exception_output(
            compile,
            runtime,
            trace,
            breakpoints,
            replay,
            &run_target.module_debug_outputs,
            capture,
        );
    }

    let mut details = serde_json::Map::new();
    details.insert("compile".to_string(), compile);
    details.insert("runtime".to_string(), runtime);
    if let Some(trace) = trace {
        details.insert("trace".to_string(), trace);
    }
    if let Some(breakpoints) = breakpoints {
        details.insert("breakpoints".to_string(), breakpoints);
    }
    if let Some(replay) = replay {
        details.insert("replay".to_string(), replay);
    }

    json!({
        "formatVersion": 1,
        "command": "sigilc run",
        "ok": false,
        "phase": "runtime",
        "error": {
            "code": codes::runtime::CHILD_EXIT,
            "phase": "runtime",
            "message": format!(
                "child process exited with nonzero status: {}",
                runtime_output.exit_code
            ),
            "details": serde_json::Value::Object(details)
        }
    })
}

fn build_runtime_exception_output(
    compile: serde_json::Value,
    runtime: serde_json::Value,
    trace: Option<serde_json::Value>,
    breakpoints: Option<serde_json::Value>,
    replay: Option<serde_json::Value>,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
    capture: &RuntimeExceptionCapture,
) -> serde_json::Value {
    let resolved_code = resolved_runtime_exception_code(capture);
    let code = resolved_code.as_str();
    let phase = phase_for_code(code);
    let normalized_message = normalize_runtime_exception_message(capture, code);
    let analysis = analyze_runtime_exception(capture, module_debug_outputs);

    let mut details = serde_json::Map::new();
    details.insert("compile".to_string(), compile);
    details.insert("runtime".to_string(), runtime);
    if let Some(trace) = trace {
        details.insert("trace".to_string(), trace);
    }
    if let Some(breakpoints) = breakpoints {
        details.insert("breakpoints".to_string(), breakpoints);
    }
    if let Some(replay) = replay {
        details.insert("replay".to_string(), replay);
    }
    details.insert(
        "exception".to_string(),
        runtime_exception_json(
            capture,
            &normalized_message,
            &analysis,
            module_debug_outputs,
        ),
    );

    let mut error = serde_json::Map::new();
    error.insert("code".to_string(), json!(code));
    error.insert("phase".to_string(), json!(phase));
    error.insert("message".to_string(), json!(normalized_message));
    error.insert("details".to_string(), serde_json::Value::Object(details));
    if let Some(sigil_expression) = &analysis.sigil_expression {
        error.insert(
            "location".to_string(),
            serde_json::to_value(&sigil_expression.span.location).unwrap(),
        );
    } else if let Some(sigil_frame) = &analysis.sigil_frame {
        error.insert(
            "location".to_string(),
            serde_json::to_value(&sigil_frame.span.location).unwrap(),
        );
    }

    json!({
        "formatVersion": 1,
        "command": "sigilc run",
        "ok": false,
        "phase": phase,
        "error": error
    })
}

fn runtime_exception_capture_from_stderr(stderr: &str) -> Option<RuntimeExceptionCapture> {
    let stack = stderr.trim();
    if stack.is_empty() {
        return None;
    }

    let first_line = stack.lines().next().unwrap_or(stack).trim();
    let headline = stack
        .lines()
        .map(str::trim)
        .find(|line| line.contains("SIGIL-") && !line.is_empty())
        .unwrap_or(first_line);
    let (name, message) = match headline.split_once(':') {
        Some((name, message)) if !name.trim().is_empty() => {
            (name.trim().to_string(), message.trim().to_string())
        }
        _ => ("Error".to_string(), headline.to_string()),
    };

    let sigil_code = recover_runtime_exception_code(&message, stack);

    Some(RuntimeExceptionCapture {
        name,
        message,
        stack: stack.to_string(),
        sigil_code,
        expression: None,
    })
}

fn recover_runtime_exception_code(message: &str, stack: &str) -> Option<String> {
    if message.contains("SIGIL-") {
        return Some(extract_error_code(message));
    }

    if stack.contains("SIGIL-") {
        let headline = stack
            .lines()
            .map(str::trim)
            .find(|line| line.contains("SIGIL-") && !line.is_empty())
            .unwrap_or(stack.trim());
        if !headline.is_empty() {
            return Some(extract_error_code(headline));
        }
    }

    None
}

fn resolved_runtime_exception_code(capture: &RuntimeExceptionCapture) -> String {
    let recovered_code = recover_runtime_exception_code(&capture.message, &capture.stack);
    let explicit_code = capture
        .sigil_code
        .as_deref()
        .filter(|code| !code.is_empty());

    match (explicit_code, recovered_code.as_deref()) {
        (Some(explicit), Some(recovered))
            if explicit == codes::runtime::UNCAUGHT_EXCEPTION
                && recovered != codes::runtime::UNCAUGHT_EXCEPTION =>
        {
            recovered.to_string()
        }
        (Some(explicit), _) => explicit.to_string(),
        (None, Some(recovered)) => recovered.to_string(),
        (None, None) => codes::runtime::UNCAUGHT_EXCEPTION.to_string(),
    }
}

fn normalize_runtime_exception_message(capture: &RuntimeExceptionCapture, code: &str) -> String {
    if code == codes::runtime::UNCAUGHT_EXCEPTION {
        if capture.message.is_empty() {
            format!("uncaught runtime exception: {}", capture.name)
        } else {
            format!(
                "uncaught runtime exception: {}: {}",
                capture.name, capture.message
            )
        }
    } else if let Some(message) = capture
        .message
        .strip_prefix(&format!("{code}: "))
        .filter(|message| !message.is_empty())
    {
        message.to_string()
    } else {
        capture.message.clone()
    }
}

fn runtime_exception_json(
    capture: &RuntimeExceptionCapture,
    normalized_message: &str,
    analysis: &RuntimeExceptionAnalysis,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> serde_json::Value {
    let mut exception = serde_json::Map::new();
    exception.insert("name".to_string(), json!(capture.name));
    exception.insert("message".to_string(), json!(normalized_message));
    exception.insert("rawStack".to_string(), json!(capture.stack));

    if let Some(frame) = &analysis.generated_frame {
        exception.insert(
            "generatedFrame".to_string(),
            json!({
                "file": frame.file,
                "line": frame.line,
                "column": frame.column
            }),
        );
    }

    if let Some(sigil_frame) = &analysis.sigil_frame {
        let mut frame = serde_json::Map::new();
        frame.insert("spanId".to_string(), json!(sigil_frame.span.span_id));
        frame.insert(
            "kind".to_string(),
            serde_json::to_value(&sigil_frame.span.kind).unwrap(),
        );
        if let Some(label) = &sigil_frame.span.label {
            frame.insert("label".to_string(), json!(label));
        }
        frame.insert("file".to_string(), json!(sigil_frame.span.source_file));
        frame.insert(
            "location".to_string(),
            serde_json::to_value(&sigil_frame.span.location).unwrap(),
        );
        if let Some(excerpt) = &sigil_frame.excerpt {
            frame.insert(
                "excerpt".to_string(),
                json!({
                    "startLine": excerpt.start_line,
                    "endLine": excerpt.end_line,
                    "text": excerpt.text
                }),
            );
        }
        exception.insert("sigilFrame".to_string(), serde_json::Value::Object(frame));
    }

    if let Some(sigil_expression) = &analysis.sigil_expression {
        let mut expression = serde_json::Map::new();
        expression.insert("spanId".to_string(), json!(sigil_expression.span.span_id));
        expression.insert(
            "kind".to_string(),
            serde_json::to_value(&sigil_expression.span.kind).unwrap(),
        );
        expression.insert("file".to_string(), json!(sigil_expression.span.source_file));
        expression.insert(
            "location".to_string(),
            serde_json::to_value(&sigil_expression.span.location).unwrap(),
        );
        expression.insert(
            "declarationKind".to_string(),
            json!(sigil_expression.capture.declaration_kind),
        );
        expression.insert(
            "declarationLabel".to_string(),
            json!(sigil_expression.capture.declaration_label),
        );
        if let Some(value) = &sigil_expression.capture.value {
            expression.insert("value".to_string(), value.clone());
        }
        if let Some(error) = &sigil_expression.capture.error {
            expression.insert("error".to_string(), error.clone());
        }
        expression.insert(
            "locals".to_string(),
            serde_json::to_value(&sigil_expression.capture.locals).unwrap(),
        );
        expression.insert(
            "stack".to_string(),
            serde_json::Value::Array(
                sigil_expression
                    .capture
                    .stack
                    .iter()
                    .map(|frame| runtime_breakpoint_frame_json(frame, module_debug_outputs))
                    .collect(),
            ),
        );
        exception.insert(
            "sigilExpression".to_string(),
            serde_json::Value::Object(expression),
        );
    }

    serde_json::Value::Object(exception)
}

fn analyze_runtime_exception(
    capture: &RuntimeExceptionCapture,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> RuntimeExceptionAnalysis {
    let sigil_expression = capture
        .expression
        .as_ref()
        .and_then(|expression| map_runtime_expression_to_sigil(expression, module_debug_outputs));
    let expression_frame = sigil_expression
        .as_ref()
        .and_then(|expression| declaration_frame_for_expression(expression, module_debug_outputs));
    let frames = parse_generated_stack_frames(&capture.stack);
    for frame in &frames {
        if let Some(sigil_frame) = map_generated_frame_to_sigil(frame, module_debug_outputs) {
            return RuntimeExceptionAnalysis {
                generated_frame: Some(frame.clone()),
                sigil_frame: Some(sigil_frame),
                sigil_expression,
            };
        }
    }

    RuntimeExceptionAnalysis {
        generated_frame: frames.into_iter().next(),
        sigil_expression,
        sigil_frame: expression_frame,
    }
}

fn parse_generated_stack_frames(stack: &str) -> Vec<ParsedGeneratedFrame> {
    stack
        .lines()
        .filter_map(parse_generated_stack_frame_line)
        .collect()
}

fn parse_generated_stack_frame_line(line: &str) -> Option<ParsedGeneratedFrame> {
    let trimmed = line.trim();
    if !trimmed.starts_with("at ") {
        return None;
    }

    let candidate = if trimmed.ends_with(')') {
        let close = trimmed.rfind(')')?;
        let open = trimmed[..close].rfind('(')?;
        &trimmed[open + 1..close]
    } else {
        trimmed.strip_prefix("at ")?.trim()
    };

    let mut parts = candidate.rsplitn(3, ':');
    let column = parts.next()?.parse::<usize>().ok()?;
    let line = parts.next()?.parse::<usize>().ok()?;
    let file = normalize_generated_frame_path(parts.next()?);
    Some(ParsedGeneratedFrame {
        file: file.to_string_lossy().to_string(),
        line,
        column,
    })
}

fn normalize_generated_frame_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    let without_file_scheme = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    canonicalize_existing_path(Path::new(without_file_scheme))
}

fn debug_snapshot_json(
    step_capture: Option<&RuntimeStepCapture>,
    runtime_output: &RuntimeOutput,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
    replay_file: &Path,
) -> serde_json::Value {
    let mut snapshot = match step_capture {
        Some(capture) => {
            let location = capture
                .module_id
                .as_deref()
                .zip(capture.span_id.as_deref())
                .and_then(|(module_id, span_id)| {
                    find_debug_span(module_debug_outputs, module_id, span_id)
                })
                .map(|span| serde_json::to_value(&span.location).unwrap());
            json!({
                "state": capture.state,
                "pauseReason": capture.pause_reason,
                "eventKind": capture.event_kind,
                "seq": capture.seq,
                "moduleId": capture.module_id,
                "sourceFile": capture.source_file,
                "spanId": capture.span_id,
                "spanKind": capture.span_kind,
                "declarationKind": capture.declaration_kind,
                "declarationLabel": capture.declaration_label,
                "functionName": capture.function_name,
                "testId": capture.test_id,
                "testName": capture.test_name,
                "testStatus": capture.test_status,
                "matched": capture.matched,
                "location": location,
                "locals": capture.locals,
                "stack": capture
                    .stack
                    .iter()
                    .map(|frame| runtime_breakpoint_frame_json(frame, module_debug_outputs))
                    .collect::<Vec<_>>(),
                "recentTrace": capture.recent_trace,
                "frameDepth": capture.frame_depth,
                "expressionDepth": capture.expression_depth,
                "lastCompleted": capture.last_completed,
                "watches": capture.watches
            })
        }
        None => json!({
            "state": "failed",
            "pauseReason": "exception",
            "eventKind": "uncaught_exception",
            "seq": 0,
            "moduleId": null,
            "sourceFile": null,
            "spanId": null,
            "spanKind": null,
            "declarationKind": null,
            "declarationLabel": null,
            "functionName": null,
            "testId": null,
            "testName": null,
            "testStatus": null,
            "matched": [],
            "location": null,
            "locals": [],
            "stack": [],
            "recentTrace": [],
            "frameDepth": 0,
            "expressionDepth": 0,
            "lastCompleted": null,
            "watches": []
        }),
    };

    let stderr_capture = runtime_exception_capture_from_stderr(&runtime_output.stderr);
    if let Some(exception_capture) = runtime_output
        .exception_capture
        .as_ref()
        .or(stderr_capture.as_ref())
    {
        let resolved_code = resolved_runtime_exception_code(exception_capture);
        let code = resolved_code.as_str();
        let normalized_message = normalize_runtime_exception_message(exception_capture, code);
        let analysis = analyze_runtime_exception(exception_capture, module_debug_outputs);
        let exception_json = runtime_exception_json(
            exception_capture,
            &normalized_message,
            &analysis,
            module_debug_outputs,
        );
        if let Some(snapshot_object) = snapshot.as_object_mut() {
            snapshot_object.insert("exception".to_string(), exception_json);
            if snapshot_object
                .get("location")
                .is_none_or(|location| location.is_null())
            {
                if let Some(sigil_expression) = &analysis.sigil_expression {
                    snapshot_object.insert(
                        "location".to_string(),
                        serde_json::to_value(&sigil_expression.span.location).unwrap(),
                    );
                } else if let Some(sigil_frame) = &analysis.sigil_frame {
                    snapshot_object.insert(
                        "location".to_string(),
                        serde_json::to_value(&sigil_frame.span.location).unwrap(),
                    );
                }
            }
        }
    }

    if let Some(snapshot_object) = snapshot.as_object_mut() {
        snapshot_object.insert("stdoutSoFar".to_string(), json!(runtime_output.stdout));
        snapshot_object.insert("stderrSoFar".to_string(), json!(runtime_output.stderr));
        snapshot_object.insert(
            "replay".to_string(),
            runtime_replay_json(
                Some("replay"),
                Some(replay_file),
                runtime_output.replay_capture.as_ref(),
            ),
        );
    }

    snapshot
}

fn debug_session_state_error(
    target_kind: DebugSessionTargetKind,
    session_path: &Path,
    session: &DebugSessionFile,
) -> Result<(), CliError> {
    output_json_error_to(
        target_kind.command_name(),
        "cli",
        codes::cli::UNEXPECTED,
        "debug session cannot be advanced from its current state",
        json!({
            "session": session_path.to_string_lossy(),
            "state": session.state
        }),
        false,
    );
    Err(CliError::Reported(1))
}

struct DebugExecution {
    runner_path: PathBuf,
    runtime_error_path: PathBuf,
    runtime_replay_path: PathBuf,
    runtime_step_path: PathBuf,
    replay_file: PathBuf,
    args: Vec<String>,
    module_debug_outputs: Vec<RuntimeModuleDebugOutput>,
}

fn prepare_debug_run_execution(
    file: &Path,
    replay_path: &Path,
    breakpoints: &DebugBreakpointSelectors,
    watches: &[String],
    action: &str,
    cursor: Option<&DebugStepCursor>,
) -> Result<DebugExecution, CliError> {
    let artifact_file = resolve_run_artifact_path(replay_path, false)?;
    let artifact = read_replay_artifact(&artifact_file)?;
    let args = artifact.entry.argv.clone();
    let graph = ModuleGraph::build(file)?;
    let (entry, binding) = build_replay_binding(file, &graph, &args)?;
    validate_replay_binding(file, &args, &entry, &binding, &artifact, &artifact_file)?;

    let compiled = compile_entry_files_with_cache(
        &[file.to_path_buf()],
        Some(graph),
        None,
        None,
        true,
        true,
        true,
        OutputFlavor::RuntimeEsm,
    )?;
    let module_debug_outputs = build_runtime_module_debug_outputs(&compiled)?;
    let breakpoint_config = resolve_breakpoint_config(
        file,
        &module_debug_outputs,
        &breakpoints.breakpoint_lines,
        &breakpoints.breakpoint_functions,
        &breakpoints.breakpoint_spans,
        BreakpointMode::Stop,
        32,
    )?;
    let trace_config_json = serde_json::to_string(&json!({
        "enabled": true,
        "maxEvents": 256,
        "expressions": true
    }))
    .map_err(|error| CliError::Codegen(format!("failed to encode debug trace config: {error}")))?;
    let breakpoint_config_json = debug_runtime_breakpoint_config_json(breakpoint_config.as_ref())?;
    let replay_config_json = serde_json::to_string(&json!({
        "mode": "replay",
        "file": artifact_file.to_string_lossy(),
        "artifact": artifact
    }))
    .map_err(|error| CliError::Codegen(format!("failed to encode debug replay config: {error}")))?;
    let step_config_json =
        debug_step_config_json(DebugSessionTargetKind::Run, action, cursor, watches)?;

    let entry_output_path = compiled.entry_output_path;
    let runner_path = entry_output_path.with_extension("debug.run.mjs");
    let runtime_error_path = unique_runtime_error_path(&entry_output_path);
    let runtime_replay_path = unique_runtime_replay_path(&entry_output_path);
    let runtime_step_path = unique_runtime_step_path(&entry_output_path);
    let module_file_name = entry_output_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let module_specifier_json = serde_json::to_string(&format!("./{}", module_file_name)).unwrap();
    let runtime_error_path_json =
        serde_json::to_string(&runtime_error_path.to_string_lossy().to_string()).unwrap();
    let runtime_replay_path_json =
        serde_json::to_string(&runtime_replay_path.to_string_lossy().to_string()).unwrap();
    let runtime_step_path_json =
        serde_json::to_string(&runtime_step_path.to_string_lossy().to_string()).unwrap();
    let filename_json = serde_json::to_string(&file.to_string_lossy().to_string()).unwrap();

    let runner_code = format!(
        r#"import {{ writeFile }} from 'node:fs/promises';
import {{ writeFileSync }} from 'node:fs';

{step_runtime}
globalThis.__sigil_trace_config = {trace_config_json};
globalThis.__sigil_trace_current = undefined;
globalThis.__sigil_breakpoint_config = {breakpoint_config_json};
globalThis.__sigil_breakpoint_current = undefined;
{replay_capture}
{error_capture}

if (
  globalThis.__sigil_replay_config?.mode === 'replay' &&
  globalThis.__sigil_replay_config?.artifact?.failure &&
  globalThis.__sigil_replay_config?.artifact?.world?.normalizedWorld == null
) {{
  const __sigil_recorded_failure = globalThis.__sigil_replay_config.artifact.failure;
  const __sigil_error = new Error(String(__sigil_recorded_failure.message ?? 'replayed runtime failure'));
  __sigil_error.sigilCode = String(__sigil_recorded_failure.code ?? 'SIGIL-RUNTIME-UNCAUGHT-EXCEPTION');
  if (typeof __sigil_recorded_failure.stack === 'string' && __sigil_recorded_failure.stack) {{
    __sigil_error.stack = __sigil_recorded_failure.stack;
  }}
  throw __sigil_error;
}}

try {{
  const __sigil_module = await import({module_specifier_json});
  const main = __sigil_module.main;
  if (typeof main !== 'function') {{
    throw new Error('No main() function found in ' + {filename_json});
  }}
  const result = await main();
  if (result !== undefined) {{
    console.log(result);
  }}
  if (typeof globalThis.__sigil_debug_mark_completed === 'function') {{
    globalThis.__sigil_debug_mark_completed('program_exit', {{}});
  }}
}} catch (error) {{
  if (__sigil_runtime_is_intentional_debug_stop(error)) {{
    // Intentional debug pause.
  }} else {{
    const captured = await __sigil_runtime_capture_error(error);
    if (typeof globalThis.__sigil_debug_mark_failed === 'function') {{
      globalThis.__sigil_debug_mark_failed(captured);
    }}
    if (captured.stack) {{
      console.error(captured.stack);
    }} else {{
      console.error(`${{captured.name}}: ${{captured.message}}`);
    }}
    process.exit(1);
  }}
}}
"#,
        step_runtime = debug_step_runtime_source(&runtime_step_path_json, &step_config_json),
        trace_config_json = trace_config_json,
        breakpoint_config_json = breakpoint_config_json,
        replay_capture =
            debug_runtime_replay_capture_source(&runtime_replay_path_json, &replay_config_json),
        error_capture = debug_runtime_error_capture_source(&runtime_error_path_json),
        module_specifier_json = module_specifier_json,
        filename_json = filename_json,
    );

    fs::write(&runner_path, runner_code)?;

    Ok(DebugExecution {
        runner_path,
        runtime_error_path,
        runtime_replay_path,
        runtime_step_path,
        replay_file: artifact_file,
        args,
        module_debug_outputs,
    })
}

fn prepare_debug_test_execution(
    path: &Path,
    replay_path: &Path,
    test_id: &str,
    breakpoints: &DebugBreakpointSelectors,
    watches: &[String],
    action: &str,
    cursor: Option<&DebugStepCursor>,
) -> Result<DebugExecution, CliError> {
    let artifact_file = resolve_run_artifact_path(replay_path, false)?;
    let artifact = read_test_replay_artifact(&artifact_file)?;
    let test_files = collect_sigil_files(path)?;
    let (request, binding) = build_test_replay_binding(
        path,
        &test_files,
        artifact.request.match_filter.as_deref(),
        None,
    )?;
    validate_test_replay_binding(path, &request, &binding, &artifact, &artifact_file)?;
    if !artifact.selected_test_ids.iter().any(|id| id == test_id) {
        return Err(CliError::Breakpoint {
            code: codes::runtime::REPLAY_BINDING_MISMATCH.to_string(),
            message: format!("test replay artifact does not contain '{}'", test_id),
            details: json!({
                "path": path.to_string_lossy(),
                "testId": test_id
            }),
        });
    }
    let recorded_test = artifact
        .tests
        .iter()
        .find(|test| test.id == test_id)
        .ok_or_else(|| CliError::Breakpoint {
            code: codes::runtime::REPLAY_BINDING_MISMATCH.to_string(),
            message: format!("test replay artifact does not contain '{}'", test_id),
            details: json!({
                "path": path.to_string_lossy(),
                "testId": test_id
            }),
        })?;
    let replay_artifact =
        recorded_test
            .replay_artifact
            .clone()
            .ok_or_else(|| CliError::Breakpoint {
                code: codes::runtime::REPLAY_BINDING_MISMATCH.to_string(),
                message: format!("test replay artifact '{}' is missing replay data", test_id),
                details: json!({
                    "path": path.to_string_lossy(),
                    "testId": test_id
                }),
            })?;
    let test_file = canonicalize_existing_path(Path::new(&recorded_test.file));
    let graph = ModuleGraph::build(&test_file)?;
    let compiled = compile_entry_files_with_cache(
        &[test_file.clone()],
        Some(graph),
        None,
        None,
        true,
        true,
        true,
        OutputFlavor::RuntimeEsm,
    )?;
    let module_debug_outputs = build_runtime_module_debug_outputs(&compiled)?;
    let breakpoint_config = resolve_breakpoint_config(
        &test_file,
        &module_debug_outputs,
        &breakpoints.breakpoint_lines,
        &breakpoints.breakpoint_functions,
        &breakpoints.breakpoint_spans,
        BreakpointMode::Stop,
        32,
    )?;
    let trace_config_json = serde_json::to_string(&json!({
        "enabled": true,
        "maxEvents": 256,
        "expressions": true
    }))
    .map_err(|error| CliError::Codegen(format!("failed to encode debug trace config: {error}")))?;
    let breakpoint_config_json = debug_runtime_breakpoint_config_json(breakpoint_config.as_ref())?;
    let replay_config_json = serde_json::to_string(&json!({
        "mode": "replay",
        "file": artifact_file.to_string_lossy(),
        "artifact": replay_artifact
    }))
    .map_err(|error| CliError::Codegen(format!("failed to encode debug replay config: {error}")))?;
    let step_config_json =
        debug_step_config_json(DebugSessionTargetKind::Test, action, cursor, watches)?;

    let entry_output_path = compiled.entry_output_path;
    let test_dir = entry_output_path.parent().unwrap().join("__sigil_test");
    fs::create_dir_all(&test_dir)?;
    let unique = format!(
        "{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let runner_path = test_dir.join(format!(
        "{}.{}.debug.runner.mjs",
        entry_output_path.file_stem().unwrap().to_string_lossy(),
        unique
    ));
    let runtime_error_path = unique_runtime_error_path(&entry_output_path);
    let runtime_replay_path = unique_runtime_replay_path(&entry_output_path);
    let runtime_step_path = unique_runtime_step_path(&entry_output_path);
    let runtime_error_path_json =
        serde_json::to_string(&runtime_error_path.to_string_lossy().to_string()).unwrap();
    let runtime_replay_path_json =
        serde_json::to_string(&runtime_replay_path.to_string_lossy().to_string()).unwrap();
    let runtime_step_path_json =
        serde_json::to_string(&runtime_step_path.to_string_lossy().to_string()).unwrap();
    let abs_ts_file = fs::canonicalize(&entry_output_path)?;
    let module_url = format!("file://{}", abs_ts_file.display());
    let module_url_json = serde_json::to_string(&module_url).unwrap();
    let test_id_json = serde_json::to_string(test_id).unwrap();

    let runner_code = format!(
        r#"import {{ writeFile }} from 'node:fs/promises';
import {{ writeFileSync }} from 'node:fs';

{step_runtime}
globalThis.__sigil_trace_config = {trace_config_json};
globalThis.__sigil_trace_current = undefined;
globalThis.__sigil_breakpoint_config = {breakpoint_config_json};
globalThis.__sigil_breakpoint_current = undefined;
{replay_capture}
{error_capture}

const moduleUrl = {module_url_json};
const selectedTestId = {test_id_json};

function __sigil_debug_summary(value) {{
  return typeof globalThis.__sigil_trace_summary === 'function'
    ? globalThis.__sigil_trace_summary(value, 1)
    : {{ kind: typeof value }};
}}

try {{
  const discoverMod = globalThis.__sigil_program_exports ?? await import(moduleUrl);
  const tests = Array.isArray(discoverMod.__sigil_tests) ? discoverMod.__sigil_tests : [];
  const selected = tests.find((candidate) => String(candidate.id) === selectedTestId);
  if (!selected) {{
    const error = new Error(`debug test '${{selectedTestId}}' not found in compiled module`);
    error.sigilCode = 'SIGIL-RUNTIME-REPLAY-BINDING-MISMATCH';
    throw error;
  }}
  globalThis.__sigil_replay_config = {replay_config_json};
  const freshMod = await import(moduleUrl + '?sigil_debug=' + encodeURIComponent(String(selected.id)) + '&ts=' + Date.now() + '_' + Math.random());
  if (typeof globalThis.__sigil_runtime_apply_program_world === 'function') {{
    await globalThis.__sigil_runtime_apply_program_world(freshMod);
  }}
  const freshTests = Array.isArray(freshMod.__sigil_tests) ? freshMod.__sigil_tests : [];
  const freshTest = freshTests.find((candidate) => String(candidate.id) === selectedTestId);
  if (!freshTest) {{
    const error = new Error(`debug test '${{selectedTestId}}' not found in isolated module reload`);
    error.sigilCode = 'SIGIL-RUNTIME-REPLAY-BINDING-MISMATCH';
    throw error;
  }}
  globalThis.__sigil_debug_step_event({{
    kind: 'test_enter',
    moduleId: selected.moduleId ?? null,
    sourceFile: selected.sourceFile ?? String(selected.id).split('::')[0],
    spanId: selected.spanId ?? null,
    spanKind: selected.spanKind ?? 'test_decl',
    declarationKind: 'test_decl',
    declarationLabel: String(selected.name ?? selected.description ?? ''),
    testId: String(selected.id),
    testName: String(selected.name ?? selected.description ?? ''),
    matched: [],
    locals: [],
    stack: [],
    recentTrace: [],
    frameDepth: 0,
    expressionDepth: 0
  }});
  const value = await freshTest.fn();
  const ok = value === true || (value && typeof value === 'object' && 'ok' in value && value.ok === true);
  const testStatus = ok ? 'pass' : 'fail';
  globalThis.__sigil_debug_step_event({{
    kind: 'test_return',
    moduleId: selected.moduleId ?? null,
    sourceFile: selected.sourceFile ?? String(selected.id).split('::')[0],
    spanId: selected.spanId ?? null,
    spanKind: selected.spanKind ?? 'test_decl',
    declarationKind: 'test_decl',
    declarationLabel: String(selected.name ?? selected.description ?? ''),
    testId: String(selected.id),
    testName: String(selected.name ?? selected.description ?? ''),
    testStatus,
    matched: [],
    locals: [],
    stack: [],
    recentTrace: typeof globalThis.__sigil_breakpoint_recent_trace === 'function' ? globalThis.__sigil_breakpoint_recent_trace() : [],
    frameDepth: 0,
    expressionDepth: 0,
    lastCompleted: {{ kind: 'test_return', testId: String(selected.id), testStatus, value: __sigil_debug_summary(value) }}
  }});
  if (typeof globalThis.__sigil_debug_mark_completed === 'function') {{
    globalThis.__sigil_debug_mark_completed('test_exit', {{
      moduleId: selected.moduleId ?? null,
      sourceFile: selected.sourceFile ?? String(selected.id).split('::')[0],
      spanId: selected.spanId ?? null,
      spanKind: selected.spanKind ?? 'test_decl',
      declarationKind: 'test_decl',
      declarationLabel: String(selected.name ?? selected.description ?? ''),
      testId: String(selected.id),
      testName: String(selected.name ?? selected.description ?? ''),
      testStatus
    }});
  }}
}} catch (error) {{
  if (__sigil_runtime_is_intentional_debug_stop(error)) {{
    // Intentional debug pause.
  }} else {{
    const captured = await __sigil_runtime_capture_error(error);
    if (typeof globalThis.__sigil_debug_mark_failed === 'function') {{
      globalThis.__sigil_debug_mark_failed(captured);
    }}
    if (captured.stack) {{
      console.error(captured.stack);
    }} else {{
      console.error(`${{captured.name}}: ${{captured.message}}`);
    }}
    process.exit(1);
  }}
}}
"#,
        step_runtime = debug_step_runtime_source(&runtime_step_path_json, &step_config_json),
        trace_config_json = trace_config_json,
        breakpoint_config_json = breakpoint_config_json,
        replay_capture =
            debug_runtime_replay_capture_source(&runtime_replay_path_json, &replay_config_json),
        error_capture = debug_runtime_error_capture_source(&runtime_error_path_json),
        module_url_json = module_url_json,
        test_id_json = test_id_json,
        replay_config_json = replay_config_json,
    );

    fs::write(&runner_path, runner_code)?;

    Ok(DebugExecution {
        runner_path,
        runtime_error_path,
        runtime_replay_path,
        runtime_step_path,
        replay_file: artifact_file,
        args: Vec::new(),
        module_debug_outputs,
    })
}

fn execute_debug_execution(execution: &DebugExecution) -> Result<serde_json::Value, CliError> {
    let runtime_output = execute_runner(
        &execution.runner_path,
        &execution.runtime_error_path,
        None,
        None,
        Some(&execution.runtime_replay_path),
        Some(&execution.runtime_step_path),
        &execution.args,
        false,
    )?;
    Ok(debug_snapshot_json(
        runtime_output.step_capture.as_ref(),
        &runtime_output,
        &execution.module_debug_outputs,
        &execution.replay_file,
    ))
}

pub fn debug_run_start_command(
    file: &Path,
    replay_path: &Path,
    watch_selectors: &[String],
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
) -> Result<(), CliError> {
    let watches = validate_debug_watch_selectors(DebugSessionTargetKind::Run, watch_selectors)?;
    let breakpoints = DebugBreakpointSelectors {
        breakpoint_lines: breakpoint_lines.to_vec(),
        breakpoint_functions: breakpoint_functions.to_vec(),
        breakpoint_spans: breakpoint_spans.to_vec(),
    };
    let snapshot = execute_debug_execution(&prepare_debug_run_execution(
        file,
        replay_path,
        &breakpoints,
        &watches,
        "start",
        None,
    )?)?;
    let session_path = unique_debug_session_path(DebugSessionTargetKind::Run)?;
    let session = DebugSessionFile {
        format_version: 1,
        kind: "sigilDebugSession".to_string(),
        target_kind: DebugSessionTargetKind::Run,
        session_id: session_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        replay_file: resolve_run_artifact_path(replay_path, false)?
            .to_string_lossy()
            .to_string(),
        path: canonicalize_existing_path(file)
            .to_string_lossy()
            .to_string(),
        test_id: None,
        breakpoints,
        watches,
        state: debug_session_state_from_snapshot(&snapshot),
        snapshot,
    };
    write_debug_session(&session_path, &session)?;
    output_debug_success(&session_path, &session);
    Ok(())
}

pub fn debug_run_session_command(
    action: DebugControlAction,
    session_path: &Path,
) -> Result<(), CliError> {
    let resolved_session = resolve_debug_session_path(session_path)?;
    let mut session = read_debug_session(&resolved_session)?;
    if session.target_kind != DebugSessionTargetKind::Run {
        output_json_error_to(
            "sigilc debug run",
            "cli",
            codes::cli::UNEXPECTED,
            "debug session target does not match `sigil debug run`",
            json!({
                "session": resolved_session.to_string_lossy(),
                "targetKind": session.target_kind.as_str()
            }),
            false,
        );
        return Err(CliError::Reported(1));
    }
    match action {
        DebugControlAction::Snapshot => {
            output_debug_success(&resolved_session, &session);
            Ok(())
        }
        DebugControlAction::Close => {
            session.state = DebugSessionState::Closed;
            if let Some(snapshot) = session.snapshot.as_object_mut() {
                snapshot.insert("state".to_string(), json!("closed"));
            }
            let _ = fs::remove_file(&resolved_session);
            output_debug_success(&resolved_session, &session);
            Ok(())
        }
        _ => {
            if session.state != DebugSessionState::Paused {
                return debug_session_state_error(
                    DebugSessionTargetKind::Run,
                    &resolved_session,
                    &session,
                );
            }
            let cursor = debug_snapshot_cursor(&session.snapshot);
            let snapshot = execute_debug_execution(&prepare_debug_run_execution(
                Path::new(&session.path),
                Path::new(&session.replay_file),
                &session.breakpoints,
                &session.watches,
                action.as_step_action().unwrap(),
                Some(&cursor),
            )?)?;
            session.state = debug_session_state_from_snapshot(&snapshot);
            session.snapshot = snapshot;
            write_debug_session(&resolved_session, &session)?;
            output_debug_success(&resolved_session, &session);
            Ok(())
        }
    }
}

pub fn debug_test_start_command(
    path: &Path,
    replay_path: &Path,
    test_id: Option<&str>,
    watch_selectors: &[String],
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
) -> Result<(), CliError> {
    let Some(test_id) = test_id else {
        output_json_error_to(
            "sigilc debug test",
            "cli",
            codes::cli::USAGE,
            "`sigil debug test start` requires `--test <id>`",
            json!({
                "path": path.to_string_lossy(),
                "option": "--test"
            }),
            false,
        );
        return Err(CliError::Reported(1));
    };
    let watches = validate_debug_watch_selectors(DebugSessionTargetKind::Test, watch_selectors)?;
    let breakpoints = DebugBreakpointSelectors {
        breakpoint_lines: breakpoint_lines.to_vec(),
        breakpoint_functions: breakpoint_functions.to_vec(),
        breakpoint_spans: breakpoint_spans.to_vec(),
    };
    let snapshot = execute_debug_execution(&prepare_debug_test_execution(
        path,
        replay_path,
        test_id,
        &breakpoints,
        &watches,
        "start",
        None,
    )?)?;
    let session_path = unique_debug_session_path(DebugSessionTargetKind::Test)?;
    let session = DebugSessionFile {
        format_version: 1,
        kind: "sigilDebugSession".to_string(),
        target_kind: DebugSessionTargetKind::Test,
        session_id: session_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        replay_file: resolve_run_artifact_path(replay_path, false)?
            .to_string_lossy()
            .to_string(),
        path: resolve_debug_session_path(path)?
            .to_string_lossy()
            .to_string(),
        test_id: Some(test_id.to_string()),
        breakpoints,
        watches,
        state: debug_session_state_from_snapshot(&snapshot),
        snapshot,
    };
    write_debug_session(&session_path, &session)?;
    output_debug_success(&session_path, &session);
    Ok(())
}

pub fn debug_test_session_command(
    action: DebugControlAction,
    session_path: &Path,
) -> Result<(), CliError> {
    let resolved_session = resolve_debug_session_path(session_path)?;
    let mut session = read_debug_session(&resolved_session)?;
    if session.target_kind != DebugSessionTargetKind::Test {
        output_json_error_to(
            "sigilc debug test",
            "cli",
            codes::cli::UNEXPECTED,
            "debug session target does not match `sigil debug test`",
            json!({
                "session": resolved_session.to_string_lossy(),
                "targetKind": session.target_kind.as_str()
            }),
            false,
        );
        return Err(CliError::Reported(1));
    }
    match action {
        DebugControlAction::Snapshot => {
            output_debug_success(&resolved_session, &session);
            Ok(())
        }
        DebugControlAction::Close => {
            session.state = DebugSessionState::Closed;
            if let Some(snapshot) = session.snapshot.as_object_mut() {
                snapshot.insert("state".to_string(), json!("closed"));
            }
            let _ = fs::remove_file(&resolved_session);
            output_debug_success(&resolved_session, &session);
            Ok(())
        }
        _ => {
            if session.state != DebugSessionState::Paused {
                return debug_session_state_error(
                    DebugSessionTargetKind::Test,
                    &resolved_session,
                    &session,
                );
            }
            let cursor = debug_snapshot_cursor(&session.snapshot);
            let snapshot = execute_debug_execution(&prepare_debug_test_execution(
                Path::new(&session.path),
                Path::new(&session.replay_file),
                session.test_id.as_deref().unwrap_or_default(),
                &session.breakpoints,
                &session.watches,
                action.as_step_action().unwrap(),
                Some(&cursor),
            )?)?;
            session.state = debug_session_state_from_snapshot(&snapshot);
            session.snapshot = snapshot;
            write_debug_session(&resolved_session, &session)?;
            output_debug_success(&resolved_session, &session);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_exception_capture_from_stderr_extracts_sigil_code() {
        let capture = runtime_exception_capture_from_stderr(
            "Error: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil\n    at main (/tmp/example.run.mjs:12:3)",
        )
        .expect("expected stderr capture");

        assert_eq!(capture.name, "Error");
        assert_eq!(
            capture.sigil_code.as_deref(),
            Some(codes::topology::ENV_NOT_FOUND)
        );
        assert_eq!(
            capture.message,
            "SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil"
        );
        assert!(capture.stack.contains(".run.mjs"));
    }

    #[test]
    fn runtime_exception_capture_from_stderr_prefers_sigil_line_after_warning() {
        let capture = runtime_exception_capture_from_stderr(
            "(node:2468) ExperimentalWarning: import assertions are deprecated\nError: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil\n    at main (/tmp/example.run.mjs:12:3)",
        )
        .expect("expected stderr capture");

        assert_eq!(capture.name, "Error");
        assert_eq!(
            capture.sigil_code.as_deref(),
            Some(codes::topology::ENV_NOT_FOUND)
        );
        assert_eq!(
            capture.message,
            "SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil"
        );
        assert!(capture.stack.contains("ExperimentalWarning"));
    }

    #[test]
    fn recover_runtime_exception_code_uses_message_when_sidecar_code_is_missing() {
        assert_eq!(
            recover_runtime_exception_code(
                "SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil",
                "Error: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil"
            )
            .as_deref(),
            Some(codes::topology::ENV_NOT_FOUND)
        );
    }

    #[test]
    fn recover_runtime_exception_code_falls_back_to_stack() {
        assert_eq!(
            recover_runtime_exception_code(
                "environment 'staging' not declared in src/topology.lib.sigil",
                "Error: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil\n    at main (/tmp/example.run.mjs:12:3)"
            )
            .as_deref(),
            Some(codes::topology::ENV_NOT_FOUND)
        );
    }

    #[test]
    fn resolved_runtime_exception_code_prefers_recovered_specific_code() {
        let capture = RuntimeExceptionCapture {
            name: "Error".to_string(),
            message: "SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil".to_string(),
            stack: "Error: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil\n    at main (/tmp/example.run.mjs:12:3)".to_string(),
            sigil_code: Some(codes::runtime::UNCAUGHT_EXCEPTION.to_string()),
            expression: None,
        };

        assert_eq!(
            resolved_runtime_exception_code(&capture),
            codes::topology::ENV_NOT_FOUND
        );
    }
}

fn map_generated_frame_to_sigil(
    frame: &ParsedGeneratedFrame,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> Option<MappedSigilFrame> {
    let frame_path = normalize_generated_frame_path(&frame.file);
    let module = module_debug_outputs
        .iter()
        .find(|module| module.output_file == frame_path)?;
    let span = span_for_generated_line(&module.span_map, frame.line)?;
    Some(MappedSigilFrame {
        excerpt: declaration_excerpt(&span),
        span,
    })
}

fn map_runtime_expression_to_sigil(
    capture: &RuntimeExpressionCapture,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> Option<MappedSigilExpression> {
    let span = find_debug_span(module_debug_outputs, &capture.module_id, &capture.span_id)?.clone();
    Some(MappedSigilExpression {
        span,
        capture: capture.clone(),
    })
}

fn declaration_frame_for_expression(
    expression: &MappedSigilExpression,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
) -> Option<MappedSigilFrame> {
    let module = module_debug_outputs
        .iter()
        .find(|module| module.module_id == expression.capture.module_id)?;
    let mut current = expression.span.clone();

    loop {
        if matches!(
            current.kind,
            DebugSpanKind::FunctionDecl | DebugSpanKind::ConstDecl | DebugSpanKind::TestDecl
        ) {
            return Some(MappedSigilFrame {
                excerpt: declaration_excerpt(&current),
                span: current,
            });
        }

        let parent_id = current.parent_span_id.as_deref()?;
        current = module
            .span_map
            .spans
            .iter()
            .find(|span| span.span_id == parent_id)?
            .clone();
    }
}

fn span_for_generated_line(span_map: &ModuleSpanMap, line: usize) -> Option<DebugSpanRecord> {
    span_map
        .spans
        .iter()
        .filter(|span| {
            span.parent_span_id.is_none()
                && matches!(
                    span.kind,
                    DebugSpanKind::FunctionDecl
                        | DebugSpanKind::ConstDecl
                        | DebugSpanKind::TestDecl
                )
        })
        .filter_map(|span| {
            let range = span.generated_range.as_ref()?;
            if line < range.start_line || line > range.end_line {
                return None;
            }
            Some((
                range.end_line.saturating_sub(range.start_line),
                span.clone(),
            ))
        })
        .min_by_key(|(width, _)| *width)
        .map(|(_, span)| span)
}

fn declaration_excerpt(span: &DebugSpanRecord) -> Option<SourceExcerpt> {
    let source = fs::read_to_string(&span.source_file).ok()?;
    let lines = source.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return None;
    }

    let decl_line = span.location.start.line.max(1);
    let start_line = if decl_line > 1 { decl_line - 1 } else { 1 };
    let end_line = usize::min(lines.len(), decl_line + 2);
    let text = (start_line..=end_line)
        .map(|line| {
            let content = lines.get(line - 1).copied().unwrap_or("");
            format!("{line:>4} | {content}")
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(SourceExcerpt {
        start_line,
        end_line,
        text,
    })
}

fn tee_reader<R: Read, W: Write>(mut reader: R, mut writer: W) -> io::Result<Vec<u8>> {
    let mut capture = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        writer.flush()?;
        capture.extend_from_slice(&buffer[..read]);
    }
    Ok(capture)
}

fn join_tee_output(
    handle: thread::JoinHandle<io::Result<Vec<u8>>>,
    stream_name: &str,
) -> Result<Vec<u8>, CliError> {
    match handle.join() {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(error)) => Err(CliError::Io(error)),
        Err(_) => Err(CliError::Runtime(format!(
            "{}: run {} forwarding thread panicked",
            codes::cli::UNEXPECTED,
            stream_name
        ))),
    }
}

fn map_runner_launch_error(error: io::Error) -> CliError {
    if error.kind() == io::ErrorKind::NotFound {
        CliError::Runtime(format!(
            "{}: node not found. Please install Node.js to run Sigil programs.",
            codes::runtime::ENGINE_NOT_FOUND
        ))
    } else {
        CliError::Runtime(format!(
            "{}: failed to execute run target: {}",
            codes::cli::UNEXPECTED,
            error
        ))
    }
}

fn output_run_error(file: &Path, error: &CliError, to_stderr: bool) {
    match error {
        CliError::Type(type_error) => output_json_error_to(
            "sigilc run",
            "typecheck",
            &type_error.code,
            &type_error.message,
            type_error_json_details(type_error),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::Validation(errors)) => {
            let message = errors
                .first()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "validation errors".to_string());
            let error_code = extract_error_code(&message);
            output_json_error_to(
                "sigilc run",
                "canonical",
                &error_code,
                &message,
                json!({
                    "file": file.to_string_lossy(),
                    "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                }),
                to_stderr,
            );
        }
        CliError::ModuleGraph(ModuleGraphError::ImportNotFound {
            module_id,
            expected_path,
        }) => output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::IMPORT_NOT_FOUND,
            &format!("module not found: {}", module_id),
            json!({
                "file": file.to_string_lossy(),
                "moduleId": module_id,
                "expectedPath": expected_path
            }),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::ImportCycle(cycle)) => output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::IMPORT_CYCLE,
            "module import cycle detected",
            json!({
                "file": file.to_string_lossy(),
                "cycle": cycle
            }),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::Io(error)) => output_json_error_to(
            "sigilc run",
            "io",
            codes::cli::UNEXPECTED,
            &error.to_string(),
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::SelectedConfigEnvRequired)
        | CliError::ModuleGraph(ModuleGraphError::SelectedConfigModuleNotFound { .. }) => {
            output_run_message_error(file, &error.to_string(), to_stderr);
        }
        CliError::ModuleGraph(ModuleGraphError::Lexer(message))
        | CliError::ModuleGraph(ModuleGraphError::Parser(message))
        | CliError::Lexer(message)
        | CliError::Parser(message)
        | CliError::Validation(message)
        | CliError::Runtime(message) => {
            output_run_message_error(file, message, to_stderr);
        }
        CliError::Breakpoint {
            code,
            message,
            details,
        } => output_json_error_to(
            "sigilc run",
            phase_for_code(code),
            code,
            message,
            details.clone(),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
        | CliError::ProjectConfig(project_error) => output_json_error_to(
            "sigilc run",
            phase_for_code(project_error.code()),
            project_error.code(),
            &project_error.to_string(),
            project_error_json_details(project_error, "file", file, serde_json::Map::new()),
            to_stderr,
        ),
        CliError::Io(error) => output_json_error_to(
            "sigilc run",
            "io",
            codes::cli::UNEXPECTED,
            &error.to_string(),
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::Codegen(message) => output_json_error_to(
            "sigilc run",
            "codegen",
            codes::cli::UNEXPECTED,
            message,
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::Reported(_) => {}
    }
}

fn output_run_message_error(file: &Path, message: &str, to_stderr: bool) {
    let error_code = extract_error_code(message);
    let (code, phase) = if error_code.starts_with("SIGIL-") {
        let phase = phase_for_code(&error_code);
        (error_code, phase)
    } else {
        (codes::cli::UNEXPECTED.to_string(), "cli")
    };

    output_json_error_to(
        "sigilc run",
        phase,
        &code,
        message,
        json!({
            "file": file.to_string_lossy()
        }),
        to_stderr,
    );
}

fn output_test_error(path: &Path, error: &CliError) {
    match error {
        CliError::Type(type_error) => {
            let message = type_error.to_string();
            let error_code = extract_error_code(&message);
            output_json_error_to(
                "sigilc test",
                "typecheck",
                &error_code,
                &message,
                json!({
                    "path": path.to_string_lossy()
                }),
                false,
            );
        }
        CliError::Validation(message) => output_test_message_error(path, message),
        CliError::Lexer(message) | CliError::Parser(message) | CliError::Runtime(message) => {
            output_test_message_error(path, message);
        }
        CliError::Breakpoint {
            code,
            message,
            details,
        } => output_json_error_to(
            "sigilc test",
            phase_for_code(code),
            code,
            message,
            details.clone(),
            false,
        ),
        CliError::ModuleGraph(ModuleGraphError::ImportNotFound {
            module_id,
            expected_path,
        }) => output_json_error_to(
            "sigilc test",
            "cli",
            codes::cli::IMPORT_NOT_FOUND,
            &format!("module not found: {}", module_id),
            json!({
                "path": path.to_string_lossy(),
                "moduleId": module_id,
                "expectedPath": expected_path
            }),
            false,
        ),
        CliError::ModuleGraph(ModuleGraphError::ImportCycle(cycle)) => output_json_error_to(
            "sigilc test",
            "cli",
            codes::cli::IMPORT_CYCLE,
            "module import cycle detected",
            json!({
                "path": path.to_string_lossy(),
                "cycle": cycle
            }),
            false,
        ),
        CliError::ModuleGraph(ModuleGraphError::Io(io_error)) => output_json_error_to(
            "sigilc test",
            "io",
            codes::cli::UNEXPECTED,
            &io_error.to_string(),
            json!({
                "path": path.to_string_lossy()
            }),
            false,
        ),
        CliError::ModuleGraph(ModuleGraphError::Validation(errors)) => {
            let message = errors
                .iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            output_test_message_error(path, &message);
        }
        CliError::ModuleGraph(ModuleGraphError::SelectedConfigEnvRequired)
        | CliError::ModuleGraph(ModuleGraphError::SelectedConfigModuleNotFound { .. }) => {
            output_test_message_error(path, &error.to_string());
        }
        CliError::ModuleGraph(ModuleGraphError::Lexer(message))
        | CliError::ModuleGraph(ModuleGraphError::Parser(message)) => {
            output_test_message_error(path, message);
        }
        CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
        | CliError::ProjectConfig(project_error) => output_json_error_to(
            "sigilc test",
            phase_for_code(project_error.code()),
            project_error.code(),
            &project_error.to_string(),
            project_error_json_details(project_error, "path", path, serde_json::Map::new()),
            false,
        ),
        CliError::Io(io_error) => output_json_error_to(
            "sigilc test",
            "io",
            codes::cli::UNEXPECTED,
            &io_error.to_string(),
            json!({
                "path": path.to_string_lossy()
            }),
            false,
        ),
        CliError::Codegen(message) => output_json_error_to(
            "sigilc test",
            "codegen",
            codes::cli::UNEXPECTED,
            message,
            json!({
                "path": path.to_string_lossy()
            }),
            false,
        ),
        CliError::Reported(_) => {}
    }
}

fn output_test_message_error(path: &Path, message: &str) {
    let error_code = extract_error_code(message);
    let (code, phase) = if error_code.starts_with("SIGIL-") {
        let phase = phase_for_code(&error_code);
        (error_code, phase)
    } else {
        (codes::cli::UNEXPECTED.to_string(), "cli")
    };

    output_json_error_to(
        "sigilc test",
        phase,
        &code,
        message,
        json!({
            "path": path.to_string_lossy()
        }),
        false,
    );
}

fn phase_for_code(code: &str) -> &'static str {
    if code.starts_with("SIGIL-LEX-") {
        "lexer"
    } else if code.starts_with("SIGIL-PARSE-") {
        "parser"
    } else if code.starts_with("SIGIL-CANON-") {
        "canonical"
    } else if code.starts_with("SIGIL-TYPE-") {
        "typecheck"
    } else if code.starts_with("SIGIL-TOPO-") {
        "topology"
    } else if code.starts_with("SIGIL-RUNTIME-") || code.starts_with("SIGIL-RUN-") {
        "runtime"
    } else if code.starts_with("SIGIL-MUTABILITY-") {
        "mutability"
    } else {
        "cli"
    }
}

/// Test command: run Sigil tests from a directory
pub fn test_command(
    path: &Path,
    selected_env: Option<&str>,
    match_filter: Option<&str>,
    trace_enabled: bool,
    trace_expr_enabled: bool,
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
    breakpoint_collect: bool,
    breakpoint_max_hits: usize,
    record_path: Option<&Path>,
    replay_path: Option<&Path>,
) -> Result<(), CliError> {
    let breakpoint_mode = if breakpoint_collect {
        BreakpointMode::Collect
    } else {
        BreakpointMode::Stop
    };
    let debug_options = TestDebugOptions {
        trace_enabled,
        trace_expr_enabled,
        breakpoint_lines: breakpoint_lines.to_vec(),
        breakpoint_functions: breakpoint_functions.to_vec(),
        breakpoint_spans: breakpoint_spans.to_vec(),
        breakpoint_mode,
        breakpoint_max_hits,
    };

    if trace_expr_enabled && !trace_enabled {
        output_json_error_to(
            "sigilc test",
            "cli",
            codes::cli::USAGE,
            "`--trace-expr` requires `--trace`",
            json!({
                "path": path.to_string_lossy(),
                "option": "--trace-expr",
                "requires": ["--trace"]
            }),
            false,
        );
        return Err(CliError::Reported(1));
    }

    if debug_options.breakpoints_requested() && breakpoint_max_hits == 0 {
        output_json_error_to(
            "sigilc test",
            "cli",
            codes::cli::USAGE,
            "`--break-max-hits` must be at least 1",
            json!({
                "path": path.to_string_lossy(),
                "option": "--break-max-hits",
                "minimum": 1
            }),
            false,
        );
        return Err(CliError::Reported(1));
    }

    if replay_path.is_some() && selected_env.is_some() {
        output_json_error_to(
            "sigilc test",
            "cli",
            codes::cli::USAGE,
            "`--replay` cannot be combined with `--env`",
            json!({
                "path": path.to_string_lossy(),
                "option": "--replay",
                "conflictsWith": "--env"
            }),
            false,
        );
        return Err(CliError::Reported(1));
    }

    // Check if tests directory exists
    if !path.exists() {
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc test",
            "ok": true,
            "summary": {
                "files": 0,
                "discovered": 0,
                "selected": 0,
                "passed": 0,
                "failed": 0,
                "errored": 0,
                "stopped": 0,
                "skipped": 0,
                "durationMs": 0
            },
            "results": []
        });
        println!("{}", serde_json::to_string(&output_json).unwrap());
        return Ok(());
    }

    let start_time = Instant::now();

    // Collect all .sigil files in test directory
    let test_files = collect_sigil_files(path)?;
    let suite_replay_mode = match prepare_test_replay_mode(
        path,
        &test_files,
        match_filter,
        selected_env,
        record_path,
        replay_path,
    ) {
        Ok(mode) => mode,
        Err(error) => {
            output_test_error(path, &error);
            return Err(CliError::Reported(1));
        }
    };
    let enforce_project_coverage = match_filter.is_none()
        && !path.is_file()
        && !(debug_options.breakpoints_requested() && breakpoint_mode == BreakpointMode::Stop);

    let run_test_file = |test_file: &PathBuf| {
        compile_and_run_tests(
            test_file,
            selected_env,
            match_filter,
            &debug_options,
            suite_replay_mode.as_ref(),
        )
    };

    let results: Vec<_> = if test_files.len() <= 1 {
        test_files.iter().map(run_test_file).collect()
    } else {
        // The SSG integration suite overflows Rayon’s default worker stack on Linux.
        // Use an explicit pool so `sigil test <dir>` is stable in CI without env hacks.
        let thread_pool = ThreadPoolBuilder::new()
            .thread_name(|index| format!("sigil-test-{index}"))
            .stack_size(TEST_WORKER_STACK_BYTES)
            .build()
            .map_err(|err| {
                CliError::Runtime(format!("Failed to configure test worker pool: {}", err))
            })?;

        thread_pool.install(|| test_files.par_iter().map(run_test_file).collect())
    };

    if let Some(error) = results.iter().find_map(|result| result.as_ref().err()) {
        output_test_error(path, error);
        return Err(CliError::Reported(1));
    }

    // Aggregate results from all files
    let mut all_results = Vec::new();
    let mut observed_calls = HashSet::new();
    let mut observed_variants: HashMap<String, HashSet<String>> = HashMap::new();
    let mut coverage_targets = HashMap::new();
    let mut discovered = 0;
    let mut selected = 0;
    let mut selected_ids = Vec::new();
    let mut recorded_tests = Vec::new();

    for result in results {
        if let Ok(test_result) = result {
            discovered += test_result.discovered;
            selected += test_result.selected;
            selected_ids.extend(test_result.selected_ids);
            observed_calls.extend(test_result.coverage_observation.calls);
            for (key, tags) in test_result.coverage_observation.variants {
                observed_variants.entry(key).or_default().extend(tags);
            }
            for target in test_result.coverage_targets {
                coverage_targets.entry(target.id.clone()).or_insert(target);
            }
            recorded_tests.extend(test_result.recorded_tests);
            all_results.extend(test_result.results);
        }
    }

    if let Some(PreparedTestReplayMode::Replay { artifact, .. }) = suite_replay_mode.as_ref() {
        if artifact.selected_test_ids != selected_ids {
            output_json_error_to(
                "sigilc test",
                "runtime",
                codes::runtime::REPLAY_BINDING_MISMATCH,
                "replay artifact selected tests do not match this run",
                json!({
                    "path": path.to_string_lossy(),
                    "expectedSelectedTestIds": artifact.selected_test_ids,
                    "actualSelectedTestIds": selected_ids
                }),
                false,
            );
            return Err(CliError::Reported(1));
        }
    }

    if enforce_project_coverage {
        for target in coverage_targets.into_values() {
            if !observed_calls.contains(&target.id) {
                all_results.push(TestResult {
                    id: format!("{}::coverage", target.id),
                    file: target.file.clone(),
                    name: format!("coverage {}", target.function_name),
                    status: "fail".to_string(),
                    duration_ms: 0,
                    location: target.location.clone(),
                    failure: Some(format!(
                        "sigil test requires '{}' to be executed by the test suite",
                        target.id
                    )),
                    trace: None,
                    breakpoints: None,
                    replay: None,
                    exception: None,
                });
                continue;
            }

            if !target.expected_variants.is_empty() {
                let observed = observed_variants.get(&target.id);
                let mut missing = target
                    .expected_variants
                    .iter()
                    .filter(|variant| {
                        observed.is_none_or(|tags| !tags.contains((*variant).as_str()))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                missing.sort();

                if !missing.is_empty() {
                    all_results.push(TestResult {
                        id: format!("{}::coverage-variants", target.id),
                        file: target.file.clone(),
                        name: format!("coverage variants {}", target.function_name),
                        status: "fail".to_string(),
                        duration_ms: 0,
                        location: target.location.clone(),
                        failure: Some(format!(
                            "sigil test requires '{}' to observe variants [{}]",
                            target.id,
                            missing.join(", ")
                        )),
                        trace: None,
                        breakpoints: None,
                        replay: None,
                        exception: None,
                    });
                }
            }
        }
    }

    // Sort results by file, then line, then column
    all_results.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.location.line.cmp(&b.location.line))
            .then_with(|| a.location.column.cmp(&b.location.column))
    });

    let passed = all_results.iter().filter(|r| r.status == "pass").count();
    let failed = all_results.iter().filter(|r| r.status == "fail").count();
    let errored = all_results.iter().filter(|r| r.status == "error").count();
    let stopped = all_results.iter().filter(|r| r.status == "stopped").count();
    let duration_ms = start_time.elapsed().as_millis();

    let ok = failed == 0 && errored == 0 && stopped == 0;

    if let Some(PreparedTestReplayMode::Record {
        artifact_file,
        request,
        binding,
    }) = suite_replay_mode.as_ref()
    {
        let artifact = TestReplayArtifact {
            format_version: 1,
            kind: "sigilTestReplay".to_string(),
            request: request.clone(),
            binding: binding.clone(),
            selected_test_ids: selected_ids.clone(),
            summary: TestReplayArtifactSummary {
                failed: failed > 0 || errored > 0,
                stopped: stopped > 0,
                selected,
                recorded_events: recorded_tests
                    .iter()
                    .filter_map(|test| test.replay_artifact.as_ref())
                    .map(|artifact| artifact.summary.recorded_events)
                    .sum(),
            },
            tests: recorded_tests.clone(),
        };
        let serialized = serde_json::to_string(&artifact).map_err(|error| {
            CliError::Runtime(format!(
                "{}: failed to serialize test replay artifact '{}': {}",
                codes::runtime::REPLAY_INVALID_ARTIFACT,
                artifact_file.display(),
                error
            ))
        })?;
        fs::write(artifact_file, serialized).map_err(|error| {
            CliError::Runtime(format!(
                "{}: failed to write test replay artifact '{}': {}",
                codes::runtime::REPLAY_INVALID_ARTIFACT,
                artifact_file.display(),
                error
            ))
        })?;
    }

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc test",
        "ok": ok,
        "summary": {
            "files": test_files.len(),
            "discovered": discovered,
            "selected": selected,
            "passed": passed,
            "failed": failed,
            "errored": errored,
            "stopped": stopped,
            "skipped": 0,
            "durationMs": duration_ms
        },
        "results": all_results
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    if !ok {
        return Err(CliError::Reported(1));
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TestResult {
    id: String,
    file: String,
    name: String,
    status: String,
    #[serde(rename = "durationMs")]
    duration_ms: u128,
    location: TestLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    breakpoints: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exception: Option<serde_json::Value>,
}

struct TestRunResult {
    discovered: usize,
    selected: usize,
    selected_ids: Vec<String>,
    results: Vec<TestResult>,
    coverage_observation: CoverageObservation,
    coverage_targets: Vec<CoverageTarget>,
    recorded_tests: Vec<TestReplayRecordedTest>,
}

#[derive(Debug, Clone)]
struct TestDebugOptions {
    trace_enabled: bool,
    trace_expr_enabled: bool,
    breakpoint_lines: Vec<String>,
    breakpoint_functions: Vec<String>,
    breakpoint_spans: Vec<String>,
    breakpoint_mode: BreakpointMode,
    breakpoint_max_hits: usize,
}

impl TestDebugOptions {
    fn breakpoints_requested(&self) -> bool {
        !self.breakpoint_lines.is_empty()
            || !self.breakpoint_functions.is_empty()
            || !self.breakpoint_spans.is_empty()
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTestRunOutput {
    discovered: usize,
    selected: usize,
    #[serde(default)]
    selected_ids: Vec<String>,
    #[serde(default)]
    coverage_targets: Vec<String>,
    #[serde(default)]
    results: Vec<RawTestResult>,
    #[serde(default)]
    recorded_tests: Vec<TestReplayRecordedTest>,
    #[serde(default)]
    runner_error: Option<RawTestRunnerError>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTestResult {
    id: String,
    file: String,
    name: String,
    status: String,
    #[serde(rename = "durationMs")]
    duration_ms: u128,
    location: TestLocation,
    #[serde(default)]
    failure: Option<String>,
    #[serde(default)]
    coverage: RawCoverageObservation,
    #[serde(default)]
    trace: Option<RuntimeTraceCapture>,
    #[serde(default)]
    breakpoints: Option<RuntimeBreakpointCapture>,
    #[serde(default)]
    replay: Option<RuntimeReplayCapture>,
    #[serde(default)]
    exception: Option<RuntimeExceptionCapture>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCoverageObservation {
    #[serde(default)]
    calls: Vec<String>,
    #[serde(default)]
    variants: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTestRunnerError {
    code: String,
    message: String,
    #[serde(default)]
    details: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestReplayArtifact {
    format_version: u32,
    kind: String,
    request: TestReplayArtifactRequest,
    binding: ReplayArtifactBinding,
    #[serde(default)]
    selected_test_ids: Vec<String>,
    summary: TestReplayArtifactSummary,
    #[serde(default)]
    tests: Vec<TestReplayRecordedTest>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestReplayArtifactRequest {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_root: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestReplayArtifactSummary {
    failed: bool,
    stopped: bool,
    selected: usize,
    recorded_events: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestReplayRecordedTest {
    id: String,
    file: String,
    name: String,
    status: String,
    location: TestLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay_artifact: Option<ReplayArtifact>,
}

#[derive(Debug, Clone)]
enum PreparedTestReplayMode {
    Record {
        artifact_file: PathBuf,
        request: TestReplayArtifactRequest,
        binding: ReplayArtifactBinding,
    },
    Replay {
        artifact_file: PathBuf,
        artifact: TestReplayArtifact,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugControlAction {
    Snapshot,
    StepInto,
    StepOver,
    StepOut,
    Continue,
    Close,
}

impl DebugControlAction {
    fn as_step_action(self) -> Option<&'static str> {
        match self {
            DebugControlAction::Snapshot | DebugControlAction::Close => None,
            DebugControlAction::StepInto => Some("stepInto"),
            DebugControlAction::StepOver => Some("stepOver"),
            DebugControlAction::StepOut => Some("stepOut"),
            DebugControlAction::Continue => Some("continue"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum DebugSessionTargetKind {
    Run,
    Test,
}

impl DebugSessionTargetKind {
    fn as_str(self) -> &'static str {
        match self {
            DebugSessionTargetKind::Run => "run",
            DebugSessionTargetKind::Test => "test",
        }
    }

    fn command_name(self) -> &'static str {
        match self {
            DebugSessionTargetKind::Run => "sigilc debug run",
            DebugSessionTargetKind::Test => "sigilc debug test",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum DebugSessionState {
    Paused,
    Completed,
    Failed,
    Closed,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugBreakpointSelectors {
    breakpoint_lines: Vec<String>,
    breakpoint_functions: Vec<String>,
    breakpoint_spans: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DebugSessionFile {
    format_version: u32,
    kind: String,
    target_kind: DebugSessionTargetKind,
    session_id: String,
    replay_file: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    test_id: Option<String>,
    breakpoints: DebugBreakpointSelectors,
    #[serde(default)]
    watches: Vec<String>,
    state: DebugSessionState,
    snapshot: serde_json::Value,
}

#[derive(Debug, Clone)]
struct DebugStepCursor {
    seq: usize,
    event_kind: Option<String>,
    span_id: Option<String>,
    frame_depth: usize,
    expression_depth: usize,
    test_id: Option<String>,
}

fn debug_snapshot_cursor(snapshot: &serde_json::Value) -> DebugStepCursor {
    DebugStepCursor {
        seq: snapshot["seq"].as_u64().unwrap_or(0) as usize,
        event_kind: snapshot["eventKind"].as_str().map(str::to_string),
        span_id: snapshot["spanId"].as_str().map(str::to_string),
        frame_depth: snapshot["frameDepth"].as_u64().unwrap_or(0) as usize,
        expression_depth: snapshot["expressionDepth"].as_u64().unwrap_or(0) as usize,
        test_id: snapshot["testId"].as_str().map(str::to_string),
    }
}

fn unique_debug_session_path(target_kind: DebugSessionTargetKind) -> Result<PathBuf, CliError> {
    let cwd = std::env::current_dir()?;
    let debug_dir = cwd.join(".local").join("debug");
    fs::create_dir_all(&debug_dir)?;
    let unique = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    Ok(debug_dir.join(format!(
        "sigil-debug-{}.{}.session.json",
        target_kind.as_str(),
        unique
    )))
}

fn resolve_debug_session_path(path: &Path) -> Result<PathBuf, CliError> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(resolved)
}

fn read_debug_session(path: &Path) -> Result<DebugSessionFile, CliError> {
    let resolved = resolve_debug_session_path(path)?;
    let contents = fs::read_to_string(&resolved).map_err(|error| CliError::Breakpoint {
        code: codes::cli::UNEXPECTED.to_string(),
        message: format!("debug session '{}' could not be read", resolved.display()),
        details: json!({
            "session": resolved.to_string_lossy(),
            "error": error.to_string()
        }),
    })?;
    let session: DebugSessionFile =
        serde_json::from_str(&contents).map_err(|error| CliError::Breakpoint {
            code: codes::cli::UNEXPECTED.to_string(),
            message: format!("debug session '{}' is invalid", resolved.display()),
            details: json!({
                "session": resolved.to_string_lossy(),
                "error": error.to_string()
            }),
        })?;
    if session.kind != "sigilDebugSession" || session.format_version != 1 {
        return Err(CliError::Breakpoint {
            code: codes::cli::UNEXPECTED.to_string(),
            message: format!(
                "'{}' is not a supported Sigil debug session",
                resolved.display()
            ),
            details: json!({
                "session": resolved.to_string_lossy()
            }),
        });
    }
    let mut session = session;
    normalize_debug_snapshot(&mut session.snapshot);
    Ok(session)
}

fn write_debug_session(path: &Path, session: &DebugSessionFile) -> Result<(), CliError> {
    let serialized = serde_json::to_string(session).map_err(|error| {
        CliError::Runtime(format!(
            "{}: failed to serialize debug session '{}': {}",
            codes::cli::UNEXPECTED,
            path.display(),
            error
        ))
    })?;
    fs::write(path, serialized)?;
    Ok(())
}

fn debug_session_json(path: &Path, session: &DebugSessionFile) -> serde_json::Value {
    let mut session_json = serde_json::Map::new();
    session_json.insert("id".to_string(), json!(session.session_id));
    session_json.insert(
        "file".to_string(),
        json!(canonicalize_existing_path(path).to_string_lossy()),
    );
    session_json.insert(
        "targetKind".to_string(),
        json!(session.target_kind.as_str()),
    );
    session_json.insert("state".to_string(), json!(session.state));
    session_json.insert("replayFile".to_string(), json!(session.replay_file));
    match session.target_kind {
        DebugSessionTargetKind::Run => {
            session_json.insert("programPath".to_string(), json!(session.path));
        }
        DebugSessionTargetKind::Test => {
            session_json.insert("testPath".to_string(), json!(session.path));
        }
    }
    if let Some(test_id) = &session.test_id {
        session_json.insert("testId".to_string(), json!(test_id));
    }
    session_json.insert("watches".to_string(), json!(session.watches));
    serde_json::Value::Object(session_json)
}

fn output_debug_success(path: &Path, session: &DebugSessionFile) {
    let mut snapshot = session.snapshot.clone();
    normalize_debug_snapshot(&mut snapshot);
    let output = json!({
        "formatVersion": 1,
        "command": session.target_kind.command_name(),
        "ok": true,
        "phase": "runtime",
        "data": {
            "session": debug_session_json(path, session),
            "snapshot": snapshot
        }
    });
    output_json_value(&output, false);
}

fn normalize_debug_snapshot(snapshot: &mut serde_json::Value) {
    if let Some(snapshot_object) = snapshot.as_object_mut() {
        snapshot_object
            .entry("watches".to_string())
            .or_insert_with(|| json!([]));
    }
}

fn debug_session_state_from_snapshot(snapshot: &serde_json::Value) -> DebugSessionState {
    match snapshot["state"].as_str() {
        Some("completed") => DebugSessionState::Completed,
        Some("failed") => DebugSessionState::Failed,
        Some("closed") => DebugSessionState::Closed,
        _ => DebugSessionState::Paused,
    }
}

fn debug_runtime_breakpoint_config_json(
    config: Option<&ResolvedBreakpointConfig>,
) -> Result<String, CliError> {
    let spans = config
        .map(|config| {
            config
                .spans
                .iter()
                .map(|(span_id, selectors)| {
                    (
                        span_id.clone(),
                        serde_json::to_value(selectors).unwrap_or_else(|_| json!([])),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
        })
        .unwrap_or_default();
    serde_json::to_string(&json!({
        "enabled": true,
        "mode": "stop",
        "maxHits": 32,
        "recentTraceLimit": 32,
        "spans": spans
    }))
    .map_err(|error| {
        CliError::Codegen(format!("failed to encode debug breakpoint config: {error}"))
    })
}

fn is_lower_camel_case_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric())
}

fn validate_debug_watch_selectors(
    target_kind: DebugSessionTargetKind,
    selectors: &[String],
) -> Result<Vec<String>, CliError> {
    for selector in selectors {
        let segments = selector.split('.').collect::<Vec<_>>();
        let invalid = selector.is_empty()
            || segments.is_empty()
            || segments
                .iter()
                .any(|segment| segment.is_empty() || !is_lower_camel_case_identifier(segment));
        if invalid {
            output_json_error_to(
                target_kind.command_name(),
                "cli",
                codes::cli::USAGE,
                &format!("invalid watch selector '{}'", selector),
                json!({
                    "selector": selector,
                    "expected": "lowerCamelCase or lowerCamelCase.field.subfield"
                }),
                false,
            );
            return Err(CliError::Reported(1));
        }
    }
    Ok(selectors.to_vec())
}

fn debug_step_config_json(
    target_kind: DebugSessionTargetKind,
    action: &str,
    cursor: Option<&DebugStepCursor>,
    watches: &[String],
) -> Result<String, CliError> {
    serde_json::to_string(&json!({
        "targetKind": target_kind.as_str(),
        "action": action,
        "startEventKind": if target_kind == DebugSessionTargetKind::Run { "function_enter" } else { "test_enter" },
        "watches": watches,
        "current": cursor.map(|cursor| json!({
            "seq": cursor.seq,
            "eventKind": cursor.event_kind,
            "spanId": cursor.span_id,
            "frameDepth": cursor.frame_depth,
            "expressionDepth": cursor.expression_depth,
            "testId": cursor.test_id
        }))
    }))
    .map_err(|error| CliError::Codegen(format!("failed to encode debug step config: {error}")))
}

fn debug_step_runtime_source(step_file_json: &str, step_config_json: &str) -> String {
    format!(
        r#"
const __sigil_debug_step_file = {step_file_json};
globalThis.__sigil_debug_step_config = {step_config_json};
globalThis.__sigil_debug_step_current = undefined;

function __sigil_debug_pause_signal() {{
  return {{ __sigilDebugPause: true }};
}}

function __sigil_debug_is_pause_signal(error) {{
  return !!(error && typeof error === 'object' && error.__sigilDebugPause === true);
}}

function __sigil_debug_state() {{
  if (!globalThis.__sigil_debug_step_current || typeof globalThis.__sigil_debug_step_current !== 'object') {{
    globalThis.__sigil_debug_step_current = {{
      nextSeq: 1,
      snapshot: null,
      config: globalThis.__sigil_debug_step_config ?? {{}}
    }};
  }}
  return globalThis.__sigil_debug_step_current;
}}

function __sigil_debug_last_completed_from_event(event) {{
  if (!event || typeof event !== 'object') return null;
  if (event.kind === 'expr_return') {{
    return {{ kind: 'expr_return', spanId: event.spanId ?? null, value: event.value ?? null }};
  }}
  if (event.kind === 'expr_throw') {{
    return {{ kind: 'expr_throw', spanId: event.spanId ?? null, error: event.error ?? null }};
  }}
  if (event.kind === 'function_return') {{
    return {{ kind: 'function_return', functionName: event.functionName ?? null, value: event.value ?? null }};
  }}
  if (event.kind === 'test_return') {{
    return {{ kind: 'test_return', testId: event.testId ?? null, testStatus: event.testStatus ?? null, value: event.value ?? null }};
  }}
  return null;
}}

function __sigil_debug_watch_is_record_like(value) {{
  return !!value &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    !(typeof value.__tag === 'string') &&
    !Array.isArray(value.__fields) &&
    !Array.isArray(value.__sigil_map);
}}

function __sigil_debug_watch_results() {{
  const config = globalThis.__sigil_debug_step_config ?? {{}};
  const selectors = Array.isArray(config.watches) ? config.watches : [];
  if (selectors.length === 0) return [];
  const locals = typeof globalThis.__sigil_breakpoint_current_locals_raw === 'function'
    ? globalThis.__sigil_breakpoint_current_locals_raw()
    : [];
  return selectors.map((selector) => {{
    const segments = String(selector).split('.');
    const root = locals.find((local) => String(local?.name ?? '') === segments[0]);
    if (!root) {{
      return {{ selector: String(selector), status: 'not_in_scope' }};
    }}
    let current = root.raw;
    for (const segment of segments.slice(1)) {{
      if (!__sigil_debug_watch_is_record_like(current) ||
          !Object.prototype.hasOwnProperty.call(current, segment)) {{
        return {{ selector: String(selector), status: 'path_missing' }};
      }}
      current = current[segment];
    }}
    const typeId =
      segments.length === 1 && root?.typeId != null ? String(root.typeId) : null;
    return {{
      selector: String(selector),
      status: 'ok',
      value: typeof globalThis.__sigil_trace_summary_typed === 'function'
        ? globalThis.__sigil_trace_summary_typed(current, 1, typeId)
        : {{ kind: typeof current }}
    }};
  }});
}}

function __sigil_debug_snapshot_from_event(stateValue, reason, event, extras = {{}}) {{
  return {{
    state: stateValue,
    pauseReason: reason,
    eventKind: String(event?.kind ?? ''),
    seq: Number(event?.seq ?? 0),
    moduleId: event?.moduleId ?? null,
    sourceFile: event?.sourceFile ?? null,
    spanId: event?.spanId ?? null,
    spanKind: event?.spanKind ?? null,
    declarationKind: event?.declarationKind ?? null,
    declarationLabel: event?.declarationLabel ?? null,
    functionName: event?.functionName ?? null,
    testId: event?.testId ?? null,
    testName: event?.testName ?? null,
    testStatus: extras.testStatus ?? event?.testStatus ?? null,
    matched: Array.isArray(event?.matched) ? event.matched : [],
    locals: Array.isArray(event?.locals) ? event.locals : [],
    stack: Array.isArray(event?.stack) ? event.stack : [],
    recentTrace: Array.isArray(event?.recentTrace) ? event.recentTrace : [],
    frameDepth: Number(event?.frameDepth ?? 0),
    expressionDepth: Number(event?.expressionDepth ?? 0),
    lastCompleted: extras.lastCompleted ?? event?.lastCompleted ?? __sigil_debug_last_completed_from_event(event),
    exception: extras.exception ?? null,
    watches: __sigil_debug_watch_results()
  }};
}}

function __sigil_debug_pause_reason(event, config) {{
  const current = config?.current ?? null;
  const currentSeq = Number(current?.seq ?? 0);
  if (Number(event?.seq ?? 0) <= currentSeq) {{
    return null;
  }}
  if (event?.kind === 'breakpoint') {{
    return 'breakpoint';
  }}
  const action = String(config?.action ?? 'continue');
  if (action === 'start') {{
    return String(event?.kind ?? '') === String(config?.startEventKind ?? '') ? 'start' : null;
  }}
  if (action === 'continue') {{
    return null;
  }}
  if (action === 'stepInto') {{
    return 'step';
  }}
  if (action === 'stepOver') {{
    if (current?.eventKind === 'expr_enter') {{
      return (event?.kind === 'expr_return' || event?.kind === 'expr_throw') &&
        Number(event?.expressionDepth ?? 0) === Number(current?.expressionDepth ?? 0)
        ? 'step'
        : null;
    }}
    if (current?.eventKind === 'function_enter') {{
      return event?.kind === 'function_return' &&
        Number(event?.frameDepth ?? 0) === Number(current?.frameDepth ?? 0)
        ? 'step'
        : null;
    }}
    if (current?.eventKind === 'test_enter') {{
      return event?.kind === 'test_return' &&
        String(event?.testId ?? '') === String(current?.testId ?? '')
        ? 'step'
        : null;
    }}
    return 'step';
  }}
  if (action === 'stepOut') {{
    const targetKind = String(config?.targetKind ?? '');
    if (Number(current?.frameDepth ?? 0) > 0) {{
      return event?.kind === 'function_return' &&
        Number(event?.frameDepth ?? 0) === Number(current?.frameDepth ?? 0)
        ? 'step'
        : null;
    }}
    if (targetKind === 'test') {{
      return event?.kind === 'test_return' &&
        (current?.testId == null || String(event?.testId ?? '') === String(current?.testId ?? ''))
        ? 'step'
        : null;
    }}
    return null;
  }}
  return null;
}}

function __sigil_debug_step_event(rawEvent) {{
  if (!rawEvent || typeof rawEvent !== 'object') return;
  const state = __sigil_debug_state();
  const event = {{ seq: state.nextSeq, ...rawEvent }};
  state.nextSeq += 1;
  state.lastEvent = event;
  const reason = __sigil_debug_pause_reason(event, state.config);
  if (reason) {{
    state.snapshot = __sigil_debug_snapshot_from_event('paused', reason, event);
    throw __sigil_debug_pause_signal();
  }}
}}

function __sigil_debug_mark_completed(kind, extras = {{}}) {{
  const state = __sigil_debug_state();
  const event = {{ kind, seq: state.nextSeq, ...extras }};
  state.nextSeq += 1;
  state.snapshot = __sigil_debug_snapshot_from_event('completed', 'exit', event, extras);
  return state.snapshot;
}}

function __sigil_debug_mark_failed(exceptionPayload) {{
  const state = __sigil_debug_state();
  const expression = exceptionPayload?.expression ?? null;
  const event = expression
    ? {{
        kind: 'uncaught_exception',
        seq: state.nextSeq,
        moduleId: expression.moduleId ?? null,
        sourceFile: expression.sourceFile ?? null,
        spanId: expression.spanId ?? null,
        spanKind: expression.spanKind ?? null,
        declarationKind: expression.declarationKind ?? null,
        declarationLabel: expression.declarationLabel ?? null,
        locals: Array.isArray(expression.locals) ? expression.locals : [],
        stack: Array.isArray(expression.stack) ? expression.stack : [],
        recentTrace:
          typeof globalThis.__sigil_breakpoint_recent_trace === 'function'
            ? globalThis.__sigil_breakpoint_recent_trace()
            : [],
        frameDepth: Array.isArray(expression.stack) ? expression.stack.length : 0,
        expressionDepth: 0
      }}
    : {{ kind: 'uncaught_exception', seq: state.nextSeq }};
  state.nextSeq += 1;
  state.snapshot = __sigil_debug_snapshot_from_event('failed', 'exception', event, {{
    exception: exceptionPayload ?? null
  }});
  return state.snapshot;
}}

function __sigil_debug_snapshot() {{
  const state = __sigil_debug_state();
  return state.snapshot ?? {{
    state: 'paused',
    pauseReason: 'start',
    eventKind: '',
    seq: 0,
    moduleId: null,
    sourceFile: null,
    spanId: null,
    spanKind: null,
    declarationKind: null,
    declarationLabel: null,
    functionName: null,
    testId: null,
    testName: null,
    testStatus: null,
    matched: [],
    locals: [],
    stack: [],
    recentTrace: [],
    frameDepth: 0,
    expressionDepth: 0,
    lastCompleted: null,
    exception: null,
    watches: []
  }};
}}

function __sigil_debug_capture_step_sync() {{
  try {{
    writeFileSync(__sigil_debug_step_file, JSON.stringify(__sigil_debug_snapshot()));
  }} catch (_captureStepError) {{
    // Best-effort debug plumbing only.
  }}
}}

process.on('exit', () => {{
  __sigil_debug_capture_step_sync();
}});

globalThis.__sigil_debug_step_event = __sigil_debug_step_event;
globalThis.__sigil_debug_mark_completed = __sigil_debug_mark_completed;
globalThis.__sigil_debug_mark_failed = __sigil_debug_mark_failed;
globalThis.__sigil_debug_is_pause_signal = __sigil_debug_is_pause_signal;
globalThis.__sigil_debug_snapshot = __sigil_debug_snapshot;
"#
    )
}

fn debug_runtime_replay_capture_source(
    runtime_replay_path_json: &str,
    replay_config_json: &str,
) -> String {
    format!(
        r#"
const __sigil_runtime_replay_file = {runtime_replay_path_json};
globalThis.__sigil_replay_config = {replay_config_json};
globalThis.__sigil_replay_current = undefined;

function __sigil_runtime_replay_payload() {{
  if (typeof globalThis.__sigil_replay_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_replay_snapshot();
    }} catch (_replayError) {{}}
  }}
  return {{
    mode: String(globalThis.__sigil_replay_config?.mode ?? ''),
    file: String(globalThis.__sigil_replay_config?.file ?? ''),
    recordedEvents: 0,
    consumedEvents: 0,
    remainingEvents: 0,
    partial: false
  }};
}}

function __sigil_runtime_capture_replay_sync() {{
  try {{
    writeFileSync(__sigil_runtime_replay_file, JSON.stringify(__sigil_runtime_replay_payload()));
  }} catch (_captureReplayError) {{
    // Best-effort debug plumbing only.
  }}
}}

process.on('exit', () => {{
  __sigil_runtime_capture_replay_sync();
}});
"#
    )
}

fn debug_runtime_error_capture_source(runtime_error_path_json: &str) -> String {
    format!(
        r#"
const __sigil_runtime_error_file = {runtime_error_path_json};

function __sigil_runtime_exception_name(error) {{
  if (error instanceof Error && error.name) {{
    return String(error.name);
  }}
  if (error && typeof error === 'object' && 'name' in error && error.name != null) {{
    return String(error.name);
  }}
  return 'Error';
}}

function __sigil_runtime_exception_message(error) {{
  if (error instanceof Error) {{
    return String(error.message ?? '');
  }}
  return String(error);
}}

function __sigil_runtime_exception_stack(error) {{
  if (error instanceof Error && typeof error.stack === 'string') {{
    return error.stack;
  }}
  return '';
}}

function __sigil_runtime_expression_payload() {{
  if (typeof globalThis.__sigil_expression_exception_payload === 'function') {{
    try {{
      return globalThis.__sigil_expression_exception_payload();
    }} catch (_captureExpressionError) {{
      return null;
    }}
  }}
  return null;
}}

async function __sigil_runtime_capture_error(error) {{
  const sigilCode =
    error && typeof error === 'object' && 'sigilCode' in error && error.sigilCode != null
      ? String(error.sigilCode)
      : null;
  const payload = {{
    message: __sigil_runtime_exception_message(error),
    name: __sigil_runtime_exception_name(error),
    sigilCode,
    expression: __sigil_runtime_expression_payload(),
    stack: __sigil_runtime_exception_stack(error)
  }};
  try {{
    await writeFile(__sigil_runtime_error_file, JSON.stringify(payload));
  }} catch (_captureError) {{
    // Best-effort debug plumbing only.
  }}
  return payload;
}}

function __sigil_runtime_is_intentional_debug_stop(error) {{
  return (
    (typeof globalThis.__sigil_breakpoint_is_stop_signal === 'function' &&
      !!globalThis.__sigil_breakpoint_is_stop_signal(error)) ||
    (typeof globalThis.__sigil_debug_is_pause_signal === 'function' &&
      !!globalThis.__sigil_debug_is_pause_signal(error))
  );
}}
"#
    )
}

#[derive(Debug, Clone, Default)]
struct CoverageObservation {
    calls: HashSet<String>,
    variants: HashMap<String, HashSet<String>>,
}

fn collect_sigil_files(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();

    if dir.is_file() && dir.extension().and_then(|s| s.to_str()) == Some("sigil") {
        files.push(dir.to_path_buf());
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(collect_sigil_files(&path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("sigil") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn compile_and_run_tests(
    file: &Path,
    selected_env: Option<&str>,
    match_filter: Option<&str>,
    debug_options: &TestDebugOptions,
    suite_replay_mode: Option<&PreparedTestReplayMode>,
) -> Result<TestRunResult, CliError> {
    let graph = ModuleGraph::build_with_env(file, selected_env)?;
    let compiled = compile_entry_files_with_cache(
        &[file.to_path_buf()],
        Some(graph),
        None,
        selected_env,
        debug_options.trace_enabled,
        debug_options.breakpoints_requested(),
        true,
        OutputFlavor::RuntimeEsm,
    )?;
    let topology_prelude = if matches!(
        suite_replay_mode,
        Some(PreparedTestReplayMode::Replay { .. })
    ) {
        None
    } else {
        runner_prelude(file, selected_env, &compiled.entry_output_path)?
    };
    let module_debug_outputs = build_runtime_module_debug_outputs(&compiled)?;
    let breakpoint_config = resolve_breakpoint_config(
        file,
        &module_debug_outputs,
        &debug_options.breakpoint_lines,
        &debug_options.breakpoint_functions,
        &debug_options.breakpoint_spans,
        debug_options.breakpoint_mode,
        debug_options.breakpoint_max_hits,
    )?;
    run_test_module(
        &compiled.entry_output_path,
        &compiled.coverage_targets,
        match_filter,
        &file.to_string_lossy(),
        topology_prelude.as_deref(),
        &module_debug_outputs,
        breakpoint_config.as_ref(),
        debug_options,
        suite_replay_mode,
    )
}

fn run_test_module(
    ts_file: &Path,
    coverage_targets: &[CoverageTarget],
    match_filter: Option<&str>,
    _source_file: &str,
    topology_prelude: Option<&str>,
    module_debug_outputs: &[RuntimeModuleDebugOutput],
    breakpoint_config: Option<&ResolvedBreakpointConfig>,
    debug_options: &TestDebugOptions,
    suite_replay_mode: Option<&PreparedTestReplayMode>,
) -> Result<TestRunResult, CliError> {
    // Create test runner directory
    let test_dir = ts_file.parent().unwrap().join("__sigil_test");
    fs::create_dir_all(&test_dir)?;

    // Create unique runner file
    let unique = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let runner_file = test_dir.join(format!(
        "{}.{}.runner.mjs",
        ts_file.file_stem().unwrap().to_string_lossy(),
        unique
    ));

    // Canonicalize the TypeScript file path for import
    let abs_ts_file = fs::canonicalize(ts_file)?;
    let module_url = format!("file://{}", abs_ts_file.display());

    // Generate test runner code
    let match_text_json = match match_filter {
        Some(m) => format!("\"{}\"", m.replace('"', "\\\"")),
        None => "null".to_string(),
    };
    let coverage_targets_json = serde_json::to_string(
        &coverage_targets
            .iter()
            .map(|target| &target.id)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let trace_runtime_enabled =
        debug_options.trace_enabled || debug_options.breakpoints_requested();
    let trace_config_json = if trace_runtime_enabled {
        serde_json::to_string(&json!({
            "enabled": true,
            "maxEvents": 256,
            "expressions": debug_options.trace_expr_enabled
        }))
        .unwrap()
    } else {
        "null".to_string()
    };
    let breakpoint_config_json = breakpoint_config
        .map(resolved_breakpoint_config_json)
        .map(|value| serde_json::to_string(&value).unwrap())
        .unwrap_or_else(|| "null".to_string());
    let suite_replay_json = match suite_replay_mode {
        Some(PreparedTestReplayMode::Record {
            artifact_file,
            request,
            binding,
        }) => serde_json::to_string(&json!({
            "mode": "record",
            "file": artifact_file.to_string_lossy(),
            "request": request,
            "binding": binding
        }))
        .unwrap(),
        Some(PreparedTestReplayMode::Replay {
            artifact_file,
            artifact,
        }) => serde_json::to_string(&json!({
            "mode": "replay",
            "file": artifact_file.to_string_lossy(),
            "artifact": artifact
        }))
        .unwrap(),
        None => "null".to_string(),
    };

    let runner_code = format!(
        r#"{topology_prelude}
const moduleUrl = "{module_url}";
const discoverMod = globalThis.__sigil_program_exports ?? await import(moduleUrl);
const tests = Array.isArray(discoverMod.__sigil_tests) ? discoverMod.__sigil_tests : [];
const matchText = {match_text_json};
const selected = matchText ? tests.filter((t) => String(t.name).includes(matchText)) : tests;
const selectedIds = selected.map((t) => String(t.id));
const results = [];
const recordedTests = [];
const startSuite = Date.now();
const __sigil_trace_config_template = {trace_config_json};
const __sigil_breakpoint_config_template = {breakpoint_config_json};
const __sigil_suite_replay = {suite_replay_json};

function __sigil_json_clone(value) {{
  if (value == null) return value;
  return JSON.parse(JSON.stringify(value));
}}

function __sigil_test_exception_name(error) {{
  if (error instanceof Error && error.name) {{
    return String(error.name);
  }}
  if (error && typeof error === 'object' && 'name' in error && error.name != null) {{
    return String(error.name);
  }}
  return 'Error';
}}

function __sigil_test_exception_message(error) {{
  if (error instanceof Error) {{
    return String(error.message ?? '');
  }}
  return String(error);
}}

function __sigil_test_exception_stack(error) {{
  if (error instanceof Error && typeof error.stack === 'string') {{
    return error.stack;
  }}
  return '';
}}

function __sigil_test_exception_payload(error) {{
  return {{
    name: __sigil_test_exception_name(error),
    message: __sigil_test_exception_message(error),
    stack: __sigil_test_exception_stack(error),
    sigilCode:
      error && typeof error === 'object' && 'sigilCode' in error && error.sigilCode != null
        ? String(error.sigilCode)
        : null,
    expression:
      typeof globalThis.__sigil_expression_exception_payload === 'function'
        ? globalThis.__sigil_expression_exception_payload()
        : null
  }};
}}

function __sigil_test_trace_payload() {{
  if (typeof globalThis.__sigil_trace_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_trace_snapshot();
    }} catch (_traceError) {{}}
  }}
  return {{ enabled: true, truncated: false, totalEvents: 0, returnedEvents: 0, droppedEvents: 0, events: [] }};
}}

function __sigil_test_breakpoint_payload() {{
  if (typeof globalThis.__sigil_breakpoint_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_breakpoint_snapshot();
    }} catch (_breakpointError) {{}}
  }}
  return {{
    enabled: true,
    mode: String(globalThis.__sigil_breakpoint_config?.mode ?? 'stop'),
    stopped: false,
    truncated: false,
    totalHits: 0,
    returnedHits: 0,
    droppedHits: 0,
    maxHits: Math.max(1, Number(globalThis.__sigil_breakpoint_config?.maxHits ?? 32)),
    hits: []
  }};
}}

function __sigil_test_replay_payload() {{
  if (typeof globalThis.__sigil_replay_snapshot === 'function') {{
    try {{
      return globalThis.__sigil_replay_snapshot();
    }} catch (_replayError) {{}}
  }}
  return {{
    mode: String(globalThis.__sigil_replay_config?.mode ?? ''),
    file: String(globalThis.__sigil_replay_config?.file ?? ''),
    recordedEvents: 0,
    consumedEvents: 0,
    remainingEvents: 0,
    partial: false
  }};
}}

function __sigil_test_is_breakpoint_stop(error) {{
  return typeof globalThis.__sigil_breakpoint_is_stop_signal === 'function'
    ? !!globalThis.__sigil_breakpoint_is_stop_signal(error)
    : false;
}}

function __sigil_test_reset_runtime_globals() {{
  globalThis.__sigil_coverage_current = {{ calls: Object.create(null), variants: Object.create(null) }};
  globalThis.__sigil_trace_config = __sigil_trace_config_template ? __sigil_json_clone(__sigil_trace_config_template) : undefined;
  globalThis.__sigil_trace_current = undefined;
  globalThis.__sigil_breakpoint_config = __sigil_breakpoint_config_template ? __sigil_json_clone(__sigil_breakpoint_config_template) : undefined;
  globalThis.__sigil_breakpoint_current = undefined;
  globalThis.__sigil_expression_current = undefined;
  globalThis.__sigil_world_current = undefined;
  globalThis.__sigil_world_template_cache = undefined;
  globalThis.__sigil_last_test_world = undefined;
  globalThis.__sigil_replay_current = undefined;
  globalThis.__sigil_replay_config = null;
}}

function __sigil_test_record_config(testMeta) {{
  return {{
    mode: 'record',
    file: String(__sigil_suite_replay?.file ?? ''),
    entry: {{
      sourceFile: String(String(testMeta?.id ?? '').split('::')[0] ?? ''),
      argv: [],
      projectRoot: __sigil_suite_replay?.request?.projectRoot ?? null
    }},
    binding: __sigil_json_clone(__sigil_suite_replay?.binding ?? {{ algorithm: 'sha256', fingerprint: '', modules: [] }})
  }};
}}

function __sigil_test_replay_entry(testId) {{
  const tests = Array.isArray(__sigil_suite_replay?.artifact?.tests) ? __sigil_suite_replay.artifact.tests : [];
  return tests.find((entry) => String(entry.id) === String(testId)) ?? null;
}}

function __sigil_test_replay_config_for(testMeta) {{
  if (!__sigil_suite_replay || !__sigil_suite_replay.mode) {{
    return null;
  }}
  if (__sigil_suite_replay.mode === 'record') {{
    return __sigil_test_record_config(testMeta);
  }}
  const entry = __sigil_test_replay_entry(testMeta?.id);
  if (!entry || !entry.replayArtifact) {{
    const error = new Error(`replay artifact does not contain test '${{String(testMeta?.id ?? '')}}'`);
    error.sigilCode = 'SIGIL-RUNTIME-REPLAY-BINDING-MISMATCH';
    throw error;
  }}
  return {{
    mode: 'replay',
    file: String(__sigil_suite_replay.file ?? ''),
    artifact: __sigil_json_clone(entry.replayArtifact)
  }};
}}

if (__sigil_suite_replay?.mode === 'replay') {{
  const recordedIds = Array.isArray(__sigil_suite_replay?.artifact?.selectedTestIds)
    ? __sigil_suite_replay.artifact.selectedTestIds.map((id) => String(id))
    : [];
  for (const id of selectedIds) {{
    if (!recordedIds.includes(id)) {{
      console.log(JSON.stringify({{
        discovered: tests.length,
        selected: selected.length,
        selectedIds,
        coverageTargets: {coverage_targets_json},
        results: [],
        recordedTests: [],
        runnerError: {{
          code: 'SIGIL-RUNTIME-REPLAY-BINDING-MISMATCH',
          message: `replay artifact does not contain selected test '${{id}}'`,
          details: {{ testId: id }}
        }}
      }}));
      process.exit(0);
    }}
  }}
}}

for (const t of selected) {{
  const start = Date.now();
  __sigil_test_reset_runtime_globals();
  try {{
    globalThis.__sigil_replay_config = __sigil_test_replay_config_for(t);
    const freshMod = await import(moduleUrl + '?sigil_test=' + encodeURIComponent(String(t.id)) + '&ts=' + Date.now() + '_' + Math.random());
    if (typeof globalThis.__sigil_runtime_apply_program_world === 'function') {{
      await globalThis.__sigil_runtime_apply_program_world(freshMod);
    }}
    const freshTests = Array.isArray(freshMod.__sigil_tests) ? freshMod.__sigil_tests : [];
    const freshTest = freshTests.find((x) => x.id === t.id);
    if (!freshTest) {{ throw new Error('Test not found in isolated module reload: ' + String(t.id)); }}
    const value = await freshTest.fn();
    const coverageState = globalThis.__sigil_coverage_current ?? {{ calls: {{}}, variants: {{}} }};
    const coverage = {{
      calls: Object.entries(coverageState.calls ?? {{}})
        .filter(([, count]) => Number(count ?? 0) > 0)
        .map(([key]) => key),
      variants: Object.fromEntries(
        Object.entries(coverageState.variants ?? {{}}).map(([key, tags]) => [key, Array.isArray(tags) ? tags : []])
      )
    }};
    delete globalThis.__sigil_coverage_current;
    const replay = __sigil_suite_replay ? __sigil_test_replay_payload() : undefined;
    if (__sigil_suite_replay?.mode === 'record') {{
      const replayArtifact =
        typeof globalThis.__sigil_replay_artifact === 'function'
          ? globalThis.__sigil_replay_artifact()
          : null;
      if (replayArtifact && globalThis.__sigil_last_test_world != null) {{
        replayArtifact.world = replayArtifact.world ?? {{}};
        replayArtifact.world.normalizedWorld = __sigil_json_clone(globalThis.__sigil_last_test_world);
      }}
      recordedTests.push({{
        id: t.id,
        file: String(t.id).split('::')[0],
        name: t.name,
        status: value === true || (value && typeof value === 'object' && 'ok' in value && value.ok === true) ? 'pass' : 'fail',
        location: {{ line: Number(t.location?.start?.line ?? 1), column: Number(t.location?.start?.column ?? 1) }},
        failure:
          value === true || (value && typeof value === 'object' && 'ok' in value && value.ok === true)
            ? null
            : String(value?.failure?.message ?? value?.failure ?? 'Test body evaluated to false'),
        replayArtifact
      }});
    }}
    if (value === true) {{
      results.push({{
        coverage,
        id: t.id,
        file: String(t.id).split('::')[0],
        name: t.name,
        status: 'pass',
        durationMs: Date.now()-start,
        location: {{ line: Number(t.location?.start?.line ?? 1), column: Number(t.location?.start?.column ?? 1) }},
        trace: {trace_enabled_result},
        breakpoints: {breakpoints_enabled_result},
        replay
      }});
    }} else if (value && typeof value === 'object' && 'ok' in value) {{
      if (value.ok === true) {{
        results.push({{
          coverage,
          id: t.id,
          file: String(t.id).split('::')[0],
          name: t.name,
          status: 'pass',
          durationMs: Date.now()-start,
          location: {{ line: Number(t.location?.start?.line ?? 1), column: Number(t.location?.start?.column ?? 1) }},
          trace: {trace_enabled_result},
          breakpoints: {breakpoints_enabled_result},
          replay
        }});
      }} else {{
        results.push({{
          coverage,
          id: t.id,
          file: String(t.id).split('::')[0],
          name: t.name,
          status: 'fail',
          durationMs: Date.now()-start,
          location: {{ line: Number(t.location?.start?.line ?? 1), column: Number(t.location?.start?.column ?? 1) }},
          failure: String(value.failure?.message ?? value.failure ?? 'Test body evaluated to false'),
          trace: {trace_enabled_result},
          breakpoints: {breakpoints_enabled_result},
          replay
        }});
      }}
    }} else {{
      results.push({{
        coverage,
        id: t.id,
        file: String(t.id).split('::')[0],
        name: t.name,
        status: 'fail',
        durationMs: Date.now()-start,
        location: {{ line: Number(t.location?.start?.line ?? 1), column: Number(t.location?.start?.column ?? 1) }},
        failure: 'Test body evaluated to false',
        trace: {trace_enabled_result},
        breakpoints: {breakpoints_enabled_result},
        replay
      }});
    }}
  }} catch (e) {{
    const coverageState = globalThis.__sigil_coverage_current ?? {{ calls: {{}}, variants: {{}} }};
    const coverage = {{
      calls: Object.entries(coverageState.calls ?? {{}})
        .filter(([, count]) => Number(count ?? 0) > 0)
        .map(([key]) => key),
      variants: Object.fromEntries(
        Object.entries(coverageState.variants ?? {{}}).map(([key, tags]) => [key, Array.isArray(tags) ? tags : []])
      )
    }};
    delete globalThis.__sigil_coverage_current;
    const replay = __sigil_suite_replay ? __sigil_test_replay_payload() : undefined;
    const location = {{ line: Number(t.location?.start?.line ?? 1), column: Number(t.location?.start?.column ?? 1) }};
    if (__sigil_test_is_breakpoint_stop(e)) {{
      if (__sigil_suite_replay?.mode === 'record') {{
        const replayArtifact =
          typeof globalThis.__sigil_replay_artifact === 'function'
            ? globalThis.__sigil_replay_artifact()
            : null;
        if (replayArtifact && globalThis.__sigil_last_test_world != null) {{
          replayArtifact.world = replayArtifact.world ?? {{}};
          replayArtifact.world.normalizedWorld = __sigil_json_clone(globalThis.__sigil_last_test_world);
        }}
        recordedTests.push({{
          id: t.id,
          file: String(t.id).split('::')[0],
          name: t.name,
          status: 'stopped',
          location,
          failure: null,
          replayArtifact
        }});
      }}
      results.push({{
        coverage,
        id: t.id,
        file: String(t.id).split('::')[0],
        name: t.name,
        status: 'stopped',
        durationMs: Date.now()-start,
        location,
        trace: {trace_enabled_result},
        breakpoints: {breakpoints_enabled_result},
        replay
      }});
    }} else {{
      const exception = __sigil_test_exception_payload(e);
      if (__sigil_suite_replay?.mode === 'record') {{
        const replayArtifact =
          typeof globalThis.__sigil_replay_artifact === 'function'
            ? globalThis.__sigil_replay_artifact()
            : null;
        if (replayArtifact && globalThis.__sigil_last_test_world != null) {{
          replayArtifact.world = replayArtifact.world ?? {{}};
          replayArtifact.world.normalizedWorld = __sigil_json_clone(globalThis.__sigil_last_test_world);
        }}
        recordedTests.push({{
          id: t.id,
          file: String(t.id).split('::')[0],
          name: t.name,
          status: 'error',
          location,
          failure: exception.message,
          replayArtifact
        }});
      }}
      results.push({{
        coverage,
        id: t.id,
        file: String(t.id).split('::')[0],
        name: t.name,
        status: 'error',
        durationMs: Date.now()-start,
        location,
        failure: exception.message,
        trace: {trace_enabled_result},
        breakpoints: {breakpoints_enabled_result},
        replay,
        exception
      }});
    }}
  }}
}}
console.log(JSON.stringify({{
  coverageTargets: {coverage_targets_json},
  results,
  discovered: tests.length,
  selected: selected.length,
  selectedIds,
  recordedTests,
  durationMs: Date.now()-startSuite
}}));
"#,
        topology_prelude = topology_prelude.unwrap_or(""),
        coverage_targets_json = coverage_targets_json,
        module_url = module_url,
        match_text_json = match_text_json,
        trace_config_json = trace_config_json,
        breakpoint_config_json = breakpoint_config_json,
        suite_replay_json = suite_replay_json,
        trace_enabled_result = if debug_options.trace_enabled {
            "__sigil_test_trace_payload()"
        } else {
            "undefined"
        },
        breakpoints_enabled_result = if breakpoint_config.is_some() {
            "__sigil_test_breakpoint_payload()"
        } else {
            "undefined"
        }
    );

    fs::write(&runner_file, runner_code)?;

    // Execute runner
    let abs_runner = fs::canonicalize(&runner_file)?;
    let output = Command::new("node")
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime("node not found".to_string())
            } else {
                CliError::Runtime(format!("Failed to execute test runner: {}", e))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Runtime(format!("Test runner failed: {}", stderr)));
    }

    // Parse test results
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw: RawTestRunOutput = serde_json::from_str(stdout.trim())
        .map_err(|e| CliError::Runtime(format!("Failed to parse test output: {}", e)))?;

    if let Some(runner_error) = raw.runner_error {
        return Err(CliError::Breakpoint {
            code: runner_error.code,
            message: runner_error.message,
            details: runner_error.details,
        });
    }

    let discovered = raw.discovered;
    let selected = raw.selected;

    let mut coverage_observation = CoverageObservation::default();
    let mut runner_coverage_targets = coverage_targets.to_vec();
    if !raw.coverage_targets.is_empty() {
        let selected_target_ids = raw
            .coverage_targets
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        runner_coverage_targets.retain(|target| selected_target_ids.contains(target.id.as_str()));
    }

    let mut results = Vec::new();
    for result in raw.results {
        for key in result.coverage.calls {
            coverage_observation.calls.insert(key);
        }
        for (key, tags) in result.coverage.variants {
            let observed = coverage_observation.variants.entry(key).or_default();
            for tag in tags {
                observed.insert(tag);
            }
        }

        let exception = result.exception.as_ref().map(|capture| {
            let resolved_code = resolved_runtime_exception_code(capture);
            let code = resolved_code.as_str();
            let normalized_message = normalize_runtime_exception_message(capture, code);
            let analysis = analyze_runtime_exception(capture, module_debug_outputs);
            runtime_exception_json(
                capture,
                &normalized_message,
                &analysis,
                module_debug_outputs,
            )
        });
        let trace = debug_options
            .trace_enabled
            .then(|| runtime_trace_json(result.trace.as_ref()));
        let breakpoints = breakpoint_config.map(|config| {
            runtime_breakpoints_json(
                Some(config),
                result.breakpoints.as_ref(),
                module_debug_outputs,
            )
        });
        let replay = suite_replay_mode.map(|mode| {
            runtime_replay_json(
                Some(match mode {
                    PreparedTestReplayMode::Record { .. } => "record",
                    PreparedTestReplayMode::Replay { .. } => "replay",
                }),
                Some(match mode {
                    PreparedTestReplayMode::Record { artifact_file, .. } => artifact_file.as_path(),
                    PreparedTestReplayMode::Replay { artifact_file, .. } => artifact_file.as_path(),
                }),
                result.replay.as_ref(),
            )
        });

        results.push(TestResult {
            id: result.id,
            file: result.file,
            name: result.name,
            status: result.status,
            duration_ms: result.duration_ms,
            location: result.location,
            failure: result.failure,
            trace,
            breakpoints,
            replay,
            exception,
        });
    }

    Ok(TestRunResult {
        discovered,
        selected,
        selected_ids: raw.selected_ids,
        results,
        coverage_observation,
        coverage_targets: runner_coverage_targets,
        recorded_tests: raw.recorded_tests,
    })
}
