//! Parser error types

use sigil_lexer::{SourceLocation, TokenType};
use thiserror::Error;

/// Parser errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("Expected {expected} at {line}:{column}, found {found}")]
    UnexpectedToken {
        expected: String,
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("Unexpected end of file, expected {expected}")]
    UnexpectedEof { expected: String },

    #[error("{message} at {line}:{column}")]
    Generic {
        message: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-PARSE-CONST-NAME: invalid constant name at {line}:{column}. Found {found}, expected lowercase identifier")]
    InvalidConstantName {
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-PARSE-NS-SEP: invalid namespace separator at {line}:{column}. Found {found}, expected ⋅")]
    InvalidNamespaceSeparator {
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("Cannot export {what} declarations at {line}:{column}")]
    CannotExport {
        what: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("Invalid effect: {effect} at {line}:{column}. Valid effects are: {valid}")]
    InvalidEffect {
        effect: String,
        valid: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },
}

impl ParseError {
    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            ParseError::UnexpectedToken { location, .. } => Some(*location),
            ParseError::UnexpectedEof { .. } => None,
            ParseError::Generic { location, .. } => Some(*location),
            ParseError::InvalidConstantName { location, .. } => Some(*location),
            ParseError::InvalidNamespaceSeparator { location, .. } => Some(*location),
            ParseError::CannotExport { location, .. } => Some(*location),
            ParseError::InvalidEffect { location, .. } => Some(*location),
        }
    }
}
