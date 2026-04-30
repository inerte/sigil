//! Integration tests for the full lex → parse → validate → typecheck pipeline.
//!
//! Each test runs a Sigil source string through all compiler phases and asserts
//! the expected outcome. Success cases verify the pipeline accepts valid programs;
//! error cases verify specific error codes or message fragments.

use sigil_diagnostics::codes;
use sigil_lexer::tokenize;
use sigil_parser::parse;
use sigil_typechecker::type_check;

/// Run lex + parse + typecheck on a source string.
fn pipeline(source: &str) -> Result<sigil_typechecker::TypeCheckResult, String> {
    let tokens = tokenize(source).map_err(|e| format!("lex error: {:?}", e))?;
    let program = parse(tokens, "test.lib.sigil").map_err(|e| format!("parse error: {:?}", e))?;
    type_check(&program, source, None)
        .map_err(|e| format!("typecheck error: {} {:?}", e.message, e.details))
}

/// Run parse + typecheck only (skip canonical validation for non-canonical test sources).
fn typecheck_only(
    source: &str,
) -> Result<sigil_typechecker::TypeCheckResult, sigil_typechecker::TypeError> {
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();
    type_check(&program, source, None)
}

// ============================================================================
// Success cases — one per major language feature
// ============================================================================

#[test]
fn pipeline_integer_arithmetic() {
    let r = pipeline("λadd(x:Int,y:Int)=>Int=x+y");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_sum_type_match() {
    let r = typecheck_only(
        "t Color=Red()|Green()|Blue()\nλname(color:Color)=>String match color{\n  Red()=>\"red\"|\n  Green()=>\"green\"|\n  Blue()=>\"blue\"\n}",
    );
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_named_product_type() {
    let r = pipeline("t Point={x:Int,y:Int}\nλorigin()=>Point={x:0,y:0}");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_generic_function() {
    let r = pipeline("λidentity[T](value:T)=>T=value");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_effect_annotation() {
    let r = typecheck_only(
        "e console:{log:λ(String)=>!Log Unit}\nλhello()=>!Log Unit=console.log(\"hi\")",
    );
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_requires_ensures() {
    let r = typecheck_only("λpos(n:Int)=>Int\nrequires n>0\nensures result>0\n=n");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_total_function_cannot_call_ordinary_function() {
    let r = typecheck_only("mode total\n\nordinary λhelper()=>Int=1\n\nλmain()=>Int=helper()");
    let e = r.unwrap_err();
    assert!(
        e.message
            .contains("Total functions cannot call ordinary function"),
        "Expected total-to-ordinary call rejection: {}",
        e.message
    );
    assert_eq!(
        e.details
            .as_ref()
            .and_then(|d| d.get("functionMode"))
            .and_then(|v| v.as_str()),
        Some("total")
    );
}

#[test]
fn pipeline_total_function_allows_shadowed_local_function_value() {
    let r = typecheck_only(
        "mode total\n\nordinary λhelper()=>Int=1\n\nλcall(helper:λ()=>Int)=>Int=helper()",
    );
    assert!(
        r.is_ok(),
        "Shadowed local function values should not inherit top-level mode: {r:?}"
    );
}

#[test]
fn pipeline_transform_uses_requires_context_with_decreases() {
    let r = typecheck_only(
        "total λhead(xs:[Int])=>Int\nrequires #xs>0\n=0\n\ntransform total λuse(xs:[Int])=>Int\nrequires #xs>0\ndecreases #xs\n=head(xs)",
    );
    assert!(
        r.is_ok(),
        "Transforms should typecheck under their requires context even when they carry decreases: {r:?}"
    );
}

#[test]
fn pipeline_match_exhaustiveness() {
    let r = typecheck_only("λtoggle(b:Bool)=>Bool match b{true=>false|false=>true}");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_list_operations() {
    let r = typecheck_only("λdouble(xs:[Int])=>[Int]=xs map λ(x:Int)=>Int=x*2");
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn pipeline_protocol_state_valid() {
    let r = typecheck_only(concat!(
        "t Handle={id:String}\n",
        "protocol Handle\n",
        "  Open → Closed via close\n",
        "  initial = Open\n",
        "  terminal = Closed\n",
        "λclose(handle:Handle)=>Bool\n",
        "requires handle.state=Open\n",
        "ensures handle.state=Closed\n",
        "=true\n",
        "λmain()=>Bool={l h=({id:\"x\"}:Handle);close(h)}",
    ));
    assert!(r.is_ok(), "Expected protocol state valid: {r:?}");
}

#[test]
fn pipeline_constrained_type() {
    let r = typecheck_only("t Score=Int where value≥0 and value≤100\nλperfect()=>Score=100");
    assert!(r.is_ok(), "{r:?}");
}

// ============================================================================
// Error cases — verify specific error codes/messages
// ============================================================================

#[test]
fn pipeline_type_mismatch_rejected() {
    let r = typecheck_only("λbad(x:Int)=>String=x");
    assert!(r.is_err(), "Expected type mismatch error");
}

#[test]
fn pipeline_unbound_variable_rejected() {
    let r = typecheck_only("λbad()=>Int=undefined_var");
    let e = r.unwrap_err();
    assert!(
        e.message.contains("Unbound") || e.message.contains("unbound"),
        "Expected unbound error, got: {}",
        e.message
    );
}

#[test]
fn pipeline_requires_violation_rejected() {
    let r = typecheck_only(
        "λpositiveOnly(value:Int)=>Int\nrequires value>0\n=value\nλmain()=>Int=positiveOnly(0)",
    );
    let e = r.unwrap_err();
    assert!(
        e.message.contains("requires clause"),
        "Expected requires violation: {}",
        e.message
    );
    assert_eq!(
        e.details
            .as_ref()
            .and_then(|d| d.get("proofKind"))
            .and_then(|v| v.as_str()),
        Some("requires")
    );
}

#[test]
fn pipeline_non_exhaustive_match_rejected() {
    let r = typecheck_only("λbad(b:Bool)=>String match b{true=>\"yes\"}");
    let e = r.unwrap_err();
    assert_eq!(e.code, codes::typecheck::MATCH_NON_EXHAUSTIVE);
}

#[test]
fn pipeline_effect_missing_rejected() {
    let r =
        typecheck_only("e console:{log:λ(String)=>!Log Unit}\nλbad()=>Unit=console.log(\"hi\")");
    assert!(r.is_err(), "Expected missing effect error");
}

#[test]
fn pipeline_protocol_unknown_type_rejected() {
    let r = typecheck_only(
        "protocol Ghost\n  Open → Closed via foo\n  initial = Open\n  terminal = Closed\nλmain()=>Bool=true",
    );
    let e = r.unwrap_err();
    assert!(
        e.message.contains("SIGIL-PROTO-UNKNOWN-TYPE"),
        "Expected SIGIL-PROTO-UNKNOWN-TYPE: {}",
        e.message
    );
}
