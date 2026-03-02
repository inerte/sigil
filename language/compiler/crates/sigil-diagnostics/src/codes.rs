//! Error code constants for all Sigil compiler errors
//!
//! All error codes follow the format: SIGIL-{PHASE}-{ERROR}
//! This module provides constants for all 55+ error codes across all compiler phases.

/// Lexer error codes (SIGIL-LEX-*)
pub mod lexer {
    pub const TAB: &str = "SIGIL-LEX-TAB";
    pub const CRLF: &str = "SIGIL-LEX-CRLF";
    pub const UNTERMINATED_STRING: &str = "SIGIL-LEX-UNTERMINATED-STRING";
    pub const UNTERMINATED_COMMENT: &str = "SIGIL-LEX-UNTERMINATED-COMMENT";
    pub const EMPTY_CHAR: &str = "SIGIL-LEX-EMPTY-CHAR";
    pub const CHAR_LENGTH: &str = "SIGIL-LEX-CHAR-LENGTH";
    pub const UNTERMINATED_CHAR: &str = "SIGIL-LEX-UNTERMINATED-CHAR";
    pub const INVALID_ESCAPE: &str = "SIGIL-LEX-INVALID-ESCAPE";
    pub const UNEXPECTED_CHAR: &str = "SIGIL-LEX-UNEXPECTED-CHAR";
}

/// Parser error codes (SIGIL-PARSE-*)
pub mod parser {
    pub const CONST_NAME: &str = "SIGIL-PARSE-CONST-NAME";
    pub const CONST_UNTYPED: &str = "SIGIL-PARSE-CONST-UNTYPED";
    pub const NS_SEP: &str = "SIGIL-PARSE-NS-SEP";
    pub const LOCAL_BINDING: &str = "SIGIL-PARSE-LOCAL-BINDING";
    pub const UNEXPECTED_TOKEN: &str = "SIGIL-PARSE-UNEXPECTED-TOKEN";
}

/// Canonical form validation error codes (SIGIL-CANON-*)
pub mod canonical {
    // Duplicate declarations
    pub const DUPLICATE_TYPE: &str = "SIGIL-CANON-DUPLICATE-TYPE";
    pub const DUPLICATE_EXTERN: &str = "SIGIL-CANON-DUPLICATE-EXTERN";
    pub const DUPLICATE_IMPORT: &str = "SIGIL-CANON-DUPLICATE-IMPORT";
    pub const DUPLICATE_CONST: &str = "SIGIL-CANON-DUPLICATE-CONST";
    pub const DUPLICATE_FUNCTION: &str = "SIGIL-CANON-DUPLICATE-FUNCTION";
    pub const DUPLICATE_TEST: &str = "SIGIL-CANON-DUPLICATE-TEST";

    // File formatting
    pub const EOF_NEWLINE: &str = "SIGIL-CANON-EOF-NEWLINE";
    pub const TRAILING_WHITESPACE: &str = "SIGIL-CANON-TRAILING-WHITESPACE";
    pub const BLANK_LINES: &str = "SIGIL-CANON-BLANK-LINES";

    // File purpose constraints
    pub const LIB_NO_MAIN: &str = "SIGIL-CANON-LIB-NO-MAIN";
    pub const EXEC_NEEDS_MAIN: &str = "SIGIL-CANON-EXEC-NEEDS-MAIN";
    pub const TEST_NEEDS_MAIN: &str = "SIGIL-CANON-TEST-NEEDS-MAIN";
    pub const TEST_LOCATION: &str = "SIGIL-CANON-TEST-LOCATION";
    pub const TEST_PATH: &str = "SIGIL-CANON-TEST-PATH";

    // Filename conventions
    pub const FILENAME_CASE: &str = "SIGIL-CANON-FILENAME-CASE";
    pub const FILENAME_INVALID_CHAR: &str = "SIGIL-CANON-FILENAME-INVALID-CHAR";
    pub const FILENAME_FORMAT: &str = "SIGIL-CANON-FILENAME-FORMAT";

    // Recursion patterns
    pub const RECURSION_ACCUMULATOR: &str = "SIGIL-CANON-RECURSION-ACCUMULATOR";
    pub const RECURSION_COLLECTION_NONSTRUCTURAL: &str =
        "SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL";
    pub const RECURSION_CPS: &str = "SIGIL-CANON-RECURSION-CPS";

    // Pattern matching
    pub const MATCH_BOOLEAN: &str = "SIGIL-CANON-MATCH-BOOLEAN";
    pub const MATCH_TUPLE_BOOLEAN: &str = "SIGIL-CANON-MATCH-TUPLE-BOOLEAN";

    // Parameter and effect ordering
    pub const PARAM_ORDER: &str = "SIGIL-CANON-PARAM-ORDER";
    pub const EFFECT_ORDER: &str = "SIGIL-CANON-EFFECT-ORDER";

    // Local bindings
    pub const LET_UNTYPED: &str = "SIGIL-CANON-LET-UNTYPED";

    // Declaration ordering
    pub const DECL_CATEGORY_ORDER: &str = "SIGIL-CANON-DECL-CATEGORY-ORDER";
    pub const DECL_EXPORT_ORDER: &str = "SIGIL-CANON-DECL-EXPORT-ORDER";
    pub const DECL_ALPHABETICAL: &str = "SIGIL-CANON-DECL-ALPHABETICAL";

    // Extern member ordering
    pub const EXTERN_MEMBER_ORDER: &str = "SIGIL-CANON-EXTERN-MEMBER-ORDER";
}

/// Type checker error codes (SIGIL-TYPE-*)
pub mod typecheck {
    pub const ERROR: &str = "SIGIL-TYPE-ERROR";
    pub const MODULE_NOT_EXPORTED: &str = "SIGIL-TYPE-MODULE-NOT-EXPORTED";
}

/// Mutability analysis error codes (SIGIL-MUTABILITY-*)
pub mod mutability {
    pub const INVALID: &str = "SIGIL-MUTABILITY-INVALID";
}

/// CLI error codes (SIGIL-CLI-*)
pub mod cli {
    pub const USAGE: &str = "SIGIL-CLI-USAGE";
    pub const UNKNOWN_COMMAND: &str = "SIGIL-CLI-UNKNOWN-COMMAND";
    pub const UNSUPPORTED_OPTION: &str = "SIGIL-CLI-UNSUPPORTED-OPTION";
    pub const UNEXPECTED: &str = "SIGIL-CLI-UNEXPECTED";
    pub const IMPORT_NOT_FOUND: &str = "SIGIL-CLI-IMPORT-NOT-FOUND";
    pub const IMPORT_CYCLE: &str = "SIGIL-CLI-IMPORT-CYCLE";
    pub const INVALID_IMPORT: &str = "SIGIL-CLI-INVALID-IMPORT";
    pub const PROJECT_ROOT_REQUIRED: &str = "SIGIL-CLI-PROJECT-ROOT-REQUIRED";
}

/// Runtime error codes (SIGIL-RUNTIME-*, SIGIL-RUN-*)
pub mod runtime {
    pub const CHILD_EXIT: &str = "SIGIL-RUNTIME-CHILD-EXIT";
    pub const ENGINE_NOT_FOUND: &str = "SIGIL-RUN-ENGINE-NOT-FOUND";
}

/// All error codes in one flat list (for documentation and testing)
pub const ALL_ERROR_CODES: &[&str] = &[
    // Lexer (9 codes)
    lexer::TAB,
    lexer::CRLF,
    lexer::UNTERMINATED_STRING,
    lexer::UNTERMINATED_COMMENT,
    lexer::EMPTY_CHAR,
    lexer::CHAR_LENGTH,
    lexer::UNTERMINATED_CHAR,
    lexer::INVALID_ESCAPE,
    lexer::UNEXPECTED_CHAR,
    // Parser (5 codes)
    parser::CONST_NAME,
    parser::CONST_UNTYPED,
    parser::NS_SEP,
    parser::LOCAL_BINDING,
    parser::UNEXPECTED_TOKEN,
    // Canonical (28 codes)
    canonical::DUPLICATE_TYPE,
    canonical::DUPLICATE_EXTERN,
    canonical::DUPLICATE_IMPORT,
    canonical::DUPLICATE_CONST,
    canonical::DUPLICATE_FUNCTION,
    canonical::DUPLICATE_TEST,
    canonical::EOF_NEWLINE,
    canonical::TRAILING_WHITESPACE,
    canonical::BLANK_LINES,
    canonical::LIB_NO_MAIN,
    canonical::EXEC_NEEDS_MAIN,
    canonical::TEST_NEEDS_MAIN,
    canonical::TEST_LOCATION,
    canonical::TEST_PATH,
    canonical::FILENAME_CASE,
    canonical::FILENAME_INVALID_CHAR,
    canonical::FILENAME_FORMAT,
    canonical::RECURSION_ACCUMULATOR,
    canonical::RECURSION_COLLECTION_NONSTRUCTURAL,
    canonical::RECURSION_CPS,
    canonical::MATCH_BOOLEAN,
    canonical::MATCH_TUPLE_BOOLEAN,
    canonical::PARAM_ORDER,
    canonical::EFFECT_ORDER,
    canonical::LET_UNTYPED,
    canonical::DECL_CATEGORY_ORDER,
    canonical::DECL_EXPORT_ORDER,
    canonical::DECL_ALPHABETICAL,
    canonical::EXTERN_MEMBER_ORDER,
    // Typecheck (2 codes)
    typecheck::ERROR,
    typecheck::MODULE_NOT_EXPORTED,
    // Mutability (1 code)
    mutability::INVALID,
    // CLI (8 codes)
    cli::USAGE,
    cli::UNKNOWN_COMMAND,
    cli::UNSUPPORTED_OPTION,
    cli::UNEXPECTED,
    cli::IMPORT_NOT_FOUND,
    cli::IMPORT_CYCLE,
    cli::INVALID_IMPORT,
    cli::PROJECT_ROOT_REQUIRED,
    // Runtime (2 codes)
    runtime::CHILD_EXIT,
    runtime::ENGINE_NOT_FOUND,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_codes_start_with_sigil() {
        for code in ALL_ERROR_CODES {
            assert!(
                code.starts_with("SIGIL-"),
                "Error code '{}' doesn't start with SIGIL-",
                code
            );
        }
    }

    #[test]
    fn all_codes_are_unique() {
        use std::collections::HashSet;
        let set: HashSet<&str> = ALL_ERROR_CODES.iter().copied().collect();
        assert_eq!(
            set.len(),
            ALL_ERROR_CODES.len(),
            "Duplicate error codes found"
        );
    }

    #[test]
    fn count_error_codes() {
        // As of implementation: 56 error codes total
        // 9 lexer + 5 parser + 29 canonical + 2 typecheck + 1 mutability + 8 CLI + 2 runtime = 56
        // (canonical has 29 because both TEST_LOCATION and TEST_PATH exist)
        assert_eq!(
            ALL_ERROR_CODES.len(),
            56,
            "Expected 56 error codes, found {}",
            ALL_ERROR_CODES.len()
        );
    }
}
