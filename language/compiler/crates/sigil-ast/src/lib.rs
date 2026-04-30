//! Abstract Syntax Tree (AST) definitions for the Sigil programming language
//!
//! This crate provides the core AST node types that represent parsed Sigil programs.
//! The AST is the intermediate representation between lexing and type checking.
//!
//! # Design Principles
//!
//! - **Direct Frontend Representation**: Mirrors the compiler's parsed Sigil syntax
//! - **Immutable by Default**: All AST nodes are immutable once constructed
//! - **Location Tracking**: Every AST node includes source location information for
//!   precise error reporting and diagnostics
//! - **Type-Safe Enums**: Uses Rust enums for sum types (Declaration, Expr, Type, etc.)

pub mod declarations;
pub mod expressions;
pub mod patterns;
pub mod types;

// Re-export location types from sigil-lexer
pub use sigil_lexer::{Position, SourceLocation};

pub use declarations::*;
pub use expressions::*;
pub use patterns::*;
pub use types::*;

/// Top-level program representation
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Program {
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    #[cfg_attr(feature = "serde", serde(default = "program_type_default"))]
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    r#type: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub default_function_mode: FunctionMode,
    pub declarations: Vec<Declaration>,
    pub location: SourceLocation,
}

fn program_type_default() -> String {
    "Program".to_string()
}

impl Program {
    pub fn new(
        declarations: Vec<Declaration>,
        location: SourceLocation,
        default_function_mode: FunctionMode,
    ) -> Self {
        Self {
            r#type: "Program".to_string(),
            default_function_mode,
            declarations,
            location,
        }
    }
}
