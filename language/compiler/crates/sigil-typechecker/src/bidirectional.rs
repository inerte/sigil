//! Bidirectional Type Checking for Sigil
//!
//! Uses two complementary modes:
//! - Synthesis (⇒): Infer type from expression structure (bottom-up)
//! - Checking (⇐): Verify expression matches expected type (top-down)
//!
//! This is simpler than Hindley-Milner because Sigil requires mandatory
//! type annotations everywhere, making the inference burden much lighter.

use crate::environment::{BindingMeta, TypeEnvironment, TypeInfo};
use crate::errors::{format_type, TypeError};
use crate::types::{ast_type_to_inference_type, types_equal, InferenceType, TConstructor, TFunction, TPrimitive, TRecord};
use crate::TypeCheckOptions;
use sigil_ast::{
    BinaryOperator, Declaration, Expr, FunctionDecl, LiteralType, PrimitiveName, Program, Type,
    TypeDef,
};
use std::collections::HashMap;

/// Type check a Sigil program
///
/// Returns a map of function names to their inferred types
pub fn type_check(
    program: &Program,
    _source_code: &str,
    options: TypeCheckOptions,
) -> Result<HashMap<String, InferenceType>, TypeError> {
    validate_surface_types(program)?;

    let mut env = TypeEnvironment::create_initial();
    let mut types = HashMap::new();

    // Register imported type registries
    if let Some(imported_type_registries) = options.imported_type_registries {
        for (module_id, type_registry) in imported_type_registries {
            env.register_imported_types(module_id, type_registry);
        }
    }

    // First pass: Register all function signatures and types
    for decl in &program.declarations {
        match decl {
            Declaration::Type(type_decl) => {
                // Register the type in the type registry
                env.register_type(
                    type_decl.name.clone(),
                    TypeInfo {
                        type_params: type_decl.type_params.clone(),
                        definition: type_decl.definition.clone(),
                    },
                );

                // Register constructor functions for sum types
                if let sigil_ast::TypeDef::Sum(sum_type) = &type_decl.definition {
                    for variant in &sum_type.variants {
                        let constructor_type = create_constructor_type(
                            &env,
                            variant,
                            &type_decl.type_params,
                            &type_decl.name,
                        )?;
                        env.bind(variant.name.clone(), constructor_type);
                    }
                }
            }

            Declaration::Function(func_decl) => {
                // Extract function type from signature
                let params: Vec<InferenceType> = func_decl
                    .params
                    .iter()
                    .map(|p| match &p.type_annotation {
                        Some(ty) => ast_type_to_inference_type_resolved(&env, ty),
                        None => Ok(InferenceType::Any),
                    })
                    .collect::<Result<_, _>>()?;

                let return_type = func_decl
                    .return_type
                    .as_ref()
                    .map(|ty| ast_type_to_inference_type_resolved(&env, ty))
                    .transpose()?
                    .unwrap_or(InferenceType::Any);

                let effects = if func_decl.effects.is_empty() {
                    None
                } else {
                    Some(func_decl.effects.iter().cloned().collect())
                };

                let func_type = InferenceType::Function(Box::new(TFunction {
                    params,
                    return_type,
                    effects,
                }));

                if func_decl.is_mockable {
                    env.bind_with_meta(
                        func_decl.name.clone(),
                        func_type.clone(),
                        BindingMeta {
                            is_mockable_function: true,
                            is_extern_namespace: false,
                        },
                    );
                } else {
                    env.bind(func_decl.name.clone(), func_type.clone());
                }

                types.insert(func_decl.name.clone(), func_type);
            }

            Declaration::Const(const_decl) => {
                // Register constant type
                let const_type = const_decl
                    .type_annotation
                    .as_ref()
                    .map(|ty| ast_type_to_inference_type_resolved(&env, ty))
                    .transpose()?
                    .unwrap_or(InferenceType::Any);

                env.bind(const_decl.name.clone(), const_type.clone());
                types.insert(const_decl.name.clone(), const_type);
            }

            Declaration::Extern(extern_decl) => {
                let namespace_name = extern_decl.module_path.join("⋅");

                if let Some(_members) = &extern_decl.members {
                    // TODO: Create record type with typed members
                    env.bind_with_meta(
                        namespace_name,
                        InferenceType::Any,
                        BindingMeta {
                            is_mockable_function: false,
                            is_extern_namespace: true,
                        },
                    );
                } else {
                    // Untyped extern: trust mode
                    env.bind_with_meta(
                        namespace_name,
                        InferenceType::Any,
                        BindingMeta {
                            is_mockable_function: false,
                            is_extern_namespace: true,
                        },
                    );
                }
            }

            Declaration::Import(import_decl) => {
                let namespace_name = import_decl.module_path.join("⋅");
                let imported_type = options
                    .imported_namespaces
                    .as_ref()
                    .and_then(|ns| ns.get(&namespace_name))
                    .cloned()
                    .unwrap_or(InferenceType::Any);

                env.bind(namespace_name, imported_type);
            }

            Declaration::Test(_) => {
                // TODO: Check test declarations
            }
        }
    }

    // Second pass: Type check function bodies
    for decl in &program.declarations {
        if let Declaration::Function(func_decl) = decl {
            check_function_decl(&env, func_decl)?;
        } else if let Declaration::Const(const_decl) = decl {
            // Type check constant value
            let value_type = synthesize(&env, &const_decl.value)?;
            if let Some(ref annotation) = const_decl.type_annotation {
                let expected_type = ast_type_to_inference_type_resolved(&env, annotation)?;
                let (normalized_value, normalized_expected) =
                    canonical_pair(&env, &value_type, &expected_type);
                if !types_equal(&normalized_value, &normalized_expected) {
                    return Err(TypeError::mismatch(
                        format!(
                            "Constant '{}' type mismatch",
                            const_decl.name
                        ),
                        Some(const_decl.location),
                        normalized_expected,
                        normalized_value,
                    ));
                }
            }
        }
    }

    Ok(types)
}

/// Canonicalize two types before any structural equality-sensitive comparison.
///
/// Sigil treats aliases and named product types as structural everywhere in the
/// checker, so comparisons must happen on normalized forms rather than raw
/// synthesized forms.
fn canonical_pair(
    env: &TypeEnvironment,
    left: &InferenceType,
    right: &InferenceType,
) -> (InferenceType, InferenceType) {
    (env.normalize_type(left), env.normalize_type(right))
}

fn resolve_qualified_type(
    env: &TypeEnvironment,
    qualified: &sigil_ast::QualifiedType,
) -> Result<InferenceType, TypeError> {
    let module_id = qualified.module_path.join("⋅");
    let type_info = env.lookup_qualified_type(&qualified.module_path, &qualified.type_name);

    let Some(type_info) = type_info else {
        if let Some(available_types) = env.get_imported_module_type_names(&module_id) {
            if !available_types.is_empty() {
                return Err(TypeError::new(
                    format!(
                        "Undefined type: {}.{}\n\nModule '{}' is imported, but it does not export a type named '{}'.\n\nAvailable exported types:\n{}",
                        module_id,
                        qualified.type_name,
                        module_id,
                        qualified.type_name,
                        available_types
                            .iter()
                            .map(|name| format!("  - {}", name))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                    Some(qualified.location),
                ));
            }
        }

        return Err(TypeError::new(
            format!(
                "Module '{}' is not imported or does not export any types.\n\nAdd this import: i {}",
                module_id, module_id
            ),
            Some(qualified.location),
        ));
    };

    let type_args = qualified
        .type_args
        .iter()
        .map(|arg| ast_type_to_inference_type_resolved(env, arg))
        .collect::<Result<Vec<_>, _>>()?;

    if type_args.len() != type_info.type_params.len() {
        return Err(TypeError::new(
            format!(
                "Type argument mismatch: {} expects {} type argument{}, but got {}",
                qualified.type_name,
                type_info.type_params.len(),
                if type_info.type_params.len() == 1 { "" } else { "s" },
                type_args.len()
            ),
            Some(qualified.location),
        ));
    }

    let qualified_name = format!("{}.{}", module_id, qualified.type_name);
    if type_info.type_params.is_empty() {
        match &type_info.definition {
            TypeDef::Product(product) => {
                let mut fields = HashMap::new();
                for field in &product.fields {
                    fields.insert(
                        field.name.clone(),
                        ast_type_to_inference_type_resolved(env, &field.field_type)?,
                    );
                }

                return Ok(InferenceType::Record(TRecord {
                    fields,
                    name: Some(qualified_name),
                }));
            }
            TypeDef::Alias(alias) => {
                return ast_type_to_inference_type_resolved(env, &alias.aliased_type);
            }
            TypeDef::Sum(_) => {}
        }
    }

    Ok(InferenceType::Constructor(TConstructor {
        name: qualified_name,
        type_args,
    }))
}

fn split_qualified_constructor_name(name: &str) -> Option<(Vec<String>, String)> {
    let dot_index = name.rfind('.')?;
    let module_id = &name[..dot_index];
    let type_name = &name[dot_index + 1..];
    Some((
        module_id.split('⋅').map(|part| part.to_string()).collect(),
        type_name.to_string(),
    ))
}

fn resolve_named_type(
    env: &TypeEnvironment,
    inference_type: &InferenceType,
) -> Result<InferenceType, TypeError> {
    match inference_type {
        InferenceType::Constructor(constructor) if constructor.type_args.is_empty() => {
            if let Some((module_path, type_name)) = split_qualified_constructor_name(&constructor.name) {
                if let Some(type_info) = env.lookup_qualified_type(&module_path, &type_name) {
                    if type_info.type_params.is_empty() {
                        return match &type_info.definition {
                            TypeDef::Product(product) => {
                                let mut fields = HashMap::new();
                                for field in &product.fields {
                                    fields.insert(
                                        field.name.clone(),
                                        ast_type_to_inference_type_resolved(env, &field.field_type)?,
                                    );
                                }
                                Ok(InferenceType::Record(TRecord {
                                    fields,
                                    name: Some(constructor.name.clone()),
                                }))
                            }
                            TypeDef::Alias(alias) => {
                                ast_type_to_inference_type_resolved(env, &alias.aliased_type)
                            }
                            TypeDef::Sum(_) => Ok(inference_type.clone()),
                        };
                    }
                }
            }

            if let Some(type_info) = env.lookup_type(&constructor.name) {
                if type_info.type_params.is_empty() {
                    return match &type_info.definition {
                        TypeDef::Product(product) => {
                            let mut fields = HashMap::new();
                            for field in &product.fields {
                                fields.insert(
                                    field.name.clone(),
                                    ast_type_to_inference_type_resolved(env, &field.field_type)?,
                                );
                            }
                            Ok(InferenceType::Record(TRecord {
                                fields,
                                name: Some(constructor.name.clone()),
                            }))
                        }
                        TypeDef::Alias(alias) => {
                            ast_type_to_inference_type_resolved(env, &alias.aliased_type)
                        }
                        TypeDef::Sum(_) => Ok(inference_type.clone()),
                    };
                }
            }

            Ok(inference_type.clone())
        }
        InferenceType::List(list) => Ok(InferenceType::List(Box::new(crate::types::TList {
            element_type: resolve_named_type(env, &list.element_type)?,
        }))),
        InferenceType::Tuple(tuple) => Ok(InferenceType::Tuple(crate::types::TTuple {
            types: tuple
                .types
                .iter()
                .map(|ty| resolve_named_type(env, ty))
                .collect::<Result<Vec<_>, _>>()?,
        })),
        InferenceType::Function(func) => Ok(InferenceType::Function(Box::new(TFunction {
            params: func
                .params
                .iter()
                .map(|ty| resolve_named_type(env, ty))
                .collect::<Result<Vec<_>, _>>()?,
            return_type: resolve_named_type(env, &func.return_type)?,
            effects: func.effects.clone(),
        }))),
        InferenceType::Record(record) => {
            let mut fields = HashMap::new();
            for (name, field_type) in &record.fields {
                fields.insert(name.clone(), resolve_named_type(env, field_type)?);
            }
            Ok(InferenceType::Record(TRecord {
                fields,
                name: record.name.clone(),
            }))
        }
        _ => Ok(inference_type.clone()),
    }
}

fn ast_type_to_inference_type_resolved(
    env: &TypeEnvironment,
    ast_type: &Type,
) -> Result<InferenceType, TypeError> {
    match ast_type {
        Type::Qualified(qualified) => resolve_qualified_type(env, qualified),
        Type::List(list_type) => Ok(InferenceType::List(Box::new(crate::types::TList {
            element_type: ast_type_to_inference_type_resolved(env, &list_type.element_type)?,
        }))),
        Type::Tuple(tuple_type) => Ok(InferenceType::Tuple(crate::types::TTuple {
            types: tuple_type
                .types
                .iter()
                .map(|ty| ast_type_to_inference_type_resolved(env, ty))
                .collect::<Result<Vec<_>, _>>()?,
        })),
        Type::Function(func_type) => Ok(InferenceType::Function(Box::new(TFunction {
            params: func_type
                .param_types
                .iter()
                .map(|ty| ast_type_to_inference_type_resolved(env, ty))
                .collect::<Result<Vec<_>, _>>()?,
            return_type: ast_type_to_inference_type_resolved(env, &func_type.return_type)?,
            effects: if func_type.effects.is_empty() {
                None
            } else {
                Some(func_type.effects.iter().cloned().collect())
            },
        }))),
        _ => resolve_named_type(env, &ast_type_to_inference_type(ast_type)),
    }
}

fn validate_surface_types(program: &Program) -> Result<(), TypeError> {
    for decl in &program.declarations {
        validate_declaration_surface_types(decl)?;
    }

    Ok(())
}

fn validate_declaration_surface_types(decl: &Declaration) -> Result<(), TypeError> {
    match decl {
        Declaration::Type(type_decl) => match &type_decl.definition {
            TypeDef::Alias(alias) => validate_surface_type(&alias.aliased_type),
            TypeDef::Product(product) => {
                for field in &product.fields {
                    validate_surface_type(&field.field_type)?;
                }
                Ok(())
            }
            TypeDef::Sum(sum) => {
                for variant in &sum.variants {
                    for field_type in &variant.types {
                        validate_surface_type(field_type)?;
                    }
                }
                Ok(())
            }
        },
        Declaration::Function(func_decl) => {
            for param in &func_decl.params {
                if let Some(param_type) = &param.type_annotation {
                    validate_surface_type(param_type)?;
                }
            }

            if let Some(return_type) = &func_decl.return_type {
                validate_surface_type(return_type)?;
            }

            validate_expr_surface_types(&func_decl.body)
        }
        Declaration::Const(const_decl) => {
            if let Some(annotation) = &const_decl.type_annotation {
                validate_surface_type(annotation)?;
            }
            validate_expr_surface_types(&const_decl.value)
        }
        Declaration::Extern(extern_decl) => {
            if let Some(members) = &extern_decl.members {
                for member in members {
                    validate_surface_type(&member.member_type)?;
                }
            }
            Ok(())
        }
        Declaration::Test(test_decl) => validate_expr_surface_types(&test_decl.body),
        Declaration::Import(_) => Ok(()),
    }
}

fn validate_expr_surface_types(expr: &Expr) -> Result<(), TypeError> {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) => Ok(()),
        Expr::Lambda(lambda) => {
            for param in &lambda.params {
                if let Some(param_type) = &param.type_annotation {
                    validate_surface_type(param_type)?;
                }
            }
            validate_surface_type(&lambda.return_type)?;
            validate_expr_surface_types(&lambda.body)
        }
        Expr::Application(app) => {
            validate_expr_surface_types(&app.func)?;
            for arg in &app.args {
                validate_expr_surface_types(arg)?;
            }
            Ok(())
        }
        Expr::Binary(bin) => {
            validate_expr_surface_types(&bin.left)?;
            validate_expr_surface_types(&bin.right)
        }
        Expr::Unary(un) => validate_expr_surface_types(&un.operand),
        Expr::Match(match_expr) => {
            validate_expr_surface_types(&match_expr.scrutinee)?;
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    validate_expr_surface_types(guard)?;
                }
                validate_expr_surface_types(&arm.body)?;
            }
            Ok(())
        }
        Expr::Let(let_expr) => {
            validate_expr_surface_types(&let_expr.value)?;
            validate_expr_surface_types(&let_expr.body)
        }
        Expr::If(if_expr) => {
            validate_expr_surface_types(&if_expr.condition)?;
            validate_expr_surface_types(&if_expr.then_branch)?;
            if let Some(else_branch) = &if_expr.else_branch {
                validate_expr_surface_types(else_branch)?;
            }
            Ok(())
        }
        Expr::List(list) => {
            for elem in &list.elements {
                validate_expr_surface_types(elem)?;
            }
            Ok(())
        }
        Expr::Record(record) => {
            for field in &record.fields {
                validate_expr_surface_types(&field.value)?;
            }
            Ok(())
        }
        Expr::Tuple(tuple) => {
            for elem in &tuple.elements {
                validate_expr_surface_types(elem)?;
            }
            Ok(())
        }
        Expr::FieldAccess(field_access) => validate_expr_surface_types(&field_access.object),
        Expr::Index(index_expr) => {
            validate_expr_surface_types(&index_expr.object)?;
            validate_expr_surface_types(&index_expr.index)
        }
        Expr::Pipeline(pipeline) => {
            validate_expr_surface_types(&pipeline.left)?;
            validate_expr_surface_types(&pipeline.right)
        }
        Expr::Map(map_expr) => {
            validate_expr_surface_types(&map_expr.list)?;
            validate_expr_surface_types(&map_expr.func)
        }
        Expr::Filter(filter_expr) => {
            validate_expr_surface_types(&filter_expr.list)?;
            validate_expr_surface_types(&filter_expr.predicate)
        }
        Expr::Fold(fold_expr) => {
            validate_expr_surface_types(&fold_expr.list)?;
            validate_expr_surface_types(&fold_expr.func)?;
            validate_expr_surface_types(&fold_expr.init)
        }
        Expr::MemberAccess(_) => Ok(()),
        Expr::WithMock(with_mock) => {
            validate_expr_surface_types(&with_mock.target)?;
            validate_expr_surface_types(&with_mock.replacement)?;
            validate_expr_surface_types(&with_mock.body)
        }
        Expr::TypeAscription(type_asc) => {
            validate_expr_surface_types(&type_asc.expr)?;
            validate_surface_type(&type_asc.ascribed_type)
        }
    }
}

fn validate_surface_type(ty: &Type) -> Result<(), TypeError> {
    match ty {
        Type::Primitive(_) => Ok(()),
        Type::List(list) => validate_surface_type(&list.element_type),
        Type::Map(map) => {
            validate_surface_type(&map.key_type)?;
            validate_surface_type(&map.value_type)
        }
        Type::Function(func) => {
            for param_type in &func.param_types {
                validate_surface_type(param_type)?;
            }
            validate_surface_type(&func.return_type)
        }
        Type::Constructor(ctor) => {
            for type_arg in &ctor.type_args {
                validate_surface_type(type_arg)?;
            }
            Ok(())
        }
        Type::Variable(var) => {
            if var.name == "Any" {
                Err(TypeError::new(
                    "The 'Any' type is reserved for untyped FFI trust mode. Use a concrete Sigil type, or use an untyped extern declaration instead.".to_string(),
                    Some(var.location),
                ))
            } else {
                Ok(())
            }
        }
        Type::Tuple(tuple) => {
            for elem in &tuple.types {
                validate_surface_type(elem)?;
            }
            Ok(())
        }
        Type::Qualified(qualified) => {
            for type_arg in &qualified.type_args {
                validate_surface_type(type_arg)?;
            }
            Ok(())
        }
    }
}

/// Create a constructor function type for a sum type variant
///
/// For example, Some : T → Option[T]
fn create_constructor_type(
    env: &TypeEnvironment,
    variant: &sigil_ast::Variant,
    type_params: &[String],
    type_name: &str,
) -> Result<InferenceType, TypeError> {
    // Convert variant field types to inference types
    let params: Vec<InferenceType> = variant
        .types
        .iter()
        .map(|field_type| {
            // True type parameters become Any for now; named types should stay named.
            if let sigil_ast::Type::Variable(var_type) = field_type {
                if type_params.contains(&var_type.name) {
                    return Ok(InferenceType::Any);
                }
            }
            ast_type_to_inference_type_resolved(env, field_type)
        })
        .collect::<Result<_, _>>()?;

    // Result type is the constructor with empty type args for now
    let result_type = InferenceType::Constructor(crate::types::TConstructor {
        name: type_name.to_string(),
        type_args: vec![],
    });

    Ok(InferenceType::Function(Box::new(TFunction {
        params,
        return_type: result_type,
        effects: None,
    })))
}

/// Type check a function declaration
fn check_function_decl(env: &TypeEnvironment, func_decl: &FunctionDecl) -> Result<(), TypeError> {
    // Create environment with parameter bindings
    let mut func_env = env.extend(None);

    for param in &func_decl.params {
        let param_type = param
            .type_annotation
            .as_ref()
            .map(|ty| ast_type_to_inference_type_resolved(env, ty))
            .transpose()?
            .unwrap_or(InferenceType::Any);
        func_env.bind(param.name.clone(), param_type);
    }

    // Get expected return type
    let expected_return_type = func_decl
        .return_type
        .as_ref()
        .map(|ty| ast_type_to_inference_type_resolved(env, ty))
        .transpose()?
        .unwrap_or(InferenceType::Any);

    // Type check body
    check(&func_env, &func_decl.body, &expected_return_type)?;

    Ok(())
}

// ============================================================================
// SYNTHESIS (⇒) - Infer type from expression
// ============================================================================

/// Synthesize (infer) type from expression
/// Returns the inferred type
fn synthesize(env: &TypeEnvironment, expr: &Expr) -> Result<InferenceType, TypeError> {
    match expr {
        Expr::Literal(lit) => Ok(synthesize_literal(lit)),

        Expr::Identifier(id) => {
            env.lookup(&id.name).ok_or_else(|| {
                TypeError::new(
                    format!("Unbound variable: {}", id.name),
                    Some(id.location),
                )
            })
        }

        Expr::Binary(bin) => synthesize_binary(env, bin),

        Expr::Unary(un) => synthesize_unary(env, un),

        Expr::Application(app) => synthesize_application(env, app),

        Expr::List(list) => synthesize_list(env, list),

        Expr::If(if_expr) => synthesize_if(env, if_expr),

        Expr::Let(let_expr) => synthesize_let(env, let_expr),

        Expr::Match(match_expr) => synthesize_match(env, match_expr),

        Expr::Lambda(lambda_expr) => synthesize_lambda(env, lambda_expr),

        Expr::Tuple(tuple_expr) => synthesize_tuple(env, tuple_expr),

        Expr::Record(record_expr) => synthesize_record(env, record_expr),

        Expr::FieldAccess(field_access) => synthesize_field_access(env, field_access),

        Expr::Index(index_expr) => synthesize_index(env, index_expr),

        Expr::MemberAccess(member_access) => synthesize_member_access(env, member_access),

        Expr::Map(map_expr) => synthesize_map(env, map_expr),

        Expr::Filter(filter_expr) => synthesize_filter(env, filter_expr),

        Expr::Fold(fold_expr) => synthesize_fold(env, fold_expr),

        Expr::WithMock(with_mock) => synthesize_with_mock(env, with_mock),

        Expr::Pipeline(pipeline) => synthesize_pipeline(env, pipeline),

        Expr::TypeAscription(type_asc) => synthesize_type_ascription(env, type_asc),
    }
}

fn synthesize_literal(lit: &sigil_ast::LiteralExpr) -> InferenceType {
    let prim_name = match lit.literal_type {
        LiteralType::Int => PrimitiveName::Int,
        LiteralType::Float => PrimitiveName::Float,
        LiteralType::String => PrimitiveName::String,
        LiteralType::Char => PrimitiveName::Char,
        LiteralType::Unit => PrimitiveName::Unit,
        LiteralType::Bool => PrimitiveName::Bool,
    };

    InferenceType::Primitive(TPrimitive { name: prim_name })
}

fn synthesize_binary(
    env: &TypeEnvironment,
    bin: &sigil_ast::BinaryExpr,
) -> Result<InferenceType, TypeError> {
    let left_type = synthesize(env, &bin.left)?;
    let right_type = synthesize(env, &bin.right)?;

    let int_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Int,
    });
    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });
    let string_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::String,
    });

    match bin.operator {
        // Arithmetic operators: ℤ → ℤ → ℤ
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Modulo => {
            // Special case: + with string operands does concatenation
            if bin.operator == BinaryOperator::Add
                && (matches!(left_type, InferenceType::Primitive(ref p) if p.name == PrimitiveName::String)
                    || matches!(right_type, InferenceType::Primitive(ref p) if p.name == PrimitiveName::String))
            {
                return Ok(string_type);
            }

            // Otherwise require both operands to be integers
            check(env, &bin.left, &int_type)?;
            check(env, &bin.right, &int_type)?;
            Ok(int_type)
        }

        // Comparison operators: ℤ → ℤ → 𝔹
        BinaryOperator::Less
        | BinaryOperator::Greater
        | BinaryOperator::LessEq
        | BinaryOperator::GreaterEq => {
            check(env, &bin.left, &int_type)?;
            check(env, &bin.right, &int_type)?;
            Ok(bool_type)
        }

        // Equality operators: T → T → 𝔹 (polymorphic)
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let (normalized_left, normalized_right) = canonical_pair(env, &left_type, &right_type);
            if !types_equal(&normalized_left, &normalized_right) {
                return Err(TypeError::new(
                    format!(
                        "Cannot compare {} with {}",
                        format_type(&normalized_left),
                        format_type(&normalized_right)
                    ),
                    Some(bin.location),
                ));
            }
            Ok(bool_type)
        }

        // Logical operators: 𝔹 → 𝔹 → 𝔹
        BinaryOperator::And | BinaryOperator::Or => {
            check(env, &bin.left, &bool_type)?;
            check(env, &bin.right, &bool_type)?;
            Ok(bool_type)
        }

        // String concatenation: 𝕊 → 𝕊 → 𝕊
        BinaryOperator::Append => {
            check(env, &bin.left, &string_type)?;
            check(env, &bin.right, &string_type)?;
            Ok(string_type)
        }

        // List append: [T] → [T] → [T]
        BinaryOperator::ListAppend => {
            let (normalized_left, normalized_right) = canonical_pair(env, &left_type, &right_type);

            if !matches!(normalized_left, InferenceType::List(_)) {
                return Err(TypeError::new(
                    format!(
                        "List append requires list operands, got {}",
                        format_type(&normalized_left)
                    ),
                    Some(bin.location),
                ));
            }
            if !types_equal(&normalized_left, &normalized_right) {
                return Err(TypeError::new(
                    format!(
                        "Cannot concatenate lists of different types: {} and {}",
                        format_type(&normalized_left),
                        format_type(&normalized_right)
                    ),
                    Some(bin.location),
                ));
            }
            Ok(normalized_left)
        }

        _ => Err(TypeError::new(
            format!("Operator {:?} not yet implemented", bin.operator),
            Some(bin.location),
        )),
    }
}

fn synthesize_unary(
    env: &TypeEnvironment,
    un: &sigil_ast::UnaryExpr,
) -> Result<InferenceType, TypeError> {
    let int_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Int,
    });
    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });

    match un.operator {
        sigil_ast::UnaryOperator::Negate => {
            check(env, &un.operand, &int_type)?;
            Ok(int_type)
        }
        sigil_ast::UnaryOperator::Not => {
            check(env, &un.operand, &bool_type)?;
            Ok(bool_type)
        }
        sigil_ast::UnaryOperator::Length => {
            // Length operator # - works on strings and lists
            let operand_type = synthesize(env, &un.operand)?;
            match operand_type {
                InferenceType::Primitive(ref p) if p.name == PrimitiveName::String => Ok(int_type),
                InferenceType::List(_) => Ok(int_type),
                _ => Err(TypeError::new(
                    format!(
                        "Length operator # requires string or list, got {}",
                        format_type(&operand_type)
                    ),
                    Some(un.location),
                )),
            }
        }
    }
}

fn synthesize_application(
    env: &TypeEnvironment,
    app: &sigil_ast::ApplicationExpr,
) -> Result<InferenceType, TypeError> {
    let fn_type = synthesize(env, &app.func)?;

    // Special case: applying 'any' type (FFI function call)
    if matches!(fn_type, InferenceType::Any) {
        return Ok(InferenceType::Any);
    }

    match fn_type {
        InferenceType::Function(ref tfunc) => {
            // Check argument count
            if app.args.len() != tfunc.params.len() {
                return Err(TypeError::new(
                    format!(
                        "Function expects {} arguments, got {}",
                        tfunc.params.len(),
                        app.args.len()
                    ),
                    Some(app.location),
                ));
            }

            // Check each argument against parameter type
            for (arg, param_type) in app.args.iter().zip(&tfunc.params) {
                check(env, arg, param_type)?;
            }

            Ok(tfunc.return_type.clone())
        }
        _ => Err(TypeError::new(
            format!("Expected function type, got {}", format_type(&fn_type)),
            Some(app.location),
        )),
    }
}

fn synthesize_list(
    env: &TypeEnvironment,
    list: &sigil_ast::ListExpr,
) -> Result<InferenceType, TypeError> {
    if list.elements.is_empty() {
        // Empty list - cannot infer element type
        // In checked position this would be ok, but in synthesis we need a type
        // Return a list of Any for now
        return Ok(InferenceType::List(Box::new(crate::types::TList {
            element_type: InferenceType::Any,
        })));
    }

    // Infer type from first element
    let first_type = synthesize(env, &list.elements[0])?;

    // Check remaining elements match
    for elem in &list.elements[1..] {
        check(env, elem, &first_type)?;
    }

    Ok(InferenceType::List(Box::new(crate::types::TList {
        element_type: first_type,
    })))
}

fn synthesize_if(
    env: &TypeEnvironment,
    if_expr: &sigil_ast::IfExpr,
) -> Result<InferenceType, TypeError> {
    // Check condition is boolean
    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });
    check(env, &if_expr.condition, &bool_type)?;

    // Synthesize then branch
    let then_type = synthesize(env, &if_expr.then_branch)?;

    // If no else branch, then branch must be Unit
    if if_expr.else_branch.is_none() {
        let unit_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Unit,
        });
        let (normalized_then, normalized_unit) = canonical_pair(env, &then_type, &unit_type);
        if !types_equal(&normalized_then, &normalized_unit) {
            return Err(TypeError::new(
                format!(
                    "If expression without else must have Unit type, got {}",
                    format_type(&normalized_then)
                ),
                Some(if_expr.location),
            ));
        }
        return Ok(then_type);
    }

    // Synthesize else branch
    let else_type = synthesize(env, if_expr.else_branch.as_ref().unwrap())?;

    // Both branches must have same type
    let (normalized_then, normalized_else) = canonical_pair(env, &then_type, &else_type);
    if !types_equal(&normalized_then, &normalized_else) {
        return Err(TypeError::new(
            format!(
                "If branches have different types: then is {}, else is {}",
                format_type(&normalized_then),
                format_type(&normalized_else)
            ),
            Some(if_expr.location),
        ));
    }

    Ok(then_type)
}

fn synthesize_let(
    env: &TypeEnvironment,
    let_expr: &sigil_ast::LetExpr,
) -> Result<InferenceType, TypeError> {
    use sigil_ast::Pattern;

    // Synthesize binding value type
    let value_type = synthesize(env, &let_expr.value)?;

    // Check pattern and get bindings
    // For now, support identifier and wildcard patterns
    // TODO: Full pattern matching support (tuples, records, etc.)
    let mut bindings = HashMap::new();
    match &let_expr.pattern {
        Pattern::Identifier(id_pattern) => {
            bindings.insert(id_pattern.name.clone(), value_type);
        }
        Pattern::Wildcard(_) => {
            // Wildcard pattern: discard the value, no bindings
            // This is valid and commonly used for effectful expressions
        }
        _ => {
            return Err(TypeError::new(
                "Let expression pattern matching not yet fully implemented".to_string(),
                Some(let_expr.location),
            ));
        }
    }

    // Extend environment and synthesize body
    let body_env = env.extend(Some(bindings));
    synthesize(&body_env, &let_expr.body)
}

fn synthesize_match(
    env: &TypeEnvironment,
    match_expr: &sigil_ast::MatchExpr,
) -> Result<InferenceType, TypeError> {
    // Synthesize scrutinee type
    let scrutinee_type = synthesize(env, &match_expr.scrutinee)?;

    if match_expr.arms.is_empty() {
        return Err(TypeError::new(
            "Match expression must have at least one arm".to_string(),
            Some(match_expr.location),
        ));
    }

    // Synthesize first arm to establish expected type
    let first_arm = &match_expr.arms[0];
    let mut first_bindings = HashMap::new();
    check_pattern(env, &first_arm.pattern, &scrutinee_type, &mut first_bindings)?;
    let first_arm_env = env.extend(Some(first_bindings));

    // Check guard if present (must be boolean)
    if let Some(ref guard) = first_arm.guard {
        let guard_type = synthesize(&first_arm_env, guard)?;
        let bool_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool,
        });
        let (normalized_guard, normalized_bool) = canonical_pair(env, &guard_type, &bool_type);
        if !types_equal(&normalized_guard, &normalized_bool) {
            return Err(TypeError::new(
                format!(
                    "Pattern guard must have type 𝔹, got {}",
                    format_type(&normalized_guard)
                ),
                Some(match_expr.location),
            ));
        }
    }

    // Synthesize first arm body to get expected type
    let expected_type = synthesize(&first_arm_env, &first_arm.body)?;

    // Check remaining arms against the first arm's type
    for arm in &match_expr.arms[1..] {
        let mut bindings = HashMap::new();
        check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)?;
        let arm_env = env.extend(Some(bindings));

        // Check guard if present (must be boolean)
        if let Some(ref guard) = arm.guard {
            let guard_type = synthesize(&arm_env, guard)?;
            let bool_type = InferenceType::Primitive(TPrimitive {
                name: PrimitiveName::Bool,
            });
            let (normalized_guard, normalized_bool) = canonical_pair(env, &guard_type, &bool_type);
            if !types_equal(&normalized_guard, &normalized_bool) {
                return Err(TypeError::new(
                    format!(
                        "Pattern guard must have type 𝔹, got {}",
                        format_type(&normalized_guard)
                    ),
                    Some(match_expr.location),
                ));
            }
        }

        // Check subsequent arms against first arm's type
        check(&arm_env, &arm.body, &expected_type)?;
    }

    Ok(expected_type)
}

fn synthesize_tuple(
    env: &TypeEnvironment,
    tuple_expr: &sigil_ast::TupleExpr,
) -> Result<InferenceType, TypeError> {
    let types: Result<Vec<_>, _> = tuple_expr
        .elements
        .iter()
        .map(|elem| synthesize(env, elem))
        .collect();

    Ok(InferenceType::Tuple(crate::types::TTuple { types: types? }))
}

fn synthesize_record(
    env: &TypeEnvironment,
    record_expr: &sigil_ast::RecordExpr,
) -> Result<InferenceType, TypeError> {
    let mut fields = HashMap::new();
    for field in &record_expr.fields {
        let field_type = synthesize(env, &field.value)?;
        fields.insert(field.name.clone(), field_type);
    }

    Ok(InferenceType::Record(crate::types::TRecord {
        fields,
        name: None, // Anonymous record
    }))
}

fn synthesize_field_access(
    env: &TypeEnvironment,
    field_access: &sigil_ast::FieldAccessExpr,
) -> Result<InferenceType, TypeError> {
    let obj_type = synthesize(env, &field_access.object)?;

    // Special case: field access on 'any' type (FFI namespace)
    if matches!(obj_type, InferenceType::Any) {
        return Ok(InferenceType::Any);
    }

    // Normalize the type to resolve type aliases (e.g., EmailParts -> {local:𝕊,domain:𝕊})
    let normalized_type = env.normalize_type(&obj_type);

    // Must be a record type
    match normalized_type {
        InferenceType::Record(ref record) => {
            if let Some(field_type) = record.fields.get(&field_access.field) {
                Ok(field_type.clone())
            } else {
                Err(TypeError::new(
                    format!(
                        "Record type {} does not have field '{}'",
                        format_type(&normalized_type),
                        field_access.field
                    ),
                    Some(field_access.location),
                ))
            }
        }
        _ => Err(TypeError::new(
            format!(
                "Field access requires record type, got {} (normalized from {})",
                format_type(&normalized_type),
                format_type(&obj_type)
            ),
            Some(field_access.location),
        )),
    }
}

fn synthesize_index(
    env: &TypeEnvironment,
    index_expr: &sigil_ast::IndexExpr,
) -> Result<InferenceType, TypeError> {
    let obj_type = synthesize(env, &index_expr.object)?;
    let int_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Int,
    });
    check(env, &index_expr.index, &int_type)?;

    // Special case: 'any' type
    if matches!(obj_type, InferenceType::Any) {
        return Ok(InferenceType::Any);
    }

    match obj_type {
        InferenceType::List(ref list) => Ok(list.element_type.clone()),
        InferenceType::Tuple(_) => {
            // Index is dynamic at compile time; return Any for now
            Ok(InferenceType::Any)
        }
        _ => Err(TypeError::new(
            format!(
                "Cannot index into non-list type {}",
                format_type(&obj_type)
            ),
            Some(index_expr.location),
        )),
    }
}

fn synthesize_member_access(
    env: &TypeEnvironment,
    member_access: &sigil_ast::MemberAccessExpr,
) -> Result<InferenceType, TypeError> {
    let namespace_name = member_access.namespace.join("⋅");

    // Check namespace exists (should be registered from extern/import declaration)
    let namespace_type = env.lookup(&namespace_name);
    if namespace_type.is_none() {
        return Err(TypeError::new(
            format!(
                "Unknown namespace '{}'. Did you forget 'e {}' or 'i {}'?",
                namespace_name, namespace_name, namespace_name
            ),
            Some(member_access.location),
        ));
    }

    let namespace_type = namespace_type.unwrap();

    // If namespace is a record (typed extern/import), check member exists
    if let InferenceType::Record(ref record) = namespace_type {
        if let Some(member_type) = record.fields.get(&member_access.member) {
            return Ok(member_type.clone());
        } else {
            return Err(TypeError::new(
                format!(
                    "Module '{}' does not export member '{}'",
                    namespace_name, member_access.member
                ),
                Some(member_access.location),
            ));
        }
    }

    // Return Any type for extern/trust-mode member access
    // Actual validation happens at link-time
    Ok(InferenceType::Any)
}

fn synthesize_map(
    env: &TypeEnvironment,
    map_expr: &sigil_ast::MapExpr,
) -> Result<InferenceType, TypeError> {
    let list_type = synthesize(env, &map_expr.list)?;

    if !matches!(list_type, InferenceType::List(_)) {
        return Err(TypeError::new(
            format!("Map (↦) requires a list, got {}", format_type(&list_type)),
            Some(map_expr.location),
        ));
    }

    let fn_type = synthesize(env, &map_expr.func)?;

    if !matches!(fn_type, InferenceType::Function(_)) {
        return Err(TypeError::new(
            format!("Map (↦) requires a function, got {}", format_type(&fn_type)),
            Some(map_expr.location),
        ));
    }

    if let (InferenceType::List(ref list), InferenceType::Function(ref func)) = (&list_type, &fn_type) {
        // Function should take 1 parameter
        if func.params.len() != 1 {
            return Err(TypeError::new(
                format!("Map (↦) function should take 1 parameter, got {}", func.params.len()),
                Some(map_expr.location),
            ));
        }

        // Check function parameter matches list element type
        let (normalized_param, normalized_elem) = canonical_pair(env, &func.params[0], &list.element_type);
        if !types_equal(&normalized_param, &normalized_elem) {
            return Err(TypeError::new(
                format!(
                    "Map (↦) function parameter type {} doesn't match list element type {}",
                    format_type(&normalized_param),
                    format_type(&normalized_elem)
                ),
                Some(map_expr.location),
            ));
        }

        // Result is list of return type
        return Ok(InferenceType::List(Box::new(crate::types::TList {
            element_type: func.return_type.clone(),
        })));
    }

    unreachable!()
}

fn synthesize_filter(
    env: &TypeEnvironment,
    filter_expr: &sigil_ast::FilterExpr,
) -> Result<InferenceType, TypeError> {
    let list_type = synthesize(env, &filter_expr.list)?;

    if !matches!(list_type, InferenceType::List(_)) {
        return Err(TypeError::new(
            format!("Filter (⊳) requires a list, got {}", format_type(&list_type)),
            Some(filter_expr.location),
        ));
    }

    let predicate_type = synthesize(env, &filter_expr.predicate)?;

    if !matches!(predicate_type, InferenceType::Function(_)) {
        return Err(TypeError::new(
            format!("Filter (⊳) requires a predicate function, got {}", format_type(&predicate_type)),
            Some(filter_expr.location),
        ));
    }

    let bool_type = InferenceType::Primitive(TPrimitive { name: PrimitiveName::Bool });

    if let (InferenceType::List(ref list), InferenceType::Function(ref pred)) = (&list_type, &predicate_type) {
        // Predicate should be T → 𝔹
        if pred.params.len() != 1 {
            return Err(TypeError::new(
                format!("Filter (⊳) predicate should take 1 parameter, got {}", pred.params.len()),
                Some(filter_expr.location),
            ));
        }

        let (normalized_param, normalized_elem) = canonical_pair(env, &pred.params[0], &list.element_type);
        if !types_equal(&normalized_param, &normalized_elem) {
            return Err(TypeError::new(
                format!(
                    "Filter (⊳) predicate parameter type {} doesn't match list element type {}",
                    format_type(&normalized_param),
                    format_type(&normalized_elem)
                ),
                Some(filter_expr.location),
            ));
        }

        let (normalized_return, normalized_bool) = canonical_pair(env, &pred.return_type, &bool_type);
        if !types_equal(&normalized_return, &normalized_bool) {
            return Err(TypeError::new(
                format!("Filter (⊳) predicate must return 𝔹, got {}", format_type(&normalized_return)),
                Some(filter_expr.location),
            ));
        }

        // Result is same list type
        return Ok(list_type);
    }

    unreachable!()
}

fn synthesize_fold(
    env: &TypeEnvironment,
    fold_expr: &sigil_ast::FoldExpr,
) -> Result<InferenceType, TypeError> {
    let list_type = synthesize(env, &fold_expr.list)?;

    if !matches!(list_type, InferenceType::List(_)) {
        return Err(TypeError::new(
            format!("Fold (⊕) requires a list, got {}", format_type(&list_type)),
            Some(fold_expr.location),
        ));
    }

    let fn_type = synthesize(env, &fold_expr.func)?;

    if !matches!(fn_type, InferenceType::Function(_)) {
        return Err(TypeError::new(
            format!("Fold (⊕) requires a function, got {}", format_type(&fn_type)),
            Some(fold_expr.location),
        ));
    }

    let init_type = synthesize(env, &fold_expr.init)?;

    if let (InferenceType::List(ref list), InferenceType::Function(ref func)) = (&list_type, &fn_type) {
        // Function should be (Acc, T) → Acc
        if func.params.len() != 2 {
            return Err(TypeError::new(
                format!("Fold (⊕) function should take 2 parameters, got {}", func.params.len()),
                Some(fold_expr.location),
            ));
        }

        // Check function signature matches (Acc, T) → Acc
        let (normalized_acc_param, normalized_init) = canonical_pair(env, &func.params[0], &init_type);
        if !types_equal(&normalized_acc_param, &normalized_init) {
            return Err(TypeError::new(
                format!(
                    "Fold (⊕) function first parameter type {} doesn't match initial value type {}",
                    format_type(&normalized_acc_param),
                    format_type(&normalized_init)
                ),
                Some(fold_expr.location),
            ));
        }

        let (normalized_elem_param, normalized_elem) = canonical_pair(env, &func.params[1], &list.element_type);
        if !types_equal(&normalized_elem_param, &normalized_elem) {
            return Err(TypeError::new(
                format!(
                    "Fold (⊕) function second parameter type {} doesn't match list element type {}",
                    format_type(&normalized_elem_param),
                    format_type(&normalized_elem)
                ),
                Some(fold_expr.location),
            ));
        }

        let (normalized_return, normalized_init) = canonical_pair(env, &func.return_type, &init_type);
        if !types_equal(&normalized_return, &normalized_init) {
            return Err(TypeError::new(
                format!(
                    "Fold (⊕) function return type {} doesn't match accumulator type {}",
                    format_type(&normalized_return),
                    format_type(&normalized_init)
                ),
                Some(fold_expr.location),
            ));
        }

        // Result is accumulator type
        return Ok(init_type);
    }

    unreachable!()
}

fn synthesize_with_mock(
    env: &TypeEnvironment,
    with_mock: &sigil_ast::WithMockExpr,
) -> Result<InferenceType, TypeError> {
    // Check target is mockable or extern
    // For now, simplified validation - just check types match
    let target_type = synthesize(env, &with_mock.target)?;
    let replacement_type = synthesize(env, &with_mock.replacement)?;

    // Replacement must be a function
    if !matches!(replacement_type, InferenceType::Function(_) | InferenceType::Any) {
        return Err(TypeError::new(
            format!(
                "with_mock replacement must be a function, got {}",
                format_type(&replacement_type)
            ),
            Some(with_mock.location),
        ));
    }

    // If both are functions, check they match
    if let (InferenceType::Function(_), InferenceType::Function(_)) = (&target_type, &replacement_type) {
        let (normalized_target, normalized_replacement) =
            canonical_pair(env, &target_type, &replacement_type);
        if !types_equal(&normalized_target, &normalized_replacement) {
            return Err(TypeError::new(
                format!(
                    "with_mock replacement type {} does not match target type {}",
                    format_type(&normalized_replacement),
                    format_type(&normalized_target)
                ),
                Some(with_mock.location),
            ));
        }
    }

    // TODO: Full extern/mockable function validation

    // Return type is the body type
    synthesize(env, &with_mock.body)
}

fn synthesize_pipeline(
    env: &TypeEnvironment,
    pipeline: &sigil_ast::PipelineExpr,
) -> Result<InferenceType, TypeError> {
    // Pipeline operators: |> (forward pipe), >> (forward compose), << (backward compose)
    // For now, simplified: just synthesize the right side
    // TODO: Full pipeline type checking with function composition validation
    synthesize(env, &pipeline.right)
}

fn synthesize_lambda(
    env: &TypeEnvironment,
    lambda_expr: &sigil_ast::LambdaExpr,
) -> Result<InferenceType, TypeError> {
    // Lambda has mandatory type annotations (enforced by parser in canonical form)
    let param_types: Vec<InferenceType> = lambda_expr
        .params
        .iter()
        .map(|p| match &p.type_annotation {
            Some(ty) => ast_type_to_inference_type_resolved(env, ty),
            None => Ok(InferenceType::Any),
        })
        .collect::<Result<_, _>>()?;

    let return_type = ast_type_to_inference_type_resolved(env, &lambda_expr.return_type)?;

    let effects = if lambda_expr.effects.is_empty() {
        None
    } else {
        Some(lambda_expr.effects.iter().cloned().collect())
    };

    // Create environment with parameter bindings
    let mut lambda_env_bindings = HashMap::new();
    for (param, param_type) in lambda_expr.params.iter().zip(&param_types) {
        lambda_env_bindings.insert(param.name.clone(), param_type.clone());
    }
    let lambda_env = env.extend(Some(lambda_env_bindings));

    // Check body against declared return type
    check(&lambda_env, &lambda_expr.body, &return_type)?;

    // TODO: Effect inference and checking
    // For now, we trust the declared effects

    Ok(InferenceType::Function(Box::new(TFunction {
        params: param_types,
        return_type,
        effects,
    })))
}

// Pattern checking helper
fn check_pattern(
    env: &TypeEnvironment,
    pattern: &sigil_ast::Pattern,
    scrutinee_type: &InferenceType,
    bindings: &mut HashMap<String, InferenceType>,
) -> Result<(), TypeError> {
    use sigil_ast::Pattern;

    match pattern {
        Pattern::Wildcard(_) => {
            // Wildcard matches anything
            Ok(())
        }
        Pattern::Identifier(id_pattern) => {
            // Bind variable to scrutinee type
            bindings.insert(id_pattern.name.clone(), scrutinee_type.clone());
            Ok(())
        }
        Pattern::Literal(lit_pattern) => {
            // Check literal type matches scrutinee
            let lit_type = match lit_pattern.literal_type {
                sigil_ast::PatternLiteralType::Int => InferenceType::Primitive(TPrimitive {
                    name: PrimitiveName::Int,
                }),
                sigil_ast::PatternLiteralType::Float => InferenceType::Primitive(TPrimitive {
                    name: PrimitiveName::Float,
                }),
                sigil_ast::PatternLiteralType::Bool => InferenceType::Primitive(TPrimitive {
                    name: PrimitiveName::Bool,
                }),
                sigil_ast::PatternLiteralType::String => InferenceType::Primitive(TPrimitive {
                    name: PrimitiveName::String,
                }),
                sigil_ast::PatternLiteralType::Char => InferenceType::Primitive(TPrimitive {
                    name: PrimitiveName::Char,
                }),
                sigil_ast::PatternLiteralType::Unit => InferenceType::Primitive(TPrimitive {
                    name: PrimitiveName::Unit,
                }),
            };

            let (normalized_lit, normalized_scrutinee) = canonical_pair(env, &lit_type, scrutinee_type);
            if !types_equal(&normalized_lit, &normalized_scrutinee) {
                return Err(TypeError::new(
                    format!(
                        "Pattern type mismatch: expected {}, got {}",
                        format_type(&normalized_scrutinee),
                        format_type(&normalized_lit)
                    ),
                    Some(lit_pattern.location),
                ));
            }
            Ok(())
        }

        Pattern::List(list_pattern) => {
            // List pattern requires list type
            if !matches!(scrutinee_type, InferenceType::List(_)) {
                return Err(TypeError::new(
                    format!(
                        "List pattern requires list type, got {}",
                        format_type(scrutinee_type)
                    ),
                    Some(list_pattern.location),
                ));
            }

            if let InferenceType::List(ref list_type) = scrutinee_type {
                // Check each element pattern
                for elem_pattern in &list_pattern.patterns {
                    check_pattern(env, elem_pattern, &list_type.element_type, bindings)?;
                }

                // Handle rest pattern if present
                if let Some(ref rest_name) = list_pattern.rest {
                    bindings.insert(rest_name.clone(), scrutinee_type.clone());
                }
            }

            Ok(())
        }

        Pattern::Tuple(tuple_pattern) => {
            // Tuple pattern requires tuple type
            if !matches!(scrutinee_type, InferenceType::Tuple(_)) {
                return Err(TypeError::new(
                    format!(
                        "Tuple pattern requires tuple type, got {}",
                        format_type(scrutinee_type)
                    ),
                    Some(tuple_pattern.location),
                ));
            }

            if let InferenceType::Tuple(ref tuple_type) = scrutinee_type {
                if tuple_pattern.patterns.len() != tuple_type.types.len() {
                    return Err(TypeError::new(
                        format!(
                            "Tuple pattern has {} elements, but type has {}",
                            tuple_pattern.patterns.len(),
                            tuple_type.types.len()
                        ),
                        Some(tuple_pattern.location),
                    ));
                }

                for (pattern, typ) in tuple_pattern.patterns.iter().zip(&tuple_type.types) {
                    check_pattern(env, pattern, typ, bindings)?;
                }
            }

            Ok(())
        }

        Pattern::Constructor(constructor_pattern) => {
            // Constructor pattern requires constructor type
            if !matches!(scrutinee_type, InferenceType::Constructor(_)) {
                return Err(TypeError::new(
                    format!(
                        "Constructor pattern requires constructor type, got {}",
                        format_type(scrutinee_type)
                    ),
                    Some(constructor_pattern.location),
                ));
            }

            // Look up the constructor in the environment
            let constructor_type = env.lookup(&constructor_pattern.name);
            if constructor_type.is_none() {
                return Err(TypeError::new(
                    format!("Unknown constructor '{}'", constructor_pattern.name),
                    Some(constructor_pattern.location),
                ));
            }

            let constructor_type = constructor_type.unwrap();

            // Constructor should be a function type
            if !matches!(constructor_type, InferenceType::Function(_)) {
                return Err(TypeError::new(
                    format!("'{}' is not a constructor", constructor_pattern.name),
                    Some(constructor_pattern.location),
                ));
            }

            if let (InferenceType::Function(ref ctor_fn), InferenceType::Constructor(ref scrutinee_ctor)) =
                (&constructor_type, scrutinee_type)
            {
                // Check that constructor's return type matches scrutinee type
                if let InferenceType::Constructor(ref return_ctor) = ctor_fn.return_type {
                    if return_ctor.name != scrutinee_ctor.name {
                        return Err(TypeError::new(
                            format!(
                                "Constructor '{}' returns '{}', expected '{}'",
                                constructor_pattern.name,
                                format_type(&ctor_fn.return_type),
                                scrutinee_ctor.name
                            ),
                            Some(constructor_pattern.location),
                        ));
                    }
                }

                // Check argument patterns against constructor parameter types
                let patterns = &constructor_pattern.patterns;
                if !patterns.is_empty() {
                    if patterns.len() != ctor_fn.params.len() {
                        return Err(TypeError::new(
                            format!(
                                "Constructor '{}' expects {} arguments, got {}",
                                constructor_pattern.name,
                                ctor_fn.params.len(),
                                patterns.len()
                            ),
                            Some(constructor_pattern.location),
                        ));
                    }

                    for (pattern, param_type) in patterns.iter().zip(&ctor_fn.params) {
                        check_pattern(env, pattern, param_type, bindings)?;
                    }
                }
            }

            Ok(())
        }

        Pattern::Record(_) => Err(TypeError::new(
            "Record pattern matching not yet implemented".to_string(),
            None,
        )),
    }
}

fn synthesize_type_ascription(
    env: &TypeEnvironment,
    type_asc: &sigil_ast::TypeAscriptionExpr,
) -> Result<InferenceType, TypeError> {
    // Convert ascribed type from AST to inference type
    let ascribed_type = ast_type_to_inference_type_resolved(env, &type_asc.ascribed_type)?;

    // Check that the expression matches the ascribed type
    check(env, &type_asc.expr, &ascribed_type)?;

    // Return the ascribed type
    Ok(ascribed_type)
}

// ============================================================================
// CHECKING (⇐) - Verify expression matches expected type
// ============================================================================

/// Check expression against expected type
/// Returns error if expression doesn't match
fn check(
    env: &TypeEnvironment,
    expr: &Expr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    // Special case: checking against 'any' type always succeeds (FFI trust mode)
    if matches!(expected_type, InferenceType::Any) {
        return Ok(());
    }

    // For most expressions: synthesize then verify equality
    let actual_type = synthesize(env, expr)?;

    // Special case: 'any' type matches anything (FFI trust mode)
    if matches!(actual_type, InferenceType::Any) {
        return Ok(());
    }

    // Normalize structural named types before equality checks.
    let (normalized_actual, normalized_expected) = canonical_pair(env, &actual_type, expected_type);

    if !types_equal(&normalized_actual, &normalized_expected) {
        return Err(TypeError::mismatch(
            format!(
                "Type mismatch: expected {}, got {}",
                format_type(&normalized_expected),
                format_type(&normalized_actual)
            ),
            None, // TODO: extract location from expression variant
            normalized_expected,
            normalized_actual,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;

    fn synthetic_loc() -> sigil_ast::SourceLocation {
        sigil_ast::SourceLocation::single(sigil_ast::Position::new(1, 1, 0))
    }

    #[test]
    fn test_simple_integer_function() {
        let source = "λadd(x:ℤ,y:ℤ)→ℤ=x+y";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());

        let types = result.unwrap();
        assert_eq!(types.len(), 1);
        assert!(types.contains_key("add"));
    }

    #[test]
    fn test_type_mismatch() {
        let source = "λbad(x:ℤ)→𝕊=x";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_literal_types() {
        let source = "λf()→ℤ=42";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_application() {
        let source = "λadd(x:ℤ,y:ℤ)→ℤ=x+y\nλmain()→ℤ=add(1,2)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_sum_type_constructors() {
        // Test that sum type constructors are registered and callable
        // Using fully specified constructor type for now
        let source = "t Color=Red|Green|Blue\nλgetRed()→Color=Red()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        // Should succeed - Red is registered as a constructor
        assert!(result.is_ok());
    }

    #[test]
    fn test_any_is_rejected_outside_ffi() {
        let source = "t Response={headers:Any}\nλmain()→Response={headers:{}}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("reserved for untyped FFI trust mode"));
    }

    #[test]
    fn test_const_annotation_normalizes_named_product_type() {
        let source = "t MkdirOptions={recursive:𝔹}\nc opts=({recursive:⊤}:MkdirOptions)\nλmain()→𝕌=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_append_normalizes_named_product_type() {
        let source = "t Todo={done:𝔹,id:ℤ,text:𝕊}\nλmain()→[Todo]=[{done:⊥,id:1,text:\"a\"}]⧺[Todo{done:⊥,id:2,text:\"b\"}]";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_normalizes_named_product_type() {
        let source = "t Todo={done:𝔹,id:ℤ,text:𝕊}\nλkeep(todo:Todo)→Todo=todo\nλmain()→[Todo]=[{done:⊥,id:1,text:\"a\"}]↦keep";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_named_product_equality_uses_canonical_form() {
        let source = "t Todo={done:𝔹,id:ℤ,text:𝕊}\nλmain()→𝔹=(({done:⊥,id:1,text:\"a\"}:Todo)={done:⊥,id:1,text:\"a\"})";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_sum_types_remain_nominal_after_normalization() {
        let source = "t Box={value:ℤ}\nt Wrap=Wrap(Box)\nλmain()→Wrap=({value:1}:Wrap)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_qualified_imported_product_type_resolves_for_field_access() {
        let source = "i src⋅types\nλslug_len(meta:src⋅types.ArticleMeta)→ℤ=#meta.slug";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut imported_type_registries = HashMap::new();
        imported_type_registries.insert(
            "src⋅types".to_string(),
            HashMap::from([(
                "ArticleMeta".to_string(),
                TypeInfo {
                    type_params: vec![],
                    definition: TypeDef::Product(sigil_ast::ProductType {
                        fields: vec![
                            sigil_ast::Field {
                                name: "title".to_string(),
                                field_type: Type::Primitive(sigil_ast::PrimitiveType {
                                    name: PrimitiveName::String,
                                    location: synthetic_loc(),
                                }),
                                location: synthetic_loc(),
                            },
                            sigil_ast::Field {
                                name: "date".to_string(),
                                field_type: Type::Primitive(sigil_ast::PrimitiveType {
                                    name: PrimitiveName::String,
                                    location: synthetic_loc(),
                                }),
                                location: synthetic_loc(),
                            },
                            sigil_ast::Field {
                                name: "author".to_string(),
                                field_type: Type::Primitive(sigil_ast::PrimitiveType {
                                    name: PrimitiveName::String,
                                    location: synthetic_loc(),
                                }),
                                location: synthetic_loc(),
                            },
                            sigil_ast::Field {
                                name: "slug".to_string(),
                                field_type: Type::Primitive(sigil_ast::PrimitiveType {
                                    name: PrimitiveName::String,
                                    location: synthetic_loc(),
                                }),
                                location: synthetic_loc(),
                            },
                        ],
                        location: synthetic_loc(),
                    }),
                },
            )]),
        );

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                imported_namespaces: None,
                imported_type_registries: Some(imported_type_registries),
                source_file: None,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_local_named_product_return_type_resolves_for_field_access() {
        let source = "t ParseResult={content:𝕊}\nλparse()→ParseResult={content:\"x\"}\nλmain()→ℤ=#(parse().content)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    // TODO: Add list pattern test when parser fully supports match expression syntax
    // The type checking logic is complete for list patterns [x,.xs]

    // TODO: Add If/Let expression tests when full parser support is confirmed
    // The type checking logic is implemented for:
    // - Match expressions with all pattern types (literal, identifier, wildcard, list, tuple, constructor)
    // - If expressions with optional else branches
    // - Let expressions with identifier patterns
    // Waiting for complete lexer/parser syntax support to test end-to-end
}
