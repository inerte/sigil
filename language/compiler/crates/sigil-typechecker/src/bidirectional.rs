//! Bidirectional Type Checking for Sigil
//!
//! Uses two complementary modes:
//! - Synthesis (⇒): Infer type from expression structure (bottom-up)
//! - Checking (⇐): Verify expression matches expected type (top-down)
//!
//! This is simpler than Hindley-Milner because Sigil requires mandatory
//! type annotations everywhere, making the inference burden much lighter.

use crate::coverage::{analyze_match_coverage, expr_summary};
use crate::effects::{
    declared_effects_cover_actual, effects_option_to_set, merge_effects, purity_from_effects,
    resolve_effect_names,
};
use crate::environment::{
    collect_type_var_ids, explicit_scheme, BindingMeta, BoundaryRule, BoundaryRuleKind,
    FunctionContract, LabelInfo, TypeEnvironment, TypeInfo,
};
use crate::errors::{format_type, TypeError};
use crate::json_codec::{
    analyze_json_codec_decl, bind_json_codec_helpers, finalize_json_codec_decl, JsonCodecSeed,
};
use crate::proof_context::{
    proof_outcome_reason, refinement_type_support_error, AssumptionCollector,
    ConstraintProofResult, ProofContext, SymbolicCollection, SymbolicRecord, SymbolicValue,
    MATCH_SCRUTINEE_BINDING,
};
use crate::typed_ir::{
    MethodSelector, StrictnessClass, TypeCheckResult, TypedBinaryExpr, TypedCallExpr,
    TypedConcurrentConfig, TypedConcurrentExpr, TypedConcurrentStep, TypedConstDecl,
    TypedConstructorCallExpr, TypedDeclaration, TypedExpr, TypedExprKind, TypedExternCallExpr,
    TypedExternDecl, TypedFieldAccessExpr, TypedFilterExpr, TypedFoldExpr, TypedFunctionDecl,
    TypedIfExpr, TypedIndexExpr, TypedLambdaExpr, TypedLetExpr, TypedListExpr, TypedMapEntryExpr,
    TypedMapExpr, TypedMapLiteralExpr, TypedMatchArm, TypedMatchExpr, TypedMethodCallExpr,
    TypedPipelineExpr, TypedProgram, TypedRecordExpr, TypedRecordField, TypedSpawnEachStep,
    TypedSpawnStep, TypedTestDecl, TypedTupleExpr, TypedTypeDecl, TypedUnaryExpr,
};
use crate::types::{
    apply_subst, ast_type_to_inference_type_with_params, types_equal, unify, EffectSet,
    InferenceType, TBorrowed, TConstructor, TFunction, TMap, TPrimitive, TRecord,
};
use crate::TypeCheckOptions;
use sigil_ast::{
    BinaryOperator, Declaration, Expr, FeatureFlagDecl, FunctionDecl, FunctionMode, LabelRef,
    LiteralExpr, LiteralType, LiteralValue, MemberRef, PrimitiveName, Program, QualifiedType,
    RecordExpr, RecordField, RuleAction, RuleDecl, SourceLocation, TransformDecl, Type, TypeDecl,
    TypeDef, UnaryOperator,
};
use sigil_diagnostics::codes;
use sigil_solver::{
    formula_and, formula_or, prove_formula, Atom, ComparisonOp, Formula, LinearExpr, SolverOutcome,
    SymbolPath,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

type TypeParamEnv = HashMap<String, InferenceType>;

static NEXT_RESOURCE_SCOPE_ID: AtomicU32 = AtomicU32::new(1);

/// Type check a Sigil program
///
/// Returns a map of function names to their inferred types
pub fn type_check(
    program: &Program,
    source_code: &str,
    options: TypeCheckOptions,
) -> Result<TypeCheckResult, TypeError> {
    let source_file = options.source_file.clone();
    (|| {
        validate_surface_types(program)?;

        let mut env = TypeEnvironment::create_initial();
        env.set_effect_catalog(options.effect_catalog.clone().unwrap_or_default());
        env.set_module_id(options.module_id.clone());
        env.set_source_file(options.source_file.clone());
        let mut types = HashMap::new();
        let mut schemes = HashMap::new();
        let reserved_value_names = collect_top_level_value_names(program);
        let mut generated_json_helper_names = HashSet::new();
        let mut json_codec_seeds: Vec<JsonCodecSeed> = Vec::new();

        // Register imported type registries
        if let Some(imported_type_registries) = options.imported_type_registries.as_ref() {
            for (module_id, type_registry) in imported_type_registries {
                env.register_imported_types(module_id.clone(), type_registry.clone());
            }
        }

        if let Some(imported_label_registries) = options.imported_label_registries.as_ref() {
            for (module_id, label_registry) in imported_label_registries {
                env.register_imported_labels(module_id.clone(), label_registry.clone());
            }
        }

        if let Some(imported_value_schemes) = options.imported_value_schemes.as_ref() {
            for (module_id, value_schemes) in imported_value_schemes {
                env.register_imported_value_schemes(module_id.clone(), value_schemes.clone());
            }
        }

        if let Some(imported_value_meta) = options.imported_value_meta.as_ref() {
            for (module_id, value_meta) in imported_value_meta {
                env.register_imported_value_meta(module_id.clone(), value_meta.clone());
            }
        }

        if let Some(imported_function_contracts) = options.imported_function_contracts.as_ref() {
            for (module_id, contracts) in imported_function_contracts {
                env.register_imported_function_contracts(module_id.clone(), contracts.clone());
            }
        }

        if let Some(imported_protocol_registries) = options.imported_protocol_registries.as_ref() {
            for (module_id, protocols) in imported_protocol_registries {
                env.register_imported_protocols(module_id.clone(), protocols.clone());
            }
        }

        if let Some(boundary_rules) = options.boundary_rules.as_ref() {
            for rule in boundary_rules {
                env.add_boundary_rule(rule.clone());
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

        if let Some(imported_namespaces) = options.imported_namespaces.as_ref() {
            for (namespace_name, imported_type) in imported_namespaces {
                env.bind(namespace_name.clone(), imported_type.clone());
            }
        }

        for decl in &program.declarations {
            if let Declaration::Label(label_decl) = decl {
                env.register_label(
                    label_decl.name.clone(),
                    LabelInfo {
                        combines: resolve_label_refs(&env, &label_decl.combines)?,
                    },
                );
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
                            constraint: type_decl.constraint.clone(),
                            labels: resolve_label_refs(&env, &type_decl.labels)?,
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

                Declaration::Derive(derive_decl) => {
                    let seed = analyze_json_codec_decl(&env, derive_decl, source_code)?;
                    for helper_name in [
                        seed.helper_names.encode.clone(),
                        seed.helper_names.decode.clone(),
                        seed.helper_names.parse.clone(),
                        seed.helper_names.stringify.clone(),
                    ] {
                        if reserved_value_names.contains(&helper_name) {
                            return Err(TypeError::new(
                                format!(
                                    "derive json for '{}' would generate helper '{}' which conflicts with an existing top-level value",
                                    seed.target_name, helper_name
                                ),
                                Some(derive_decl.location),
                            ));
                        }
                        if !generated_json_helper_names.insert(helper_name.clone()) {
                            return Err(TypeError::new(
                                format!(
                                    "derive json helper '{}' is generated more than once in this module",
                                    helper_name
                                ),
                                Some(derive_decl.location),
                            ));
                        }
                    }
                    bind_json_codec_helpers(&mut env, &mut types, &seed);
                    json_codec_seeds.push(seed);
                }

                Declaration::Label(_) => {}

                Declaration::Protocol(protocol_decl) => {
                    if env.lookup_type(&protocol_decl.name).is_none() {
                        return Err(TypeError::new(
                            format!(
                                "SIGIL-PROTO-UNKNOWN-TYPE: protocol '{}' references a type that is not declared in this file",
                                protocol_decl.name
                            ),
                            Some(protocol_decl.location),
                        ));
                    }

                    let mut all_states: std::collections::BTreeSet<String> =
                        std::collections::BTreeSet::new();
                    all_states.insert(protocol_decl.initial.clone());
                    all_states.insert(protocol_decl.terminal.clone());
                    for t in &protocol_decl.transitions {
                        all_states.insert(t.from.clone());
                        all_states.insert(t.to.clone());
                    }
                    let states: Vec<String> = all_states.into_iter().collect();

                    let mut transitions: HashMap<String, (String, String)> = HashMap::new();
                    for t in &protocol_decl.transitions {
                        for fn_name in &t.via {
                            transitions
                                .insert(fn_name.clone(), (t.from.clone(), t.to.clone()));
                        }
                    }

                    env.register_protocol(
                        protocol_decl.name.clone(),
                        crate::environment::ProtocolSpec {
                            states,
                            transitions,
                            initial: protocol_decl.initial.clone(),
                            terminal: protocol_decl.terminal.clone(),
                        },
                    );
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
                        .map(|ty| {
                            ast_type_to_inference_type_resolved(&env, Some(&type_param_env), ty)
                        })
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
                        env.bind_scheme_with_meta(
                            func_decl.name.clone(),
                            scheme.clone(),
                            BindingMeta {
                                function_mode: Some(func_decl.mode),
                                return_labels: declared_type_labels(
                                    &env,
                                    Some(&type_param_env),
                                    func_decl.return_type.as_ref(),
                                )?,
                                ..BindingMeta::default()
                            },
                        );
                        schemes.insert(func_decl.name.clone(), scheme);
                        func_type.clone()
                    };

                    if func_decl.type_params.is_empty() {
                        env.bind_with_meta(
                            func_decl.name.clone(),
                            binding_type.clone(),
                            BindingMeta {
                                function_mode: Some(func_decl.mode),
                                return_labels: declared_type_labels(
                                    &env,
                                    Some(&type_param_env),
                                    func_decl.return_type.as_ref(),
                                )?,
                                ..BindingMeta::default()
                            },
                        );
                    }

                    if func_decl.requires.is_some() || func_decl.ensures.is_some() {
                        env.register_function_contract(
                            func_decl.name.clone(),
                            FunctionContract {
                                params: func_decl
                                    .params
                                    .iter()
                                    .map(|param| param.name.clone())
                                    .collect(),
                                requires: func_decl.requires.clone(),
                                ensures: func_decl.ensures.clone(),
                            },
                        );
                    }

                    types.insert(func_decl.name.clone(), binding_type);
                }

                Declaration::Transform(TransformDecl {
                    function: func_decl,
                }) => {
                    let type_param_env = make_type_param_env(&func_decl.type_params);
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
                        .map(|ty| {
                            ast_type_to_inference_type_resolved(&env, Some(&type_param_env), ty)
                        })
                        .transpose()?
                        .unwrap_or(InferenceType::Any);

                    let effects = if func_decl.effects.is_empty() {
                        None
                    } else {
                        Some(resolve_effect_names(
                            &env,
                            &func_decl.effects,
                            func_decl.location,
                            "transform signature",
                        )?)
                    };

                    let func_type = InferenceType::Function(Box::new(TFunction {
                        params,
                        return_type,
                        effects,
                    }));

                    let binding_meta = BindingMeta {
                        is_transform: true,
                        function_mode: Some(func_decl.mode),
                        return_labels: declared_type_labels(
                            &env,
                            Some(&type_param_env),
                            func_decl.return_type.as_ref(),
                        )?,
                        ..BindingMeta::default()
                    };

                    let binding_type = if func_decl.type_params.is_empty() {
                        func_type.clone()
                    } else {
                        let mut quantified_vars = HashSet::new();
                        collect_type_var_ids(&func_type, &mut quantified_vars);
                        let scheme = explicit_scheme(&func_type, &quantified_vars);
                        env.bind_scheme_with_meta(
                            func_decl.name.clone(),
                            scheme.clone(),
                            binding_meta.clone(),
                        );
                        schemes.insert(func_decl.name.clone(), scheme);
                        func_type.clone()
                    };

                    if func_decl.type_params.is_empty() {
                        env.bind_with_meta(
                            func_decl.name.clone(),
                            binding_type.clone(),
                            binding_meta,
                        );
                    }

                    if func_decl.requires.is_some() || func_decl.ensures.is_some() {
                        env.register_function_contract(
                            func_decl.name.clone(),
                            FunctionContract {
                                params: func_decl
                                    .params
                                    .iter()
                                    .map(|param| param.name.clone())
                                    .collect(),
                                requires: func_decl.requires.clone(),
                                ensures: func_decl.ensures.clone(),
                            },
                        );
                    }

                    types.insert(func_decl.name.clone(), binding_type);
                }

                Declaration::Rule(rule_decl) => {
                    env.add_boundary_rule(resolve_boundary_rule(&env, rule_decl)?);
                }

                Declaration::Const(const_decl) => {
                    // Register constant type
                    let const_type = const_decl
                        .type_annotation
                        .as_ref()
                        .map(|ty| ast_type_to_inference_type_resolved(&env, None, ty))
                        .transpose()?
                        .unwrap_or(InferenceType::Any);

                    env.bind_with_meta(
                        const_decl.name.clone(),
                        const_type.clone(),
                        BindingMeta {
                            labels: labels_for_type(&env, &const_type),
                            ..BindingMeta::default()
                        },
                    );
                    types.insert(const_decl.name.clone(), const_type);
                }

                Declaration::FeatureFlag(feature_flag_decl) => {
                    let flag_value_type = ast_type_to_inference_type_resolved(
                        &env,
                        None,
                        &feature_flag_decl.flag_type,
                    )?;
                    validate_feature_flag_value_type(
                        &env,
                        &feature_flag_decl.name,
                        &flag_value_type,
                        feature_flag_decl.location,
                    )?;
                    let flag_type = feature_flag_descriptor_type(&feature_flag_decl.flag_type);
                    let inferred_flag_type =
                        ast_type_to_inference_type_resolved(&env, None, &flag_type)?;
                    env.bind_with_meta(
                        feature_flag_decl.name.clone(),
                        inferred_flag_type.clone(),
                        BindingMeta::default(),
                    );
                    types.insert(feature_flag_decl.name.clone(), inferred_flag_type);
                }

                Declaration::Extern(extern_decl) => {
                    let namespace_name = extern_decl.module_path.join("::");

                    if let Some(members) = &extern_decl.members {
                        let mut fields = HashMap::new();
                        for member in members {
                            let member_type =
                                if matches!(member.kind, sigil_ast::ExternMemberKind::Subscription)
                                {
                                    extern_subscription_member_type(&env, member)?
                                } else {
                                    ast_type_to_inference_type_resolved(
                                        &env,
                                        None,
                                        &member.member_type,
                                    )?
                                };
                            fields.insert(member.name.clone(), member_type);
                            env.register_extern_member_kind(
                                namespace_name.clone(),
                                member.name.clone(),
                                member.kind,
                            );
                        }
                        env.bind_with_meta(
                            namespace_name,
                            InferenceType::Record(TRecord { fields, name: None }),
                            BindingMeta {
                                is_extern_namespace: true,
                                ..BindingMeta::default()
                            },
                        );
                    } else {
                        // Untyped extern: trust mode
                        env.bind_with_meta(
                            namespace_name,
                            InferenceType::Any,
                            BindingMeta {
                                is_extern_namespace: true,
                                ..BindingMeta::default()
                            },
                        );
                    }
                }

                Declaration::Test(test_decl) => {
                    check_test_decl(&env, test_decl)?;
                }
            }
        }

        validate_type_constraints(&env, program)?;
        validate_protocol_contracts(&env, program)?;
        validate_protocol_state_runtime_usage(&env, program)?;

        let mut typed_declarations = Vec::new();
        let mut json_codec_seed_iter = json_codec_seeds.into_iter();

        // Second pass: Type check function bodies and build typed IR
        for decl in &program.declarations {
            if let Declaration::Function(func_decl) = decl {
                check_function_decl(&env, func_decl)?;
                typed_declarations.push(TypedDeclaration::Function(build_typed_function_decl(
                    &env, func_decl, false,
                )?));
            } else if let Declaration::Transform(TransformDecl { function }) = decl {
                check_transform_decl(&env, function)?;
                typed_declarations.push(TypedDeclaration::Function(build_typed_function_decl(
                    &env, function, true,
                )?));
            } else if let Declaration::Const(const_decl) = decl {
                if let Some(ref annotation) = const_decl.type_annotation {
                    let expected_type =
                        ast_type_to_inference_type_resolved(&env, None, annotation)?;
                    check(&env, &const_decl.value, &expected_type).map_err(|error| {
                        TypeError::new(
                            format!(
                                "Constant '{}' type mismatch: {}",
                                const_decl.name, error.message
                            ),
                            error.location.or(Some(const_decl.location)),
                        )
                    })?;
                }
                typed_declarations.push(TypedDeclaration::Const(build_typed_const_decl(
                    &env, const_decl,
                )?));
            } else if let Declaration::FeatureFlag(feature_flag_decl) = decl {
                typed_declarations.push(TypedDeclaration::Const(build_typed_feature_flag_decl(
                    &env,
                    feature_flag_decl,
                )?));
            } else if let Declaration::Type(type_decl) = decl {
                typed_declarations.push(TypedDeclaration::Type(TypedTypeDecl {
                    ast: type_decl.clone(),
                }));
            } else if let Declaration::Derive(_) = decl {
                let seed = json_codec_seed_iter.next().ok_or_else(|| {
                    TypeError::new(
                        "internal error: missing derived json codec seed".to_string(),
                        Some(decl_location(decl)),
                    )
                })?;
                typed_declarations.push(TypedDeclaration::JsonCodec(finalize_json_codec_decl(
                    &env, seed,
                )?));
            } else if let Declaration::Label(_) | Declaration::Rule(_) = decl {
                // Labels and rules are compile-time only.
            } else if let Declaration::Extern(extern_decl) = decl {
                typed_declarations.push(TypedDeclaration::Extern(TypedExternDecl {
                    ast: extern_decl.clone(),
                }));
            } else if let Declaration::Test(test_decl) = decl {
                check_test_decl(&env, test_decl)?;
                typed_declarations.push(TypedDeclaration::Test(build_typed_test_decl(
                    &env, test_decl,
                )?));
            } else if let Declaration::Effect(_) = decl {
                // Effect declarations are compile-time only and do not appear in typed IR.
            } else if let Declaration::Protocol(_) = decl {
                // Protocol declarations are compile-time only — consumed in first pass.
            }
        }

        Ok(TypeCheckResult {
            declaration_types: types,
            declaration_schemes: schemes,
            declaration_meta: env.binding_meta_snapshot(),
            label_registry: env.label_registry_snapshot(),
            function_contracts: env.function_contracts_snapshot(),
            protocol_registry: env.protocol_registry_owned_snapshot(),
            boundary_rules: env.boundary_rules_snapshot(),
            typed_program: TypedProgram {
                declarations: typed_declarations,
            },
        })
    })()
    .map_err(|error: TypeError| error.with_source_file_if_missing(source_file))
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

fn resolve_label_ref(env: &TypeEnvironment, label_ref: &LabelRef) -> Result<String, TypeError> {
    if label_ref.module_path.is_empty() {
        env.lookup_label(&label_ref.name).ok_or_else(|| {
            TypeError::new(
                format!("Unknown label '{}'", label_ref.name),
                Some(label_ref.location),
            )
        })?;
        return Ok(env
            .module_id()
            .map(|module_id| format!("{}.{}", module_id, label_ref.name))
            .unwrap_or_else(|| label_ref.name.clone()));
    }

    env.lookup_qualified_label(&label_ref.module_path, &label_ref.name)
        .ok_or_else(|| {
            TypeError::new(
                format!(
                    "Unknown label '{}.{}'",
                    label_ref.module_path.join("::"),
                    label_ref.name
                ),
                Some(label_ref.location),
            )
        })?;
    Ok(format!(
        "{}.{}",
        label_ref.module_path.join("::"),
        label_ref.name
    ))
}

fn resolve_label_refs(
    env: &TypeEnvironment,
    label_refs: &[LabelRef],
) -> Result<BTreeSet<String>, TypeError> {
    label_refs
        .iter()
        .map(|label_ref| resolve_label_ref(env, label_ref))
        .collect()
}

fn declared_type_labels(
    env: &TypeEnvironment,
    type_param_env: Option<&TypeParamEnv>,
    ast_type: Option<&Type>,
) -> Result<BTreeSet<String>, TypeError> {
    let Some(ast_type) = ast_type else {
        return Ok(BTreeSet::new());
    };
    let typ = ast_type_to_inference_type_resolved(env, type_param_env, ast_type)?;
    Ok(labels_for_type(env, &typ))
}

fn lookup_named_type_info(env: &TypeEnvironment, name: &str) -> Option<TypeInfo> {
    if let Some((module_path, type_name)) = split_qualified_constructor_name(name) {
        return env.lookup_qualified_type(&module_path, &type_name);
    }
    env.lookup_type(name)
}

fn collect_top_level_value_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for decl in &program.declarations {
        match decl {
            Declaration::Function(function) => {
                names.insert(function.name.clone());
            }
            Declaration::Transform(transform) => {
                names.insert(transform.function.name.clone());
            }
            Declaration::Const(const_decl) => {
                names.insert(const_decl.name.clone());
            }
            Declaration::FeatureFlag(feature_flag_decl) => {
                names.insert(feature_flag_decl.name.clone());
            }
            Declaration::Extern(extern_decl) => {
                names.insert(extern_decl.module_path.join("::"));
            }
            Declaration::Type(type_decl) => {
                if let TypeDef::Sum(sum) = &type_decl.definition {
                    for variant in &sum.variants {
                        names.insert(variant.name.clone());
                    }
                }
            }
            Declaration::Derive(_)
            | Declaration::Label(_)
            | Declaration::Rule(_)
            | Declaration::Effect(_)
            | Declaration::Protocol(_)
            | Declaration::Test(_) => {}
        }
    }
    names
}

fn decl_location(decl: &Declaration) -> SourceLocation {
    match decl {
        Declaration::Function(function) => function.location,
        Declaration::Transform(transform) => transform.function.location,
        Declaration::Type(type_decl) => type_decl.location,
        Declaration::Derive(derive_decl) => derive_decl.location,
        Declaration::Protocol(protocol_decl) => protocol_decl.location,
        Declaration::Label(label_decl) => label_decl.location,
        Declaration::Rule(rule_decl) => rule_decl.location,
        Declaration::Effect(effect_decl) => effect_decl.location,
        Declaration::FeatureFlag(feature_flag_decl) => feature_flag_decl.location,
        Declaration::Const(const_decl) => const_decl.location,
        Declaration::Test(test_decl) => test_decl.location,
        Declaration::Extern(extern_decl) => extern_decl.location,
    }
}

fn label_closure(env: &TypeEnvironment, direct: &BTreeSet<String>) -> BTreeSet<String> {
    let mut active = direct.clone();

    loop {
        let mut changed = false;
        for (label_name, info) in env.all_labels() {
            if active.contains(&label_name) {
                continue;
            }
            if info
                .combines
                .iter()
                .any(|component| active.contains(component))
            {
                active.insert(label_name);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    active
}

fn labels_for_type(env: &TypeEnvironment, typ: &InferenceType) -> BTreeSet<String> {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Var(_) | InferenceType::Any => BTreeSet::new(),
        InferenceType::List(list) => label_closure(env, &labels_for_type(env, &list.element_type)),
        InferenceType::Map(map) => {
            let mut labels = labels_for_type(env, &map.key_type);
            labels.extend(labels_for_type(env, &map.value_type));
            label_closure(env, &labels)
        }
        InferenceType::Tuple(tuple) => {
            let mut labels = BTreeSet::new();
            for item in &tuple.types {
                labels.extend(labels_for_type(env, item));
            }
            label_closure(env, &labels)
        }
        InferenceType::Function(_) => BTreeSet::new(),
        InferenceType::Record(record) => {
            let mut labels = BTreeSet::new();
            for field_type in record.fields.values() {
                labels.extend(labels_for_type(env, field_type));
            }
            if let Some(name) = &record.name {
                if let Some(info) = lookup_named_type_info(env, name) {
                    labels.extend(info.labels);
                }
            }
            label_closure(env, &labels)
        }
        InferenceType::Constructor(constructor) => {
            let mut labels = BTreeSet::new();
            if let Some(info) = lookup_named_type_info(env, &constructor.name) {
                labels.extend(info.labels);
            }
            for arg in &constructor.type_args {
                labels.extend(labels_for_type(env, arg));
            }
            label_closure(env, &labels)
        }
        InferenceType::Owned(inner) => labels_for_type(env, inner),
        InferenceType::Borrowed(borrowed) => labels_for_type(env, &borrowed.resource_type),
    }
}

fn format_label_set(labels: &BTreeSet<String>) -> String {
    labels.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn ensure_label_subset(
    env: &TypeEnvironment,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
    location: SourceLocation,
    context: &str,
) -> Result<(), TypeError> {
    let actual_labels = labels_for_type(env, actual_type);
    if actual_labels.is_empty() {
        return Ok(());
    }

    let expected_labels = labels_for_type(env, expected_type);
    if actual_labels.is_subset(&expected_labels) {
        return Ok(());
    }

    let dropped: BTreeSet<String> = actual_labels
        .difference(&expected_labels)
        .cloned()
        .collect();

    Err(TypeError::new(
        format!(
            "{} would drop required labels: {}",
            context,
            format_label_set(&dropped)
        ),
        Some(location),
    ))
}

fn resolve_member_ref(env: &TypeEnvironment, member_ref: &MemberRef) -> String {
    if member_ref.module_path.is_empty() {
        return env
            .module_id()
            .map(|module_id| format!("{}.{}", module_id, member_ref.member))
            .unwrap_or_else(|| member_ref.member.clone());
    }
    format!(
        "{}.{}",
        member_ref.module_path.join("::"),
        member_ref.member
    )
}

fn resolve_boundary_rule(
    env: &TypeEnvironment,
    rule_decl: &RuleDecl,
) -> Result<BoundaryRule, TypeError> {
    if !member_ref_targets_named_topology_boundary(env, &rule_decl.boundary) {
        return Err(TypeError::new(
            "Boundary rules must target named topology boundaries".to_string(),
            Some(rule_decl.boundary.location),
        ));
    }

    let labels = resolve_label_refs(env, &rule_decl.labels)?;
    let boundary = resolve_member_ref(env, &rule_decl.boundary);
    let action = match &rule_decl.action {
        RuleAction::Allow { .. } => BoundaryRuleKind::Allow,
        RuleAction::Block { .. } => BoundaryRuleKind::Block,
        RuleAction::Through { transform, .. } => {
            let transform_name = resolve_member_ref(env, transform);
            let transform_meta = if transform.module_path.is_empty() {
                env.lookup_meta(&transform.member)
            } else {
                env.lookup_qualified_value_meta(&transform.module_path, &transform.member)
            };
            let Some(transform_meta) = transform_meta else {
                return Err(TypeError::new(
                    format!("Unknown transform '{}'", transform_name),
                    Some(transform.location),
                ));
            };
            if !transform_meta.is_transform {
                return Err(TypeError::new(
                    format!("'{}' is not a transform declaration", transform_name),
                    Some(transform.location),
                ));
            }
            BoundaryRuleKind::Through(transform_name)
        }
    };

    Ok(BoundaryRule {
        labels,
        boundary,
        action,
    })
}

fn resolve_qualified_type(
    env: &TypeEnvironment,
    type_param_env: Option<&TypeParamEnv>,
    qualified: &sigil_ast::QualifiedType,
) -> Result<InferenceType, TypeError> {
    let requested_module_id = qualified.module_path.join("::");
    let resolved_module_id = env
        .remap_package_local_module_id(&requested_module_id)
        .unwrap_or_else(|| requested_module_id.clone());
    let resolved_module_path = resolved_module_id
        .split("::")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let type_info = env.lookup_qualified_type(&resolved_module_path, &qualified.type_name);

    let Some(type_info) = type_info else {
        if let Some(available_types) = env.get_imported_module_type_names(&resolved_module_id) {
            if !available_types.is_empty() {
                return Err(TypeError::new(
                    format!(
                        "Undefined type: {}.{}\n\nModule '{}' is referenced here, but it does not export a type named '{}'.\n\nAvailable exported types:\n{}",
                        resolved_module_id,
                        qualified.type_name,
                        resolved_module_id,
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
                "Module '{}' is unavailable or does not export any types.",
                resolved_module_id
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

    let qualified_name = format!("{}.{}", resolved_module_id, qualified.type_name);
    if type_info.type_params.is_empty() {
        if type_info.constraint.is_some() {
            return Ok(InferenceType::Constructor(TConstructor {
                name: qualified_name,
                type_args,
            }));
        }
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
                if !type_info.labels.is_empty() {
                    return Ok(InferenceType::Constructor(TConstructor {
                        name: qualified_name,
                        type_args: Vec::new(),
                    }));
                }
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

pub(crate) fn split_qualified_constructor_name(name: &str) -> Option<(Vec<String>, String)> {
    let dot_index = name.rfind('.')?;
    let module_id = &name[..dot_index];
    let type_name = &name[dot_index + 1..];
    Some((
        module_id.split("::").map(|part| part.to_string()).collect(),
        type_name.to_string(),
    ))
}

pub(crate) fn constructor_display_name(module_path: &[String], name: &str) -> String {
    if module_path.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", module_path.join("::"), name)
    }
}

pub(crate) fn lookup_constructor_type(
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
                        if type_info.constraint.is_some() {
                            return Ok(inference_type.clone());
                        }
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
                                if !type_info.labels.is_empty() {
                                    return Ok(inference_type.clone());
                                }
                                ast_type_to_inference_type_resolved(env, None, &alias.aliased_type)
                            }
                            TypeDef::Sum(_) => Ok(inference_type.clone()),
                        };
                    }
                }
            }

            if let Some(type_info) = env.lookup_type(&constructor.name) {
                if type_info.type_params.is_empty() {
                    if type_info.constraint.is_some() {
                        return Ok(inference_type.clone());
                    }
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
                            if !type_info.labels.is_empty() {
                                return Ok(inference_type.clone());
                            }
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

pub(crate) fn ast_type_to_inference_type_resolved(
    env: &TypeEnvironment,
    type_param_env: Option<&TypeParamEnv>,
    ast_type: &Type,
) -> Result<InferenceType, TypeError> {
    match ast_type {
        Type::Qualified(qualified) => resolve_qualified_type(env, type_param_env, qualified),
        Type::Constructor(constructor) => {
            let type_args = constructor
                .type_args
                .iter()
                .map(|arg| ast_type_to_inference_type_resolved(env, type_param_env, arg))
                .collect::<Result<Vec<_>, _>>()?;
            if constructor.name == "Owned" {
                if type_args.len() != 1 {
                    return Err(TypeError::new(
                        "Owned type constructor expects exactly one type argument".to_string(),
                        Some(constructor.location),
                    ));
                }
                Ok(InferenceType::Owned(Box::new(type_args[0].clone())))
            } else {
                Ok(InferenceType::Constructor(TConstructor {
                    name: constructor.name.clone(),
                    type_args,
                }))
            }
        }
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

fn extern_subscription_member_type(
    env: &TypeEnvironment,
    member: &sigil_ast::ExternMember,
) -> Result<InferenceType, TypeError> {
    let Type::Function(function_type) = &member.member_type else {
        return Err(TypeError::new(
            "Subscription extern members must use function types".to_string(),
            Some(member.location),
        ));
    };

    let params = function_type
        .param_types
        .iter()
        .map(|param| ast_type_to_inference_type_resolved(env, None, param))
        .collect::<Result<Vec<_>, _>>()?;
    let payload_type = ast_type_to_inference_type_resolved(env, None, &function_type.return_type)?;
    let mut effects = HashSet::new();
    effects.insert("Stream".to_string());

    Ok(InferenceType::Function(Box::new(TFunction {
        params,
        return_type: InferenceType::Owned(Box::new(InferenceType::Constructor(TConstructor {
            name: "stdlib::stream.Source".to_string(),
            type_args: vec![payload_type],
        }))),
        effects: Some(effects),
    })))
}

fn validate_surface_types(program: &Program) -> Result<(), TypeError> {
    for decl in &program.declarations {
        validate_declaration_surface_types(decl)?;
    }

    Ok(())
}

fn validate_type_constraints(env: &TypeEnvironment, program: &Program) -> Result<(), TypeError> {
    for decl in &program.declarations {
        let Declaration::Type(type_decl) = decl else {
            continue;
        };
        let Some(constraint) = &type_decl.constraint else {
            continue;
        };

        validate_refinement_constraint_shape(env, type_decl)?;

        let value_type = constraint_value_type_for_decl(env, type_decl)?;
        let mut bindings = HashMap::new();
        bindings.insert("value".to_string(), value_type);
        let constraint_env = env.extend(Some(bindings));
        let constraint_type = synthesize(&constraint_env, constraint)?;

        if !same_type(&constraint_env, &constraint_type, &bool_type()) {
            return Err(TypeError::new(
                format!(
                    "Type constraint for '{}' must return Bool, got {}",
                    type_decl.name,
                    format_type(&constraint_type)
                ),
                Some(expr_location(constraint)),
            ));
        }
    }

    Ok(())
}

fn validate_protocol_contracts(env: &TypeEnvironment, program: &Program) -> Result<(), TypeError> {
    for decl in &program.declarations {
        let Declaration::Protocol(protocol_decl) = decl else {
            continue;
        };
        let Some(spec) = env.lookup_protocol(&protocol_decl.name) else {
            continue;
        };
        for (fn_name, (from_state, to_state)) in &spec.transitions {
            let Some(func_decl) = find_protocol_function_decl(program, fn_name) else {
                return Err(protocol_contract_error(
                    &protocol_decl.name,
                    fn_name,
                    "has no local function declaration",
                    protocol_decl.location,
                ));
            };
            let Some(contract) = env.lookup_function_contract(fn_name) else {
                return Err(protocol_contract_error(
                    &protocol_decl.name,
                    fn_name,
                    "has no requires/ensures state annotations",
                    protocol_decl.location,
                ));
            };
            let Some(requires) = contract.requires.as_ref() else {
                return Err(protocol_contract_error(
                    &protocol_decl.name,
                    fn_name,
                    "does not require the protocol source state",
                    protocol_decl.location,
                ));
            };
            let Some(ensures) = contract.ensures.as_ref() else {
                return Err(protocol_contract_error(
                    &protocol_decl.name,
                    fn_name,
                    "does not ensure the protocol target state",
                    protocol_decl.location,
                ));
            };

            let func_env = function_contract_env(env, func_decl, false)?;
            if !contract_clause_contains_state_assertion(
                &func_env,
                requires,
                &protocol_decl.name,
                from_state,
            ) {
                return Err(protocol_contract_error(
                    &protocol_decl.name,
                    fn_name,
                    &format!("must require {}.state={}", protocol_decl.name, from_state),
                    expr_location(requires),
                ));
            }

            let ensures_env = function_contract_env(env, func_decl, true)?;
            if !contract_clause_contains_state_assertion(
                &ensures_env,
                ensures,
                &protocol_decl.name,
                to_state,
            ) {
                return Err(protocol_contract_error(
                    &protocol_decl.name,
                    fn_name,
                    &format!("must ensure {}.state={}", protocol_decl.name, to_state),
                    expr_location(ensures),
                ));
            }
        }
    }
    Ok(())
}

fn protocol_contract_error(
    protocol_name: &str,
    fn_name: &str,
    reason: &str,
    location: SourceLocation,
) -> TypeError {
    TypeError::new(
        format!(
            "SIGIL-PROTO-MISSING-CONTRACT: function '{}' is listed in protocol '{}' via clause but {}",
            fn_name, protocol_name, reason
        ),
        Some(location),
    )
}

fn find_protocol_function_decl<'a>(program: &'a Program, name: &str) -> Option<&'a FunctionDecl> {
    program.declarations.iter().find_map(|decl| match decl {
        Declaration::Function(func_decl) if func_decl.name == name => Some(func_decl),
        Declaration::Transform(TransformDecl { function }) if function.name == name => {
            Some(function)
        }
        _ => None,
    })
}

fn function_contract_env(
    env: &TypeEnvironment,
    func_decl: &FunctionDecl,
    include_result: bool,
) -> Result<TypeEnvironment, TypeError> {
    let type_param_env = make_type_param_env(&func_decl.type_params);
    let mut bindings = HashMap::new();
    for param in &func_decl.params {
        let param_type = param
            .type_annotation
            .as_ref()
            .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
            .transpose()?
            .unwrap_or(InferenceType::Any);
        bindings.insert(param.name.clone(), env.normalize_type(&param_type));
    }

    if include_result {
        let return_type = func_decl
            .return_type
            .as_ref()
            .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
            .transpose()?
            .unwrap_or(InferenceType::Any);
        bindings.insert("result".to_string(), return_type);
    }

    Ok(env.extend(Some(bindings)))
}

fn contract_clause_contains_state_assertion(
    env: &TypeEnvironment,
    expr: &Expr,
    protocol_name: &str,
    state_name: &str,
) -> bool {
    match expr {
        Expr::Binary(binary) if binary.operator == BinaryOperator::And => {
            contract_clause_contains_state_assertion(env, &binary.left, protocol_name, state_name)
                || contract_clause_contains_state_assertion(
                    env,
                    &binary.right,
                    protocol_name,
                    state_name,
                )
        }
        Expr::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            state_equality_matches_protocol(
                env,
                &binary.left,
                &binary.right,
                protocol_name,
                state_name,
            ) || state_equality_matches_protocol(
                env,
                &binary.right,
                &binary.left,
                protocol_name,
                state_name,
            )
        }
        Expr::TypeAscription(type_asc) => {
            contract_clause_contains_state_assertion(env, &type_asc.expr, protocol_name, state_name)
        }
        _ => false,
    }
}

fn state_equality_matches_protocol(
    env: &TypeEnvironment,
    state_access: &Expr,
    state_label: &Expr,
    protocol_name: &str,
    state_name: &str,
) -> bool {
    let Expr::Identifier(label) = state_label else {
        return false;
    };
    if label.name != state_name {
        return false;
    }
    let Expr::FieldAccess(field_access) = state_access else {
        return false;
    };
    if field_access.field != "state" {
        return false;
    }
    let Ok(object_type) = synthesize(env, &field_access.object) else {
        return false;
    };
    resolve_protocol_type_name(&env.normalize_type(&object_type), env).as_deref()
        == Some(protocol_name)
}

fn validate_protocol_state_runtime_usage(
    env: &TypeEnvironment,
    program: &Program,
) -> Result<(), TypeError> {
    for decl in &program.declarations {
        match decl {
            Declaration::Function(func_decl) => {
                let body_env = function_contract_env(env, func_decl, false)?;
                validate_protocol_state_runtime_expr(&body_env, &func_decl.body)?;
            }
            Declaration::Transform(TransformDecl { function }) => {
                let body_env = function_contract_env(env, function, false)?;
                validate_protocol_state_runtime_expr(&body_env, &function.body)?;
            }
            Declaration::Const(const_decl) => {
                validate_protocol_state_runtime_expr(env, &const_decl.value)?;
            }
            Declaration::Derive(_) => {}
            Declaration::FeatureFlag(feature_flag_decl) => {
                validate_protocol_state_runtime_expr(env, &feature_flag_decl.default)?;
            }
            Declaration::Test(test_decl) => {
                let mut body_env = env.clone();
                for binding in &test_decl.world_bindings {
                    validate_protocol_state_runtime_expr(&body_env, &binding.value)?;
                    let binding_type = binding
                        .type_annotation
                        .as_ref()
                        .map(|ty| ast_type_to_inference_type_resolved(&body_env, None, ty))
                        .transpose()?
                        .map_or_else(|| synthesize(&body_env, &binding.value), Ok)?;
                    let mut bindings = HashMap::new();
                    bindings.insert(binding.name.clone(), binding_type);
                    body_env = body_env.extend(Some(bindings));
                }
                validate_protocol_state_runtime_expr(&body_env, &test_decl.body)?;
            }
            Declaration::Type(_)
            | Declaration::Protocol(_)
            | Declaration::Label(_)
            | Declaration::Rule(_)
            | Declaration::Effect(_)
            | Declaration::Extern(_) => {}
        }
    }
    Ok(())
}

fn validate_protocol_state_runtime_expr(
    env: &TypeEnvironment,
    expr: &Expr,
) -> Result<(), TypeError> {
    match expr {
        Expr::Identifier(identifier) => {
            if env.lookup(&identifier.name).is_none()
                && env.is_protocol_state_label(&identifier.name)
            {
                return Err(TypeError::new(
                    format!(
                        "Protocol state label '{}' is contract-only; use it only in requires/ensures clauses like handle.state={}",
                        identifier.name, identifier.name
                    ),
                    Some(identifier.location),
                ));
            }
            Ok(())
        }
        Expr::Literal(_) | Expr::MemberAccess(_) => Ok(()),
        Expr::Lambda(lambda) => {
            let mut bindings = HashMap::new();
            for param in &lambda.params {
                let param_type = param
                    .type_annotation
                    .as_ref()
                    .map(|ty| ast_type_to_inference_type_resolved(env, None, ty))
                    .transpose()?
                    .unwrap_or(InferenceType::Any);
                bindings.insert(param.name.clone(), param_type);
            }
            let lambda_env = env.extend(Some(bindings));
            validate_protocol_state_runtime_expr(&lambda_env, &lambda.body)
        }
        Expr::Application(app) => {
            validate_protocol_state_runtime_expr(env, &app.func)?;
            for arg in &app.args {
                validate_protocol_state_runtime_expr(env, arg)?;
            }
            Ok(())
        }
        Expr::Binary(binary) => {
            validate_protocol_state_runtime_expr(env, &binary.left)?;
            validate_protocol_state_runtime_expr(env, &binary.right)
        }
        Expr::Unary(unary) => validate_protocol_state_runtime_expr(env, &unary.operand),
        Expr::Match(match_expr) => {
            validate_protocol_state_runtime_expr(env, &match_expr.scrutinee)?;
            let scrutinee_type = synthesize(env, &match_expr.scrutinee)?;
            for arm in &match_expr.arms {
                let mut bindings = HashMap::new();
                check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)?;
                let arm_env = env.extend(Some(bindings));
                if let Some(guard) = &arm.guard {
                    validate_protocol_state_runtime_expr(&arm_env, guard)?;
                }
                validate_protocol_state_runtime_expr(&arm_env, &arm.body)?;
            }
            Ok(())
        }
        Expr::Let(let_expr) => {
            validate_protocol_state_runtime_expr(env, &let_expr.value)?;
            let value_type = synthesize(env, &let_expr.value)?;
            let mut bindings = HashMap::new();
            if let sigil_ast::Pattern::Identifier(identifier) = &let_expr.pattern {
                bindings.insert(identifier.name.clone(), value_type);
            }
            let body_env = env.extend(Some(bindings));
            validate_protocol_state_runtime_expr(&body_env, &let_expr.body)
        }
        Expr::Using(using_expr) => {
            validate_protocol_state_runtime_expr(env, &using_expr.value)?;
            let value_type = synthesize(env, &using_expr.value)?;
            let body_env = if let InferenceType::Owned(inner_type) = value_type {
                let mut bindings = HashMap::new();
                bindings.insert(using_expr.name.clone(), (*inner_type).clone());
                env.extend(Some(bindings))
            } else {
                env.clone()
            };
            validate_protocol_state_runtime_expr(&body_env, &using_expr.body)
        }
        Expr::If(if_expr) => {
            validate_protocol_state_runtime_expr(env, &if_expr.condition)?;
            validate_protocol_state_runtime_expr(env, &if_expr.then_branch)?;
            if let Some(else_branch) = &if_expr.else_branch {
                validate_protocol_state_runtime_expr(env, else_branch)?;
            }
            Ok(())
        }
        Expr::List(list) => {
            for element in &list.elements {
                validate_protocol_state_runtime_expr(env, element)?;
            }
            Ok(())
        }
        Expr::Record(record) => {
            for field in &record.fields {
                validate_protocol_state_runtime_expr(env, &field.value)?;
            }
            Ok(())
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_protocol_state_runtime_expr(env, &entry.key)?;
                validate_protocol_state_runtime_expr(env, &entry.value)?;
            }
            Ok(())
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_protocol_state_runtime_expr(env, element)?;
            }
            Ok(())
        }
        Expr::FieldAccess(field_access) => {
            validate_protocol_state_runtime_expr(env, &field_access.object)?;
            if field_access.field == "state" {
                let object_type = synthesize(env, &field_access.object)?;
                if resolve_protocol_type_name(&env.normalize_type(&object_type), env).is_some() {
                    return Err(TypeError::new(
                        "Protocol state access is contract-only; use handle.state only in requires/ensures clauses".to_string(),
                        Some(field_access.location),
                    ));
                }
            }
            Ok(())
        }
        Expr::Index(index_expr) => {
            validate_protocol_state_runtime_expr(env, &index_expr.object)?;
            validate_protocol_state_runtime_expr(env, &index_expr.index)
        }
        Expr::Pipeline(pipeline) => {
            validate_protocol_state_runtime_expr(env, &pipeline.left)?;
            validate_protocol_state_runtime_expr(env, &pipeline.right)
        }
        Expr::Map(map_expr) => {
            validate_protocol_state_runtime_expr(env, &map_expr.list)?;
            validate_protocol_state_runtime_expr(env, &map_expr.func)
        }
        Expr::Filter(filter_expr) => {
            validate_protocol_state_runtime_expr(env, &filter_expr.list)?;
            validate_protocol_state_runtime_expr(env, &filter_expr.predicate)
        }
        Expr::Fold(fold_expr) => {
            validate_protocol_state_runtime_expr(env, &fold_expr.list)?;
            validate_protocol_state_runtime_expr(env, &fold_expr.func)?;
            validate_protocol_state_runtime_expr(env, &fold_expr.init)
        }
        Expr::Concurrent(concurrent_expr) => {
            validate_protocol_state_runtime_expr(env, &concurrent_expr.width)?;
            if let Some(policy) = &concurrent_expr.policy {
                for field in &policy.fields {
                    validate_protocol_state_runtime_expr(env, &field.value)?;
                }
            }
            for step in &concurrent_expr.steps {
                match step {
                    sigil_ast::ConcurrentStep::Spawn(spawn) => {
                        validate_protocol_state_runtime_expr(env, &spawn.expr)?;
                    }
                    sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_protocol_state_runtime_expr(env, &spawn_each.list)?;
                        validate_protocol_state_runtime_expr(env, &spawn_each.func)?;
                    }
                }
            }
            Ok(())
        }
        Expr::TypeAscription(type_asc) => validate_protocol_state_runtime_expr(env, &type_asc.expr),
    }
}

fn constraint_value_type_for_decl(
    env: &TypeEnvironment,
    type_decl: &TypeDecl,
) -> Result<InferenceType, TypeError> {
    let type_param_env = make_type_param_env(&type_decl.type_params);
    match &type_decl.definition {
        TypeDef::Alias(alias) => {
            ast_type_to_inference_type_resolved(env, Some(&type_param_env), &alias.aliased_type)
        }
        TypeDef::Product(product) => {
            let mut fields = HashMap::new();
            for field in &product.fields {
                fields.insert(
                    field.name.clone(),
                    ast_type_to_inference_type_resolved(
                        env,
                        Some(&type_param_env),
                        &field.field_type,
                    )?,
                );
            }
            Ok(InferenceType::Record(TRecord { fields, name: None }))
        }
        TypeDef::Sum(_) => Ok(InferenceType::Constructor(TConstructor {
            name: type_decl.name.clone(),
            type_args: type_decl
                .type_params
                .iter()
                .filter_map(|name| type_param_env.get(name).cloned())
                .collect(),
        })),
    }
}

/// Returns true if the expression is a protocol state assertion (involves `.state` field access).
/// Such clauses are axiomatic — state transitions are declared, not proven from the body.
fn ensures_is_state_assertion(expr: &Expr) -> bool {
    match expr {
        Expr::Binary(b) => {
            matches!(b.operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
                && (expr_has_state_access(&b.left) || expr_has_state_access(&b.right))
        }
        Expr::Unary(u) => ensures_is_state_assertion(&u.operand),
        _ => false,
    }
}

fn expr_has_state_access(expr: &Expr) -> bool {
    match expr {
        Expr::FieldAccess(f) => f.field == "state",
        _ => false,
    }
}

pub(crate) fn expr_location(expr: &Expr) -> sigil_lexer::SourceLocation {
    match expr {
        Expr::Literal(expr) => expr.location,
        Expr::Identifier(expr) => expr.location,
        Expr::Lambda(expr) => expr.location,
        Expr::Application(expr) => expr.location,
        Expr::Binary(expr) => expr.location,
        Expr::Unary(expr) => expr.location,
        Expr::Match(expr) => expr.location,
        Expr::Let(expr) => expr.location,
        Expr::Using(expr) => expr.location,
        Expr::If(expr) => expr.location,
        Expr::List(expr) => expr.location,
        Expr::Record(expr) => expr.location,
        Expr::MapLiteral(expr) => expr.location,
        Expr::Tuple(expr) => expr.location,
        Expr::FieldAccess(expr) => expr.location,
        Expr::Index(expr) => expr.location,
        Expr::Pipeline(expr) => expr.location,
        Expr::Map(expr) => expr.location,
        Expr::Filter(expr) => expr.location,
        Expr::Fold(expr) => expr.location,
        Expr::Concurrent(expr) => expr.location,
        Expr::MemberAccess(expr) => expr.location,
        Expr::TypeAscription(expr) => expr.location,
    }
}

fn fresh_resource_scope_id() -> u32 {
    NEXT_RESOURCE_SCOPE_ID.fetch_add(1, Ordering::SeqCst)
}

fn borrowed_type(resource_type: InferenceType, scope_id: u32) -> InferenceType {
    InferenceType::Borrowed(Box::new(TBorrowed {
        resource_type,
        scope_id,
    }))
}

fn type_contains_owned(typ: &InferenceType) -> bool {
    match typ {
        InferenceType::Owned(_) => true,
        InferenceType::Borrowed(borrowed) => type_contains_owned(&borrowed.resource_type),
        InferenceType::Function(function) => {
            function.params.iter().any(type_contains_owned)
                || type_contains_owned(&function.return_type)
        }
        InferenceType::List(list) => type_contains_owned(&list.element_type),
        InferenceType::Map(map) => {
            type_contains_owned(&map.key_type) || type_contains_owned(&map.value_type)
        }
        InferenceType::Tuple(tuple) => tuple.types.iter().any(type_contains_owned),
        InferenceType::Record(record) => record.fields.values().any(type_contains_owned),
        InferenceType::Constructor(constructor) => {
            constructor.type_args.iter().any(type_contains_owned)
        }
        InferenceType::Primitive(_) | InferenceType::Var(_) | InferenceType::Any => false,
    }
}

fn type_contains_borrowed_scope(typ: &InferenceType, scope_id: u32) -> bool {
    match typ {
        InferenceType::Borrowed(borrowed) => {
            borrowed.scope_id == scope_id
                || type_contains_borrowed_scope(&borrowed.resource_type, scope_id)
        }
        InferenceType::Owned(inner) => type_contains_borrowed_scope(inner, scope_id),
        InferenceType::Function(function) => {
            function
                .params
                .iter()
                .any(|param| type_contains_borrowed_scope(param, scope_id))
                || type_contains_borrowed_scope(&function.return_type, scope_id)
        }
        InferenceType::List(list) => type_contains_borrowed_scope(&list.element_type, scope_id),
        InferenceType::Map(map) => {
            type_contains_borrowed_scope(&map.key_type, scope_id)
                || type_contains_borrowed_scope(&map.value_type, scope_id)
        }
        InferenceType::Tuple(tuple) => tuple
            .types
            .iter()
            .any(|item| type_contains_borrowed_scope(item, scope_id)),
        InferenceType::Record(record) => record
            .fields
            .values()
            .any(|field_type| type_contains_borrowed_scope(field_type, scope_id)),
        InferenceType::Constructor(constructor) => constructor
            .type_args
            .iter()
            .any(|arg| type_contains_borrowed_scope(arg, scope_id)),
        InferenceType::Primitive(_) | InferenceType::Var(_) | InferenceType::Any => false,
    }
}

fn reject_owned_aggregate_members(
    kind: &str,
    location: sigil_lexer::SourceLocation,
    member_types: impl IntoIterator<Item = InferenceType>,
) -> Result<(), TypeError> {
    if member_types
        .into_iter()
        .any(|member_type| type_contains_owned(&member_type))
    {
        return Err(TypeError::new(
            format!(
                "{} values may not be stored inside {} literals",
                "Owned", kind
            ),
            Some(location),
        ));
    }
    Ok(())
}

fn lookup_constrained_type_info(
    env: &TypeEnvironment,
    typ: &InferenceType,
) -> Option<(String, TypeInfo, Vec<InferenceType>)> {
    let InferenceType::Constructor(constructor) = typ else {
        return None;
    };

    if let Some((module_path, type_name)) = split_qualified_constructor_name(&constructor.name) {
        let info = env.lookup_qualified_type(&module_path, &type_name)?;
        if info.constraint.is_some() {
            return Some((
                constructor.name.clone(),
                info,
                constructor.type_args.clone(),
            ));
        }
    }

    let info = env.lookup_type(&constructor.name)?;
    if info.constraint.is_some() {
        return Some((
            constructor.name.clone(),
            info,
            constructor.type_args.clone(),
        ));
    }

    None
}

fn matches_expected_type(
    env: &TypeEnvironment,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
) -> bool {
    let (normalized_actual, normalized_expected) = canonical_pair(env, actual_type, expected_type);
    if matches!(
        normalized_actual,
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Never
        })
    ) {
        return true;
    }
    if types_equal(&normalized_actual, &normalized_expected) {
        return true;
    }

    if let Ok(subst) = unify(&normalized_actual, &normalized_expected) {
        let unified_actual = apply_subst(&subst, &normalized_actual);
        let unified_expected = apply_subst(&subst, &normalized_expected);
        return types_equal(&unified_actual, &unified_expected);
    }

    false
}

fn same_branch_type(
    env: &TypeEnvironment,
    left: &InferenceType,
    right: &InferenceType,
) -> Result<bool, TypeError> {
    let (normalized_left, normalized_right) = canonical_pair(env, left, right);
    if types_equal(&normalized_left, &normalized_right) {
        return Ok(true);
    }

    if let Ok(subst) = unify(&normalized_left, &normalized_right) {
        let unified_left = apply_subst(&subst, &normalized_left);
        let unified_right = apply_subst(&subst, &normalized_right);
        return Ok(types_equal(&unified_left, &unified_right));
    }

    Ok(false)
}

fn is_never_type(env: &TypeEnvironment, typ: &InferenceType) -> bool {
    let normalized = env.normalize_type(typ);
    matches!(
        normalized,
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Never
        })
    )
}

fn type_join_with_never(
    env: &TypeEnvironment,
    left: &InferenceType,
    right: &InferenceType,
) -> Result<Option<InferenceType>, TypeError> {
    let left_is_never = is_never_type(env, left);
    let right_is_never = is_never_type(env, right);

    if left_is_never && right_is_never {
        return Ok(Some(never_type()));
    }
    if left_is_never {
        return Ok(Some(right.clone()));
    }
    if right_is_never {
        return Ok(Some(left.clone()));
    }
    if same_branch_type(env, left, right)? {
        return Ok(Some(left.clone()));
    }
    Ok(None)
}

#[derive(Clone, Copy)]
struct TerminatorInfo {
    kind: &'static str,
    location: SourceLocation,
}

fn process_exit_call(expr: &Expr) -> bool {
    let expr = match expr {
        Expr::TypeAscription(ascribed) => &ascribed.expr,
        _ => expr,
    };
    let Expr::Application(app) = expr else {
        return false;
    };
    match &app.func {
        Expr::MemberAccess(member) => {
            member
                .namespace
                .last()
                .is_some_and(|segment| segment == "process")
                && member.member == "exit"
        }
        Expr::FieldAccess(field_access) => {
            field_access.field == "exit"
                && matches!(
                    &field_access.object,
                    Expr::Identifier(identifier) if identifier.name == "process"
                )
        }
        _ => false,
    }
}

fn terminating_expr_info(
    env: &TypeEnvironment,
    expr: &Expr,
) -> Result<Option<TerminatorInfo>, TypeError> {
    let expr_type = synthesize(env, expr)?;
    if !is_never_type(env, &expr_type) {
        return Ok(None);
    }

    if process_exit_call(expr) {
        return Ok(Some(TerminatorInfo {
            kind: "processExit",
            location: expr_location(expr),
        }));
    }

    match expr {
        Expr::If(if_expr) => {
            if let Some(else_branch) = &if_expr.else_branch {
                let then_terminates = terminating_expr_info(env, &if_expr.then_branch)?.is_some();
                let else_terminates = terminating_expr_info(env, else_branch)?.is_some();
                if then_terminates && else_terminates {
                    return Ok(Some(TerminatorInfo {
                        kind: "if",
                        location: if_expr.location,
                    }));
                }
            }
        }
        Expr::Match(match_expr) => {
            let all_terminate = match_expr
                .arms
                .iter()
                .try_fold(true, |all_terminating, arm| {
                    terminating_expr_info(env, &arm.body)
                        .map(|info| all_terminating && info.is_some())
                })?;
            if all_terminate {
                return Ok(Some(TerminatorInfo {
                    kind: "match",
                    location: match_expr.location,
                }));
            }
        }
        _ => {}
    }

    Ok(Some(TerminatorInfo {
        kind: "neverType",
        location: expr_location(expr),
    }))
}

fn unreachable_code_error(
    body: &Expr,
    terminator: TerminatorInfo,
    unreachable_kind: &'static str,
) -> TypeError {
    TypeError::new(
        "Unreachable code after terminating expression".to_string(),
        Some(expr_location(body)),
    )
    .with_code(codes::typecheck::UNREACHABLE_CODE)
    .with_detail("proofBasis", "syntactic/control-flow certainty only")
    .with_detail("terminatorKind", terminator.kind)
    .with_detail(
        "terminatorLocation",
        serde_json::json!({
            "start": {
                "line": terminator.location.start.line,
                "column": terminator.location.start.column,
                "offset": terminator.location.start.offset,
            },
            "end": {
                "line": terminator.location.end.line,
                "column": terminator.location.end.column,
                "offset": terminator.location.end.offset,
            },
        }),
    )
    .with_detail("unreachableKind", unreachable_kind)
}

fn underlying_type_for_constrained_info(
    env: &TypeEnvironment,
    type_info: &TypeInfo,
    type_name: &str,
    type_args: &[InferenceType],
) -> Result<Option<InferenceType>, TypeError> {
    let type_param_env: TypeParamEnv = type_info
        .type_params
        .iter()
        .cloned()
        .zip(type_args.iter().cloned())
        .collect();

    match &type_info.definition {
        TypeDef::Alias(alias) => Ok(Some(ast_type_to_inference_type_resolved(
            env,
            Some(&type_param_env),
            &alias.aliased_type,
        )?)),
        TypeDef::Product(product) => {
            let mut fields = HashMap::new();
            for field in &product.fields {
                fields.insert(
                    field.name.clone(),
                    ast_type_to_inference_type_resolved(
                        env,
                        Some(&type_param_env),
                        &field.field_type,
                    )?,
                );
            }
            Ok(Some(InferenceType::Record(TRecord {
                fields,
                name: Some(type_name.to_string()),
            })))
        }
        TypeDef::Sum(_) => Ok(None),
    }
}

#[derive(Debug, Clone)]
struct ResolvedConstrainedType {
    name: String,
    constraint: Expr,
    underlying_type: InferenceType,
}

fn resolve_constrained_type(
    env: &TypeEnvironment,
    typ: &InferenceType,
) -> Result<Option<ResolvedConstrainedType>, TypeError> {
    let Some((type_name, type_info, type_args)) = lookup_constrained_type_info(env, typ) else {
        return Ok(None);
    };

    let Some(underlying_type) =
        underlying_type_for_constrained_info(env, &type_info, &type_name, &type_args)?
    else {
        return Err(TypeError::new(
            format!(
                "Constrained type '{}' must be an alias or product type",
                type_name
            ),
            None,
        ));
    };

    Ok(type_info
        .constraint
        .clone()
        .map(|constraint| ResolvedConstrainedType {
            name: type_name,
            constraint,
            underlying_type,
        }))
}

fn symbolic_value_for_type_path(
    env: &TypeEnvironment,
    typ: &InferenceType,
    path: &SymbolPath,
) -> Result<SymbolicValue, String> {
    if let Some(constrained) = resolve_constrained_type(env, typ).map_err(|err| err.message)? {
        return symbolic_value_for_type_path(env, &constrained.underlying_type, path);
    }

    // Protocol-typed handles get a State symbolic value keyed by the binding path.
    // Named product types normalize to Record with a `name` field — check both shapes.
    let normalized = env.normalize_type(typ);
    if let Some(protocol_name) = resolve_protocol_type_name(&normalized, env) {
        return Ok(SymbolicValue::State {
            path: path.clone(),
            protocol: protocol_name,
        });
    }

    match env.normalize_type(typ) {
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        }) => Ok(SymbolicValue::Int(LinearExpr::from_path(path.clone()))),
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool,
        }) => Ok(SymbolicValue::Bool(Formula::Atom(Atom::BoolEq {
            path: path.clone(),
            value: true,
        }))),
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::String,
        })
        | InferenceType::List(_)
        | InferenceType::Map(_) => Ok(SymbolicValue::Collection(SymbolicCollection::Path(
            path.clone(),
        ))),
        InferenceType::Record(record) => Ok(SymbolicValue::Record(SymbolicRecord::Path {
            base: path.clone(),
            fields: record.fields,
        })),
        other => Err(format!(
            "values of type {} are not part of Sigil's canonical refinement proof fragment",
            format_type(&other)
        )),
    }
}

fn collect_identifier_assumption(
    env: &TypeEnvironment,
    name: &str,
    collector: &mut AssumptionCollector,
) -> Result<(), String> {
    if !collector.seen_bindings.insert(name.to_string()) {
        return Ok(());
    }

    let Some(binding_type) = env.lookup(name) else {
        return Ok(());
    };

    let Some(constrained) =
        resolve_constrained_type(env, &binding_type).map_err(|err| err.message)?
    else {
        return Ok(());
    };

    let placeholder =
        symbolic_value_for_type_path(env, &constrained.underlying_type, &SymbolPath::root(name))?;
    let assumption = symbolic_formula_from_expr(
        env,
        &ProofContext::default(),
        &constrained.constraint,
        Some(&placeholder),
        &mut AssumptionCollector::default(),
    )?;
    collector.assumptions.push(assumption);
    Ok(())
}

fn symbolic_value_from_expr(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    value_binding: Option<&SymbolicValue>,
    collector: &mut AssumptionCollector,
) -> Result<SymbolicValue, String> {
    match expr {
        Expr::Literal(literal) => match literal.value {
            LiteralValue::Int(value) => Ok(SymbolicValue::Int(LinearExpr::int(value))),
            LiteralValue::Bool(value) => Ok(SymbolicValue::Bool(if value {
                Formula::True
            } else {
                Formula::False
            })),
            LiteralValue::String(ref value) => Ok(SymbolicValue::Collection(
                SymbolicCollection::KnownLength(LinearExpr::int(value.chars().count() as i64)),
            )),
            _ => Err(
                "only Int, Bool, and String literals participate in refinement proofs".to_string(),
            ),
        },
        Expr::Identifier(identifier) => {
            if identifier.name == "value" {
                if let Some(value_binding) = value_binding {
                    return Ok(value_binding.clone());
                }
            }

            if let Some(symbolic_binding) = proof_context.lookup_symbolic_binding(&identifier.name)
            {
                return Ok(symbolic_binding);
            }

            // UpperCamelCase identifiers not bound as values may be state labels.
            // (Sum-type constructors are registered as regular bindings, so lookup catches them.)
            if identifier
                .name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && env.lookup(&identifier.name).is_none()
            {
                return Ok(SymbolicValue::StateLabel(identifier.name.clone()));
            }

            collect_identifier_assumption(env, &identifier.name, collector)?;
            let Some(binding_type) = env.lookup(&identifier.name) else {
                return Err(format!(
                    "unknown identifier '{}' in refinement proof",
                    identifier.name
                ));
            };
            // Try the standard symbolic path first.
            if let Ok(value) = symbolic_value_for_type_path(
                env,
                &binding_type,
                &SymbolPath::root(&identifier.name),
            ) {
                return Ok(value);
            }
            // Fallback: if the proof context has a StateEq assumption about this binding,
            // the identifier is a protocol-typed handle — recover the protocol name from it.
            let path = SymbolPath::root(&identifier.name);
            for assumption in &proof_context.assumptions {
                if let Formula::Atom(Atom::StateEq {
                    path: assumption_path,
                    protocol,
                    ..
                }) = assumption
                {
                    if assumption_path == &path {
                        return Ok(SymbolicValue::State {
                            path,
                            protocol: protocol.clone(),
                        });
                    }
                }
            }
            Err(format!(
                "values of type {} are not part of Sigil's canonical refinement proof fragment",
                format_type(&binding_type)
            ))
        }
        Expr::Unary(unary) => {
            if unary.operator == UnaryOperator::Length {
                return match &unary.operand {
                    Expr::List(list) => Ok(SymbolicValue::Int(LinearExpr::int(
                        list.elements.len() as i64,
                    ))),
                    Expr::MapLiteral(map) => Ok(SymbolicValue::Int(LinearExpr::int(
                        map.entries.len() as i64,
                    ))),
                    _ => {
                        let operand = symbolic_value_from_expr(
                            env,
                            proof_context,
                            &unary.operand,
                            value_binding,
                            collector,
                        )?;
                        match operand {
                            SymbolicValue::Collection(collection) => {
                                Ok(SymbolicValue::Int(collection.length_expr()))
                            }
                            _ => Err(
                                "length in refinement proofs requires a String, list, or map operand"
                                    .to_string(),
                            ),
                        }
                    }
                };
            }

            let operand = symbolic_value_from_expr(
                env,
                proof_context,
                &unary.operand,
                value_binding,
                collector,
            )?;
            match (unary.operator, operand) {
                (UnaryOperator::Negate, SymbolicValue::Int(value)) => {
                    Ok(SymbolicValue::Int(value.negate()))
                }
                (UnaryOperator::Not, SymbolicValue::Bool(formula)) => {
                    Ok(SymbolicValue::Bool(Formula::Not(Box::new(formula))))
                }
                _ => Err("unsupported unary operator in refinement proof".to_string()),
            }
        }
        Expr::Binary(binary) => {
            let left = symbolic_value_from_expr(
                env,
                proof_context,
                &binary.left,
                value_binding,
                collector,
            )?;
            let right = symbolic_value_from_expr(
                env,
                proof_context,
                &binary.right,
                value_binding,
                collector,
            )?;
            // Protocol state equality requires env for state index lookup — handle before
            // falling into the generic symbolic_value_from_binary which has no env.
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                let equals = binary.operator == BinaryOperator::Equal;
                let state_result = match (&left, &right) {
                    (
                        SymbolicValue::State { path, protocol },
                        SymbolicValue::StateLabel(state_name),
                    )
                    | (
                        SymbolicValue::StateLabel(state_name),
                        SymbolicValue::State { path, protocol },
                    ) => Some(state_equality_formula(
                        env, path, protocol, state_name, equals,
                    )),
                    _ => None,
                };
                if let Some(result) = state_result {
                    return result;
                }
            }
            symbolic_value_from_binary(binary.operator, left, right)
        }
        Expr::List(list) => Ok(SymbolicValue::Collection(SymbolicCollection::KnownLength(
            LinearExpr::int(list.elements.len() as i64),
        ))),
        Expr::MapLiteral(map) => Ok(SymbolicValue::Collection(SymbolicCollection::KnownLength(
            LinearExpr::int(map.entries.len() as i64),
        ))),
        Expr::Filter(f) => {
            // Filter never increases list length. This is required for `decreases` proofs when
            // a recursive call passes `(xs filter …)` in place of a list-typed parameter.
            let list_sym =
                symbolic_value_from_expr(env, proof_context, &f.list, value_binding, collector)?;
            let SymbolicValue::Collection(sc) = list_sym else {
                return Err(
                    "filter: list did not lower to a collection in refinement proof".to_string(),
                );
            };
            let p_in = match sc {
                SymbolicCollection::Path(p) => p,
                SymbolicCollection::KnownLength(_) => {
                    return Err(
                        "filter: need a path-backed list to relate output length in proofs"
                            .to_string(),
                    );
                }
            };
            let p_out = SymbolPath::root(&format!("$filter_result_{}", f.location.start.offset));
            let len_in = LinearExpr::from_path(p_in.length());
            let len_out = LinearExpr::from_path(p_out.length());
            let ax = linear_compare(len_out, ComparisonOp::Le, len_in);
            collector.assumptions.push(ax);
            Ok(SymbolicValue::Collection(SymbolicCollection::Path(p_out)))
        }
        Expr::Record(record) => Ok(SymbolicValue::Record(SymbolicRecord::Literal(
            record
                .fields
                .iter()
                .map(|field| (field.name.clone(), field.value.clone()))
                .collect(),
        ))),
        Expr::FieldAccess(field_access) => {
            // `.state` on a protocol-typed binding produces a State symbolic value.
            if field_access.field == "state" {
                // Try type synthesis first (works when the identifier is in scope normally).
                if let Ok(base_type) = synthesize(env, &field_access.object) {
                    if let Some(type_name) = resolve_protocol_type_name(&base_type, env) {
                        let path = match &field_access.object {
                            Expr::Identifier(ident) => SymbolPath::root(&ident.name),
                            _ => SymbolPath::root("$handle"),
                        };
                        return Ok(SymbolicValue::State {
                            path,
                            protocol: type_name,
                        });
                    }
                }
                // Fallback: if the base is an identifier bound as a State in the proof context
                // (e.g. `result` in `call_ensure_assumptions`), propagate it directly.
                if let Expr::Identifier(ident) = &field_access.object {
                    if let Some(SymbolicValue::State { path, protocol }) =
                        proof_context.lookup_symbolic_binding(&ident.name)
                    {
                        return Ok(SymbolicValue::State { path, protocol });
                    }
                }
            }

            match symbolic_value_from_expr(
                env,
                proof_context,
                &field_access.object,
                value_binding,
                collector,
            )? {
                SymbolicValue::Record(SymbolicRecord::Literal(fields)) => {
                    let Some(field_expr) = fields.get(&field_access.field) else {
                        return Err(format!(
                            "record literal is missing field '{}' in refinement proof",
                            field_access.field
                        ));
                    };
                    symbolic_value_from_expr(
                        env,
                        proof_context,
                        field_expr,
                        value_binding,
                        collector,
                    )
                }
                SymbolicValue::Record(SymbolicRecord::Path { base, fields }) => {
                    let Some(field_type) = fields.get(&field_access.field) else {
                        return Err(format!(
                            "record type is missing field '{}' in refinement proof",
                            field_access.field
                        ));
                    };
                    symbolic_value_for_type_path(env, field_type, &base.field(&field_access.field))
                }
                _ => Err("field access in refinement proofs requires an exact record".to_string()),
            }
        }
        Expr::TypeAscription(type_asc) => {
            let inner = symbolic_value_from_expr(
                env,
                proof_context,
                &type_asc.expr,
                value_binding,
                collector,
            )?;
            // If the ascribed type is protocol-registered, inject the initial state assumption so
            // that inline protocol-typed values (e.g. `({id:"x"}:Ticket)`) can satisfy state requires.
            if let Ok(ascribed_type) = synthesize(env, expr) {
                let normalized = env.normalize_type(&ascribed_type);
                if let Some(protocol_name) = resolve_protocol_type_name(&normalized, env) {
                    if let Some(spec) = env.lookup_protocol(&protocol_name) {
                        let state_index = spec
                            .states
                            .iter()
                            .position(|s| s == &spec.initial)
                            .unwrap_or(0) as i64;
                        let path = SymbolPath::root("$inline_handle");
                        collector.assumptions.push(Formula::Atom(Atom::StateEq {
                            path: path.clone(),
                            state_index,
                            protocol: protocol_name.clone(),
                        }));
                        return Ok(SymbolicValue::State {
                            path,
                            protocol: protocol_name,
                        });
                    }
                }
            }
            Ok(inner)
        }
        Expr::Application(app) => {
            let Some(contract) = lookup_contract_for_call(env, &app.func) else {
                return Err("unsupported expression shape in refinement proof".to_string());
            };

            let return_type = synthesize_application(env, app).map_err(|error| error.message)?;
            let result_symbolic =
                symbolic_value_for_type_path(env, &return_type, &SymbolPath::root("$call_result"))?;
            let assumptions = call_ensure_assumptions(
                env,
                proof_context,
                &contract,
                &app.args,
                Some(result_symbolic.clone()),
            )?;
            collector.assumptions.extend(assumptions);
            Ok(result_symbolic)
        }
        _ => Err("unsupported expression shape in refinement proof".to_string()),
    }
}

fn symbolic_formula_from_expr(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    value_binding: Option<&SymbolicValue>,
    collector: &mut AssumptionCollector,
) -> Result<Formula, String> {
    match symbolic_value_from_expr(env, proof_context, expr, value_binding, collector)? {
        SymbolicValue::Bool(formula) => Ok(formula),
        SymbolicValue::State { .. } | SymbolicValue::StateLabel(_) => Err(
            "protocol state access must appear inside an equality comparison (e.g. handle.state = StateName)"
                .to_string(),
        ),
        _ => Err("refinement expressions must evaluate to Bool".to_string()),
    }
}

fn symbolic_value_from_binary(
    operator: BinaryOperator,
    left: SymbolicValue,
    right: SymbolicValue,
) -> Result<SymbolicValue, String> {
    match operator {
        BinaryOperator::Add => match (left, right) {
            (SymbolicValue::Int(left), SymbolicValue::Int(right)) => {
                Ok(SymbolicValue::Int(left.add(&right)))
            }
            _ => Err("addition in refinement proofs requires Int operands".to_string()),
        },
        BinaryOperator::Subtract => match (left, right) {
            (SymbolicValue::Int(left), SymbolicValue::Int(right)) => {
                Ok(SymbolicValue::Int(left.subtract(&right)))
            }
            _ => Err("subtraction in refinement proofs requires Int operands".to_string()),
        },
        BinaryOperator::Equal => symbolic_equality_formula(left, right, true),
        BinaryOperator::NotEqual => symbolic_equality_formula(left, right, false),
        BinaryOperator::Less => symbolic_int_comparison(left, right, ComparisonOp::Lt),
        BinaryOperator::LessEq => symbolic_int_comparison(left, right, ComparisonOp::Le),
        BinaryOperator::Greater => symbolic_int_comparison(left, right, ComparisonOp::Gt),
        BinaryOperator::GreaterEq => symbolic_int_comparison(left, right, ComparisonOp::Ge),
        BinaryOperator::And => match (left, right) {
            (SymbolicValue::Bool(left), SymbolicValue::Bool(right)) => {
                Ok(SymbolicValue::Bool(formula_and(vec![left, right])))
            }
            _ => Err("and in refinement proofs requires Bool operands".to_string()),
        },
        BinaryOperator::Or => match (left, right) {
            (SymbolicValue::Bool(left), SymbolicValue::Bool(right)) => {
                Ok(SymbolicValue::Bool(formula_or(vec![left, right])))
            }
            _ => Err("or in refinement proofs requires Bool operands".to_string()),
        },
        _ => Err("unsupported binary operator in refinement proof".to_string()),
    }
}

fn symbolic_equality_formula(
    left: SymbolicValue,
    right: SymbolicValue,
    equals: bool,
) -> Result<SymbolicValue, String> {
    match (left, right) {
        (SymbolicValue::Int(left), SymbolicValue::Int(right)) => {
            let diff = left.subtract(&right);
            Ok(SymbolicValue::Bool(Formula::Atom(Atom::IntCmp {
                form: diff.form,
                op: if equals {
                    ComparisonOp::Eq
                } else {
                    ComparisonOp::Ne
                },
                rhs: -diff.constant,
            })))
        }
        (SymbolicValue::Bool(left), SymbolicValue::Bool(right)) => {
            let formula = if equals {
                formula_or(vec![
                    formula_and(vec![left.clone(), right.clone()]),
                    formula_and(vec![
                        Formula::Not(Box::new(left)),
                        Formula::Not(Box::new(right)),
                    ]),
                ])
            } else {
                formula_or(vec![
                    formula_and(vec![left.clone(), Formula::Not(Box::new(right.clone()))]),
                    formula_and(vec![Formula::Not(Box::new(left)), right]),
                ])
            };
            Ok(SymbolicValue::Bool(formula))
        }
        _ => {
            Err("equality in refinement proofs requires matching Int or Bool operands".to_string())
        }
    }
}

/// Resolve the type name of a handle for protocol lookup, peeling resource wrappers.
/// Named product types normalize to `Record` with a `name` field, so we check both shapes.
fn resolve_protocol_type_name(ty: &InferenceType, env: &TypeEnvironment) -> Option<String> {
    match ty {
        InferenceType::Constructor(tcons) => {
            if env.lookup_protocol(&tcons.name).is_some() {
                Some(tcons.name.clone())
            } else {
                None
            }
        }
        InferenceType::Record(record) => {
            if let Some(name) = &record.name {
                if env.lookup_protocol(name).is_some() {
                    return Some(name.clone());
                }
            }
            None
        }
        InferenceType::Owned(inner) => resolve_protocol_type_name(inner, env),
        InferenceType::Borrowed(borrowed) => {
            resolve_protocol_type_name(&borrowed.resource_type, env)
        }
        _ => None,
    }
}

/// Build a protocol state equality formula: `handle.state = StateName`.
fn state_equality_formula(
    env: &TypeEnvironment,
    path: &SymbolPath,
    protocol: &str,
    state_name: &str,
    equals: bool,
) -> Result<SymbolicValue, String> {
    let spec = env
        .lookup_protocol(protocol)
        .ok_or_else(|| format!("unknown protocol '{}'", protocol))?;
    let state_index = spec
        .states
        .iter()
        .position(|s| s == state_name)
        .ok_or_else(|| {
            format!(
                "'{}' is not a valid state in protocol '{}' (valid states: {})",
                state_name,
                protocol,
                spec.states.join(", ")
            )
        })? as i64;
    let formula = Formula::Atom(Atom::StateEq {
        path: path.clone(),
        state_index,
        protocol: protocol.to_string(),
    });
    Ok(SymbolicValue::Bool(if equals {
        formula
    } else {
        Formula::Not(Box::new(formula))
    }))
}

fn symbolic_int_comparison(
    left: SymbolicValue,
    right: SymbolicValue,
    op: ComparisonOp,
) -> Result<SymbolicValue, String> {
    let (SymbolicValue::Int(left), SymbolicValue::Int(right)) = (left, right) else {
        return Err("refinement comparisons require Int operands".to_string());
    };
    let diff = left.subtract(&right);
    Ok(SymbolicValue::Bool(Formula::Atom(Atom::IntCmp {
        form: diff.form,
        op,
        rhs: -diff.constant,
    })))
}

fn validate_refinement_constraint_shape(
    env: &TypeEnvironment,
    type_decl: &TypeDecl,
) -> Result<(), TypeError> {
    let Some(constraint) = &type_decl.constraint else {
        return Ok(());
    };

    if matches!(type_decl.definition, TypeDef::Sum(_)) {
        return Err(TypeError::new(
            format!(
                "Type constraint for '{}' is only supported on alias and product types",
                type_decl.name
            ),
            Some(type_decl.location),
        ));
    }

    let value_type = constraint_value_type_for_decl(env, type_decl)?;
    let placeholder = symbolic_value_for_type_path(env, &value_type, &SymbolPath::root("value"))
        .map_err(|reason| {
            let mut error = refinement_type_support_error(&type_decl.name, &reason);
            error.location = Some(expr_location(constraint));
            error
        })?;
    symbolic_formula_from_expr(
        env,
        &ProofContext::default(),
        constraint,
        Some(&placeholder),
        &mut AssumptionCollector::default(),
    )
    .map_err(|reason| {
        let mut error = refinement_type_support_error(&type_decl.name, &reason);
        error.location = Some(expr_location(constraint));
        error
    })?;

    Ok(())
}

fn lower_symbolic_formula(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    value_binding: Option<&SymbolicValue>,
) -> Result<(Formula, Vec<Formula>), String> {
    let mut collector = AssumptionCollector::default();
    let formula =
        symbolic_formula_from_expr(env, proof_context, expr, value_binding, &mut collector)?;
    Ok((formula, collector.assumptions))
}

fn lower_symbolic_value(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    value_binding: Option<&SymbolicValue>,
) -> Result<(SymbolicValue, Vec<Formula>), String> {
    let mut collector = AssumptionCollector::default();
    let value = symbolic_value_from_expr(env, proof_context, expr, value_binding, &mut collector)?;
    Ok((value, collector.assumptions))
}

fn prove_expr_satisfies_constraint(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    constraint: &Expr,
) -> Result<ConstraintProofResult, String> {
    match expr {
        Expr::If(if_expr) => {
            let (condition_formula, condition_assumptions) =
                lower_symbolic_formula(env, proof_context, &if_expr.condition, None)?;
            let then_context = proof_context
                .with_assumptions_replacing_state(condition_assumptions.clone())
                .with_assumption(condition_formula.clone());
            let then_ok = prove_expr_satisfies_constraint(
                env,
                &then_context,
                &if_expr.then_branch,
                constraint,
            )?;
            if !then_ok.proved() {
                return Ok(then_ok);
            }

            if let Some(else_branch) = &if_expr.else_branch {
                let else_context = proof_context
                    .with_assumptions_replacing_state(condition_assumptions)
                    .with_assumption(Formula::Not(Box::new(condition_formula)));
                prove_expr_satisfies_constraint(env, &else_context, else_branch, constraint)
            } else {
                Ok(ConstraintProofResult::Failed(prove_formula(
                    &proof_context.assumptions,
                    &Formula::False,
                )))
            }
        }
        Expr::Let(let_expr) => {
            let value_type = synthesize(env, &let_expr.value).map_err(|error| error.message)?;
            let mut bindings = HashMap::new();
            if let sigil_ast::Pattern::Identifier(id_pattern) = &let_expr.pattern {
                bindings.insert(id_pattern.name.clone(), value_type.clone());
            }
            let body_env = env.extend(Some(bindings));
            let body_context = let_proof_context(
                env,
                proof_context,
                &let_expr.pattern,
                &let_expr.value,
                &value_type,
            );
            prove_expr_satisfies_constraint(&body_env, &body_context, &let_expr.body, constraint)
        }
        Expr::Match(match_expr) => {
            prove_match_expr_satisfies_constraint(env, proof_context, match_expr, constraint)
        }
        Expr::TypeAscription(type_asc) => {
            prove_expr_satisfies_constraint(env, proof_context, &type_asc.expr, constraint)
        }
        _ => {
            let (actual, actual_assumptions) =
                lower_symbolic_value(env, proof_context, expr, None)?;
            let goal_context =
                proof_context.with_symbolic_bindings([("result".to_string(), actual.clone())]);
            let (goal, goal_assumptions) =
                lower_symbolic_formula(env, &goal_context, constraint, Some(&actual))?;
            let assumptions = proof_context
                .assumptions
                .iter()
                .cloned()
                .chain(actual_assumptions)
                .chain(goal_assumptions)
                .collect::<Vec<_>>();
            let check = prove_formula(&assumptions, &goal);
            match check.outcome {
                SolverOutcome::Proved => Ok(ConstraintProofResult::Proved),
                _ => Ok(ConstraintProofResult::Failed(check)),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MatchArmRefinement {
    pub(crate) body_context: ProofContext,
    pub(crate) condition_formula: Option<Formula>,
    pub(crate) guard_supported: bool,
    pub(crate) unsupported_facts: Vec<String>,
}

pub(crate) fn match_scrutinee_symbolic_root() -> SymbolPath {
    SymbolPath::root(MATCH_SCRUTINEE_BINDING)
}

/// When the `match` scrutinee is a simple parameter (or other identifier), use
/// that binding as the path root for list-pattern refinements (length, `tail` =
/// `#scrutinee - n`, etc.) so the proof lines up with measures like
/// `decreases #nodes` that use `Identifier(nodes)::__len`. Otherwise fall back to
/// the synthetic `$match_scrutinee` root.
fn scrutinee_path_for_match_refinement(scrutinee: &Expr) -> SymbolPath {
    match scrutinee {
        Expr::Identifier(id) => SymbolPath::root(&id.name),
        _ => match_scrutinee_symbolic_root(),
    }
}

pub(crate) fn scrutinee_proof_context(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    scrutinee: &Expr,
) -> ProofContext {
    if let Ok((_, assumptions)) = lower_symbolic_value(env, proof_context, scrutinee, None) {
        proof_context.with_assumptions_replacing_state(assumptions)
    } else {
        proof_context.clone()
    }
}

fn state_assumption_for_path(
    proof_context: &ProofContext,
    path: &SymbolPath,
    protocol: &str,
) -> Option<i64> {
    proof_context.assumptions.iter().find_map(|assumption| {
        let Formula::Atom(Atom::StateEq {
            path: assumption_path,
            protocol: assumption_protocol,
            state_index,
        }) = assumption
        else {
            return None;
        };
        if assumption_path == path && assumption_protocol == protocol {
            Some(*state_index)
        } else {
            None
        }
    })
}

fn protocol_state_assumptions_for_alias(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    value_type: &InferenceType,
    source_path: Option<&SymbolPath>,
    target_path: &SymbolPath,
) -> Vec<Formula> {
    if let Ok(Some(constrained)) = resolve_constrained_type(env, value_type) {
        return protocol_state_assumptions_for_alias(
            env,
            proof_context,
            &constrained.underlying_type,
            source_path,
            target_path,
        );
    }

    let normalized = env.normalize_type(value_type);
    if let Some(protocol_name) = resolve_protocol_type_name(&normalized, env) {
        let Some(spec) = env.lookup_protocol(&protocol_name) else {
            return Vec::new();
        };
        let initial_state = spec
            .states
            .iter()
            .position(|state| state == &spec.initial)
            .unwrap_or(0) as i64;
        let state_index = source_path
            .and_then(|path| state_assumption_for_path(proof_context, path, &protocol_name))
            .or_else(|| state_assumption_for_path(proof_context, target_path, &protocol_name))
            .unwrap_or(initial_state);
        return vec![Formula::Atom(Atom::StateEq {
            path: target_path.clone(),
            state_index,
            protocol: protocol_name,
        })];
    }

    match normalized {
        InferenceType::Owned(inner) => protocol_state_assumptions_for_alias(
            env,
            proof_context,
            &inner,
            source_path,
            target_path,
        ),
        InferenceType::Borrowed(borrowed) => protocol_state_assumptions_for_alias(
            env,
            proof_context,
            &borrowed.resource_type,
            source_path,
            target_path,
        ),
        InferenceType::Record(record) => {
            let mut assumptions = Vec::new();
            for (field_name, field_type) in sorted_record_field_types(&record) {
                let field_target = target_path.field(&field_name);
                let field_source = source_path.map(|path| path.field(&field_name));
                assumptions.extend(protocol_state_assumptions_for_alias(
                    env,
                    proof_context,
                    &field_type,
                    field_source.as_ref(),
                    &field_target,
                ));
            }
            assumptions
        }
        InferenceType::Tuple(tuple) => {
            let mut assumptions = Vec::new();
            for (index, item_type) in tuple.types.iter().enumerate() {
                let item_target = target_path.tuple_index(index);
                let item_source = source_path.map(|path| path.tuple_index(index));
                assumptions.extend(protocol_state_assumptions_for_alias(
                    env,
                    proof_context,
                    item_type,
                    item_source.as_ref(),
                    &item_target,
                ));
            }
            assumptions
        }
        _ => Vec::new(),
    }
}

fn symbolic_path_for_alias_expr(expr: &Expr) -> Option<SymbolPath> {
    match expr {
        Expr::Identifier(identifier) => Some(SymbolPath::root(&identifier.name)),
        Expr::FieldAccess(field_access) => symbolic_path_for_alias_expr(&field_access.object)
            .map(|path| path.field(&field_access.field)),
        Expr::TypeAscription(type_ascription) => {
            symbolic_path_for_alias_expr(&type_ascription.expr)
        }
        _ => None,
    }
}

fn let_proof_context(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    pattern: &sigil_ast::Pattern,
    value: &Expr,
    value_type: &InferenceType,
) -> ProofContext {
    let binding_name = match pattern {
        sigil_ast::Pattern::Identifier(id_pattern) => Some(id_pattern.name.as_str()),
        _ => None,
    };

    let value_app = match value {
        Expr::Application(app) => Some(app.as_ref()),
        Expr::TypeAscription(type_asc) => {
            if let Expr::Application(app) = &type_asc.expr {
                Some(app.as_ref())
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(app) = value_app {
        if let Some(contract) = lookup_contract_for_call(env, &app.func) {
            let result_path = binding_name
                .map(SymbolPath::root)
                .unwrap_or_else(|| SymbolPath::root("$let_result"));
            if let Ok(result_symbolic) = symbolic_value_for_type_path(env, value_type, &result_path)
            {
                if let Ok(assumptions) = call_ensure_assumptions(
                    env,
                    proof_context,
                    &contract,
                    &app.args,
                    Some(result_symbolic),
                ) {
                    return proof_context
                        .clone()
                        .with_assumptions_replacing_state(assumptions);
                }
            }
        }
    }

    let Some(binding_name) = binding_name else {
        return proof_context.clone();
    };

    let binding_path = SymbolPath::root(binding_name);
    let source_path = symbolic_path_for_alias_expr(value);
    let protocol_assumptions = protocol_state_assumptions_for_alias(
        env,
        proof_context,
        value_type,
        source_path.as_ref(),
        &binding_path,
    );
    if !protocol_assumptions.is_empty() {
        return proof_context.clone().with_assumptions(protocol_assumptions);
    }

    if !matches!(
        env.normalize_type(value_type),
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool,
        })
    ) {
        return proof_context.clone();
    }

    match lower_symbolic_formula(env, proof_context, value, None) {
        Ok((formula, assumptions)) => proof_context
            .with_assumptions_replacing_state(assumptions)
            .with_symbolic_bindings([(binding_name.to_string(), SymbolicValue::Bool(formula))]),
        Err(_) => proof_context.clone(),
    }
}

fn protocol_initial_state_proof_context(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    binding_name: &str,
    value_type: &InferenceType,
) -> ProofContext {
    let assumptions = protocol_state_assumptions_for_alias(
        env,
        proof_context,
        value_type,
        None,
        &SymbolPath::root(binding_name),
    );
    proof_context.clone().with_assumptions(assumptions)
}

fn collect_pattern_protocol_state_assumptions(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    pattern: &sigil_ast::Pattern,
    scrutinee_type: &InferenceType,
    scrutinee_path: &SymbolPath,
    assumptions: &mut Vec<Formula>,
) -> Result<(), TypeError> {
    use sigil_ast::Pattern;

    match pattern {
        Pattern::Wildcard(_) | Pattern::Literal(_) => Ok(()),
        Pattern::Identifier(identifier_pattern) => {
            assumptions.extend(protocol_state_assumptions_for_alias(
                env,
                proof_context,
                scrutinee_type,
                Some(scrutinee_path),
                &SymbolPath::root(&identifier_pattern.name),
            ));
            Ok(())
        }
        Pattern::List(list_pattern) => {
            let InferenceType::List(list_type) = scrutinee_type else {
                return Ok(());
            };

            for (index, pattern) in list_pattern.patterns.iter().enumerate() {
                collect_pattern_protocol_state_assumptions(
                    env,
                    proof_context,
                    pattern,
                    &list_type.element_type,
                    &list_pattern_path(scrutinee_path, index),
                    assumptions,
                )?;
            }

            if let Some(rest_name) = &list_pattern.rest {
                let mut rest_path = scrutinee_path.clone();
                for _ in 0..list_pattern.patterns.len() {
                    rest_path = rest_path.list_tail();
                }
                assumptions.extend(protocol_state_assumptions_for_alias(
                    env,
                    proof_context,
                    scrutinee_type,
                    Some(&rest_path),
                    &SymbolPath::root(rest_name),
                ));
            }

            Ok(())
        }
        Pattern::Tuple(tuple_pattern) => {
            let InferenceType::Tuple(tuple_type) = env.normalize_type(scrutinee_type) else {
                return Ok(());
            };

            for (index, (pattern, item_type)) in tuple_pattern
                .patterns
                .iter()
                .zip(&tuple_type.types)
                .enumerate()
            {
                collect_pattern_protocol_state_assumptions(
                    env,
                    proof_context,
                    pattern,
                    item_type,
                    &scrutinee_path.tuple_index(index),
                    assumptions,
                )?;
            }

            Ok(())
        }
        Pattern::Constructor(constructor_pattern) => {
            let Some(constructor_type) = lookup_constructor_type(
                env,
                &constructor_pattern.module_path,
                &constructor_pattern.name,
            )?
            else {
                return Ok(());
            };

            let InferenceType::Function(ctor_fn) = constructor_type else {
                return Ok(());
            };

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

            for (index, (pattern, param_type)) in constructor_pattern
                .patterns
                .iter()
                .zip(&ctor_fn.params)
                .enumerate()
            {
                collect_pattern_protocol_state_assumptions(
                    env,
                    proof_context,
                    pattern,
                    &apply_subst(&subst, param_type),
                    &scrutinee_path.variant_field(index),
                    assumptions,
                )?;
            }

            Ok(())
        }
        Pattern::Record(record_pattern) => {
            let InferenceType::Record(record_type) = env.normalize_type(scrutinee_type) else {
                return Ok(());
            };

            for field in &record_pattern.fields {
                let Some(field_type) = record_type.fields.get(&field.name) else {
                    continue;
                };
                let field_path = scrutinee_path.field(&field.name);
                match &field.pattern {
                    Some(pattern) => collect_pattern_protocol_state_assumptions(
                        env,
                        proof_context,
                        pattern,
                        field_type,
                        &field_path,
                        assumptions,
                    )?,
                    None => assumptions.extend(protocol_state_assumptions_for_alias(
                        env,
                        proof_context,
                        field_type,
                        Some(&field_path),
                        &SymbolPath::root(&field.name),
                    )),
                }
            }

            Ok(())
        }
    }
}

pub(crate) fn scrutinee_symbolic_value(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    scrutinee: &Expr,
    scrutinee_type: &InferenceType,
) -> Option<SymbolicValue> {
    lower_symbolic_value(env, proof_context, scrutinee, None)
        .ok()
        .map(|(value, _)| value)
        .or_else(|| {
            symbolic_value_for_type_path(env, scrutinee_type, &match_scrutinee_symbolic_root()).ok()
        })
}

fn list_pattern_path(path: &SymbolPath, index: usize) -> SymbolPath {
    let mut next = path.clone();
    for _ in 0..index {
        next = next.list_tail();
    }
    next
}

fn formula_for_literal_at_path(
    env: &TypeEnvironment,
    scrutinee_type: &InferenceType,
    path: &SymbolPath,
    current_symbolic: Option<&SymbolicValue>,
    pattern: &sigil_ast::Pattern,
) -> Option<Formula> {
    use sigil_ast::{Pattern, PatternLiteralValue};
    let Pattern::Literal(literal) = pattern else {
        return None;
    };
    let scrutinee_value = current_symbolic
        .cloned()
        .or_else(|| symbolic_value_for_type_path(env, scrutinee_type, path).ok())?;

    match (&literal.value, &scrutinee_value) {
        (PatternLiteralValue::Bool(value), SymbolicValue::Bool(formula)) => {
            if *value {
                Some(formula.clone())
            } else {
                Some(Formula::Not(Box::new(formula.clone())))
            }
        }
        (PatternLiteralValue::Int(value), SymbolicValue::Int(expr)) => {
            Some(Formula::Atom(Atom::IntCmp {
                form: expr.form.clone(),
                op: ComparisonOp::Eq,
                rhs: value.saturating_sub(expr.constant),
            }))
        }
        _ => None,
    }
}

fn combine_pattern_formulas(parts: Vec<Formula>) -> Option<Formula> {
    if parts.is_empty() {
        None
    } else {
        Some(formula_and(parts))
    }
}

pub(crate) fn sorted_record_field_types(record: &TRecord) -> Vec<(String, InferenceType)> {
    let mut fields = record
        .fields
        .iter()
        .map(|(name, field_type)| (name.clone(), field_type.clone()))
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    fields
}

fn pattern_refinement_formula(
    env: &TypeEnvironment,
    scrutinee_type: &InferenceType,
    path: &SymbolPath,
    current_symbolic: Option<&SymbolicValue>,
    pattern: &sigil_ast::Pattern,
) -> Option<Formula> {
    use sigil_ast::Pattern;

    match pattern {
        Pattern::Wildcard(_) | Pattern::Identifier(_) => None,
        Pattern::Literal(_) => {
            formula_for_literal_at_path(env, scrutinee_type, path, current_symbolic, pattern)
        }
        Pattern::Tuple(tuple_pattern) => {
            let InferenceType::Tuple(tuple_type) = env.normalize_type(scrutinee_type) else {
                return None;
            };
            let mut parts = Vec::new();
            for (index, (item_pattern, item_type)) in tuple_pattern
                .patterns
                .iter()
                .zip(tuple_type.types.iter())
                .enumerate()
            {
                if let Some(formula) = pattern_refinement_formula(
                    env,
                    item_type,
                    &path.tuple_index(index),
                    None,
                    item_pattern,
                ) {
                    parts.push(formula);
                }
            }
            combine_pattern_formulas(parts)
        }
        Pattern::List(list_pattern) => {
            let InferenceType::List(list_type) = env.normalize_type(scrutinee_type) else {
                return None;
            };

            let head_count = list_pattern.patterns.len() as i64;
            let mut parts = Vec::new();
            let length_expr = LinearExpr::from_path(path.length());
            parts.push(Formula::Atom(Atom::IntCmp {
                form: length_expr.form.clone(),
                op: if list_pattern.rest.is_some() {
                    ComparisonOp::Ge
                } else {
                    ComparisonOp::Eq
                },
                rhs: head_count,
            }));
            // With a rest (`.tail`) binding, the tail list is exactly `n` `listTail` steps from the
            // scrutinee; length of that tail is `#scrutinee - n`. This is required for list
            // recursive measures (`decreases #xs`) to prove #tail < #xs in the cons arm.
            if list_pattern.rest.is_some() && head_count > 0 {
                let n_heads = list_pattern.patterns.len();
                let tail_path = (0..n_heads).fold(path.clone(), |p, _| p.list_tail());
                let tail_len = LinearExpr::from_path(tail_path.length());
                let scr_len = LinearExpr::from_path(path.length());
                let expect = scr_len.subtract(&LinearExpr::int(head_count));
                parts.push(linear_compare(tail_len, ComparisonOp::Eq, expect));
            }

            for (index, item_pattern) in list_pattern.patterns.iter().enumerate() {
                let item_path = list_pattern_path(path, index).list_head();
                if let Some(formula) = pattern_refinement_formula(
                    env,
                    &list_type.element_type,
                    &item_path,
                    None,
                    item_pattern,
                ) {
                    parts.push(formula);
                }
            }

            combine_pattern_formulas(parts)
        }
        Pattern::Constructor(constructor_pattern) => {
            let Some(constructor_type) = lookup_constructor_type(
                env,
                &constructor_pattern.module_path,
                &constructor_pattern.name,
            )
            .ok()
            .flatten() else {
                return None;
            };
            let InferenceType::Function(ctor_fn) = constructor_type else {
                return None;
            };
            let subst = unify(&ctor_fn.return_type, scrutinee_type).ok()?;
            let mut parts = Vec::new();
            for (index, (item_pattern, item_type)) in constructor_pattern
                .patterns
                .iter()
                .zip(ctor_fn.params.iter())
                .enumerate()
            {
                if let Some(formula) = pattern_refinement_formula(
                    env,
                    &apply_subst(&subst, item_type),
                    &path.variant_field(index),
                    None,
                    item_pattern,
                ) {
                    parts.push(formula);
                }
            }
            combine_pattern_formulas(parts)
        }
        Pattern::Record(record_pattern) => {
            let InferenceType::Record(record_type) = env.normalize_type(scrutinee_type) else {
                return None;
            };
            let mut parts = Vec::new();
            for field in &record_pattern.fields {
                let field_type = record_type.fields.get(&field.name)?;
                let nested_pattern = field.pattern.as_ref()?;
                if let Some(formula) = pattern_refinement_formula(
                    env,
                    field_type,
                    &path.field(&field.name),
                    None,
                    nested_pattern,
                ) {
                    parts.push(formula);
                }
            }
            combine_pattern_formulas(parts)
        }
    }
}

fn collect_pattern_symbolic_bindings(
    env: &TypeEnvironment,
    pattern: &sigil_ast::Pattern,
    scrutinee_type: &InferenceType,
    scrutinee_path: &SymbolPath,
    current_symbolic: Option<&SymbolicValue>,
    bindings: &mut HashMap<String, SymbolicValue>,
) -> Result<(), TypeError> {
    use sigil_ast::Pattern;

    match pattern {
        Pattern::Wildcard(_) | Pattern::Literal(_) => Ok(()),
        Pattern::Identifier(identifier) => {
            let symbolic = current_symbolic
                .cloned()
                .or_else(|| symbolic_value_for_type_path(env, scrutinee_type, scrutinee_path).ok());
            if let Some(symbolic) = symbolic {
                bindings.insert(identifier.name.clone(), symbolic);
            }
            Ok(())
        }
        Pattern::List(list_pattern) => {
            let InferenceType::List(list_type) = scrutinee_type else {
                return Ok(());
            };

            for (index, pattern) in list_pattern.patterns.iter().enumerate() {
                let mut item_path = scrutinee_path.clone();
                for _ in 0..index {
                    item_path = item_path.list_tail();
                }
                let head_path = item_path.list_head();
                collect_pattern_symbolic_bindings(
                    env,
                    pattern,
                    &list_type.element_type,
                    &head_path,
                    None,
                    bindings,
                )?;
            }

            if let Some(rest_name) = &list_pattern.rest {
                let mut rest_path = scrutinee_path.clone();
                for _ in 0..list_pattern.patterns.len() {
                    rest_path = rest_path.list_tail();
                }
                if let Some(rest_symbolic) =
                    symbolic_value_for_type_path(env, scrutinee_type, &rest_path).ok()
                {
                    bindings.insert(rest_name.clone(), rest_symbolic);
                }
            }

            Ok(())
        }
        Pattern::Tuple(tuple_pattern) => {
            let InferenceType::Tuple(tuple_type) = scrutinee_type else {
                return Ok(());
            };

            for (index, (pattern, item_type)) in tuple_pattern
                .patterns
                .iter()
                .zip(&tuple_type.types)
                .enumerate()
            {
                collect_pattern_symbolic_bindings(
                    env,
                    pattern,
                    item_type,
                    &scrutinee_path.tuple_index(index),
                    None,
                    bindings,
                )?;
            }

            Ok(())
        }
        Pattern::Constructor(constructor_pattern) => {
            let Some(constructor_type) = lookup_constructor_type(
                env,
                &constructor_pattern.module_path,
                &constructor_pattern.name,
            )?
            else {
                return Ok(());
            };

            let InferenceType::Function(ctor_fn) = constructor_type else {
                return Ok(());
            };

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

            for (index, (pattern, param_type)) in constructor_pattern
                .patterns
                .iter()
                .zip(&ctor_fn.params)
                .enumerate()
            {
                collect_pattern_symbolic_bindings(
                    env,
                    pattern,
                    &apply_subst(&subst, param_type),
                    &scrutinee_path.variant_field(index),
                    None,
                    bindings,
                )?;
            }

            Ok(())
        }
        Pattern::Record(record_pattern) => {
            let InferenceType::Record(record_type) = env.normalize_type(scrutinee_type) else {
                return Ok(());
            };

            for field in &record_pattern.fields {
                let Some(field_type) = record_type.fields.get(&field.name) else {
                    continue;
                };
                let field_path = scrutinee_path.field(&field.name);
                match &field.pattern {
                    Some(pattern) => collect_pattern_symbolic_bindings(
                        env,
                        pattern,
                        field_type,
                        &field_path,
                        None,
                        bindings,
                    )?,
                    None => {
                        if let Ok(symbolic) =
                            symbolic_value_for_type_path(env, field_type, &field_path)
                        {
                            bindings.insert(field.name.clone(), symbolic);
                        }
                    }
                }
            }

            Ok(())
        }
    }
}

pub(crate) fn match_arm_refinement(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    scrutinee: &Expr,
    scrutinee_type: &InferenceType,
    arm: &sigil_ast::MatchArm,
) -> Result<MatchArmRefinement, TypeError> {
    let scrutinee_symbolic =
        scrutinee_symbolic_value(env, proof_context, scrutinee, scrutinee_type);
    let mut symbolic_bindings = HashMap::new();
    let match_path = scrutinee_path_for_match_refinement(scrutinee);
    collect_pattern_symbolic_bindings(
        env,
        &arm.pattern,
        scrutinee_type,
        &match_path,
        scrutinee_symbolic.as_ref(),
        &mut symbolic_bindings,
    )?;
    let mut pattern_state_assumptions = Vec::new();
    collect_pattern_protocol_state_assumptions(
        env,
        proof_context,
        &arm.pattern,
        scrutinee_type,
        &match_path,
        &mut pattern_state_assumptions,
    )?;

    let pattern_formula = pattern_refinement_formula(
        env,
        scrutinee_type,
        &match_path,
        scrutinee_symbolic.as_ref(),
        &arm.pattern,
    );
    let mut body_context = proof_context.with_symbolic_bindings(symbolic_bindings);
    if !pattern_state_assumptions.is_empty() {
        body_context = body_context.with_assumptions(pattern_state_assumptions);
    }
    if let Some(pattern_formula) = &pattern_formula {
        body_context = body_context.with_assumption(pattern_formula.clone());
    }

    let mut condition_formula = pattern_formula;
    let mut guard_supported = arm.guard.is_none();
    let mut unsupported_facts = Vec::new();

    if let Some(guard) = &arm.guard {
        match lower_symbolic_formula(env, &body_context, guard, None) {
            Ok((guard_formula, assumptions)) => {
                body_context = body_context
                    .with_assumptions_replacing_state(assumptions)
                    .with_assumption(guard_formula.clone());
                condition_formula = Some(match condition_formula {
                    Some(pattern_formula) => formula_and(vec![pattern_formula, guard_formula]),
                    None => guard_formula,
                });
                guard_supported = true;
            }
            Err(_) => {
                guard_supported = false;
                unsupported_facts.push(expr_summary(guard));
                condition_formula = None;
            }
        }
    }

    Ok(MatchArmRefinement {
        body_context,
        condition_formula,
        guard_supported,
        unsupported_facts,
    })
}

fn prove_match_expr_satisfies_constraint(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    match_expr: &sigil_ast::MatchExpr,
    constraint: &Expr,
) -> Result<ConstraintProofResult, String> {
    let scrutinee_type = synthesize(env, &match_expr.scrutinee).map_err(|error| error.message)?;
    let base_match_context = scrutinee_proof_context(env, proof_context, &match_expr.scrutinee);
    let mut fallthrough_context = base_match_context.clone();

    for arm in &match_expr.arms {
        let mut bindings = HashMap::new();
        check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)
            .map_err(|error| error.message)?;
        let arm_env = env.extend(Some(bindings));
        let arm_refinement = match_arm_refinement(
            env,
            &fallthrough_context,
            &match_expr.scrutinee,
            &scrutinee_type,
            arm,
        )
        .map_err(|error| error.message)?;

        let arm_proof = prove_expr_satisfies_constraint(
            &arm_env,
            &arm_refinement.body_context,
            &arm.body,
            constraint,
        )?;
        if !arm_proof.proved() {
            return Ok(arm_proof);
        }

        if let Some(condition_formula) = arm_refinement.condition_formula {
            fallthrough_context =
                fallthrough_context.with_assumption(Formula::Not(Box::new(condition_formula)));
        }
    }

    Ok(ConstraintProofResult::Proved)
}

fn type_flows_without_new_proof(
    env: &TypeEnvironment,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
) -> Result<bool, TypeError> {
    if matches_expected_type(env, actual_type, expected_type) {
        return Ok(true);
    }

    let Some(constrained) = resolve_constrained_type(env, actual_type)? else {
        return Ok(false);
    };

    type_flows_without_new_proof(env, &constrained.underlying_type, expected_type)
}

fn try_refinement_compatibility(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
    location: sigil_lexer::SourceLocation,
) -> Result<bool, TypeError> {
    try_refinement_compatibility_with_contexts(
        env,
        proof_context,
        proof_context,
        expr,
        actual_type,
        expected_type,
        location,
    )
}

fn try_refinement_compatibility_with_contexts(
    env: &TypeEnvironment,
    check_context: &ProofContext,
    refinement_context: &ProofContext,
    expr: &Expr,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
    location: sigil_lexer::SourceLocation,
) -> Result<bool, TypeError> {
    if let Some(expected_refinement) = resolve_constrained_type(env, expected_type)? {
        check_with_context(
            env,
            check_context,
            expr,
            &expected_refinement.underlying_type,
        )?;
        let proof_result = prove_expr_satisfies_constraint(
            env,
            refinement_context,
            expr,
            &expected_refinement.constraint,
        )
        .map_err(|reason| {
            TypeError::new(
                format!(
                    "Constraint for '{}' could not be proven here: {}",
                    expected_refinement.name, reason,
                ),
                Some(location),
            )
        })?;

        if proof_result.proved() {
            return Ok(true);
        }

        let mut error = TypeError::new(
            format!(
                "Constraint for '{}' could not be proven here",
                expected_refinement.name
            ),
            Some(location),
        );
        if let Some(check) = proof_result.failed_check() {
            error = error
                .with_detail("proof", check)
                .with_detail("proofKind", "refinement")
                .with_detail("proofSummary", proof_outcome_reason(&check.outcome));
        }
        return Err(error);
    }

    if let Some(actual_refinement) = resolve_constrained_type(env, actual_type)? {
        if type_flows_without_new_proof(env, &actual_refinement.underlying_type, expected_type)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn ensure_expr_matches_expected(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
    location: sigil_lexer::SourceLocation,
) -> Result<(), TypeError> {
    ensure_expr_matches_expected_with_contexts(
        env,
        proof_context,
        proof_context,
        expr,
        actual_type,
        expected_type,
        location,
    )
}

fn ensure_expr_matches_expected_with_contexts(
    env: &TypeEnvironment,
    check_context: &ProofContext,
    refinement_context: &ProofContext,
    expr: &Expr,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
    location: sigil_lexer::SourceLocation,
) -> Result<(), TypeError> {
    ensure_label_subset(
        env,
        actual_type,
        expected_type,
        location,
        "Type-directed flow",
    )?;

    if matches_expected_type(env, actual_type, expected_type) {
        return Ok(());
    }

    if try_refinement_compatibility_with_contexts(
        env,
        check_context,
        refinement_context,
        expr,
        actual_type,
        expected_type,
        location,
    )? {
        return Ok(());
    }

    let (normalized_actual, normalized_expected) = canonical_pair(env, actual_type, expected_type);
    Err(TypeError::mismatch(
        format!(
            "Type mismatch: expected {}, got {}",
            format_type(&normalized_expected),
            format_type(&normalized_actual)
        ),
        Some(location),
        normalized_expected,
        normalized_actual,
    ))
}

fn validate_declaration_surface_types(decl: &Declaration) -> Result<(), TypeError> {
    match decl {
        Declaration::Label(_) | Declaration::Rule(_) => Ok(()),
        Declaration::Derive(derive_decl) => validate_surface_type(&derive_decl.target),
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
        Declaration::FeatureFlag(feature_flag_decl) => {
            validate_surface_type(&feature_flag_decl.flag_type)?;
            validate_expr_surface_types(&feature_flag_decl.default)
        }
        Declaration::Transform(transform_decl) => validate_declaration_surface_types(
            &Declaration::Function(transform_decl.function.clone()),
        ),
        Declaration::Function(func_decl) => {
            for param in &func_decl.params {
                if let Some(param_type) = &param.type_annotation {
                    validate_surface_type(param_type)?;
                }
            }

            if let Some(return_type) = &func_decl.return_type {
                validate_surface_type(return_type)?;
            }

            if let Some(requires) = &func_decl.requires {
                validate_expr_surface_types(requires)?;
            }

            if let Some(ensures) = &func_decl.ensures {
                validate_expr_surface_types(ensures)?;
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
        Declaration::Protocol(_) => Ok(()),
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
        Expr::Using(using_expr) => {
            validate_expr_surface_types(&using_expr.value)?;
            validate_expr_surface_types(&using_expr.body)
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

pub(crate) fn create_constructor_type_with_result_name(
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

fn validate_contract_clause(
    env: &TypeEnvironment,
    function_name: &str,
    clause_name: &str,
    expr: &Expr,
) -> Result<(), TypeError> {
    let clause_type = synthesize(env, expr)?;
    if !same_type(env, &clause_type, &bool_type()) {
        return Err(TypeError::new(
            format!(
                "Function '{}' {} clause must return Bool, got {}",
                function_name,
                clause_name,
                format_type(&clause_type)
            ),
            Some(expr_location(expr)),
        ));
    }

    let typed = build_typed_expr(env, expr)?;
    if !typed.effects.is_empty() {
        return Err(TypeError::new(
            format!(
                "Function '{}' {} clause must be pure",
                function_name, clause_name
            ),
            Some(expr_location(expr)),
        ));
    }

    lower_symbolic_formula(env, &ProofContext::default(), expr, None).map_err(|reason| {
        let mut error = TypeError::new(
            format!(
                "Function '{}' {} clause uses unsupported proof syntax: {}",
                function_name, clause_name, reason
            ),
            Some(expr_location(expr)),
        );
        error = error.with_detail("proofKind", "contract");
        error
    })?;

    Ok(())
}

/// Validate a decreases clause: must be Int (single measure) or a tuple of
/// Int expressions (lexicographic measure), pure, and lowerable to the
/// canonical proof fragment.
///
/// Returns the components of the measure as a Vec of Exprs (one for single,
/// many for lex).
fn validate_decreases_clause<'expr>(
    env: &TypeEnvironment,
    function_name: &str,
    expr: &'expr Expr,
) -> Result<Vec<&'expr Expr>, TypeError> {
    let components: Vec<&Expr> = match expr {
        Expr::Tuple(tuple) => {
            if tuple.elements.is_empty() {
                return Err(TypeError::new(
                    format!(
                        "Function '{}' decreases clause cannot be an empty tuple",
                        function_name
                    ),
                    Some(expr_location(expr)),
                )
                .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
            }
            tuple.elements.iter().collect()
        }
        _ => vec![expr],
    };

    for (index, component) in components.iter().enumerate() {
        let component_type = synthesize(env, component)?;
        if !same_type(env, &component_type, &int_type()) {
            return Err(TypeError::new(
                format!(
                    "Function '{}' decreases clause component {} must have type Int, got {}",
                    function_name,
                    index + 1,
                    format_type(&component_type)
                ),
                Some(expr_location(component)),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
        }

        let typed = build_typed_expr(env, component)?;
        if !typed.effects.is_empty() {
            return Err(TypeError::new(
                format!("Function '{}' decreases clause must be pure", function_name),
                Some(expr_location(component)),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
        }

        // Each component must lower to a SymbolicValue::Int in the canonical
        // proof fragment. We lower into a fresh proof context (no extra
        // bindings) just to validate fragment admissibility here. The full
        // proof obligations are discharged in prove_measure_well_founded with
        // the actual function body context.
        let (value, _) =
            lower_symbolic_value(env, &ProofContext::default(), component, None).map_err(|reason| {
                TypeError::new(
                    format!(
                        "Function '{}' decreases clause component {} is not in the canonical proof fragment: {}",
                        function_name,
                        index + 1,
                        reason
                    ),
                    Some(expr_location(component)),
                )
                .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT)
            })?;
        if !matches!(value, SymbolicValue::Int(_)) {
            return Err(TypeError::new(
                format!(
                    "Function '{}' decreases clause component {} did not lower to an Int measure",
                    function_name,
                    index + 1
                ),
                Some(expr_location(component)),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
        }
    }

    Ok(components)
}

/// A list-typed function parameter is treated as having non-negative length for
/// termination: `#param` (length) is a valid well-founded Int measure, without
/// requiring a separate `requires #param≥0` (which is often unprovable at
/// call sites, e.g. for `item:T` in polymorphic `contains`/`fold` callbacks).
/// Also, for record-typed parameters, each field of list type (e.g. `graph.nodes`)
/// is treated as having non-negative length so measures like
/// `decreases (#graph.nodes+(-#visited), #queue)` are bounded below in the entry
/// pass without a separate `requires`.
fn list_param_length_nonneg_assumptions(
    env: &TypeEnvironment,
    params: &[sigil_ast::Param],
) -> Vec<Formula> {
    let mut out = Vec::new();
    for p in params {
        let Some(ty) = env.lookup(&p.name) else {
            continue;
        };
        out.extend(param_list_length_nonneg_for_measure(env, &p.name, &ty));
    }
    out
}

/// Emit `0 ≤` length formulas for a parameter: direct list params and every
/// list-typed field of a record/constructor-wrapped product param.
fn param_list_length_nonneg_for_measure(
    env: &TypeEnvironment,
    param_name: &str,
    ty: &InferenceType,
) -> Vec<Formula> {
    let mut out = Vec::new();
    let nty = env.normalize_type(ty);
    let nty: &InferenceType = match &nty {
        InferenceType::Owned(inner) => inner.as_ref(),
        InferenceType::Borrowed(b) => &b.resource_type,
        other => other,
    };
    match nty {
        InferenceType::List(_)
        | InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::String,
        }) => {
            let len = LinearExpr::from_path(SymbolPath::root(param_name).length());
            out.push(linear_compare(len, ComparisonOp::Ge, LinearExpr::int(0)));
        }
        InferenceType::Record(rec) => {
            for (field, fty) in sorted_record_field_types(rec) {
                if !matches!(env.normalize_type(&fty), InferenceType::List(_)) {
                    continue;
                }
                let plen = LinearExpr::from_path(
                    SymbolPath::root(param_name).field(field.as_str()).length(),
                );
                out.push(linear_compare(plen, ComparisonOp::Ge, LinearExpr::int(0)));
            }
        }
        _ => {}
    }
    out
}

/// A bare `Int` function parameter used as a single-Int `decreases` measure
/// (e.g. `decreases fuel`) is treated as taking values in **ℕ** for the
/// "bounded below by 0" check, matching how list measures use non-negative
/// length without a redundant `requires`. Callers should pass non-negative
/// `fuel` when it is a termination measure; see language docs for the proof
/// fragment.
fn int_param_id_nonneg_assumptions_for_decreases(
    env: &TypeEnvironment,
    params: &[sigil_ast::Param],
    measure_components: &[&Expr],
) -> Vec<Formula> {
    use std::collections::HashSet;
    let param_names: HashSet<&str> = params.iter().map(|p| p.name.as_str()).collect();
    let mut out = Vec::new();
    for component in measure_components {
        let Expr::Identifier(id) = *component else {
            continue;
        };
        if !param_names.contains(id.name.as_str()) {
            continue;
        };
        let Some(pty) = env.lookup(&id.name) else {
            continue;
        };
        let t = env.normalize_type(&pty);
        if !matches!(
            t,
            InferenceType::Primitive(TPrimitive {
                name: PrimitiveName::Int,
            })
        ) {
            continue;
        }
        let p = LinearExpr::from_path(SymbolPath::root(&id.name));
        out.push(linear_compare(p, ComparisonOp::Ge, LinearExpr::int(0)));
    }
    out
}

/// Returns `true` if a parameter name is referenced in one of the decreases
/// components. Used to relax self-call argument lowering: parameters that do
/// not appear in the measure (e.g. `item:T` when the measure is `#xs` only) do
/// not need to be lowered for lexicographic decrease checks.
fn decreases_components_use_param(components: &[&Expr], param_name: &str) -> bool {
    for component in components {
        if expr_mentions_param(component, param_name) {
            return true;
        }
    }
    false
}

fn expr_mentions_param(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Identifier(i) => i.name == name,
        Expr::Unary(u) => expr_mentions_param(&u.operand, name),
        Expr::Binary(b) => {
            expr_mentions_param(&b.left, name) || expr_mentions_param(&b.right, name)
        }
        Expr::TypeAscription(asc) => expr_mentions_param(&asc.expr, name),
        Expr::Tuple(t) => t.elements.iter().any(|e| expr_mentions_param(e, name)),
        Expr::Application(a) => {
            expr_mentions_param(&a.func, name)
                || a.args.iter().any(|a| expr_mentions_param(a, name))
        }
        Expr::FieldAccess(f) => expr_mentions_param(&f.object, name),
        Expr::Index(i) => {
            expr_mentions_param(&i.object, name) || expr_mentions_param(&i.index, name)
        }
        Expr::MemberAccess(_) => false,
        Expr::If(i) => {
            expr_mentions_param(&i.condition, name)
                || expr_mentions_param(&i.then_branch, name)
                || i.else_branch
                    .as_ref()
                    .map_or(false, |b| expr_mentions_param(b, name))
        }
        Expr::Let(l) => expr_mentions_param(&l.value, name) || expr_mentions_param(&l.body, name),
        Expr::Match(m) => {
            expr_mentions_param(&m.scrutinee, name)
                || m.arms
                    .iter()
                    .any(|arm| expr_mentions_param(&arm.body, name))
        }
        // Literals, lists, lambdas, etc. don’t need full handling for
        // function-parameter references in a decreases clause in practice.
        _ => false,
    }
}

/// Walk `body` and discharge the termination obligations for `decreases`:
///   1. Every measure component is bounded below by 0 under the function's
///      preconditions.
///   2. At every recursive call to `function_name` reached during the walk,
///      the substituted measure is strictly smaller than the entry measure
///      (lexicographically for tuple measures).
///
/// The proof context is threaded through match-arms, if/else, let-bindings,
/// and using-bindings so each self-call site is discharged under the exact
/// assumptions accumulated at that site.
fn prove_measure_well_founded(
    env: &TypeEnvironment,
    func_decl: &FunctionDecl,
    measure_components: &[&Expr],
    entry_context: &ProofContext,
) -> Result<(), TypeError> {
    let function_name = &func_decl.name;
    let location = expr_location(
        func_decl
            .decreases
            .as_ref()
            .expect("prove_measure_well_founded called without decreases clause"),
    );
    let list_param_nonneg: Vec<Formula> =
        list_param_length_nonneg_assumptions(env, &func_decl.params);
    let int_param_id_nonneg: Vec<Formula> =
        int_param_id_nonneg_assumptions_for_decreases(env, &func_decl.params, measure_components);

    // Bound check: each entry-component is >= 0 under the function's requires.
    for (index, component) in measure_components.iter().enumerate() {
        let (value, extra_assumptions) = lower_symbolic_value(env, entry_context, component, None)
            .map_err(|reason| {
                TypeError::new(
                    format!(
                        "Function '{}' decreases clause component {} could not be lowered: {}",
                        function_name,
                        index + 1,
                        reason
                    ),
                    Some(location),
                )
                .with_code(sigil_diagnostics::codes::proof::MEASURE_UNBOUNDED_BELOW)
            })?;
        let SymbolicValue::Int(linear) = value else {
            return Err(TypeError::new(
                format!(
                    "Function '{}' decreases clause component {} did not lower to an Int measure",
                    function_name,
                    index + 1
                ),
                Some(location),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
        };

        let goal = linear_compare(linear, ComparisonOp::Ge, LinearExpr::int(0));
        let mut all_assumptions = entry_context.assumptions.clone();
        all_assumptions.extend(list_param_nonneg.iter().cloned());
        all_assumptions.extend(int_param_id_nonneg.iter().cloned());
        all_assumptions.extend(extra_assumptions);
        let check = sigil_solver::prove_formula(&all_assumptions, &goal);
        if !matches!(check.outcome, SolverOutcome::Proved) {
            return Err(TypeError::new(
                format!(
                    "Function '{}' decreases clause component {} could not be proven >= 0",
                    function_name,
                    index + 1
                ),
                Some(location),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_UNBOUNDED_BELOW));
        }
    }

    // Strict-decrease check: walk the body, at each self-call substitute the
    // arguments for parameters and prove `next < entry` lexicographically.
    let entry_values = measure_components
        .iter()
        .map(|component| lower_symbolic_value(env, entry_context, component, None))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|reason| {
            TypeError::new(
                format!(
                    "Function '{}' decreases clause could not be lowered for the entry measure: {}",
                    function_name, reason
                ),
                Some(location),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT)
        })?;

    let mut entry_linears = Vec::with_capacity(entry_values.len());
    let mut entry_extras = Vec::new();
    for (value, extras) in entry_values {
        let SymbolicValue::Int(linear) = value else {
            return Err(TypeError::new(
                format!(
                    "Function '{}' decreases clause did not lower to Int measures",
                    function_name
                ),
                Some(location),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
        };
        entry_linears.push(linear);
        entry_extras.extend(extras);
    }

    let walker_context = entry_context
        .with_assumptions_replacing_state(entry_extras)
        .with_assumptions(list_param_nonneg.iter().cloned())
        .with_assumptions(int_param_id_nonneg.iter().cloned());

    walk_for_decreases(
        env,
        &walker_context,
        &func_decl.body,
        function_name,
        &func_decl.params,
        measure_components,
        &entry_linears,
        location,
        0,
    )
}

const MAX_WALK_FOR_DECREASES_DEPTH: u32 = 4096;

/// Recursively walk an expression, threading proof context, and discharge a
/// strict-decrease obligation at each self-call to `function_name`.
#[allow(clippy::too_many_arguments)]
fn walk_for_decreases(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    function_name: &str,
    params: &[sigil_ast::Param],
    measure_components: &[&Expr],
    entry_linears: &[LinearExpr],
    decreases_location: sigil_lexer::SourceLocation,
    rec_depth: u32,
) -> Result<(), TypeError> {
    if rec_depth > MAX_WALK_FOR_DECREASES_DEPTH {
        return Err(
            TypeError::new(
                "termination measure walk exceeded an internal depth limit; simplify nested `match`/`if` in the function body, or file a bug with a small reproducer".to_string(),
                Some(decreases_location),
            )
            .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT),
        );
    }
    let child_depth = rec_depth + 1;
    match expr {
        Expr::Application(app) => {
            // Recurse into func and args first to find nested self-calls.
            walk_for_decreases(
                env,
                proof_context,
                &app.func,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            for arg in &app.args {
                walk_for_decreases(
                    env,
                    proof_context,
                    arg,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }

            // Self-call detection.
            let is_self_call = matches!(
                &app.func,
                Expr::Identifier(sigil_ast::IdentifierExpr { name, .. }) if name == function_name
            );
            if !is_self_call || app.args.len() != params.len() {
                return Ok(());
            }

            // Build the substituted-argument context: parameters bound to the
            // symbolic values of the call's arguments.
            let mut call_bindings = HashMap::new();
            let mut extra_assumptions = Vec::new();
            for (param, arg) in params.iter().zip(&app.args) {
                let used = decreases_components_use_param(measure_components, &param.name);
                let (value, extras) = match lower_symbolic_value(env, proof_context, arg, None) {
                    Ok(v) => v,
                    Err(_) if !used => {
                        // Measure does not reference this parameter; omit binding (e.g. `item:T`
                        // in `λcontains(x,xs) decreases #xs` — only `xs` is substituted for tail).
                        continue;
                    }
                    Err(_) => {
                        return Err(TypeError::new(
                            format!(
                                "Function '{}' could not lower self-call argument for '{}' to the canonical proof fragment; refactor the call site or the measure",
                                function_name, param.name
                            ),
                            Some(expr_location(arg)),
                        )
                        .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_DECREASING));
                    }
                };
                call_bindings.insert(param.name.clone(), value);
                extra_assumptions.extend(extras);
            }

            let call_context = proof_context
                .with_symbolic_bindings(call_bindings)
                .with_assumptions_replacing_state(extra_assumptions);

            // Compute the next-measure components under call context.
            let mut next_linears = Vec::with_capacity(measure_components.len());
            let mut next_extras = Vec::new();
            for (index, component) in measure_components.iter().enumerate() {
                let (value, extras) =
                    lower_symbolic_value(env, &call_context, component, None).map_err(|reason| {
                        TypeError::new(
                            format!(
                                "Function '{}' could not lower decreases component {} at recursive call: {}",
                                function_name,
                                index + 1,
                                reason
                            ),
                            Some(app.location),
                        )
                        .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_DECREASING)
                    })?;
                let SymbolicValue::Int(linear) = value else {
                    return Err(TypeError::new(
                        format!(
                            "Function '{}' decreases component {} did not lower to Int at self-call",
                            function_name,
                            index + 1
                        ),
                        Some(app.location),
                    )
                    .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_IN_FRAGMENT));
                };
                next_linears.push(linear);
                next_extras.extend(extras);
            }

            // Build the strict-decrease formula. For lex measures of length n:
            //   (n[0] < e[0]) OR (n[0] = e[0] AND n[1] < e[1]) OR ...
            //   OR (n[0] = e[0] AND ... AND n[n-1] < e[n-1])
            // For single measure: n[0] < e[0].
            let goal = build_lex_decrease_formula(&next_linears, entry_linears);

            let mut all_assumptions = proof_context.assumptions.clone();
            all_assumptions.extend(call_context.assumptions.iter().cloned());
            all_assumptions.extend(next_extras);
            let check = sigil_solver::prove_formula(&all_assumptions, &goal);
            if !matches!(check.outcome, SolverOutcome::Proved) {
                let mut error = TypeError::new(
                    format!(
                        "Function '{}' decreases measure could not be proven strictly decreasing at this recursive call",
                        function_name
                    ),
                    Some(app.location),
                )
                .with_code(sigil_diagnostics::codes::proof::MEASURE_NOT_DECREASING);
                error = error.with_detail("decreasesAt", format!("{:?}", decreases_location));
                return Err(error);
            }

            Ok(())
        }
        Expr::Identifier(_) | Expr::Literal(_) | Expr::MemberAccess(_) => Ok(()),
        Expr::Lambda(lambda) => walk_for_decreases(
            env,
            proof_context,
            &lambda.body,
            function_name,
            params,
            measure_components,
            entry_linears,
            decreases_location,
            child_depth,
        ),
        Expr::Binary(bin) => {
            walk_for_decreases(
                env,
                proof_context,
                &bin.left,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &bin.right,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Unary(un) => walk_for_decreases(
            env,
            proof_context,
            &un.operand,
            function_name,
            params,
            measure_components,
            entry_linears,
            decreases_location,
            child_depth,
        ),
        Expr::Match(m) => {
            walk_for_decreases(
                env,
                proof_context,
                &m.scrutinee,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            // Use the existing match-arm-refinement infrastructure so each
            // arm body is walked under the proof context that contains:
            //   - pattern bindings as symbolic values
            //   - the pattern's refinement formula as an assumption
            //   - the arm's guard (if supported) as an assumption
            // This is what makes a self-call inside `match n { value => self(value-1) }`
            // discharge with `value` known to equal `n`.
            let scrutinee_type = match synthesize(env, &m.scrutinee) {
                Ok(t) => t,
                Err(_) => {
                    for arm in &m.arms {
                        walk_for_decreases(
                            env,
                            proof_context,
                            &arm.body,
                            function_name,
                            params,
                            measure_components,
                            entry_linears,
                            decreases_location,
                            child_depth,
                        )?;
                    }
                    return Ok(());
                }
            };
            let base_match_context = scrutinee_proof_context(env, proof_context, &m.scrutinee);
            for arm in &m.arms {
                let mut bindings = HashMap::new();
                let arm_env = match check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)
                {
                    Ok(_) => env.extend(Some(bindings)),
                    Err(_) => env.extend(None),
                };
                let arm_context = match match_arm_refinement(
                    env,
                    &base_match_context,
                    &m.scrutinee,
                    &scrutinee_type,
                    arm,
                ) {
                    Ok(refinement) => refinement.body_context,
                    Err(_) => base_match_context.clone(),
                };
                walk_for_decreases(
                    &arm_env,
                    &arm_context,
                    &arm.body,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }
            Ok(())
        }
        Expr::Let(l) => {
            walk_for_decreases(
                env,
                proof_context,
                &l.value,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            let body_context = if let sigil_ast::Pattern::Identifier(id) = &l.pattern {
                if let Ok((value, extras)) =
                    lower_symbolic_value(env, proof_context, &l.value, None)
                {
                    proof_context
                        .clone()
                        .with_assumptions_replacing_state(extras)
                        .with_symbolic_bindings([(id.name.clone(), value)])
                } else {
                    // Full lowering can fail (e.g. `filter` under match-bound locals) while
                    // `synthesize` also fails: the let-body env is not the root `TypeEnvironment`
                    // during this walk, so we cannot re-typecheck the initializer.  If the
                    // binding is explicitly type-annotated, use that surface type for a path
                    // symbolic value (enough for `decreases` on `#x`).
                    let from_ascription = if let Expr::TypeAscription(asc) = &l.value {
                        ast_type_to_inference_type_resolved(env, None, &asc.ascribed_type).ok()
                    } else {
                        None
                    };
                    let value_t = from_ascription.or_else(|| synthesize(env, &l.value).ok());
                    if let Some(value_t) = value_t {
                        if let Ok(sv) =
                            symbolic_value_for_type_path(env, &value_t, &SymbolPath::root(&id.name))
                        {
                            proof_context
                                .clone()
                                .with_symbolic_bindings([(id.name.clone(), sv)])
                        } else {
                            proof_context.clone()
                        }
                    } else {
                        proof_context.clone()
                    }
                }
            } else {
                proof_context.clone()
            };
            walk_for_decreases(
                env,
                &body_context,
                &l.body,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Using(u) => {
            walk_for_decreases(
                env,
                proof_context,
                &u.value,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &u.body,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::If(i) => {
            walk_for_decreases(
                env,
                proof_context,
                &i.condition,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            // Thread condition assumption into each branch.
            let then_context = match lower_symbolic_formula(env, proof_context, &i.condition, None)
            {
                Ok((formula, assumptions)) => proof_context
                    .with_assumptions_replacing_state(assumptions.clone())
                    .with_assumption(formula),
                Err(_) => proof_context.clone(),
            };
            walk_for_decreases(
                env,
                &then_context,
                &i.then_branch,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            if let Some(else_branch) = &i.else_branch {
                let else_context =
                    match lower_symbolic_formula(env, proof_context, &i.condition, None) {
                        Ok((formula, assumptions)) => proof_context
                            .with_assumptions_replacing_state(assumptions)
                            .with_assumption(Formula::Not(Box::new(formula))),
                        Err(_) => proof_context.clone(),
                    };
                walk_for_decreases(
                    env,
                    &else_context,
                    else_branch,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }
            Ok(())
        }
        Expr::List(list) => {
            for element in &list.elements {
                walk_for_decreases(
                    env,
                    proof_context,
                    element,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }
            Ok(())
        }
        Expr::Record(record) => {
            for field in &record.fields {
                walk_for_decreases(
                    env,
                    proof_context,
                    &field.value,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }
            Ok(())
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                walk_for_decreases(
                    env,
                    proof_context,
                    &entry.key,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
                walk_for_decreases(
                    env,
                    proof_context,
                    &entry.value,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }
            Ok(())
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                walk_for_decreases(
                    env,
                    proof_context,
                    element,
                    function_name,
                    params,
                    measure_components,
                    entry_linears,
                    decreases_location,
                    child_depth,
                )?;
            }
            Ok(())
        }
        Expr::FieldAccess(field_access) => walk_for_decreases(
            env,
            proof_context,
            &field_access.object,
            function_name,
            params,
            measure_components,
            entry_linears,
            decreases_location,
            child_depth,
        ),
        Expr::Index(index) => {
            walk_for_decreases(
                env,
                proof_context,
                &index.object,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &index.index,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Pipeline(pipeline) => {
            walk_for_decreases(
                env,
                proof_context,
                &pipeline.left,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &pipeline.right,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Map(m) => {
            walk_for_decreases(
                env,
                proof_context,
                &m.list,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &m.func,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Filter(f) => {
            walk_for_decreases(
                env,
                proof_context,
                &f.list,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &f.predicate,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Fold(f) => {
            walk_for_decreases(
                env,
                proof_context,
                &f.list,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &f.init,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )?;
            walk_for_decreases(
                env,
                proof_context,
                &f.func,
                function_name,
                params,
                measure_components,
                entry_linears,
                decreases_location,
                child_depth,
            )
        }
        Expr::Concurrent(concurrent) => {
            for step in &concurrent.steps {
                match step {
                    sigil_ast::ConcurrentStep::Spawn(spawn) => {
                        walk_for_decreases(
                            env,
                            proof_context,
                            &spawn.expr,
                            function_name,
                            params,
                            measure_components,
                            entry_linears,
                            decreases_location,
                            child_depth,
                        )?;
                    }
                    sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                        walk_for_decreases(
                            env,
                            proof_context,
                            &spawn_each.list,
                            function_name,
                            params,
                            measure_components,
                            entry_linears,
                            decreases_location,
                            child_depth,
                        )?;
                        walk_for_decreases(
                            env,
                            proof_context,
                            &spawn_each.func,
                            function_name,
                            params,
                            measure_components,
                            entry_linears,
                            decreases_location,
                            child_depth,
                        )?;
                    }
                }
            }
            Ok(())
        }
        Expr::TypeAscription(type_ascription) => walk_for_decreases(
            env,
            proof_context,
            &type_ascription.expr,
            function_name,
            params,
            measure_components,
            entry_linears,
            decreases_location,
            child_depth,
        ),
    }
}

/// Build the strict lexicographic-decrease formula for a measure of any length:
///   (n[0] < e[0]) OR (n[0] = e[0] AND n[1] < e[1]) OR ...
fn build_lex_decrease_formula(next: &[LinearExpr], entry: &[LinearExpr]) -> Formula {
    debug_assert_eq!(next.len(), entry.len());
    let mut clauses: Vec<Formula> = Vec::with_capacity(next.len());
    for (i, (next_i, entry_i)) in next.iter().zip(entry.iter()).enumerate() {
        let mut clause_parts: Vec<Formula> = Vec::with_capacity(i + 1);
        for j in 0..i {
            clause_parts.push(linear_compare(
                next[j].clone(),
                ComparisonOp::Eq,
                entry[j].clone(),
            ));
        }
        clause_parts.push(linear_compare(
            next_i.clone(),
            ComparisonOp::Lt,
            entry_i.clone(),
        ));
        clauses.push(formula_and(clause_parts));
    }
    formula_or(clauses)
}

/// Build a comparison formula `lhs op rhs` from two LinearExprs by lowering to
/// the canonical `LinearForm op rhs_constant` shape the solver expects.
fn linear_compare(lhs: LinearExpr, op: ComparisonOp, rhs: LinearExpr) -> Formula {
    let diff = lhs.subtract(&rhs);
    Formula::Atom(Atom::IntCmp {
        form: diff.form,
        op,
        rhs: -diff.constant,
    })
}

/// Build call-site bindings for a function contract, only for parameters
/// that appear in `clause` (a `requires` or `ensures` expression). This avoids
/// having to lower arguments that the clause does not name (e.g. `⧺`-built
/// list accumulators) while still discharging `requires n≥0`-style facts.
fn contract_context_for_call(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    contract: &FunctionContract,
    args: &[Expr],
    clause: &Expr,
) -> Result<ProofContext, String> {
    let mut next = proof_context.clone();

    for (param_name, arg) in contract.params.iter().zip(args) {
        if !expr_mentions_param(clause, param_name) {
            continue;
        }
        let (symbolic_arg, assumptions) = lower_symbolic_value(env, proof_context, arg, None)?;
        next = next
            .with_assumptions_replacing_state(assumptions)
            .with_symbolic_bindings([(param_name.clone(), symbolic_arg)]);
    }

    Ok(next)
}

fn prove_contract_clause(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
) -> Result<ConstraintProofResult, String> {
    let (goal, assumptions) = lower_symbolic_formula(env, proof_context, expr, None)?;
    let all_assumptions = proof_context
        .assumptions
        .iter()
        .cloned()
        .chain(assumptions)
        .collect::<Vec<_>>();
    let check = prove_formula(&all_assumptions, &goal);
    match check.outcome {
        SolverOutcome::Proved => Ok(ConstraintProofResult::Proved),
        _ => Ok(ConstraintProofResult::Failed(check)),
    }
}

fn lookup_contract_for_call(env: &TypeEnvironment, func: &Expr) -> Option<FunctionContract> {
    match func {
        Expr::Identifier(identifier) => env.lookup_function_contract(&identifier.name),
        Expr::MemberAccess(member) => {
            env.lookup_qualified_function_contract(&member.namespace, &member.member)
        }
        _ => None,
    }
}

fn lookup_binding_meta_for_call(env: &TypeEnvironment, func: &Expr) -> Option<BindingMeta> {
    match func {
        Expr::Identifier(identifier) => env.lookup_meta(&identifier.name),
        Expr::MemberAccess(member) => {
            env.lookup_qualified_value_meta(&member.namespace, &member.member)
        }
        _ => None,
    }
}

fn call_target_name(func: &Expr) -> Option<String> {
    match func {
        Expr::Identifier(identifier) => Some(identifier.name.clone()),
        Expr::MemberAccess(member) => {
            Some(format!("{}.{}", member.namespace.join("::"), member.member))
        }
        _ => None,
    }
}

fn enforce_call_mode(
    env: &TypeEnvironment,
    func: &Expr,
    location: sigil_lexer::SourceLocation,
) -> Result<(), TypeError> {
    if env.current_function_mode() != FunctionMode::Total {
        return Ok(());
    }

    let Some(meta) = lookup_binding_meta_for_call(env, func) else {
        return Ok(());
    };

    if meta.function_mode != Some(FunctionMode::Ordinary) {
        return Ok(());
    }

    let target = call_target_name(func).unwrap_or_else(|| "<function>".to_string());
    Err(TypeError::new(
        format!("Total functions cannot call ordinary function '{}'", target),
        Some(location),
    )
    .with_detail("functionMode", "total")
    .with_detail("calleeMode", "ordinary"))
}

fn call_ensure_assumptions(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    contract: &FunctionContract,
    args: &[Expr],
    result_value: Option<SymbolicValue>,
) -> Result<Vec<Formula>, String> {
    let Some(ensures) = &contract.ensures else {
        return Ok(Vec::new());
    };

    let mut call_context = contract_context_for_call(env, proof_context, contract, args, ensures)?;
    if expr_mentions_param(ensures, "result") {
        let Some(result_value) = result_value else {
            return Ok(Vec::new());
        };
        call_context = call_context.with_symbolic_bindings([("result".to_string(), result_value)]);
    }
    let (formula, assumptions) = lower_symbolic_formula(env, &call_context, ensures, None)?;
    let mut combined = assumptions;
    combined.push(formula);
    Ok(combined)
}

fn enforce_call_requires(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    contract: &FunctionContract,
    args: &[Expr],
    location: sigil_lexer::SourceLocation,
) -> Result<(), TypeError> {
    let Some(requires) = &contract.requires else {
        return Ok(());
    };

    let call_context = contract_context_for_call(env, proof_context, contract, args, requires)
        .map_err(|reason| {
            TypeError::new(
                format!("Call requires unsupported proof inputs: {}", reason),
                Some(location),
            )
        })?;
    let proof = prove_contract_clause(env, &call_context, requires).map_err(|reason| {
        TypeError::new(
            format!("Call requires unsupported proof syntax: {}", reason),
            Some(location),
        )
    })?;

    if proof.proved() {
        return Ok(());
    }

    let mut error = TypeError::new(
        "Call does not satisfy requires clause".to_string(),
        Some(location),
    )
    .with_detail("proofKind", "requires");
    if let Some(check) = proof.failed_check() {
        error = error
            .with_detail("proof", check)
            .with_detail("proofSummary", proof_outcome_reason(&check.outcome));
    }
    Err(error)
}

fn call_result_proof_context(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    contract: Option<&FunctionContract>,
    args: &[Expr],
    result_type: &InferenceType,
) -> Result<ProofContext, TypeError> {
    let Some(contract) = contract else {
        return Ok(proof_context.clone());
    };

    let result_symbolic =
        symbolic_value_for_type_path(env, result_type, &SymbolPath::root("$call_result")).ok();

    let assumptions = call_ensure_assumptions(env, proof_context, contract, args, result_symbolic)
        .map_err(|reason| {
            TypeError::new(
                format!("Call ensures unsupported proof syntax: {}", reason),
                None,
            )
        })?;
    Ok(proof_context
        .clone()
        .with_assumptions_replacing_state(assumptions))
}

/// Type check a function declaration
fn check_function_decl(env: &TypeEnvironment, func_decl: &FunctionDecl) -> Result<(), TypeError> {
    let type_param_env = make_type_param_env(&func_decl.type_params);
    // Create environment with parameter bindings
    let mut func_env = env.extend(None);
    func_env.set_current_function_mode(func_decl.mode);

    for param in &func_decl.params {
        let param_type = param
            .type_annotation
            .as_ref()
            .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
            .transpose()?
            .unwrap_or(InferenceType::Any);
        func_env.bind(param.name.clone(), env.normalize_type(&param_type));
    }

    // Get expected return type
    let expected_return_type = func_decl
        .return_type
        .as_ref()
        .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
        .transpose()?
        .unwrap_or(InferenceType::Any);

    if let Some(requires) = &func_decl.requires {
        validate_contract_clause(&func_env, &func_decl.name, "requires", requires)?;
    }

    let measure_components = if let Some(decreases) = &func_decl.decreases {
        Some(validate_decreases_clause(
            &func_env,
            &func_decl.name,
            decreases,
        )?)
    } else {
        None
    };

    if let Some(ensures) = &func_decl.ensures {
        let mut ensures_bindings = HashMap::new();
        ensures_bindings.insert("result".to_string(), expected_return_type.clone());
        let ensures_env = func_env.extend(Some(ensures_bindings));
        validate_contract_clause(&ensures_env, &func_decl.name, "ensures", ensures)?;
    }

    // Type check body
    let param_nonneg = list_param_length_nonneg_assumptions(&func_env, &func_decl.params);
    let body_context = if let Some(requires) = &func_decl.requires {
        let (formula, assumptions) =
            lower_symbolic_formula(&func_env, &ProofContext::default(), requires, None).map_err(
                |reason| {
                    TypeError::new(
                        format!(
                            "Function '{}' requires clause could not be lowered: {}",
                            func_decl.name, reason
                        ),
                        Some(expr_location(requires)),
                    )
                },
            )?;
        ProofContext::default()
            .with_assumptions_replacing_state(assumptions)
            .with_assumption(formula)
            .with_assumptions(param_nonneg)
    } else {
        ProofContext::default().with_assumptions(param_nonneg)
    };

    check_with_context(
        &func_env,
        &body_context,
        &func_decl.body,
        &expected_return_type,
    )?;

    if let Some(ensures) = &func_decl.ensures {
        // Protocol state ensures clauses (e.g. `ensures handle.state = Closed`) are axiomatic —
        // they declare state transitions enforced by the runtime, not proven from the Sigil body.
        // Skip the body proof when the ensures clause is a state assertion (involves `.state`).
        let is_state_only_ensures = ensures_is_state_assertion(ensures);

        let proof = if is_state_only_ensures {
            ConstraintProofResult::proved_trivially()
        } else {
            prove_expr_satisfies_constraint(&func_env, &body_context, &func_decl.body, ensures)
                .map_err(|reason| {
                    TypeError::new(
                        format!(
                            "Function '{}' ensures clause could not be proven: {}",
                            func_decl.name, reason
                        ),
                        Some(expr_location(ensures)),
                    )
                })?
        };
        if !proof.proved() {
            let mut error = TypeError::new(
                format!(
                    "Function '{}' ensures clause could not be proven",
                    func_decl.name
                ),
                Some(expr_location(ensures)),
            );
            if let Some(check) = proof.failed_check() {
                error = error
                    .with_detail("proof", check)
                    .with_detail("proofKind", "ensures")
                    .with_detail("proofSummary", proof_outcome_reason(&check.outcome));
            }
            return Err(error);
        }
    }

    let typed_body = build_typed_expr(&func_env, &func_decl.body)?;
    declared_effects_cover_actual(
        env,
        &func_decl.effects,
        &typed_body.effects,
        func_decl.location,
        &format!("Function '{}'", func_decl.name),
    )?;

    let body_labels = labels_for_type(&func_env, &typed_body.typ);
    let return_labels = labels_for_type(&func_env, &expected_return_type);
    if !body_labels.is_subset(&return_labels) {
        let missing = body_labels
            .difference(&return_labels)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(TypeError::new(
            format!(
                "Function '{}' returns labelled data that is not declared on its return type: {}",
                func_decl.name, missing
            ),
            Some(func_decl.location),
        ));
    }

    // Termination proof: if a `decreases` clause is present, walk the body
    // and discharge the bound + strict-decrease obligations. The validator
    // already requires `decreases` on every self-recursive function; this
    // pass proves the measure works.
    if let Some(measure_components) = measure_components {
        prove_measure_well_founded(&func_env, func_decl, &measure_components, &body_context)?;
    }

    Ok(())
}

fn check_transform_decl(env: &TypeEnvironment, func_decl: &FunctionDecl) -> Result<(), TypeError> {
    let type_param_env = make_type_param_env(&func_decl.type_params);
    let mut func_env = env.extend(None);
    func_env.set_current_function_mode(func_decl.mode);

    for param in &func_decl.params {
        let param_type = param
            .type_annotation
            .as_ref()
            .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
            .transpose()?
            .unwrap_or(InferenceType::Any);
        let body_param_type = func_env.normalize_type(&param_type);
        func_env.bind(param.name.clone(), body_param_type);
    }

    let expected_return_type = func_decl
        .return_type
        .as_ref()
        .map(|ty| ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty))
        .transpose()?
        .unwrap_or(InferenceType::Any);

    if let Some(requires) = &func_decl.requires {
        validate_contract_clause(&func_env, &func_decl.name, "requires", requires)?;
    }

    let measure_components = if let Some(decreases) = &func_decl.decreases {
        Some(validate_decreases_clause(
            &func_env,
            &func_decl.name,
            decreases,
        )?)
    } else {
        None
    };

    if let Some(ensures) = &func_decl.ensures {
        let mut ensures_bindings = HashMap::new();
        ensures_bindings.insert("result".to_string(), expected_return_type.clone());
        let ensures_env = func_env.extend(Some(ensures_bindings));
        validate_contract_clause(&ensures_env, &func_decl.name, "ensures", ensures)?;
    }

    let param_nonneg = list_param_length_nonneg_assumptions(&func_env, &func_decl.params);
    let body_context = if let Some(requires) = &func_decl.requires {
        let (formula, assumptions) =
            lower_symbolic_formula(&func_env, &ProofContext::default(), requires, None).map_err(
                |reason| {
                    TypeError::new(
                        format!(
                            "Transform '{}' requires clause could not be lowered: {}",
                            func_decl.name, reason
                        ),
                        Some(expr_location(requires)),
                    )
                },
            )?;
        ProofContext::default()
            .with_assumptions_replacing_state(assumptions)
            .with_assumption(formula)
            .with_assumptions(param_nonneg.clone())
    } else {
        ProofContext::default().with_assumptions(param_nonneg.clone())
    };

    check_with_context(
        &func_env,
        &body_context,
        &func_decl.body,
        &expected_return_type,
    )?;

    let typed_body = build_typed_expr(&func_env, &func_decl.body)?;
    declared_effects_cover_actual(
        env,
        &func_decl.effects,
        &typed_body.effects,
        func_decl.location,
        &format!("Transform '{}'", func_decl.name),
    )?;

    if let Some(measure_components) = measure_components {
        prove_measure_well_founded(&func_env, func_decl, &measure_components, &body_context)?;
    }

    Ok(())
}

fn check_test_decl(
    env: &TypeEnvironment,
    test_decl: &sigil_ast::TestDecl,
) -> Result<(), TypeError> {
    let mut body_env = env.clone();

    for binding in &test_decl.world_bindings {
        let expected_type = match &binding.type_annotation {
            Some(ty) => ast_type_to_inference_type_resolved(&body_env, None, ty)?,
            None => synthesize(&body_env, &binding.value)?,
        };

        check_with_context(
            &body_env,
            &ProofContext::default(),
            &binding.value,
            &expected_type,
        )
        .map_err(|error| {
            TypeError::new(
                format!(
                    "Test world binding '{}' type mismatch: {}",
                    binding.name, error.message
                ),
                error.location.or(Some(binding.location)),
            )
        })?;

        let typed_binding = build_typed_const_decl(&body_env, binding)?;
        if !typed_binding.value.effects.is_empty() {
            return Err(TypeError::new(
                "test world bindings must be pure".to_string(),
                Some(binding.location),
            ));
        }

        let mut new_bindings = HashMap::new();
        new_bindings.insert(typed_binding.name.clone(), typed_binding.typ.clone());
        body_env = body_env.extend(Some(new_bindings));
    }

    check_with_context(
        &body_env,
        &ProofContext::default(),
        &test_decl.body,
        &bool_type(),
    )?;

    let typed_body = build_typed_expr(&body_env, &test_decl.body)?;
    declared_effects_cover_actual(
        &body_env,
        &test_decl.effects,
        &typed_body.effects,
        test_decl.location,
        &format!("Test '{}'", test_decl.description),
    )?;

    Ok(())
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

fn unit_type() -> InferenceType {
    InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Unit,
    })
}

fn never_type() -> InferenceType {
    InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Never,
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
    strip_param_labels: bool,
) -> Result<TypedFunctionDecl, TypeError> {
    let type_param_env = make_type_param_env(&func_decl.type_params);
    let mut lambda_env_bindings = HashMap::new();
    for param in &func_decl.params {
        if let Some(ref ty) = param.type_annotation {
            let param_type = ast_type_to_inference_type_resolved(env, Some(&type_param_env), ty)?;
            let body_param_type = if strip_param_labels {
                env.normalize_type(&param_type)
            } else {
                param_type
            };
            lambda_env_bindings.insert(param.name.clone(), body_param_type);
        }
    }
    let mut function_env = env.extend(Some(lambda_env_bindings));
    function_env.set_current_function_mode(func_decl.mode);
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
        requires: func_decl.requires.clone(),
        decreases: func_decl.decreases.clone(),
        ensures: func_decl.ensures.clone(),
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

fn build_typed_feature_flag_decl(
    env: &TypeEnvironment,
    feature_flag_decl: &FeatureFlagDecl,
) -> Result<TypedConstDecl, TypeError> {
    let expected_value_type =
        ast_type_to_inference_type_resolved(env, None, &feature_flag_decl.flag_type)?;
    check(env, &feature_flag_decl.default, &expected_value_type).map_err(|error| {
        TypeError::new(
            format!(
                "featureFlag '{}' default value mismatch: {}",
                feature_flag_decl.name, error.message
            ),
            error.location.or(Some(feature_flag_decl.location)),
        )
    })?;

    let value = build_typed_expr(env, &feature_flag_decl.default)?;
    if !value.effects.is_empty() {
        return Err(TypeError::new(
            format!(
                "featureFlag '{}' default value must be pure",
                feature_flag_decl.name
            ),
            Some(feature_flag_decl.location),
        ));
    }

    let descriptor_type = feature_flag_descriptor_type(&feature_flag_decl.flag_type);
    let synthetic_decl = sigil_ast::ConstDecl {
        name: feature_flag_decl.name.clone(),
        type_annotation: Some(descriptor_type),
        value: synthetic_feature_flag_expr(env, feature_flag_decl),
        location: feature_flag_decl.location,
    };
    build_typed_const_decl(env, &synthetic_decl)
}

fn synthetic_feature_flag_expr(env: &TypeEnvironment, feature_flag_decl: &FeatureFlagDecl) -> Expr {
    let location = feature_flag_decl.location;
    let id = feature_flag_runtime_id(env.module_id(), &feature_flag_decl.name);
    Expr::Record(RecordExpr {
        fields: vec![
            RecordField {
                name: "createdAt".to_string(),
                value: Expr::Literal(LiteralExpr {
                    value: LiteralValue::String(feature_flag_decl.created_at.clone()),
                    literal_type: LiteralType::String,
                    location: feature_flag_decl.created_at_location,
                }),
                location: feature_flag_decl.created_at_location,
            },
            RecordField {
                name: "default".to_string(),
                value: feature_flag_decl.default.clone(),
                location: expr_location(&feature_flag_decl.default),
            },
            RecordField {
                name: "id".to_string(),
                value: Expr::Literal(LiteralExpr {
                    value: LiteralValue::String(id),
                    literal_type: LiteralType::String,
                    location,
                }),
                location,
            },
        ],
        location,
    })
}

fn feature_flag_descriptor_type(flag_type: &Type) -> Type {
    Type::Qualified(QualifiedType {
        module_path: vec!["stdlib".to_string(), "featureFlags".to_string()],
        type_name: "Flag".to_string(),
        type_args: vec![flag_type.clone()],
        location: type_location(flag_type),
    })
}

fn validate_feature_flag_value_type(
    env: &TypeEnvironment,
    flag_name: &str,
    typ: &InferenceType,
    location: SourceLocation,
) -> Result<(), TypeError> {
    let normalized = env.normalize_type(typ);
    let is_allowed = matches!(
        normalized,
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool
        })
    ) || feature_flag_sum_type_name(env, &normalized).is_some();

    if is_allowed {
        Ok(())
    } else {
        Err(TypeError::new(
            format!(
                "featureFlag '{}' must use Bool or a named sum type, got {}",
                flag_name,
                format_type(&normalized)
            ),
            Some(location),
        ))
    }
}

fn feature_flag_sum_type_name(env: &TypeEnvironment, typ: &InferenceType) -> Option<String> {
    let InferenceType::Constructor(constructor) = typ else {
        return None;
    };

    if let Some((module_path, type_name)) =
        feature_flag_split_qualified_type_name(&constructor.name)
    {
        return env
            .lookup_qualified_type(&module_path, &type_name)
            .and_then(|info| matches!(info.definition, TypeDef::Sum(_)).then_some(type_name));
    }

    env.lookup_type(&constructor.name).and_then(|info| {
        matches!(info.definition, TypeDef::Sum(_)).then_some(constructor.name.clone())
    })
}

fn feature_flag_split_qualified_type_name(name: &str) -> Option<(Vec<String>, String)> {
    let (module_id, type_name) = name.rsplit_once('.')?;
    if module_id.is_empty() || type_name.is_empty() {
        return None;
    }
    Some((
        module_id
            .split("::")
            .map(|segment| segment.to_string())
            .collect(),
        type_name.to_string(),
    ))
}

fn feature_flag_runtime_id(module_id: Option<&str>, flag_name: &str) -> String {
    let Some(module_id) = module_id else {
        return flag_name.to_string();
    };
    let parts = module_id.split("::").collect::<Vec<_>>();
    let normalized_module = if parts.len() >= 3
        && (parts[0] == "package" || parts[0] == "packageConfig")
        && parts[2].starts_with('v')
    {
        let mut rebuilt = vec![parts[0], parts[1]];
        rebuilt.extend(parts[3..].iter().copied());
        rebuilt.join("::")
    } else {
        module_id.to_string()
    };
    format!("{normalized_module}.{flag_name}")
}

pub(crate) fn type_location(ty: &Type) -> SourceLocation {
    match ty {
        Type::Primitive(primitive) => primitive.location,
        Type::List(list) => list.location,
        Type::Map(map) => map.location,
        Type::Function(function) => function.location,
        Type::Constructor(constructor) => constructor.location,
        Type::Variable(variable) => variable.location,
        Type::Tuple(tuple) => tuple.location,
        Type::Qualified(qualified) => qualified.location,
    }
}

fn build_typed_test_decl(
    env: &TypeEnvironment,
    test_decl: &sigil_ast::TestDecl,
) -> Result<TypedTestDecl, TypeError> {
    let mut body_env = env.clone();
    let mut world_bindings = Vec::new();

    for binding in &test_decl.world_bindings {
        let typed_binding = build_typed_const_decl(&body_env, binding)?;
        if !typed_binding.value.effects.is_empty() {
            return Err(TypeError::new(
                "test world bindings must be pure".to_string(),
                Some(binding.location),
            ));
        }
        let mut new_bindings = HashMap::new();
        new_bindings.insert(typed_binding.name.clone(), typed_binding.typ.clone());
        body_env = body_env.extend(Some(new_bindings));
        world_bindings.push(typed_binding);
    }

    let body = build_typed_expr(&body_env, &test_decl.body)?;
    declared_effects_cover_actual(
        &body_env,
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
        world_bindings,
        body,
        location: test_decl.location,
    })
}

pub(crate) fn build_typed_expr(env: &TypeEnvironment, expr: &Expr) -> Result<TypedExpr, TypeError> {
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
        Expr::Using(using_expr) => {
            let value = build_typed_expr(env, &using_expr.value)?;
            let InferenceType::Owned(inner_type) = value.typ.clone() else {
                return Err(TypeError::new(
                    "using initializer must have type Owned[T]".to_string(),
                    Some(using_expr.location),
                ));
            };
            let scope_id = fresh_resource_scope_id();
            let mut bindings = HashMap::new();
            bindings.insert(
                using_expr.name.clone(),
                borrowed_type((*inner_type).clone(), scope_id),
            );
            let body_env = env.extend(Some(bindings));
            let body = build_typed_expr(&body_env, &using_expr.body)?;
            Ok(typed_expr(
                TypedExprKind::Using(crate::typed_ir::TypedUsingExpr {
                    body: Box::new(body.clone()),
                    name: using_expr.name.clone(),
                    scope_id,
                    value: Box::new(value.clone()),
                }),
                typ,
                merge_effects([value.effects, body.effects]),
                StrictnessClass::Deferred,
                using_expr.location,
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
        let subscription = env
            .lookup_extern_member_kind(&member_access.namespace, &member_access.member)
            .is_some_and(|kind| matches!(kind, sigil_ast::ExternMemberKind::Subscription));
        return Ok(typed_expr(
            TypedExprKind::ExternCall(TypedExternCallExpr {
                namespace: member_access.namespace.clone(),
                member: member_access.member.clone(),
                mock_key: format!(
                    "extern:{}.{}",
                    member_access.namespace.join("/"),
                    member_access.member
                ),
                subscription,
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

        Expr::Identifier(id) => {
            // Protocol state name labels (UpperCamelCase identifiers used as `handle.state = Label`)
            // are not regular Sigil bindings. They synthesize as Bool because they only appear
            // inside state equality comparisons which are Bool-typed.
            if id
                .name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
                && env.lookup(&id.name).is_none()
            {
                if env.is_protocol_state_label(&id.name) {
                    return Ok(InferenceType::Primitive(TPrimitive {
                        name: PrimitiveName::Bool,
                    }));
                }
            }
            env.lookup(&id.name).ok_or_else(|| {
                TypeError::new(format!("Unbound variable: {}", id.name), Some(id.location))
            })
        }

        Expr::Binary(bin) => synthesize_binary(env, bin),

        Expr::Unary(un) => synthesize_unary(env, un),

        Expr::Application(app) => synthesize_application(env, app),

        Expr::List(list) => synthesize_list(env, list),

        Expr::If(if_expr) => synthesize_if(env, if_expr),

        Expr::Let(let_expr) => synthesize_let(env, let_expr),

        Expr::Using(using_expr) => synthesize_using(env, using_expr),

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
    let float_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Float,
    });
    let string_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::String,
    });

    let is_float = |t: &InferenceType| matches!(t, InferenceType::Primitive(p) if p.name == PrimitiveName::Float);

    match bin.operator {
        // Arithmetic operators: Int => Int => Int, or Float => Float => Float
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

            // If either operand is Float, both must be Float
            if is_float(&left_type) || is_float(&right_type) {
                require_synthesized_operand_type(env, &left_type, &float_type, bin.location)?;
                require_synthesized_operand_type(env, &right_type, &float_type, bin.location)?;
                return Ok(float_type);
            }

            // Otherwise require both operands to be integers
            require_synthesized_operand_type(env, &left_type, &int_type, bin.location)?;
            require_synthesized_operand_type(env, &right_type, &int_type, bin.location)?;
            Ok(int_type)
        }

        // Comparison operators: Int => Int => Bool, or Float => Float => Bool
        BinaryOperator::Less
        | BinaryOperator::Greater
        | BinaryOperator::LessEq
        | BinaryOperator::GreaterEq => {
            if is_float(&left_type) || is_float(&right_type) {
                require_synthesized_operand_type(env, &left_type, &float_type, bin.location)?;
                require_synthesized_operand_type(env, &right_type, &float_type, bin.location)?;
            } else {
                require_synthesized_operand_type(env, &left_type, &int_type, bin.location)?;
                require_synthesized_operand_type(env, &right_type, &int_type, bin.location)?;
            }
            Ok(bool_type)
        }

        // Equality operators: T => T => Bool (polymorphic)
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            if matches!(
                left_type,
                InferenceType::Owned(_) | InferenceType::Borrowed(_)
            ) || matches!(
                right_type,
                InferenceType::Owned(_) | InferenceType::Borrowed(_)
            ) {
                return Err(TypeError::new(
                    "Resource values may not be compared for equality".to_string(),
                    Some(bin.location),
                ));
            }
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
            require_synthesized_operand_type(env, &left_type, &bool_type, bin.location)?;
            require_synthesized_operand_type(env, &right_type, &bool_type, bin.location)?;
            Ok(bool_type)
        }

        // String concatenation: String => String => String
        BinaryOperator::Append => {
            require_synthesized_operand_type(env, &left_type, &string_type, bin.location)?;
            require_synthesized_operand_type(env, &right_type, &string_type, bin.location)?;
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

fn require_synthesized_operand_type(
    env: &TypeEnvironment,
    actual_type: &InferenceType,
    expected_type: &InferenceType,
    location: SourceLocation,
) -> Result<(), TypeError> {
    if type_flows_without_new_proof(env, actual_type, expected_type)? {
        return Ok(());
    }
    let (normalized_actual, normalized_expected) = canonical_pair(env, actual_type, expected_type);
    Err(TypeError::new(
        format!(
            "Operator operand type mismatch: expected {}, got {}",
            format_type(&normalized_expected),
            format_type(&normalized_actual)
        ),
        Some(location),
    ))
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

    let float_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Float,
    });

    match un.operator {
        sigil_ast::UnaryOperator::Negate => {
            let operand_type = synthesize(env, &un.operand)?;
            if matches!(operand_type, InferenceType::Primitive(ref p) if p.name == PrimitiveName::Float)
            {
                require_synthesized_operand_type(env, &operand_type, &float_type, un.location)?;
                Ok(float_type)
            } else {
                require_synthesized_operand_type(env, &operand_type, &int_type, un.location)?;
                Ok(int_type)
            }
        }
        sigil_ast::UnaryOperator::Not => {
            let operand_type = synthesize(env, &un.operand)?;
            require_synthesized_operand_type(env, &operand_type, &bool_type, un.location)?;
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
    enforce_call_mode(env, &app.func, app.location)?;

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
                let expected_param = apply_subst(&subst, param_type);
                let (normalized_arg, normalized_param) =
                    canonical_pair(env, &arg_type, &expected_param);
                if let Ok(next_subst) = unify(&normalized_arg, &normalized_param) {
                    subst.extend(next_subst);
                    continue;
                }

                if try_refinement_compatibility(
                    env,
                    &ProofContext::default(),
                    arg,
                    &arg_type,
                    &expected_param,
                    app.location,
                )? {
                    continue;
                }

                return Err(TypeError::new(
                    format!(
                        "Function argument type mismatch: expected {}, got {}",
                        format_type(&normalized_param),
                        format_type(&normalized_arg)
                    ),
                    Some(app.location),
                ));
            }

            // Note: enforce_call_requires is intentionally NOT called here.
            // synthesize_application is called from build_typed_expr after check_with_context
            // has already enforced requires with the correct proof context. Calling it here
            // with ProofContext::default() would incorrectly fail protocol state requires.
            // The authoritative requires check happens in check_application (via check_with_context).

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

fn find_project_root_for_source(path: &str) -> Option<PathBuf> {
    let start_path = Path::new(path);
    let mut current = if start_path.is_file() {
        start_path.parent()?.to_path_buf()
    } else {
        start_path.to_path_buf()
    };

    loop {
        if current.join("sigil.json").exists() {
            return Some(current);
        }

        current = current.parent()?.to_path_buf();
    }
}

fn is_project_mode_source(env: &TypeEnvironment) -> bool {
    env.source_file()
        .and_then(find_project_root_for_source)
        .is_some()
}

fn is_canonical_stdlib_source(env: &TypeEnvironment) -> bool {
    env.source_file()
        .map(|path| path.replace('\\', "/").contains("/language/stdlib/"))
        .unwrap_or(false)
}

fn topology_call_member(expr: &Expr) -> Option<(&[String], &str)> {
    if let Expr::MemberAccess(member_access) = expr {
        return Some((&member_access.namespace, member_access.member.as_str()));
    }

    None
}

fn boundary_payload_arg_indices(module_id: &str, member: &str, arg_len: usize) -> Vec<usize> {
    match module_id {
        "stdlib::httpClient" => match member {
            "request" => (0..arg_len.min(1)).collect(),
            "get" | "getJson" | "delete" | "deleteJson" => (1..arg_len).collect(),
            "post" | "postJson" | "put" | "putJson" | "patch" | "patchJson" => {
                (0..arg_len).filter(|index| *index != 1).collect()
            }
            _ => Vec::new(),
        },
        "stdlib::tcpClient" => match member {
            "request" => (0..arg_len.min(1)).collect(),
            "send" => (1..arg_len).collect(),
            _ => Vec::new(),
        },
        "stdlib::file" => match file_handle_arg_index(member) {
            Some(handle_index) => (0..arg_len)
                .filter(|index| *index != handle_index)
                .collect(),
            None => Vec::new(),
        },
        "stdlib::log" if member == "write" => (0..arg_len.min(1)).collect(),
        "stdlib::process" if matches!(member, "runAt" | "startAt") => {
            (0..arg_len).filter(|index| *index != 1).collect()
        }
        _ => Vec::new(),
    }
}

fn file_handle_arg_index(member: &str) -> Option<usize> {
    match member {
        "appendTextAt" | "writeTextAt" => Some(2),
        "existsAt" | "listDirAt" | "makeDirAt" | "makeDirsAt" | "makeTempDirAt" | "readTextAt"
        | "removeAt" | "removeTreeAt" => Some(1),
        _ => None,
    }
}

#[derive(Debug, Clone)]
enum BoundaryPayload {
    Direct {
        labels: BTreeSet<String>,
    },
    Through {
        transform: String,
        source_labels: BTreeSet<String>,
    },
}

fn direct_topology_boundary_name(env: &TypeEnvironment, expr: &Expr) -> Option<String> {
    match expr {
        Expr::MemberAccess(member_access)
            if member_access.namespace == ["src".to_string(), "topology".to_string()] =>
        {
            Some(format!("src::topology.{}", member_access.member))
        }
        Expr::Identifier(identifier) => {
            let typ = env.lookup(&identifier.name)?;
            if !is_named_topology_boundary_type(&typ) {
                return None;
            }

            Some(
                env.module_id()
                    .map(|module_id| format!("{}.{}", module_id, identifier.name))
                    .unwrap_or_else(|| identifier.name.clone()),
            )
        }
        _ => None,
    }
}

fn resolve_transform_call_name(env: &TypeEnvironment, expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(identifier) => {
            let meta = env.lookup_meta(&identifier.name)?;
            if !meta.is_transform {
                return None;
            }
            Some(format!("{}.{}", env.module_id()?, identifier.name))
        }
        Expr::MemberAccess(member_access) => {
            let meta =
                env.lookup_qualified_value_meta(&member_access.namespace, &member_access.member)?;
            if !meta.is_transform {
                return None;
            }
            Some(format!(
                "{}.{}",
                member_access.namespace.join("::"),
                member_access.member
            ))
        }
        _ => None,
    }
}

fn boundary_payload_for_expr(
    env: &TypeEnvironment,
    expr: &Expr,
) -> Result<Option<BoundaryPayload>, TypeError> {
    let direct_labels = labels_for_type(env, &synthesize(env, expr)?);
    if !direct_labels.is_empty() {
        return Ok(Some(BoundaryPayload::Direct {
            labels: direct_labels,
        }));
    }

    let Expr::Application(app) = expr else {
        return Ok(None);
    };
    let Some(transform) = resolve_transform_call_name(env, &app.func) else {
        return Ok(None);
    };

    let mut source_labels = BTreeSet::new();
    for arg in &app.args {
        source_labels.extend(labels_for_type(env, &synthesize(env, arg)?));
    }
    let source_labels = label_closure(env, &source_labels);
    if source_labels.is_empty() {
        return Ok(None);
    }

    Ok(Some(BoundaryPayload::Through {
        transform,
        source_labels,
    }))
}

fn resolve_boundary_action(
    env: &TypeEnvironment,
    boundary_name: &str,
    labels: &BTreeSet<String>,
    location: SourceLocation,
) -> Result<BoundaryRuleKind, TypeError> {
    let mut matched_allow = false;
    let mut matched_through = BTreeSet::new();

    for rule in env.boundary_rules() {
        if rule.boundary != boundary_name || !rule.labels.is_subset(labels) {
            continue;
        }
        match &rule.action {
            BoundaryRuleKind::Block => {
                return Ok(BoundaryRuleKind::Block);
            }
            BoundaryRuleKind::Allow => matched_allow = true,
            BoundaryRuleKind::Through(transform) => {
                matched_through.insert(transform.clone());
            }
        }
    }

    if matched_allow && !matched_through.is_empty() {
        return Err(TypeError::new(
            format!(
                "Boundary '{}' has ambiguous rules for labels {}: both Allow() and Through(...) match",
                boundary_name,
                format_label_set(labels)
            ),
            Some(location),
        ));
    }

    if matched_through.len() > 1 {
        return Err(TypeError::new(
            format!(
                "Boundary '{}' has multiple Through(...) rules for labels {}",
                boundary_name,
                format_label_set(labels)
            ),
            Some(location),
        ));
    }

    if matched_allow {
        return Ok(BoundaryRuleKind::Allow);
    }

    if let Some(transform) = matched_through.iter().next() {
        return Ok(BoundaryRuleKind::Through(transform.clone()));
    }

    Err(TypeError::new(
        format!(
            "Boundary '{}' requires an explicit rule for labels {}",
            boundary_name,
            format_label_set(labels)
        ),
        Some(location),
    ))
}

fn enforce_boundary_payload(
    env: &TypeEnvironment,
    boundary_expr: &Expr,
    payload_expr: &Expr,
    location: SourceLocation,
) -> Result<(), TypeError> {
    let Some(payload) = boundary_payload_for_expr(env, payload_expr)? else {
        return Ok(());
    };

    let Some(boundary_name) = direct_topology_boundary_name(env, boundary_expr) else {
        return Err(TypeError::new(
            "Labelled boundary crossings must use a direct named topology handle".to_string(),
            Some(location),
        ));
    };

    let active_labels = match &payload {
        BoundaryPayload::Direct { labels } => labels.clone(),
        BoundaryPayload::Through { source_labels, .. } => source_labels.clone(),
    };
    let action = resolve_boundary_action(env, &boundary_name, &active_labels, location)?;

    match (payload, action) {
        (_, BoundaryRuleKind::Block) => Err(TypeError::new(
            format!(
                "Boundary '{}' blocks labels {}",
                boundary_name,
                format_label_set(&active_labels)
            ),
            Some(location),
        )),
        (BoundaryPayload::Direct { .. }, BoundaryRuleKind::Allow) => Ok(()),
        (BoundaryPayload::Direct { .. }, BoundaryRuleKind::Through(expected_transform)) => {
            Err(TypeError::new(
                format!(
                    "Boundary '{}' requires transform '{}' for labels {}",
                    boundary_name,
                    expected_transform,
                    format_label_set(&active_labels)
                ),
                Some(location),
            ))
        }
        (BoundaryPayload::Through { .. }, BoundaryRuleKind::Allow) => Ok(()),
        (
            BoundaryPayload::Through {
                transform,
                source_labels: _,
            },
            BoundaryRuleKind::Through(expected_transform),
        ) if transform == expected_transform => Ok(()),
        (
            BoundaryPayload::Through {
                transform,
                source_labels: _,
            },
            BoundaryRuleKind::Through(expected_transform),
        ) => Err(TypeError::new(
            format!(
                "Boundary '{}' requires transform '{}', but '{}' was used",
                boundary_name, expected_transform, transform
            ),
            Some(location),
        )),
    }
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

fn is_fs_root_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".FsRoot") || tcons.name == "FsRoot")
}

fn is_log_sink_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".LogSink") || tcons.name == "LogSink")
}

fn is_pty_handle_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".PtyHandle") || tcons.name == "PtyHandle")
}

fn is_process_handle_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".ProcessHandle") || tcons.name == "ProcessHandle")
}

fn is_sql_handle_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".SqlHandle") || tcons.name == "SqlHandle")
}

fn is_tcp_dependency_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".TcpServiceDependency") || tcons.name == "TcpServiceDependency")
}

fn is_websocket_handle_type(typ: &InferenceType) -> bool {
    matches!(typ, InferenceType::Constructor(tcons) if tcons.name.ends_with(".WebSocketHandle") || tcons.name == "WebSocketHandle")
}

fn is_named_topology_boundary_type(typ: &InferenceType) -> bool {
    is_http_dependency_type(typ)
        || is_fs_root_type(typ)
        || is_log_sink_type(typ)
        || is_pty_handle_type(typ)
        || is_process_handle_type(typ)
        || is_sql_handle_type(typ)
        || is_tcp_dependency_type(typ)
        || is_websocket_handle_type(typ)
}

fn member_ref_targets_named_topology_boundary(
    env: &TypeEnvironment,
    member_ref: &MemberRef,
) -> bool {
    if member_ref.module_path == ["src".to_string(), "topology".to_string()] {
        return true;
    }

    if member_ref.module_path.is_empty() {
        return env
            .lookup(&member_ref.member)
            .is_some_and(|typ| is_named_topology_boundary_type(&typ));
    }

    false
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
        let restricted = matches!(
            member,
            "environment"
                | "fsRoot"
                | "httpService"
                | "logSink"
                | "ptyHandle"
                | "processHandle"
                | "sqlHandle"
                | "tcpService"
                | "websocketHandle"
        );

        if restricted && is_project_mode_source(env) && !is_canonical_topology_source(env) {
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

        if restricted && is_project_mode_source(env) && !is_canonical_config_source(env) {
            return Err(TypeError::new(
                format!(
                    "{}: config helper constructors must live in config/*.lib.sigil",
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
    let fs_handle_arg_index = if module_id == "stdlib::file" {
        file_handle_arg_index(member)
    } else if module_id == "stdlib::fsWatch" && matches!(member, "watchAt") {
        Some(1)
    } else {
        None
    };
    let log_handle_arg_index = if module_id == "stdlib::log" && member == "write" {
        Some(1)
    } else {
        None
    };
    let process_handle_arg_index =
        if module_id == "stdlib::process" && matches!(member, "runAt" | "startAt") {
            Some(1)
        } else {
            None
        };
    let pty_handle_arg_index =
        if module_id == "stdlib::pty" && matches!(member, "spawnAt" | "spawnManagedAt") {
            Some(0)
        } else {
            None
        };
    let sql_handle_arg_index = if module_id == "stdlib::sql"
        && matches!(
            member,
            "all"
                | "begin"
                | "execDelete"
                | "execInsert"
                | "execUpdate"
                | "one"
                | "rawExec"
                | "rawQuery"
                | "rawQueryOne"
        ) {
        Some(0)
    } else {
        None
    };
    let websocket_handle_arg_index = if (module_id == "stdlib::websocket"
        && matches!(member, "connections" | "route"))
        || (module_id == "stdlib::httpServer"
            && matches!(member, "websocketConnections" | "websocketRoute"))
    {
        Some(0)
    } else {
        None
    };

    if http_handle_arg_index.is_none()
        && tcp_handle_arg_index.is_none()
        && fs_handle_arg_index.is_none()
        && log_handle_arg_index.is_none()
        && process_handle_arg_index.is_none()
        && pty_handle_arg_index.is_none()
        && sql_handle_arg_index.is_none()
        && websocket_handle_arg_index.is_none()
    {
        return Ok(());
    }

    let handle_index = http_handle_arg_index
        .or(tcp_handle_arg_index)
        .or(fs_handle_arg_index)
        .or(log_handle_arg_index)
        .or(process_handle_arg_index)
        .or(pty_handle_arg_index)
        .or(sql_handle_arg_index)
        .or(websocket_handle_arg_index)
        .unwrap();
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
                    "{}: stdlib::httpClient requires a named HttpServiceDependency as its first argument",
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
                    "{}: stdlib::tcpClient requires a named TcpServiceDependency as its first argument",
                    code
                ),
                Some(app.location),
            ));
        }
    }
    if fs_handle_arg_index.is_some() && !is_fs_root_type(&handle_type) {
        return Err(TypeError::new(
            if module_id == "stdlib::fsWatch" {
                "stdlib::fsWatch.watchAt requires a named FsRoot".to_string()
            } else {
                "stdlib::file.*At requires a named FsRoot".to_string()
            },
            Some(app.location),
        ));
    }
    if log_handle_arg_index.is_some() && !is_log_sink_type(&handle_type) {
        return Err(TypeError::new(
            "stdlib::log.write requires a named LogSink".to_string(),
            Some(app.location),
        ));
    }
    if process_handle_arg_index.is_some() && !is_process_handle_type(&handle_type) {
        return Err(TypeError::new(
            "stdlib::process.runAt/startAt requires a named ProcessHandle".to_string(),
            Some(app.location),
        ));
    }
    if pty_handle_arg_index.is_some() && !is_pty_handle_type(&handle_type) {
        return Err(TypeError::new(
            "stdlib::pty.spawnAt/spawnManagedAt requires a named PtyHandle".to_string(),
            Some(app.location),
        ));
    }
    if sql_handle_arg_index.is_some() && !is_sql_handle_type(&handle_type) {
        return Err(TypeError::new(
            "stdlib::sql execution requires a named SqlHandle".to_string(),
            Some(app.location),
        ));
    }
    if websocket_handle_arg_index.is_some() && !is_websocket_handle_type(&handle_type) {
        return Err(TypeError::new(
            if module_id == "stdlib::httpServer" {
                "stdlib::httpServer.websocketConnections/websocketRoute requires a named WebSocketHandle".to_string()
            } else {
                "stdlib::websocket.connections/route requires a named WebSocketHandle".to_string()
            },
            Some(app.location),
        ));
    }

    let payload_args: Vec<&Expr> = if module_id == "stdlib::httpClient" {
        match member {
            "request" => app.args.iter().take(1).collect(),
            "get" | "getJson" | "delete" | "deleteJson" => app.args.iter().skip(1).collect(),
            "post" | "postJson" | "put" | "putJson" | "patch" | "patchJson" => app
                .args
                .iter()
                .enumerate()
                .filter_map(|(index, expr)| (index != handle_index).then_some(expr))
                .collect(),
            _ => Vec::new(),
        }
    } else if module_id == "stdlib::tcpClient" {
        match member {
            "request" => app.args.iter().take(1).collect(),
            "send" => app.args.iter().skip(1).collect(),
            _ => Vec::new(),
        }
    } else if module_id == "stdlib::file" {
        app.args
            .iter()
            .enumerate()
            .filter_map(|(index, expr)| (index != handle_index).then_some(expr))
            .collect()
    } else if module_id == "stdlib::log" {
        app.args.iter().take(1).collect()
    } else if module_id == "stdlib::process" {
        app.args
            .iter()
            .enumerate()
            .filter_map(|(index, expr)| (index != handle_index).then_some(expr))
            .collect()
    } else {
        Vec::new()
    };

    for payload_arg in payload_args {
        enforce_boundary_payload(env, handle_arg, payload_arg, app.location)?;
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
    reject_owned_aggregate_members("list", list.location, [first_type.clone()])?;

    // Check remaining elements match
    for elem in &list.elements[1..] {
        check(env, elem, &first_type)?;
        reject_owned_aggregate_members("list", list.location, [synthesize(env, elem)?])?;
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
    let condition_type = synthesize(env, &if_expr.condition)?;
    require_synthesized_operand_type(env, &condition_type, &bool_type, if_expr.location)?;

    // Synthesize then branch
    let then_type = synthesize(env, &if_expr.then_branch)?;

    // If no else branch, then branch must be Unit
    if if_expr.else_branch.is_none() {
        let unit = unit_type();
        if !matches_expected_type(env, &then_type, &unit) {
            let normalized_then = env.normalize_type(&then_type);
            return Err(TypeError::new(
                format!(
                    "If expression without else must have Unit type, got {}",
                    format_type(&normalized_then)
                ),
                Some(if_expr.location),
            ));
        }
        return Ok(unit);
    }

    // Synthesize else branch
    let else_type = synthesize(env, if_expr.else_branch.as_ref().unwrap())?;

    let Some(joined_type) = type_join_with_never(env, &then_type, &else_type)? else {
        let (normalized_then, normalized_else) = canonical_pair(env, &then_type, &else_type);
        return Err(TypeError::new(
            format!(
                "If branches have different types: then is {}, else is {}",
                format_type(&normalized_then),
                format_type(&normalized_else)
            ),
            Some(if_expr.location),
        ));
    };

    Ok(joined_type)
}

fn synthesize_let(
    env: &TypeEnvironment,
    let_expr: &sigil_ast::LetExpr,
) -> Result<InferenceType, TypeError> {
    use sigil_ast::Pattern;

    // Synthesize binding value type
    let value_type = synthesize(env, &let_expr.value)?;
    if let Some(terminator) = terminating_expr_info(env, &let_expr.value)? {
        return Err(unreachable_code_error(
            &let_expr.body,
            terminator,
            "letBody",
        ));
    }
    if matches!(value_type, InferenceType::Owned(_)) {
        return Err(TypeError::new(
            "Owned values must be introduced with using, not l".to_string(),
            Some(let_expr.location),
        ));
    }

    // Let bindings currently support identifier and wildcard patterns only.
    // Match expressions handle the richer tuple/list/sum pattern surface.
    let mut bindings = HashMap::new();
    match &let_expr.pattern {
        Pattern::Identifier(id_pattern) => {
            bindings.insert(id_pattern.name.clone(), value_type.clone());
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

fn synthesize_using(
    env: &TypeEnvironment,
    using_expr: &sigil_ast::UsingExpr,
) -> Result<InferenceType, TypeError> {
    let value_type = synthesize(env, &using_expr.value)?;
    if let Some(terminator) = terminating_expr_info(env, &using_expr.value)? {
        return Err(unreachable_code_error(
            &using_expr.body,
            terminator,
            "usingBody",
        ));
    }
    let InferenceType::Owned(ref inner_type) = value_type else {
        return Err(TypeError::new(
            "using initializer must have type Owned[T]".to_string(),
            Some(using_expr.location),
        ));
    };

    let scope_id = fresh_resource_scope_id();
    let mut bindings = HashMap::new();
    bindings.insert(
        using_expr.name.clone(),
        borrowed_type(*inner_type.clone(), scope_id),
    );
    let body_env = env.extend(Some(bindings));
    let body_type = synthesize(&body_env, &using_expr.body)?;
    if type_contains_borrowed_scope(&body_type, scope_id) {
        return Err(TypeError::new(
            format!(
                "Borrowed resource '{}' escapes its using scope",
                using_expr.name
            ),
            Some(using_expr.location),
        ));
    }
    Ok(body_type)
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

    let base_match_context =
        scrutinee_proof_context(env, &ProofContext::default(), &match_expr.scrutinee);
    let mut fallthrough_context = base_match_context.clone();
    let mut joined_type: Option<InferenceType> = None;

    for arm in &match_expr.arms {
        let mut bindings = HashMap::new();
        check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)?;
        let arm_env = env.extend(Some(bindings));
        let arm_refinement = match_arm_refinement(
            env,
            &fallthrough_context,
            &match_expr.scrutinee,
            &scrutinee_type,
            arm,
        )?;
        let arm_context = arm_refinement.body_context.clone();

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
            check_with_context(&arm_env, &arm_context, guard, &bool_type)?;
        }

        let arm_body_type = synthesize(&arm_env, &arm.body)?;
        joined_type = match joined_type {
            None => Some(arm_body_type),
            Some(current) => {
                let Some(next) = type_join_with_never(env, &current, &arm_body_type)? else {
                    let (normalized_current, normalized_arm) =
                        canonical_pair(env, &current, &arm_body_type);
                    return Err(TypeError::new(
                        format!(
                            "Match arms have different types: expected {}, got {}",
                            format_type(&normalized_current),
                            format_type(&normalized_arm)
                        ),
                        Some(expr_location(&arm.body)),
                    ));
                };
                Some(next)
            }
        };

        if let Some(condition_formula) = arm_refinement.condition_formula {
            fallthrough_context =
                fallthrough_context.with_assumption(Formula::Not(Box::new(condition_formula)));
        }
    }

    analyze_match_coverage(env, &ProofContext::default(), &scrutinee_type, match_expr)?;

    Ok(joined_type.unwrap_or_else(never_type))
}

fn synthesize_tuple(
    env: &TypeEnvironment,
    tuple_expr: &sigil_ast::TupleExpr,
) -> Result<InferenceType, TypeError> {
    let types: Vec<_> = tuple_expr
        .elements
        .iter()
        .map(|elem| synthesize(env, elem))
        .collect::<Result<Vec<_>, _>>()?;
    reject_owned_aggregate_members("tuple", tuple_expr.location, types.clone())?;

    Ok(InferenceType::Tuple(crate::types::TTuple { types }))
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
    reject_owned_aggregate_members("record", record_expr.location, fields.values().cloned())?;

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
    reject_owned_aggregate_members(
        "map",
        map_expr.location,
        [key_type.clone(), value_type.clone()],
    )?;

    for entry in map_expr.entries.iter().skip(1) {
        check(env, &entry.key, &key_type)?;
        check(env, &entry.value, &value_type)?;
        reject_owned_aggregate_members(
            "map",
            map_expr.location,
            [synthesize(env, &entry.key)?, synthesize(env, &entry.value)?],
        )?;
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
    if field_access_starts_with_process_env(field_access)
        && is_project_mode_source(env)
        && !is_canonical_config_source(env)
    {
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

    // Protocol state access: `handle.state` on a protocol-typed value synthesizes as Bool
    // (it only makes sense inside a comparison like `handle.state = Open`).
    if field_access.field == "state" {
        if resolve_protocol_type_name(&obj_type, env).is_some() {
            return Ok(InferenceType::Primitive(TPrimitive {
                name: PrimitiveName::Bool,
            }));
        }
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
    let index_type = synthesize(env, &index_expr.index)?;
    require_synthesized_operand_type(env, &index_type, &int_type, index_expr.location)?;

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

    // Check namespace exists (should be registered from extern declarations or referenced modules)
    let namespace_type = env.lookup(&namespace_name);
    if namespace_type.is_none() {
        return Err(TypeError::new(
            format!("Unknown namespace '{}'", namespace_name),
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

    // If namespace is a record, check member exists
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
        Pattern::Record(record_pattern) => {
            let InferenceType::Record(record_type) = env.normalize_type(scrutinee_type) else {
                return Err(TypeError::new(
                    format!(
                        "Record pattern requires record type, got {}",
                        format_type(scrutinee_type)
                    ),
                    Some(record_pattern.location),
                ));
            };

            let expected_fields = sorted_record_field_types(&record_type);
            if record_pattern.fields.len() != expected_fields.len() {
                return Err(TypeError::new(
                    format!(
                        "Exact record pattern for {} must mention all {} fields, got {}",
                        format_type(&InferenceType::Record(record_type.clone())),
                        expected_fields.len(),
                        record_pattern.fields.len()
                    ),
                    Some(record_pattern.location),
                ));
            }

            for (field_name, field_type) in expected_fields {
                let Some(field_pattern) = record_pattern
                    .fields
                    .iter()
                    .find(|field| field.name == field_name)
                else {
                    return Err(TypeError::new(
                        format!("Exact record pattern is missing field '{}'", field_name),
                        Some(record_pattern.location),
                    ));
                };
                if let Some(pattern) = &field_pattern.pattern {
                    check_pattern(env, pattern, &field_type, bindings)?;
                } else {
                    bindings.insert(field_name, field_type);
                }
            }

            Ok(())
        }
    }
}

fn synthesize_type_ascription(
    env: &TypeEnvironment,
    type_asc: &sigil_ast::TypeAscriptionExpr,
) -> Result<InferenceType, TypeError> {
    let ascribed_type = ast_type_to_inference_type_resolved(env, None, &type_asc.ascribed_type)?;
    if let InferenceType::Owned(inner_type) = &ascribed_type {
        if is_canonical_stdlib_source(env) {
            check(env, &type_asc.expr, inner_type)?;
            return Ok(ascribed_type);
        }
    }

    let proof_sensitive_call = match &type_asc.expr {
        Expr::Application(app) => lookup_contract_for_call(env, &app.func).is_some(),
        _ => false,
    };

    if !proof_sensitive_call {
        check(env, &type_asc.expr, &ascribed_type)?;
        return Ok(ascribed_type);
    }

    let actual_type = synthesize(env, &type_asc.expr)?;
    let (normalized_actual, normalized_expected) =
        canonical_pair(env, &actual_type, &ascribed_type);
    if !types_equal(&normalized_actual, &normalized_expected) {
        return Err(TypeError::mismatch(
            format!(
                "Type mismatch: expected {}, got {}",
                format_type(&normalized_expected),
                format_type(&normalized_actual)
            ),
            Some(type_asc.location),
            normalized_expected,
            normalized_actual,
        ));
    }

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
    check_with_context(env, &ProofContext::default(), expr, expected_type)
}

fn check_with_context(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    // Special case: checking against 'any' type always succeeds (FFI trust mode)
    if matches!(expected_type, InferenceType::Any) {
        return Ok(());
    }

    if let Expr::Binary(binary) = expr {
        return check_binary(env, proof_context, binary, expected_type);
    }

    if let Expr::Application(app) = expr {
        return check_application(env, proof_context, app, expected_type);
    }

    if let Expr::If(if_expr) = expr {
        return check_if(env, proof_context, if_expr, expected_type);
    }

    if let Expr::Let(let_expr) = expr {
        return check_let(env, proof_context, let_expr, expected_type);
    }

    if let Expr::Using(using_expr) = expr {
        return check_using(env, proof_context, using_expr, expected_type);
    }

    if let Expr::Match(match_expr) = expr {
        return check_match(env, proof_context, match_expr, expected_type);
    }

    let normalized_expected = env.normalize_type(expected_type);
    match (expr, &normalized_expected) {
        (Expr::List(list_expr), InferenceType::List(list_type)) => {
            return check_list(env, list_expr, &list_type.element_type);
        }
        (Expr::Tuple(tuple_expr), InferenceType::Tuple(tuple_type)) => {
            return check_tuple(env, tuple_expr, &tuple_type.types);
        }
        (Expr::Record(record_expr), InferenceType::Record(record_type)) => {
            return check_record(env, record_expr, &record_type.fields);
        }
        (Expr::MapLiteral(map_expr), InferenceType::Map(map_type)) => {
            return check_map_literal(
                env,
                map_expr,
                &map_type.key_type,
                &map_type.value_type,
                expected_type,
            );
        }
        (Expr::MapLiteral(map_expr), _) if map_expr.entries.is_empty() => {
            return Err(TypeError::new(
                format!(
                    "Empty map literal requires a map type context, got {}",
                    format_type(expected_type)
                ),
                None,
            ));
        }
        _ => {}
    }

    let actual_type = synthesize(env, expr)?;

    if matches!(actual_type, InferenceType::Any) {
        return Ok(());
    }

    ensure_expr_matches_expected(
        env,
        proof_context,
        expr,
        &actual_type,
        expected_type,
        expr_location(expr),
    )
}

fn check_binary(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    bin: &sigil_ast::BinaryExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    let actual_type = synthesize_binary(env, bin)?;
    let left_type = synthesize(env, &bin.left)?;
    let right_type = synthesize(env, &bin.right)?;

    check_with_context(env, proof_context, &bin.left, &left_type)?;
    let right_context = expression_result_proof_context(env, proof_context, &bin.left, &left_type)?;
    check_with_context(env, &right_context, &bin.right, &right_type)?;
    let final_context =
        expression_result_proof_context(env, &right_context, &bin.right, &right_type)?;

    ensure_expr_matches_expected_with_contexts(
        env,
        proof_context,
        &final_context,
        &Expr::Binary(Box::new(bin.clone())),
        &actual_type,
        expected_type,
        bin.location,
    )
}

fn expression_result_proof_context(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    expr: &Expr,
    result_type: &InferenceType,
) -> Result<ProofContext, TypeError> {
    match expr {
        Expr::Application(app) => {
            let contract = lookup_contract_for_call(env, &app.func);
            call_result_proof_context(
                env,
                proof_context,
                contract.as_ref(),
                &app.args,
                result_type,
            )
        }
        Expr::TypeAscription(type_asc) => {
            expression_result_proof_context(env, proof_context, &type_asc.expr, result_type)
        }
        Expr::Binary(binary) => {
            let left_type = synthesize(env, &binary.left)?;
            let right_type = synthesize(env, &binary.right)?;
            let right_context =
                expression_result_proof_context(env, proof_context, &binary.left, &left_type)?;
            expression_result_proof_context(env, &right_context, &binary.right, &right_type)
        }
        _ => Ok(proof_context.clone()),
    }
}

fn check_application(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    app: &sigil_ast::ApplicationExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    let call_contract = lookup_contract_for_call(env, &app.func);
    enforce_call_mode(env, &app.func, app.location)?;
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

    let boundary_payload_indices = topology_call_member(&app.func)
        .map(|(namespace, member)| {
            boundary_payload_arg_indices(&namespace.join("::"), member, app.args.len())
        })
        .unwrap_or_default();

    let mut subst = HashMap::new();
    for (index, (arg, param_type)) in app.args.iter().zip(&tfunc.params).enumerate() {
        let arg_type = synthesize(env, arg)?;
        let expected_param = apply_subst(&subst, param_type);
        let (normalized_arg, normalized_param) = canonical_pair(env, &arg_type, &expected_param);
        if let Ok(next_subst) = unify(&normalized_arg, &normalized_param) {
            if !boundary_payload_indices.contains(&index) {
                ensure_label_subset(
                    env,
                    &arg_type,
                    &expected_param,
                    expr_location(arg),
                    "Function argument flow",
                )?;
            }
            subst.extend(next_subst);
            continue;
        }

        if try_refinement_compatibility(
            env,
            proof_context,
            arg,
            &arg_type,
            &expected_param,
            app.location,
        )? {
            if !boundary_payload_indices.contains(&index) {
                ensure_label_subset(
                    env,
                    &arg_type,
                    &expected_param,
                    expr_location(arg),
                    "Function argument flow",
                )?;
            }
            continue;
        }

        return Err(TypeError::new(
            format!(
                "Function argument type mismatch: expected {}, got {}",
                format_type(&normalized_param),
                format_type(&normalized_arg)
            ),
            Some(app.location),
        ));
    }

    if let Some(contract) = call_contract.as_ref() {
        enforce_call_requires(env, proof_context, contract, &app.args, app.location)?;
    }

    let actual_return = apply_subst(&subst, &tfunc.return_type);
    let result_context = call_result_proof_context(
        env,
        proof_context,
        call_contract.as_ref(),
        &app.args,
        &actual_return,
    )?;
    ensure_expr_matches_expected_with_contexts(
        env,
        proof_context,
        &result_context,
        &Expr::Application(Box::new(app.clone())),
        &actual_return,
        expected_type,
        app.location,
    )
}

fn check_if(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    if_expr: &sigil_ast::IfExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    check_with_context(env, proof_context, &if_expr.condition, &bool_type())?;

    let narrowed = lower_symbolic_formula(env, proof_context, &if_expr.condition, None).ok();
    let then_context = if let Some((condition_formula, condition_assumptions)) = narrowed.clone() {
        proof_context
            .with_assumptions_replacing_state(condition_assumptions)
            .with_assumption(condition_formula)
    } else {
        proof_context.clone()
    };
    let else_context = if let Some((condition_formula, condition_assumptions)) = narrowed {
        proof_context
            .with_assumptions_replacing_state(condition_assumptions)
            .with_assumption(Formula::Not(Box::new(condition_formula)))
    } else {
        proof_context.clone()
    };

    if if_expr.else_branch.is_none() {
        check_with_context(env, &then_context, &if_expr.then_branch, &unit_type())?;
        if !type_flows_without_new_proof(env, &unit_type(), expected_type)? {
            let (normalized_actual, normalized_expected) =
                canonical_pair(env, &unit_type(), expected_type);
            return Err(TypeError::mismatch(
                format!(
                    "Type mismatch: expected {}, got {}",
                    format_type(&normalized_expected),
                    format_type(&normalized_actual)
                ),
                Some(if_expr.location),
                normalized_expected,
                normalized_actual,
            ));
        }
        return Ok(());
    }

    check_with_context(env, &then_context, &if_expr.then_branch, expected_type)?;
    check_with_context(
        env,
        &else_context,
        if_expr.else_branch.as_ref().unwrap(),
        expected_type,
    )
}

fn check_let(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    let_expr: &sigil_ast::LetExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    use sigil_ast::Pattern;

    let value_type = synthesize(env, &let_expr.value)?;

    // Enforce requires clauses on the value expression with the correct proof context.
    // synthesize alone uses an empty proof context; we need the ambient context here.
    let value_app = match &let_expr.value {
        Expr::Application(app) => Some(app.as_ref()),
        Expr::TypeAscription(type_asc) => {
            if let Expr::Application(app) = &type_asc.expr {
                Some(app.as_ref())
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(app) = value_app {
        if let Some(contract) = lookup_contract_for_call(env, &app.func) {
            enforce_call_requires(env, proof_context, &contract, &app.args, app.location)?;
        }
    }

    if let Some(terminator) = terminating_expr_info(env, &let_expr.value)? {
        return Err(unreachable_code_error(
            &let_expr.body,
            terminator,
            "letBody",
        ));
    }
    if matches!(value_type, InferenceType::Owned(_)) {
        return Err(TypeError::new(
            "Owned values must be introduced with using, not l".to_string(),
            Some(let_expr.location),
        ));
    }
    let mut bindings = HashMap::new();
    match &let_expr.pattern {
        Pattern::Identifier(id_pattern) => {
            bindings.insert(id_pattern.name.clone(), value_type.clone());
        }
        Pattern::Wildcard(_) => {}
        _ => {
            return Err(TypeError::new(
                "Let expression pattern matching not yet fully implemented".to_string(),
                Some(let_expr.location),
            ));
        }
    }

    let body_env = env.extend(Some(bindings));
    let body_context = let_proof_context(
        env,
        proof_context,
        &let_expr.pattern,
        &let_expr.value,
        &value_type,
    );
    check_with_context(&body_env, &body_context, &let_expr.body, expected_type)
}

fn check_using(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    using_expr: &sigil_ast::UsingExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    let value_type = synthesize(env, &using_expr.value)?;

    let value_app = match &using_expr.value {
        Expr::Application(app) => Some(app.as_ref()),
        Expr::TypeAscription(type_asc) => {
            if let Expr::Application(app) = &type_asc.expr {
                Some(app.as_ref())
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(app) = value_app {
        if let Some(contract) = lookup_contract_for_call(env, &app.func) {
            enforce_call_requires(env, proof_context, &contract, &app.args, app.location)?;
        }
    }

    if let Some(terminator) = terminating_expr_info(env, &using_expr.value)? {
        return Err(unreachable_code_error(
            &using_expr.body,
            terminator,
            "usingBody",
        ));
    }
    let InferenceType::Owned(ref inner_type) = value_type else {
        return Err(TypeError::new(
            "using initializer must have type Owned[T]".to_string(),
            Some(using_expr.location),
        ));
    };

    let scope_id = fresh_resource_scope_id();
    let mut bindings = HashMap::new();
    bindings.insert(
        using_expr.name.clone(),
        borrowed_type(*inner_type.clone(), scope_id),
    );
    let value_context =
        expression_result_proof_context(env, proof_context, &using_expr.value, &value_type)?;
    let body_context = protocol_initial_state_proof_context(
        env,
        &value_context,
        &using_expr.name,
        inner_type.as_ref(),
    );
    let body_env = env.extend(Some(bindings));
    check_with_context(&body_env, &body_context, &using_expr.body, expected_type)?;

    let body_type = synthesize(&body_env, &using_expr.body)?;
    if type_contains_borrowed_scope(&body_type, scope_id) {
        return Err(TypeError::new(
            format!(
                "Borrowed resource '{}' escapes its using scope",
                using_expr.name
            ),
            Some(using_expr.location),
        ));
    }

    Ok(())
}

fn check_match(
    env: &TypeEnvironment,
    proof_context: &ProofContext,
    match_expr: &sigil_ast::MatchExpr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    let scrutinee_type = synthesize(env, &match_expr.scrutinee)?;

    if match_expr.arms.is_empty() {
        return Err(TypeError::new(
            "Match expression must have at least one arm".to_string(),
            Some(match_expr.location),
        ));
    }

    let base_match_context = scrutinee_proof_context(env, proof_context, &match_expr.scrutinee);
    let mut fallthrough_context = base_match_context.clone();

    for arm in &match_expr.arms {
        let mut bindings = HashMap::new();
        check_pattern(env, &arm.pattern, &scrutinee_type, &mut bindings)?;
        let arm_env = env.extend(Some(bindings));
        let arm_refinement = match_arm_refinement(
            env,
            &fallthrough_context,
            &match_expr.scrutinee,
            &scrutinee_type,
            arm,
        )?;
        let arm_context = arm_refinement.body_context.clone();

        if let Some(guard) = &arm.guard {
            check_with_context(&arm_env, &arm_context, guard, &bool_type())?;
        }

        check_with_context(&arm_env, &arm_context, &arm.body, expected_type)?;

        if let Some(condition_formula) = arm_refinement.condition_formula {
            fallthrough_context =
                fallthrough_context.with_assumption(Formula::Not(Box::new(condition_formula)));
        }
    }

    analyze_match_coverage(env, proof_context, &scrutinee_type, match_expr)
}

fn check_list(
    env: &TypeEnvironment,
    list_expr: &sigil_ast::ListExpr,
    expected_element_type: &InferenceType,
) -> Result<(), TypeError> {
    for element in &list_expr.elements {
        check(env, element, expected_element_type)?;
        reject_owned_aggregate_members("list", list_expr.location, [synthesize(env, element)?])?;
    }
    Ok(())
}

fn check_tuple(
    env: &TypeEnvironment,
    tuple_expr: &sigil_ast::TupleExpr,
    expected_types: &[InferenceType],
) -> Result<(), TypeError> {
    if tuple_expr.elements.len() != expected_types.len() {
        return Err(TypeError::new(
            format!(
                "Tuple has {} elements, expected {}",
                tuple_expr.elements.len(),
                expected_types.len()
            ),
            Some(tuple_expr.location),
        ));
    }

    for (element, expected_type) in tuple_expr.elements.iter().zip(expected_types) {
        check(env, element, expected_type)?;
    }
    reject_owned_aggregate_members(
        "tuple",
        tuple_expr.location,
        tuple_expr
            .elements
            .iter()
            .map(|element| synthesize(env, element))
            .collect::<Result<Vec<_>, _>>()?,
    )?;

    Ok(())
}

fn check_record(
    env: &TypeEnvironment,
    record_expr: &sigil_ast::RecordExpr,
    expected_fields: &HashMap<String, InferenceType>,
) -> Result<(), TypeError> {
    if record_expr.fields.len() != expected_fields.len() {
        return Err(TypeError::new(
            format!(
                "Record has {} fields, expected {}",
                record_expr.fields.len(),
                expected_fields.len()
            ),
            Some(record_expr.location),
        ));
    }

    for field in &record_expr.fields {
        let Some(expected_type) = expected_fields.get(&field.name) else {
            return Err(TypeError::new(
                format!("Unexpected record field '{}'", field.name),
                Some(field.location),
            ));
        };
        check(env, &field.value, expected_type)?;
    }
    reject_owned_aggregate_members(
        "record",
        record_expr.location,
        record_expr
            .fields
            .iter()
            .map(|field| synthesize(env, &field.value))
            .collect::<Result<Vec<_>, _>>()?,
    )?;

    Ok(())
}

fn check_map_literal(
    env: &TypeEnvironment,
    map_expr: &sigil_ast::MapLiteralExpr,
    expected_key_type: &InferenceType,
    expected_value_type: &InferenceType,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
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

    for entry in &map_expr.entries {
        check(env, &entry.key, expected_key_type)?;
        check(env, &entry.value, expected_value_type)?;
    }
    reject_owned_aggregate_members(
        "map",
        map_expr.location,
        map_expr
            .entries
            .iter()
            .flat_map(|entry| [synthesize(env, &entry.key), synthesize(env, &entry.value)])
            .collect::<Result<Vec<_>, _>>()?,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::{
        pattern_to_space, space_intersection, space_is_empty, total_space_for_type,
    };
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
            constraint: None,
            labels: BTreeSet::new(),
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
            constraint: None,
            labels: BTreeSet::new(),
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
            constraint: None,
            labels: BTreeSet::new(),
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
            ..TypeCheckOptions::default()
        }
    }

    fn option_test_env() -> TypeEnvironment {
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
            constraint: None,
            labels: BTreeSet::new(),
        };

        let mut env = TypeEnvironment::create_initial();
        env.register_type("Option".to_string(), option_info.clone());

        let some_type = create_constructor_type_with_result_name(
            &env,
            match &option_info.definition {
                TypeDef::Sum(sum) => &sum.variants[0],
                _ => unreachable!(),
            },
            &option_info.type_params,
            "Option",
        )
        .unwrap();
        let none_type = create_constructor_type_with_result_name(
            &env,
            match &option_info.definition {
                TypeDef::Sum(sum) => &sum.variants[1],
                _ => unreachable!(),
            },
            &option_info.type_params,
            "Option",
        )
        .unwrap();

        let mut quantified_vars = HashSet::new();
        collect_type_var_ids(&some_type, &mut quantified_vars);
        env.bind_scheme(
            "Some".to_string(),
            explicit_scheme(&some_type, &quantified_vars),
        );

        let mut quantified_vars = HashSet::new();
        collect_type_var_ids(&none_type, &mut quantified_vars);
        env.bind_scheme(
            "None".to_string(),
            explicit_scheme(&none_type, &quantified_vars),
        );

        env
    }

    #[test]
    fn test_simple_integer_function() {
        let source = "λadd(x:Int,y:Int)=>Int=x+y";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");

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
        let source = "λslug_len(meta:µArticleMeta)=>Int=#meta.slug";
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
                    constraint: None,
                    labels: BTreeSet::new(),
                },
            )]),
        );

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                effect_catalog: None,
                imported_namespaces: Some(HashMap::from([(
                    "src::types".to_string(),
                    InferenceType::Record(TRecord {
                        fields: HashMap::new(),
                        name: Some("src::types".to_string()),
                    }),
                )])),
                imported_type_registries: Some(imported_type_registries),
                imported_value_schemes: None,
                ..TypeCheckOptions::default()
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
    fn test_constrained_alias_direct_literal_promotion_typechecks() {
        let source = "t BirthYear=Int where value>1800 and value<10000\nλmain()=>BirthYear=1988";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_rejects_unprovable_literal_contradiction() {
        let source = "t BirthYear=Int where value>1800 and value<10000\nλmain()=>BirthYear=1700";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("Constraint for 'BirthYear' could not be proven here"));
    }

    #[test]
    fn test_constrained_alias_promotes_for_function_arguments() {
        let source =
            "t BirthYear=Int where value>1800 and value<10000\nλkeep(year:BirthYear)=>BirthYear=year\nλmain()=>BirthYear=keep(1988)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_widens_to_underlying_primitive() {
        let source =
            "t BirthYear=Int where value>1800 and value<10000\nλasInt(year:BirthYear)=>Int=year\nλmain()=>Int=asInt(1988)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_proves_simple_arithmetic_from_parameter_constraint() {
        let source = "t Positive=Int where value>0\nλincrement(value:Positive)=>Positive=value+1";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_narrows_through_match_on_boolean_fact() {
        let source =
            "t BirthYear=Int where value>1800\nλpromote(year:Int)=>BirthYear match year>1800{\n  true=>year|\n  false=>1900\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_narrows_later_match_arms_from_fallthrough() {
        let source =
            "t NonPositive=Int where value≤0\nλkeep(value:Int)=>NonPositive match value>0{\n  true=>0|\n  false=>value\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_narrows_through_match_guard() {
        let source =
            "t BirthYear=Int where value>1800\nλpromote(year:Int)=>BirthYear match year{\n  candidate when candidate>1800=>candidate|\n  _=>1900\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_narrows_through_direct_boolean_local_alias() {
        let source =
            "t BirthYear=Int where value>1800\nλpromote(year:Int)=>BirthYear=l ok=((year>1800):Bool);match ok{\n  true=>year|\n  false=>1900\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_alias_rejects_opaque_boolean_local_alias_for_narrowing() {
        let source =
            "t BirthYear=Int where value>1800\nλisBirthYear(year:Int)=>Bool=year>1800\nλpromote(year:Int)=>BirthYear=l ok=((isBirthYear(year)):Bool);match ok{\n  true=>year|\n  false=>1900\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("Constraint for 'BirthYear' could not be proven here"));
    }

    #[test]
    fn test_constrained_product_direct_record_promotion_typechecks() {
        let source =
            "t DateRange={end:Int,start:Int} where value.end≥value.start\nλmain()=>DateRange={end:2,start:1}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_product_rejects_unprovable_record_literal() {
        let source =
            "t DateRange={end:Int,start:Int} where value.end≥value.start\nλmain()=>DateRange={end:1,start:2}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("Constraint for 'DateRange' could not be proven here"));
    }

    #[test]
    fn test_function_requires_and_ensures_typecheck() {
        let source = "t BirthYear=Int where value>1800\nλnormalizeYear(raw:Int)=>Int\nrequires raw>0\nensures result>1800\nmatch raw>1800{\n  true=>raw|\n  false=>1900\n}\nλmain()=>BirthYear=normalizeYear(100)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_call_requires_clause_rejects_unproven_call_site() {
        let source =
            "λpositiveOnly(value:Int)=>Int\nrequires value>0\n=value\nλmain()=>Int=positiveOnly(0)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert!(error
            .message
            .contains("Call does not satisfy requires clause"));
        let details = error.details.unwrap();
        assert_eq!(
            details.get("proofKind").unwrap(),
            &serde_json::json!("requires")
        );
    }

    #[test]
    fn test_function_ensures_flows_across_calls_into_refinement() {
        let source = "t BirthYear=Int where value>1800\nλnormalizeYear(raw:Int)=>Int\nensures result>1800\nmatch raw>1800{\n  true=>raw|\n  false=>1900\n}\nλmain()=>BirthYear=normalizeYear(10)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_function_ensures_rejects_unprovable_body() {
        let source = "λbad(raw:Int)=>Int\nensures result>raw\n=raw";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert!(error
            .message
            .contains("Function 'bad' ensures clause could not be proven"));
        let details = error.details.unwrap();
        assert_eq!(
            details.get("proofKind").unwrap(),
            &serde_json::json!("ensures")
        );
    }

    #[test]
    fn test_exact_record_patterns_typecheck_and_cover_bool_space() {
        let source =
            "t Flagged={done:Bool,id:Int}\nλmain(item:Flagged)=>Int match item{\n  {done:true,id}=>id|\n  {done:false,id}=>id\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_exact_record_pattern_can_feed_refinement_from_field_fact() {
        let source =
            "t BirthYear=Int where value>1800\n\nt User={birthYear:Int,name:String}\n\nλpick(user:User)=>BirthYear match user{\n  {birthYear:year,name}=>match year>1800{\n    true=>year|\n    false=>1900\n  }\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_type_supports_builtin_length_measures() {
        let source = "t NonEmpty=String where #value>0\nλmain()=>NonEmpty=\"ok\"";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_constrained_type_rejects_still_unsupported_refinement_syntax() {
        let source = "t Positive=Int where value*2>0\nλmain()=>Unit=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("unsupported refinement syntax"));
    }

    #[test]
    fn test_constrained_sum_type_is_rejected() {
        let source = "t Bad=Some(Int)|None() where true\nλmain()=>Unit=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/types.lib.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("only supported on alias and product types"));
    }

    #[test]
    fn test_type_constructor_with_qualified_type_args_resolves_nested_qualified_types() {
        let source = "λmain()=>Result[µPersistedState,String]=Ok({nextId:1})";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let imported_type_registries = HashMap::from([(
            "src::types".to_string(),
            HashMap::from([(
                "PersistedState".to_string(),
                TypeInfo {
                    type_params: vec![],
                    definition: TypeDef::Product(sigil_ast::ProductType {
                        fields: vec![sigil_ast::Field {
                            name: "nextId".to_string(),
                            field_type: Type::Primitive(sigil_ast::PrimitiveType {
                                name: PrimitiveName::Int,
                                location: synthetic_loc(),
                            }),
                            location: synthetic_loc(),
                        }],
                        location: synthetic_loc(),
                    }),
                    constraint: None,
                    labels: BTreeSet::new(),
                },
            )]),
        )]);

        let mut options = core_prelude_type_options();
        options.imported_type_registries = Some(
            options
                .imported_type_registries
                .clone()
                .unwrap_or_default()
                .into_iter()
                .chain(imported_type_registries)
                .collect(),
        );

        let result = type_check(&program, source, options);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_process_env_access_is_rejected_outside_config_modules() {
        let source = "e process\nλmain()=>String=(process.env.sigilSiteBasePath:String)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "src/main.sigil").unwrap();
        let temp_root = std::env::temp_dir().join(format!(
            "sigil-typechecker-process-env-project-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(temp_root.join("src")).unwrap();
        std::fs::write(
            temp_root.join("sigil.json"),
            "{\n  \"name\": \"processEnvFixture\",\n  \"version\": \"2026-04-13T00-00-00Z\"\n}\n",
        )
        .unwrap();

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: None,
                imported_value_schemes: None,
                source_file: Some(
                    temp_root
                        .join("src/main.sigil")
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..TypeCheckOptions::default()
            },
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("process.env access is only allowed in config/*.lib.sigil"));
        let _ = std::fs::remove_dir_all(temp_root);
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
                ..TypeCheckOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_env_access_is_allowed_in_standalone_single_files() {
        let source = "e process\nλmain()=>String=(process.env.sigilSiteBasePath:String)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "singleFile.sigil").unwrap();

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                effect_catalog: None,
                imported_namespaces: None,
                imported_type_registries: None,
                imported_value_schemes: None,
                source_file: Some("/tmp/singleFile.sigil".to_string()),
                ..TypeCheckOptions::default()
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
    fn test_function_alias_preserves_qualified_error_type_in_match() {
        let source = "t Decoder[T]=λ(String)=>Result[T,§decode.DecodeError]\nλmain(decoder:Decoder[String],value:String)=>Result[String,§decode.DecodeError] match decoder(value){\n  Ok(text)=>Ok(text)|\n  Err(error)=>Err(error)\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut options = core_prelude_type_options();
        options.imported_type_registries = Some(
            options
                .imported_type_registries
                .clone()
                .unwrap_or_default()
                .into_iter()
                .chain(HashMap::from([(
                    "stdlib::decode".to_string(),
                    HashMap::from([(
                        "DecodeError".to_string(),
                        TypeInfo {
                            type_params: vec![],
                            definition: TypeDef::Product(sigil_ast::ProductType {
                                fields: vec![
                                    sigil_ast::Field {
                                        name: "message".to_string(),
                                        field_type: Type::Primitive(sigil_ast::PrimitiveType {
                                            name: PrimitiveName::String,
                                            location: synthetic_loc(),
                                        }),
                                        location: synthetic_loc(),
                                    },
                                    sigil_ast::Field {
                                        name: "path".to_string(),
                                        field_type: Type::List(Box::new(sigil_ast::ListType {
                                            element_type: Type::Primitive(
                                                sigil_ast::PrimitiveType {
                                                    name: PrimitiveName::String,
                                                    location: synthetic_loc(),
                                                },
                                            ),
                                            location: synthetic_loc(),
                                        })),
                                        location: synthetic_loc(),
                                    },
                                ],
                                location: synthetic_loc(),
                            }),
                            constraint: None,
                            labels: BTreeSet::new(),
                        },
                    )]),
                )]))
                .collect(),
        );

        let result = type_check(&program, source, options);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_qualified_imported_constructor_expression_typechecks() {
        let source = "λmk()=>µTopologicalSortResult=µOrdering([1,2,3])";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut imported_type_registries = HashMap::new();
        imported_type_registries.insert(
            "src::types".to_string(),
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
                    constraint: None,
                    labels: BTreeSet::new(),
                },
            )]),
        );

        let result = type_check(
            &program,
            source,
            TypeCheckOptions {
                effect_catalog: None,
                imported_namespaces: Some(HashMap::from([(
                    "src::types".to_string(),
                    InferenceType::Record(TRecord {
                        fields: HashMap::new(),
                        name: Some("src::types".to_string()),
                    }),
                )])),
                imported_type_registries: Some(imported_type_registries),
                imported_value_schemes: None,
                ..TypeCheckOptions::default()
            },
        );
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_qualified_imported_constructor_pattern_typechecks() {
        let source = "λproject(result:µTopologicalSortResult)=>[Int] match result{µOrdering(order)=>order|µCycleDetected()=>[]}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut imported_type_registries = HashMap::new();
        imported_type_registries.insert(
            "src::types".to_string(),
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
                    constraint: None,
                    labels: BTreeSet::new(),
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
                ..TypeCheckOptions::default()
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_imported_namespace_function_returning_option_of_record_binds_record_payload() {
        let source = "λmain()=>Bool match •formula.parseChecksums(\"x\",\"y\"){Some(checksums)=>checksums.darwinArm64=\"a\"|None()=>false}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut options = core_prelude_type_options();
        options.imported_namespaces = Some(HashMap::from([(
            "src::formula".to_string(),
            InferenceType::Record(TRecord {
                fields: HashMap::from([(
                    "parseChecksums".to_string(),
                    InferenceType::Function(Box::new(TFunction {
                        params: vec![
                            InferenceType::Primitive(TPrimitive {
                                name: PrimitiveName::String,
                            }),
                            InferenceType::Primitive(TPrimitive {
                                name: PrimitiveName::String,
                            }),
                        ],
                        return_type: InferenceType::Constructor(TConstructor {
                            name: "Option".to_string(),
                            type_args: vec![InferenceType::Record(TRecord {
                                fields: HashMap::from([
                                    (
                                        "darwinArm64".to_string(),
                                        InferenceType::Primitive(TPrimitive {
                                            name: PrimitiveName::String,
                                        }),
                                    ),
                                    (
                                        "darwinX64".to_string(),
                                        InferenceType::Primitive(TPrimitive {
                                            name: PrimitiveName::String,
                                        }),
                                    ),
                                ]),
                                name: Some("src::types.ReleaseChecksums".to_string()),
                            })],
                        }),
                        effects: None,
                    })),
                )]),
                name: Some("src::formula".to_string()),
            }),
        )]));

        let result = type_check(&program, source, options);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn test_imported_namespace_function_returning_result_of_record_binds_record_payload() {
        let source = "λmain()=>Bool match •todoJson.decodeState(\"{}\"){Ok(state)=>state.nextId=1|Err(_)=>false}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut options = core_prelude_type_options();
        options.imported_namespaces = Some(HashMap::from([(
            "src::todoJson".to_string(),
            InferenceType::Record(TRecord {
                fields: HashMap::from([(
                    "decodeState".to_string(),
                    InferenceType::Function(Box::new(TFunction {
                        params: vec![InferenceType::Primitive(TPrimitive {
                            name: PrimitiveName::String,
                        })],
                        return_type: InferenceType::Constructor(TConstructor {
                            name: "Result".to_string(),
                            type_args: vec![
                                InferenceType::Record(TRecord {
                                    fields: HashMap::from([(
                                        "nextId".to_string(),
                                        InferenceType::Primitive(TPrimitive {
                                            name: PrimitiveName::Int,
                                        }),
                                    )]),
                                    name: Some("src::types.PersistedState".to_string()),
                                }),
                                InferenceType::Primitive(TPrimitive {
                                    name: PrimitiveName::String,
                                }),
                            ],
                        }),
                        effects: None,
                    })),
                )]),
                name: Some("src::todoJson".to_string()),
            }),
        )]));

        let result = type_check(&program, source, options);
        assert!(result.is_ok(), "{result:?}");
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
        let source = "λmain()=>Option[Int]=Some(42)";
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
            InferenceType::Owned(inner) => assert_no_var_cycles(inner, seen),
            InferenceType::Borrowed(borrowed) => {
                assert_no_var_cycles(&borrowed.resource_type, seen);
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

    #[test]
    fn test_extern_subscription_members_elaborate_to_owned_sources() {
        let source = "e nodePty:{onData: subscribes λ(Int)=>String}\nλmain(session:Int)=>!Stream String=using source=nodePty.onData(session){\"ready\"}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_using_owned_resource_typechecks_when_body_consumes_borrowed_value() {
        let source =
            "e resources:{open:λ()=>Owned[Int]}\nλmain()=>Int=using value=resources.open(){value+1}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_using_rejects_borrowed_resource_escape() {
        let source =
            "e resources:{open:λ()=>Owned[Int]}\nλmain()=>Int=using value=resources.open(){value}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert!(error.to_string().contains("escapes its using scope"));
    }

    #[test]
    fn test_let_binding_rejects_owned_values() {
        let source = "e resources:{open:λ()=>Owned[Int]}\nλmain()=>Int=l value=(resources.open():Owned[Int]);0";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert!(error
            .to_string()
            .contains("Owned values must be introduced with using, not l"));
    }

    #[test]
    fn test_let_body_after_process_exit_is_rejected_as_unreachable() {
        let source = "e process:{exit:λ(Int)=>!Process Never}\nλmain()=>!Process Unit={l _=(process.exit(1):Never);()}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::UNREACHABLE_CODE);
        let details = error.details.unwrap();
        assert_eq!(details.get("unreachableKind").unwrap(), "letBody");
        assert_eq!(details.get("terminatorKind").unwrap(), "processExit");
    }

    #[test]
    fn test_using_body_after_process_exit_is_rejected_as_unreachable() {
        let source = "e process:{exit:λ(Int)=>!Process Never}\nλmain()=>!Process Unit=using source=process.exit(1){()}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::UNREACHABLE_CODE);
        let details = error.details.unwrap();
        assert_eq!(details.get("unreachableKind").unwrap(), "usingBody");
        assert_eq!(details.get("terminatorKind").unwrap(), "processExit");
    }

    #[test]
    fn test_exhaustively_terminating_match_makes_following_let_body_unreachable() {
        let source = "e process:{exit:λ(Int)=>!Process Never}\nλmain(flag:Bool)=>!Process Unit={l _=(match flag{true=>process.exit(1)|false=>process.exit(2)});()}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::UNREACHABLE_CODE);
        let details = error.details.unwrap();
        assert_eq!(details.get("terminatorKind").unwrap(), "match");
    }

    #[test]
    fn test_exhaustively_terminating_if_makes_following_let_body_unreachable() {
        let never_fn = InferenceType::Function(Box::new(TFunction {
            params: vec![],
            return_type: never_type(),
            effects: None,
        }));
        let env =
            TypeEnvironment::new().extend(Some(HashMap::from([("stop".to_string(), never_fn)])));
        let stop_call = Expr::Application(Box::new(sigil_ast::ApplicationExpr {
            func: Expr::Identifier(sigil_ast::IdentifierExpr {
                name: "stop".to_string(),
                location: synthetic_loc(),
            }),
            args: vec![],
            location: synthetic_loc(),
        }));
        let expr = Expr::Let(Box::new(sigil_ast::LetExpr {
            pattern: sigil_ast::Pattern::Wildcard(sigil_ast::WildcardPattern {
                location: synthetic_loc(),
            }),
            value: Expr::If(Box::new(sigil_ast::IfExpr {
                condition: Expr::Literal(LiteralExpr {
                    value: LiteralValue::Bool(true),
                    literal_type: LiteralType::Bool,
                    location: synthetic_loc(),
                }),
                then_branch: stop_call.clone(),
                else_branch: Some(stop_call),
                location: synthetic_loc(),
            })),
            body: Expr::Literal(LiteralExpr {
                value: LiteralValue::Unit,
                literal_type: LiteralType::Unit,
                location: synthetic_loc(),
            }),
            location: synthetic_loc(),
        }));

        let error = synthesize(&env, &expr).unwrap_err();
        assert_eq!(error.code, codes::typecheck::UNREACHABLE_CODE);
        let details = error.details.unwrap();
        assert_eq!(details.get("terminatorKind").unwrap(), "if");
    }

    #[test]
    fn test_if_without_else_after_terminating_branch_is_not_considered_unreachable() {
        let never_fn = InferenceType::Function(Box::new(TFunction {
            params: vec![],
            return_type: never_type(),
            effects: None,
        }));
        let env =
            TypeEnvironment::new().extend(Some(HashMap::from([("stop".to_string(), never_fn)])));
        let stop_call = Expr::Application(Box::new(sigil_ast::ApplicationExpr {
            func: Expr::Identifier(sigil_ast::IdentifierExpr {
                name: "stop".to_string(),
                location: synthetic_loc(),
            }),
            args: vec![],
            location: synthetic_loc(),
        }));
        let expr = Expr::Let(Box::new(sigil_ast::LetExpr {
            pattern: sigil_ast::Pattern::Wildcard(sigil_ast::WildcardPattern {
                location: synthetic_loc(),
            }),
            value: Expr::If(Box::new(sigil_ast::IfExpr {
                condition: Expr::Literal(LiteralExpr {
                    value: LiteralValue::Bool(true),
                    literal_type: LiteralType::Bool,
                    location: synthetic_loc(),
                }),
                then_branch: stop_call,
                else_branch: None,
                location: synthetic_loc(),
            })),
            body: Expr::Literal(LiteralExpr {
                value: LiteralValue::Unit,
                literal_type: LiteralType::Unit,
                location: synthetic_loc(),
            }),
            location: synthetic_loc(),
        }));

        let result = synthesize(&env, &expr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_match_branch_with_process_exit_and_value_typechecks() {
        let source = "e process:{exit:λ(Int)=>!Process Never}\nλmain(flag:Bool)=>!Process Int match flag{true=>process.exit(1)|false=>1}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_exit_typechecks_against_unit_return() {
        let source =
            "e process:{exit:λ(Int)=>!Process Never}\nλmain()=>!Process Unit=process.exit(1)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_match_bool_non_exhaustive_reports_missing_false() {
        let source = "λmain(x:Bool)=>String match x{\n  true=>\"yes\"\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::MATCH_NON_EXHAUSTIVE);
        let details = error.details.unwrap();
        assert_eq!(
            details.get("suggestedMissingArms").unwrap(),
            &serde_json::json!(["false"])
        );
    }

    #[test]
    fn test_match_sum_non_exhaustive_reports_missing_variant() {
        let source = "λmain(opt:Option[Int])=>Int match opt{\n  Some(value)=>value\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, core_prelude_type_options()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::MATCH_NON_EXHAUSTIVE);
        let details = error.details.unwrap();
        assert_eq!(
            details.get("suggestedMissingArms").unwrap(),
            &serde_json::json!(["None()"])
        );
    }

    #[test]
    fn test_option_constructor_pattern_space_intersects_total_space() {
        let env = option_test_env();
        let scrutinee_type = InferenceType::Constructor(TConstructor {
            name: "Option".to_string(),
            type_args: vec![InferenceType::Primitive(TPrimitive {
                name: PrimitiveName::Int,
            })],
        });

        let total = total_space_for_type(&env, &scrutinee_type).unwrap();
        let pattern = sigil_ast::Pattern::Constructor(sigil_ast::ConstructorPattern {
            module_path: vec![],
            name: "Some".to_string(),
            patterns: vec![sigil_ast::Pattern::Identifier(
                sigil_ast::IdentifierPattern {
                    name: "value".to_string(),
                    location: synthetic_loc(),
                },
            )],
            location: synthetic_loc(),
        });
        let mut bindings = HashMap::new();
        let arm_space = pattern_to_space(
            &env,
            &scrutinee_type,
            &pattern,
            &mut bindings,
            &vec![],
            &mut std::collections::BTreeSet::new(),
        )
        .unwrap();
        let useful = space_intersection(&total, &arm_space);

        assert!(
            !space_is_empty(&total) && !space_is_empty(&arm_space) && !space_is_empty(&useful),
            "total={total:?} arm={arm_space:?} useful={useful:?}"
        );
    }

    #[test]
    fn test_match_guard_redundancy_is_rejected() {
        let source = "λmain(x:Int)=>String match x{\n  n when n>0=>\"p\"|\n  n when n>1=>\"pp\"|\n  _=>\"rest\"\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::MATCH_REDUNDANT_PATTERN);
    }

    #[test]
    fn test_match_guard_redundancy_tracks_direct_boolean_aliases() {
        let source = "λmain(x:Int)=>String=l ok=((x>0):Bool);match x{\n  n when ok=>\"p\"|\n  n when n>1=>\"pp\"|\n  _=>\"rest\"\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::MATCH_REDUNDANT_PATTERN);
    }

    #[test]
    fn test_match_unreachable_arm_is_rejected() {
        let source = "λmain(x:Bool)=>String match x{\n  _=>\"all\"|\n  true=>\"yes\"\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::MATCH_UNREACHABLE_ARM);
        let details = error.details.unwrap();
        assert_eq!(details.get("coveredByArm").unwrap(), &serde_json::json!(0));
    }

    #[test]
    fn test_match_list_nil_cons_is_exhaustive() {
        let source = "λmain(xs:[Int])=>Int match xs{\n  []=>0|\n  [head,.tail]=>head\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_match_list_of_sum_values_is_exhaustive() {
        let source = "t Outcome=Success(Int)|Failure(String)|Aborted()\n\nλmain(outcomes:[Outcome])=>Int match outcomes{\n  []=>0|\n  [head,.tail]=>match head{\n    Success(value)=>value|\n    Failure(_)=>0|\n    Aborted()=>0\n  }\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_match_recursive_sum_is_exhaustive() {
        let source = "t JsonValue=JsonArray([JsonValue])|JsonBool(Bool)|JsonNull()|JsonString(String)\n\nλmain(value:JsonValue)=>Int match value{\n  JsonArray(_)=>0|\n  JsonBool(_)=>1|\n  JsonNull()=>2|\n  JsonString(_)=>3\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_match_tuple_reports_missing_combination() {
        let source =
            "λmain()=>String match (true,false){\n  (true,true)=>\"a\"|\n  (true,false)=>\"b\"\n}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let error = type_check(&program, source, TypeCheckOptions::default()).unwrap_err();
        assert_eq!(error.code, codes::typecheck::MATCH_NON_EXHAUSTIVE);
        let details = error.details.unwrap();
        assert_eq!(
            details.get("suggestedMissingArms").unwrap(),
            &serde_json::json!(["(false,true)", "(false,false)"])
        );
    }

    // Match coverage now has explicit tests for Bool, tuples, lists, sums, and
    // supported guard reasoning. Record patterns remain intentionally unsupported.

    // ========================================================================
    // Protocol type tests
    // ========================================================================

    #[test]
    fn test_protocol_declaration_parses_and_registers() {
        let source = concat!(
            "t Handle={id:String}\n",
            "protocol Handle\n",
            "  Open → Closed via close\n",
            "  initial = Open\n",
            "  terminal = Closed\n",
            "λclose(handle:Handle)=>Bool\n",
            "requires handle.state = Open\n",
            "ensures handle.state = Closed\n",
            "=true\n",
            "λmain()=>Bool={l h=({id:\"x\"}:Handle);close(h)}",
        );
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(
            result.is_ok(),
            "Protocol declaration should parse and register: {result:?}"
        );
    }

    #[test]
    fn test_protocol_state_violation_rejected() {
        // `close` requires Open state. After calling close, the proof context knows
        // the handle is Closed. A second close call should fail the requires check.
        let source = concat!(
            "t Handle={id:String}\n",
            "protocol Handle\n",
            "  Open → Closed via close\n",
            "  initial = Open\n",
            "  terminal = Closed\n",
            "λclose(handle:Handle)=>Bool\n",
            "requires handle.state = Open\n",
            "ensures handle.state = Closed\n",
            "=true\n",
            "λdoubleClose(h:Handle)=>Bool={l _=(close(h):Bool);close(h)}",
        );
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(
            result.is_err(),
            "Second close on a Closed handle should fail"
        );
        let err = result.unwrap_err();
        assert!(
            err.message.contains("requires clause") || err.message.contains("requires"),
            "Expected requires violation, got: {}",
            err.message
        );
    }

    #[test]
    fn test_protocol_unknown_type_rejected() {
        let source = concat!(
            "protocol NonExistentType\n",
            "  Open → Closed via foo\n",
            "  initial = Open\n",
            "  terminal = Closed\n",
            "λmain()=>Bool=true",
        );
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err(), "Protocol on unknown type should fail");
        assert!(
            result
                .unwrap_err()
                .message
                .contains("SIGIL-PROTO-UNKNOWN-TYPE"),
            "Expected SIGIL-PROTO-UNKNOWN-TYPE error"
        );
    }

    #[test]
    fn test_protocol_state_contract_validates_correctly() {
        // A function with a valid requires/ensures state contract should compile.
        let source = concat!(
            "t Conn={id:String}\n",
            "protocol Conn\n",
            "  Open → Open via send\n",
            "  Open → Closed via close\n",
            "  initial = Open\n",
            "  terminal = Closed\n",
            "λsend(conn:Conn,msg:String)=>Bool\n",
            "requires conn.state = Open\n",
            "ensures conn.state = Open\n",
            "=true\n",
            "λclose(conn:Conn)=>Bool\n",
            "requires conn.state = Open\n",
            "ensures conn.state = Closed\n",
            "=true\n",
            "λmain()=>Bool=true",
        );
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(
            result.is_ok(),
            "Valid protocol contracts should typecheck: {result:?}"
        );
    }
}
