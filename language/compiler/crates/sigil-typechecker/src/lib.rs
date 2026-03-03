//! Sigil Type Checker - Public API
//!
//! Main entry point for type checking Sigil programs

pub mod environment;
pub mod errors;
pub mod types;
pub mod bidirectional;
pub mod typed_ir;

// Re-export main types
pub use environment::{BindingMeta, TypeEnvironment, TypeInfo};
pub use errors::{format_type, TypeError};
pub use types::{InferenceType, TypeScheme};
pub use typed_ir::{
    PurityClass, StrictnessClass, TypeCheckResult, TypedDeclaration, TypedExpr, TypedExprKind,
    TypedProgram,
};

use sigil_ast::Program;
use std::collections::HashMap;
/// Options for type checking
#[derive(Debug, Clone, Default)]
pub struct TypeCheckOptions {
    pub imported_namespaces: Option<HashMap<String, InferenceType>>,
    pub imported_type_registries: Option<HashMap<String, HashMap<String, TypeInfo>>>,
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
