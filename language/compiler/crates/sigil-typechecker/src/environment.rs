//! Sigil Type Checker - Type Environment
//!
//! Manages variable bindings during type checking.
//! Uses explicit schemes for declared generic bindings without HM let-polymorphism.

use crate::types::{apply_subst, fresh_type_var, InferenceType, Substitution, TMap, TypeScheme};
use sigil_ast::{TypeDef, Variant};
use std::collections::{HashMap, HashSet};

/// Type information for user-defined types
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub type_params: Vec<String>,   // Generic type parameters (e.g., ['T', 'E'] for Result[T,E])
    pub definition: TypeDef,         // The type definition (SumType, ProductType, or TypeAlias)
}

#[derive(Debug, Clone, Default)]
pub struct BindingMeta {
    pub is_extern_namespace: bool,
}

/// Type environment (Γ in type theory notation)
///
/// Maps variable names to their types
/// Supports nested scopes via parent chaining
#[derive(Debug, Clone)]
pub struct TypeEnvironment {
    bindings: HashMap<String, InferenceType>,
    schemes: HashMap<String, TypeScheme>,
    binding_meta: HashMap<String, BindingMeta>,
    type_registry: HashMap<String, TypeInfo>,               // User-defined types
    imported_type_registries: HashMap<String, HashMap<String, TypeInfo>>, // Types from imported modules
    imported_value_schemes: HashMap<String, HashMap<String, TypeScheme>>,
    source_file: Option<String>,
    parent: Option<Box<TypeEnvironment>>,
}

impl TypeEnvironment {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            schemes: HashMap::new(),
            binding_meta: HashMap::new(),
            type_registry: HashMap::new(),
            imported_type_registries: HashMap::new(),
            imported_value_schemes: HashMap::new(),
            source_file: None,
            parent: None,
        }
    }

    /// Create a new environment with a parent
    fn with_parent(parent: TypeEnvironment) -> Self {
        Self {
            bindings: HashMap::new(),
            schemes: HashMap::new(),
            binding_meta: HashMap::new(),
            type_registry: HashMap::new(),
            imported_type_registries: HashMap::new(),
            imported_value_schemes: HashMap::new(),
            source_file: parent.source_file.clone(),
            parent: Some(Box::new(parent)),
        }
    }

    pub fn set_source_file(&mut self, source_file: Option<String>) {
        self.source_file = source_file;
    }

    pub fn source_file(&self) -> Option<&str> {
        self.source_file.as_deref()
    }

    /// Look up a variable's type
    ///
    /// Searches this environment and all parent environments
    pub fn lookup(&self, name: &str) -> Option<InferenceType> {
        if let Some(typ) = self.bindings.get(name) {
            return Some(typ.clone());
        }

        if let Some(scheme) = self.schemes.get(name) {
            return Some(instantiate_scheme(scheme));
        }

        // Search parent scope
        self.parent.as_ref()?.lookup(name)
    }

    /// Bind a variable to a type
    ///
    /// Only affects the current scope
    pub fn bind(&mut self, name: String, typ: InferenceType) {
        self.bindings.insert(name, typ);
    }

    /// Bind an explicitly generic declaration as a scheme.
    pub fn bind_scheme(&mut self, name: String, scheme: TypeScheme) {
        self.schemes.insert(name, scheme);
    }

    /// Bind a variable with metadata
    pub fn bind_with_meta(&mut self, name: String, typ: InferenceType, meta: BindingMeta) {
        self.bindings.insert(name.clone(), typ);
        self.binding_meta.insert(name, meta);
    }

    /// Bind an explicitly generic declaration as a scheme with metadata.
    pub fn bind_scheme_with_meta(&mut self, name: String, scheme: TypeScheme, meta: BindingMeta) {
        self.schemes.insert(name.clone(), scheme);
        self.binding_meta.insert(name, meta);
    }

    /// Look up binding metadata
    pub fn lookup_meta(&self, name: &str) -> Option<BindingMeta> {
        if let Some(meta) = self.binding_meta.get(name) {
            return Some(meta.clone());
        }

        // Search parent scope
        self.parent.as_ref()?.lookup_meta(name)
    }

    /// Register a user-defined type
    ///
    /// Stores type definition for later lookup during type checking
    pub fn register_type(&mut self, name: String, info: TypeInfo) {
        self.type_registry.insert(name, info);
    }

    /// Look up a user-defined type
    ///
    /// Searches this environment and all parent environments
    pub fn lookup_type(&self, name: &str) -> Option<TypeInfo> {
        if let Some(info) = self.type_registry.get(name) {
            return Some(info.clone());
        }

        // Search parent scope
        self.parent.as_ref()?.lookup_type(name)
    }

    /// Register types from an imported module
    pub fn register_imported_types(&mut self, module_id: String, types: HashMap<String, TypeInfo>) {
        self.imported_type_registries.insert(module_id, types);
    }

    /// Register exported value schemes from an imported module.
    pub fn register_imported_value_schemes(
        &mut self,
        module_id: String,
        value_schemes: HashMap<String, TypeScheme>,
    ) {
        self.imported_value_schemes.insert(module_id, value_schemes);
    }

    /// Look up a qualified type from an imported module
    ///
    /// Example: lookup_qualified_type(["src", "types"], "ArticleMeta")
    pub fn lookup_qualified_type(&self, module_path: &[String], type_name: &str) -> Option<TypeInfo> {
        let module_id = module_path.join("⋅");
        if let Some(registry) = self.imported_type_registries.get(&module_id) {
            if let Some(info) = registry.get(type_name) {
                return Some(info.clone());
            }
        }

        // Check parent scope
        self.parent.as_ref()?.lookup_qualified_type(module_path, type_name)
    }

    /// Look up an imported value member with fresh instantiation.
    pub fn lookup_qualified_value(
        &self,
        module_path: &[String],
        member_name: &str,
    ) -> Option<InferenceType> {
        let module_id = module_path.join("⋅");
        if let Some(registry) = self.imported_value_schemes.get(&module_id) {
            if let Some(scheme) = registry.get(member_name) {
                return Some(instantiate_scheme(scheme));
            }
        }

        self.parent
            .as_ref()?
            .lookup_qualified_value(module_path, member_name)
    }

    /// Look up a qualified constructor from an imported module.
    ///
    /// Returns the sum type name that owns the constructor and the variant definition.
    pub fn lookup_qualified_constructor(
        &self,
        module_path: &[String],
        constructor_name: &str,
    ) -> Option<(String, Vec<String>, Variant, Vec<String>)> {
        let module_id = module_path.join("⋅");

        if let Some(registry) = self.imported_type_registries.get(&module_id) {
            let mut matches = Vec::new();

            for (type_name, info) in registry {
                if let TypeDef::Sum(sum_type) = &info.definition {
                    for variant in &sum_type.variants {
                        if variant.name == constructor_name {
                            matches.push((
                                type_name.clone(),
                                module_path.to_vec(),
                                variant.clone(),
                                info.type_params.clone(),
                            ));
                        }
                    }
                }
            }

            if matches.len() == 1 {
                return matches.into_iter().next();
            }

            if matches.len() > 1 {
                return None;
            }
        }

        self.parent
            .as_ref()?
            .lookup_qualified_constructor(module_path, constructor_name)
    }

    /// Get all exported type names from a module (for error messages)
    pub fn get_imported_module_type_names(&self, module_id: &str) -> Option<Vec<String>> {
        if let Some(registry) = self.imported_type_registries.get(module_id) {
            let mut names: Vec<String> = registry.keys().cloned().collect();
            names.sort();
            return Some(names);
        }

        self.parent.as_ref()?.get_imported_module_type_names(module_id)
    }

    /// Create a child environment with additional bindings
    ///
    /// Example: when entering a lambda or match arm with pattern bindings
    pub fn extend(&self, new_bindings: Option<HashMap<String, InferenceType>>) -> TypeEnvironment {
        let mut child = Self::with_parent(self.clone());
        if let Some(bindings) = new_bindings {
            for (name, typ) in bindings {
                child.bind(name, typ);
            }
        }
        child
    }

    /// Get all bindings in this scope (for debugging/testing)
    pub fn get_bindings(&self) -> HashMap<String, InferenceType> {
        self.bindings.clone()
    }

    /// Get all explicit schemes in this scope (for tooling/exports).
    pub fn get_schemes(&self) -> HashMap<String, TypeScheme> {
        self.schemes.clone()
    }

    /// Create the initial environment with built-in operators
    pub fn create_initial() -> TypeEnvironment {
        let env = TypeEnvironment::new();

        // Built-in operators are handled directly in synthesize_binary/synthesize_unary
        // This environment is primarily for user-defined functions and constants

        env
    }

    /// Normalize a type to its canonical semantic form before equality checks.
    ///
    /// Sigil treats aliases and named product types as structural everywhere in the
    /// checker. This function resolves those names recursively so checker paths can
    /// compare one canonical meaning instead of branch-specific raw syntax shapes.
    ///
    /// Sum types remain nominal and are not rewritten into structural records.
    pub fn normalize_type(&self, ty: &InferenceType) -> InferenceType {
        match ty {
            InferenceType::Constructor(ctor) => {
                let qualified_lookup = split_qualified_type_name(&ctor.name)
                    .and_then(|(module_path, type_name)| self.lookup_qualified_type(&module_path, &type_name));
                let local_lookup = self.lookup_type(&ctor.name);
                let normalized_type_args: Vec<InferenceType> = ctor
                    .type_args
                    .iter()
                    .map(|arg| self.normalize_type(arg))
                    .collect();

                if let Some(type_info) = qualified_lookup.or(local_lookup) {
                    let type_param_bindings =
                        build_type_param_bindings(&type_info.type_params, &normalized_type_args);
                    // Resolve type definition to its underlying structure
                    match &type_info.definition {
                        TypeDef::Alias(alias) => {
                            // Convert the aliased AST type to InferenceType and normalize recursively
                            use crate::types::ast_type_to_inference_type_with_params;
                            let underlying = ast_type_to_inference_type_with_params(
                                &alias.aliased_type,
                                Some(&type_param_bindings),
                            );
                            // Recursively normalize in case of nested aliases
                            return self.normalize_type(&underlying);
                        }
                        TypeDef::Product(product) => {
                            // Convert product type to record type for structural comparison
                            let fields: std::collections::HashMap<String, InferenceType> = product
                                .fields
                                .iter()
                                .map(|f| {
                                    use crate::types::ast_type_to_inference_type_with_params;
                                    let field_type = ast_type_to_inference_type_with_params(
                                        &f.field_type,
                                        Some(&type_param_bindings),
                                    );
                                    (f.name.clone(), self.normalize_type(&field_type))
                                })
                                .collect();
                            return InferenceType::Record(crate::types::TRecord {
                                fields,
                                name: Some(ctor.name.clone()),
                            });
                        }
                        TypeDef::Sum(_) => {
                            // Sum types remain nominal, but their type arguments still
                            // normalize structurally so Result[Response,Error] can
                            // compare canonical nested meanings.
                            return InferenceType::Constructor(crate::types::TConstructor {
                                name: ctor.name.clone(),
                                type_args: normalized_type_args,
                            });
                        }
                    }
                }
                // Not a known alias/product/sum type; still normalize nested args.
                InferenceType::Constructor(crate::types::TConstructor {
                    name: ctor.name.clone(),
                    type_args: normalized_type_args,
                })
            }
            // For other types, recursively normalize nested types
            InferenceType::List(list) => {
                let normalized_elem = self.normalize_type(&list.element_type);
                InferenceType::List(Box::new(crate::types::TList {
                    element_type: normalized_elem,
                }))
            }
            InferenceType::Map(map) => {
                let normalized_key = self.normalize_type(&map.key_type);
                let normalized_value = self.normalize_type(&map.value_type);
                InferenceType::Map(Box::new(TMap {
                    key_type: normalized_key,
                    value_type: normalized_value,
                }))
            }
            InferenceType::Tuple(tuple) => {
                let normalized_types: Vec<InferenceType> = tuple
                    .types
                    .iter()
                    .map(|t| self.normalize_type(t))
                    .collect();
                InferenceType::Tuple(crate::types::TTuple {
                    types: normalized_types,
                })
            }
            InferenceType::Function(func) => {
                let normalized_params: Vec<InferenceType> = func
                    .params
                    .iter()
                    .map(|p| self.normalize_type(p))
                    .collect();
                let normalized_return = self.normalize_type(&func.return_type);
                InferenceType::Function(Box::new(crate::types::TFunction {
                    params: normalized_params,
                    effects: func.effects.clone(),
                    return_type: normalized_return,
                }))
            }
            InferenceType::Record(record) => {
                // Normalize field types
                let normalized_fields: std::collections::HashMap<String, InferenceType> = record
                    .fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.normalize_type(ty)))
                    .collect();
                InferenceType::Record(crate::types::TRecord {
                    fields: normalized_fields,
                    name: record.name.clone(),
                })
            }
            // Other types don't need normalization
            _ => ty.clone(),
        }
    }
}

fn split_qualified_type_name(name: &str) -> Option<(Vec<String>, String)> {
    let dot_index = name.rfind('.')?;
    let module_id = &name[..dot_index];
    let type_name = &name[dot_index + 1..];
    Some((
        module_id.split('⋅').map(|part| part.to_string()).collect(),
        type_name.to_string(),
    ))
}

fn build_type_param_bindings(
    type_params: &[String],
    type_args: &[InferenceType],
) -> HashMap<String, InferenceType> {
    type_params
        .iter()
        .cloned()
        .zip(type_args.iter().cloned())
        .collect()
}

fn instantiate_scheme(scheme: &TypeScheme) -> InferenceType {
    let mut subst: Substitution = HashMap::new();
    for var_id in &scheme.quantified_vars {
        subst.insert(*var_id, fresh_type_var(None));
    }
    apply_subst(&subst, &scheme.typ)
}

pub fn explicit_scheme(typ: &InferenceType, quantified_vars: &HashSet<u32>) -> TypeScheme {
    TypeScheme {
        quantified_vars: quantified_vars.clone(),
        typ: typ.clone(),
    }
}

pub fn collect_type_var_ids(typ: &InferenceType, ids: &mut HashSet<u32>) {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => {}
        InferenceType::Var(tvar) => {
            ids.insert(tvar.id);
            if let Some(instance) = &tvar.instance {
                collect_type_var_ids(instance, ids);
            }
        }
        InferenceType::Function(func) => {
            for param in &func.params {
                collect_type_var_ids(param, ids);
            }
            collect_type_var_ids(&func.return_type, ids);
        }
        InferenceType::List(list) => collect_type_var_ids(&list.element_type, ids),
        InferenceType::Map(map) => {
            collect_type_var_ids(&map.key_type, ids);
            collect_type_var_ids(&map.value_type, ids);
        }
        InferenceType::Tuple(tuple) => {
            for item in &tuple.types {
                collect_type_var_ids(item, ids);
            }
        }
        InferenceType::Record(record) => {
            for field_type in record.fields.values() {
                collect_type_var_ids(field_type, ids);
            }
        }
        InferenceType::Constructor(constructor) => {
            for arg in &constructor.type_args {
                collect_type_var_ids(arg, ids);
            }
        }
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{InferenceType, TPrimitive};
    use sigil_ast::PrimitiveName;

    #[test]
    fn test_bind_and_lookup() {
        let mut env = TypeEnvironment::new();
        let int_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });

        env.bind("x".to_string(), int_type.clone());
        assert_eq!(env.lookup("x"), Some(int_type));
        assert_eq!(env.lookup("y"), None);
    }

    #[test]
    fn test_extend() {
        let mut env = TypeEnvironment::new();
        let int_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });
        let bool_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool,
        });

        env.bind("x".to_string(), int_type.clone());

        let mut new_bindings = HashMap::new();
        new_bindings.insert("y".to_string(), bool_type.clone());
        let child_env = env.extend(Some(new_bindings));

        // Child should see both parent and its own bindings
        assert_eq!(child_env.lookup("x"), Some(int_type));
        assert_eq!(child_env.lookup("y"), Some(bool_type));

        // Parent should not see child bindings
        assert_eq!(env.lookup("y"), None);
    }
}
