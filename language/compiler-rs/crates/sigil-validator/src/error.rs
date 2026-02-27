//! Validation error types

use sigil_lexer::SourceLocation;
use thiserror::Error;

/// Validation errors for canonical form violations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ValidationError {
    #[error("SIGIL-CANON-DUPLICATE-{kind}: Duplicate {what} declaration: \"{name}\"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate {what} declaration.")]
    DuplicateDeclaration {
        kind: String,
        what: String,
        name: String,
        location: SourceLocation,
        first_location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-ACCUMULATOR: Accumulator-passing style detected in function '{function_name}'.\n\nThe parameter(s) [{params}] are accumulators (grow during recursion).\nSigil does NOT support tail-call optimization or accumulator-passing style.\n\nUse simple recursion without accumulator parameters.")]
    AccumulatorParameter {
        function_name: String,
        params: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-CPS: Recursive function '{function_name}' returns a function type.\n\nThis is Continuation Passing Style (CPS), which encodes an accumulator in the returned function.\n\nRecursive functions must return a VALUE, not a FUNCTION.")]
    ContinuationPassingStyle {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL: Recursive function '{function_name}' has collection parameter but doesn't use structural recursion.\n\nSigil enforces ONE way: structural recursion for collections.")]
    NonStructuralRecursion {
        function_name: String,
        param_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-PATTERN-REDUNDANT: Redundant pattern in match expression.\n\nPattern already covered by previous patterns.")]
    RedundantPattern { location: SourceLocation },

    #[error("SIGIL-CANON-PATTERN-UNREACHABLE: Unreachable pattern in match expression.\n\nThis pattern will never match.")]
    UnreachablePattern { location: SourceLocation },

    #[error("SIGIL-SURFACE-MISSING-RETURN-TYPE: Missing return type annotation.\n\nAll functions must have explicit return type annotations (canonical form).")]
    MissingReturnType {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-SURFACE-MISSING-PARAM-TYPE: Missing parameter type annotation for '{param_name}'.\n\nAll parameters must have explicit type annotations (canonical form).")]
    MissingParamType {
        param_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-FILE-PURPOSE-NONE: {message}")]
    FilePurposeNone {
        message: String,
    },

    #[error("SIGIL-CANON-FILE-PURPOSE-BOTH: {message}")]
    FilePurposeBoth {
        message: String,
    },

    #[error("SIGIL-CANON-TEST-LOCATION: {message}")]
    TestLocationInvalid {
        message: String,
        file_path: String,
    },

    #[error("SIGIL-CANON-TEST-NO-EXPORTS: {message}")]
    TestNoExports {
        message: String,
    },

    #[error("SIGIL-CANON-LIB-NO-MAIN: {message}")]
    LibNoMain {
        message: String,
    },

    #[error("SIGIL-CANON-EXEC-NEEDS-MAIN: {message}")]
    ExecNeedsMain {
        message: String,
    },

    #[error("SIGIL-CANON-TEST-NEEDS-MAIN: {message}")]
    TestNeedsMain {
        message: String,
    },
}

impl ValidationError {
    pub fn location(&self) -> SourceLocation {
        match self {
            ValidationError::DuplicateDeclaration { location, .. } => *location,
            ValidationError::AccumulatorParameter { location, .. } => *location,
            ValidationError::ContinuationPassingStyle { location, .. } => *location,
            ValidationError::NonStructuralRecursion { location, .. } => *location,
            ValidationError::RedundantPattern { location } => *location,
            ValidationError::UnreachablePattern { location } => *location,
            ValidationError::MissingReturnType { location, .. } => *location,
            ValidationError::MissingParamType { location, .. } => *location,
            ValidationError::FilePurposeNone { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::FilePurposeBoth { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::TestLocationInvalid { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::TestNoExports { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::LibNoMain { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::ExecNeedsMain { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::TestNeedsMain { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
        }
    }
}
