//! Command implementations for CLI

use crate::module_graph::{
    entry_module_key, load_project_effect_catalog_for, LoadedModule, ModuleGraph, ModuleGraphError,
};
use crate::project::{get_project_config, ProjectConfig, ProjectConfigError};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use rayon::{prelude::*, ThreadPoolBuilder};
use serde_json::json;
use sha2::{Digest, Sha256};
use sigil_ast::{Declaration, Program, SourceLocation, Type, TypeDef};
use sigil_codegen::{
    collect_module_span_map, world_runtime_helpers_source, CodegenOptions, DebugSpanKind,
    DebugSpanRecord, ModuleSpanMap, TypeScriptGenerator,
};
use sigil_diagnostics::codes;
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::types::{
    InferenceType, TConstructor, TFunction, TList, TMap, TRecord, TTuple,
};
use sigil_typechecker::{
    type_check, TypeCheckOptions, TypeError, TypeInfo, TypeScheme, TypedDeclaration, TypedProgram,
};
use sigil_validator::{
    print_canonical_program_with_effects, validate_canonical_form_with_options,
    validate_typed_canonical_form, ValidationError, ValidationOptions,
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

/// Extract error code from error message (format: "SIGIL-CANON-XXX: message")
fn extract_error_code(message: &str) -> String {
    if let Some(index) = message.find("SIGIL-") {
        let suffix = &message[index..];
        if let Some(colon_pos) = suffix.find(':') {
            return suffix[..colon_pos].to_string();
        }
        return suffix
            .split_whitespace()
            .next()
            .unwrap_or("SIGIL-ERROR")
            .to_string();
    }
    if let Some(colon_pos) = message.find(':') {
        message[..colon_pos].to_string()
    } else {
        "SIGIL-ERROR".to_string()
    }
}

fn format_validation_errors(errors: &[ValidationError]) -> String {
    if errors.is_empty() {
        "validation errors".to_string()
    } else {
        errors
            .iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Output a JSON error message matching the Sigil CLI format
fn output_json_error(
    command: &str,
    phase: &str,
    error_code: &str,
    message: &str,
    details: serde_json::Value,
) {
    output_json_error_to(command, phase, error_code, message, details, false);
}

fn output_json_error_to(
    command: &str,
    phase: &str,
    error_code: &str,
    message: &str,
    details: serde_json::Value,
    to_stderr: bool,
) {
    let output = json!({
        "formatVersion": 1,
        "command": command,
        "ok": false,
        "phase": phase,
        "error": {
            "code": error_code,
            "phase": phase,
            "message": message,
            "details": details
        }
    });
    output_json_value(&output, to_stderr);
}

fn output_json_value(output: &serde_json::Value, to_stderr: bool) {
    let serialized = serde_json::to_string(output).unwrap();
    if to_stderr {
        eprintln!("{}", serialized);
    } else {
        println!("{}", serialized);
    }
}

fn type_error_json_details(error: &TypeError) -> serde_json::Value {
    let mut details = serde_json::Map::new();

    if let Some(source_file) = &error.source_file {
        details.insert("file".to_string(), json!(source_file));
    }

    if let Some(location) = error.location {
        details.insert(
            "location".to_string(),
            json!({
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
            }),
        );
    }

    if let Some(expected) = &error.expected {
        details.insert(
            "expected".to_string(),
            json!(sigil_typechecker::format_type(expected)),
        );
    }

    if let Some(actual) = &error.actual {
        details.insert(
            "found".to_string(),
            json!(sigil_typechecker::format_type(actual)),
        );
    }

    if let Some(extra) = &error.details {
        for (key, value) in extra {
            details.insert(key.clone(), value.clone());
        }
    }

    serde_json::Value::Object(details)
}

fn merge_json_details(
    base: serde_json::Value,
    extra: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut merged = match base {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    merged.extend(extra);
    serde_json::Value::Object(merged)
}

fn output_inspect_error(
    command: &str,
    file: &Path,
    error: &CliError,
    extra_details: serde_json::Map<String, serde_json::Value>,
) {
    match error {
        CliError::Type(type_error) => output_json_error(
            command,
            "typecheck",
            &type_error.code,
            &type_error.message,
            merge_json_details(type_error_json_details(type_error), extra_details),
        ),
        CliError::ModuleGraph(ModuleGraphError::Validation(errors)) => {
            let message = errors
                .first()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "validation errors".to_string());
            let error_code = extract_error_code(&message);
            output_json_error(
                command,
                "canonical",
                &error_code,
                &message,
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy(),
                        "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::ModuleGraph(ModuleGraphError::ImportNotFound {
            module_id,
            expected_path,
        }) => output_json_error(
            command,
            "cli",
            codes::cli::IMPORT_NOT_FOUND,
            &format!("module not found: {}", module_id),
            merge_json_details(
                json!({
                    "file": file.to_string_lossy(),
                    "moduleId": module_id,
                    "expectedPath": expected_path
                }),
                extra_details,
            ),
        ),
        CliError::ModuleGraph(ModuleGraphError::ImportCycle(cycle)) => output_json_error(
            command,
            "cli",
            codes::cli::IMPORT_CYCLE,
            "module import cycle detected",
            merge_json_details(
                json!({
                    "file": file.to_string_lossy(),
                    "cycle": cycle
                }),
                extra_details,
            ),
        ),
        CliError::ModuleGraph(ModuleGraphError::Lexer(message))
        | CliError::Lexer(message)
        | CliError::ModuleGraph(ModuleGraphError::Parser(message))
        | CliError::Parser(message)
        | CliError::Validation(message)
        | CliError::Runtime(message) => {
            let error_code = extract_error_code(message);
            let phase = if error_code.starts_with("SIGIL-") {
                phase_for_code(&error_code)
            } else {
                match error {
                    CliError::ModuleGraph(ModuleGraphError::Lexer(_)) | CliError::Lexer(_) => {
                        "lexer"
                    }
                    CliError::ModuleGraph(ModuleGraphError::Parser(_)) | CliError::Parser(_) => {
                        "parser"
                    }
                    CliError::Validation(_) => "canonical",
                    CliError::Runtime(_) => "runtime",
                    _ => "cli",
                }
            };
            output_json_error(
                command,
                phase,
                if error_code.starts_with("SIGIL-") {
                    &error_code
                } else {
                    codes::cli::UNEXPECTED
                },
                message,
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
        | CliError::ProjectConfig(project_error) => output_json_error(
            command,
            "cli",
            codes::cli::UNEXPECTED,
            &project_error.to_string(),
            merge_json_details(
                json!({
                    "file": file.to_string_lossy()
                }),
                extra_details,
            ),
        ),
        CliError::Io(error) | CliError::ModuleGraph(ModuleGraphError::Io(error)) => {
            output_json_error(
                command,
                "io",
                codes::cli::UNEXPECTED,
                &error.to_string(),
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::Codegen(message) => output_json_error(
            command,
            "codegen",
            codes::cli::UNEXPECTED,
            message,
            merge_json_details(
                json!({
                    "file": file.to_string_lossy()
                }),
                extra_details,
            ),
        ),
        CliError::Breakpoint {
            code,
            message,
            details,
        } => output_json_error(
            command,
            phase_for_code(code),
            code,
            message,
            merge_json_details(details.clone(), extra_details),
        ),
        CliError::Reported(_) => {}
    }
}

struct CompileDirectoryIgnore {
    root: PathBuf,
    explicit_paths: Vec<PathBuf>,
    gitignore: Option<Gitignore>,
}

impl CompileDirectoryIgnore {
    fn new(
        root: &Path,
        ignore_paths: &[PathBuf],
        ignore_from: Option<&Path>,
    ) -> Result<Self, CliError> {
        let root = fs::canonicalize(root)?;
        let explicit_paths = ignore_paths
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_path_buf())
                } else {
                    Ok(root.join(path))
                }
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        let gitignore = if let Some(ignore_from) = ignore_from {
            let resolved_ignore_from = if ignore_from.is_absolute() {
                ignore_from.to_path_buf()
            } else {
                std::env::current_dir()?.join(ignore_from)
            };
            let mut builder = GitignoreBuilder::new(&root);
            if let Some(error) = builder.add(&resolved_ignore_from) {
                return Err(CliError::Validation(format!(
                    "failed to load ignore rules from '{}': {}",
                    resolved_ignore_from.display(),
                    error
                )));
            }
            Some(builder.build().map_err(|error| {
                CliError::Validation(format!(
                    "failed to parse ignore rules from '{}': {}",
                    resolved_ignore_from.display(),
                    error
                ))
            })?)
        } else {
            None
        };

        Ok(Self {
            root,
            explicit_paths,
            gitignore,
        })
    }

    fn should_ignore(&self, path: &Path, is_dir: bool) -> bool {
        if path
            .components()
            .any(|component| component.as_os_str() == ".local")
        {
            return true;
        }

        if self
            .explicit_paths
            .iter()
            .any(|ignore| path.starts_with(ignore))
        {
            return true;
        }

        self.gitignore.as_ref().is_some_and(|gitignore| {
            gitignore
                .matched_path_or_any_parents(path, is_dir)
                .is_ignore()
        })
    }
}

#[derive(Clone)]
struct CompileBatchGroup {
    first_index: usize,
    files: Vec<PathBuf>,
}

struct CompileEntryOutput {
    input: PathBuf,
    output: PathBuf,
    span_map: PathBuf,
    project_root: Option<PathBuf>,
}

struct CompileBatchOutputs {
    compiled_modules: usize,
    entries: Vec<CompileEntryOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectMode {
    Types,
    Validate,
    Codegen,
    World,
}

impl InspectMode {
    fn command_name(self) -> &'static str {
        match self {
            InspectMode::Types => "sigilc inspect types",
            InspectMode::Validate => "sigilc inspect validate",
            InspectMode::Codegen => "sigilc inspect codegen",
            InspectMode::World => "sigilc inspect world",
        }
    }

    fn phase(self) -> &'static str {
        match self {
            InspectMode::Types => "typecheck",
            InspectMode::Validate => "canonical",
            InspectMode::Codegen => "codegen",
            InspectMode::World => "topology",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            InspectMode::Types => "inspect types",
            InspectMode::Validate => "inspect validate",
            InspectMode::Codegen => "inspect codegen",
            InspectMode::World => "inspect world",
        }
    }
}

#[derive(Clone)]
struct AnalyzedModule {
    module_id: String,
    file_path: PathBuf,
    project: Option<ProjectConfig>,
    typed_program: TypedProgram,
    declaration_types: HashMap<String, InferenceType>,
    declaration_schemes: HashMap<String, TypeScheme>,
    declaration_span_ids: Vec<Option<String>>,
}

struct AnalyzedGraphOutputs {
    compiled_modules: usize,
    modules: HashMap<String, AnalyzedModule>,
    coverage_targets: Vec<CoverageTarget>,
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
    let mut tests = 0usize;
    let mut externs = 0usize;

    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(_) => functions += 1,
            Declaration::Type(_) => types += 1,
            Declaration::Effect(_) => effects += 1,
            Declaration::Const(_) => consts += 1,
            Declaration::Test(_) => tests += 1,
            Declaration::Extern(_) => externs += 1,
        }
    }

    serde_json::json!({
        "declarations": program.declarations.len(),
        "functions": functions,
        "types": types,
        "effects": effects,
        "consts": consts,
        "tests": tests,
        "externs": externs
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

fn is_sigil_source_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("sigil")
}

fn walk_compile_directory(
    dir: &Path,
    ignore: &CompileDirectoryIgnore,
    files: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let mut entries = fs::read_dir(dir)?
        .collect::<Result<Vec<_>, std::io::Error>>()?
        .into_iter()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        let is_dir = path.is_dir();
        if ignore.should_ignore(&path, is_dir) {
            continue;
        }

        if is_dir {
            walk_compile_directory(&path, ignore, files)?;
        } else if is_sigil_source_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn collect_sigil_targets(
    command_name: &str,
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<Vec<PathBuf>, CliError> {
    if is_sigil_source_file(path) {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_file() {
        return Err(CliError::Validation(format!(
            "{} expects a .sigil file or directory, got '{}'",
            command_name,
            path.display()
        )));
    }

    let ignore = CompileDirectoryIgnore::new(path, ignore_paths, ignore_from)?;
    let mut files = Vec::new();
    walk_compile_directory(&ignore.root, &ignore, &mut files)?;
    files.sort();
    Ok(files)
}

fn group_compile_targets(files: &[PathBuf]) -> Result<Vec<CompileBatchGroup>, CliError> {
    let mut project_buckets: HashMap<PathBuf, Vec<(usize, PathBuf, String)>> = HashMap::new();
    let mut standalone_bucket: Vec<(usize, PathBuf, String)> = Vec::new();

    for (index, file) in files.iter().enumerate() {
        let module_key = entry_module_key(file)?;
        if let Some(project) = get_project_config(file)? {
            project_buckets
                .entry(project.root.clone())
                .or_default()
                .push((index, file.clone(), module_key));
        } else {
            standalone_bucket.push((index, file.clone(), module_key));
        }
    }

    let mut groups = Vec::new();

    let mut project_roots = project_buckets.keys().cloned().collect::<Vec<_>>();
    project_roots.sort();
    for root in project_roots {
        let mut bucket = project_buckets.remove(&root).unwrap_or_default();
        bucket.sort_by(|a, b| a.1.cmp(&b.1));
        let mut packed_groups: Vec<(CompileBatchGroup, HashSet<String>)> = Vec::new();
        for (index, file, module_key) in bucket {
            if let Some((group, seen_keys)) = packed_groups
                .iter_mut()
                .find(|(_, seen_keys)| !seen_keys.contains(&module_key))
            {
                group.files.push(file);
                seen_keys.insert(module_key);
            } else {
                let mut seen_keys = HashSet::new();
                seen_keys.insert(module_key);
                packed_groups.push((
                    CompileBatchGroup {
                        first_index: index,
                        files: vec![file],
                    },
                    seen_keys,
                ));
            }
        }
        groups.extend(packed_groups.into_iter().map(|(group, _)| group));
    }

    if !standalone_bucket.is_empty() {
        standalone_bucket.sort_by(|a, b| a.1.cmp(&b.1));
        groups.push(CompileBatchGroup {
            first_index: standalone_bucket
                .iter()
                .map(|(index, _, _)| *index)
                .min()
                .unwrap_or(0),
            files: standalone_bucket
                .into_iter()
                .map(|(_, file, _)| file)
                .collect(),
        });
    }

    groups.sort_by_key(|group| group.first_index);
    for group in &mut groups {
        group.files.sort();
    }
    Ok(groups)
}

fn inspect_type_declarations(module: &AnalyzedModule) -> Vec<serde_json::Value> {
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

fn inspect_type_summary(declarations: &[serde_json::Value]) -> serde_json::Value {
    let mut functions = 0usize;
    let mut consts = 0usize;
    let mut tests = 0usize;

    for declaration in declarations {
        match declaration["kind"].as_str() {
            Some("function") => functions += 1,
            Some("const") => consts += 1,
            Some("test") => tests += 1,
            _ => {}
        }
    }

    serde_json::json!({
        "declarations": declarations.len(),
        "functions": functions,
        "consts": consts,
        "tests": tests
    })
}

fn inspect_types_file_result(input: &Path, module: &AnalyzedModule) -> serde_json::Value {
    let declarations = inspect_type_declarations(module);
    serde_json::json!({
        "input": input.to_string_lossy(),
        "moduleId": module.module_id,
        "sourceFile": module.file_path.to_string_lossy(),
        "project": project_json(module.project.as_ref()),
        "summary": inspect_type_summary(&declarations),
        "declarations": declarations
    })
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
        InspectMode::Types => inspect_types_command(path, ignore_paths, ignore_from),
        InspectMode::Validate => inspect_validate_command(path, ignore_paths, ignore_from),
        InspectMode::Codegen => inspect_codegen_command(path, ignore_paths, ignore_from),
        InspectMode::World => inspect_world_command(
            path,
            selected_env.expect("inspect world requires an environment"),
        ),
    }
}

fn inspect_codegen_command(
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        inspect_codegen_directory_command(path, ignore_paths, ignore_from)
    } else {
        inspect_codegen_single_file_command(path)
    }
}

fn inspect_codegen_single_file_command(file: &Path) -> Result<(), CliError> {
    let graph = match ModuleGraph::build(file) {
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
    let generated = match generate_module_graph_outputs(&graph, None, false, false, false) {
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
        let graph = match ModuleGraph::build_many(&group.files) {
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
        let generated = match generate_module_graph_outputs(&graph, None, false, false, false) {
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
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        inspect_types_directory_command(path, ignore_paths, ignore_from)
    } else {
        inspect_types_single_file_command(path)
    }
}

fn inspect_types_single_file_command(file: &Path) -> Result<(), CliError> {
    let graph = match ModuleGraph::build(file) {
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
        let graph = match ModuleGraph::build_many(&group.files) {
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

pub fn inspect_world_command(path: &Path, env: &str) -> Result<(), CliError> {
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

fn inspect_world_result(path: &Path, env: &str) -> Result<serde_json::Value, CliError> {
    let project = get_project_config(path)?.ok_or_else(|| {
        CliError::Validation(format!(
            "{}: no Sigil project found while inspecting runtime world",
            codes::topology::MISSING_MODULE
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
    "tcpDependencies": Array.from(__sigil_inspect_topology.tcp).sort()
  }},
  "summary": {{
    "clockKind": String(__sigil_inspect_world.clock?.kind ?? ""),
    "fsKind": String(__sigil_inspect_world.fs?.kind ?? ""),
    "logKind": String(__sigil_inspect_world.log?.kind ?? ""),
    "processKind": String(__sigil_inspect_world.process?.kind ?? ""),
    "randomKind": String(__sigil_inspect_world.random?.kind ?? ""),
    "timerKind": String(__sigil_inspect_world.timer?.kind ?? ""),
    "httpBindings": Object.keys(__sigil_inspect_world.http ?? {{}}).length,
    "tcpBindings": Object.keys(__sigil_inspect_world.tcp ?? {{}}).length
  }},
  "normalizedWorld": __sigil_inspect_world
}}));
"#,
            world_helpers = world_runtime_helpers_source(),
            prelude = prelude
        ),
    )?;

    let abs_runner = fs::canonicalize(&runner_path)?;
    let output = Command::new("pnpm")
        .args(["exec", "node", "--import", "tsx"])
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime(
                    "pnpm not found. Please install pnpm to inspect Sigil runtime worlds."
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

    let runner_json =
        serde_json::from_slice::<serde_json::Value>(&output.stdout).map_err(|error| {
            CliError::Runtime(format!(
                "inspect world runner emitted invalid JSON: {}",
                error
            ))
        })?;

    Ok(serde_json::json!({
        "input": path.to_string_lossy(),
        "project": project_json(Some(&project)),
        "projectRoot": project.root.to_string_lossy(),
        "environment": env,
        "topology": runner_json["topology"].clone(),
        "summary": runner_json["summary"].clone(),
        "normalizedWorld": runner_json["normalizedWorld"].clone()
    }))
}

fn compile_group(group: &CompileBatchGroup) -> Result<CompileBatchOutputs, CliError> {
    let graph = ModuleGraph::build_many(&group.files)?;
    let compiled_modules = graph.topo_order.len();
    let entry_modules = group
        .files
        .iter()
        .map(|file| {
            let module_key = entry_module_key(file)?;
            let module = graph.modules.get(&module_key).ok_or_else(|| {
                CliError::Codegen(format!(
                    "batch compile could not resolve entry module '{}'",
                    file.display()
                ))
            })?;
            Ok((
                file.clone(),
                module_key,
                module.project.as_ref().map(|project| project.root.clone()),
            ))
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    let compiled = compile_module_graph(graph, None, false, false, false)?;
    let entries = entry_modules
        .into_iter()
        .map(|(input, module_id, project_root)| {
            let output = compiled
                .module_outputs
                .get(&module_id)
                .cloned()
                .ok_or_else(|| {
                    CliError::Codegen(format!(
                        "batch compile did not produce output for '{}'",
                        input.display()
                    ))
                })?;
            let span_map = compiled
                .span_map_outputs
                .get(&module_id)
                .cloned()
                .ok_or_else(|| {
                    CliError::Codegen(format!(
                        "batch compile did not produce span map for '{}'",
                        input.display()
                    ))
                })?;
            Ok(CompileEntryOutput {
                input,
                output,
                span_map,
                project_root,
            })
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    Ok(CompileBatchOutputs {
        compiled_modules,
        entries,
    })
}

fn compile_single_file_command(
    file: &Path,
    output: Option<&Path>,
    show_types: bool,
) -> Result<(), CliError> {
    let graph = match ModuleGraph::build(file) {
        Ok(graph) => graph,
        Err(ModuleGraphError::Validation(errors)) => {
            if let Some(first_error) = errors.first() {
                let error_msg = first_error.to_string();
                let error_code = extract_error_code(&error_msg);

                output_json_error(
                    "sigilc compile",
                    "canonical",
                    &error_code,
                    &error_msg,
                    json!({
                        "file": file.to_string_lossy(),
                        "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                    }),
                );
            }
            return Err(ModuleGraphError::Validation(errors).into());
        }
        Err(error) => return Err(error.into()),
    };

    let entry_module_id = graph.topo_order.last().unwrap().clone();
    let entry_module = graph.modules.get(&entry_module_id).unwrap();
    let all_module_sources = graph
        .topo_order
        .iter()
        .map(|module_id| {
            (
                module_id.clone(),
                graph.modules[module_id]
                    .file_path
                    .to_string_lossy()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let project_json = project_json(entry_module.project.as_ref());

    let compiled = match compile_module_graph(graph, output, false, false, false) {
        Ok(compiled) => compiled,
        Err(CliError::Type(type_error)) => {
            output_json_error(
                "sigilc compile",
                "typecheck",
                &type_error.code,
                &type_error.message,
                type_error_json_details(&type_error),
            );
            return Err(CliError::Type(type_error));
        }
        Err(error) => return Err(error),
    };
    let entry_output = compiled.entry_output_path.clone();

    let all_modules: Vec<serde_json::Value> = all_module_sources
        .into_iter()
        .map(|(module_id, source_file)| {
            let output_file = compiled
                .module_outputs
                .get(&module_id)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();
            let span_map_file = compiled
                .span_map_outputs
                .get(&module_id)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();

            serde_json::json!({
                "moduleId": module_id,
                "sourceFile": source_file,
                "outputFile": output_file,
                "spanMapFile": span_map_file
            })
        })
        .collect();

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc compile",
        "ok": true,
        "phase": "codegen",
        "data": {
            "input": file.to_string_lossy(),
            "outputs": {
                "rootTs": entry_output.to_string_lossy(),
                "rootSpanMap": compiled.entry_span_map_path.to_string_lossy(),
                "allModules": all_modules
            },
            "project": project_json,
            "typecheck": {
                "ok": true,
                "inferred": if show_types { vec![] as Vec<serde_json::Value> } else { vec![] }
            }
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

fn compile_directory_command(
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files = collect_sigil_targets("compile", path, ignore_paths, ignore_from)?;
    let groups = group_compile_targets(&files)?;
    let group_count = groups.len();
    let file_order = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut compiled_file_count = 0usize;
    let mut compiled_module_count = 0usize;
    let mut compiled_entries = Vec::new();

    for group in groups {
        let first_file = group.files.first().cloned();
        let batch = match compile_group(&group) {
            Ok(batch) => batch,
            Err(error) => {
                match &error {
                    CliError::Type(type_error) => {
                        let mut details = match type_error_json_details(type_error) {
                            serde_json::Value::Object(map) => map,
                            _ => serde_json::Map::new(),
                        };
                        details.insert(
                            "input".to_string(),
                            json!(path.to_string_lossy().to_string()),
                        );
                        details.insert(
                            "file".to_string(),
                            json!(type_error.source_file.clone().or_else(|| first_file
                                .as_ref()
                                .map(|file| file.to_string_lossy().to_string()))),
                        );
                        details.insert("discovered".to_string(), json!(files.len()));
                        details.insert("compiled".to_string(), json!(compiled_file_count));
                        details.insert(
                            "durationMs".to_string(),
                            json!(start_time.elapsed().as_millis()),
                        );
                        output_json_error(
                            "sigilc compile",
                            "typecheck",
                            &type_error.code,
                            &type_error.message,
                            serde_json::Value::Object(details),
                        );
                    }
                    _ => {
                        let message = error.to_string();
                        let error_code = extract_error_code(&message);
                        output_json_error(
                            "sigilc compile",
                            "codegen",
                            &error_code,
                            &message,
                            json!({
                                "input": path.to_string_lossy(),
                                "file": first_file.map(|file| file.to_string_lossy().to_string()),
                                "discovered": files.len(),
                                "compiled": compiled_file_count,
                                "durationMs": start_time.elapsed().as_millis()
                            }),
                        );
                    }
                }
                return Err(error);
            }
        };

        compiled_module_count += batch.compiled_modules;
        compiled_file_count += batch.entries.len();
        compiled_entries.extend(batch.entries);
    }

    compiled_entries
        .sort_by_key(|entry| file_order.get(&entry.input).copied().unwrap_or(usize::MAX));
    let file_results = compiled_entries
        .into_iter()
        .map(|entry| {
            serde_json::json!({
                "input": entry.input.to_string_lossy(),
                "rootTs": entry.output.to_string_lossy(),
                "rootSpanMap": entry.span_map.to_string_lossy(),
                "projectRoot": entry.project_root.map(|root| root.to_string_lossy().to_string())
            })
        })
        .collect::<Vec<_>>();

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc compile",
        "ok": true,
        "phase": "codegen",
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "compiled": compiled_file_count,
                "groups": group_count,
                "modules": compiled_module_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

/// Compile command: compile a Sigil file to TypeScript
pub fn compile_command(
    path: &Path,
    output: Option<&Path>,
    show_types: bool,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        if output.is_some() {
            return Err(CliError::Validation(
                "compile -o is only valid when compiling a single file".to_string(),
            ));
        }
        compile_directory_command(path, ignore_paths, ignore_from)
    } else {
        compile_single_file_command(path, output, show_types)
    }
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
                    "engine": "node+tsx",
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
    let graph = ModuleGraph::build(file)?;
    let replay_mode = prepare_replay_mode(file, &graph, record_path, replay_path, args)?;
    let topology_prelude = if matches!(
        replay_mode.as_ref(),
        Some(PreparedReplayMode::Replay { .. })
    ) {
        String::new()
    } else {
        runner_prelude(file, &graph, selected_env)?.unwrap_or_default()
    };
    let trace_runtime_enabled = trace_enabled || breakpoints_requested;
    let compiled = compile_module_graph(graph, None, trace_enabled, breakpoints_requested, true)?;
    let module_debug_outputs = build_runtime_module_debug_outputs(&compiled)?;
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

    let runner_path = entry_output_path.with_extension("run.ts");
    let runtime_error_path = unique_runtime_error_path(&entry_output_path);
    let runtime_trace_path = trace_enabled.then(|| unique_runtime_trace_path(&entry_output_path));
    let runtime_breakpoint_path = breakpoint_config
        .as_ref()
        .map(|_| unique_runtime_breakpoint_path(&entry_output_path));
    let runtime_replay_path = replay_mode
        .as_ref()
        .map(|_| unique_runtime_replay_path(&entry_output_path));
    let module_name = entry_output_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let module_specifier_json = serde_json::to_string(&format!("./{}", module_name)).unwrap();
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
  const __sigil_module = await import({module_specifier_json});
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
}} catch (error) {{
  if (__sigil_runtime_is_breakpoint_stop(error)) {{
    // Intentional early stop for machine-first breakpoint debugging.
  }} else {{
  const captured = await __sigil_runtime_capture_error(error);
  {catch_replay_mark}
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
    let start_time = Instant::now();
    if stream_output {
        if io::stdout().is_terminal() || io::stderr().is_terminal() {
            let status = Command::new("pnpm")
                .args(["exec", "node", "--import", "tsx"])
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
            });
        }

        let mut child = Command::new("pnpm")
            .args(["exec", "node", "--import", "tsx"])
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
        });
    }

    let output = Command::new("pnpm")
        .args(["exec", "node", "--import", "tsx"])
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
        .join(format!("inspect-world.{unique}.run.ts"))
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
    format!("{:x}", Sha256::digest(bytes))
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
            fingerprint: format!("{:x}", fingerprint_hasher.finalize()),
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
        let graph = ModuleGraph::build(test_file)?;
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
            fingerprint: format!("{:x}", fingerprint_hasher.finalize()),
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
        canonicalize_existing_path(path).to_string_lossy().to_string()
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
    record_path: Option<&Path>,
    replay_path: Option<&Path>,
) -> Result<Option<PreparedTestReplayMode>, CliError> {
    let (request, binding) = build_test_replay_binding(path, test_files, match_filter)?;

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
    serde_json::from_str(&contents).ok()
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
        "engine": "node+tsx",
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
    let code = capture
        .sigil_code
        .as_deref()
        .filter(|code| !code.is_empty())
        .unwrap_or(codes::runtime::UNCAUGHT_EXCEPTION);
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

    let sigil_code = stack
        .contains("SIGIL-")
        .then(|| extract_error_code(headline));

    Some(RuntimeExceptionCapture {
        name,
        message,
        stack: stack.to_string(),
        sigil_code,
        expression: None,
    })
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
        sigil_frame: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_exception_capture_from_stderr_extracts_sigil_code() {
        let capture = runtime_exception_capture_from_stderr(
            "Error: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil\n    at main (/tmp/example.run.ts:12:3)",
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
        assert!(capture.stack.contains(".run.ts"));
    }

    #[test]
    fn runtime_exception_capture_from_stderr_prefers_sigil_line_after_warning() {
        let capture = runtime_exception_capture_from_stderr(
            "(node:2468) ExperimentalWarning: import assertions are deprecated\nError: SIGIL-TOPO-ENV-NOT-FOUND: environment 'staging' not declared in src/topology.lib.sigil\n    at main (/tmp/example.run.ts:12:3)",
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
            "{}: pnpm not found. Please install pnpm to run Sigil programs.",
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
            "cli",
            codes::cli::UNEXPECTED,
            &project_error.to_string(),
            json!({
                "file": file.to_string_lossy()
            }),
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
        CliError::ModuleGraph(ModuleGraphError::Lexer(message))
        | CliError::ModuleGraph(ModuleGraphError::Parser(message)) => {
            output_test_message_error(path, message);
        }
        CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
        | CliError::ProjectConfig(project_error) => output_json_error_to(
            "sigilc test",
            "cli",
            codes::cli::UNEXPECTED,
            &project_error.to_string(),
            json!({
                "path": path.to_string_lossy()
            }),
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
    let suite_replay_mode =
        match prepare_test_replay_mode(path, &test_files, match_filter, record_path, replay_path) {
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

pub fn validate_command(path: &Path, env: &str) -> Result<(), CliError> {
    let project_root = get_project_config(path)?
        .map(|project| project.root)
        .ok_or_else(|| {
            CliError::Validation(format!(
                "{}: no Sigil project found while validating topology",
                codes::topology::MISSING_MODULE
            ))
        })?;

    if !topology_source_path(&project_root).exists() {
        return Err(CliError::Validation(format!(
            "{}: topology-aware projects require src/topology.lib.sigil",
            codes::topology::MISSING_MODULE
        )));
    }

    let _compiled = compile_topology_module(&project_root)?;
    let prelude = build_world_runtime_prelude(&project_root, env, true)?;
    let runner_path = project_root.join(".local/topology.validate.run.ts");
    if let Some(parent) = runner_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &runner_path,
        format!(
            r#"{prelude}
console.log(JSON.stringify({{
  ok: true,
  environment: {env_json}
}}));
"#,
            prelude = prelude,
            env_json = serde_json::to_string(env).unwrap()
        ),
    )?;

    let abs_runner = fs::canonicalize(&runner_path)?;
    let output = Command::new("pnpm")
        .args(&["exec", "node", "--import", "tsx"])
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime(
                    "pnpm not found. Please install pnpm to validate Sigil topology.".to_string(),
                )
            } else {
                CliError::Runtime(format!("Failed to execute topology validation: {}", e))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(CliError::Validation(if message.is_empty() {
            "topology validation failed".to_string()
        } else {
            message.to_string()
        }));
    }

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc validate",
        "ok": true,
        "phase": "topology",
        "data": {
            "environment": env,
            "projectRoot": project_root.to_string_lossy()
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TestLocation {
    line: usize,
    column: usize,
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

#[derive(Debug, Clone)]
struct CoverageTarget {
    id: String,
    expected_variants: Vec<String>,
    file: String,
    function_name: String,
    location: TestLocation,
}

#[derive(Debug, Clone, Default)]
struct CoverageObservation {
    calls: HashSet<String>,
    variants: HashMap<String, HashSet<String>>,
}

struct CompiledGraphOutputs {
    entry_output_path: PathBuf,
    entry_span_map_path: PathBuf,
    module_outputs: HashMap<String, PathBuf>,
    span_map_outputs: HashMap<String, PathBuf>,
    coverage_targets: Vec<CoverageTarget>,
}

#[derive(Debug, Clone)]
struct GeneratedModuleOutput {
    output_path: PathBuf,
    span_map: ModuleSpanMap,
    span_map_path: PathBuf,
    ts_code: String,
}

struct GeneratedGraphOutputs {
    coverage_targets: Vec<CoverageTarget>,
    entry_output_path: PathBuf,
    entry_span_map_path: PathBuf,
    module_outputs: HashMap<String, GeneratedModuleOutput>,
}

fn analyze_module_graph(graph: &ModuleGraph) -> Result<AnalyzedGraphOutputs, CliError> {
    let mut compiled_modules = HashMap::new();
    let mut compiled_schemes = HashMap::new();
    let mut coverage_targets = Vec::new();
    let mut type_registries = HashMap::new();
    let mut analyzed_modules = HashMap::new();

    for module_id in &graph.topo_order {
        let module = &graph.modules[module_id];

        let imported_namespaces = build_imported_namespaces(&module.ast, &compiled_modules);
        let imported_type_regs = build_imported_type_registries(&module.ast, &type_registries);
        let imported_value_schemes = build_imported_value_schemes(&module.ast, &compiled_schemes);
        let effect_catalog = load_project_effect_catalog_for(&module.file_path)?;

        let typecheck_result = type_check(
            &module.ast,
            &module.source,
            Some(TypeCheckOptions {
                effect_catalog,
                imported_namespaces: Some(imported_namespaces),
                imported_type_registries: Some(imported_type_regs.clone()),
                imported_value_schemes: Some(imported_value_schemes),
                source_file: Some(module.file_path.to_string_lossy().to_string()),
            }),
        )
        .map_err(CliError::Type)?;

        validate_typed_canonical_form(
            &typecheck_result.typed_program,
            Some(module.file_path.to_string_lossy().as_ref()),
        )
        .map_err(|errors| CliError::Validation(format_validation_errors(&errors)))?;

        coverage_targets.extend(collect_module_coverage_targets(
            module,
            &typecheck_result.typed_program,
            &imported_type_regs,
            &typecheck_result
                .typed_program
                .declarations
                .iter()
                .filter_map(|decl| match decl {
                    TypedDeclaration::Type(type_decl) => Some((
                        type_decl.ast.name.clone(),
                        TypeInfo {
                            type_params: type_decl.ast.type_params.clone(),
                            definition: type_decl.ast.definition.clone(),
                            constraint: type_decl.ast.constraint.clone(),
                        },
                    )),
                    _ => None,
                })
                .collect(),
        ));

        let collected_span_map = collect_module_span_map(
            module_id,
            &module.file_path.to_string_lossy(),
            "",
            &typecheck_result.typed_program,
        );

        let sigil_typechecker::typed_ir::TypeCheckResult {
            declaration_types,
            declaration_schemes,
            typed_program,
        } = typecheck_result;

        compiled_schemes.insert(module_id.clone(), declaration_schemes.clone());
        compiled_modules.insert(module_id.clone(), declaration_types.clone());
        type_registries.insert(
            module_id.clone(),
            extract_type_registry(&module.ast, &module.file_path, module_id),
        );
        analyzed_modules.insert(
            module_id.clone(),
            AnalyzedModule {
                module_id: module_id.clone(),
                file_path: module.file_path.clone(),
                project: module.project.clone(),
                typed_program,
                declaration_types,
                declaration_schemes,
                declaration_span_ids: collected_span_map.declaration_span_ids,
            },
        );
    }

    Ok(AnalyzedGraphOutputs {
        compiled_modules: graph.topo_order.len(),
        modules: analyzed_modules,
        coverage_targets,
    })
}

fn generate_module_graph_outputs(
    graph: &ModuleGraph,
    output_override: Option<&Path>,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
) -> Result<GeneratedGraphOutputs, CliError> {
    let analyzed = analyze_module_graph(graph)?;
    let entry_module_id = graph
        .topo_order
        .last()
        .ok_or_else(|| CliError::Codegen("codegen requires at least one module".to_string()))?;
    let mut entry_output_path = PathBuf::new();
    let mut entry_span_map_path = PathBuf::new();
    let mut module_outputs = HashMap::new();

    for module_id in &graph.topo_order {
        let module = &graph.modules[module_id];
        let analyzed_module = analyzed.modules.get(module_id).ok_or_else(|| {
            CliError::Codegen(format!(
                "codegen could not resolve analyzed module '{}'",
                module.file_path.display()
            ))
        })?;
        let output_path = if module_id == entry_module_id && output_override.is_some() {
            output_override.unwrap().to_path_buf()
        } else {
            get_module_output_path(module)
        };
        let codegen_options = CodegenOptions {
            module_id: Some(module_id.clone()),
            source_file: Some(module.file_path.to_string_lossy().to_string()),
            output_file: Some(output_path.to_string_lossy().to_string()),
            trace,
            breakpoints,
            expression_debug,
        };
        let mut codegen = TypeScriptGenerator::new(codegen_options);
        let ts_code = codegen
            .generate(&analyzed_module.typed_program)
            .map_err(|e| CliError::Codegen(format!("{}", e)))?;
        let span_map = codegen.generated_span_map().cloned().ok_or_else(|| {
            CliError::Codegen(format!(
                "codegen did not produce a span map for '{}'",
                module.file_path.display()
            ))
        })?;
        let span_map_path = span_map_output_path(&output_path);

        module_outputs.insert(
            module_id.clone(),
            GeneratedModuleOutput {
                output_path: output_path.clone(),
                span_map,
                span_map_path: span_map_path.clone(),
                ts_code,
            },
        );

        if module_id == entry_module_id {
            entry_output_path = output_path;
            entry_span_map_path = span_map_path;
        }
    }

    Ok(GeneratedGraphOutputs {
        coverage_targets: analyzed.coverage_targets,
        entry_output_path,
        entry_span_map_path,
        module_outputs,
    })
}

fn compile_module_graph(
    graph: ModuleGraph,
    output_override: Option<&Path>,
    trace: bool,
    breakpoints: bool,
    expression_debug: bool,
) -> Result<CompiledGraphOutputs, CliError> {
    let generated = generate_module_graph_outputs(
        &graph,
        output_override,
        trace,
        breakpoints,
        expression_debug,
    )?;
    let mut module_outputs = HashMap::new();
    let mut span_map_outputs = HashMap::new();
    for (module_id, generated_output) in generated.module_outputs {
        if let Some(parent) = generated_output.output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&generated_output.output_path, generated_output.ts_code)?;
        write_span_map_file(&generated_output.span_map_path, &generated_output.span_map)?;
        module_outputs.insert(module_id.clone(), generated_output.output_path);
        span_map_outputs.insert(module_id, generated_output.span_map_path);
    }

    Ok(CompiledGraphOutputs {
        entry_output_path: generated.entry_output_path,
        entry_span_map_path: generated.entry_span_map_path,
        module_outputs,
        span_map_outputs,
        coverage_targets: generated.coverage_targets,
    })
}

fn span_map_output_path(output_path: &Path) -> PathBuf {
    output_path.with_extension("span.json")
}

fn write_span_map_file(path: &Path, span_map: &ModuleSpanMap) -> Result<(), CliError> {
    let serialized = serde_json::to_string(span_map)
        .map_err(|error| CliError::Codegen(format!("failed to serialize span map: {}", error)))?;
    fs::write(path, serialized)?;
    Ok(())
}

fn topology_source_path(project_root: &Path) -> PathBuf {
    project_root.join("src/topology.lib.sigil")
}

fn config_source_path(project_root: &Path, env_name: &str) -> PathBuf {
    project_root
        .join("config")
        .join(format!("{}.lib.sigil", env_name))
}

fn compile_topology_module(project_root: &Path) -> Result<CompiledGraphOutputs, CliError> {
    let topology_source = topology_source_path(project_root);
    if !topology_source.exists() {
        return Err(CliError::Validation(format!(
            "{}: topology-aware projects require src/topology.lib.sigil",
            codes::topology::MISSING_MODULE
        )));
    }

    let graph = ModuleGraph::build(&topology_source)?;
    compile_module_graph(graph, None, false, false, false)
}

fn compile_config_module(
    project_root: &Path,
    env_name: &str,
) -> Result<CompiledGraphOutputs, CliError> {
    let config_source = config_source_path(project_root, env_name);
    if !config_source.exists() {
        return Err(CliError::Validation(format!(
            "{}: topology environment '{}' requires config/{}.lib.sigil",
            codes::topology::MISSING_CONFIG_MODULE,
            env_name,
            env_name
        )));
    }

    let graph = ModuleGraph::build(&config_source)?;
    compile_module_graph(graph, None, false, false, false)
}

fn build_world_runtime_prelude(
    project_root: &Path,
    env_name: &str,
    topology_present: bool,
) -> Result<String, CliError> {
    let topology_url = if topology_present {
        let topology_outputs = compile_topology_module(project_root)?;
        let topology_output = topology_outputs.entry_output_path;
        Some(format!(
            "file://{}",
            fs::canonicalize(topology_output)?.display()
        ))
    } else {
        None
    };
    let config_outputs = compile_config_module(project_root, env_name)?;
    let config_output = config_outputs.entry_output_path;
    let config_url = format!("file://{}", fs::canonicalize(config_output)?.display());
    let env_name_json = serde_json::to_string(env_name).unwrap();

    Ok(format!(
        r#"{topology_import}
const __sigil_config_module = await import("{config_url}");
const __sigil_config_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_config_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);
const __sigil_world_env_name = {env_name_json};

function __sigil_runtime_fail(code, message) {{
  const error = new Error(`${{code}}: ${{message}}`);
  error.sigilCode = code;
  throw error;
}}

function __sigil_runtime_collect_topology(moduleExports) {{
  const envs = new Set();
  const http = new Set();
  const tcp = new Set();
  for (const value of Object.values(moduleExports ?? {{}})) {{
    if (value?.__tag === 'Environment') {{
      envs.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'HttpServiceDependency') {{
      http.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'TcpServiceDependency') {{
      tcp.add(String(value.__fields?.[0] ?? ''));
    }}
  }}
  return {{ envs, http, tcp }};
}}

function __sigil_runtime_collect_world_dependency_names(entries, expectedTag) {{
  if (!Array.isArray(entries)) {{
    __sigil_runtime_fail("{binding_kind}", `world ${{
      expectedTag === 'HttpEntry' ? 'http' : 'tcp'
    }} entries must be a list`);
  }}
  const seen = new Set();
  for (const entry of entries) {{
    if (!entry || typeof entry !== 'object' || entry.__tag !== expectedTag) {{
      __sigil_runtime_fail("{binding_kind}", `world entries must be tagged as ${{expectedTag}}`);
    }}
    const dependencyName = String(entry.__fields?.[0]?.dependencyName ?? '');
    if (!dependencyName) {{
      __sigil_runtime_fail("{binding_kind}", 'world entries must include dependencyName');
    }}
    if (seen.has(dependencyName)) {{
      __sigil_runtime_fail("{duplicate_binding}", `duplicate world entry for '${{dependencyName}}'`);
    }}
    seen.add(dependencyName);
  }}
  return seen;
}}

function __sigil_runtime_read_world(configExports) {{
  const world = configExports.world;
  if (!world || typeof world !== 'object') {{
    __sigil_runtime_fail("{invalid_config}", "config module must export a 'world' value");
  }}
  for (const field of ['clock', 'fs', 'http', 'log', 'process', 'random', 'tcp', 'timer']) {{
    if (!(field in world)) {{
      __sigil_runtime_fail("{invalid_config}", `world is missing '${{field}}'`);
    }}
  }}
  return world;
}}

const __sigil_world_value = __sigil_runtime_read_world(__sigil_config_exports);
const __sigil_topology_info = __sigil_runtime_collect_topology(globalThis.__sigil_topology_exports ?? {{}});
if (__sigil_topology_info.envs.size > 0 && !__sigil_topology_info.envs.has(__sigil_world_env_name)) {{
  __sigil_runtime_fail("{env_not_found}", `environment '${{__sigil_world_env_name}}' not declared in src/topology.lib.sigil`);
}}
const __sigil_http_world_names = __sigil_runtime_collect_world_dependency_names(__sigil_world_value.http, 'HttpEntry');
const __sigil_tcp_world_names = __sigil_runtime_collect_world_dependency_names(__sigil_world_value.tcp, 'TcpEntry');
for (const dependencyName of __sigil_topology_info.http) {{
  if (!__sigil_http_world_names.has(dependencyName)) {{
    __sigil_runtime_fail("{missing_binding}", `missing HTTP world entry for '${{dependencyName}}' in environment '${{__sigil_world_env_name}}'`);
  }}
}}
for (const dependencyName of __sigil_topology_info.tcp) {{
  if (!__sigil_tcp_world_names.has(dependencyName)) {{
    __sigil_runtime_fail("{missing_binding}", `missing TCP world entry for '${{dependencyName}}' in environment '${{__sigil_world_env_name}}'`);
  }}
}}
for (const dependencyName of __sigil_http_world_names) {{
  if (__sigil_topology_info.http.size > 0 && !__sigil_topology_info.http.has(dependencyName)) {{
    __sigil_runtime_fail("{invalid_handle}", `HTTP world entry references undeclared dependency '${{dependencyName}}'`);
  }}
}}
for (const dependencyName of __sigil_tcp_world_names) {{
  if (__sigil_topology_info.tcp.size > 0 && !__sigil_topology_info.tcp.has(dependencyName)) {{
    __sigil_runtime_fail("{invalid_handle}", `TCP world entry references undeclared dependency '${{dependencyName}}'`);
  }}
}}
globalThis.__sigil_world_env_name = __sigil_world_env_name;
globalThis.__sigil_world_value = __sigil_world_value;
globalThis.__sigil_world_template_cache = undefined;
globalThis.__sigil_world_current = undefined;
"#,
        topology_import = topology_url.map_or_else(
            || "globalThis.__sigil_topology_exports = null;".to_string(),
            |topology_url| {
                format!(
                    r#"const __sigil_topology_module = await import("{topology_url}");
globalThis.__sigil_topology_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_topology_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);"#
                )
            }
        ),
        config_url = config_url,
        env_name_json = env_name_json,
        invalid_handle = codes::topology::INVALID_HANDLE,
        binding_kind = codes::topology::BINDING_KIND_MISMATCH,
        missing_binding = codes::topology::MISSING_BINDING,
        duplicate_binding = codes::topology::DUPLICATE_BINDING,
        env_not_found = codes::topology::ENV_NOT_FOUND,
        invalid_config = codes::topology::INVALID_CONFIG_MODULE,
    ))
}

fn project_root_and_runtime(
    path: &Path,
    graph: &ModuleGraph,
) -> Result<Option<(PathBuf, bool, bool)>, ProjectConfigError> {
    let Some(project) = get_project_config(path)? else {
        return Ok(None);
    };
    let topology_present = topology_source_path(&project.root).exists();
    let config_imported = graph
        .modules
        .keys()
        .any(|module_id| module_id.starts_with("config::"));
    Ok(Some((project.root, topology_present, config_imported)))
}

fn runner_prelude(
    path: &Path,
    graph: &ModuleGraph,
    selected_env: Option<&str>,
) -> Result<Option<String>, CliError> {
    let Some((project_root, topology_present, config_imported)) =
        project_root_and_runtime(path, graph)?
    else {
        return Ok(None);
    };

    if !topology_present && !config_imported {
        return Ok(None);
    }

    let env_name = selected_env.ok_or_else(|| {
        CliError::Validation(format!(
            "{}: runtime-world projects require --env <name>",
            codes::topology::ENV_REQUIRED
        ))
    })?;

    build_world_runtime_prelude(&project_root, env_name, topology_present).map(Some)
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
    let graph = ModuleGraph::build(file)?;
    let topology_prelude = if matches!(suite_replay_mode, Some(PreparedTestReplayMode::Replay { .. }))
    {
        None
    } else {
        runner_prelude(file, &graph, selected_env)?
    };
    let compiled = compile_module_graph(
        graph,
        None,
        debug_options.trace_enabled,
        debug_options.breakpoints_requested(),
        true,
    )?;
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
        "{}.{}.runner.ts",
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
    let trace_runtime_enabled = debug_options.trace_enabled || debug_options.breakpoints_requested();
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
const discoverMod = await import(moduleUrl);
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
    let output = Command::new("pnpm")
        .args(&["exec", "node", "--import", "tsx"])
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime("pnpm not found".to_string())
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
            let code = capture
                .sigil_code
                .as_deref()
                .filter(|code| !code.is_empty())
                .unwrap_or(codes::runtime::UNCAUGHT_EXCEPTION);
            let normalized_message = normalize_runtime_exception_message(capture, code);
            let analysis = analyze_runtime_exception(capture, module_debug_outputs);
            runtime_exception_json(capture, &normalized_message, &analysis, module_debug_outputs)
        });
        let trace = debug_options
            .trace_enabled
            .then(|| runtime_trace_json(result.trace.as_ref()));
        let breakpoints = breakpoint_config.map(|config| {
            runtime_breakpoints_json(Some(config), result.breakpoints.as_ref(), module_debug_outputs)
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

// ============================================================================
// Multi-module Compilation Helpers
// ============================================================================

/// Build imported namespaces from already-compiled modules
///
/// For each import, creates a namespace type (record) containing exported functions/constants
fn build_imported_namespaces(
    _ast: &Program,
    compiled_modules: &HashMap<String, HashMap<String, InferenceType>>,
) -> HashMap<String, InferenceType> {
    let mut imported = HashMap::new();

    for (module_id, types) in compiled_modules {
        let mut fields = HashMap::new();
        for (name, typ) in types {
            fields.insert(
                name.clone(),
                qualify_inference_type_in_context(typ, module_id),
            );
        }

        imported.insert(
            module_id.clone(),
            InferenceType::Record(TRecord {
                fields,
                name: Some(module_id.clone()),
            }),
        );
    }

    imported
}

fn is_core_prelude_name(name: &str) -> bool {
    matches!(
        name,
        "ConcurrentOutcome"
            | "Option"
            | "Result"
            | "Aborted"
            | "Failure"
            | "Success"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
    )
}

fn qualify_inference_type_in_context(typ: &InferenceType, module_id: &str) -> InferenceType {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Var(_) | InferenceType::Any => typ.clone(),
        InferenceType::Function(func) => InferenceType::Function(Box::new(TFunction {
            params: func
                .params
                .iter()
                .map(|param| qualify_inference_type_in_context(param, module_id))
                .collect(),
            return_type: qualify_inference_type_in_context(&func.return_type, module_id),
            effects: func.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: qualify_inference_type_in_context(&list.element_type, module_id),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: qualify_inference_type_in_context(&map.key_type, module_id),
            value_type: qualify_inference_type_in_context(&map.value_type, module_id),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| qualify_inference_type_in_context(item, module_id))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        qualify_inference_type_in_context(field_type, module_id),
                    )
                })
                .collect(),
            name: record.name.as_ref().map(|name| {
                if is_core_prelude_name(name) {
                    name.clone()
                } else if name.contains("::") {
                    name.clone()
                } else if name.contains('.') {
                    name.clone()
                } else {
                    format!("{}.{}", module_id, name)
                }
            }),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: if is_core_prelude_name(&constructor.name) {
                constructor.name.clone()
            } else if constructor.name.contains("::") || constructor.name.contains('.') {
                constructor.name.clone()
            } else {
                format!("{}.{}", module_id, constructor.name)
            },
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| qualify_inference_type_in_context(arg, module_id))
                .collect(),
        }),
    }
}

/// Build imported type registries from dependencies
///
/// Extracts type definitions (sum types, product types) from imported modules
fn build_imported_type_registries(
    _ast: &Program,
    type_registries: &HashMap<String, HashMap<String, TypeInfo>>,
) -> HashMap<String, HashMap<String, TypeInfo>> {
    type_registries.clone()
}

fn build_imported_value_schemes(
    _ast: &Program,
    compiled_schemes: &HashMap<String, HashMap<String, TypeScheme>>,
) -> HashMap<String, HashMap<String, TypeScheme>> {
    let mut imported = HashMap::new();

    for (module_id, schemes) in compiled_schemes {
        imported.insert(
            module_id.clone(),
            schemes
                .iter()
                .map(|(name, scheme)| {
                    (
                        name.clone(),
                        qualify_scheme_for_module(module_id.as_str(), scheme),
                    )
                })
                .collect(),
        );
    }

    imported
}

fn qualify_inference_type_for_module(
    module_id: &str,
    typ: &sigil_typechecker::InferenceType,
) -> sigil_typechecker::InferenceType {
    use sigil_typechecker::types::{TConstructor, TFunction, TList, TRecord, TTuple, TVar};
    use sigil_typechecker::InferenceType;

    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => typ.clone(),
        InferenceType::Var(var) => InferenceType::Var(Box::new(TVar {
            id: var.id,
            name: var.name.clone(),
            instance: var
                .instance
                .as_ref()
                .map(|instance| qualify_inference_type_for_module(module_id, instance)),
        })),
        InferenceType::Function(function) => InferenceType::Function(Box::new(TFunction {
            params: function
                .params
                .iter()
                .map(|param| qualify_inference_type_for_module(module_id, param))
                .collect(),
            return_type: qualify_inference_type_for_module(module_id, &function.return_type),
            effects: function.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: qualify_inference_type_for_module(module_id, &list.element_type),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: qualify_inference_type_for_module(module_id, &map.key_type),
            value_type: qualify_inference_type_for_module(module_id, &map.value_type),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| qualify_inference_type_for_module(module_id, item))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        qualify_inference_type_for_module(module_id, field_type),
                    )
                })
                .collect(),
            name: record.name.as_ref().map(|name| {
                if is_core_prelude_name(name) {
                    name.clone()
                } else if name.contains('.') {
                    name.clone()
                } else {
                    format!("{}.{}", module_id, name)
                }
            }),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: if is_core_prelude_name(&constructor.name) {
                constructor.name.clone()
            } else if constructor.name.contains('.') {
                constructor.name.clone()
            } else {
                format!("{}.{}", module_id, constructor.name)
            },
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| qualify_inference_type_for_module(module_id, arg))
                .collect(),
        }),
    }
}

fn qualify_scheme_for_module(module_id: &str, scheme: &TypeScheme) -> TypeScheme {
    TypeScheme {
        quantified_vars: scheme.quantified_vars.clone(),
        typ: qualify_inference_type_for_module(module_id, &scheme.typ),
    }
}

fn qualify_type_in_context(
    ast_type: &Type,
    module_id: &str,
    local_type_registry: &HashMap<String, TypeInfo>,
    type_params: &[String],
) -> Type {
    match ast_type {
        Type::Primitive(_) => ast_type.clone(),
        Type::Qualified(qualified) => Type::Qualified(sigil_ast::QualifiedType {
            module_path: qualified.module_path.clone(),
            type_name: qualified.type_name.clone(),
            type_args: qualified
                .type_args
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            location: qualified.location,
        }),
        Type::List(list_type) => Type::List(Box::new(sigil_ast::ListType {
            element_type: qualify_type_in_context(
                &list_type.element_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: list_type.location,
        })),
        Type::Map(map_type) => Type::Map(Box::new(sigil_ast::MapType {
            key_type: qualify_type_in_context(
                &map_type.key_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            value_type: qualify_type_in_context(
                &map_type.value_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: map_type.location,
        })),
        Type::Function(func_type) => Type::Function(Box::new(sigil_ast::FunctionType {
            param_types: func_type
                .param_types
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            effects: func_type.effects.clone(),
            return_type: qualify_type_in_context(
                &func_type.return_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: func_type.location,
        })),
        Type::Tuple(tuple_type) => Type::Tuple(sigil_ast::TupleType {
            types: tuple_type
                .types
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            location: tuple_type.location,
        }),
        Type::Variable(var_type) => {
            if is_core_prelude_name(&var_type.name)
                || type_params.contains(&var_type.name)
                || !local_type_registry.contains_key(&var_type.name)
            {
                return ast_type.clone();
            }

            Type::Qualified(sigil_ast::QualifiedType {
                module_path: module_id.split("::").map(|s| s.to_string()).collect(),
                type_name: var_type.name.clone(),
                type_args: vec![],
                location: var_type.location,
            })
        }
        Type::Constructor(constructor) => {
            let qualified_args = constructor
                .type_args
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect();

            if local_type_registry.contains_key(&constructor.name)
                && !type_params.contains(&constructor.name)
            {
                Type::Qualified(sigil_ast::QualifiedType {
                    module_path: module_id.split("::").map(|s| s.to_string()).collect(),
                    type_name: constructor.name.clone(),
                    type_args: qualified_args,
                    location: constructor.location,
                })
            } else {
                Type::Constructor(sigil_ast::TypeConstructor {
                    name: constructor.name.clone(),
                    type_args: qualified_args,
                    location: constructor.location,
                })
            }
        }
    }
}

fn qualify_type_def(
    type_def: &TypeDef,
    module_id: &str,
    local_type_registry: &HashMap<String, TypeInfo>,
    type_params: &[String],
) -> TypeDef {
    match type_def {
        TypeDef::Product(product) => TypeDef::Product(sigil_ast::ProductType {
            fields: product
                .fields
                .iter()
                .map(|field| sigil_ast::Field {
                    name: field.name.clone(),
                    field_type: qualify_type_in_context(
                        &field.field_type,
                        module_id,
                        local_type_registry,
                        type_params,
                    ),
                    location: field.location,
                })
                .collect(),
            location: product.location,
        }),
        TypeDef::Alias(alias) => TypeDef::Alias(sigil_ast::TypeAlias {
            aliased_type: qualify_type_in_context(
                &alias.aliased_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: alias.location,
        }),
        TypeDef::Sum(sum) => TypeDef::Sum(sigil_ast::SumType {
            variants: sum
                .variants
                .iter()
                .map(|variant| sigil_ast::Variant {
                    name: variant.name.clone(),
                    types: variant
                        .types
                        .iter()
                        .map(|ty| {
                            qualify_type_in_context(ty, module_id, local_type_registry, type_params)
                        })
                        .collect(),
                    location: variant.location,
                })
                .collect(),
            location: sum.location,
        }),
    }
}

/// Extract type registry from a module's AST
///
/// Collects all exported type definitions for use by dependent modules
fn extract_type_registry(
    ast: &Program,
    file_path: &std::path::Path,
    module_id: &str,
) -> HashMap<String, TypeInfo> {
    let mut registry = HashMap::new();

    // Only .lib.sigil files export types
    let is_lib_file = file_path.to_string_lossy().ends_with(".lib.sigil");

    let mut local_type_registry = HashMap::new();
    for decl in &ast.declarations {
        if let Declaration::Type(type_decl) = decl {
            local_type_registry.insert(
                type_decl.name.clone(),
                TypeInfo {
                    type_params: type_decl.type_params.clone(),
                    definition: type_decl.definition.clone(),
                    constraint: type_decl.constraint.clone(),
                },
            );
        }
    }

    for decl in &ast.declarations {
        if let Declaration::Type(type_decl) = decl {
            if is_lib_file {
                registry.insert(
                    type_decl.name.clone(),
                    TypeInfo {
                        type_params: type_decl.type_params.clone(),
                        definition: qualify_type_def(
                            &type_decl.definition,
                            module_id,
                            &local_type_registry,
                            &type_decl.type_params,
                        ),
                        constraint: type_decl.constraint.clone(),
                    },
                );
            }
        }
    }

    registry
}

fn coverage_variant_names_for_type_def(type_def: &TypeDef) -> Vec<String> {
    match type_def {
        TypeDef::Sum(sum) => sum
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn coverage_expected_variants(
    return_type: &InferenceType,
    local_type_registry: &HashMap<String, TypeInfo>,
    imported_type_regs: &HashMap<String, HashMap<String, TypeInfo>>,
) -> Vec<String> {
    let InferenceType::Constructor(constructor) = return_type else {
        return Vec::new();
    };

    match constructor.name.as_str() {
        "Option" => return vec!["None".to_string(), "Some".to_string()],
        "Result" => return vec!["Err".to_string(), "Ok".to_string()],
        _ => {}
    }

    if let Some(info) = local_type_registry.get(&constructor.name) {
        let variants = coverage_variant_names_for_type_def(&info.definition);
        if !variants.is_empty() {
            return variants;
        }
    }

    let mut imported_matches = imported_type_regs
        .values()
        .filter_map(|registry| registry.get(&constructor.name))
        .map(|info| coverage_variant_names_for_type_def(&info.definition))
        .filter(|variants| !variants.is_empty())
        .collect::<Vec<_>>();

    if imported_matches.len() == 1 {
        return imported_matches.pop().unwrap();
    }

    Vec::new()
}

fn collect_module_coverage_targets(
    module: &LoadedModule,
    typed_program: &TypedProgram,
    imported_type_regs: &HashMap<String, HashMap<String, TypeInfo>>,
    local_type_registry: &HashMap<String, TypeInfo>,
) -> Vec<CoverageTarget> {
    let Some(project) = &module.project else {
        return Vec::new();
    };

    let normalized_path = module.file_path.to_string_lossy().replace('\\', "/");
    let normalized_root = project.root.to_string_lossy().replace('\\', "/");
    if !normalized_path.starts_with(&normalized_root) || !normalized_path.contains("/src/") {
        return Vec::new();
    }
    if normalized_path.contains("/tests/") {
        return Vec::new();
    }

    let is_lib_file = normalized_path.ends_with(".lib.sigil");
    let is_exec_file = normalized_path.ends_with(".sigil") && !is_lib_file;
    if !is_lib_file && !is_exec_file {
        return Vec::new();
    }

    let mut targets = Vec::new();

    for decl in &typed_program.declarations {
        let TypedDeclaration::Function(function) = decl else {
            continue;
        };

        let expected_variants = coverage_expected_variants(
            &function.return_type,
            local_type_registry,
            imported_type_regs,
        );
        let id = format!("{}::{}", module.id, function.name);
        targets.push(CoverageTarget {
            id,
            expected_variants,
            file: normalized_path.clone(),
            function_name: function.name.clone(),
            location: TestLocation {
                line: function.location.start.line,
                column: function.location.start.column,
            },
        });
    }

    targets
}

/// Get output path for a compiled module
///
/// Converts module ID to file path, using repo root's .local directory:
/// - stdlib::list => <repo_root>/.local/language/stdlib/list.ts
/// - src::utils => <repo_root>/.local/path/to/src/utils.ts
fn get_module_output_path(module: &LoadedModule) -> PathBuf {
    use std::env;
    use std::fs;

    // Check if this is a project file
    if let Some(project) = module.project.clone() {
        // Use project's output directory
        let path_str = module.id.replace("::", "/");
        return project
            .root
            .join(&project.layout.out)
            .join(format!("{}.ts", path_str));
    }

    // For non-project files, use repo root's .local/
    // Find repo root by walking up from source file
    let abs_source =
        fs::canonicalize(&module.file_path).unwrap_or_else(|_| module.file_path.clone());
    let mut repo_root = abs_source.parent().unwrap().to_path_buf();

    // Walk up to find .git directory (repo root marker)
    while !repo_root.join(".git").exists() {
        if let Some(parent) = repo_root.parent() {
            repo_root = parent.to_path_buf();
        } else {
            // If we can't find .git, fall back to current directory
            repo_root = env::current_dir().unwrap();
            break;
        }
    }

    if module.id.contains("::") {
        return repo_root
            .join(".local")
            .join(format!("{}.ts", module.id.replace("::", "/")));
    }

    // Calculate relative path from repo root to source file
    let rel_source = abs_source.strip_prefix(&repo_root).unwrap_or(&abs_source);

    // Build output path: <repo_root>/.local/<rel_path>.ts
    let mut output = repo_root.join(".local");
    if let Some(parent) = rel_source.parent() {
        output = output.join(parent);
    }
    if let Some(file_name) = rel_source.file_name().and_then(|name| name.to_str()) {
        let stem = file_name
            .strip_suffix(".lib.sigil")
            .or_else(|| file_name.strip_suffix(".sigil"))
            .unwrap_or(file_name);
        output = output.join(format!("{}.ts", stem));
    }

    output
}
