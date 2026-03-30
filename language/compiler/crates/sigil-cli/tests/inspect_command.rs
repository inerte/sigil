use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
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

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = repo_root().join("target").join(format!(
        "sigil-cli-inspect-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_program(dir: &Path, name: &str, source: &str) -> PathBuf {
    let file = dir.join(name);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file, source).unwrap();
    file
}

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

#[test]
fn inspect_types_reports_top_level_types_and_spans() {
    let dir = temp_dir("types-single");
    let file = write_program(
        &dir,
        "generic.lib.sigil",
        "c answer=(41:Int)\n\nλidentity[T](x:T)=>T=x\n",
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect types");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "typecheck");
    assert_eq!(json["data"]["summary"]["functions"], 1);
    assert_eq!(json["data"]["summary"]["consts"], 1);

    let declarations = json["data"]["declarations"].as_array().unwrap();
    assert_eq!(declarations.len(), 2);
    let identity = declarations
        .iter()
        .find(|declaration| declaration["name"] == "identity")
        .unwrap();
    let answer = declarations
        .iter()
        .find(|declaration| declaration["name"] == "answer")
        .unwrap();
    assert_eq!(identity["type"], "∀T. (T) => T");
    assert!(!identity["spanId"].as_str().unwrap().is_empty());
    assert_eq!(identity["location"]["start"]["line"], 3);
    assert_eq!(answer["type"], "Int");
}

#[test]
fn inspect_types_directory_reports_requested_modules_only() {
    let dir = temp_dir("types-directory");
    write_program(
        &dir,
        "sigil.json",
        "{\"name\":\"inspect-types\",\"version\":\"0.1.0\"}\n",
    );
    let helper = write_program(&dir, "src/helper.lib.sigil", "λdouble(x:Int)=>Int=x*2\n");
    let main = write_program(&dir, "src/main.sigil", "λmain()=>Int=•helper.double(21)\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect types");
    assert_eq!(json["data"]["summary"]["discovered"], 2);
    assert_eq!(json["data"]["summary"]["inspected"], 2);

    let files = json["data"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    let main_result = files
        .iter()
        .find(|entry| entry["input"] == main.to_string_lossy().to_string())
        .unwrap();
    let helper_result = files
        .iter()
        .find(|entry| entry["input"] == helper.to_string_lossy().to_string())
        .unwrap();

    assert_eq!(main_result["moduleId"], "src::main");
    assert_eq!(main_result["declarations"].as_array().unwrap().len(), 1);
    assert_eq!(main_result["declarations"][0]["name"], "main");
    assert_eq!(main_result["declarations"][0]["type"], "() => Int");
    assert_eq!(helper_result["moduleId"], "src::helper");
    assert_eq!(helper_result["declarations"].as_array().unwrap().len(), 1);
    assert_eq!(helper_result["declarations"][0]["name"], "double");
}

#[test]
fn inspect_types_emits_json_error_on_type_failure() {
    let dir = temp_dir("types-error");
    let file = write_program(&dir, "broken.sigil", "λmain()=>Int=\"oops\"\n");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("types")
        .arg(&file)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect types");
    assert_eq!(json["ok"], false);
    assert_eq!(json["phase"], "typecheck");
}

#[test]
fn inspect_validate_returns_canonical_source_for_noncanonical_input() {
    let dir = temp_dir("validate-single");
    let file = write_program(&dir, "main.sigil", "λmain()=>Int=1");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("validate")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect validate");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["alreadyCanonical"], false);
    assert_eq!(json["data"]["validation"]["ok"], false);
    assert_eq!(json["data"]["canonicalSource"], "λmain()=>Int=1\n");
    assert!(!json["data"]["validation"]["errors"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn inspect_validate_directory_reports_per_file_status() {
    let dir = temp_dir("validate-directory");
    let canonical = write_program(&dir, "ok.sigil", "λmain()=>Int=1\n");
    let noncanonical = write_program(&dir, "no_newline.sigil", "λmain()=>Int=2");

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("inspect")
        .arg("validate")
        .arg(&dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigilc inspect validate");
    assert_eq!(json["data"]["summary"]["discovered"], 2);
    assert_eq!(json["data"]["summary"]["inspected"], 2);

    let files = json["data"]["files"].as_array().unwrap();
    let canonical_result = files
        .iter()
        .find(|entry| entry["input"] == canonical.to_string_lossy().to_string())
        .unwrap();
    let noncanonical_result = files
        .iter()
        .find(|entry| entry["input"] == noncanonical.to_string_lossy().to_string())
        .unwrap();

    assert_eq!(canonical_result["alreadyCanonical"], true);
    assert_eq!(canonical_result["validation"]["ok"], true);
    assert_eq!(noncanonical_result["alreadyCanonical"], false);
    assert_eq!(noncanonical_result["validation"]["ok"], false);
    assert_eq!(noncanonical_result["canonicalSource"], "λmain()=>Int=2\n");
}
