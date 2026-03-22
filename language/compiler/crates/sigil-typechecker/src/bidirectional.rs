//! Bidirectional Type Checking for Sigil
//!
//! Uses two complementary modes:
//! - Synthesis (⇒): Infer type from expression structure (bottom-up)
//! - Checking (⇐): Verify expression matches expected type (top-down)
//!
//! This is simpler than Hindley-Milner because Sigil requires mandatory
//! type annotations everywhere, making the inference burden much lighter.

use crate::environment::{
    collect_type_var_ids, explicit_scheme, BindingMeta, TypeEnvironment, TypeInfo,
};
use crate::errors::{format_type, TypeError};
use crate::typed_ir::{
    MethodSelector, PurityClass, StrictnessClass, TypeCheckResult, TypedBinaryExpr, TypedCallExpr,
    TypedConcurrentConfig, TypedConcurrentExpr, TypedConcurrentStep, TypedConstDecl,
    TypedConstructorCallExpr, TypedDeclaration, TypedExpr, TypedExprKind, TypedExternCallExpr,
    TypedExternDecl, TypedFieldAccessExpr, TypedFilterExpr, TypedFoldExpr, TypedFunctionDecl,
    TypedIfExpr, TypedImportDecl, TypedIndexExpr, TypedLambdaExpr, TypedLetExpr, TypedListExpr,
    TypedMapEntryExpr, TypedMapExpr, TypedMapLiteralExpr, TypedMatchArm, TypedMatchExpr,
    TypedMethodCallExpr, TypedPipelineExpr, TypedProgram, TypedRecordExpr, TypedRecordField,
    TypedSpawnEachStep, TypedSpawnStep, TypedTestDecl, TypedTupleExpr, TypedTypeDecl,
    TypedUnaryExpr, TypedWithMockExpr, WithMockTarget,
};
use crate::types::{
    apply_subst, ast_type_to_inference_type_with_params, types_equal, unify, EffectSet,
    InferenceType, TConstructor, TFunction, TMap, TPrimitive, TRecord,
};
use crate::TypeCheckOptions;
use sigil_ast::{
    BinaryOperator, Declaration, Expr, FunctionDecl, LiteralType, PrimitiveName, Program, Type,
    TypeDef,
};
use sigil_diagnostics::codes;
use std::collections::{HashMap, HashSet};

type TypeParamEnv = HashMap<String, InferenceType>;

/// Type check a Sigil program
///
/// Returns a map of function names to their inferred types
pub fn type_check(
    program: &Program,
    _source_code: &str,
    options: TypeCheckOptions,
) -> Result<TypeCheckResult, TypeError> {
    validate_surface_types(program)?;

    let mut env = TypeEnvironment::create_initial();
    env.set_effect_catalog(options.effect_catalog.clone().unwrap_or_default());
    env.set_source_file(options.source_file.clone());
    let mut types = HashMap::new();
    let mut schemes = HashMap::new();

    // Register imported type registries
    if let Some(imported_type_registries) = options.imported_type_registries.as_ref() {
        for (module_id, type_registry) in imported_type_registries {
            env.register_imported_types(module_id.clone(), type_registry.clone());
        }
    }

    if let Some(imported_value_schemes) = options.imported_value_schemes.as_ref() {
        for (module_id, value_schemes) in imported_value_schemes {
            env.register_imported_value_schemes(module_id.clone(), value_schemes.clone());
        }
    }

    // Seed the implicit core prelude into the unqualified environment.
    if let Some(prelude_types) = options
        .imported_type_registries
        .as_ref()
        .and_then(|registries| registries.get("core::prelude"))
    {
        for (name, info) in prelude_types {
            env.register_type(name.clone(), info.clone());
        }
    }

    if let Some(prelude_schemes) = options
        .imported_value_schemes
        .as_ref()
        .and_then(|schemes| schemes.get("core::prelude"))
    {
        for (name, scheme) in prelude_schemes {
            env.bind_scheme(name.clone(), scheme.clone());
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
                        if type_decl.type_params.is_empty() {
                            env.bind(variant.name.clone(), constructor_type.clone());
                            types.insert(variant.name.clone(), constructor_type.clone());
                            schemes.insert(
                                variant.name.clone(),
                                explicit_scheme(&constructor_type, &HashSet::new()),
                            );
                        } else {
                            let mut quantified_vars = HashSet::new();
                            collect_type_var_ids(&constructor_type, &mut quantified_vars);
                            types.insert(variant.name.clone(), constructor_type.clone());
                            schemes.insert(
                                variant.name.clone(),
                                explicit_scheme(&constructor_type, &quantified_vars),
                            );
                            env.bind_scheme(
                                variant.name.clone(),
                                explicit_scheme(&constructor_type, &quantified_vars),
                            );
                        }
                    }
                }
            }

            Declaration::Effect(_) => {}

            Declaration::Function(func_decl) => {
                let type_param_env = make_type_param_env(&func_decl.type_params);
                // Extract function type from signature
                let params: Vec<InferenceType> = func_decl
                    .params
                    .iter()
                    .map(|p| match &p.type_annotation {
                        Some(ty) => {
                            ast_type_to_inference_type_resolved(&env, Some(&type_param_env), ty)
                        }
                        None => Ok(InferenceType::Any),
                    })
                    .collect::<Result<_, _>>()?;

                let return_type = func_decl
                    .return_type
                    .as_ref()
                    .map(|ty| ast_type_to_inference_type_resolved(&env, Some(&type_param_env), ty))
                    .transpose()?
                    .unwrap_or(InferenceType::Any);

                let effects = if func_decl.effects.is_empty() {
                    None
                } else {
                    Some(resolve_effect_names(
                        &env,
                        &func_decl.effects,
                        func_decl.location,
                        "function signature",
                    )?)
                };

                let func_type = InferenceType::Function(Box::new(TFunction {
                    params,
                    return_type,
                    effects,
                }));

                let binding_type = if func_decl.type_params.is_empty() {
                    func_type.clone()
                } else {
                    let mut quantified_vars = HashSet::new();
                    collect_type_var_ids(&func_type, &mut quantified_vars);
                    let scheme = explicit_scheme(&func_type, &quantified_vars);
                    env.bind_scheme(func_decl.name.clone(), scheme.clone());
                    schemes.insert(func_decl.name.clone(), scheme);
                    func_type.clone()
                };

                if func_decl.type_params.is_empty() {
                    env.bind(func_decl.name.clone(), binding_type.clone());
                }

                types.insert(func_decl.name.clone(), binding_type);
            }

            Declaration::Const(const_decl) => {
                // Register constant type
                let const_type = const_decl
                    .type_annotation
                    .as_ref()
                    .map(|ty| ast_type_to_inference_type_resolved(&env, None, ty))
                    .transpose()?
                    .unwrap_or(InferenceType::Any);

                env.bind(const_decl.name.clone(), const_type.clone());
                types.insert(const_decl.name.clone(), const_type);
            }

            Declaration::Extern(extern_decl) => {
                let namespace_name = extern_decl.module_path.join("::");

                if let Some(members) = &extern_decl.members {
                    let mut fields = HashMap::new();
                    for member in members {
                        let member_type =
                            ast_type_to_inference_type_resolved(&env, None, &member.member_type)?;
                        fields.insert(member.name.clone(), member_type);
                    }
                    env.bind_with_meta(
                        namespace_name,
                        InferenceType::Record(TRecord { fields, name: None }),
                        BindingMeta {
                            is_extern_namespace: true,
                        },
                    );
                } else {
                    // Untyped extern: trust mode
                    env.bind_with_meta(
                        namespace_name,
                        InferenceType::Any,
                        BindingMeta {
                            is_extern_namespace: true,
                        },
                    );
                }
            }

            Declaration::Import(import_decl) => {
                let namespace_name = import_decl.module_path.join("::");
                let imported_type = options
                    .imported_namespaces
                    .as_ref()
                    .and_then(|ns: &HashMap<String, InferenceType>| ns.get(&namespace_name))
                    .cloned()
                    .unwrap_or(InferenceType::Any);

                env.bind(namespace_name, imported_type);
            }

            Declaration::Test(_) => {
                // TODO: Check test declarations
            }
        }
    }

    let mut typed_declarations = Vec::new();

    // Second pass: Type check function bodies and build typed IR
    for decl in &program.declarations {
        if let Declaration::Function(func_decl) = decl {
            check_function_decl(&env, func_decl)?;
            typed_declarations.push(TypedDeclaration::Function(build_typed_function_decl(
                &env, func_decl,
            )?));
        } else if let Declaration::Const(const_decl) = decl {
            // Type check constant value
            let value_type = synthesize(&env, &const_decl.value)?;
            if let Some(ref annotation) = const_decl.type_annotation {
                let expected_type = ast_type_to_inference_type_resolved(&env, None, annotation)?;
                let (normalized_value, normalized_expected) =
                    canonical_pair(&env, &value_type, &expected_type);
                if !types_equal(&normalized_value, &normalized_expected) {
                    return Err(TypeError::mismatch(
                        format!("Constant '{}' type mismatch", const_decl.name),
                        Some(const_decl.location),
                        normalized_expected,
                        normalized_value,
                    ));
                }
            }
            typed_declarations.push(TypedDeclaration::Const(build_typed_const_decl(
                &env, const_decl,
            )?));
        } else if let Declaration::Type(type_decl) = decl {
            typed_declarations.push(TypedDeclaration::Type(TypedTypeDecl {
                ast: type_decl.clone(),
            }));
        } else if let Declaration::Import(import_decl) = decl {
            typed_declarations.push(TypedDeclaration::Import(TypedImportDecl {
                ast: import_decl.clone(),
            }));
        } else if let Declaration::Extern(extern_decl) = decl {
            typed_declarations.push(TypedDeclaration::Extern(TypedExternDecl {
                ast: extern_decl.clone(),
            }));
        } else if let Declaration::Test(test_decl) = decl {
            typed_declarations.push(TypedDeclaration::Test(build_typed_test_decl(
                &env, test_decl,
            )?));
        } else if let Declaration::Effect(_) = decl {
            // Effect declarations are compile-time only and do not appear in typed IR.
        }
    }

    Ok(TypeCheckResult {
        declaration_types: types,
        declaration_schemes: schemes,
        typed_program: TypedProgram {
            declarations: typed_declarations,
        },
    })
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

fn make_type_param_env(type_params: &[String]) -> TypeParamEnv {
    type_params
        .iter()
        .cloned()
        .map(|name| {
            let typ = crate::types::fresh_type_var(Some(name.clone()));
            (name, typ)
        })
        .collect()
}

fn resolve_qualified_type(
    env: &TypeEnvironment,
    type_param_env: Option<&TypeParamEnv>,
    qualified: &sigil_ast::QualifiedType,
) -> Result<InferenceType, TypeError> {
    let module_id = qualified.module_path.join("::");
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
        .map(|arg| ast_type_to_inference_type_resolved(env, type_param_env, arg))
        .collect::<Result<Vec<_>, _>>()?;

    if type_args.len() != type_info.type_params.len() {
        return Err(TypeError::new(
            format!(
                "Type argument mismatch: {} expects {} type argument{}, but got {}",
                qualified.type_name,
                type_info.type_params.len(),
                if type_info.type_params.len() == 1 {
                    ""
                } else {
                    "s"
                },
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
                        ast_type_to_inference_type_resolved(
                            env,
                            type_param_env,
                            &field.field_type,
                        )?,
                    );
                }

                return Ok(InferenceType::Record(TRecord {
                    fields,
                    name: Some(qualified_name),
                }));
            }
            TypeDef::Alias(alias) => {
                return ast_type_to_inference_type_resolved(
                    env,
                    type_param_env,
                    &alias.aliased_type,
                );
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
        module_id.split("::").map(|part| part.to_string()).collect(),
        type_name.to_string(),
    ))
}

fn constructor_display_name(module_path: &[String], name: &str) -> String {
    if module_path.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", module_path.join("::"), name)
    }
}

fn lookup_constructor_type(
    env: &TypeEnvironment,
    module_path: &[String],
    name: &str,
) -> Result<Option<InferenceType>, TypeError> {
    if module_path.is_empty() {
        return Ok(env.lookup(name));
    }

    if let Some((type_name, qualified_module_path, variant, type_params)) =
        env.lookup_qualified_constructor(module_path, name)
    {
        let qualified_type_name = format!("{}.{}", qualified_module_path.join("::"), type_name);
        return Ok(Some(create_constructor_type_with_result_name(
            env,
            &variant,
            &type_params,
            &qualified_type_name,
        )?));
    }

    Ok(None)
}

fn resolve_named_type(
    env: &TypeEnvironment,
    inference_type: &InferenceType,
) -> Result<InferenceType, TypeError> {
    match inference_type {
        InferenceType::Constructor(constructor) if constructor.type_args.is_empty() => {
            if let Some((module_path, type_name)) =
                split_qualified_constructor_name(&constructor.name)
            {
                if let Some(type_info) = env.lookup_qualified_type(&module_path, &type_name) {
                    if type_info.type_params.is_empty() {
                        return match &type_info.definition {
                            TypeDef::Product(product) => {
                                let mut fields = HashMap::new();
                                for field in &product.fields {
                                    fields.insert(
                                        field.name.clone(),
                                        ast_type_to_inference_type_resolved(
                                            env,
                                            None,
                                            &field.field_type,
                                        )?,
                                    );
                                }
                                Ok(InferenceType::Record(TRecord {
                                    fields,
                                    name: Some(constructor.name.clone()),
                                }))
                            }
                            TypeDef::Alias(alias) => {
                                ast_type_to_inference_type_resolved(env, None, &alias.aliased_type)
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
                                    ast_type_to_inference_type_resolved(
                                        env,
                                        None,
                                        &field.field_type,
                                    )?,
                                );
                            }
                            Ok(InferenceType::Record(TRecord {
                                fields,
                                name: Some(constructor.name.clone()),
                            }))
                        }
                        TypeDef::Alias(alias) => {
                            ast_type_to_inference_type_resolved(env, None, &alias.aliased_type)
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
    type_param_env: Option<&TypeParamEnv>,
    ast_type: &Type,
) -> Result<InferenceType, TypeError> {
    match ast_type {
        Type::Qualified(qualified) => resolve_qualified_type(env, type_param_env, qualified),
        Type::List(list_type) => Ok(InferenceType::List(Box::new(crate::types::TList {
            element_type: ast_type_to_inference_type_resolved(
                env,
                type_param_env,
                &list_type.element_type,
            )?,
        }))),
        Type::Tuple(tuple_type) => Ok(InferenceType::Tuple(crate::types::TTuple {
            types: tuple_type
                .types
                .iter()
                .map(|ty| ast_type_to_inference_type_resolved(env, type_param_env, ty))
                .collect::<Result<Vec<_>, _>>()?,
        })),
        Type::Function(func_type) => Ok(InferenceType::Function(Box::new(TFunction {
            params: func_type
                .param_types
                .iter()
                .map(|ty| ast_type_to_inference_type_resolved(env, type_param_env, ty))
                .collect::<Result<Vec<_>, _>>()?,
            return_type: ast_type_to_inference_type_resolved(
                env,
                type_param_env,
                &func_type.return_type,
            )?,
            effects: if func_type.effects.is_empty() {
                None
            } else {
                Some(resolve_effect_names(
                    env,
                    &func_type.effects,
                    func_type.location,
                    "function type",
                )?)
            },
        }))),
        _ => {
            let inferred = ast_type_to_inference_type_with_params(ast_type, type_param_env);
            resolve_named_type(env, &inferred)
        }
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
        Declaration::Effect(_) => Ok(()),
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
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_expr_surface_types(&entry.key)?;
                validate_expr_surface_types(&entry.value)?;
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
        Expr::Concurrent(concurrent_expr) => {
            validate_expr_surface_types(&concurrent_expr.width)?;
            if let Some(policy) = &concurrent_expr.policy {
                for field in &policy.fields {
                    validate_expr_surface_types(&field.value)?;
                }
            }
            for step in &concurrent_expr.steps {
                match step {
                    sigil_ast::ConcurrentStep::Spawn(spawn) => {
                        validate_expr_surface_types(&spawn.expr)?;
                    }
                    sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_expr_surface_types(&spawn_each.list)?;
                        validate_expr_surface_types(&spawn_each.func)?;
                    }
                }
            }
            Ok(())
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
/// For example, Some : T => Option[T]
fn create_constructor_type(
    env: &TypeEnvironment,
    variant: &sigil_ast::Variant,
    type_params: &[String],
    type_name: &str,
) -> Result<InferenceType, TypeError> {
    create_constructor_type_with_result_name(env, variant, type_params, type_name)
}

fn create_constructor_type_with_result_name(
    env: &TypeEnvironment,
    variant: &sigil_ast::Variant,
    type_params: &[String],
    result_type_name: &str,
) -> Result<InferenceType, TypeError> {
    let type_param_env = make_type_param_env(type_params);

    // Convert variant field types to inference types
    let params: Vec<InferenceType> = variant
        .types
        .iter()
        .map(|field_type| {
            ast_type_to_inference_type_resolved(env, Some(&type_param_env), field_type)
        })
        .collect::<Result<_, _>>()?;

    // Result type is the generic constructor with all declared type arguments.
    let result_type = InferenceType::Constructor(crate::types::TConstructor {
        name: result_type_name.to_string(),
        type_args: type_params
            .iter()
            .map(|name| {
                type_param_env
                    .get(name)
                    .cloned()
                    .expect("type parameter must exist in constructor environment")
            })
            .collect(),
    });

    Ok(InferenceType::Function(Box::new(TFunction {
        params,
        return_type: result_type,
        effects: None,
    })))
}

/// Type check a function declaration
fn check_function_decl(env: &TypeEnvironment, func_decl: &FunctionDecl) -> Result<(), TypeError> {
    let type_param_env = make_type_param_env(&func_decl.type_params);
    // Create environment with parameter bindings
    let mut func_env = env.extend(None);

    for param in &func_decl.params {
        let param_type = param
            .type_annotation
            .as_ref()
            .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
            .transpose()?
            .unwrap_or(InferenceType::Any);
        func_env.bind(param.name.clone(), param_type);
    }

    // Get expected return type
    let expected_return_type = func_decl
        .return_type
        .as_ref()
        .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
        .transpose()?
        .unwrap_or(InferenceType::Any);

    // Type check body
    check(&func_env, &func_decl.body, &expected_return_type)?;
    let typed_body = build_typed_expr(&func_env, &func_decl.body)?;
    declared_effects_cover_actual(
        env,
        &func_decl.effects,
        &typed_body.effects,
        func_decl.location,
        &format!("Function '{}'", func_decl.name),
    )?;

    Ok(())
}

fn effects_option_to_set(effects: &Option<EffectSet>) -> EffectSet {
    effects.clone().unwrap_or_default()
}

fn resolve_effect_names(
    env: &TypeEnvironment,
    effects: &[String],
    location: sigil_ast::SourceLocation,
    context: &str,
) -> Result<EffectSet, TypeError> {
    env.effect_catalog()
        .expand_effect_names(effects)
        .map(|expanded| expanded.into_iter().collect())
        .map_err(|message| TypeError::new(format!("{}: {}", context, message), Some(location)))
}

fn declared_effects_cover_actual(
    env: &TypeEnvironment,
    declared_surface_effects: &[String],
    actual_effects: &EffectSet,
    location: sigil_ast::SourceLocation,
    context: &str,
) -> Result<(), TypeError> {
    let declared_effects = resolve_effect_names(env, declared_surface_effects, location, context)?;
    if actual_effects.is_subset(&declared_effects) {
        return Ok(());
    }

    let mut missing: Vec<String> = actual_effects
        .difference(&declared_effects)
        .cloned()
        .collect();
    missing.sort();

    Err(TypeError::new(
        format!(
            "{} is missing declared effects: {}",
            context,
            missing
                .into_iter()
                .map(|effect| format!("!{}", effect))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Some(location),
    ))
}

fn merge_effects(values: impl IntoIterator<Item = EffectSet>) -> EffectSet {
    let mut merged = HashSet::new();
    for value in values {
        merged.extend(value);
    }
    merged
}

fn purity_from_effects(effects: &EffectSet) -> PurityClass {
    if effects.is_empty() {
        PurityClass::Pure
    } else {
        PurityClass::Effectful
    }
}

fn typed_expr(
    kind: TypedExprKind,
    typ: InferenceType,
    effects: EffectSet,
    strictness: StrictnessClass,
    location: sigil_ast::SourceLocation,
) -> TypedExpr {
    TypedExpr {
        kind,
        typ,
        purity: purity_from_effects(&effects),
        effects,
        strictness,
        location,
    }
}

fn bool_type() -> InferenceType {
    InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    })
}

fn int_type() -> InferenceType {
    InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Int,
    })
}

fn concurrent_outcome_type(
    success_type: InferenceType,
    error_type: InferenceType,
) -> InferenceType {
    InferenceType::Constructor(TConstructor {
        name: "ConcurrentOutcome".to_string(),
        type_args: vec![success_type, error_type],
    })
}

fn option_type(inner: InferenceType) -> InferenceType {
    InferenceType::Constructor(TConstructor {
        name: "Option".to_string(),
        type_args: vec![inner],
    })
}

fn concurrent_jitter_record_type() -> InferenceType {
    InferenceType::Record(TRecord {
        fields: HashMap::from([
            ("max".to_string(), int_type()),
            ("min".to_string(), int_type()),
        ]),
        name: None,
    })
}

fn concurrent_policy_fields<'a>(
    policy: Option<&'a sigil_ast::RecordExpr>,
    location: sigil_ast::SourceLocation,
) -> Result<(Option<&'a Expr>, Option<&'a Expr>, Option<&'a Expr>), TypeError> {
    let mut jitter_ms = None;
    let mut stop_on = None;
    let mut window_ms = None;
    let mut seen = HashSet::new();

    let Some(policy) = policy else {
        return Ok((None, None, None));
    };

    for field in &policy.fields {
        if !seen.insert(field.name.clone()) {
            return Err(TypeError::new(
                format!(
                    "Concurrent region policy field '{}' is duplicated",
                    field.name
                ),
                Some(field.location),
            ));
        }

        match field.name.as_str() {
            "jitterMs" => jitter_ms = Some(&field.value),
            "stopOn" => stop_on = Some(&field.value),
            "windowMs" => window_ms = Some(&field.value),
            _ => {
                return Err(TypeError::new(
                    format!(
                        "Unknown concurrent region policy field '{}'. Use exactly jitterMs, stopOn, and windowMs.",
                        field.name
                    ),
                    Some(field.location),
                ));
            }
        }
    }

    if policy.fields.is_empty() {
        return Err(TypeError::new(
            "Concurrent region policy record must not be empty".to_string(),
            Some(location),
        ));
    }

    Ok((jitter_ms, stop_on, window_ms))
}

fn result_type_parts(
    env: &TypeEnvironment,
    typ: &InferenceType,
) -> Option<(InferenceType, InferenceType)> {
    let normalized = env.normalize_type(typ);
    match normalized {
        InferenceType::Constructor(tcons)
            if tcons.name == "Result" && tcons.type_args.len() == 2 =>
        {
            Some((tcons.type_args[0].clone(), tcons.type_args[1].clone()))
        }
        _ => None,
    }
}

fn same_type(env: &TypeEnvironment, left: &InferenceType, right: &InferenceType) -> bool {
    let (normalized_left, normalized_right) = canonical_pair(env, left, right);
    types_equal(&normalized_left, &normalized_right)
}

fn build_typed_function_decl(
    env: &TypeEnvironment,
    func_decl: &FunctionDecl,
) -> Result<TypedFunctionDecl, TypeError> {
    let type_param_env = make_type_param_env(&func_decl.type_params);
    let mut lambda_env_bindings = HashMap::new();
    for param in &func_decl.params {
        if let Some(ref ty) = param.type_annotation {
            lambda_env_bindings.insert(
                param.name.clone(),
                ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty)?,
            );
        }
    }
    let function_env = env.extend(Some(lambda_env_bindings));
    let body = build_typed_expr(&function_env, &func_decl.body)?;

    let return_type = func_decl
        .return_type
        .as_ref()
        .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
        .transpose()?
        .unwrap_or(InferenceType::Any);

    Ok(TypedFunctionDecl {
        name: func_decl.name.clone(),
        type_params: func_decl.type_params.clone(),
        params: func_decl.params.clone(),
        return_type,
        effects: if func_decl.effects.is_empty() {
            None
        } else {
            Some(resolve_effect_names(
                env,
                &func_decl.effects,
                func_decl.location,
                "function signature",
            )?)
        },
        body,
        location: func_decl.location,
    })
}

fn build_typed_const_decl(
    env: &TypeEnvironment,
    const_decl: &sigil_ast::ConstDecl,
) -> Result<TypedConstDecl, TypeError> {
    let value = build_typed_expr(env, &const_decl.value)?;
    let typ = const_decl
        .type_annotation
        .as_ref()
        .map(|ty| ast_type_to_inference_type_resolved(env, None, ty))
        .transpose()?
        .unwrap_or_else(|| value.typ.clone());

    Ok(TypedConstDecl {
        name: const_decl.name.clone(),
        type_annotation: const_decl.type_annotation.clone(),
        typ,
        value,
        location: const_decl.location,
    })
}

fn build_typed_test_decl(
    env: &TypeEnvironment,
    test_decl: &sigil_ast::TestDecl,
) -> Result<TypedTestDecl, TypeError> {
    let body = build_typed_expr(env, &test_decl.body)?;
    declared_effects_cover_actual(
        env,
        &test_decl.effects,
        &body.effects,
        test_decl.location,
        &format!("Test '{}'", test_decl.description),
    )?;
    Ok(TypedTestDecl {
        description: test_decl.description.clone(),
        effects: if test_decl.effects.is_empty() {
            None
        } else {
            Some(resolve_effect_names(
                env,
                &test_decl.effects,
                test_decl.location,
                "test signature",
            )?)
        },
        body,
        location: test_decl.location,
    })
}

fn build_typed_expr(env: &TypeEnvironment, expr: &Expr) -> Result<TypedExpr, TypeError> {
    let typ = synthesize(env, expr)?;
    match expr {
        Expr::Literal(lit) => Ok(typed_expr(
            TypedExprKind::Literal(lit.clone()),
            typ,
            HashSet::new(),
            StrictnessClass::Deferred,
            lit.location,
        )),
        Expr::Identifier(id) => Ok(typed_expr(
            TypedExprKind::Identifier(id.clone()),
            typ,
            HashSet::new(),
            StrictnessClass::Deferred,
            id.location,
        )),
        Expr::MemberAccess(member_access) => {
            if lookup_constructor_type(env, &member_access.namespace, &member_access.member)?
                .is_some()
            {
                Ok(typed_expr(
                    TypedExprKind::NamespaceMember {
                        namespace: member_access.namespace.clone(),
                        member: member_access.member.clone(),
                    },
                    typ,
                    HashSet::new(),
                    StrictnessClass::Deferred,
                    member_access.location,
                ))
            } else {
                let mut effects = HashSet::new();
                if let InferenceType::Function(tfunc) = &typ {
                    effects.extend(effects_option_to_set(&tfunc.effects));
                }
                Ok(typed_expr(
                    TypedExprKind::NamespaceMember {
                        namespace: member_access.namespace.clone(),
                        member: member_access.member.clone(),
                    },
                    typ,
                    effects,
                    StrictnessClass::Deferred,
                    member_access.location,
                ))
            }
        }
        Expr::Lambda(lambda) => {
            let mut lambda_env_bindings = HashMap::new();
            for param in &lambda.params {
                if let Some(ref ty) = param.type_annotation {
                    lambda_env_bindings.insert(
                        param.name.clone(),
                        ast_type_to_inference_type_resolved(env, None, ty)?,
                    );
                }
            }
            let lambda_env = env.extend(Some(lambda_env_bindings));
            let body = build_typed_expr(&lambda_env, &lambda.body)?;
            let effects =
                resolve_effect_names(env, &lambda.effects, lambda.location, "lambda signature")?;
            Ok(typed_expr(
                TypedExprKind::Lambda(TypedLambdaExpr {
                    params: lambda.params.clone(),
                    effects: if effects.is_empty() {
                        None
                    } else {
                        Some(effects.clone())
                    },
                    return_type: ast_type_to_inference_type_resolved(
                        env,
                        None,
                        &lambda.return_type,
                    )?,
                    body: Box::new(body),
                }),
                typ,
                effects,
                StrictnessClass::Deferred,
                lambda.location,
            ))
        }
        Expr::Application(app) => build_typed_application(env, app, typ),
        Expr::Binary(bin) => {
            let left = build_typed_expr(env, &bin.left)?;
            let right = build_typed_expr(env, &bin.right)?;
            let effects = merge_effects([left.effects.clone(), right.effects.clone()]);
            Ok(typed_expr(
                TypedExprKind::Binary(TypedBinaryExpr {
                    left: Box::new(left),
                    operator: bin.operator,
                    right: Box::new(right),
                }),
                typ,
                effects,
                StrictnessClass::Strict,
                bin.location,
            ))
        }
        Expr::Unary(un) => {
            let operand = build_typed_expr(env, &un.operand)?;
            let effects = operand.effects.clone();
            Ok(typed_expr(
                TypedExprKind::Unary(TypedUnaryExpr {
                    operand: Box::new(operand),
                    operator: un.operator,
                }),
                typ,
                effects,
                StrictnessClass::Strict,
                un.location,
            ))
        }
        Expr::If(if_expr) => {
            let condition = build_typed_expr(env, &if_expr.condition)?;
            let then_branch = build_typed_expr(env, &if_expr.then_branch)?;
            let else_branch = if_expr
                .else_branch
                .as_ref()
                .map(|branch| build_typed_expr(env, branch).map(Box::new))
                .transpose()?;
            let mut effect_sets = vec![condition.effects.clone(), then_branch.effects.clone()];
            if let Some(ref branch) = else_branch {
                effect_sets.push(branch.effects.clone());
            }
            Ok(typed_expr(
                TypedExprKind::If(TypedIfExpr {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch,
                }),
                typ,
                merge_effects(effect_sets),
                StrictnessClass::Strict,
                if_expr.location,
            ))
        }
        Expr::Let(let_expr) => {
            let value = build_typed_expr(env, &let_expr.value)?;
            let mut bindings = HashMap::new();
            if let sigil_ast::Pattern::Identifier(id_pattern) = &let_expr.pattern {
                bindings.insert(id_pattern.name.clone(), value.typ.clone());
            }
            let body_env = env.extend(Some(bindings));
            let body = build_typed_expr(&body_env, &let_expr.body)?;
            Ok(typed_expr(
                TypedExprKind::Let(TypedLetExpr {
                    pattern: let_expr.pattern.clone(),
                    value: Box::new(value.clone()),
                    body: Box::new(body.clone()),
                }),
                typ,
                merge_effects([value.effects, body.effects]),
                StrictnessClass::Deferred,
                let_expr.location,
            ))
        }
        Expr::Match(match_expr) => {
            let scrutinee = build_typed_expr(env, &match_expr.scrutinee)?;
            let mut arm_effects = vec![scrutinee.effects.clone()];
            let mut arms = Vec::new();
            let scrutinee_type = synthesize(env, &match_expr.scrutinee)?;
            for arm in &match_expr.arms {
                let mut bindings = HashMap::new();
                check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)?;
                let arm_env = env.extend(Some(bindings));
                let guard = arm
                    .guard
                    .as_ref()
                    .map(|g| build_typed_expr(&arm_env, g).map(Box::new))
                    .transpose()?;
                let body = Box::new(build_typed_expr(&arm_env, &arm.body)?);
                if let Some(ref guard_expr) = guard {
                    arm_effects.push(guard_expr.effects.clone());
                }
                arm_effects.push(body.effects.clone());
                arms.push(TypedMatchArm {
                    pattern: arm.pattern.clone(),
                    guard,
                    body,
                    location: arm.location,
                });
            }
            Ok(typed_expr(
                TypedExprKind::Match(TypedMatchExpr {
                    scrutinee: Box::new(scrutinee),
                    arms,
                }),
                typ,
                merge_effects(arm_effects),
                StrictnessClass::Strict,
                match_expr.location,
            ))
        }
        Expr::List(list) => {
            let elements = list
                .elements
                .iter()
                .map(|element| build_typed_expr(env, element))
                .collect::<Result<Vec<_>, _>>()?;
            let effects = merge_effects(elements.iter().map(|element| element.effects.clone()));
            Ok(typed_expr(
                TypedExprKind::List(TypedListExpr { elements }),
                typ,
                effects,
                StrictnessClass::Deferred,
                list.location,
            ))
        }
        Expr::Tuple(tuple) => {
            let elements = tuple
                .elements
                .iter()
                .map(|element| build_typed_expr(env, element))
                .collect::<Result<Vec<_>, _>>()?;
            let effects = merge_effects(elements.iter().map(|element| element.effects.clone()));
            Ok(typed_expr(
                TypedExprKind::Tuple(TypedTupleExpr { elements }),
                typ,
                effects,
                StrictnessClass::Deferred,
                tuple.location,
            ))
        }
        Expr::Record(record) => {
            let fields = record
                .fields
                .iter()
                .map(|field| {
                    Ok(TypedRecordField {
                        name: field.name.clone(),
                        value: build_typed_expr(env, &field.value)?,
                        location: field.location,
                    })
                })
                .collect::<Result<Vec<_>, TypeError>>()?;
            let effects = merge_effects(fields.iter().map(|field| field.value.effects.clone()));
            Ok(typed_expr(
                TypedExprKind::Record(TypedRecordExpr { fields }),
                typ,
                effects,
                StrictnessClass::Deferred,
                record.location,
            ))
        }
        Expr::MapLiteral(map) => {
            let entries = map
                .entries
                .iter()
                .map(|entry| {
                    Ok(TypedMapEntryExpr {
                        key: build_typed_expr(env, &entry.key)?,
                        value: build_typed_expr(env, &entry.value)?,
                        location: entry.location,
                    })
                })
                .collect::<Result<Vec<_>, TypeError>>()?;
            let effects = merge_effects(
                entries
                    .iter()
                    .flat_map(|entry| [entry.key.effects.clone(), entry.value.effects.clone()]),
            );
            Ok(typed_expr(
                TypedExprKind::MapLiteral(TypedMapLiteralExpr { entries }),
                typ,
                effects,
                StrictnessClass::Deferred,
                map.location,
            ))
        }
        Expr::FieldAccess(field_access) => {
            let object = build_typed_expr(env, &field_access.object)?;
            let effects = object.effects.clone();
            Ok(typed_expr(
                TypedExprKind::FieldAccess(TypedFieldAccessExpr {
                    object: Box::new(object),
                    field: field_access.field.clone(),
                }),
                typ,
                effects,
                StrictnessClass::Strict,
                field_access.location,
            ))
        }
        Expr::Index(index_expr) => {
            let object = build_typed_expr(env, &index_expr.object)?;
            let index = build_typed_expr(env, &index_expr.index)?;
            let effects = merge_effects([object.effects.clone(), index.effects.clone()]);
            Ok(typed_expr(
                TypedExprKind::Index(TypedIndexExpr {
                    object: Box::new(object),
                    index: Box::new(index),
                }),
                typ,
                effects,
                StrictnessClass::Strict,
                index_expr.location,
            ))
        }
        Expr::Map(map_expr) => {
            let list = build_typed_expr(env, &map_expr.list)?;
            let func = build_typed_expr(env, &map_expr.func)?;
            let effects = merge_effects([list.effects.clone(), func.effects.clone()]);
            Ok(typed_expr(
                TypedExprKind::Map(TypedMapExpr {
                    list: Box::new(list),
                    func: Box::new(func),
                }),
                typ,
                effects,
                StrictnessClass::Deferred,
                map_expr.location,
            ))
        }
        Expr::Filter(filter_expr) => {
            let list = build_typed_expr(env, &filter_expr.list)?;
            let predicate = build_typed_expr(env, &filter_expr.predicate)?;
            let effects = merge_effects([list.effects.clone(), predicate.effects.clone()]);
            Ok(typed_expr(
                TypedExprKind::Filter(TypedFilterExpr {
                    list: Box::new(list),
                    predicate: Box::new(predicate),
                }),
                typ,
                effects,
                StrictnessClass::Deferred,
                filter_expr.location,
            ))
        }
        Expr::Fold(fold_expr) => {
            let list = build_typed_expr(env, &fold_expr.list)?;
            let func = build_typed_expr(env, &fold_expr.func)?;
            let init = build_typed_expr(env, &fold_expr.init)?;
            let effects = merge_effects([
                list.effects.clone(),
                func.effects.clone(),
                init.effects.clone(),
            ]);
            Ok(typed_expr(
                TypedExprKind::Fold(TypedFoldExpr {
                    list: Box::new(list),
                    func: Box::new(func),
                    init: Box::new(init),
                }),
                typ,
                effects,
                StrictnessClass::Strict,
                fold_expr.location,
            ))
        }
        Expr::Concurrent(concurrent_expr) => build_typed_concurrent(env, concurrent_expr, typ),
        Expr::Pipeline(pipeline) => {
            let left = build_typed_expr(env, &pipeline.left)?;
            let right = build_typed_expr(env, &pipeline.right)?;
            let effects = merge_effects([left.effects.clone(), right.effects.clone()]);
            Ok(typed_expr(
                TypedExprKind::Pipeline(TypedPipelineExpr {
                    left: Box::new(left),
                    operator: pipeline.operator,
                    right: Box::new(right),
                }),
                typ,
                effects,
                StrictnessClass::Deferred,
                pipeline.location,
            ))
        }
        Expr::WithMock(with_mock) => {
            let replacement = build_typed_expr(env, &with_mock.replacement)?;
            let body = build_typed_expr(env, &with_mock.body)?;
            let target = match &with_mock.target {
                Expr::Identifier(id) => WithMockTarget::LocalFunction(id.name.clone()),
                Expr::MemberAccess(member_access) => WithMockTarget::ExternMember {
                    namespace: member_access.namespace.clone(),
                    member: member_access.member.clone(),
                    mock_key: format!(
                        "extern:{}.{}",
                        member_access.namespace.join("/"),
                        member_access.member
                    ),
                },
                _ => {
                    return Err(TypeError::new(
                        "withMock target must be an identifier or imported member access"
                            .to_string(),
                        Some(with_mock.location),
                    ))
                }
            };
            Ok(typed_expr(
                TypedExprKind::WithMock(TypedWithMockExpr {
                    target,
                    replacement: Box::new(replacement.clone()),
                    body: Box::new(body.clone()),
                }),
                typ,
                merge_effects([replacement.effects, body.effects]),
                StrictnessClass::Deferred,
                with_mock.location,
            ))
        }
        Expr::TypeAscription(type_asc) => {
            let ascribed_type =
                ast_type_to_inference_type_resolved(env, None, &type_asc.ascribed_type)?;
            match &type_asc.expr {
                Expr::MapLiteral(map_expr) if map_expr.entries.is_empty() => Ok(typed_expr(
                    TypedExprKind::MapLiteral(TypedMapLiteralExpr {
                        entries: Vec::new(),
                    }),
                    ascribed_type,
                    HashSet::new(),
                    StrictnessClass::Deferred,
                    type_asc.location,
                )),
                _ => {
                    let mut inner = build_typed_expr(env, &type_asc.expr)?;
                    inner.typ = ascribed_type;
                    Ok(inner)
                }
            }
        }
    }
}

fn build_typed_concurrent(
    env: &TypeEnvironment,
    concurrent_expr: &sigil_ast::ConcurrentExpr,
    typ: InferenceType,
) -> Result<TypedExpr, TypeError> {
    let config = TypedConcurrentConfig {
        jitter_ms: concurrent_expr
            .policy
            .as_ref()
            .and_then(|policy| policy.fields.iter().find(|field| field.name == "jitterMs"))
            .map(|field| build_typed_expr(env, &field.value).map(Box::new))
            .transpose()?,
        stop_on: concurrent_expr
            .policy
            .as_ref()
            .and_then(|policy| policy.fields.iter().find(|field| field.name == "stopOn"))
            .map(|field| build_typed_expr(env, &field.value).map(Box::new))
            .transpose()?,
        width: Box::new(build_typed_expr(env, &concurrent_expr.width)?),
        window_ms: concurrent_expr
            .policy
            .as_ref()
            .and_then(|policy| policy.fields.iter().find(|field| field.name == "windowMs"))
            .map(|field| build_typed_expr(env, &field.value).map(Box::new))
            .transpose()?,
    };

    let mut effect_sets = vec![config.width.effects.clone()];
    if let Some(jitter_ms) = &config.jitter_ms {
        effect_sets.push(jitter_ms.effects.clone());
    }
    if let Some(stop_on) = &config.stop_on {
        effect_sets.push(stop_on.effects.clone());
    }
    if let Some(window_ms) = &config.window_ms {
        effect_sets.push(window_ms.effects.clone());
    }
    let mut effects = merge_effects(effect_sets);

    let mut steps = Vec::new();
    for step in &concurrent_expr.steps {
        match step {
            sigil_ast::ConcurrentStep::Spawn(spawn) => {
                let expr = build_typed_expr(env, &spawn.expr)?;
                effects.extend(expr.effects.clone());
                steps.push(TypedConcurrentStep::Spawn(TypedSpawnStep {
                    expr: Box::new(expr),
                    location: spawn.location,
                }));
            }
            sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                let list = build_typed_expr(env, &spawn_each.list)?;
                let func = build_typed_expr(env, &spawn_each.func)?;
                effects.extend(list.effects.clone());
                effects.extend(func.effects.clone());
                steps.push(TypedConcurrentStep::SpawnEach(TypedSpawnEachStep {
                    func: Box::new(func),
                    list: Box::new(list),
                    location: spawn_each.location,
                }));
            }
        }
    }

    Ok(typed_expr(
        TypedExprKind::Concurrent(TypedConcurrentExpr {
            config,
            name: concurrent_expr.name.clone(),
            steps,
        }),
        typ,
        effects,
        StrictnessClass::Deferred,
        concurrent_expr.location,
    ))
}

fn build_typed_application(
    env: &TypeEnvironment,
    app: &sigil_ast::ApplicationExpr,
    typ: InferenceType,
) -> Result<TypedExpr, TypeError> {
    let args = app
        .args
        .iter()
        .map(|arg| build_typed_expr(env, arg))
        .collect::<Result<Vec<_>, _>>()?;

    if let Expr::MemberAccess(member_access) = &app.func {
        if member_access
            .member
            .chars()
            .next()
            .is_some_and(|ch| ch.is_uppercase())
        {
            let effects = merge_effects(args.iter().map(|arg| arg.effects.clone()));
            return Ok(typed_expr(
                TypedExprKind::ConstructorCall(TypedConstructorCallExpr {
                    module_path: Some(member_access.namespace.clone()),
                    constructor: member_access.member.clone(),
                    args,
                }),
                typ,
                effects,
                StrictnessClass::Deferred,
                app.location,
            ));
        }

        let mut effects = merge_effects(args.iter().map(|arg| arg.effects.clone()));
        if let InferenceType::Function(tfunc) = synthesize_member_access(env, member_access)? {
            effects.extend(effects_option_to_set(&tfunc.effects));
        }
        return Ok(typed_expr(
            TypedExprKind::ExternCall(TypedExternCallExpr {
                namespace: member_access.namespace.clone(),
                member: member_access.member.clone(),
                mock_key: format!(
                    "extern:{}.{}",
                    member_access.namespace.join("/"),
                    member_access.member
                ),
                args,
            }),
            typ,
            effects,
            StrictnessClass::Deferred,
            app.location,
        ));
    }

    if let Expr::Identifier(id) = &app.func {
        if id.name.chars().next().is_some_and(|ch| ch.is_uppercase()) {
            let effects = merge_effects(args.iter().map(|arg| arg.effects.clone()));
            return Ok(typed_expr(
                TypedExprKind::ConstructorCall(TypedConstructorCallExpr {
                    module_path: None,
                    constructor: id.name.clone(),
                    args,
                }),
                typ,
                effects,
                StrictnessClass::Deferred,
                app.location,
            ));
        }
    }

    if let Expr::FieldAccess(field_access) = &app.func {
        let receiver = build_typed_expr(env, &field_access.object)?;
        let mut effects = merge_effects(args.iter().map(|arg| arg.effects.clone()));
        effects.extend(receiver.effects.clone());
        if let InferenceType::Function(tfunc) = synthesize_field_access(env, field_access)? {
            effects.extend(effects_option_to_set(&tfunc.effects));
        }
        return Ok(typed_expr(
            TypedExprKind::MethodCall(TypedMethodCallExpr {
                receiver: Box::new(receiver),
                selector: MethodSelector::Field(field_access.field.clone()),
                args,
            }),
            typ,
            effects,
            StrictnessClass::Deferred,
            app.location,
        ));
    }

    if let Expr::Index(index_expr) = &app.func {
        let receiver = build_typed_expr(env, &index_expr.object)?;
        let index = build_typed_expr(env, &index_expr.index)?;
        let mut effects = merge_effects(args.iter().map(|arg| arg.effects.clone()));
        effects.extend(receiver.effects.clone());
        effects.extend(index.effects.clone());
        return Ok(typed_expr(
            TypedExprKind::MethodCall(TypedMethodCallExpr {
                receiver: Box::new(receiver),
                selector: MethodSelector::Index(Box::new(index)),
                args,
            }),
            typ,
            effects,
            StrictnessClass::Deferred,
            app.location,
        ));
    }

    let func = build_typed_expr(env, &app.func)?;
    let mut effects = merge_effects(args.iter().map(|arg| arg.effects.clone()));
    effects.extend(func.effects.clone());
    if let InferenceType::Function(tfunc) = &func.typ {
        effects.extend(effects_option_to_set(&tfunc.effects));
    }
    Ok(typed_expr(
        TypedExprKind::Call(TypedCallExpr {
            func: Box::new(func),
            args,
        }),
        typ,
        effects,
        StrictnessClass::Deferred,
        app.location,
    ))
}

// ============================================================================
// SYNTHESIS (⇒) - Infer type from expression
// ============================================================================

/// Synthesize (infer) type from expression
/// Returns the inferred type
fn synthesize(env: &TypeEnvironment, expr: &Expr) -> Result<InferenceType, TypeError> {
    match expr {
        Expr::Literal(lit) => Ok(synthesize_literal(lit)),

        Expr::Identifier(id) => env.lookup(&id.name).ok_or_else(|| {
            TypeError::new(format!("Unbound variable: {}", id.name), Some(id.location))
        }),

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

        Expr::MapLiteral(map_expr) => synthesize_map_literal(env, map_expr),

        Expr::FieldAccess(field_access) => synthesize_field_access(env, field_access),

        Expr::Index(index_expr) => synthesize_index(env, index_expr),

        Expr::MemberAccess(member_access) => synthesize_member_access(env, member_access),

        Expr::Map(map_expr) => synthesize_map(env, map_expr),

        Expr::Filter(filter_expr) => synthesize_filter(env, filter_expr),

        Expr::Fold(fold_expr) => synthesize_fold(env, fold_expr),

        Expr::Concurrent(concurrent_expr) => synthesize_concurrent(env, concurrent_expr),

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
        // Arithmetic operators: Int => Int => Int
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

        // Comparison operators: Int => Int => Bool
        BinaryOperator::Less
        | BinaryOperator::Greater
        | BinaryOperator::LessEq
        | BinaryOperator::GreaterEq => {
            check(env, &bin.left, &int_type)?;
            check(env, &bin.right, &int_type)?;
            Ok(bool_type)
        }

        // Equality operators: T => T => Bool (polymorphic)
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

        // Logical operators: Bool => Bool => Bool
        BinaryOperator::And | BinaryOperator::Or => {
            check(env, &bin.left, &bool_type)?;
            check(env, &bin.right, &bool_type)?;
            Ok(bool_type)
        }

        // String concatenation: String => String => String
        BinaryOperator::Append => {
            check(env, &bin.left, &string_type)?;
            check(env, &bin.right, &string_type)?;
            Ok(string_type)
        }

        // List append: [T] => [T] => [T]
        BinaryOperator::ListAppend => {
            let (normalized_left, normalized_right) = canonical_pair(env, &left_type, &right_type);

            match (&normalized_left, &normalized_right) {
                (InferenceType::List(_), InferenceType::List(_)) => {
                    let subst = unify(&normalized_left, &normalized_right).map_err(|_message| {
                        TypeError::new(
                            format!(
                                "Cannot concatenate lists of different types: {} and {}",
                                format_type(&normalized_left),
                                format_type(&normalized_right)
                            ),
                            Some(bin.location),
                        )
                    })?;
                    Ok(apply_subst(&subst, &normalized_left))
                }
                _ => Err(TypeError::new(
                    format!(
                        "List append requires list operands, got {} and {}",
                        format_type(&normalized_left),
                        format_type(&normalized_right)
                    ),
                    Some(bin.location),
                )),
            }
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
            // Length operator # - works on strings, lists, and maps
            let operand_type = synthesize(env, &un.operand)?;
            match operand_type {
                InferenceType::Primitive(ref p) if p.name == PrimitiveName::String => Ok(int_type),
                InferenceType::List(_) => Ok(int_type),
                InferenceType::Map(_) => Ok(int_type),
                _ => Err(TypeError::new(
                    format!(
                        "Length operator # requires string, list, or map, got {}",
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
    validate_topology_application(env, app)?;

    let raw_fn_type = synthesize(env, &app.func)?;
    let fn_type = env.normalize_type(&raw_fn_type);

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
            let mut subst = HashMap::new();
            for (arg, param_type) in app.args.iter().zip(&tfunc.params) {
                let arg_type = synthesize(env, arg)?;
                let (normalized_arg, normalized_param) = canonical_pair(env, &arg_type, param_type);
                let next_subst = unify(&normalized_arg, &normalized_param).map_err(|message| {
                    TypeError::new(
                        format!(
                            "Function argument type mismatch: expected {}, got {} ({})",
                            format_type(&normalized_param),
                            format_type(&normalized_arg),
                            message
                        ),
                        Some(app.location),
                    )
                })?;
                subst.extend(next_subst);
            }

            Ok(apply_subst(&subst, &tfunc.return_type))
        }
        _ => Err(TypeError::new(
            format!("Expected function type, got {}", format_type(&fn_type)),
            Some(app.location),
        )),
    }
}

fn is_canonical_topology_source(env: &TypeEnvironment) -> bool {
    env.source_file()
        .map(|path| path.replace('\\', "/").ends_with("/src/topology.lib.sigil"))
        .unwrap_or(false)
}

fn is_canonical_config_source(env: &TypeEnvironment) -> bool {
    env.source_file()
        .map(|path| {
            let normalized = path.replace('\\', "/");
            normalized.contains("/config/") && normalized.ends_with(".lib.sigil")
        })
        .unwrap_or(false)
}

fn topology_call_member(expr: &Expr) -> Option<(&[String], &str)> {
    if let Expr::MemberAccess(member_access) = expr {
        return Some((&member_access.namespace, member_access.member.as_str()));
    }

    None
}

fn field_access_starts_with_process_env(field_access: &sigil_ast::FieldAccessExpr) -> bool {
    match &field_access.object {
        Expr::Identifier(identifier) => identifier.name == "process" && field_access.field == "env",
        Expr::FieldAccess(parent) => field_access_starts_with_process_env(parent),
        _ => false,
    }
}

fn is_http_dependency_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".HttpServiceDependency") || tcons.name == "HttpServiceDependency")
}

fn is_tcp_dependency_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".TcpServiceDependency") || tcons.name == "TcpServiceDependency")
}

fn validate_topology_application(
    env: &TypeEnvironment,
    app: &sigil_ast::ApplicationExpr,
) -> Result<(), TypeError> {
    let Some((namespace, member)) = topology_call_member(&app.func) else {
        return Ok(());
    };

    let module_id = namespace.join("::");

    if module_id == "stdlib::topology" {
        let restricted = matches!(member, "httpService" | "tcpService" | "environment");

        if restricted && !is_canonical_topology_source(env) {
            return Err(TypeError::new(
                format!(
                    "{}: topology declarations must live in src::topology via src/topology.lib.sigil",
                    codes::topology::CONSTRUCTOR_LOCATION
                ),
                Some(app.location),
            ));
        }

        return Ok(());
    }

    if module_id == "stdlib::config" {
        let restricted = matches!(
            member,
            "bindings" | "bindHttp" | "bindHttpEnv" | "bindTcp" | "bindTcpEnv"
        );

        if restricted && !is_canonical_config_source(env) {
            return Err(TypeError::new(
                format!(
                    "{}: config bindings must live in config/*.lib.sigil",
                    codes::topology::CONSTRUCTOR_LOCATION
                ),
                Some(app.location),
            ));
        }

        return Ok(());
    }

    let http_handle_arg_index = if module_id == "stdlib::httpClient" {
        match member {
            "get" | "getJson" | "delete" | "deleteJson" => Some(0),
            "post" | "postJson" | "put" | "putJson" | "patch" | "patchJson" => Some(1),
            _ => None,
        }
    } else {
        None
    };
    let tcp_handle_arg_index =
        if module_id == "stdlib::tcpClient" && matches!(member, "request" | "send") {
            Some(0)
        } else {
            None
        };

    if http_handle_arg_index.is_none() && tcp_handle_arg_index.is_none() {
        return Ok(());
    }

    let handle_index = http_handle_arg_index.or(tcp_handle_arg_index).unwrap();
    let Some(handle_arg) = app.args.get(handle_index) else {
        return Ok(());
    };
    let handle_type = env.normalize_type(&synthesize(env, handle_arg)?);

    if http_handle_arg_index.is_some() {
        if matches!(handle_arg, Expr::Literal(_)) {
            return Err(TypeError::new(
                format!(
                    "{}: stdlib::httpClient calls must use src::topology dependency handles, not raw URLs",
                    codes::topology::RAW_ENDPOINT_FORBIDDEN
                ),
                Some(app.location),
            ));
        }

        if !is_http_dependency_type(&handle_type) {
            let code = if is_tcp_dependency_type(&handle_type) {
                codes::topology::DEPENDENCY_KIND_MISMATCH
            } else {
                codes::topology::INVALID_HANDLE
            };
            return Err(TypeError::new(
                format!(
                    "{}: stdlib::httpClient requires a HttpServiceDependency from src::topology as its first argument",
                    code
                ),
                Some(app.location),
            ));
        }
    }

    if tcp_handle_arg_index.is_some() {
        if matches!(handle_arg, Expr::Literal(_)) {
            return Err(TypeError::new(
                format!(
                    "{}: stdlib::tcpClient calls must use src::topology dependency handles, not raw hosts or ports",
                    codes::topology::RAW_ENDPOINT_FORBIDDEN
                ),
                Some(app.location),
            ));
        }

        if !is_tcp_dependency_type(&handle_type) {
            let code = if is_http_dependency_type(&handle_type) {
                codes::topology::DEPENDENCY_KIND_MISMATCH
            } else {
                codes::topology::INVALID_HANDLE
            };
            return Err(TypeError::new(
                format!(
                    "{}: stdlib::tcpClient requires a TcpServiceDependency from src::topology as its first argument",
                    code
                ),
                Some(app.location),
            ));
        }
    }

    Ok(())
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
    check_pattern(
        env,
        &first_arm.pattern,
        &scrutinee_type,
        &mut first_bindings,
    )?;
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
                    "Pattern guard must have type Bool, got {}",
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
                        "Pattern guard must have type Bool, got {}",
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

fn synthesize_map_literal(
    env: &TypeEnvironment,
    map_expr: &sigil_ast::MapLiteralExpr,
) -> Result<InferenceType, TypeError> {
    if map_expr.entries.is_empty() {
        return Err(TypeError::new(
            "Cannot infer empty map literal type. Add contextual type information for {↦}."
                .to_string(),
            Some(map_expr.location),
        ));
    }

    let key_type = synthesize(env, &map_expr.entries[0].key)?;
    let value_type = synthesize(env, &map_expr.entries[0].value)?;

    for entry in map_expr.entries.iter().skip(1) {
        check(env, &entry.key, &key_type)?;
        check(env, &entry.value, &value_type)?;
    }

    Ok(InferenceType::Map(Box::new(TMap {
        key_type,
        value_type,
    })))
}

fn synthesize_field_access(
    env: &TypeEnvironment,
    field_access: &sigil_ast::FieldAccessExpr,
) -> Result<InferenceType, TypeError> {
    if field_access_starts_with_process_env(field_access) && !is_canonical_config_source(env) {
        return Err(TypeError::new(
            format!(
                "{}: process.env access is only allowed in config/*.lib.sigil",
                codes::topology::ENV_ACCESS_LOCATION
            ),
            Some(field_access.location),
        ));
    }

    let obj_type = synthesize(env, &field_access.object)?;

    // Special case: field access on 'any' type (FFI namespace)
    if matches!(obj_type, InferenceType::Any) {
        return Ok(InferenceType::Any);
    }

    // Normalize the type to resolve type aliases (e.g., EmailParts -> {local:String,domain:String})
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
            format!("Cannot index into non-list type {}", format_type(&obj_type)),
            Some(index_expr.location),
        )),
    }
}

fn synthesize_member_access(
    env: &TypeEnvironment,
    member_access: &sigil_ast::MemberAccessExpr,
) -> Result<InferenceType, TypeError> {
    let namespace_name = member_access.namespace.join("::");

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

    if let Some(constructor_type) =
        lookup_constructor_type(env, &member_access.namespace, &member_access.member)?
    {
        return Ok(constructor_type);
    }

    if let Some(member_type) =
        env.lookup_qualified_value(&member_access.namespace, &member_access.member)
    {
        return Ok(member_type);
    }

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
            format!("map requires a list, got {}", format_type(&list_type)),
            Some(map_expr.location),
        ));
    }

    let fn_type = env.normalize_type(&synthesize(env, &map_expr.func)?);

    if !matches!(fn_type, InferenceType::Function(_)) {
        return Err(TypeError::new(
            format!("map requires a function, got {}", format_type(&fn_type)),
            Some(map_expr.location),
        ));
    }

    if let (InferenceType::List(ref list), InferenceType::Function(ref func)) =
        (&list_type, &fn_type)
    {
        if func
            .effects
            .as_ref()
            .is_some_and(|effects| !effects.is_empty())
        {
            return Err(TypeError::new(
                "map callback must be pure. Sigil treats map as a canonical data-parallel operator, so effectful callbacks are not allowed.".to_string(),
                Some(map_expr.location),
            ));
        }

        // Function should take 1 parameter
        if func.params.len() != 1 {
            return Err(TypeError::new(
                format!(
                    "map function should take 1 parameter, got {}",
                    func.params.len()
                ),
                Some(map_expr.location),
            ));
        }

        // Check function parameter matches list element type
        let (normalized_param, normalized_elem) =
            canonical_pair(env, &func.params[0], &list.element_type);
        if !types_equal(&normalized_param, &normalized_elem) {
            return Err(TypeError::new(
                format!(
                    "map function parameter type {} doesn't match list element type {}",
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
            format!("filter requires a list, got {}", format_type(&list_type)),
            Some(filter_expr.location),
        ));
    }

    let predicate_type = env.normalize_type(&synthesize(env, &filter_expr.predicate)?);

    if !matches!(predicate_type, InferenceType::Function(_)) {
        return Err(TypeError::new(
            format!(
                "filter requires a predicate function, got {}",
                format_type(&predicate_type)
            ),
            Some(filter_expr.location),
        ));
    }

    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });

    if let (InferenceType::List(ref list), InferenceType::Function(ref pred)) =
        (&list_type, &predicate_type)
    {
        if pred
            .effects
            .as_ref()
            .is_some_and(|effects| !effects.is_empty())
        {
            return Err(TypeError::new(
                "filter predicate must be pure. Sigil treats filter as a canonical data-parallel operator, so effectful callbacks are not allowed.".to_string(),
                Some(filter_expr.location),
            ));
        }

        // Predicate should be T => Bool
        if pred.params.len() != 1 {
            return Err(TypeError::new(
                format!(
                    "filter predicate should take 1 parameter, got {}",
                    pred.params.len()
                ),
                Some(filter_expr.location),
            ));
        }

        let (normalized_param, normalized_elem) =
            canonical_pair(env, &pred.params[0], &list.element_type);
        if !types_equal(&normalized_param, &normalized_elem) {
            return Err(TypeError::new(
                format!(
                    "filter predicate parameter type {} doesn't match list element type {}",
                    format_type(&normalized_param),
                    format_type(&normalized_elem)
                ),
                Some(filter_expr.location),
            ));
        }

        let (normalized_return, normalized_bool) =
            canonical_pair(env, &pred.return_type, &bool_type);
        if !types_equal(&normalized_return, &normalized_bool) {
            return Err(TypeError::new(
                format!(
                    "filter predicate must return Bool, got {}",
                    format_type(&normalized_return)
                ),
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
            format!("reduce requires a list, got {}", format_type(&list_type)),
            Some(fold_expr.location),
        ));
    }

    let fn_type = env.normalize_type(&synthesize(env, &fold_expr.func)?);

    if !matches!(fn_type, InferenceType::Function(_)) {
        return Err(TypeError::new(
            format!("reduce requires a function, got {}", format_type(&fn_type)),
            Some(fold_expr.location),
        ));
    }

    let init_type = synthesize(env, &fold_expr.init)?;

    if let (InferenceType::List(ref list), InferenceType::Function(ref func)) =
        (&list_type, &fn_type)
    {
        // Function should be (Acc, T) => Acc
        if func.params.len() != 2 {
            return Err(TypeError::new(
                format!(
                    "reduce function should take 2 parameters, got {}",
                    func.params.len()
                ),
                Some(fold_expr.location),
            ));
        }

        // Check function signature matches (Acc, T) => Acc
        let (normalized_acc_param, normalized_init) =
            canonical_pair(env, &func.params[0], &init_type);
        if !types_equal(&normalized_acc_param, &normalized_init) {
            return Err(TypeError::new(
                format!(
                    "reduce function first parameter type {} doesn't match initial value type {}",
                    format_type(&normalized_acc_param),
                    format_type(&normalized_init)
                ),
                Some(fold_expr.location),
            ));
        }

        let (normalized_elem_param, normalized_elem) =
            canonical_pair(env, &func.params[1], &list.element_type);
        if !types_equal(&normalized_elem_param, &normalized_elem) {
            return Err(TypeError::new(
                format!(
                    "reduce function second parameter type {} doesn't match list element type {}",
                    format_type(&normalized_elem_param),
                    format_type(&normalized_elem)
                ),
                Some(fold_expr.location),
            ));
        }

        let (normalized_return, normalized_init) =
            canonical_pair(env, &func.return_type, &init_type);
        if !types_equal(&normalized_return, &normalized_init) {
            return Err(TypeError::new(
                format!(
                    "reduce function return type {} doesn't match accumulator type {}",
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

fn synthesize_concurrent(
    env: &TypeEnvironment,
    concurrent_expr: &sigil_ast::ConcurrentExpr,
) -> Result<InferenceType, TypeError> {
    let (jitter_ms_expr, stop_on_expr, window_ms_expr) =
        concurrent_policy_fields(concurrent_expr.policy.as_ref(), concurrent_expr.location)?;

    check(env, &concurrent_expr.width, &int_type())?;
    if let Some(jitter_ms_expr) = jitter_ms_expr {
        check(
            env,
            jitter_ms_expr,
            &option_type(concurrent_jitter_record_type()),
        )?;
    }
    if let Some(window_ms_expr) = window_ms_expr {
        check(env, window_ms_expr, &option_type(int_type()))?;
    }

    if concurrent_expr.steps.is_empty() {
        return Err(TypeError::new(
            "Concurrent region must contain at least one spawn or spawnEach step".to_string(),
            Some(concurrent_expr.location),
        ));
    }

    let mut common_success_type: Option<InferenceType> = None;
    let mut common_error_type: Option<InferenceType> = None;

    for step in &concurrent_expr.steps {
        match step {
            sigil_ast::ConcurrentStep::Spawn(spawn) => {
                let typed_expr = build_typed_expr(env, &spawn.expr)?;
                if typed_expr.effects.is_empty() {
                    return Err(TypeError::new(
                        "spawn requires an effectful computation returning Result[T,E]".to_string(),
                        Some(spawn.location),
                    ));
                }

                let Some((success_type, error_type)) = result_type_parts(env, &typed_expr.typ)
                else {
                    return Err(TypeError::new(
                        format!(
                            "spawn requires a Result[T,E] computation, got {}",
                            format_type(&typed_expr.typ)
                        ),
                        Some(spawn.location),
                    ));
                };

                if let Some(common_success) = &common_success_type {
                    if !same_type(env, common_success, &success_type) {
                        return Err(TypeError::new(
                            format!(
                                "Concurrent region child success types must match, found {} and {}",
                                format_type(common_success),
                                format_type(&success_type)
                            ),
                            Some(spawn.location),
                        ));
                    }
                } else {
                    common_success_type = Some(success_type);
                }

                if let Some(common_error) = &common_error_type {
                    if !same_type(env, common_error, &error_type) {
                        return Err(TypeError::new(
                            format!(
                                "Concurrent region child error types must match, found {} and {}",
                                format_type(common_error),
                                format_type(&error_type)
                            ),
                            Some(spawn.location),
                        ));
                    }
                } else {
                    common_error_type = Some(error_type);
                }
            }
            sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                let list_type = env.normalize_type(&synthesize(env, &spawn_each.list)?);
                let InferenceType::List(list) = list_type else {
                    return Err(TypeError::new(
                        format!(
                            "spawnEach requires a list, got {}",
                            format_type(&synthesize(env, &spawn_each.list)?)
                        ),
                        Some(spawn_each.location),
                    ));
                };

                let fn_type = env.normalize_type(&synthesize(env, &spawn_each.func)?);
                let InferenceType::Function(func) = fn_type else {
                    return Err(TypeError::new(
                        format!(
                            "spawnEach requires a function, got {}",
                            format_type(&synthesize(env, &spawn_each.func)?)
                        ),
                        Some(spawn_each.location),
                    ));
                };

                if func
                    .effects
                    .as_ref()
                    .is_none_or(|effects| effects.is_empty())
                {
                    return Err(TypeError::new(
                        "spawnEach requires an effectful function returning Result[T,E]"
                            .to_string(),
                        Some(spawn_each.location),
                    ));
                }

                if func.params.len() != 1 {
                    return Err(TypeError::new(
                        format!(
                            "spawnEach function should take 1 parameter, got {}",
                            func.params.len()
                        ),
                        Some(spawn_each.location),
                    ));
                }

                if !same_type(env, &func.params[0], &list.element_type) {
                    return Err(TypeError::new(
                        format!(
                            "spawnEach function parameter type {} doesn't match list element type {}",
                            format_type(&func.params[0]),
                            format_type(&list.element_type)
                        ),
                        Some(spawn_each.location),
                    ));
                }

                let Some((success_type, error_type)) = result_type_parts(env, &func.return_type)
                else {
                    return Err(TypeError::new(
                        format!(
                            "spawnEach function must return Result[T,E], got {}",
                            format_type(&func.return_type)
                        ),
                        Some(spawn_each.location),
                    ));
                };

                if let Some(common_success) = &common_success_type {
                    if !same_type(env, common_success, &success_type) {
                        return Err(TypeError::new(
                            format!(
                                "Concurrent region child success types must match, found {} and {}",
                                format_type(common_success),
                                format_type(&success_type)
                            ),
                            Some(spawn_each.location),
                        ));
                    }
                } else {
                    common_success_type = Some(success_type);
                }

                if let Some(common_error) = &common_error_type {
                    if !same_type(env, common_error, &error_type) {
                        return Err(TypeError::new(
                            format!(
                                "Concurrent region child error types must match, found {} and {}",
                                format_type(common_error),
                                format_type(&error_type)
                            ),
                            Some(spawn_each.location),
                        ));
                    }
                } else {
                    common_error_type = Some(error_type);
                }
            }
        }
    }

    let success_type = common_success_type.unwrap();
    let error_type = common_error_type.unwrap();
    if let Some(stop_on_expr) = stop_on_expr {
        let stop_on_type = env.normalize_type(&synthesize(env, stop_on_expr)?);
        let InferenceType::Function(stop_on_fn) = stop_on_type else {
            return Err(TypeError::new(
                format!(
                    "Concurrent region stopOn must be a pure function, got {}",
                    format_type(&synthesize(env, stop_on_expr)?)
                ),
                Some(concurrent_expr.location),
            ));
        };

        if stop_on_fn
            .effects
            .as_ref()
            .is_some_and(|effects| !effects.is_empty())
        {
            return Err(TypeError::new(
                "Concurrent region stopOn must be pure".to_string(),
                Some(concurrent_expr.location),
            ));
        }

        if stop_on_fn.params.len() != 1 {
            return Err(TypeError::new(
                format!(
                    "Concurrent region stopOn must take 1 parameter, got {}",
                    stop_on_fn.params.len()
                ),
                Some(concurrent_expr.location),
            ));
        }

        if !same_type(env, &stop_on_fn.params[0], &error_type) {
            return Err(TypeError::new(
                format!(
                    "Concurrent region stopOn parameter type {} doesn't match child error type {}",
                    format_type(&stop_on_fn.params[0]),
                    format_type(&error_type)
                ),
                Some(concurrent_expr.location),
            ));
        }

        if !same_type(env, &stop_on_fn.return_type, &bool_type()) {
            return Err(TypeError::new(
                format!(
                    "Concurrent region stopOn must return Bool, got {}",
                    format_type(&stop_on_fn.return_type)
                ),
                Some(concurrent_expr.location),
            ));
        }
    }

    Ok(InferenceType::List(Box::new(crate::types::TList {
        element_type: concurrent_outcome_type(success_type, error_type),
    })))
}

fn synthesize_with_mock(
    env: &TypeEnvironment,
    with_mock: &sigil_ast::WithMockExpr,
) -> Result<InferenceType, TypeError> {
    // Check target/replacement compatibility.
    // Canonical placement rules are enforced earlier by validation.
    let target_type = synthesize(env, &with_mock.target)?;
    let replacement_type = synthesize(env, &with_mock.replacement)?;

    // Replacement must be a function
    if !matches!(
        replacement_type,
        InferenceType::Function(_) | InferenceType::Any
    ) {
        return Err(TypeError::new(
            format!(
                "withMock replacement must be a function, got {}",
                format_type(&replacement_type)
            ),
            Some(with_mock.location),
        ));
    }

    // If both are functions, check they match
    if let (InferenceType::Function(_), InferenceType::Function(_)) =
        (&target_type, &replacement_type)
    {
        let (normalized_target, normalized_replacement) =
            canonical_pair(env, &target_type, &replacement_type);
        if !types_equal(&normalized_target, &normalized_replacement) {
            return Err(TypeError::new(
                format!(
                    "withMock replacement type {} does not match target type {}",
                    format_type(&normalized_replacement),
                    format_type(&normalized_target)
                ),
                Some(with_mock.location),
            ));
        }
    }

    // TODO: Full target-kind validation beyond simple type compatibility.

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
            Some(ty) => ast_type_to_inference_type_resolved(env, None, ty),
            None => Ok(InferenceType::Any),
        })
        .collect::<Result<_, _>>()?;

    let return_type = ast_type_to_inference_type_resolved(env, None, &lambda_expr.return_type)?;

    let effects = if lambda_expr.effects.is_empty() {
        None
    } else {
        Some(resolve_effect_names(
            env,
            &lambda_expr.effects,
            lambda_expr.location,
            "lambda signature",
        )?)
    };

    // Create environment with parameter bindings
    let mut lambda_env_bindings = HashMap::new();
    for (param, param_type) in lambda_expr.params.iter().zip(&param_types) {
        lambda_env_bindings.insert(param.name.clone(), param_type.clone());
    }
    let lambda_env = env.extend(Some(lambda_env_bindings));

    // Check body against declared return type
    check(&lambda_env, &lambda_expr.body, &return_type)?;
    let typed_body = build_typed_expr(&lambda_env, &lambda_expr.body)?;
    declared_effects_cover_actual(
        env,
        &lambda_expr.effects,
        &typed_body.effects,
        lambda_expr.location,
        "Lambda",
    )?;

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

            let (normalized_lit, normalized_scrutinee) =
                canonical_pair(env, &lit_type, scrutinee_type);
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
            let constructor_type = lookup_constructor_type(
                env,
                &constructor_pattern.module_path,
                &constructor_pattern.name,
            )?;
            if constructor_type.is_none() {
                return Err(TypeError::new(
                    format!(
                        "Unknown constructor '{}'",
                        constructor_display_name(
                            &constructor_pattern.module_path,
                            &constructor_pattern.name
                        )
                    ),
                    Some(constructor_pattern.location),
                ));
            }

            let constructor_type = constructor_type.unwrap();

            // Constructor should be a function type
            if !matches!(constructor_type, InferenceType::Function(_)) {
                return Err(TypeError::new(
                    format!(
                        "'{}' is not a constructor",
                        constructor_display_name(
                            &constructor_pattern.module_path,
                            &constructor_pattern.name
                        )
                    ),
                    Some(constructor_pattern.location),
                ));
            }

            if let (
                InferenceType::Function(ref ctor_fn),
                InferenceType::Constructor(ref scrutinee_ctor),
            ) = (&constructor_type, scrutinee_type)
            {
                // Check that constructor's return type matches scrutinee type
                if let InferenceType::Constructor(ref return_ctor) = ctor_fn.return_type {
                    if return_ctor.name != scrutinee_ctor.name {
                        return Err(TypeError::new(
                            format!(
                                "Constructor '{}' returns '{}', expected '{}'",
                                constructor_display_name(
                                    &constructor_pattern.module_path,
                                    &constructor_pattern.name
                                ),
                                format_type(&ctor_fn.return_type),
                                scrutinee_ctor.name
                            ),
                            Some(constructor_pattern.location),
                        ));
                    }
                }

                let subst = unify(&ctor_fn.return_type, scrutinee_type).map_err(|message| {
                    TypeError::new(
                        format!(
                            "Constructor '{}' does not match scrutinee type {} ({})",
                            constructor_display_name(
                                &constructor_pattern.module_path,
                                &constructor_pattern.name
                            ),
                            format_type(scrutinee_type),
                            message
                        ),
                        Some(constructor_pattern.location),
                    )
                })?;

                // Check argument patterns against constructor parameter types
                let patterns = &constructor_pattern.patterns;
                if !patterns.is_empty() {
                    if patterns.len() != ctor_fn.params.len() {
                        return Err(TypeError::new(
                            format!(
                                "Constructor '{}' expects {} arguments, got {}",
                                constructor_display_name(
                                    &constructor_pattern.module_path,
                                    &constructor_pattern.name
                                ),
                                ctor_fn.params.len(),
                                patterns.len()
                            ),
                            Some(constructor_pattern.location),
                        ));
                    }

                    for (pattern, param_type) in patterns.iter().zip(&ctor_fn.params) {
                        let instantiated_param = apply_subst(&subst, param_type);
                        check_pattern(env, pattern, &instantiated_param, bindings)?;
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
    let ascribed_type = ast_type_to_inference_type_resolved(env, None, &type_asc.ascribed_type)?;

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

    if let Expr::MapLiteral(map_expr) = expr {
        if map_expr.entries.is_empty() {
            return match expected_type {
                InferenceType::Map(_) => Ok(()),
                _ => Err(TypeError::new(
                    format!(
                        "Empty map literal requires a map type context, got {}",
                        format_type(expected_type)
                    ),
                    None,
                )),
            };
        }
    }

    if let Expr::Application(app) = expr {
        return check_application(env, app, expected_type);
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
        if let Ok(subst) = unify(&normalized_actual, &normalized_expected) {
            let unified_actual = apply_subst(&subst, &normalized_actual);
            let unified_expected = apply_subst(&subst, &normalized_expected);
            if types_equal(&unified_actual, &unified_expected) {
                return Ok(());
            }
        }
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

fn check_application(
    env: &TypeEnvironment,
    app: &sigil_ast::ApplicationExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    let raw_fn_type = synthesize(env, &app.func)?;
    let fn_type = env.normalize_type(&raw_fn_type);

    if matches!(fn_type, InferenceType::Any) {
        return Ok(());
    }

    let InferenceType::Function(tfunc) = fn_type else {
        return Err(TypeError::new(
            format!("Expected function type, got {}", format_type(&fn_type)),
            Some(app.location),
        ));
    };

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

    let mut subst = HashMap::new();
    for (arg, param_type) in app.args.iter().zip(&tfunc.params) {
        let arg_type = synthesize(env, arg)?;
        let expected_param = apply_subst(&subst, param_type);
        let (normalized_arg, normalized_param) = canonical_pair(env, &arg_type, &expected_param);
        let next_subst = unify(&normalized_arg, &normalized_param).map_err(|message| {
            TypeError::new(
                format!(
                    "Function argument type mismatch: expected {}, got {} ({})",
                    format_type(&normalized_param),
                    format_type(&normalized_arg),
                    message
                ),
                Some(app.location),
            )
        })?;
        subst.extend(next_subst);
    }

    let actual_return = apply_subst(&subst, &tfunc.return_type);
    let (normalized_actual, normalized_expected) =
        canonical_pair(env, &actual_return, expected_type);
    let next_subst = unify(&normalized_actual, &normalized_expected).map_err(|message| {
        TypeError::new(
            format!(
                "Type mismatch: expected {}, got {} ({})",
                format_type(&normalized_expected),
                format_type(&normalized_actual),
                message
            ),
            Some(app.location),
        )
    })?;
    subst.extend(next_subst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;
    use std::collections::HashMap;

    fn synthetic_loc() -> sigil_ast::SourceLocation {
        sigil_ast::SourceLocation::single(sigil_ast::Position::new(1, 1, 0))
    }

    fn core_prelude_type_options() -> TypeCheckOptions {
        let concurrent_outcome_info = TypeInfo {
            type_params: vec!["T".to_string(), "E".to_string()],
            definition: TypeDef::Sum(sigil_ast::SumType {
                variants: vec![
                    sigil_ast::Variant {
                        name: "Aborted".to_string(),
                        types: vec![],
                        location: synthetic_loc(),
                    },
                    sigil_ast::Variant {
                        name: "Failure".to_string(),
                        types: vec![Type::Variable(sigil_ast::TypeVariable {
                            name: "E".to_string(),
                            location: synthetic_loc(),
                        })],
                        location: synthetic_loc(),
                    },
                    sigil_ast::Variant {
                        name: "Success".to_string(),
                        types: vec![Type::Variable(sigil_ast::TypeVariable {
                            name: "T".to_string(),
                            location: synthetic_loc(),
                        })],
                        location: synthetic_loc(),
                    },
                ],
                location: synthetic_loc(),
            }),
        };

        let option_info = TypeInfo {
            type_params: vec!["T".to_string()],
            definition: TypeDef::Sum(sigil_ast::SumType {
                variants: vec![
                    sigil_ast::Variant {
                        name: "Some".to_string(),
                        types: vec![Type::Variable(sigil_ast::TypeVariable {
                            name: "T".to_string(),
                            location: synthetic_loc(),
                        })],
                        location: synthetic_loc(),
                    },
                    sigil_ast::Variant {
                        name: "None".to_string(),
                        types: vec![],
                        location: synthetic_loc(),
                    },
                ],
                location: synthetic_loc(),
            }),
        };

        let result_info = TypeInfo {
            type_params: vec!["T".to_string(), "E".to_string()],
            definition: TypeDef::Sum(sigil_ast::SumType {
                variants: vec![
                    sigil_ast::Variant {
                        name: "Ok".to_string(),
                        types: vec![Type::Variable(sigil_ast::TypeVariable {
                            name: "T".to_string(),
                            location: synthetic_loc(),
                        })],
                        location: synthetic_loc(),
                    },
                    sigil_ast::Variant {
                        name: "Err".to_string(),
                        types: vec![Type::Variable(sigil_ast::TypeVariable {
                            name: "E".to_string(),
                            location: synthetic_loc(),
                        })],
                        location: synthetic_loc(),
                    },
                ],
                location: synthetic_loc(),
            }),
        };

        let prelude_registry = HashMap::from([
            (
                "ConcurrentOutcome".to_string(),
                concurrent_outcome_info.clone(),
            ),
            ("Option".to_string(), option_info.clone()),
            ("Result".to_string(), result_info.clone()),
        ]);

        let concurrent_outcome_aborted_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &concurrent_outcome_info.definition {
                TypeDef::Sum(sum) => &sum.variants[0],
                _ => unreachable!(),
            },
            &concurrent_outcome_info.type_params,
            "ConcurrentOutcome",
        )
        .unwrap();
        let concurrent_outcome_failure_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &concurrent_outcome_info.definition {
                TypeDef::Sum(sum) => &sum.variants[1],
                _ => unreachable!(),
            },
            &concurrent_outcome_info.type_params,
            "ConcurrentOutcome",
        )
        .unwrap();
        let concurrent_outcome_success_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &concurrent_outcome_info.definition {
                TypeDef::Sum(sum) => &sum.variants[2],
                _ => unreachable!(),
            },
            &concurrent_outcome_info.type_params,
            "ConcurrentOutcome",
        )
        .unwrap();
        let option_some_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &option_info.definition {
                TypeDef::Sum(sum) => &sum.variants[0],
                _ => unreachable!(),
            },
            &option_info.type_params,
            "Option",
        )
        .unwrap();
        let option_none_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &option_info.definition {
                TypeDef::Sum(sum) => &sum.variants[1],
                _ => unreachable!(),
            },
            &option_info.type_params,
            "Option",
        )
        .unwrap();
        let result_ok_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &result_info.definition {
                TypeDef::Sum(sum) => &sum.variants[0],
                _ => unreachable!(),
            },
            &result_info.type_params,
            "Result",
        )
        .unwrap();
        let result_err_type = create_constructor_type_with_result_name(
            &TypeEnvironment::new(),
            match &result_info.definition {
                TypeDef::Sum(sum) => &sum.variants[1],
                _ => unreachable!(),
            },
            &result_info.type_params,
            "Result",
        )
        .unwrap();

        let mut prelude_schemes = HashMap::new();
        for (name, typ) in [
            ("Aborted", concurrent_outcome_aborted_type),
            ("Failure", concurrent_outcome_failure_type),
            ("Success", concurrent_outcome_success_type),
            ("Some", option_some_type),
            ("None", option_none_type),
            ("Ok", result_ok_type),
            ("Err", result_err_type),
        ] {
            let mut quantified_vars = HashSet::new();
            collect_type_var_ids(&typ, &mut quantified_vars);
            prelude_schemes.insert(name.to_string(), explicit_scheme(&typ, &quantified_vars));
        }

        TypeCheckOptions {
            effect_catalog: None,
            imported_namespaces: None,
            imported_type_registries: Some(HashMap::from([(
                "core::prelude".to_string(),
                prelude_registry,
            )])),
            imported_value_schemes: Some(HashMap::from([(
                "core::prelude".to_string(),
                prelude_schemes,
            )])),
            source_file: None,
        }
    }

    #[test]
    fn test_simple_integer_function() {
        let source = "λadd(x:Int,y:Int)=>Int=x+y";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());

        let types = result.unwrap();
        assert_eq!(types.declaration_types.len(), 1);
        assert!(types.declaration_types.contains_key("add"));
    }

    #[test]
    fn test_type_mismatch() {
        let source = "λbad(x:Int)=>String=x";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_literal_types() {
        let source = "λf()=>Int=42";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_application() {
        let source = "λadd(x:Int,y:Int)=>Int=x+y\nλmain()=>Int=add(1,2)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_sum_type_constructors() {
        // Test that sum type constructors are registered and callable
        // Using fully specified constructor type for now
        let source = "t Color=Red|Green|Blue\nλgetRed()=>Color=Red()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        // Should succeed - Red is registered as a constructor
        assert!(result.is_ok());
    }

    #[test]
    fn test_any_is_rejected_outside_ffi() {
        let source = "t Response={headers:Any}\nλmain()=>Response={headers:{}}";
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
        let source = "t MkdirOptions={recursive:Bool}\nc opts=({recursive:true}:MkdirOptions)\nλmain()=>Unit=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_append_normalizes_named_product_type() {
        let source = "t Todo={done:Bool,id:Int,text:String}\nλmain()=>[Todo]=[{done:false,id:1,text:\"a\"}]⧺[Todo{done:false,id:2,text:\"b\"}]";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_normalizes_named_product_type() {
        let source = "t Todo={done:Bool,id:Int,text:String}\nλkeep(todo:Todo)=>Todo=todo\nλmain()=>[Todo]=[{done:false,id:1,text:\"a\"}] map keep";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_rejects_effectful_callback() {
        let source = "e console:{log:λ(String)=>!Log Unit}\nλdouble(x:Int)=>!Log Int={l _=(console.log(\"x\"):Unit);x*2}\nλmain()=>[Int]=[1,2,3] map double";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("map callback must be pure"));
    }

    #[test]
    fn test_filter_rejects_effectful_callback() {
        let source = "e console:{log:λ(String)=>!Log Unit}\nλkeep(x:Int)=>!Log Bool={l _=(console.log(\"x\"):Unit);x>0}\nλmain()=>[Int]=[1,2,3] filter keep";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("filter predicate must be pure"));
    }

    #[test]
    fn test_named_product_equality_uses_canonical_form() {
        let source = "t Todo={done:Bool,id:Int,text:String}\nλmain()=>Bool=(({done:false,id:1,text:\"a\"}:Todo)={done:false,id:1,text:\"a\"})";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_named_product_normalizes_inside_generic_constructor_args() {
        let source = "t Error={code:Int,msg:String}\nt Response={body:String,headers:{String↦String},status:Int}\nλmain()=>Result[Response,Error]=Ok(Response{body:\"OK\",headers:({↦}:{String↦String}),status:200})";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_sum_types_remain_nominal_after_normalization() {
        let source = "t Box={value:Int}\nt Wrap=Wrap(Box)\nλmain()=>Wrap=({value:1}:Wrap)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_qualified_imported_product_type_resolves_for_field_access() {
        let source = "i src::types\nλslug_len(meta:src::types.ArticleMeta)=>Int=#meta.slug";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut imported_type_registries = HashMap::new();
        imported_type_registries.insert(
            "src::types".to_string(),
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
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: Some(imported_type_registries),
                imported_value_schemes: None,
                source_file: None,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_local_named_product_return_type_resolves_for_field_access() {
        let source = "t ParseResult={content:String}\nλparse()=>ParseResult={content:\"x\"}\nλmain()=>Int=#(parse().content)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_exact_record_rejects_missing_field() {
        let source = "t Message={createdAt:String,text:String}\nλmain()=>Message={createdAt:\"2026-03-07T00:00:00.000Z\"}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_exact_record_rejects_extra_field() {
        let source = "t Message={createdAt:String,text:String}\nλmain()=>Message={createdAt:\"2026-03-07T00:00:00.000Z\",debug:\"no\",text:\"hello\"}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_exact_records_do_not_width_subtype() {
        let source = "t Message={createdAt:String,text:String}\nt Summary={text:String}\nλheadline(summary:Summary)=>String=summary.text\nλmain()=>String=headline(({createdAt:\"2026-03-07T00:00:00.000Z\",text:\"hello\"}:Message))";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("Function argument type mismatch"));
    }

    #[test]
    fn test_validated_wrapper_stays_distinct_from_primitive() {
        let source = "t UserId=UserId(Int)\nλmain()=>UserId=42";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_process_env_access_is_rejected_outside_config_modules() {
        let source = "e process\nλmain()=>String=(process.env.sigilSiteBasePath:String)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/main.sigil").unwrap();

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: None,
                imported_value_schemes: None,
                source_file: Some("/tmp/project/src/main.sigil".to_string()),
            },
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("process.env access is only allowed in config/*.lib.sigil"));
    }

    #[test]
    fn test_process_env_access_is_allowed_in_config_modules() {
        let source = "e process\nλmain()=>String=(process.env.sigilSiteBasePath:String)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "config/local.lib.sigil").unwrap();

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: None,
                imported_value_schemes: None,
                source_file: Some("/tmp/project/config/local.lib.sigil".to_string()),
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_alias_normalizes_for_application() {
        let source = "t Decoder[T]=λ(String)=>Result[T,String]\nλparseInt(text:String)=>Result[Int,String]=Ok(42)\nλrun(decoder:Decoder[Int],input:String)=>Result[Int,String]=decoder(input)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_qualified_imported_constructor_expression_typechecks() {
        let source =
            "i src::graphTypes\nλmk()=>src::graphTypes.TopologicalSortResult=src::graphTypes.Ordering([1,2,3])";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut imported_type_registries = HashMap::new();
        imported_type_registries.insert(
            "src::graphTypes".to_string(),
            HashMap::from([(
                "TopologicalSortResult".to_string(),
                TypeInfo {
                    type_params: vec![],
                    definition: TypeDef::Sum(sigil_ast::SumType {
                        variants: vec![
                            sigil_ast::Variant {
                                name: "CycleDetected".to_string(),
                                types: vec![],
                                location: synthetic_loc(),
                            },
                            sigil_ast::Variant {
                                name: "Ordering".to_string(),
                                types: vec![Type::List(Box::new(sigil_ast::ListType {
                                    element_type: Type::Primitive(sigil_ast::PrimitiveType {
                                        name: PrimitiveName::Int,
                                        location: synthetic_loc(),
                                    }),
                                    location: synthetic_loc(),
                                }))],
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
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: Some(imported_type_registries),
                imported_value_schemes: None,
                source_file: None,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_qualified_imported_constructor_pattern_typechecks() {
        let source = "i src::graphTypes\nλproject(result:src::graphTypes.TopologicalSortResult)=>[Int] match result{src::graphTypes.Ordering(order)=>order|src::graphTypes.CycleDetected()=>[]}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut imported_type_registries = HashMap::new();
        imported_type_registries.insert(
            "src::graphTypes".to_string(),
            HashMap::from([(
                "TopologicalSortResult".to_string(),
                TypeInfo {
                    type_params: vec![],
                    definition: TypeDef::Sum(sigil_ast::SumType {
                        variants: vec![
                            sigil_ast::Variant {
                                name: "CycleDetected".to_string(),
                                types: vec![],
                                location: synthetic_loc(),
                            },
                            sigil_ast::Variant {
                                name: "Ordering".to_string(),
                                types: vec![Type::List(Box::new(sigil_ast::ListType {
                                    element_type: Type::Primitive(sigil_ast::PrimitiveType {
                                        name: PrimitiveName::Int,
                                        location: synthetic_loc(),
                                    }),
                                    location: synthetic_loc(),
                                }))],
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
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: Some(imported_type_registries),
                imported_value_schemes: None,
                source_file: None,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_explicit_generic_function_typechecks() {
        let source = "λidentity[T](x:T)=>T=x\nλmain()=>Int=identity(42)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_imported_generic_constructor_typechecks() {
        let source = "i core::prelude\nλmain()=>Option[Int]=Some(42)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_prelude_result_helper_typechecks() {
        let source = "λnormalize[T,E](res:Result[T,E])=>Result[T,E] match res{Ok(value)=>Ok(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_prelude_bind_result_typechecks() {
        let source = "λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E] match res{Ok(value)=>fn(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_prelude_bind_result_call_expr_builds() {
        let source = "λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E] match res{Ok(value)=>fn(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let Declaration::Function(func_decl) = &program.declarations[0] else {
            panic!("expected function declaration");
        };
        let sigil_ast::Expr::Match(match_expr) = &func_decl.body else {
            panic!("expected match body");
        };
        let call_expr = &match_expr.arms[0].body;

        let options = core_prelude_type_options();
        let mut env = TypeEnvironment::new();
        if let Some(prelude_types) = options
            .imported_type_registries
            .as_ref()
            .and_then(|regs| regs.get("core::prelude"))
        {
            for (name, info) in prelude_types {
                env.register_type(name.clone(), info.clone());
            }
        }
        if let Some(prelude_schemes) = options
            .imported_value_schemes
            .as_ref()
            .and_then(|schemes| schemes.get("core::prelude"))
        {
            for (name, scheme) in prelude_schemes {
                env.bind_scheme(name.clone(), scheme.clone());
            }
        }

        let type_param_env = make_type_param_env(&func_decl.type_params);
        let fn_type = ast_type_to_inference_type_resolved(
            &env,
            Some(&type_param_env),
            func_decl.params[0].type_annotation.as_ref().unwrap(),
        )
        .unwrap();
        let value_type = type_param_env.get("T").unwrap().clone();

        let mut bindings = HashMap::new();
        bindings.insert("fn".to_string(), fn_type);
        bindings.insert("value".to_string(), value_type);
        let call_env = env.extend(Some(bindings));

        let result = build_typed_expr(&call_env, call_expr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_prelude_bind_result_match_expr_builds() {
        let source = "λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E] match res{Ok(value)=>fn(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let Declaration::Function(func_decl) = &program.declarations[0] else {
            panic!("expected function declaration");
        };

        let options = core_prelude_type_options();
        let mut env = TypeEnvironment::new();
        if let Some(prelude_types) = options
            .imported_type_registries
            .as_ref()
            .and_then(|regs| regs.get("core::prelude"))
        {
            for (name, info) in prelude_types {
                env.register_type(name.clone(), info.clone());
            }
        }
        if let Some(prelude_schemes) = options
            .imported_value_schemes
            .as_ref()
            .and_then(|schemes| schemes.get("core::prelude"))
        {
            for (name, scheme) in prelude_schemes {
                env.bind_scheme(name.clone(), scheme.clone());
            }
        }

        let type_param_env = make_type_param_env(&func_decl.type_params);
        let mut bindings = HashMap::new();
        for param in &func_decl.params {
            if let Some(ref ty) = param.type_annotation {
                bindings.insert(
                    param.name.clone(),
                    ast_type_to_inference_type_resolved(&env, Some(&type_param_env), ty).unwrap(),
                );
            }
        }
        let function_env = env.extend(Some(bindings));

        let result = build_typed_expr(&function_env, &func_decl.body);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_prelude_err_arm_expr_builds() {
        let source = "λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E] match res{Ok(value)=>fn(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let Declaration::Function(func_decl) = &program.declarations[0] else {
            panic!("expected function declaration");
        };
        let sigil_ast::Expr::Match(match_expr) = &func_decl.body else {
            panic!("expected match body");
        };
        let err_expr = &match_expr.arms[1].body;

        let options = core_prelude_type_options();
        let mut env = TypeEnvironment::new();
        if let Some(prelude_types) = options
            .imported_type_registries
            .as_ref()
            .and_then(|regs| regs.get("core::prelude"))
        {
            for (name, info) in prelude_types {
                env.register_type(name.clone(), info.clone());
            }
        }
        if let Some(prelude_schemes) = options
            .imported_value_schemes
            .as_ref()
            .and_then(|schemes| schemes.get("core::prelude"))
        {
            for (name, scheme) in prelude_schemes {
                env.bind_scheme(name.clone(), scheme.clone());
            }
        }

        let type_param_env = make_type_param_env(&func_decl.type_params);
        let error_type = type_param_env.get("E").unwrap().clone();
        let mut bindings = HashMap::new();
        bindings.insert("error".to_string(), error_type);
        let err_env = env.extend(Some(bindings));

        let result = build_typed_expr(&err_env, err_expr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_core_prelude_err_arm_checks_against_result_ue() {
        let source = "λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E] match res{Ok(value)=>fn(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let Declaration::Function(func_decl) = &program.declarations[0] else {
            panic!("expected function declaration");
        };
        let sigil_ast::Expr::Match(match_expr) = &func_decl.body else {
            panic!("expected match body");
        };
        let err_expr = &match_expr.arms[1].body;

        let options = core_prelude_type_options();
        let mut env = TypeEnvironment::new();
        if let Some(prelude_types) = options
            .imported_type_registries
            .as_ref()
            .and_then(|regs| regs.get("core::prelude"))
        {
            for (name, info) in prelude_types {
                env.register_type(name.clone(), info.clone());
            }
        }
        if let Some(prelude_schemes) = options
            .imported_value_schemes
            .as_ref()
            .and_then(|schemes| schemes.get("core::prelude"))
        {
            for (name, scheme) in prelude_schemes {
                env.bind_scheme(name.clone(), scheme.clone());
            }
        }

        let type_param_env = make_type_param_env(&func_decl.type_params);
        let error_type = type_param_env.get("E").unwrap().clone();
        let expected_type = ast_type_to_inference_type_resolved(
            &env,
            Some(&type_param_env),
            func_decl.return_type.as_ref().unwrap(),
        )
        .unwrap();
        let mut bindings = HashMap::new();
        bindings.insert("error".to_string(), error_type);
        let err_env = env.extend(Some(bindings));

        let result = check(&err_env, err_expr, &expected_type);
        assert!(result.is_ok());
    }

    fn assert_no_var_cycles(typ: &InferenceType, seen: &mut HashSet<u32>) {
        match typ {
            InferenceType::Primitive(_) | InferenceType::Any => {}
            InferenceType::Var(var) => {
                assert!(
                    seen.insert(var.id),
                    "cyclic type variable instance chain detected for var {}",
                    var.id
                );
                if let Some(instance) = &var.instance {
                    assert_no_var_cycles(instance, seen);
                }
                seen.remove(&var.id);
            }
            InferenceType::Function(function) => {
                for param in &function.params {
                    assert_no_var_cycles(param, seen);
                }
                assert_no_var_cycles(&function.return_type, seen);
            }
            InferenceType::List(list) => assert_no_var_cycles(&list.element_type, seen),
            InferenceType::Map(map) => {
                assert_no_var_cycles(&map.key_type, seen);
                assert_no_var_cycles(&map.value_type, seen);
            }
            InferenceType::Tuple(tuple) => {
                for item in &tuple.types {
                    assert_no_var_cycles(item, seen);
                }
            }
            InferenceType::Record(record) => {
                for field_type in record.fields.values() {
                    assert_no_var_cycles(field_type, seen);
                }
            }
            InferenceType::Constructor(constructor) => {
                for arg in &constructor.type_args {
                    assert_no_var_cycles(arg, seen);
                }
            }
        }
    }

    #[test]
    fn test_core_prelude_bind_result_scheme_has_no_cycles() {
        let source = "λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E] match res{Ok(value)=>fn(value)|Err(error)=>Err(error)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options()).unwrap();
        let scheme = result.declaration_schemes.get("bind_result").unwrap();
        let mut seen = HashSet::new();
        assert_no_var_cycles(&scheme.typ, &mut seen);
    }

    #[test]
    fn test_core_prelude_map_literal_typechecks() {
        let source = "λmain()=>{String↦Int}={\"a\"↦1}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_local_bindings_do_not_generalize() {
        let ok_source = "λmain()=>Int=l id=(λ(x:Int)=>Int=x);id(42)";
        let ok_tokens = tokenize(ok_source).unwrap();
        let ok_program = parse(ok_tokens, "test.sigil").unwrap();
        let ok_result = type_check(&ok_program, ok_source, TypeCheckOptions::default());
        assert!(ok_result.is_ok());

        let failing_source = "λmain()=>Unit=l id=(λ(x:Int)=>Int=x);id(\"oops\")";
        let failing_tokens = tokenize(failing_source).unwrap();
        let failing_program = parse(failing_tokens, "test.sigil").unwrap();
        let failing_result = type_check(
            &failing_program,
            failing_source,
            TypeCheckOptions::default(),
        );
        assert!(failing_result.is_err());
    }

    #[test]
    fn test_concurrent_region_typechecks() {
        let source = "e clock:{tick:λ()=>!Timer Unit}\nλmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@1{spawnEach [1,2] process}\nλprocess(value:Int)=>!Timer Result[Int,String]={l _=(clock.tick():Unit);Ok(value)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_ok());
    }

    #[test]
    fn test_concurrent_spawn_rejects_pure_result() {
        let source = "λmain()=>[ConcurrentOutcome[Int,String]]=concurrent urlAudit@1{spawn Ok(1)}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("spawn requires an effectful computation"));
    }

    #[test]
    fn test_concurrent_stop_on_must_match_error_type() {
        let source = "e clock:{tick:λ()=>!Timer Unit}\nλmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@1:{stopOn:stopOn}{spawn one()}\nλone()=>!Timer Result[Int,String]={l _=(clock.tick():Unit);Ok(1)}\nλstopOn(err:Int)=>Bool=false";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, core_prelude_type_options());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("stopOn parameter type"));
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
