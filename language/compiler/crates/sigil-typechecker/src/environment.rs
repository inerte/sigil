//! Sigil Type Checker - Type Environment
//!
//! Manages variable bindings during type checking.
//! Simplified from HM version - no type schemes, direct InferenceType bindings.

use crate::types::InferenceType;
use sigil_ast::TypeDef;
use std::collections::HashMap;

/// Type information for user-defined types
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub type_params: Vec<String>,   // Generic type parameters (e.g., ['T', 'E'] for Result[T,E])
    pub definition: TypeDef,         // The type definition (SumType, ProductType, or TypeAlias)
}

#[derive(Debug, Clone, Default)]
pub struct BindingMeta {
    pub is_mockable_function: bool,
    pub is_extern_namespace: bool,
}

/// Type environment (Γ in type theory notation)
///
/// Maps variable names to their types
/// Supports nested scopes via parent chaining
#[derive(Debug, Clone)]
pub struct TypeEnvironment {
    bindings: HashMap<String, InferenceType>,
    binding_meta: HashMap<String, BindingMeta>,
    type_registry: HashMap<String, TypeInfo>,               // User-defined types
    imported_type_registries: HashMap<String, HashMap<String, TypeInfo>>, // Types from imported modules
    parent: Option<Box<TypeEnvironment>>,
}

impl TypeEnvironment {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            binding_meta: HashMap::new(),
            type_registry: HashMap::new(),
            imported_type_registries: HashMap::new(),
            parent: None,
        }
    }

    /// Create a new environment with a parent
    fn with_parent(parent: TypeEnvironment) -> Self {
        Self {
            bindings: HashMap::new(),
            binding_meta: HashMap::new(),
            type_registry: HashMap::new(),
            imported_type_registries: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    /// Look up a variable's type
    ///
    /// Searches this environment and all parent environments
    pub fn lookup(&self, name: &str) -> Option<InferenceType> {
        if let Some(typ) = self.bindings.get(name) {
            return Some(typ.clone());
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

    /// Bind a variable with metadata
    pub fn bind_with_meta(&mut self, name: String, typ: InferenceType, meta: BindingMeta) {
        self.bindings.insert(name.clone(), typ);
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

    /// Create the initial environment with built-in operators
    pub fn create_initial() -> TypeEnvironment {
        let env = TypeEnvironment::new();

        // Built-in operators are handled directly in synthesize_binary/synthesize_unary
        // This environment is primarily for user-defined functions and constants

        env
    }

    /// Normalize a type by resolving type aliases
    ///
    /// If the type is a Constructor that refers to a type alias, resolve it to the underlying type.
    /// This enables structural compatibility for type aliases to record types.
    pub fn normalize_type(&self, ty: &InferenceType) -> InferenceType {
        match ty {
            InferenceType::Constructor(ctor) => {
                // Look up the constructor in the type registry
                if let Some(type_info) = self.lookup_type(&ctor.name) {
                    // Resolve type definition to its underlying structure
                    match &type_info.definition {
                        TypeDef::Alias(alias) => {
                            // Convert the aliased AST type to InferenceType and normalize recursively
                            use crate::types::ast_type_to_inference_type;
                            let underlying = ast_type_to_inference_type(&alias.aliased_type);
                            // Recursively normalize in case of nested aliases
                            return self.normalize_type(&underlying);
                        }
                        TypeDef::Product(product) => {
                            // Convert product type to record type for structural comparison
                            let fields: std::collections::HashMap<String, InferenceType> = product
                                .fields
                                .iter()
                                .map(|f| {
                                    use crate::types::ast_type_to_inference_type;
                                    (f.name.clone(), ast_type_to_inference_type(&f.field_type))
                                })
                                .collect();
                            return InferenceType::Record(crate::types::TRecord {
                                fields,
                                name: Some(ctor.name.clone()),
                            });
                        }
                        TypeDef::Sum(_) => {
                            // Sum types cannot be normalized to records
                        }
                    }
                }
                // Not an alias, return as-is
                ty.clone()
            }
            // For other types, recursively normalize nested types
            InferenceType::List(list) => {
                let normalized_elem = self.normalize_type(&list.element_type);
                InferenceType::List(Box::new(crate::types::TList {
                    element_type: normalized_elem,
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
