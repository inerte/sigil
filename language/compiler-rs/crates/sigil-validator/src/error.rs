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

    #[error("{message}")]
    DeclarationOrder {
        message: String,
    },

    #[error("SIGIL-CANON-FILENAME-CASE: Filenames must be lowercase\n\nFile: {filename}\nFound uppercase in: {basename}\nRename to: {suggested}\n\nSigil enforces ONE way: lowercase filenames with hyphens for word separation.")]
    FilenameCase {
        filename: String,
        basename: String,
        suggested: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-FILENAME-INVALID-CHAR: Filenames cannot contain {invalid_char}\n\nFile: {filename}\nFound in: {basename}\nRename to: {suggested}\n\nSigil enforces ONE way: use hyphens (-) not underscores (_) for word separation.")]
    FilenameInvalidChar {
        filename: String,
        basename: String,
        suggested: String,
        invalid_char: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-FILENAME-FORMAT: {message}\n\nFile: {filename}")]
    FilenameFormat {
        filename: String,
        message: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-EOF-NEWLINE: file must end with newline\n\nFile: {filename}")]
    EOFNewline {
        filename: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-TRAILING-WHITESPACE: trailing whitespace\n\nFile: {filename}\nLine: {line}")]
    TrailingWhitespace {
        filename: String,
        line: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-BLANK-LINES: multiple consecutive blank lines\n\nFile: {filename}\nLine: {line}")]
    BlankLines {
        filename: String,
        line: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-PARAM-ORDER: Parameter out of alphabetical order in function '{function_name}'\n\nFound: {param_name} at position {position}\nAfter: {prev_param}\n\nParameters must be alphabetically ordered.\nExpected '{param_name}' to come before '{prev_param}'.\n\nCorrect order: {expected_order:?}\n\nSigil enforces ONE WAY: canonical parameter ordering.")]
    ParameterOrder {
        function_name: String,
        param_name: String,
        prev_param: String,
        position: usize,
        expected_order: Vec<String>,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-EFFECT-ORDER: Effect out of alphabetical order in function '{function_name}'\n\nFound: !{effect_name} at position {position}\nAfter: !{prev_effect}\n\nEffects must be alphabetically ordered.\nExpected '{effect_name}' to come before '{prev_effect}'.\n\nCorrect order: {expected_order:?}\n\nSigil enforces ONE WAY: canonical effect ordering.")]
    EffectOrder {
        function_name: String,
        effect_name: String,
        prev_effect: String,
        position: usize,
        expected_order: Vec<String>,
        location: SourceLocation,
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
            ValidationError::DeclarationOrder { .. } => SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
            ValidationError::FilenameCase { location, .. } => *location,
            ValidationError::FilenameInvalidChar { location, .. } => *location,
            ValidationError::FilenameFormat { location, .. } => *location,
            ValidationError::EOFNewline { location, .. } => *location,
            ValidationError::TrailingWhitespace { location, .. } => *location,
            ValidationError::BlankLines { location, .. } => *location,
            ValidationError::ParameterOrder { location, .. } => *location,
            ValidationError::EffectOrder { location, .. } => *location,
        }
    }
}
