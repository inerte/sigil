//! Validation error types

use sigil_diagnostics::{codes, Diagnostic, SigilPhase, SourcePoint, SourceSpan};
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

    #[error("SIGIL-CANON-DECL-CATEGORY-ORDER: {message}")]
    DeclarationOrderOld {
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

    #[error("SIGIL-CANON-TEST-PATH: Test declarations only allowed under project tests/ directory\n\nFile: {file_path}")]
    TestPath {
        file_path: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-DECL-EXPORT-ORDER: Declarations with 'export' must come before non-exported declarations\n\nFound non-exported '{prev_name}' before exported '{current_name}'")]
    DeclExportOrder {
        current_name: String,
        prev_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-EXTERN-MEMBER-ORDER: Extern members must be in alphabetical order\n\nFound: {member_name} at position {position}\nAfter: {prev_member}\n\nExpected '{member_name}' to come before '{prev_member}'.")]
    ExternMemberOrder {
        member_name: String,
        prev_member: String,
        position: usize,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-LET-UNTYPED: Let binding '{binding_name}' must have type ascription\n\nUse: l {binding_name}=(value:Type)")]
    LetUntyped {
        binding_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-MATCH-BOOLEAN: Cannot pattern match on boolean expression\n\nUse if-expression instead: (condition)→thenBranch|elseBranch")]
    MatchBoolean {
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-MATCH-TUPLE-BOOLEAN: Cannot pattern match on tuple containing booleans\n\nPattern match discriminates on structure, not boolean values.")]
    MatchTupleBoolean {
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-DECL-CATEGORY-ORDER: Declarations out of category order\n\nExpected: types → externs → imports → consts → functions → tests\nFound: {found_category} after {prev_category}")]
    DeclCategoryOrder {
        found_category: String,
        prev_category: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-DECL-ALPHABETICAL: Declarations within category must be alphabetical\n\nFound: {decl_name} after {prev_name} (both are {category})")]
    DeclAlphabetical {
        decl_name: String,
        prev_name: String,
        category: String,
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
            ValidationError::DeclarationOrderOld { .. } => SourceLocation {
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
            ValidationError::TestPath { location, .. } => *location,
            ValidationError::DeclExportOrder { location, .. } => *location,
            ValidationError::ExternMemberOrder { location, .. } => *location,
            ValidationError::LetUntyped { location, .. } => *location,
            ValidationError::MatchBoolean { location } => *location,
            ValidationError::MatchTupleBoolean { location } => *location,
            ValidationError::DeclCategoryOrder { location, .. } => *location,
            ValidationError::DeclAlphabetical { location, .. } => *location,
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

impl From<ValidationError> for Diagnostic {
    fn from(error: ValidationError) -> Self {
        // Extract filename from error if available, otherwise use placeholder
        let get_file = || "<unknown>".to_string();

        match error {
            ValidationError::DuplicateDeclaration { kind, what, name, location, first_location } => {
                Diagnostic::new(
                    format!("SIGIL-CANON-DUPLICATE-{}", kind.to_uppercase()),
                    SigilPhase::Canonical,
                    format!("Duplicate {} declaration: \"{}\"", what, name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("first_location", format!("{}:{}", first_location.start.line, first_location.start.column))
            }

            ValidationError::ParameterOrder { function_name, param_name, prev_param, position, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::PARAM_ORDER,
                    SigilPhase::Canonical,
                    format!("Parameter '{}' out of alphabetical order in function '{}'", param_name, function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&param_name, &prev_param)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::EffectOrder { function_name, effect_name, prev_effect, position, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::EFFECT_ORDER,
                    SigilPhase::Canonical,
                    format!("Effect '{}' out of alphabetical order in function '{}'", effect_name, function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&effect_name, &prev_effect)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::TestPath { file_path, location } => {
                Diagnostic::new(
                    codes::canonical::TEST_PATH,
                    SigilPhase::Canonical,
                    "Test declarations only allowed under project tests/ directory",
                )
                .with_location(source_location_to_span(file_path.clone(), location))
                .with_details("file_path", &file_path)
            }

            ValidationError::DeclExportOrder { current_name, prev_name, location } => {
                Diagnostic::new(
                    codes::canonical::DECL_EXPORT_ORDER,
                    SigilPhase::Canonical,
                    "Declarations with 'export' must come before non-exported declarations",
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&current_name, &prev_name)
            }

            ValidationError::ExternMemberOrder { member_name, prev_member, position, location } => {
                Diagnostic::new(
                    codes::canonical::EXTERN_MEMBER_ORDER,
                    SigilPhase::Canonical,
                    format!("Extern member '{}' out of alphabetical order", member_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&member_name, &prev_member)
            }

            ValidationError::LetUntyped { binding_name, location } => {
                Diagnostic::new(
                    codes::canonical::LET_UNTYPED,
                    SigilPhase::Canonical,
                    format!("Let binding '{}' must have type ascription", binding_name),
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::MatchBoolean { location } => {
                Diagnostic::new(
                    codes::canonical::MATCH_BOOLEAN,
                    SigilPhase::Canonical,
                    "Cannot pattern match on boolean expression",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::MatchTupleBoolean { location } => {
                Diagnostic::new(
                    codes::canonical::MATCH_TUPLE_BOOLEAN,
                    SigilPhase::Canonical,
                    "Cannot pattern match on tuple containing booleans",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::DeclCategoryOrder { found_category, prev_category, location } => {
                Diagnostic::new(
                    codes::canonical::DECL_CATEGORY_ORDER,
                    SigilPhase::Canonical,
                    "Declarations out of category order",
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&found_category, &prev_category)
            }

            ValidationError::DeclAlphabetical { decl_name, prev_name, category, location } => {
                Diagnostic::new(
                    codes::canonical::DECL_ALPHABETICAL,
                    SigilPhase::Canonical,
                    format!("Declarations within {} category must be alphabetical", category),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&decl_name, &prev_name)
            }

            // Handle other existing errors with generic conversions
            _ => {
                let message = format!("{}", error);
                let code = if message.starts_with("SIGIL-") {
                    message.split_whitespace().next().unwrap_or("SIGIL-CANON-ERROR").to_string()
                } else {
                    "SIGIL-CANON-ERROR".to_string()
                };
                Diagnostic::new(code, SigilPhase::Canonical, message)
                    .with_location(source_location_to_span(get_file(), error.location()))
            }
        }
    }
}
