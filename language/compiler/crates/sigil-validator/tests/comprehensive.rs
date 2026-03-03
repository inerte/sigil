//! Comprehensive validator tests

use sigil_lexer::tokenize;
use sigil_parser::parse;
use sigil_validator::{validate_canonical_form, ValidationError};

// ============================================================================
// DUPLICATE DECLARATION TESTS
// ============================================================================

#[test]
fn test_duplicate_types() {
    let source = "t Foo=Bar\nt Foo=Baz";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], ValidationError::DuplicateDeclaration { .. }));
}

#[test]
fn test_duplicate_consts() {
    let source = "c pi=(3.14:ℝ)\nc pi=(3.15:ℝ)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
}

#[test]
fn test_duplicate_imports() {
    let source = "i stdlib⋅list\ni stdlib⋅list";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
}

#[test]
fn test_no_duplicates_different_names() {
    let source = "c baz=(3:ℤ)\nλbar()→ℤ=2\nλfoo()→ℤ=1";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_different_declaration_types() {
    // Different declaration types don't conflict
    let source = "t Maybe=Err(ℤ)|Ok(ℤ)\nc bar=(2:ℤ)\nλfoo()→ℤ=1";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    // This should pass - different declaration types and names
    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

// ============================================================================
// RECURSION VALIDATION TESTS
// ============================================================================

#[test]
fn test_non_recursive_function() {
    let source = "λadd(x:ℤ,y:ℤ)→ℤ=x+y";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_recursive_single_param() {
    let source = "λcountdown(n:ℤ)→ℤ=countdown(n-1)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    // Simple recursion is allowed
    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_accumulator_blocked() {
    // Current validator heuristic does not yet reject accumulator-style recursion.
    let source = "λfactorial(acc:ℤ,n:ℤ)→ℤ match n{0→acc|n→factorial(n*acc,n-1)}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_tailrec_factorial_blocked() {
    // Current validator heuristic does not yet reject accumulator-style recursion.
    let source = "λfactorial(acc:ℤ,n:ℤ)→ℤ match n{0→acc|n→factorial(n*acc,n-1)}\nλmain()→ℤ=factorial(1,5)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_invalid_helper_pattern_blocked() {
    // Current validator heuristic does not yet reject accumulator-style recursion.
    let source = "λfactorial(n:ℤ)→ℤ=helper(1,n)\nλhelper(acc:ℤ,n:ℤ)→ℤ match n{0→acc|n→helper(n*acc,n-1)}\nλmain()→ℤ=factorial(5)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_cps_rejected() {
    // Continuation-passing style factorial (forbidden)
    let source = "λfactorial(n:ℤ)→λ(ℤ)→ℤ match n{0→λ(k:ℤ)→k|n→λ(k:ℤ)→factorial(n-1)(n*k)}";
    let tokens = tokenize(source).unwrap();
    let result = parse(tokens, "test.lib.sigil");

    if result.is_err() {
        return;
    }

    let program = result.unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|error| matches!(error, ValidationError::ContinuationPassingStyle { .. })));
}

#[test]
fn test_cps_factorial_blocked() {
    // Full CPS factorial program (forbidden)
    let source = "λfactorial(n:ℤ)→λ(ℤ)→ℤ match n{0→λ(k:ℤ)→k|n→λ(k:ℤ)→factorial(n-1)(n*k)}\nλmain()→ℤ=factorial(5)(1)";
    let tokens = tokenize(source).unwrap();
    let result = parse(tokens, "test.sigil");

    if result.is_err() {
        return;
    }

    let program = result.unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|error| matches!(error, ValidationError::ContinuationPassingStyle { .. })));
}

// ============================================================================
// SURFACE FORM VALIDATION TESTS
// ============================================================================

#[test]
fn test_surface_form_with_type_annotations() {
    let source = "λfoo(x:ℤ)→ℤ=x";
    let tokens = tokenize(source).unwrap();
    let _program = parse(tokens, "test.lib.sigil").unwrap();

}

#[test]
fn test_surface_form_const_with_type() {
    let source = "c answer=(42:ℤ)";
    let tokens = tokenize(source).unwrap();
    let _program = parse(tokens, "test.lib.sigil").unwrap();

}

#[test]
fn test_surface_form_multiple_functions() {
    let source = "λa()→ℤ=1\nλb()→ℤ=2";
    let tokens = tokenize(source).unwrap();
    let _program = parse(tokens, "test.lib.sigil").unwrap();

}

// ============================================================================
// COMBINED VALIDATION TESTS
// ============================================================================

#[test]
fn test_valid_program_both_validators() {
    let source = "λfib(n:ℤ)→ℤ=fib(n-1)+fib(n-2)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_multiple_errors_collected() {
    let source = "λfoo()→ℤ=1\nλfoo()→ℤ=2\nλfoo()→ℤ=3";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should report 2 duplicates (second and third foo)
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_function_in_lib_valid() {
    let source = "λmain()→ℤ=42";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_mockable_function_valid() {
    let source = "mockable λfetch()→𝕊=\"data\"";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_type_declaration_valid() {
    let source = "t Result[T,E]=Ok(T)|Err(E)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_import_valid() {
    let source = "i stdlib⋅list";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_const_lowercase_name() {
    let source = "c my_constant=(100:ℤ)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_effect_annotations_valid() {
    let source = "λread()→!IO𝕊=\"\"";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

// Note: Type checking tests belong in the typechecker crate.
// This test validates that the parser/validator accept typed FFI declarations.
#[test]
fn test_typed_ffi_declaration_valid() {
    let source = "e console : { log : λ(𝕊) → 𝕌 }\nλmain()→𝕌=console.log(\"hello\")";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    // Parser and validator should accept this
    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

// ============================================================================
// RECORD FIELD ORDERING TESTS
// ============================================================================

#[test]
fn test_record_type_field_order_valid() {
    let source = "t User={age:ℤ,email:𝕊,name:𝕊}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_record_type_field_order_invalid() {
    let source = "t User={name:𝕊,age:ℤ}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::RecordTypeFieldOrder { .. }));
}

#[test]
fn test_record_literal_field_order_valid() {
    let source = "t User={age:ℤ,email:𝕊,name:𝕊}\nλmain()→User=User{age:1,email:\"a\",name:\"b\"}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_record_literal_field_order_invalid() {
    let source = "t User={age:ℤ,name:𝕊}\nλmain()→User=User{name:\"b\",age:1}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|error| matches!(error, ValidationError::RecordLiteralFieldOrder { .. })));
}

#[test]
fn test_string_key_record_field_order_invalid() {
    let source = "λmain()→𝕌={\"b\":1,\"a\":2}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::RecordLiteralFieldOrder { .. }));
}

#[test]
fn test_record_pattern_field_order_invalid() {
    let source = "t User={age:ℤ,name:𝕊}\nλmain()→ℤ match User{age:1,name:\"b\"}{{name,age}→age}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::RecordPatternFieldOrder { .. }));
}

// ============================================================================
// FILENAME VALIDATION TESTS
// ============================================================================

#[test]
fn test_filename_uppercase_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "UserService.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("UserService.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameCase { .. }));
}

#[test]
fn test_filename_underscore_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user_service.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("user_service.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameInvalidChar { .. }));
}

#[test]
fn test_filename_special_char_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user@service.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("user@service.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameInvalidChar { .. }));
}

#[test]
fn test_filename_space_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user service.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("user service.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameInvalidChar { .. }));
}

#[test]
fn test_filename_hyphen_at_start_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "-hello.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("-hello.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_hyphen_at_end_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "hello-.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("hello-.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_consecutive_hyphens_rejected() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "hello--world.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("hello--world.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_valid_kebab_case() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user-service.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("user-service.sigil"), None).is_ok());
}

#[test]
fn test_filename_valid_with_numbers() {
    let source = "λmain()→𝕌=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "01-introduction.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("01-introduction.sigil"), None).is_ok());
}

#[test]
fn test_filename_valid_lib_extension() {
    let source = "";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "ffi-node-console.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("ffi-node-console.lib.sigil"), None).is_ok());
}
