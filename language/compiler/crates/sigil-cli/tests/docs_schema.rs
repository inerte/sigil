use jsonschema::Validator;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

fn cli_schema() -> Validator {
    let schema_path = repo_root().join("language/spec/cli-json.schema.json");
    let schema_text = fs::read_to_string(schema_path).unwrap();
    let schema_json: Value = serde_json::from_str(&schema_text).unwrap();
    jsonschema::validator_for(&schema_json).unwrap()
}

fn assert_schema_valid(schema: &Validator, instance: &Value) {
    let rendered = schema
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    if !rendered.is_empty() {
        panic!("schema validation failed:\n{}", rendered.join("\n"));
    }
}

#[test]
fn docs_json_outputs_match_cli_schema() {
    let schema = cli_schema();

    let commands = [
        vec!["docs", "list"],
        vec!["docs", "search", "syntax reference"],
        vec![
            "docs",
            "show",
            "docs/syntax-reference",
            "--start-line",
            "1",
            "--end-line",
            "3",
        ],
        vec!["docs", "context", "--list"],
        vec!["docs", "context", "packages"],
    ];

    for args in commands {
        let output = Command::new(sigil_bin()).args(&args).output().unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        let json = parse_json(&output.stdout);
        assert_schema_valid(&schema, &json);
    }

    let failure = Command::new(sigil_bin())
        .args(["docs", "show", "docs/does-not-exist"])
        .output()
        .unwrap();
    assert!(!failure.status.success());
    let failure_json = parse_json(&failure.stdout);
    assert_schema_valid(&schema, &failure_json);
}
