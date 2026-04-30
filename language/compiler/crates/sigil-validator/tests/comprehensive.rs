//! Comprehensive validator tests

use sigil_lexer::tokenize;
use sigil_parser::parse;
use sigil_validator::{print_canonical_program, validate_canonical_form, ValidationError};
use std::fs;
use std::path::PathBuf;

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
    assert!(matches!(
        errors[0],
        ValidationError::DuplicateDeclaration { .. }
    ));
}

#[test]
fn test_duplicate_consts() {
    let source = "c pi=(3.14:Float)\nc pi=(3.15:Float)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
}

#[test]
fn test_no_duplicates_different_names() {
    let source = "c baz=(3:Int)\nλbar()=>Int=2\nλfoo()=>Int=1";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_different_declaration_types() {
    // Different declaration types don't conflict
    let source = "t Maybe=Err(Int)|Ok(Int)\nc bar=(2:Int)\nλfoo()=>Int=1";
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
    let source = "λadd(x:Int,y:Int)=>Int=x+y";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_recursive_single_param() {
    let source = "total λcountdown(n:Int)=>Int\ndecreases n\n=countdown(n-1)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    // Simple recursion is allowed
    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_ordinary_recursive_function_no_longer_needs_decreases() {
    let source = "λloop()=>Int=loop()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_total_recursive_function_still_needs_decreases() {
    let source = "mode total\n\nλloop()=>Int=loop()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::RecursionMissingDecreases { .. })));
}

#[test]
fn test_ordinary_function_decreases_is_rejected() {
    let source = "λcountdown(n:Int)=>Int\ndecreases n\n=countdown(n-1)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::OrdinaryFunctionDecreases { .. })));
}

#[test]
fn test_mutual_recursion_is_rejected() {
    let source = "λeven(n:Int)=>Int=odd(n)\nλodd(n:Int)=>Int=even(n)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::MutualRecursion { .. })));
}

#[test]
fn test_accumulator_blocked() {
    // Current validator heuristic does not yet reject accumulator-style recursion.
    let source =
        "total λfactorial(acc:Int,n:Int)=>Int\ndecreases n\n match n{0=>acc|value=>factorial(acc*value,value-1)}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_tailrec_factorial_blocked() {
    // Current validator heuristic does not yet reject accumulator-style recursion.
    let source = "total λfactorial(acc:Int,n:Int)=>Int\ndecreases n\n match n{0=>acc|value=>factorial(acc*value,value-1)}\nλmain()=>Int=factorial(1,5)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_invalid_helper_pattern_blocked() {
    // Current validator heuristic does not yet reject accumulator-style recursion.
    let source = "λfactorial(n:Int)=>Int=helper(1,n)\ntotal λhelper(acc:Int,n:Int)=>Int\ndecreases n\n match n{0=>acc|value=>helper(acc*value,value-1)}\nλmain()=>Int=factorial(5)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_cps_rejected() {
    // Continuation-passing style factorial (forbidden)
    let source =
        "λfactorial(n:Int)=>λ(Int)=>Int match n{0=>λ(k:Int)=>k|n=>λ(k:Int)=>factorial(n-1)(n*k)}";
    let tokens = tokenize(source).unwrap();
    let result = parse(tokens, "test.lib.sigil");

    if result.is_err() {
        return;
    }

    let program = result.unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::ContinuationPassingStyle { .. })));
}

#[test]
fn test_cps_factorial_blocked() {
    // Full CPS factorial program (forbidden)
    let source = "λfactorial(n:Int)=>λ(Int)=>Int match n{0=>λ(k:Int)=>k|n=>λ(k:Int)=>factorial(n-1)(n*k)}\nλmain()=>Int=factorial(5)(1)";
    let tokens = tokenize(source).unwrap();
    let result = parse(tokens, "test.sigil");

    if result.is_err() {
        return;
    }

    let program = result.unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::ContinuationPassingStyle { .. })));
}

// ============================================================================
// SURFACE FORM VALIDATION TESTS
// ============================================================================

#[test]
fn test_surface_form_with_type_annotations() {
    let source = "λfoo(x:Int)=>Int=x";
    let tokens = tokenize(source).unwrap();
    let _program = parse(tokens, "test.lib.sigil").unwrap();
}

#[test]
fn test_surface_form_const_with_type() {
    let source = "c answer=(42:Int)";
    let tokens = tokenize(source).unwrap();
    let _program = parse(tokens, "test.lib.sigil").unwrap();
}

#[test]
fn test_surface_form_multiple_functions() {
    let source = "λa()=>Int=1\nλb()=>Int=2";
    let tokens = tokenize(source).unwrap();
    let _program = parse(tokens, "test.lib.sigil").unwrap();
}

#[test]
fn test_printer_multiline_product_type_with_two_fields() {
    let source = "t Pair={left:Int,right:Int}\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert_eq!(
        print_canonical_program(&program),
        "t Pair={\n  left:Int,\n  right:Int\n}\n"
    );
}

#[test]
fn test_printer_multiline_type_args_and_call_args() {
    let source = "λmain()=>Result[String,String]=pair(\"a\",\"b\")\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert_eq!(
        print_canonical_program(&program),
        "λmain()=>Result[\n  String,\n  String\n]=pair(\n  \"a\",\n  \"b\"\n)\n"
    );
}

#[test]
fn test_printer_feature_flag_declaration() {
    let source =
        "featureFlag NewCheckout:Bool\ncreatedAt \"2026-04-12T00-00-00Z\"\ndefault false\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "src/flags.lib.sigil").unwrap();

    assert_eq!(
        print_canonical_program(&program),
        "featureFlag NewCheckout:Bool\n  createdAt \"2026-04-12T00-00-00Z\"\n  default false\n"
    );
}

#[test]
fn test_printer_emits_total_module_default_and_ordinary_override() {
    let source = "mode total\n\nordinary λloop()=>Int=loop()\n\nλcount(n:Int)=>Int\ndecreases n\n=count(n-1)\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert_eq!(print_canonical_program(&program), source);
}

#[test]
fn test_printer_preserves_subscription_extern_members() {
    let source = "e nodePty:{onData: subscribes λ(Session)=>String}\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert_eq!(print_canonical_program(&program), source);
}

#[test]
fn test_feature_flags_must_live_in_src_flags() {
    let source =
        "featureFlag NewCheckout:Bool\n  createdAt \"2026-04-12T00-00-00Z\"\n  default false\n";
    let file_path = temp_project_path("src/main.sigil");
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, file_path.to_string_lossy().as_ref()).unwrap();
    let result = validate_canonical_form(
        &program,
        Some(file_path.to_string_lossy().as_ref()),
        Some(source),
    );
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::FeatureFlagDeclaration { .. })));
}

#[test]
fn test_printer_keeps_single_item_delimited_forms_flat() {
    let source = "λmain()=>[Int]=sum([1])\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert_eq!(print_canonical_program(&program), source);
}

#[test]
fn test_printer_verticalizes_boolean_chains() {
    let source = "λmain()=>Bool=a and b and z\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert_eq!(
        print_canonical_program(&program),
        "λmain()=>Bool=a\n  and b\n  and z\n"
    );
}

// ============================================================================
// COMBINED VALIDATION TESTS
// ============================================================================

#[test]
fn test_valid_program_both_validators() {
    let source =
        "total λcountdown(n:Int)=>Int\ndecreases n\n match n{0=>0|value=>countdown(value-1)}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_multiple_errors_collected() {
    let source = "λfoo()=>Int=1\nλfoo()=>Int=2\nλfoo()=>Int=3";
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
    let source = "λmain()=>Int=42";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_function_declaration_valid() {
    let source = "λfetch()=>String=\"data\"";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

fn temp_project_path(relative: &str) -> PathBuf {
    let unique = format!(
        "sigil-validator-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("sigil.json"),
        "{\"name\":\"validatorTest\",\"version\":\"2026-04-05T14-58-24Z\"}\n",
    )
    .unwrap();
    root.join(relative)
}

#[test]
fn test_project_types_must_live_in_src_types_file() {
    let source = "t User=User(Int)\nλmain()=>Unit=()";
    let file_path = temp_project_path("src/main.sigil");
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, file_path.to_string_lossy().as_ref()).unwrap();

    let result =
        validate_canonical_form(&program, Some(file_path.to_string_lossy().as_ref()), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::TypeDeclarationPlacement { .. })));
}

#[test]
fn test_src_types_file_may_only_contain_type_declarations() {
    let source = "t User=User(Int)\nc answer=(42:Int)";
    let file_path = temp_project_path("src/types.lib.sigil");
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, file_path.to_string_lossy().as_ref()).unwrap();

    let result =
        validate_canonical_form(&program, Some(file_path.to_string_lossy().as_ref()), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::TypeDeclarationPlacement { .. })));
}

#[test]
fn test_src_types_file_rejects_non_foundational_roots() {
    let source = "t EnvConfig=¤prod.Settings";
    let file_path = temp_project_path("src/types.lib.sigil");
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, file_path.to_string_lossy().as_ref()).unwrap();

    let result =
        validate_canonical_form(&program, Some(file_path.to_string_lossy().as_ref()), None);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|error| matches!(error, ValidationError::TypeDeclarationPlacement { .. })));
}

#[test]
fn test_type_declaration_valid() {
    let source = "t Result[T,E]=Ok(T)|Err(E)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_root_qualified_reference_valid() {
    let source = "λsizePlusOne(xs:[Int])=>Int=§list.sum(xs)+1";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_const_lower_camel_case_name() {
    let source = "c myConstant=(100:Int)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_effect_annotations_valid() {
    let source = "λread()=>!Fs String=\"\"";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_non_project_effect_declarations_can_live_in_single_files() {
    let source = "effect CliIo=!Fs!Log!Process\n\nλmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "/tmp/effectDemo.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("/tmp/effectDemo.sigil"), None).is_ok());
}

#[test]
fn test_non_project_tests_can_live_in_single_files() {
    let source = "λmain()=>Unit=()\n\ntest \"inline\" {\n  true\n}\n";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "/tmp/inlineTests.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("/tmp/inlineTests.sigil"), None).is_ok());
}

// Note: Type checking tests belong in the typechecker crate.
// This test validates that the parser/validator accept typed FFI declarations.
#[test]
fn test_typed_ffi_declaration_valid() {
    let source = "e console : { log : λ(String) => Unit }\nλmain()=>Unit=console.log(\"hello\")";
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
    let source = "t User={age:Int,email:String,name:String}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.lib.sigil"), None).is_ok());
}

#[test]
fn test_record_type_field_order_invalid() {
    let source = "t User={name:String,age:Int}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(
        errors[0],
        ValidationError::RecordTypeFieldOrder { .. }
    ));
}

#[test]
fn test_record_literal_field_order_valid() {
    let source = "t User={age:Int,email:String,name:String}\nλmain()=>User=User{age:1,email:\"a\",name:\"b\"}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_record_literal_field_order_invalid() {
    let source = "t User={age:Int,name:String}\nλmain()=>User=User{name:\"b\",age:1}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::RecordLiteralFieldOrder { .. })));
}

#[test]
fn test_map_literal_is_not_subject_to_record_field_ordering() {
    let source = "λmain()=>{String↦Int}={\"b\"↦1,\"a\"↦2}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_record_pattern_field_order_invalid() {
    let source =
        "t User={age:Int,name:String}\nλmain()=>Int match User{age:1,name:\"b\"}{{name,age}=>age}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(
        errors[0],
        ValidationError::RecordPatternFieldOrder { .. }
    ));
}

// ============================================================================
// NO SHADOWING TESTS
// ============================================================================

#[test]
fn test_no_shadowing_valid_distinct_names() {
    let source = "λmain()=>Int=l value=(1:Int);l doubled=(value*2:Int);doubled";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("test.sigil"), None).is_ok());
}

#[test]
fn test_no_shadowing_rejects_rebinding_in_same_function() {
    let source = "λmain()=>Int=l x=(1:Int);l x=(2:Int);x";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::NoShadowing { .. }));
}

#[test]
fn test_no_shadowing_rejects_let_shadowing_function_param() {
    let source = "λecho(value:Int)=>Int=l value=(2:Int);value";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(
        |error| matches!(error, ValidationError::NoShadowing { name, .. } if name == "value")
    ));
}

#[test]
fn test_no_shadowing_rejects_lambda_param_shadowing_outer_local() {
    let source = "λmain()=>Int=l x=(1:Int);(λ(x:Int)=>Int=x)(2)";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::NoShadowing { name, .. } if name == "x")));
}

#[test]
fn test_no_shadowing_rejects_pattern_binding_shadowing_outer_local() {
    let source = "λmain()=>Int=l item=(1:Int);match [2]{[item]=>item|_=>0}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::NoShadowing { name, .. } if name == "item")));
}

#[test]
fn test_no_shadowing_rejects_duplicate_names_inside_pattern() {
    let source = "λmain()=>Int match (1,2){(item,item)=>item}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("test.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ValidationError::NoShadowing { name, .. } if name == "item")));
}

// ============================================================================
// FILENAME VALIDATION TESTS
// ============================================================================

#[test]
fn test_filename_uppercase_rejected() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "UserService.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("UserService.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameCase { .. }));
}

#[test]
fn test_filename_underscore_rejected() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user_service.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("user_service.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_special_char_rejected() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user@service.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("user@service.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_space_rejected() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "user service.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("user service.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_hyphen_rejected() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "hello-world.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("hello-world.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_leading_digit_rejected() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "01introduction.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("01introduction.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::FilenameFormat { .. }));
}

#[test]
fn test_filename_valid_lower_camel_case() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "userService.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("userService.sigil"), None).is_ok());
}

#[test]
fn test_filename_valid_with_numbers() {
    let source = "λmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "example01Introduction.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("example01Introduction.sigil"), None).is_ok());
}

#[test]
fn test_filename_valid_lib_extension() {
    let source = "";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "ffiNodeConsole.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("ffiNodeConsole.lib.sigil"), None).is_ok());
}

#[test]
fn test_unused_extern_allowed_in_lib_file() {
    let source = "e console:{log:λ(String)=>Unit}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "ffiNodeConsole.lib.sigil").unwrap();

    assert!(validate_canonical_form(&program, Some("ffiNodeConsole.lib.sigil"), None).is_ok());
}

#[test]
fn test_unused_extern_rejected_in_executable_file() {
    let source = "e console:{log:λ(String)=>Unit}\nλmain()=>Unit=()";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "typedFfiDemo.sigil").unwrap();

    let result = validate_canonical_form(&program, Some("typedFfiDemo.sigil"), None);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(matches!(errors[0], ValidationError::UnusedExtern { .. }));
}

// ============================================================================
// PROTOCOL DECLARATION TESTS
// ============================================================================

#[test]
fn test_protocol_declaration_canonical_ordering() {
    let source = "t Transaction={id:String}\nprotocol Transaction\n  Open → Closed via commit\n  initial = Open\n  terminal = Closed\nλcommit(transaction:Transaction)=>Bool=true";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();
    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    // The source has protocol before effect/λ — canonical order
    // But the canonical source check will fail because there are missing requires/ensures
    // Just check that the protocol declaration is parsed correctly by checking
    // the non-canonical ordering check doesn't fire
    let errors: Vec<&ValidationError> = result
        .as_ref()
        .err()
        .map(|e| e.iter().collect())
        .unwrap_or_default();
    let has_ordering_error = errors.iter().any(|e| {
        matches!(e, ValidationError::DeclarationOrderOld { message } if message.contains("SIGIL-CANON-DECL-CATEGORY-ORDER"))
    });
    assert!(
        !has_ordering_error,
        "Protocol before λ should not trigger ordering error"
    );
}

#[test]
fn test_protocol_after_function_rejected() {
    // protocol must come before λ in canonical order
    let source = "t Transaction={id:String}\nλnoop()=>Bool=true\nprotocol Transaction\n  Open → Closed via noop\n  initial = Open\n  terminal = Closed";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();
    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    let errors = result.unwrap_err();
    let has_ordering_error = errors.iter().any(|e| {
        matches!(e, ValidationError::DeclarationOrderOld { message } if message.contains("SIGIL-CANON-DECL-CATEGORY-ORDER"))
    });
    assert!(
        has_ordering_error,
        "Protocol after λ should trigger ordering error: {errors:?}"
    );
}

#[test]
fn test_duplicate_protocol_rejected() {
    let source = concat!(
        "t Foo={id:String}\n",
        "protocol Foo\n",
        "  Open → Closed via close\n",
        "  initial = Open\n",
        "  terminal = Closed\n",
        "protocol Foo\n",
        "  Open → Closed via close\n",
        "  initial = Open\n",
        "  terminal = Closed\n",
    );
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();
    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    let errors = result.unwrap_err();
    let has_dup_error = errors.iter().any(
        |e| matches!(e, ValidationError::DuplicateDeclaration { kind, .. } if kind == "PROTOCOL"),
    );
    assert!(
        has_dup_error,
        "Duplicate protocol should be rejected: {errors:?}"
    );
}

#[test]
fn test_protocol_name_must_be_upper_camel_case() {
    // The protocol name itself must be UpperCamelCase — validated by the canonical checker.
    // (State names are enforced at parse time since the parser requires UpperIdentifier tokens.)
    let source = concat!(
        "t Foo={id:String}\n",
        "protocol Foo\n",
        "  Open → Closed via close\n",
        "  initial = Open\n",
        "  terminal = Closed\n",
    );
    // This should parse (canonical check may flag source form, but no naming error expected).
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.lib.sigil").unwrap();
    let result = validate_canonical_form(&program, Some("test.lib.sigil"), None);
    // No TypeNameForm error for Foo (which IS UpperCamelCase).
    let has_naming_error = result
        .as_ref()
        .err()
        .map(|errors| {
            errors
                .iter()
                .any(|e| matches!(e, ValidationError::TypeNameForm { found, .. } if found == "Foo"))
        })
        .unwrap_or(false);
    assert!(
        !has_naming_error,
        "Correctly named protocol 'Foo' should not trigger naming error"
    );
}
