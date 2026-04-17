//! Sigil Type Checker - Core Type System
//!
//! Internal type representations used during type checking.
//! These are distinct from AST types and optimized for unification and substitution.

use sigil_ast::{PrimitiveName, Type as AstType};
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
    Map(Box<TMap>),
    Tuple(TTuple),
    Record(TRecord),
    Constructor(TConstructor),
    Owned(Box<InferenceType>),
    Borrowed(Box<TBorrowed>),
    Any, // For FFI namespaces - trust mode, validated at link-time
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TPrimitive {
    pub name: PrimitiveName,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TVar {
    pub id: u32,                         // Unique type variable ID
    pub name: Option<String>,            // Optional name for display (α, β, T, U)
    pub instance: Option<InferenceType>, // For unification - points to actual type when unified
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
pub struct TMap {
    pub key_type: InferenceType,
    pub value_type: InferenceType,
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
    pub name: String,                  // e.g., "Option", "Result", "Maybe"
    pub type_args: Vec<InferenceType>, // Generic type arguments
}

#[derive(Debug, Clone, PartialEq)]
pub struct TBorrowed {
    pub resource_type: InferenceType,
    pub scope_id: u32,
}

// ============================================================================
// TYPE SCHEMES (Polymorphism)
// ============================================================================

/// Type scheme for polymorphic types (∀α₁...αₙ.τ)
///
/// Example:
///   identity : ∀T. T => T
///   map : ∀T,U. (T => U) => [T] => [U]
#[derive(Debug, Clone, PartialEq)]
pub struct TypeScheme {
    pub quantified_vars: HashSet<u32>, // Type variable IDs that are quantified
    pub typ: InferenceType,            // The actual type with quantified variables
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
pub type EffectSet = HashSet<String>; // {'IO', 'Network', 'Error', 'Mut'}

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

        InferenceType::Function(tfunc) => InferenceType::Function(Box::new(TFunction {
            params: tfunc.params.iter().map(|p| apply_subst(subst, p)).collect(),
            return_type: apply_subst(subst, &tfunc.return_type),
            effects: tfunc.effects.clone(),
        })),

        InferenceType::List(tlist) => InferenceType::List(Box::new(TList {
            element_type: apply_subst(subst, &tlist.element_type),
        })),

        InferenceType::Map(tmap) => InferenceType::Map(Box::new(TMap {
            key_type: apply_subst(subst, &tmap.key_type),
            value_type: apply_subst(subst, &tmap.value_type),
        })),

        InferenceType::Tuple(ttuple) => InferenceType::Tuple(TTuple {
            types: ttuple.types.iter().map(|t| apply_subst(subst, t)).collect(),
        }),

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

        InferenceType::Constructor(tcons) => InferenceType::Constructor(TConstructor {
            name: tcons.name.clone(),
            type_args: tcons
                .type_args
                .iter()
                .map(|arg| apply_subst(subst, arg))
                .collect(),
        }),

        InferenceType::Owned(inner) => InferenceType::Owned(Box::new(apply_subst(subst, inner))),

        InferenceType::Borrowed(borrowed) => InferenceType::Borrowed(Box::new(TBorrowed {
            resource_type: apply_subst(subst, &borrowed.resource_type),
            scope_id: borrowed.scope_id,
        })),

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
        InferenceType::Owned(inner) => InferenceType::Owned(Box::new(prune(inner))),
        InferenceType::Borrowed(borrowed) => InferenceType::Borrowed(Box::new(TBorrowed {
            resource_type: prune(&borrowed.resource_type),
            scope_id: borrowed.scope_id,
        })),
        _ => typ.clone(),
    }
}

/// Unify two types, producing a substitution when they are compatible.
pub fn unify(left: &InferenceType, right: &InferenceType) -> Result<Substitution, String> {
    let mut subst = Substitution::new();
    unify_into(left, right, &mut subst)?;
    Ok(subst)
}

fn bind_var(tvar: &TVar, typ: &InferenceType, subst: &mut Substitution) -> Result<(), String> {
    if let Some(existing) = subst.get(&tvar.id).cloned() {
        return unify_into(&existing, typ, subst);
    }

    let pruned = prune(typ);
    if let InferenceType::Var(other) = &pruned {
        if other.id == tvar.id {
            return Ok(());
        }
    }

    if occurs_in(tvar.id, &pruned, subst) {
        return Err("Recursive type detected during unification".to_string());
    }

    subst.insert(tvar.id, pruned);
    Ok(())
}

fn occurs_in(id: u32, typ: &InferenceType, subst: &Substitution) -> bool {
    let typ = apply_subst(subst, typ);
    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => false,
        InferenceType::Var(tvar) => tvar.id == id,
        InferenceType::Function(func) => {
            func.params.iter().any(|param| occurs_in(id, param, subst))
                || occurs_in(id, &func.return_type, subst)
        }
        InferenceType::List(list) => occurs_in(id, &list.element_type, subst),
        InferenceType::Map(map) => {
            occurs_in(id, &map.key_type, subst) || occurs_in(id, &map.value_type, subst)
        }
        InferenceType::Tuple(tuple) => tuple.types.iter().any(|item| occurs_in(id, item, subst)),
        InferenceType::Record(record) => record
            .fields
            .values()
            .any(|field_type| occurs_in(id, field_type, subst)),
        InferenceType::Constructor(constructor) => constructor
            .type_args
            .iter()
            .any(|arg| occurs_in(id, arg, subst)),
        InferenceType::Owned(inner) => occurs_in(id, &inner, subst),
        InferenceType::Borrowed(borrowed) => occurs_in(id, &borrowed.resource_type, subst),
    }
}

fn unify_into(
    left: &InferenceType,
    right: &InferenceType,
    subst: &mut Substitution,
) -> Result<(), String> {
    let left = apply_subst(subst, left);
    let right = apply_subst(subst, right);

    match (&left, &right) {
        (InferenceType::Any, _) | (_, InferenceType::Any) => Ok(()),
        (InferenceType::Var(tvar), other) => bind_var(tvar, other, subst),
        (other, InferenceType::Var(tvar)) => bind_var(tvar, other, subst),
        (InferenceType::Primitive(p1), InferenceType::Primitive(p2)) if p1.name == p2.name => {
            Ok(())
        }
        (InferenceType::List(l1), InferenceType::List(l2)) => {
            unify_into(&l1.element_type, &l2.element_type, subst)
        }
        (InferenceType::Map(m1), InferenceType::Map(m2)) => {
            unify_into(&m1.key_type, &m2.key_type, subst)?;
            unify_into(&m1.value_type, &m2.value_type, subst)
        }
        (InferenceType::Tuple(t1), InferenceType::Tuple(t2))
            if t1.types.len() == t2.types.len() =>
        {
            for (left_item, right_item) in t1.types.iter().zip(&t2.types) {
                unify_into(left_item, right_item, subst)?;
            }
            Ok(())
        }
        (InferenceType::Function(f1), InferenceType::Function(f2))
            if f1.params.len() == f2.params.len() =>
        {
            for (left_param, right_param) in f1.params.iter().zip(&f2.params) {
                unify_into(left_param, right_param, subst)?;
            }
            unify_into(&f1.return_type, &f2.return_type, subst)
        }
        (InferenceType::Record(r1), InferenceType::Record(r2))
            if r1.fields.len() == r2.fields.len() =>
        {
            for (field_name, left_field) in &r1.fields {
                let right_field = r2
                    .fields
                    .get(field_name)
                    .ok_or_else(|| format!("Missing record field '{}'", field_name))?;
                unify_into(left_field, right_field, subst)?;
            }
            Ok(())
        }
        (InferenceType::Owned(inner1), InferenceType::Owned(inner2)) => {
            unify_into(inner1, inner2, subst)
        }
        (InferenceType::Borrowed(borrowed), other) => {
            unify_into(&borrowed.resource_type, other, subst)
        }
        (other, InferenceType::Borrowed(borrowed)) => {
            unify_into(other, &borrowed.resource_type, subst)
        }
        (InferenceType::Constructor(c1), InferenceType::Constructor(c2))
            if c1.name == c2.name && c1.type_args.len() == c2.type_args.len() =>
        {
            for (left_arg, right_arg) in c1.type_args.iter().zip(&c2.type_args) {
                unify_into(left_arg, right_arg, subst)?;
            }
            Ok(())
        }
        _ => Err(format!(
            "Cannot unify {} with {}",
            format_inference_type(&left),
            format_inference_type(&right)
        )),
    }
}

fn format_inference_type(typ: &InferenceType) -> String {
    match typ {
        InferenceType::Primitive(p) => p.name.to_string(),
        InferenceType::Var(tvar) => tvar.name.clone().unwrap_or_else(|| format!("t{}", tvar.id)),
        InferenceType::Function(func) => {
            let params = func
                .params
                .iter()
                .map(format_inference_type)
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "λ({})=>{}",
                params,
                format_inference_type(&func.return_type)
            )
        }
        InferenceType::List(list) => format!("[{}]", format_inference_type(&list.element_type)),
        InferenceType::Map(map) => format!(
            "{{{}↦{}}}",
            format_inference_type(&map.key_type),
            format_inference_type(&map.value_type)
        ),
        InferenceType::Tuple(tuple) => format!(
            "({})",
            tuple
                .types
                .iter()
                .map(format_inference_type)
                .collect::<Vec<_>>()
                .join(",")
        ),
        InferenceType::Record(record) => {
            let mut items = record
                .fields
                .iter()
                .map(|(name, typ)| format!("{}:{}", name, format_inference_type(typ)))
                .collect::<Vec<_>>();
            items.sort();
            format!("{{{}}}", items.join(","))
        }
        InferenceType::Constructor(constructor) => {
            if constructor.type_args.is_empty() {
                constructor.name.clone()
            } else {
                format!(
                    "{}[{}]",
                    constructor.name,
                    constructor
                        .type_args
                        .iter()
                        .map(format_inference_type)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
        }
        InferenceType::Owned(inner) => format!("Owned[{}]", format_inference_type(inner)),
        InferenceType::Borrowed(borrowed) => format_inference_type(&borrowed.resource_type),
        InferenceType::Any => "Any".to_string(),
    }
}

/// Check if two already-canonicalized types are structurally equal.
///
/// This function does not resolve named aliases or named product types by itself.
/// Checker call sites must normalize both sides first when comparing user-facing
/// types for compatibility.
pub fn types_equal(t1: &InferenceType, t2: &InferenceType) -> bool {
    let t1 = prune(t1);
    let t2 = prune(t2);

    match (&t1, &t2) {
        (InferenceType::Primitive(p1), InferenceType::Primitive(p2)) => p1.name == p2.name,

        (InferenceType::Var(v1), InferenceType::Var(v2)) => v1.id == v2.id,

        (InferenceType::Function(f1), InferenceType::Function(f2)) => {
            f1.params.len() == f2.params.len()
                && f1
                    .params
                    .iter()
                    .zip(&f2.params)
                    .all(|(p1, p2)| types_equal(p1, p2))
                && types_equal(&f1.return_type, &f2.return_type)
        }

        (InferenceType::List(l1), InferenceType::List(l2)) => {
            types_equal(&l1.element_type, &l2.element_type)
        }

        (InferenceType::Map(m1), InferenceType::Map(m2)) => {
            types_equal(&m1.key_type, &m2.key_type) && types_equal(&m1.value_type, &m2.value_type)
        }

        (InferenceType::Tuple(t1), InferenceType::Tuple(t2)) => {
            t1.types.len() == t2.types.len()
                && t1
                    .types
                    .iter()
                    .zip(&t2.types)
                    .all(|(ty1, ty2)| types_equal(ty1, ty2))
        }

        (InferenceType::Record(r1), InferenceType::Record(r2)) => {
            r1.fields.len() == r2.fields.len()
                && r1.fields.iter().all(|(name, ty1)| {
                    r2.fields
                        .get(name)
                        .map_or(false, |ty2| types_equal(ty1, ty2))
                })
        }

        (InferenceType::Owned(inner1), InferenceType::Owned(inner2)) => types_equal(inner1, inner2),

        (InferenceType::Borrowed(borrowed), other) => types_equal(&borrowed.resource_type, other),

        (other, InferenceType::Borrowed(borrowed)) => types_equal(other, &borrowed.resource_type),

        (InferenceType::Constructor(c1), InferenceType::Constructor(c2)) => {
            c1.name == c2.name
                && c1.type_args.len() == c2.type_args.len()
                && c1
                    .type_args
                    .iter()
                    .zip(&c2.type_args)
                    .all(|(a1, a2)| types_equal(a1, a2))
        }

        (InferenceType::Any, _) | (_, InferenceType::Any) => true,

        _ => false,
    }
}

/// Convert AST type to InferenceType
///
/// Used when type annotations are present in the source
pub fn ast_type_to_inference_type(ast_type: &AstType) -> InferenceType {
    ast_type_to_inference_type_with_params(ast_type, None)
}

/// Convert AST type to InferenceType using an explicit type-parameter environment.
pub fn ast_type_to_inference_type_with_params(
    ast_type: &AstType,
    type_params: Option<&HashMap<String, InferenceType>>,
) -> InferenceType {
    match ast_type {
        AstType::Primitive(p) => InferenceType::Primitive(TPrimitive { name: p.name }),

        AstType::List(list_type) => InferenceType::List(Box::new(TList {
            element_type: ast_type_to_inference_type_with_params(
                &list_type.element_type,
                type_params,
            ),
        })),

        AstType::Map(map_type) => InferenceType::Map(Box::new(TMap {
            key_type: ast_type_to_inference_type_with_params(&map_type.key_type, type_params),
            value_type: ast_type_to_inference_type_with_params(&map_type.value_type, type_params),
        })),

        AstType::Tuple(tuple_type) => InferenceType::Tuple(TTuple {
            types: tuple_type
                .types
                .iter()
                .map(|item| ast_type_to_inference_type_with_params(item, type_params))
                .collect(),
        }),

        AstType::Function(func_type) => InferenceType::Function(Box::new(TFunction {
            params: func_type
                .param_types
                .iter()
                .map(|item| ast_type_to_inference_type_with_params(item, type_params))
                .collect(),
            return_type: ast_type_to_inference_type_with_params(
                &func_type.return_type,
                type_params,
            ),
            effects: if func_type.effects.is_empty() {
                None
            } else {
                Some(func_type.effects.iter().cloned().collect())
            },
        })),

        AstType::Constructor(tc) => {
            let type_args = tc
                .type_args
                .iter()
                .map(|item| ast_type_to_inference_type_with_params(item, type_params))
                .collect::<Vec<_>>();
            if tc.name == "Owned" && type_args.len() == 1 {
                InferenceType::Owned(Box::new(type_args[0].clone()))
            } else {
                InferenceType::Constructor(TConstructor {
                    name: tc.name.clone(),
                    type_args,
                })
            }
        }

        AstType::Qualified(qualified) => InferenceType::Constructor(TConstructor {
            name: if qualified.module_path.is_empty() {
                qualified.type_name.clone()
            } else {
                format!(
                    "{}.{}",
                    qualified.module_path.join("::"),
                    qualified.type_name
                )
            },
            type_args: qualified
                .type_args
                .iter()
                .map(|item| ast_type_to_inference_type_with_params(item, type_params))
                .collect(),
        }),

        AstType::Variable(var_type) => {
            if let Some(type_param_env) = type_params {
                if let Some(bound_type) = type_param_env.get(&var_type.name) {
                    return bound_type.clone();
                }
            }

            InferenceType::Constructor(TConstructor {
                name: var_type.name.clone(),
                type_args: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_type_var() {
        let v1 = fresh_type_var(Some("T".to_string()));
        let v2 = fresh_type_var(Some("U".to_string()));

        match (&v1, &v2) {
            (InferenceType::Var(tv1), InferenceType::Var(tv2)) => {
                assert!(tv1.id < tv2.id);
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
