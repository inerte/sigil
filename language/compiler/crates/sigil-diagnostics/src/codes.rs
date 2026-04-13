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
    pub const LEGACY_BOOL: &str = "SIGIL-LEX-LEGACY-BOOL";
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
    pub const SOURCE_FORM: &str = "SIGIL-CANON-SOURCE-FORM";
    pub const DELIMITER_SPACING: &str = "SIGIL-CANON-DELIMITER-SPACING";
    pub const OPERATOR_SPACING: &str = "SIGIL-CANON-OPERATOR-SPACING";
    pub const MATCH_LAYOUT: &str = "SIGIL-CANON-MATCH-LAYOUT";
    pub const MATCH_ARM_LAYOUT: &str = "SIGIL-CANON-MATCH-ARM-LAYOUT";
    pub const REDUNDANT_PARENS: &str = "SIGIL-CANON-REDUNDANT-PARENS";
    pub const MATCH_BODY_BLOCK: &str = "SIGIL-CANON-MATCH-BODY-BLOCK";

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
    pub const IDENTIFIER_FORM: &str = "SIGIL-CANON-IDENTIFIER-FORM";
    pub const TYPE_NAME_FORM: &str = "SIGIL-CANON-TYPE-NAME-FORM";
    pub const CONSTRUCTOR_NAME_FORM: &str = "SIGIL-CANON-CONSTRUCTOR-NAME-FORM";
    pub const TYPE_VAR_FORM: &str = "SIGIL-CANON-TYPE-VAR-FORM";
    pub const RECORD_FIELD_FORM: &str = "SIGIL-CANON-RECORD-FIELD-FORM";
    pub const MODULE_PATH_FORM: &str = "SIGIL-CANON-MODULE-PATH-FORM";
    pub const RECORD_EXACTNESS: &str = "SIGIL-CANON-RECORD-EXACTNESS";

    // Recursion patterns
    pub const RECURSION_ACCUMULATOR: &str = "SIGIL-CANON-RECURSION-ACCUMULATOR";
    pub const RECURSION_COLLECTION_NONSTRUCTURAL: &str =
        "SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL";
    pub const RECURSION_CPS: &str = "SIGIL-CANON-RECURSION-CPS";
    pub const RECURSION_APPEND_RESULT: &str = "SIGIL-CANON-RECURSION-APPEND-RESULT";
    pub const RECURSION_ALL_CLONE: &str = "SIGIL-CANON-RECURSION-ALL-CLONE";
    pub const RECURSION_ANY_CLONE: &str = "SIGIL-CANON-RECURSION-ANY-CLONE";
    pub const RECURSION_FILTER_CLONE: &str = "SIGIL-CANON-RECURSION-FILTER-CLONE";
    pub const RECURSION_FIND_CLONE: &str = "SIGIL-CANON-RECURSION-FIND-CLONE";
    pub const RECURSION_FLATMAP_CLONE: &str = "SIGIL-CANON-RECURSION-FLATMAP-CLONE";
    pub const RECURSION_FOLD_CLONE: &str = "SIGIL-CANON-RECURSION-FOLD-CLONE";
    pub const RECURSION_MAP_CLONE: &str = "SIGIL-CANON-RECURSION-MAP-CLONE";
    pub const RECURSION_REVERSE_CLONE: &str = "SIGIL-CANON-RECURSION-REVERSE-CLONE";
    pub const BRANCHING_SELF_RECURSION: &str = "SIGIL-CANON-BRANCHING-SELF-RECURSION";
    pub const TRAVERSAL_FILTER_COUNT: &str = "SIGIL-CANON-TRAVERSAL-FILTER-COUNT";
    pub const HELPER_DIRECT_WRAPPER: &str = "SIGIL-CANON-HELPER-DIRECT-WRAPPER";

    // Parameter and effect ordering
    pub const PARAM_ORDER: &str = "SIGIL-CANON-PARAM-ORDER";
    pub const EFFECT_ORDER: &str = "SIGIL-CANON-EFFECT-ORDER";
    pub const RECORD_TYPE_FIELD_ORDER: &str = "SIGIL-CANON-RECORD-TYPE-FIELD-ORDER";
    pub const RECORD_LITERAL_FIELD_ORDER: &str = "SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER";
    pub const RECORD_PATTERN_FIELD_ORDER: &str = "SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER";
    pub const NO_SHADOWING: &str = "SIGIL-CANON-NO-SHADOWING";

    // Local bindings
    pub const LET_UNTYPED: &str = "SIGIL-CANON-LET-UNTYPED";
    pub const SINGLE_USE_PURE_BINDING: &str = "SIGIL-CANON-SINGLE-USE-PURE-BINDING";
    pub const UNUSED_IMPORT: &str = "SIGIL-CANON-UNUSED-IMPORT";
    pub const UNUSED_EXTERN: &str = "SIGIL-CANON-UNUSED-EXTERN";
    pub const UNUSED_BINDING: &str = "SIGIL-CANON-UNUSED-BINDING";
    pub const UNUSED_DECLARATION: &str = "SIGIL-CANON-UNUSED-DECLARATION";

    // Declaration ordering
    pub const DECL_CATEGORY_ORDER: &str = "SIGIL-CANON-DECL-CATEGORY-ORDER";
    pub const DECL_EXPORT_ORDER: &str = "SIGIL-CANON-DECL-EXPORT-ORDER";
    pub const DECL_ALPHABETICAL: &str = "SIGIL-CANON-DECL-ALPHABETICAL";

    // Extern member ordering
    pub const EXTERN_MEMBER_ORDER: &str = "SIGIL-CANON-EXTERN-MEMBER-ORDER";
    pub const FEATURE_FLAG_DECLARATION: &str = "SIGIL-CANON-FEATURE-FLAG-DECL";
}

/// Type checker error codes (SIGIL-TYPE-*)
pub mod typecheck {
    pub const ERROR: &str = "SIGIL-TYPE-ERROR";
    pub const MODULE_NOT_EXPORTED: &str = "SIGIL-TYPE-MODULE-NOT-EXPORTED";
    pub const MATCH_NON_EXHAUSTIVE: &str = "SIGIL-TYPE-MATCH-NON-EXHAUSTIVE";
    pub const MATCH_REDUNDANT_PATTERN: &str = "SIGIL-TYPE-MATCH-REDUNDANT-PATTERN";
    pub const MATCH_UNREACHABLE_ARM: &str = "SIGIL-TYPE-MATCH-UNREACHABLE-ARM";
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
    pub const BREAKPOINT_NOT_FOUND: &str = "SIGIL-CLI-BREAKPOINT-NOT-FOUND";
    pub const BREAKPOINT_AMBIGUOUS: &str = "SIGIL-CLI-BREAKPOINT-AMBIGUOUS";
    pub const IMPORT_NOT_FOUND: &str = "SIGIL-CLI-IMPORT-NOT-FOUND";
    pub const IMPORT_CYCLE: &str = "SIGIL-CLI-IMPORT-CYCLE";
    pub const INVALID_IMPORT: &str = "SIGIL-CLI-INVALID-IMPORT";
    pub const PROJECT_MAIN_REQUIRED: &str = "SIGIL-CLI-PROJECT-MAIN-REQUIRED";
    pub const PROJECT_ROOT_REQUIRED: &str = "SIGIL-CLI-PROJECT-ROOT-REQUIRED";
    pub const CONFIG_ENV_REQUIRED: &str = "SIGIL-CLI-CONFIG-ENV-REQUIRED";
    pub const CONFIG_MODULE_NOT_FOUND: &str = "SIGIL-CLI-CONFIG-MODULE-NOT-FOUND";
}

/// Topology error codes (SIGIL-TOPO-*)
pub mod topology {
    pub const BINDING_KIND_MISMATCH: &str = "SIGIL-TOPO-BINDING-KIND-MISMATCH";
    pub const CONSTRUCTOR_LOCATION: &str = "SIGIL-TOPO-CONSTRUCTOR-LOCATION";
    pub const DEPENDENCY_KIND_MISMATCH: &str = "SIGIL-TOPO-DEPENDENCY-KIND-MISMATCH";
    pub const DUPLICATE_BINDING: &str = "SIGIL-TOPO-DUPLICATE-BINDING";
    pub const DUPLICATE_DEPENDENCY: &str = "SIGIL-TOPO-DUPLICATE-DEPENDENCY";
    pub const ENV_ACCESS_LOCATION: &str = "SIGIL-TOPO-ENV-ACCESS-LOCATION";
    pub const ENV_NOT_FOUND: &str = "SIGIL-TOPO-ENV-NOT-FOUND";
    pub const ENV_REQUIRED: &str = "SIGIL-TOPO-ENV-REQUIRED";
    pub const INVALID_CONFIG_MODULE: &str = "SIGIL-TOPO-INVALID-CONFIG-MODULE";
    pub const INVALID_HANDLE: &str = "SIGIL-TOPO-INVALID-HANDLE";
    pub const LOCAL_WORLD_REQUIRED: &str = "SIGIL-TOPO-LOCAL-WORLD-REQUIRED";
    pub const MISSING_BINDING: &str = "SIGIL-TOPO-MISSING-BINDING";
    pub const MISSING_CONFIG_MODULE: &str = "SIGIL-TOPO-MISSING-CONFIG-MODULE";
    pub const MISSING_MODULE: &str = "SIGIL-TOPO-MISSING-MODULE";
    pub const RAW_ENDPOINT_FORBIDDEN: &str = "SIGIL-TOPO-RAW-ENDPOINT-FORBIDDEN";
}

/// Runtime error codes (SIGIL-RUNTIME-*, SIGIL-RUN-*)
pub mod runtime {
    pub const CHILD_EXIT: &str = "SIGIL-RUNTIME-CHILD-EXIT";
    pub const REPLAY_BINDING_MISMATCH: &str = "SIGIL-RUNTIME-REPLAY-BINDING-MISMATCH";
    pub const REPLAY_DIVERGED: &str = "SIGIL-RUNTIME-REPLAY-DIVERGED";
    pub const REPLAY_INVALID_ARTIFACT: &str = "SIGIL-RUNTIME-REPLAY-INVALID-ARTIFACT";
    pub const UNCAUGHT_EXCEPTION: &str = "SIGIL-RUNTIME-UNCAUGHT-EXCEPTION";
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
    lexer::LEGACY_BOOL,
    // Parser (5 codes)
    parser::CONST_NAME,
    parser::CONST_UNTYPED,
    parser::NS_SEP,
    parser::LOCAL_BINDING,
    parser::UNEXPECTED_TOKEN,
    // Canonical codes
    canonical::DUPLICATE_TYPE,
    canonical::DUPLICATE_EXTERN,
    canonical::DUPLICATE_IMPORT,
    canonical::DUPLICATE_CONST,
    canonical::DUPLICATE_FUNCTION,
    canonical::DUPLICATE_TEST,
    canonical::EOF_NEWLINE,
    canonical::TRAILING_WHITESPACE,
    canonical::BLANK_LINES,
    canonical::SOURCE_FORM,
    canonical::DELIMITER_SPACING,
    canonical::OPERATOR_SPACING,
    canonical::MATCH_LAYOUT,
    canonical::MATCH_ARM_LAYOUT,
    canonical::REDUNDANT_PARENS,
    canonical::MATCH_BODY_BLOCK,
    canonical::RECORD_EXACTNESS,
    canonical::LIB_NO_MAIN,
    canonical::EXEC_NEEDS_MAIN,
    canonical::TEST_NEEDS_MAIN,
    canonical::TEST_LOCATION,
    canonical::TEST_PATH,
    canonical::FILENAME_CASE,
    canonical::FILENAME_INVALID_CHAR,
    canonical::FILENAME_FORMAT,
    canonical::IDENTIFIER_FORM,
    canonical::TYPE_NAME_FORM,
    canonical::CONSTRUCTOR_NAME_FORM,
    canonical::TYPE_VAR_FORM,
    canonical::RECORD_FIELD_FORM,
    canonical::MODULE_PATH_FORM,
    canonical::RECURSION_ACCUMULATOR,
    canonical::RECURSION_COLLECTION_NONSTRUCTURAL,
    canonical::RECURSION_CPS,
    canonical::RECURSION_APPEND_RESULT,
    canonical::RECURSION_ALL_CLONE,
    canonical::RECURSION_ANY_CLONE,
    canonical::RECURSION_FILTER_CLONE,
    canonical::RECURSION_FIND_CLONE,
    canonical::RECURSION_FLATMAP_CLONE,
    canonical::RECURSION_FOLD_CLONE,
    canonical::RECURSION_MAP_CLONE,
    canonical::RECURSION_REVERSE_CLONE,
    canonical::BRANCHING_SELF_RECURSION,
    canonical::TRAVERSAL_FILTER_COUNT,
    canonical::HELPER_DIRECT_WRAPPER,
    canonical::PARAM_ORDER,
    canonical::EFFECT_ORDER,
    canonical::RECORD_TYPE_FIELD_ORDER,
    canonical::RECORD_LITERAL_FIELD_ORDER,
    canonical::RECORD_PATTERN_FIELD_ORDER,
    canonical::NO_SHADOWING,
    canonical::LET_UNTYPED,
    canonical::SINGLE_USE_PURE_BINDING,
    canonical::UNUSED_IMPORT,
    canonical::UNUSED_EXTERN,
    canonical::UNUSED_BINDING,
    canonical::UNUSED_DECLARATION,
    canonical::DECL_CATEGORY_ORDER,
    canonical::DECL_EXPORT_ORDER,
    canonical::DECL_ALPHABETICAL,
    canonical::EXTERN_MEMBER_ORDER,
    canonical::FEATURE_FLAG_DECLARATION,
    // Typecheck codes
    typecheck::ERROR,
    typecheck::MODULE_NOT_EXPORTED,
    typecheck::MATCH_NON_EXHAUSTIVE,
    typecheck::MATCH_REDUNDANT_PATTERN,
    typecheck::MATCH_UNREACHABLE_ARM,
    // Mutability (1 code)
    mutability::INVALID,
    // CLI (10 codes)
    cli::BREAKPOINT_NOT_FOUND,
    cli::BREAKPOINT_AMBIGUOUS,
    cli::USAGE,
    cli::UNKNOWN_COMMAND,
    cli::UNSUPPORTED_OPTION,
    cli::UNEXPECTED,
    cli::IMPORT_NOT_FOUND,
    cli::IMPORT_CYCLE,
    cli::INVALID_IMPORT,
    cli::PROJECT_MAIN_REQUIRED,
    cli::PROJECT_ROOT_REQUIRED,
    cli::CONFIG_ENV_REQUIRED,
    cli::CONFIG_MODULE_NOT_FOUND,
    // Topology (10 codes)
    topology::BINDING_KIND_MISMATCH,
    topology::CONSTRUCTOR_LOCATION,
    topology::DEPENDENCY_KIND_MISMATCH,
    topology::DUPLICATE_BINDING,
    topology::DUPLICATE_DEPENDENCY,
    topology::ENV_ACCESS_LOCATION,
    topology::ENV_NOT_FOUND,
    topology::ENV_REQUIRED,
    topology::INVALID_CONFIG_MODULE,
    topology::INVALID_HANDLE,
    topology::LOCAL_WORLD_REQUIRED,
    topology::MISSING_BINDING,
    topology::MISSING_CONFIG_MODULE,
    topology::MISSING_MODULE,
    topology::RAW_ENDPOINT_FORBIDDEN,
    // Runtime (3 codes)
    runtime::CHILD_EXIT,
    runtime::REPLAY_BINDING_MISMATCH,
    runtime::REPLAY_DIVERGED,
    runtime::REPLAY_INVALID_ARTIFACT,
    runtime::UNCAUGHT_EXCEPTION,
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
        // Keep this in sync when adding or removing diagnostic codes.
        assert_eq!(
            ALL_ERROR_CODES.len(),
            117,
            "Expected 117 error codes, found {}",
            ALL_ERROR_CODES.len()
        );
    }
}
