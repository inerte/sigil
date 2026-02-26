//! Parser for the Sigil programming language
//!
//! This module provides a recursive descent parser that constructs an AST
//! from a stream of tokens produced by the lexer.
//!
//! # Design Principles
//!
//! - **Recursive Descent**: Simple, maintainable parser structure
//! - **Exact TypeScript Compatibility**: Matches TypeScript parser behavior
//! - **Error Recovery**: Clear error messages with source locations
//! - **Canonical Form**: Only accepts canonical Sigil syntax

mod parser_impl;
pub mod error;

pub use parser_impl::*;
pub use error::*;
