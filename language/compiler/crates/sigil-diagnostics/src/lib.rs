//! Sigil Diagnostics Library
//!
//! Provides unified error reporting infrastructure for the Sigil compiler.
//! This crate defines:
//! - Diagnostic types (error messages with structured metadata)
//! - All 55+ error code constants
//! - Helper functions for creating diagnostics
//! - Human-readable and JSON formatting
//!
//! # Example
//!
//! ```
//! use sigil_diagnostics::{codes, types::{Diagnostic, SigilPhase}, helpers::source_point};
//!
//! let diag = Diagnostic::new(codes::lexer::TAB, SigilPhase::Lexer, "tab characters not allowed");
//! println!("{}", diag.format_human());
//! ```

pub mod codes;
pub mod helpers;
pub mod types;

// Re-export commonly used types
pub use types::{
    CommandEnvelope, Diagnostic, Fixit, SigilPhase, SourcePoint, SourceSpan, Suggestion,
    SymbolTarget,
};
