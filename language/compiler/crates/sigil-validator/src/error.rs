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

    #[error("SIGIL-CANON-RECURSION-APPEND-RESULT: Recursive function '{function_name}' appends to the recursive result.\n\nSigil rejects recursive result-building of the form self(rest)⧺rhs.\nThis shape is non-canonical and usually rebuilds lists inefficiently.\n\nUse map, filter, reduce, or a wrapper plus accumulator helper with one final reverse.")]
    RecursiveAppendResult {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-ALL-CLONE: Recursive function '{function_name}' is a hand-rolled all.\n\nSigil rejects exact recursive all clones and requires the canonical stdlib surface.\n\nUse stdlib::list.all(pred,xs) instead of custom recursive universal checks.")]
    RecursiveAllClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-ANY-CLONE: Recursive function '{function_name}' is a hand-rolled any.\n\nSigil rejects exact recursive any clones and requires the canonical stdlib surface.\n\nUse stdlib::list.any(pred,xs) instead of custom recursive existential checks.")]
    RecursiveAnyClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-MAP-CLONE: Recursive function '{function_name}' is a hand-rolled map.\n\nSigil rejects exact recursive map clones and requires the canonical operator.\n\nUse xs map f instead of custom recursive list projection.")]
    RecursiveMapClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-FILTER-CLONE: Recursive function '{function_name}' is a hand-rolled filter.\n\nSigil rejects exact recursive filter clones and requires the canonical operator.\n\nUse xs filter pred instead of custom recursive list filtering.")]
    RecursiveFilterClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-FIND-CLONE: Recursive function '{function_name}' is a hand-rolled find.\n\nSigil rejects exact recursive find clones and requires the canonical stdlib surface.\n\nUse stdlib::list.find(pred,xs) instead of custom recursive element search.")]
    RecursiveFindClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-FLATMAP-CLONE: Recursive function '{function_name}' is a hand-rolled flatMap.\n\nSigil rejects exact recursive flatMap clones and requires the canonical stdlib surface.\n\nUse stdlib::list.flatMap(fn,xs) instead of custom recursive flattening projection.")]
    RecursiveFlatMapClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-REVERSE-CLONE: Recursive function '{function_name}' is a hand-rolled reverse.\n\nSigil rejects the classic self(rest)⧺[x] reverse shape.\n\nUse stdlib::list.reverse instead.")]
    RecursiveReverseClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-FOLD-CLONE: Recursive function '{function_name}' is a hand-rolled fold.\n\nSigil rejects exact recursive list-reduction clones and requires the canonical reduction surface.\n\nUse xs reduce fn from init or stdlib::list.fold instead of custom recursive reduction.")]
    RecursiveFoldClone {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-BRANCHING-SELF-RECURSION: Recursive function '{function_name}' uses non-canonical branching self-recursion.\n\nSigil rejects exact sibling self-calls that reduce the same parameter while keeping the other arguments unchanged. That shape duplicates work instead of following one canonical recursion path.\n\nUse a wrapper plus helper accumulator pattern, or another canonical state-threading helper shape, instead.")]
    BranchingSelfRecursion {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-MISSING-DECREASES: Self-recursive function '{function_name}' is missing a `decreases` clause.\n\nSigil requires every self-recursive function to declare a termination measure with `decreases <expr>`. The measure must lower to the canonical proof fragment (an Int expression or a tuple of Int expressions) and must strictly decrease at every recursive call while staying bounded below.\n\nAdd a `decreases` clause between `requires` and `ensures` (canonical clause order: requires => decreases => ensures), e.g. `decreases n` for an integer counter or `decreases #xs` for a shrinking list.")]
    RecursionMissingDecreases {
        function_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-MUTUAL-RECURSION: Functions [{cycle}] form a mutual-recursion cycle.\n\nSigil rejects mutual recursion. Refactor the cycle into a single self-recursive function with a sum-typed mode parameter (e.g. `t Mode=A()|B()` plus one helper that pattern-matches on the mode).\n\nMutual recursion's termination measures are too rich for Sigil's canonical proof fragment, so the canonical answer is to collapse the cycle.")]
    MutualRecursion {
        cycle: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-TRAVERSAL-FILTER-COUNT: Expression uses filter then length for counting.\n\nSigil rejects the exact shape #(xs filter pred) when a canonical one-pass counting path exists.\n\nUse stdlib::list.countIf(pred,xs) instead.")]
    FilterThenCount { location: SourceLocation },

    #[error("SIGIL-CANON-HELPER-DIRECT-WRAPPER: Function '{function_name}' is an exact wrapper around canonical helper '{canonical_helper}'.\n\nSigil rejects exact top-level helper aliases when a canonical helper surface already exists.\n\nUse {canonical_surface} directly instead.")]
    HelperDirectWrapper {
        function_name: String,
        canonical_helper: String,
        canonical_surface: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL: Recursive function '{function_name}' has collection parameter but doesn't use structural recursion.\n\nSigil enforces ONE way: structural recursion for collections.")]
    NonStructuralRecursion {
        function_name: String,
        param_name: String,
        location: SourceLocation,
    },

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
    FilePurposeNone { message: String },

    #[error("SIGIL-CANON-FILE-PURPOSE-BOTH: {message}")]
    FilePurposeBoth { message: String },

    #[error("SIGIL-CANON-TEST-LOCATION: {message}")]
    TestLocationInvalid { message: String, file_path: String },

    #[error("SIGIL-CANON-TEST-NO-EXPORTS: {message}")]
    TestNoExports { message: String },

    #[error("SIGIL-CANON-LIB-NO-MAIN: {message}")]
    LibNoMain { message: String },

    #[error("SIGIL-CANON-EXEC-NEEDS-MAIN: {message}")]
    ExecNeedsMain { message: String },

    #[error("SIGIL-CANON-TEST-NEEDS-MAIN: {message}")]
    TestNeedsMain { message: String },

    #[error("SIGIL-CANON-DECL-CATEGORY-ORDER: {message}")]
    DeclarationOrderOld { message: String },

    #[error("SIGIL-CANON-FILENAME-CASE: filenames must start with a lowercase letter\n\nFile: {filename}\nFound: {basename}\nRename to: {suggested}\n\nSigil enforces ONE way: filenames must be lowerCamelCase.")]
    FilenameCase {
        filename: String,
        basename: String,
        suggested: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-FILENAME-INVALID-CHAR: filenames cannot contain {invalid_char}\n\nFile: {filename}\nFound in: {basename}\nRename to: {suggested}\n\nSigil enforces ONE way: filenames must be lowerCamelCase.")]
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

    #[error("SIGIL-CANON-SOURCE-FORM: source is not written in Sigil's one true canonical form\n\nWrite the file exactly as:\n\n{canonical_source}")]
    SourceForm {
        canonical_source: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-EFFECT-DECL-PLACEMENT: {message}")]
    EffectDeclarationPlacement {
        message: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-TYPE-DECL-PLACEMENT: {message}")]
    TypeDeclarationPlacement {
        message: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-POLICY-DECL-PLACEMENT: {message}")]
    PolicyDeclarationPlacement {
        message: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-FEATURE-FLAG-DECL: {message}")]
    FeatureFlagDeclaration {
        message: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-IDENTIFIER-FORM: value identifiers must be lowerCamelCase\n\nFound: {found}\nExpected form: lowerCamelCase{suggestion}")]
    IdentifierForm {
        found: String,
        suggestion: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-TYPE-NAME-FORM: type names must be UpperCamelCase\n\nFound: {found}\nExpected form: UpperCamelCase{suggestion}")]
    TypeNameForm {
        found: String,
        suggestion: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-CONSTRUCTOR-NAME-FORM: constructor names must be UpperCamelCase\n\nFound: {found}\nExpected form: UpperCamelCase{suggestion}")]
    ConstructorNameForm {
        found: String,
        suggestion: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-TYPE-VAR-FORM: type variables must be UpperCamelCase\n\nFound: {found}\nExpected form: UpperCamelCase{suggestion}")]
    TypeVarForm {
        found: String,
        suggestion: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECORD-FIELD-FORM: record fields must be lowerCamelCase\n\nFound: {found}\nExpected form: lowerCamelCase{suggestion}")]
    RecordFieldForm {
        found: String,
        suggestion: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-MODULE-PATH-FORM: module path segments must be lowerCamelCase\n\nFound: {found}\nExpected form: lowerCamelCase{suggestion}")]
    ModulePathForm {
        found: String,
        suggestion: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-EOF-NEWLINE: file must end with newline\n\nFile: {filename}")]
    EOFNewline {
        filename: String,
        location: SourceLocation,
    },

    #[error(
        "SIGIL-CANON-TRAILING-WHITESPACE: trailing whitespace\n\nFile: {filename}\nLine: {line}"
    )]
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

    #[error("SIGIL-CANON-DELIMITER-SPACING: non-canonical delimiter spacing\n\nUse tight delimiters with no spaces just inside brackets, parentheses, or braces.")]
    DelimiterSpacing { location: SourceLocation },

    #[error("SIGIL-CANON-OPERATOR-SPACING: non-canonical operator spacing\n\nUse no spaces around ':', '=>', '=', '|', '+', '-', '*', '/', and '%'.")]
    OperatorSpacing { location: SourceLocation },

    #[error("SIGIL-CANON-MATCH-LAYOUT: non-canonical match layout\n\nSingle-arm match may stay on one line. Multi-arm match must use multiline canonical layout.")]
    MatchLayout { location: SourceLocation },

    #[error("SIGIL-CANON-MATCH-ARM-LAYOUT: non-canonical match arm layout\n\nEach multiline match arm header must start as 'pattern=>'. The body must begin on that same line. Continued body lines must stay in canonical indented form.")]
    MatchArmLayout { location: SourceLocation },

    #[error("SIGIL-CANON-REDUNDANT-PARENS: redundant parentheses\n\nRemove parentheses that do not change the canonical expression shape.")]
    RedundantParens { location: SourceLocation },

    #[error("SIGIL-CANON-MATCH-BODY-BLOCK: direct match bodies must not be wrapped in a block\n\nUse 'λf()=>T match ...' instead of wrapping the match in '{{...}}'.")]
    MatchBodyBlock { location: SourceLocation },

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

    #[error("SIGIL-CANON-RECORD-TYPE-FIELD-ORDER: Record type fields out of alphabetical order in '{type_name}'\n\nFound: {field_name} at position {position}\nAfter: {prev_field}\n\nRecord type fields must be alphabetically ordered.\nExpected '{field_name}' to come before '{prev_field}'.\n\nCorrect order: {expected_order:?}\n\nSigil enforces ONE WAY: canonical record field ordering.")]
    RecordTypeFieldOrder {
        type_name: String,
        field_name: String,
        prev_field: String,
        position: usize,
        expected_order: Vec<String>,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER: Record literal fields out of alphabetical order\n\nFound: {field_name} at position {position}\nAfter: {prev_field}\n\nRecord literal fields must be alphabetically ordered.\nExpected '{field_name}' to come before '{prev_field}'.\n\nCorrect order: {expected_order:?}\n\nSigil enforces ONE WAY: canonical record field ordering.")]
    RecordLiteralFieldOrder {
        field_name: String,
        prev_field: String,
        position: usize,
        expected_order: Vec<String>,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER: Record pattern fields out of alphabetical order\n\nFound: {field_name} at position {position}\nAfter: {prev_field}\n\nRecord pattern fields must be alphabetically ordered.\nExpected '{field_name}' to come before '{prev_field}'.\n\nCorrect order: {expected_order:?}\n\nSigil enforces ONE WAY: canonical record field ordering.")]
    RecordPatternFieldOrder {
        field_name: String,
        prev_field: String,
        position: usize,
        expected_order: Vec<String>,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-NO-SHADOWING: Binding '{name}' shadows an existing {previous_kind} binding.\n\nPrevious binding: {previous_kind} '{name}' at line {previous_line}, column {previous_column}\n\nSigil requires ONE WAY: one local name, one meaning.\nUse a new name instead of rebinding '{name}'.")]
    NoShadowing {
        name: String,
        current_kind: String,
        previous_kind: String,
        location: SourceLocation,
        previous_location: SourceLocation,
        previous_line: usize,
        previous_column: usize,
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

    #[error("SIGIL-CANON-SINGLE-USE-PURE-BINDING: Single-use pure binding '{binding_name}' must be inlined\n\nSigil enforces ONE WAY: pure intermediates used once stay inline.\nBindings exist for reuse, effects, destructuring, or syntax-required staging.")]
    SingleUsePureBinding {
        binding_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-DEAD-PURE-DISCARD: Wildcard sequencing must not discard pure expressions\n\nSigil reserves 'l _=(...)' for sequencing observable effects.\nThis expression is pure, so discarding it contributes nothing.\nUse the value, inline it into a real use, or delete it.")]
    DeadPureDiscard { location: SourceLocation },

    #[error("SIGIL-CANON-UNUSED-EXTERN: Extern '{extern_path}' is never used\n\nSigil rejects dead extern declarations.\nRemove the extern or use it.")]
    UnusedExtern {
        extern_path: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-UNUSED-BINDING: Binding '{binding_name}' is never used\n\nNamed bindings must contribute meaning.\nUse '_' when discarding a value only for its effects, otherwise inline or delete it.")]
    UnusedBinding {
        binding_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-UNUSED-DECLARATION: Unused executable {decl_kind} '{decl_name}'\n\nDead top-level executable declarations are non-canonical unless they are part of Sigil's runtime-facing surface.\nRemove the {decl_kind} or make main/tests reach it.")]
    UnusedDeclaration {
        decl_kind: String,
        decl_name: String,
        location: SourceLocation,
    },

    #[error("SIGIL-CANON-DECL-CATEGORY-ORDER: Declarations out of category order\n\nExpected: types => externs => consts => functions => tests\nFound: {found_category} after {prev_category}")]
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
            ValidationError::RecursiveAppendResult { location, .. } => *location,
            ValidationError::RecursiveAllClone { location, .. } => *location,
            ValidationError::RecursiveAnyClone { location, .. } => *location,
            ValidationError::RecursiveMapClone { location, .. } => *location,
            ValidationError::RecursiveFilterClone { location, .. } => *location,
            ValidationError::RecursiveFindClone { location, .. } => *location,
            ValidationError::RecursiveFlatMapClone { location, .. } => *location,
            ValidationError::RecursiveReverseClone { location, .. } => *location,
            ValidationError::RecursiveFoldClone { location, .. } => *location,
            ValidationError::BranchingSelfRecursion { location, .. } => *location,
            ValidationError::FilterThenCount { location } => *location,
            ValidationError::HelperDirectWrapper { location, .. } => *location,
            ValidationError::NonStructuralRecursion { location, .. } => *location,
            ValidationError::MissingReturnType { location, .. } => *location,
            ValidationError::MissingParamType { location, .. } => *location,
            ValidationError::FilePurposeNone { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::FilePurposeBoth { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::TestLocationInvalid { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::TestNoExports { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::LibNoMain { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::ExecNeedsMain { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::TestNeedsMain { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::DeclarationOrderOld { .. } => SourceLocation {
                start: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: sigil_lexer::Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
            ValidationError::FilenameCase { location, .. } => *location,
            ValidationError::FilenameInvalidChar { location, .. } => *location,
            ValidationError::FilenameFormat { location, .. } => *location,
            ValidationError::SourceForm { location, .. } => *location,
            ValidationError::EffectDeclarationPlacement { location, .. } => *location,
            ValidationError::TypeDeclarationPlacement { location, .. } => *location,
            ValidationError::PolicyDeclarationPlacement { location, .. } => *location,
            ValidationError::FeatureFlagDeclaration { location, .. } => *location,
            ValidationError::IdentifierForm { location, .. } => *location,
            ValidationError::TypeNameForm { location, .. } => *location,
            ValidationError::ConstructorNameForm { location, .. } => *location,
            ValidationError::TypeVarForm { location, .. } => *location,
            ValidationError::RecordFieldForm { location, .. } => *location,
            ValidationError::ModulePathForm { location, .. } => *location,
            ValidationError::EOFNewline { location, .. } => *location,
            ValidationError::TrailingWhitespace { location, .. } => *location,
            ValidationError::BlankLines { location, .. } => *location,
            ValidationError::DelimiterSpacing { location } => *location,
            ValidationError::OperatorSpacing { location } => *location,
            ValidationError::MatchLayout { location } => *location,
            ValidationError::MatchArmLayout { location } => *location,
            ValidationError::RedundantParens { location } => *location,
            ValidationError::MatchBodyBlock { location } => *location,
            ValidationError::ParameterOrder { location, .. } => *location,
            ValidationError::EffectOrder { location, .. } => *location,
            ValidationError::RecordTypeFieldOrder { location, .. } => *location,
            ValidationError::RecordLiteralFieldOrder { location, .. } => *location,
            ValidationError::RecordPatternFieldOrder { location, .. } => *location,
            ValidationError::NoShadowing { location, .. } => *location,
            ValidationError::TestPath { location, .. } => *location,
            ValidationError::DeclExportOrder { location, .. } => *location,
            ValidationError::ExternMemberOrder { location, .. } => *location,
            ValidationError::LetUntyped { location, .. } => *location,
            ValidationError::SingleUsePureBinding { location, .. } => *location,
            ValidationError::DeadPureDiscard { location } => *location,
            ValidationError::UnusedExtern { location, .. } => *location,
            ValidationError::UnusedBinding { location, .. } => *location,
            ValidationError::UnusedDeclaration { location, .. } => *location,
            ValidationError::DeclCategoryOrder { location, .. } => *location,
            ValidationError::DeclAlphabetical { location, .. } => *location,
            ValidationError::RecursionMissingDecreases { location, .. } => *location,
            ValidationError::MutualRecursion { location, .. } => *location,
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

            ValidationError::ParameterOrder { function_name, param_name, prev_param, position: _, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::PARAM_ORDER,
                    SigilPhase::Canonical,
                    format!("Parameter '{}' out of alphabetical order in function '{}'", param_name, function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&param_name, &prev_param)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::EffectOrder { function_name, effect_name, prev_effect, position: _, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::EFFECT_ORDER,
                    SigilPhase::Canonical,
                    format!("Effect '{}' out of alphabetical order in function '{}'", effect_name, function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&effect_name, &prev_effect)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::SingleUsePureBinding { binding_name, location } => {
                Diagnostic::new(
                    codes::canonical::SINGLE_USE_PURE_BINDING,
                    SigilPhase::Canonical,
                    format!("Single-use pure binding '{}' must be inlined", binding_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details(
                    "guidance",
                    "Bindings exist for reuse, effects, destructuring, or syntax-required staging.",
                )
            }

            ValidationError::DeadPureDiscard { location } => Diagnostic::new(
                codes::canonical::DEAD_PURE_DISCARD,
                SigilPhase::Canonical,
                "Wildcard sequencing must not discard pure expressions".to_string(),
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_details(
                "guidance",
                "Use the value, inline it into a real use, or delete it. `l _=(...)` is reserved for sequencing effects.",
            ),

            ValidationError::UnusedExtern {
                extern_path,
                location,
            } => Diagnostic::new(
                codes::canonical::UNUSED_EXTERN,
                SigilPhase::Canonical,
                format!("Extern '{}' is never used", extern_path),
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_details("guidance", "Remove the extern or use it."),

            ValidationError::UnusedBinding {
                binding_name,
                location,
            } => Diagnostic::new(
                codes::canonical::UNUSED_BINDING,
                SigilPhase::Canonical,
                format!("Binding '{}' is never used", binding_name),
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_details(
                "guidance",
                "Use '_' when discarding a value only for its effects, otherwise inline or delete it.",
            ),

            ValidationError::UnusedDeclaration {
                decl_kind,
                decl_name,
                location,
            } => Diagnostic::new(
                codes::canonical::UNUSED_DECLARATION,
                SigilPhase::Canonical,
                format!("Unused executable {} '{}'", decl_kind, decl_name),
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_details(
                "guidance",
                "Remove the declaration or make main/tests reach it.",
            ),

            ValidationError::RecursiveAppendResult { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_APPEND_RESULT,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' appends to the recursive result", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use map, filter, reduce, or a wrapper plus accumulator helper with one final reverse.")
            }

            ValidationError::RecursiveAllClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_ALL_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled all", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use stdlib::list.all(pred,xs) instead of custom recursive universal checks.")
            }

            ValidationError::RecursiveAnyClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_ANY_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled any", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use stdlib::list.any(pred,xs) instead of custom recursive existential checks.")
            }

            ValidationError::RecursiveMapClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_MAP_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled map", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use xs map f instead of custom recursive list projection.")
            }

            ValidationError::RecursiveFilterClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_FILTER_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled filter", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use xs filter pred instead of custom recursive list filtering.")
            }

            ValidationError::RecursiveFindClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_FIND_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled find", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use stdlib::list.find(pred,xs) instead of custom recursive element search.")
            }

            ValidationError::RecursiveFlatMapClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_FLATMAP_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled flatMap", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use stdlib::list.flatMap(fn,xs) instead of custom recursive flattening projection.")
            }

            ValidationError::RecursiveReverseClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_REVERSE_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled reverse", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use stdlib::list.reverse instead.")
            }

            ValidationError::RecursiveFoldClone { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::RECURSION_FOLD_CLONE,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' is a hand-rolled fold", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use xs reduce fn from init or stdlib::list.fold instead of custom recursive reduction.")
            }

            ValidationError::BranchingSelfRecursion { function_name, location } => {
                Diagnostic::new(
                    codes::canonical::BRANCHING_SELF_RECURSION,
                    SigilPhase::Canonical,
                    format!("Recursive function '{}' uses non-canonical branching self-recursion", function_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details(
                    "guidance",
                    "Use a wrapper plus helper accumulator pattern, or another canonical state-threading helper shape, instead of sibling self-calls over the same reduced parameter.",
                )
            }

            ValidationError::FilterThenCount { location } => {
                Diagnostic::new(
                    codes::canonical::TRAVERSAL_FILTER_COUNT,
                    SigilPhase::Canonical,
                    "filter followed by length is not canonical".to_string(),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("guidance", "Use stdlib::list.countIf(pred,xs) instead of #(xs filter pred).")
            }

            ValidationError::HelperDirectWrapper {
                function_name,
                canonical_helper,
                canonical_surface,
                location,
            } => Diagnostic::new(
                codes::canonical::HELPER_DIRECT_WRAPPER,
                SigilPhase::Canonical,
                format!(
                    "Function '{}' duplicates canonical helper '{}'",
                    function_name, canonical_helper
                ),
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_details("kind", "direct_wrapper")
            .with_details("guidance", format!("Use {} directly instead.", canonical_surface)),

            ValidationError::RecordTypeFieldOrder { type_name, field_name, prev_field, position: _, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::RECORD_TYPE_FIELD_ORDER,
                    SigilPhase::Canonical,
                    format!("Record type field '{}' out of alphabetical order in '{}'", field_name, type_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&field_name, &prev_field)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::RecordLiteralFieldOrder { field_name, prev_field, position: _, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::RECORD_LITERAL_FIELD_ORDER,
                    SigilPhase::Canonical,
                    format!("Record literal field '{}' out of alphabetical order", field_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&field_name, &prev_field)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::RecordPatternFieldOrder { field_name, prev_field, position: _, expected_order, location } => {
                Diagnostic::new(
                    codes::canonical::RECORD_PATTERN_FIELD_ORDER,
                    SigilPhase::Canonical,
                    format!("Record pattern field '{}' out of alphabetical order", field_name),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_found_expected(&field_name, &prev_field)
                .with_details("expected_order", format!("{:?}", expected_order))
            }

            ValidationError::NoShadowing {
                name,
                current_kind,
                previous_kind,
                location,
                previous_location,
                previous_line,
                previous_column,
            } => {
                Diagnostic::new(
                    codes::canonical::NO_SHADOWING,
                    SigilPhase::Canonical,
                    format!("{} '{}' shadows an existing {} binding", current_kind, name, previous_kind),
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("name", name)
                .with_details("previous_kind", previous_kind)
                .with_details("previous_location", format!("{}:{}", previous_line, previous_column))
                .with_details(
                    "previous_binding_end",
                    format!("{}:{}", previous_location.end.line, previous_location.end.column),
                )
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

            ValidationError::ExternMemberOrder { member_name, prev_member, position: _, location } => {
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

            ValidationError::SourceForm { canonical_source, location } => {
                Diagnostic::new(
                    codes::canonical::SOURCE_FORM,
                    SigilPhase::Canonical,
                    "Source is not written in Sigil's one true canonical form",
                )
                .with_location(source_location_to_span(get_file(), location))
                .with_details("canonical_source", canonical_source)
            }

            ValidationError::EffectDeclarationPlacement { message, location } => Diagnostic::new(
                codes::canonical::SOURCE_FORM,
                SigilPhase::Canonical,
                message,
            )
            .with_location(source_location_to_span(get_file(), location)),

            ValidationError::TypeDeclarationPlacement { message, location } => Diagnostic::new(
                codes::canonical::SOURCE_FORM,
                SigilPhase::Canonical,
                message,
            )
            .with_location(source_location_to_span(get_file(), location)),

            ValidationError::PolicyDeclarationPlacement { message, location } => Diagnostic::new(
                codes::canonical::SOURCE_FORM,
                SigilPhase::Canonical,
                message,
            )
            .with_location(source_location_to_span(get_file(), location)),

            ValidationError::FeatureFlagDeclaration { message, location } => Diagnostic::new(
                codes::canonical::FEATURE_FLAG_DECLARATION,
                SigilPhase::Canonical,
                message,
            )
            .with_location(source_location_to_span(get_file(), location)),

            ValidationError::IdentifierForm { found, suggestion, location } => Diagnostic::new(
                codes::canonical::IDENTIFIER_FORM,
                SigilPhase::Canonical,
                "value identifiers must be lowerCamelCase",
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_found_expected(&found, "lowerCamelCase")
            .with_details("suggestion", suggestion),

            ValidationError::TypeNameForm { found, suggestion, location } => Diagnostic::new(
                codes::canonical::TYPE_NAME_FORM,
                SigilPhase::Canonical,
                "type names must be UpperCamelCase",
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_found_expected(&found, "UpperCamelCase")
            .with_details("suggestion", suggestion),

            ValidationError::ConstructorNameForm { found, suggestion, location } => Diagnostic::new(
                codes::canonical::CONSTRUCTOR_NAME_FORM,
                SigilPhase::Canonical,
                "constructor names must be UpperCamelCase",
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_found_expected(&found, "UpperCamelCase")
            .with_details("suggestion", suggestion),

            ValidationError::TypeVarForm { found, suggestion, location } => Diagnostic::new(
                codes::canonical::TYPE_VAR_FORM,
                SigilPhase::Canonical,
                "type variables must be UpperCamelCase",
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_found_expected(&found, "UpperCamelCase")
            .with_details("suggestion", suggestion),

            ValidationError::RecordFieldForm { found, suggestion, location } => Diagnostic::new(
                codes::canonical::RECORD_FIELD_FORM,
                SigilPhase::Canonical,
                "record fields must be lowerCamelCase",
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_found_expected(&found, "lowerCamelCase")
            .with_details("suggestion", suggestion),

            ValidationError::ModulePathForm { found, suggestion, location } => Diagnostic::new(
                codes::canonical::MODULE_PATH_FORM,
                SigilPhase::Canonical,
                "module path segments must be lowerCamelCase",
            )
            .with_location(source_location_to_span(get_file(), location))
            .with_found_expected(&found, "lowerCamelCase")
            .with_details("suggestion", suggestion),

            ValidationError::DelimiterSpacing { location } => {
                Diagnostic::new(
                    codes::canonical::DELIMITER_SPACING,
                    SigilPhase::Canonical,
                    "non-canonical delimiter spacing",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::OperatorSpacing { location } => {
                Diagnostic::new(
                    codes::canonical::OPERATOR_SPACING,
                    SigilPhase::Canonical,
                    "non-canonical operator spacing",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::MatchLayout { location } => {
                Diagnostic::new(
                    codes::canonical::MATCH_LAYOUT,
                    SigilPhase::Canonical,
                    "non-canonical match layout",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::MatchArmLayout { location } => {
                Diagnostic::new(
                    codes::canonical::MATCH_ARM_LAYOUT,
                    SigilPhase::Canonical,
                    "non-canonical match arm layout",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::RedundantParens { location } => {
                Diagnostic::new(
                    codes::canonical::REDUNDANT_PARENS,
                    SigilPhase::Canonical,
                    "redundant parentheses",
                )
                .with_location(source_location_to_span(get_file(), location))
            }

            ValidationError::MatchBodyBlock { location } => {
                Diagnostic::new(
                    codes::canonical::MATCH_BODY_BLOCK,
                    SigilPhase::Canonical,
                    "direct match bodies must not be wrapped in a block",
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
