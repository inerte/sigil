//! Sigil Type Checker - Core Type System
//!
//! Internal type representations used during type checking.
//! These are distinct from AST types and optimized for unification and substitution.

use sigil_ast::{PrimitiveName, Type as AstType, TypeConstructor};
use sigil_lexer::SourceLocation;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};

// ============================================================================
// INFERENCE TYPES
// ============================================================================

/// Internal type representation for type inference
///
/// This is separate from AST::Type to allow for features like:
/// - Type variables with instance tracking (for unification)
/// - Efficient substitution
/// - Type schemes for polymorphism
#[derive(Debug, Clone, PartialEq)]
pub enum InferenceType {
    Primitive(TPrimitive),
    Var(Box<TVar>),
    Function(Box<TFunction>),
    List(Box<TList>),
    Tuple(TTuple),
    Record(TRecord),
    Constructor(TConstructor),
    Any, // For FFI namespaces - trust mode, validated at link-time
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TPrimitive {
    pub name: PrimitiveName,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TVar {
    pub id: u32,                           // Unique type variable ID
    pub name: Option<String>,              // Optional name for display (α, β, T, U)
    pub instance: Option<InferenceType>,   // For unification - points to actual type when unified
}

#[derive(Debug, Clone, PartialEq)]
pub struct TFunction {
    pub params: Vec<InferenceType>,
    pub return_type: InferenceType,
    pub effects: Option<EffectSet>, // Effect tracking (!IO, !Network, etc.)
}

#[derive(Debug, Clone, PartialEq)]
pub struct TList {
    pub element_type: InferenceType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TTuple {
    pub types: Vec<InferenceType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TRecord {
    pub fields: HashMap<String, InferenceType>,
    pub name: Option<String>, // For user-defined types
}

#[derive(Debug, Clone, PartialEq)]
pub struct TConstructor {
    pub name: String,              // e.g., "Option", "Result", "Maybe"
    pub type_args: Vec<InferenceType>, // Generic type arguments
}

// ============================================================================
// TYPE SCHEMES (Polymorphism)
// ============================================================================

/// Type scheme for polymorphic types (∀α₁...αₙ.τ)
///
/// Example:
///   identity : ∀T. T → T
///   map : ∀T,U. (T → U) → [T] → [U]
#[derive(Debug, Clone, PartialEq)]
pub struct TypeScheme {
    pub quantified_vars: HashSet<u32>, // Type variable IDs that are quantified
    pub typ: InferenceType,             // The actual type with quantified variables
}

// ============================================================================
// SUBSTITUTIONS
// ============================================================================

/// Type substitution (mapping from type variables to types)
///
/// Example: [α₁ ↦ Int, α₂ ↦ String]
pub type Substitution = HashMap<u32, InferenceType>;

// ============================================================================
// EFFECTS
// ============================================================================

/// Effect system for tracking side effects
/// Tracks compile-time effects for function calls
pub type EffectSet = HashSet<String>; // {'IO', 'Network', 'Async', 'Error', 'Mut'}

// ============================================================================
// TYPE VARIABLE GENERATION
// ============================================================================

static NEXT_TYPE_VAR_ID: AtomicU32 = AtomicU32::new(0);

/// Create a fresh type variable
pub fn fresh_type_var(name: Option<String>) -> InferenceType {
    let id = NEXT_TYPE_VAR_ID.fetch_add(1, Ordering::SeqCst);
    InferenceType::Var(Box::new(TVar {
        id,
        name,
        instance: None,
    }))
}

/// Reset type variable counter (for testing)
#[cfg(test)]
pub fn reset_type_var_counter() {
    NEXT_TYPE_VAR_ID.store(0, Ordering::SeqCst);
}

// ============================================================================
// TYPE UTILITIES
// ============================================================================

/// Apply a substitution to a type
///
/// Recursively replaces type variables with their substituted types
pub fn apply_subst(subst: &Substitution, typ: &InferenceType) -> InferenceType {
    match typ {
        InferenceType::Primitive(p) => InferenceType::Primitive(p.clone()),

        InferenceType::Var(tvar) => {
            // If this variable has a substitution, use it
            if let Some(subst_type) = subst.get(&tvar.id) {
                // Recursively apply substitution (in case the substitution contains more variables)
                return apply_subst(subst, subst_type);
            }
            // If variable has an instance (from unification), follow it
            if let Some(ref instance) = tvar.instance {
                return apply_subst(subst, instance);
            }
            InferenceType::Var(tvar.clone())
        }

        InferenceType::Function(tfunc) => {
            InferenceType::Function(Box::new(TFunction {
                params: tfunc.params.iter().map(|p| apply_subst(subst, p)).collect(),
                return_type: apply_subst(subst, &tfunc.return_type),
                effects: tfunc.effects.clone(),
            }))
        }

        InferenceType::List(tlist) => {
            InferenceType::List(Box::new(TList {
                element_type: apply_subst(subst, &tlist.element_type),
            }))
        }

        InferenceType::Tuple(ttuple) => {
            InferenceType::Tuple(TTuple {
                types: ttuple.types.iter().map(|t| apply_subst(subst, t)).collect(),
            })
        }

        InferenceType::Record(trec) => {
            let mut new_fields = HashMap::new();
            for (field_name, field_type) in &trec.fields {
                new_fields.insert(field_name.clone(), apply_subst(subst, field_type));
            }
            InferenceType::Record(TRecord {
                fields: new_fields,
                name: trec.name.clone(),
            })
        }

        InferenceType::Constructor(tcons) => {
            InferenceType::Constructor(TConstructor {
                name: tcons.name.clone(),
                type_args: tcons.type_args.iter().map(|arg| apply_subst(subst, arg)).collect(),
            })
        }

        InferenceType::Any => InferenceType::Any,
    }
}

/// Apply substitution to a type scheme
///
/// Only substitute free variables (not quantified ones)
pub fn apply_subst_to_scheme(subst: &Substitution, scheme: &TypeScheme) -> TypeScheme {
    // Filter out quantified variables from substitution
    let mut filtered_subst = HashMap::new();
    for (var_id, subst_type) in subst {
        if !scheme.quantified_vars.contains(var_id) {
            filtered_subst.insert(*var_id, subst_type.clone());
        }
    }

    TypeScheme {
        quantified_vars: scheme.quantified_vars.clone(),
        typ: apply_subst(&filtered_subst, &scheme.typ),
    }
}

/// Dereference a type variable chain (follow instances)
///
/// Returns the final type after following all instance links
pub fn prune(typ: &InferenceType) -> InferenceType {
    match typ {
        InferenceType::Var(tvar) => {
            if let Some(ref instance) = tvar.instance {
                // Follow the chain and update the instance for path compression
                let pruned = prune(instance);
                pruned
            } else {
                typ.clone()
            }
        }
        _ => typ.clone(),
    }
}

/// Check if two types are equal (structurally)
///
/// Follows type variable instances before comparing
pub fn types_equal(t1: &InferenceType, t2: &InferenceType) -> bool {
    let t1 = prune(t1);
    let t2 = prune(t2);

    match (&t1, &t2) {
        (InferenceType::Primitive(p1), InferenceType::Primitive(p2)) => p1.name == p2.name,

        (InferenceType::Var(v1), InferenceType::Var(v2)) => v1.id == v2.id,

        (InferenceType::Function(f1), InferenceType::Function(f2)) => {
            f1.params.len() == f2.params.len()
                && f1.params.iter().zip(&f2.params).all(|(p1, p2)| types_equal(p1, p2))
                && types_equal(&f1.return_type, &f2.return_type)
        }

        (InferenceType::List(l1), InferenceType::List(l2)) => {
            types_equal(&l1.element_type, &l2.element_type)
        }

        (InferenceType::Tuple(t1), InferenceType::Tuple(t2)) => {
            t1.types.len() == t2.types.len()
                && t1.types.iter().zip(&t2.types).all(|(ty1, ty2)| types_equal(ty1, ty2))
        }

        (InferenceType::Record(r1), InferenceType::Record(r2)) => {
            r1.fields.len() == r2.fields.len()
                && r1.fields.iter().all(|(name, ty1)| {
                    r2.fields.get(name).map_or(false, |ty2| types_equal(ty1, ty2))
                })
        }

        (InferenceType::Constructor(c1), InferenceType::Constructor(c2)) => {
            c1.name == c2.name
                && c1.type_args.len() == c2.type_args.len()
                && c1.type_args.iter().zip(&c2.type_args).all(|(a1, a2)| types_equal(a1, a2))
        }

        (InferenceType::Any, _) | (_, InferenceType::Any) => true,

        _ => false,
    }
}

/// Convert AST type to InferenceType
///
/// Used when type annotations are present in the source
pub fn ast_type_to_inference_type(ast_type: &AstType) -> InferenceType {
    match ast_type {
        AstType::Primitive(p) => InferenceType::Primitive(TPrimitive { name: p.name }),

        AstType::List(list_type) => InferenceType::List(Box::new(TList {
            element_type: ast_type_to_inference_type(&list_type.element_type),
        })),

        AstType::Tuple(tuple_type) => InferenceType::Tuple(TTuple {
            types: tuple_type.types.iter().map(ast_type_to_inference_type).collect(),
        }),

        AstType::Function(func_type) => InferenceType::Function(Box::new(TFunction {
            params: func_type.param_types.iter().map(ast_type_to_inference_type).collect(),
            return_type: ast_type_to_inference_type(&func_type.return_type),
            effects: if func_type.effects.is_empty() {
                None
            } else {
                Some(func_type.effects.iter().cloned().collect())
            },
        })),

        AstType::Constructor(tc) => InferenceType::Constructor(TConstructor {
            name: tc.name.clone(),
            type_args: tc.type_args.iter().map(ast_type_to_inference_type).collect(),
        }),

        AstType::Variable(var_type) => {
            // Type variables in AST - could be either:
            // 1. An actual type parameter (T, U, etc.)
            // 2. A reference to a named type without args (Color, Option, etc.)
            // For now, treat uppercase single letters as type params, others as constructors
            if var_type.name.len() == 1 && var_type.name.chars().next().unwrap().is_uppercase() {
                // Likely a type parameter: T, U, etc.
                fresh_type_var(Some(var_type.name.clone()))
            } else {
                // Likely a named type reference: Color, Option, etc.
                // Convert to constructor with empty type args
                InferenceType::Constructor(TConstructor {
                    name: var_type.name.clone(),
                    type_args: vec![],
                })
            }
        }

        // Other AST type variants can be added as needed
        _ => InferenceType::Any, // Fallback for unsupported types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_type_var() {
        reset_type_var_counter();
        let v1 = fresh_type_var(Some("T".to_string()));
        let v2 = fresh_type_var(Some("U".to_string()));

        match (&v1, &v2) {
            (InferenceType::Var(tv1), InferenceType::Var(tv2)) => {
                assert_eq!(tv1.id, 0);
                assert_eq!(tv2.id, 1);
                assert_eq!(tv1.name.as_ref().unwrap(), "T");
                assert_eq!(tv2.name.as_ref().unwrap(), "U");
            }
            _ => panic!("Expected type variables"),
        }
    }

    #[test]
    fn test_types_equal() {
        let int1 = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });
        let int2 = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });
        let bool_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool,
        });

        assert!(types_equal(&int1, &int2));
        assert!(!types_equal(&int1, &bool_type));
    }

    #[test]
    fn test_apply_subst() {
        reset_type_var_counter();
        let var = fresh_type_var(Some("T".to_string()));
        let int_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });

        let mut subst = HashMap::new();
        if let InferenceType::Var(tv) = &var {
            subst.insert(tv.id, int_type.clone());
        }

        let result = apply_subst(&subst, &var);
        assert_eq!(result, int_type);
    }
}
