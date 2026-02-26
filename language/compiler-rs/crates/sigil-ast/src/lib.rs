//! Abstract Syntax Tree (AST) definitions for the Sigil programming language
//!
//! This crate provides the core AST node types that represent parsed Sigil programs.
//! The AST is the intermediate representation between lexing and type checking.
//!
//! # Design Principles
//!
//! - **Direct TypeScript Translation**: Maintains exact structural parity with the
//!   TypeScript AST implementation in `/language/compiler/src/parser/ast.ts`
//! - **Immutable by Default**: All AST nodes are immutable once constructed
//! - **Location Tracking**: Every AST node includes source location information for
//!   precise error reporting and diagnostics
//! - **Type-Safe Enums**: Uses Rust enums for sum types (Declaration, Expr, Type, etc.)

pub mod declarations;
pub mod types;
pub mod expressions;
pub mod patterns;

// Re-export location types from sigil-lexer
pub use sigil_lexer::{Position, SourceLocation};

pub use declarations::*;
pub use types::*;
pub use expressions::*;
pub use patterns::*;

/// Top-level program representation
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Program {
    pub declarations: Vec<Declaration>,
    pub location: SourceLocation,
}

impl Program {
    pub fn new(declarations: Vec<Declaration>, location: SourceLocation) -> Self {
        Self { declarations, location }
    }
}
