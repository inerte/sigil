//! Parser error types

use sigil_diagnostics::{codes, Diagnostic, SigilPhase, SourcePoint, SourceSpan};
use sigil_lexer::{SourceLocation, TokenType};
use thiserror::Error;

/// Parser errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("SIGIL-PARSE-UNEXPECTED-TOKEN {file}:{line}:{column} unexpected token (found {found}, expected {expected})")]
    UnexpectedToken {
        file: String,
        expected: String,
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-PARSE-UNEXPECTED-TOKEN {file}:EOF unexpected end of file (expected {expected})")]
    UnexpectedEof {
        file: String,
        expected: String,
    },

    #[error("SIGIL-PARSE-CONST-NAME {file}:{line}:{column} invalid constant name (found {found}, expected lowercase identifier)")]
    InvalidConstantName {
        file: String,
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-PARSE-CONST-UNTYPED {file}:{line}:{column} const value must use type ascription: c {name}=(value:Type)")]
    UntypedConstant {
        file: String,
        name: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-PARSE-NS-SEP {file}:{line}:{column} invalid namespace separator (found {found}, expected ⋅)")]
    InvalidNamespaceSeparator {
        file: String,
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-PARSE-LOCAL-BINDING {file}:{line}:{column} invalid local binding keyword (found {found}, expected l)")]
    InvalidLocalBinding {
        file: String,
        found: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("Cannot export {what} declarations at {file}:{line}:{column}")]
    CannotExport {
        file: String,
        what: String,
        line: usize,
        column: usize,
        location: SourceLocation,
    },

    #[error("Invalid effect: {effect} at {file}:{line}:{column}. Valid effects are: {valid}")]
    InvalidEffect {
        file: String,
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
            ParseError::InvalidConstantName { location, .. } => Some(*location),
            ParseError::UntypedConstant { location, .. } => Some(*location),
            ParseError::InvalidNamespaceSeparator { location, .. } => Some(*location),
            ParseError::InvalidLocalBinding { location, .. } => Some(*location),
            ParseError::CannotExport { location, .. } => Some(*location),
            ParseError::InvalidEffect { location, .. } => Some(*location),
        }
    }
}

/// Convert SourceLocation from lexer to SourceSpan for diagnostics
fn source_location_to_span(file: String, loc: SourceLocation) -> SourceSpan {
    SourceSpan::with_end(
        file,
        SourcePoint::with_offset(loc.start.line, loc.start.column, loc.start.offset),
        SourcePoint::with_offset(loc.end.line, loc.end.column, loc.end.offset),
    )
}

impl From<ParseError> for Diagnostic {
    fn from(error: ParseError) -> Self {
        match error {
            ParseError::UnexpectedToken {
                file,
                expected,
                found,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::UNEXPECTED_TOKEN,
                SigilPhase::Parser,
                "unexpected token",
            )
            .with_location(source_location_to_span(file, location))
            .with_found_expected(&found, &expected),

            ParseError::UnexpectedEof { file, expected } => {
                Diagnostic::new(
                    codes::parser::UNEXPECTED_TOKEN,
                    SigilPhase::Parser,
                    format!("unexpected end of file, expected {}", expected),
                )
                .with_details("file", &file)
                .with_expected(&expected)
            }

            ParseError::InvalidConstantName {
                file,
                found,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::CONST_NAME,
                SigilPhase::Parser,
                "invalid constant name",
            )
            .with_location(source_location_to_span(file, location))
            .with_found_expected(&found, "lowercase identifier"),

            ParseError::UntypedConstant {
                file,
                name,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::CONST_UNTYPED,
                SigilPhase::Parser,
                format!("const value must use type ascription: c {}=(value:Type)", name),
            )
            .with_location(source_location_to_span(file, location))
            .with_details("name", &name),

            ParseError::InvalidNamespaceSeparator {
                file,
                found,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::NS_SEP,
                SigilPhase::Parser,
                "invalid namespace separator",
            )
            .with_location(source_location_to_span(file, location))
            .with_found_expected(&found, "⋅"),

            ParseError::InvalidLocalBinding {
                file,
                found,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::LOCAL_BINDING,
                SigilPhase::Parser,
                "invalid local binding keyword",
            )
            .with_location(source_location_to_span(file, location))
            .with_found_expected(&found, "l"),

            ParseError::CannotExport {
                file,
                what,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::UNEXPECTED_TOKEN,
                SigilPhase::Parser,
                format!("cannot export {} declarations", what),
            )
            .with_location(source_location_to_span(file, location))
            .with_details("what", &what),

            ParseError::InvalidEffect {
                file,
                effect,
                valid,
                line,
                column,
                location,
            } => Diagnostic::new(
                codes::parser::UNEXPECTED_TOKEN,
                SigilPhase::Parser,
                format!("invalid effect: {}. Valid effects are: {}", effect, valid),
            )
            .with_location(source_location_to_span(file, location))
            .with_found_expected(&effect, &valid),
        }
    }
}
