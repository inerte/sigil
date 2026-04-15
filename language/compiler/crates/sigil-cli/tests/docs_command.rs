use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn sigil_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sigil"))
}

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

#[test]
fn help_surfaces_include_docs_command() {
    let help_output = Command::new(sigil_bin()).arg("--help").output().unwrap();
    assert!(help_output.status.success(), "{help_output:?}");
    let help_stdout = String::from_utf8(help_output.stdout).unwrap();
    assert!(help_stdout.contains("docs"));

    let subcommand_help = Command::new(sigil_bin()).arg("help").output().unwrap();
    assert!(subcommand_help.status.success(), "{subcommand_help:?}");
    let subcommand_stdout = String::from_utf8(subcommand_help.stdout).unwrap();
    assert!(subcommand_stdout.contains("docs"));
}

#[test]
fn docs_help_lists_retrieval_subcommands() {
    let output = Command::new(sigil_bin())
        .arg("docs")
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("list"));
    assert!(stdout.contains("search"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("context"));
}

#[test]
fn docs_list_returns_guides_docs_specs_and_articles() {
    let output = Command::new(sigil_bin())
        .arg("docs")
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigil docs list");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "docs");

    let documents = json["data"]["documents"].as_array().unwrap();
    assert_eq!(documents[0]["docId"], "guide/root-readme");
    assert_eq!(documents[1]["docId"], "guide/language-readme");
    assert!(documents
        .iter()
        .any(|document| document["docId"] == "docs/syntax-reference"));
    assert!(documents
        .iter()
        .any(|document| document["docId"] == "spec/grammar"));
    assert!(documents
        .iter()
        .any(|document| document["docId"] == "article/packages-use-npm-as-transport"));
}

#[test]
fn docs_show_returns_numbered_lines_and_respects_ranges() {
    let full_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("show")
        .arg("docs/syntax-reference")
        .output()
        .unwrap();

    assert!(full_output.status.success(), "{full_output:?}");
    let full_json = parse_json(&full_output.stdout);
    assert_eq!(full_json["data"]["document"]["docId"], "docs/syntax-reference");
    assert_eq!(full_json["data"]["document"]["title"], "Sigil Syntax Reference");
    assert_eq!(
        full_json["data"]["document"]["path"],
        "language/docs/syntax-reference.md"
    );
    let lines = full_json["data"]["document"]["lines"].as_array().unwrap();
    assert!(!lines.is_empty());
    assert_eq!(lines[0]["line"], 1);
    assert_eq!(lines[0]["section"], "Sigil Syntax Reference");

    let line_count = full_json["data"]["document"]["lineCount"].as_u64().unwrap();
    let ranged_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("show")
        .arg("docs/syntax-reference")
        .arg("--start-line")
        .arg("2")
        .arg("--end-line")
        .arg("999999")
        .output()
        .unwrap();

    assert!(ranged_output.status.success(), "{ranged_output:?}");
    let ranged_json = parse_json(&ranged_output.stdout);
    let ranged_lines = ranged_json["data"]["document"]["lines"].as_array().unwrap();
    assert_eq!(ranged_json["data"]["range"]["startLine"], 2);
    assert_eq!(ranged_json["data"]["range"]["endLine"], line_count);
    assert_eq!(ranged_lines.first().unwrap()["line"], 2);
    assert_eq!(ranged_lines.last().unwrap()["line"], line_count);
}

#[test]
fn docs_search_returns_context_windows_and_ranks_guides_first() {
    let output = Command::new(sigil_bin())
        .arg("docs")
        .arg("search")
        .arg("syntax reference")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigil docs search");
    let results = json["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0]["docId"], "guide/language-readme");
    assert_eq!(results[0]["kind"], "guide");
    assert_eq!(results[0]["isExactPhrase"], true);
    assert!(results.iter().any(|result| result["docId"] == "docs/syntax-reference"));

    let windowed_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("search")
        .arg("feature flags")
        .output()
        .unwrap();

    assert!(windowed_output.status.success(), "{windowed_output:?}");
    let windowed_json = parse_json(&windowed_output.stdout);
    let windowed_results = windowed_json["data"]["results"].as_array().unwrap();
    assert!(!windowed_results.is_empty());
    let first = &windowed_results[0];
    assert!(first["before"].as_array().unwrap().len() <= 2);
    assert_eq!(first["match"].as_array().unwrap().len(), 1);
    assert!(first["after"].as_array().unwrap().len() <= 2);
}

#[test]
fn docs_context_lists_and_shows_curated_bundles() {
    let list_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("context")
        .arg("--list")
        .output()
        .unwrap();

    assert!(list_output.status.success(), "{list_output:?}");
    let list_json = parse_json(&list_output.stdout);
    let contexts = list_json["data"]["contexts"].as_array().unwrap();
    assert!(contexts.iter().any(|context| context["id"] == "packages"));
    assert!(contexts
        .iter()
        .any(|context| context["id"] == "feature-flags"));

    let show_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("context")
        .arg("packages")
        .output()
        .unwrap();

    assert!(show_output.status.success(), "{show_output:?}");
    let show_json = parse_json(&show_output.stdout);
    assert_eq!(show_json["data"]["context"]["id"], "packages");
    let included_docs = show_json["data"]["context"]["includedDocs"]
        .as_array()
        .unwrap();
    assert!(included_docs
        .iter()
        .any(|document| document["docId"] == "docs/packages"));
    assert!(included_docs
        .iter()
        .any(|document| document["docId"] == "spec/packages"));
    assert!(included_docs.iter().any(|document| {
        document["docId"] == "article/packages-use-npm-as-transport"
    }));
}

#[test]
fn docs_invalid_inputs_return_explicit_error_codes() {
    let missing_doc_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("show")
        .arg("docs/does-not-exist")
        .output()
        .unwrap();
    assert!(!missing_doc_output.status.success());
    let missing_doc_json = parse_json(&missing_doc_output.stdout);
    assert_eq!(missing_doc_json["error"]["code"], "SIGIL-CLI-DOC-NOT-FOUND");

    let missing_context_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("context")
        .arg("does-not-exist")
        .output()
        .unwrap();
    assert!(!missing_context_output.status.success());
    let missing_context_json = parse_json(&missing_context_output.stdout);
    assert_eq!(
        missing_context_json["error"]["code"],
        "SIGIL-CLI-DOC-CONTEXT-NOT-FOUND"
    );

    let invalid_range_output = Command::new(sigil_bin())
        .arg("docs")
        .arg("show")
        .arg("docs/syntax-reference")
        .arg("--start-line")
        .arg("0")
        .output()
        .unwrap();
    assert!(!invalid_range_output.status.success());
    let invalid_range_json = parse_json(&invalid_range_output.stdout);
    assert_eq!(
        invalid_range_json["error"]["code"],
        "SIGIL-CLI-DOC-INVALID-LINE-RANGE"
    );
}
