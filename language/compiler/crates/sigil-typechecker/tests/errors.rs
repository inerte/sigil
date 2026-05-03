//! Table-driven error message tests.
//!
//! Each row is (source, expected_fragment_in_error_message). The full lex+parse+typecheck
//! pipeline is run and the error message must contain the expected fragment.

use sigil_diagnostics::codes;
use sigil_lexer::tokenize;
use sigil_parser::parse;
use sigil_typechecker::{type_check, EffectCatalog, TypeCheckOptions};

fn expect_error(source: &str, expected_fragment: &str) {
    expect_error_with_options(source, None, expected_fragment);
}

fn expect_error_with_options(
    source: &str,
    options: Option<TypeCheckOptions>,
    expected_fragment: &str,
) {
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();
    let result = type_check(&program, source, options);
    match result {
        Ok(_) => panic!(
            "Expected error containing '{}' but typecheck succeeded.\nSource: {}",
            expected_fragment, source
        ),
        Err(e) => {
            let full_msg = format!("{} {:?}", e.message, e.details);
            assert!(
                full_msg.contains(expected_fragment),
                "Expected '{}' in error, got: {}",
                expected_fragment,
                full_msg
            );
        }
    }
}

fn expect_error_code(source: &str, expected_code: &str) {
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();
    let result = type_check(&program, source, None);
    let e = result.expect_err(&format!(
        "Expected error code {} but succeeded",
        expected_code
    ));
    assert_eq!(
        e.code, expected_code,
        "Expected code {}, got code {}. Message: {}",
        expected_code, e.code, e.message
    );
}

// ============================================================================
// Type errors
// ============================================================================

#[test]
fn type_mismatch_int_vs_string() {
    expect_error("λbad(x:Int)=>String=x", "Type mismatch");
}

#[test]
fn unbound_variable() {
    expect_error("λbad()=>Int=no_such_variable", "Unbound");
}

#[test]
fn wrong_argument_count() {
    expect_error(
        "λf(x:Int,y:Int)=>Int=x+y\nλbad()=>Int=f(1)",
        "expects 2 arguments",
    );
}

#[test]
fn argument_type_mismatch() {
    expect_error("λf(x:Int)=>Int=x\nλbad()=>Int=f(true)", "type mismatch");
}

// ============================================================================
// Match exhaustiveness
// ============================================================================

#[test]
fn non_exhaustive_bool_match() {
    expect_error_code(
        "λbad(b:Bool)=>Int match b{true=>1}",
        codes::typecheck::MATCH_NON_EXHAUSTIVE,
    );
}

#[test]
fn non_exhaustive_sum_match() {
    expect_error_code(
        "t Coin=Heads()|Tails()\nλbad(coin:Coin)=>Int match coin{Heads()=>1}",
        codes::typecheck::MATCH_NON_EXHAUSTIVE,
    );
}

// ============================================================================
// Requires/ensures violations
// ============================================================================

#[test]
fn requires_violated_at_call_site() {
    expect_error(
        "λpos(n:Int)=>Int\nrequires n>0\n=n\nλbad()=>Int=pos(-5)",
        "requires clause",
    );
}

#[test]
fn ensures_cannot_be_proven() {
    expect_error("λbad(n:Int)=>Int\nensures result>n\n=n", "ensures clause");
}

// ============================================================================
// Effect errors
// ============================================================================

#[test]
fn missing_effect_annotation() {
    expect_error(
        "e console:{log:λ(String)=>!Log Unit}\nλbad()=>Unit=console.log(\"hi\")",
        "missing declared effects",
    );
}

#[test]
fn unused_effect_annotation_on_function() {
    expect_error("λbad()=>!Log Unit=()", "unused declared effects");
}

#[test]
fn unused_effect_annotation_on_lambda() {
    expect_error(
        "λbad()=>Unit=(λ()=>!Log Unit=())()",
        "unused declared effects",
    );
}

#[test]
fn unused_effect_annotation_on_transform() {
    expect_error(
        "transform λbad(value:Int)=>!Log Int=value",
        "unused declared effects",
    );
}

#[test]
fn unused_effect_annotation_on_test() {
    expect_error(
        "λmain()=>Unit=()\ntest \"unused effect\" =>!Log { true }",
        "unused declared effects",
    );
}

#[test]
fn alias_overdeclaration_is_rejected() {
    expect_error(
        "effect CliIo=!Fs!Log\ne console:{log:λ(String)=>!Log Unit}\nλbad()=>!CliIo Unit=console.log(\"hi\")",
        "unused declared effects",
    );
}

#[test]
fn alias_overdeclaration_is_rejected_with_injected_effect_catalog() {
    let source =
        "effect CliIo=!Fs!Log\ne console:{log:λ(String)=>!Log Unit}\nλbad()=>!CliIo Unit=console.log(\"hi\")";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();
    let effect_catalog = EffectCatalog::from_program(&program).unwrap();

    expect_error_with_options(
        source,
        Some(TypeCheckOptions {
            effect_catalog: Some(effect_catalog),
            ..TypeCheckOptions::default()
        }),
        "unused declared effects",
    );
}

#[test]
fn self_recursion_does_not_justify_effects() {
    expect_error("λbad()=>!Log Unit=bad()", "unused declared effects");
}

// ============================================================================
// Protocol type errors
// ============================================================================

#[test]
fn protocol_unknown_type() {
    expect_error(
        "protocol Ghost\n  Open → Closed via foo\n  initial = Open\n  terminal = Closed\nλmain()=>Bool=true",
        "SIGIL-PROTO-UNKNOWN-TYPE",
    );
}

#[test]
fn protocol_state_violation() {
    expect_error(
        concat!(
            "t Ticket={id:String}\n",
            "protocol Ticket\n",
            "  Open → Closed via close\n",
            "  initial = Open\n",
            "  terminal = Closed\n",
            "λclose(ticket:Ticket)=>Bool\n",
            "requires ticket.state=Open\n",
            "ensures ticket.state=Closed\n",
            "=true\n",
            "λbad(ticket:Ticket)=>Bool={\n",
            "  l _=(close(ticket):Bool);\n",
            "  close(ticket)\n",
            "}",
        ),
        "requires clause",
    );
}

#[test]
fn derive_json_generic_root_rejected() {
    expect_error(
        "t Box[T]=Box(T)\nderive json Box",
        "must be monomorphic; generic type declarations are not supported as public derive roots in v1",
    );
}

#[test]
fn derive_json_non_string_map_keys_rejected() {
    expect_error(
        "t Bad={values:{Int↦String}}\nderive json Bad",
        "derive json only supports maps with String keys",
    );
}

#[test]
fn derive_json_ambiguous_option_payload_rejected() {
    expect_error(
        concat!(
            "t MaybeInt=MaybeInt(Option[Int])\n",
            "t Bad={payload:Option[MaybeInt]}\n",
            "derive json Bad",
        ),
        "derive json rejects Option payloads whose canonical encoding can already be null",
    );
}
