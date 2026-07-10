use jsonschema::Validator;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_path_buf()
}

fn sigil_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sigil"))
}

fn schema() -> Validator {
    let text = fs::read_to_string(repo_root().join("language/spec/cli-json.schema.json")).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    jsonschema::validator_for(&value).unwrap()
}

fn parse_and_validate(output: &std::process::Output) -> Value {
    assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let errors = schema()
        .iter_errors(&value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {}", errors.join("\n"));
    value
}

#[test]
fn capabilities_reports_the_canonical_machine_interface() {
    let output = Command::new(sigil_bin())
        .arg("capabilities")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = parse_and_validate(&output);
    assert_eq!(value["formatVersion"], 1);
    assert_eq!(value["command"], "sigil capabilities");
    assert_eq!(value["analysis"]["status"], "notApplicable");
    assert!(value["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["name"] == "sigil inspect trust"));
}

#[test]
fn invalid_cli_usage_is_a_canonical_diagnostic() {
    let output = Command::new(sigil_bin())
        .arg("does-not-exist")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value = parse_and_validate(&output);
    assert_eq!(value["command"], "sigil");
    assert_eq!(value["diagnostics"][0]["code"], "SIGIL-CLI-USAGE");
    assert_eq!(value["diagnostics"][0]["severity"], "error");
}

#[test]
fn inspect_trust_reports_typed_externs_and_uses() {
    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .args(["inspect", "trust", "language/examples/typedFfiDemo.sigil"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value = parse_and_validate(&output);
    assert_eq!(value["command"], "sigil inspect trust");
    assert_eq!(value["analysis"]["level"], "typed");
    let assumptions = value["data"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|file| file["assumptions"].as_array().unwrap())
        .collect::<Vec<_>>();
    let console = assumptions
        .iter()
        .find(|item| item["kind"] == "extern" && item["namespace"] == "console")
        .unwrap();
    assert_eq!(console["trustMode"], "typed");
    assert_eq!(console["uses"].as_array().unwrap().len(), 1);
}

#[test]
fn inspect_trust_preserves_independent_results_on_partial_failure() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = repo_root().join("target").join(format!(
        "sigil-trust-partial-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("good.sigil"), "λmain()=>Unit=()\n").unwrap();
    fs::write(directory.join("bad.sigil"), "λmain(\n").unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("trust")
        .arg(&directory)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&directory);

    assert!(!output.status.success());
    let value = parse_and_validate(&output);
    assert_eq!(value["analysis"]["status"], "partial");
    assert_eq!(value["analysis"]["level"], "mixed");
    assert_eq!(value["data"]["summary"]["analyzedFiles"], 1);
    let files = value["data"]["files"].as_array().unwrap();
    assert!(files
        .iter()
        .any(|file| file["path"].as_str().unwrap().ends_with("good.sigil")));
    assert!(!files
        .iter()
        .any(|file| file["path"].as_str().unwrap().ends_with("bad.sigil")));
    assert_eq!(value["diagnostics"].as_array().unwrap().len(), 1);
}
