//! Lexer for the Sigil programming language
//!
//! Tokenizes Sigil source code with canonical formatting enforcement.
//! The lexer enforces formatting rules at tokenization time - incorrectly
//! formatted code produces lexical errors.
//!
//! # Design Principles
//!
//! - **Canonical Formatting**: No tabs, precise whitespace requirements
//! - **Unicode Symbols**: λ, →, ≡, ⋅, ∧, ∨, ¬, ≤, ≥, ≠, etc.
//! - **Deterministic**: Same input always produces same token stream
//! - **Error Recovery**: Clear error messages with source locations

pub mod token;
mod lexer_impl;

pub use token::*;
pub use lexer_impl::*;
