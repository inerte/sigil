//! Sigil Type Checker - Public API
//!
//! Main entry point for type checking Sigil programs

pub mod bidirectional;
pub mod coverage;
pub mod effects;
pub mod environment;
pub mod errors;
pub mod json_codec;
pub mod proof_context;
pub mod typed_ir;
pub mod types;

// Re-export main types
pub use effects::{EffectCatalog, PRIMITIVE_EFFECTS};
pub use environment::{
    BindingMeta, BoundaryRule, BoundaryRuleKind, FunctionContract, LabelInfo, ProtocolSpec,
    TypeEnvironment, TypeInfo,
};
pub use errors::{format_type, TypeError};
pub use typed_ir::{
    PurityClass, StrictnessClass, TypeCheckResult, TypedDeclaration, TypedExpr, TypedExprKind,
    TypedProgram,
};
pub use types::{InferenceType, TypeScheme};

use sigil_ast::Program;
use std::collections::HashMap;
/// Options for type checking
#[derive(Debug, Clone, Default)]
pub struct TypeCheckOptions {
    pub imported_namespaces: Option<HashMap<String, InferenceType>>,
    pub imported_type_registries: Option<HashMap<String, HashMap<String, TypeInfo>>>,
    pub imported_label_registries: Option<HashMap<String, HashMap<String, LabelInfo>>>,
    pub imported_value_schemes: Option<HashMap<String, HashMap<String, TypeScheme>>>,
    pub imported_value_meta: Option<HashMap<String, HashMap<String, BindingMeta>>>,
    pub imported_function_contracts: Option<HashMap<String, HashMap<String, FunctionContract>>>,
    pub imported_protocol_registries: Option<HashMap<String, HashMap<String, ProtocolSpec>>>,
    pub boundary_rules: Option<Vec<BoundaryRule>>,
    pub effect_catalog: Option<EffectCatalog>,
    pub module_id: Option<String>,
    pub source_file: Option<String>,
}

/// Type check a Sigil program
///
/// Returns inferred declaration types together with the typed semantic IR.
/// Returns TypeError if type checking fails
pub fn type_check(
    program: &Program,
    source_code: &str,
    options: Option<TypeCheckOptions>,
) -> Result<TypeCheckResult, TypeError> {
    bidirectional::type_check(program, source_code, options.unwrap_or_default())
}
