use crate::bidirectional::{
    ast_type_to_inference_type_resolved, build_typed_expr, expr_location,
    split_qualified_constructor_name, type_location,
};
use crate::environment::{BindingMeta, TypeEnvironment, TypeInfo};
use crate::errors::{format_type, TypeError};
use crate::typed_ir::{
    JsonCodecField, JsonCodecHelperNames, JsonCodecNamedBody, JsonCodecNamedType, JsonCodecType,
    JsonCodecVariant, JsonConstraintValidator, TypedJsonCodecDecl,
};
use crate::types::{
    ast_type_to_inference_type_with_params, InferenceType, TConstructor, TPrimitive,
};
use sigil_ast::{DeriveDecl, Expr, FunctionMode, PrimitiveName, Type, TypeDef};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonCodecSurfaceInfo {
    pub target_name: String,
    pub helper_names: JsonCodecHelperNames,
}

#[derive(Debug, Clone)]
pub(crate) struct JsonCodecSeed {
    pub target: Type,
    pub target_name: String,
    pub target_type_id: String,
    pub root_type: InferenceType,
    pub helper_names: JsonCodecHelperNames,
    pub named_types: Vec<JsonCodecNamedTypeSeed>,
    pub location: sigil_ast::SourceLocation,
}

#[derive(Debug, Clone)]
pub(crate) struct JsonCodecNamedTypeSeed {
    pub type_id: String,
    pub resolved_name: String,
    pub base_name: String,
    pub helper_suffix: String,
    pub body: JsonCodecNamedBody,
    pub nullable: bool,
    pub constraint: Option<JsonConstraintValidatorSeed>,
}

#[derive(Debug, Clone)]
pub(crate) struct JsonConstraintValidatorSeed {
    source: String,
    failure_message: String,
    value_type: InferenceType,
    predicate: Expr,
}

#[derive(Debug, Clone)]
struct NamedTypeInstance {
    type_id: String,
    resolved_name: String,
    base_name: String,
    type_args: Vec<InferenceType>,
    info: TypeInfo,
}

#[derive(Debug, Clone)]
struct AnalyzedCodecType {
    typ: JsonCodecType,
    nullable: bool,
}

pub(crate) fn analyze_json_codec_decl(
    env: &TypeEnvironment,
    derive_decl: &DeriveDecl,
    source_code: &str,
) -> Result<JsonCodecSeed, TypeError> {
    let root = resolve_root_target(env, derive_decl)?;
    let root_type = ast_type_to_inference_type_resolved(env, None, &derive_decl.target)?;
    let target_name = root.base_name.clone();
    let target_type_id = root.type_id.clone();
    let helper_names = helper_names_for_target(&target_name);

    let mut analyzer = JsonCodecAnalyzer::new(env, source_code);
    analyzer.analyze_named_instance(root)?;

    Ok(JsonCodecSeed {
        target: derive_decl.target.clone(),
        target_name,
        target_type_id,
        root_type,
        helper_names,
        named_types: analyzer.entries,
        location: derive_decl.location,
    })
}

pub(crate) fn bind_json_codec_helpers(
    env: &mut TypeEnvironment,
    declaration_types: &mut HashMap<String, InferenceType>,
    seed: &JsonCodecSeed,
) {
    let json_value_type = InferenceType::Constructor(TConstructor {
        name: "stdlib::json.JsonValue".to_string(),
        type_args: Vec::new(),
    });
    let decode_error_type = InferenceType::Constructor(TConstructor {
        name: "stdlib::decode.DecodeError".to_string(),
        type_args: Vec::new(),
    });
    let result_type = |ok_type: InferenceType| {
        InferenceType::Constructor(TConstructor {
            name: "Result".to_string(),
            type_args: vec![ok_type, decode_error_type.clone()],
        })
    };
    let string_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::String,
    });

    let bindings = [
        (
            seed.helper_names.encode.clone(),
            InferenceType::Function(Box::new(crate::types::TFunction {
                params: vec![seed.root_type.clone()],
                return_type: json_value_type.clone(),
                effects: None,
            })),
        ),
        (
            seed.helper_names.decode.clone(),
            InferenceType::Function(Box::new(crate::types::TFunction {
                params: vec![json_value_type.clone()],
                return_type: result_type(seed.root_type.clone()),
                effects: None,
            })),
        ),
        (
            seed.helper_names.parse.clone(),
            InferenceType::Function(Box::new(crate::types::TFunction {
                params: vec![string_type.clone()],
                return_type: result_type(seed.root_type.clone()),
                effects: None,
            })),
        ),
        (
            seed.helper_names.stringify.clone(),
            InferenceType::Function(Box::new(crate::types::TFunction {
                params: vec![seed.root_type.clone()],
                return_type: string_type,
                effects: None,
            })),
        ),
    ];

    for (name, typ) in bindings {
        env.bind_with_meta(
            name.clone(),
            typ.clone(),
            BindingMeta {
                function_mode: Some(FunctionMode::Total),
                ..BindingMeta::default()
            },
        );
        declaration_types.insert(name, typ);
    }
}

pub(crate) fn finalize_json_codec_decl(
    env: &TypeEnvironment,
    seed: JsonCodecSeed,
) -> Result<TypedJsonCodecDecl, TypeError> {
    let mut named_types = Vec::with_capacity(seed.named_types.len());
    for named in seed.named_types {
        let constraint = if let Some(seed_validator) = named.constraint {
            let mut bindings = HashMap::new();
            bindings.insert("value".to_string(), seed_validator.value_type);
            let constraint_env = env.extend(Some(bindings));
            let predicate = build_typed_expr(&constraint_env, &seed_validator.predicate)?;
            Some(JsonConstraintValidator {
                source: seed_validator.source,
                failure_message: seed_validator.failure_message,
                predicate,
            })
        } else {
            None
        };

        named_types.push(JsonCodecNamedType {
            type_id: named.type_id,
            resolved_name: named.resolved_name,
            base_name: named.base_name,
            helper_suffix: named.helper_suffix,
            body: named.body,
            nullable: named.nullable,
            constraint,
        });
    }

    Ok(TypedJsonCodecDecl {
        target: seed.target,
        target_name: seed.target_name,
        target_type_id: seed.target_type_id,
        root_type: seed.root_type,
        helper_names: seed.helper_names,
        named_types,
        location: seed.location,
    })
}

pub fn derive_json_surface_info_for_type(
    root_type: &InferenceType,
    module_id: Option<&str>,
    source_file: Option<&str>,
    local_type_registry: &HashMap<String, TypeInfo>,
    imported_type_registries: &HashMap<String, HashMap<String, TypeInfo>>,
) -> Result<JsonCodecSurfaceInfo, TypeError> {
    let mut env = TypeEnvironment::new();
    env.set_module_id(module_id.map(ToOwned::to_owned));
    env.set_source_file(source_file.map(ToOwned::to_owned));
    for (name, info) in local_type_registry {
        env.register_type(name.clone(), info.clone());
    }
    for (imported_module_id, registry) in imported_type_registries {
        env.register_imported_types(imported_module_id.clone(), registry.clone());
    }

    let instance = match root_type {
        InferenceType::Constructor(constructor) => {
            resolve_nested_named_instance(&env, constructor, zero_source_location())?
        }
        InferenceType::Record(record) => {
            let Some(name) = &record.name else {
                return Err(TypeError::new(
                    format!(
                        "derive json expects a named type target, got {}",
                        format_type(root_type)
                    ),
                    Some(zero_source_location()),
                ));
            };
            resolve_nested_named_instance(
                &env,
                &TConstructor {
                    name: name.clone(),
                    type_args: Vec::new(),
                },
                zero_source_location(),
            )?
        }
        other => {
            return Err(TypeError::new(
                format!(
                    "derive json expects a named type target, got {}",
                    format_type(other)
                ),
                Some(zero_source_location()),
            ));
        }
    };

    let target_name = instance.base_name.clone();
    let helper_names = helper_names_for_target(&target_name);
    let mut analyzer = JsonCodecAnalyzer::new(&env, "");
    analyzer.analyze_named_instance(instance)?;
    Ok(JsonCodecSurfaceInfo {
        target_name,
        helper_names,
    })
}

fn resolve_root_target(
    env: &TypeEnvironment,
    derive_decl: &DeriveDecl,
) -> Result<NamedTypeInstance, TypeError> {
    match &derive_decl.target {
        Type::Constructor(constructor) => {
            if !constructor.type_args.is_empty() {
                return Err(TypeError::new(
                    "derive json targets must not include type arguments in v1; derive a monomorphic named alias or wrapper instead".to_string(),
                    Some(constructor.location),
                ));
            }
            let Some(info) = env.lookup_type(&constructor.name) else {
                return Err(TypeError::new(
                    format!(
                        "derive json target '{}' must name a type declared in this module",
                        constructor.name
                    ),
                    Some(constructor.location),
                ));
            };
            if !info.type_params.is_empty() {
                return Err(TypeError::new(
                    format!(
                        "derive json target '{}' must be monomorphic; generic type declarations are not supported as public derive roots in v1",
                        constructor.name
                    ),
                    Some(constructor.location),
                ));
            }
            Ok(NamedTypeInstance {
                type_id: constructor.name.clone(),
                resolved_name: constructor.name.clone(),
                base_name: constructor.name.clone(),
                type_args: Vec::new(),
                info,
            })
        }
        Type::Qualified(qualified) => {
            if !qualified.type_args.is_empty() {
                return Err(TypeError::new(
                    "derive json targets must not include type arguments in v1; derive a monomorphic named alias or wrapper instead".to_string(),
                    Some(qualified.location),
                ));
            }
            let requested_module_id = qualified.module_path.join("::");
            let resolved_module_id = env
                .remap_package_local_module_id(&requested_module_id)
                .unwrap_or(requested_module_id);
            let resolved_path = resolved_module_id
                .split("::")
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let Some(info) = env.lookup_qualified_type(&resolved_path, &qualified.type_name) else {
                return Err(TypeError::new(
                    format!(
                        "derive json target '{}' is not exported from module '{}'",
                        qualified.type_name, resolved_module_id
                    ),
                    Some(qualified.location),
                ));
            };
            if !info.type_params.is_empty() {
                return Err(TypeError::new(
                    format!(
                        "derive json target '{}.{}' must be monomorphic; generic type declarations are not supported as public derive roots in v1",
                        resolved_module_id, qualified.type_name
                    ),
                    Some(qualified.location),
                ));
            }
            Ok(NamedTypeInstance {
                type_id: format!("{}.{}", resolved_module_id, qualified.type_name),
                resolved_name: format!("{}.{}", resolved_module_id, qualified.type_name),
                base_name: qualified.type_name.clone(),
                type_args: Vec::new(),
                info,
            })
        }
        other => Err(TypeError::new(
            format!(
                "derive json expects a named type target, got {}",
                format_type(&ast_type_to_inference_type_with_params(other, None))
            ),
            Some(type_location(other)),
        )),
    }
}

struct JsonCodecAnalyzer<'a> {
    env: &'a TypeEnvironment,
    source_code: &'a str,
    entries: Vec<JsonCodecNamedTypeSeed>,
    index_by_type_id: HashMap<String, usize>,
    active_stack: Vec<String>,
}

impl<'a> JsonCodecAnalyzer<'a> {
    fn new(env: &'a TypeEnvironment, source_code: &'a str) -> Self {
        Self {
            env,
            source_code,
            entries: Vec::new(),
            index_by_type_id: HashMap::new(),
            active_stack: Vec::new(),
        }
    }

    fn analyze_named_instance(
        &mut self,
        instance: NamedTypeInstance,
    ) -> Result<AnalyzedCodecType, TypeError> {
        if self.index_by_type_id.contains_key(&instance.type_id) {
            let entry = &self.entries[self.index_by_type_id[&instance.type_id]];
            return Ok(AnalyzedCodecType {
                typ: JsonCodecType::Named {
                    type_id: instance.type_id,
                    helper_suffix: entry.helper_suffix.clone(),
                },
                nullable: entry.nullable,
            });
        }

        if self.active_stack.contains(&instance.type_id) {
            return Err(TypeError::new(
                format!(
                    "derive json does not yet support recursive type graphs; '{}' participates in a cycle",
                    instance.base_name
                ),
                None,
            ));
        }

        self.active_stack.push(instance.type_id.clone());

        let type_param_bindings: HashMap<String, InferenceType> = instance
            .info
            .type_params
            .iter()
            .cloned()
            .zip(instance.type_args.iter().cloned())
            .collect();

        if instance.info.constraint.is_some() && matches!(instance.info.definition, TypeDef::Sum(_))
        {
            self.active_stack.pop();
            return Err(TypeError::new(
                format!(
                    "derive json does not yet support constrained sum type '{}'",
                    instance.base_name
                ),
                None,
            ));
        }

        let (body, nullable) = match &instance.info.definition {
            TypeDef::Alias(alias) => {
                let inner = self.analyze_ast_type(&alias.aliased_type, &type_param_bindings)?;
                (
                    JsonCodecNamedBody::Alias { inner: inner.typ },
                    inner.nullable,
                )
            }
            TypeDef::Product(product) => {
                let mut fields = Vec::with_capacity(product.fields.len());
                for field in &product.fields {
                    let analyzed =
                        self.analyze_ast_type(&field.field_type, &type_param_bindings)?;
                    fields.push(JsonCodecField {
                        name: field.name.clone(),
                        typ: analyzed.typ,
                    });
                }
                (JsonCodecNamedBody::Product { fields }, false)
            }
            TypeDef::Sum(sum) => {
                if sum.variants.len() == 1
                    && sum.variants[0].name == instance.base_name
                    && sum.variants[0].types.len() == 1
                {
                    let inner =
                        self.analyze_ast_type(&sum.variants[0].types[0], &type_param_bindings)?;
                    (
                        JsonCodecNamedBody::Wrapper {
                            variant_name: sum.variants[0].name.clone(),
                            inner: inner.typ,
                        },
                        inner.nullable,
                    )
                } else {
                    let mut variants = Vec::with_capacity(sum.variants.len());
                    for variant in &sum.variants {
                        let mut fields = Vec::with_capacity(variant.types.len());
                        for field_type in &variant.types {
                            fields
                                .push(self.analyze_ast_type(field_type, &type_param_bindings)?.typ);
                        }
                        variants.push(JsonCodecVariant {
                            name: variant.name.clone(),
                            fields,
                        });
                    }
                    (JsonCodecNamedBody::Sum { variants }, false)
                }
            }
        };

        let constraint =
            instance
                .info
                .constraint
                .as_ref()
                .map(|predicate| JsonConstraintValidatorSeed {
                    source: normalized_constraint_source(self.source_code, predicate),
                    failure_message: format!(
                        "constraint failed for {}: {}",
                        instance.base_name,
                        normalized_constraint_source(self.source_code, predicate)
                    ),
                    value_type: constraint_value_type(&instance.info, &type_param_bindings),
                    predicate: predicate.clone(),
                });

        let helper_suffix = helper_suffix_for_type_id(&instance.type_id);
        let entry = JsonCodecNamedTypeSeed {
            type_id: instance.type_id.clone(),
            resolved_name: instance.resolved_name.clone(),
            base_name: instance.base_name.clone(),
            helper_suffix: helper_suffix.clone(),
            body,
            nullable,
            constraint,
        };

        let index = self.entries.len();
        self.index_by_type_id
            .insert(instance.type_id.clone(), index);
        self.entries.push(entry);
        self.active_stack.pop();

        Ok(AnalyzedCodecType {
            typ: JsonCodecType::Named {
                type_id: instance.type_id,
                helper_suffix,
            },
            nullable,
        })
    }

    fn analyze_ast_type(
        &mut self,
        ast_type: &Type,
        type_param_bindings: &HashMap<String, InferenceType>,
    ) -> Result<AnalyzedCodecType, TypeError> {
        match ast_type {
            Type::Variable(variable) => {
                if let Some(bound) = type_param_bindings.get(&variable.name) {
                    return self.analyze_bound_type(bound, variable.location);
                }

                if self.env.lookup_type(&variable.name).is_some() {
                    let inference =
                        ast_type_to_inference_type_with_params(ast_type, Some(type_param_bindings));
                    return self.analyze_bound_type(&inference, variable.location);
                }

                Err(TypeError::new(
                    format!(
                        "derive json could not resolve type parameter '{}' while analyzing a generic type instantiation",
                        variable.name
                    ),
                    Some(variable.location),
                ))
            }
            _ => {
                let inference =
                    ast_type_to_inference_type_with_params(ast_type, Some(type_param_bindings));
                self.analyze_bound_type(&inference, type_location(ast_type))
            }
        }
    }

    fn analyze_bound_type(
        &mut self,
        typ: &InferenceType,
        location: sigil_ast::SourceLocation,
    ) -> Result<AnalyzedCodecType, TypeError> {
        match typ {
            InferenceType::Primitive(primitive) => match primitive.name {
                PrimitiveName::Bool => Ok(AnalyzedCodecType {
                    typ: JsonCodecType::Bool,
                    nullable: false,
                }),
                PrimitiveName::Float => Ok(AnalyzedCodecType {
                    typ: JsonCodecType::Float,
                    nullable: false,
                }),
                PrimitiveName::Int => Ok(AnalyzedCodecType {
                    typ: JsonCodecType::Int,
                    nullable: false,
                }),
                PrimitiveName::String => Ok(AnalyzedCodecType {
                    typ: JsonCodecType::String,
                    nullable: false,
                }),
                PrimitiveName::Char | PrimitiveName::Unit | PrimitiveName::Never => {
                    Err(TypeError::new(
                        format!(
                            "derive json does not support {} values in v1",
                            format_type(typ)
                        ),
                        Some(location),
                    ))
                }
            },
            InferenceType::List(list) => {
                let element = self.analyze_bound_type(&list.element_type, location)?;
                Ok(AnalyzedCodecType {
                    typ: JsonCodecType::List(Box::new(element.typ)),
                    nullable: false,
                })
            }
            InferenceType::Map(map) => {
                match &map.key_type {
                    InferenceType::Primitive(TPrimitive {
                        name: PrimitiveName::String,
                    }) => {}
                    other => {
                        return Err(TypeError::new(
                            format!(
                                "derive json only supports maps with String keys, found {}",
                                format_type(other)
                            ),
                            Some(location),
                        ));
                    }
                }

                let value = self.analyze_bound_type(&map.value_type, location)?;
                Ok(AnalyzedCodecType {
                    typ: JsonCodecType::MapString(Box::new(value.typ)),
                    nullable: false,
                })
            }
            InferenceType::Constructor(constructor) if constructor.name == "Option" => {
                if constructor.type_args.len() != 1 {
                    return Err(TypeError::new(
                        "Option expects exactly one type argument".to_string(),
                        Some(location),
                    ));
                }
                let inner = self.analyze_bound_type(&constructor.type_args[0], location)?;
                if inner.nullable {
                    return Err(TypeError::new(
                        "derive json rejects Option payloads whose canonical encoding can already be null".to_string(),
                        Some(location),
                    ));
                }
                Ok(AnalyzedCodecType {
                    typ: JsonCodecType::Option(Box::new(inner.typ)),
                    nullable: true,
                })
            }
            InferenceType::Constructor(constructor) => {
                let instance = resolve_nested_named_instance(self.env, constructor, location)?;
                self.analyze_named_instance(instance)
            }
            other => Err(TypeError::new(
                format!(
                    "derive json does not support {} values in v1",
                    format_type(other)
                ),
                Some(location),
            )),
        }
    }
}

fn resolve_nested_named_instance(
    env: &TypeEnvironment,
    constructor: &TConstructor,
    location: sigil_ast::SourceLocation,
) -> Result<NamedTypeInstance, TypeError> {
    if let Some((module_path, type_name)) = split_qualified_constructor_name(&constructor.name) {
        let requested_module_id = module_path.join("::");
        let resolved_module_id = env
            .remap_package_local_module_id(&requested_module_id)
            .unwrap_or(requested_module_id);
        let resolved_path = resolved_module_id
            .split("::")
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let Some(info) = env.lookup_qualified_type(&resolved_path, &type_name) else {
            return Err(TypeError::new(
                format!(
                    "derive json could not resolve named type '{}.{}'",
                    resolved_module_id, type_name
                ),
                Some(location),
            ));
        };
        validate_named_type_arity(&type_name, &info, &constructor.type_args, location)?;
        let resolved_name = format!("{}.{}", resolved_module_id, type_name);
        return Ok(NamedTypeInstance {
            type_id: render_type_id(&resolved_name, &constructor.type_args),
            resolved_name,
            base_name: type_name,
            type_args: constructor.type_args.clone(),
            info,
        });
    }

    if let Some(info) = env.lookup_type(&constructor.name) {
        validate_named_type_arity(&constructor.name, &info, &constructor.type_args, location)?;
        return Ok(NamedTypeInstance {
            type_id: render_type_id(&constructor.name, &constructor.type_args),
            resolved_name: constructor.name.clone(),
            base_name: constructor.name.clone(),
            type_args: constructor.type_args.clone(),
            info,
        });
    }

    let core_prelude_path = ["core".to_string(), "prelude".to_string()];
    if let Some(info) = env.lookup_qualified_type(&core_prelude_path, &constructor.name) {
        validate_named_type_arity(&constructor.name, &info, &constructor.type_args, location)?;
        let resolved_name = format!("core::prelude.{}", constructor.name);
        return Ok(NamedTypeInstance {
            type_id: render_type_id(&resolved_name, &constructor.type_args),
            resolved_name,
            base_name: constructor.name.clone(),
            type_args: constructor.type_args.clone(),
            info,
        });
    }

    Err(TypeError::new(
        format!(
            "derive json could not resolve named type '{}'",
            constructor.name
        ),
        Some(location),
    ))
}

fn validate_named_type_arity(
    type_name: &str,
    info: &TypeInfo,
    type_args: &[InferenceType],
    location: sigil_ast::SourceLocation,
) -> Result<(), TypeError> {
    if type_args.len() != info.type_params.len() {
        return Err(TypeError::new(
            format!(
                "derive json expected {} type argument{} for '{}', but found {}",
                info.type_params.len(),
                if info.type_params.len() == 1 { "" } else { "s" },
                type_name,
                type_args.len()
            ),
            Some(location),
        ));
    }
    Ok(())
}

fn helper_names_for_target(target_name: &str) -> JsonCodecHelperNames {
    JsonCodecHelperNames {
        encode: format!("encode{}", target_name),
        decode: format!("decode{}", target_name),
        parse: format!("parse{}", target_name),
        stringify: format!("stringify{}", target_name),
    }
}

fn helper_suffix_for_type_id(type_id: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_underscore = false;
    for ch in type_id.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch);
            last_was_underscore = false;
        } else if !last_was_underscore {
            sanitized.push('_');
            last_was_underscore = true;
        }
    }
    let sanitized = sanitized.trim_matches('_');
    let sanitized = if sanitized.is_empty() {
        "type".to_string()
    } else {
        sanitized.to_string()
    };
    format!("{}_{}", sanitized, stable_hash(type_id))
}

fn zero_source_location() -> sigil_ast::SourceLocation {
    sigil_ast::SourceLocation {
        start: sigil_ast::Position {
            line: 1,
            column: 1,
            offset: 0,
        },
        end: sigil_ast::Position {
            line: 1,
            column: 1,
            offset: 0,
        },
    }
}

fn stable_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn render_type_id(base_name: &str, type_args: &[InferenceType]) -> String {
    if type_args.is_empty() {
        base_name.to_string()
    } else {
        format!(
            "{}[{}]",
            base_name,
            type_args
                .iter()
                .map(format_type)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn constraint_value_type(
    info: &TypeInfo,
    type_param_bindings: &HashMap<String, InferenceType>,
) -> InferenceType {
    match &info.definition {
        TypeDef::Alias(alias) => {
            ast_type_to_inference_type_with_params(&alias.aliased_type, Some(type_param_bindings))
        }
        TypeDef::Product(product) => InferenceType::Record(crate::types::TRecord {
            fields: product
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        ast_type_to_inference_type_with_params(
                            &field.field_type,
                            Some(type_param_bindings),
                        ),
                    )
                })
                .collect(),
            name: None,
        }),
        TypeDef::Sum(_) => InferenceType::Constructor(TConstructor {
            name: "<unsupported constrained sum>".to_string(),
            type_args: Vec::new(),
        }),
    }
}

fn normalized_constraint_source(source_code: &str, expr: &Expr) -> String {
    let location = expr_location(expr);
    source_code
        .get(location.start.offset..location.end.offset)
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "constraint".to_string())
}
