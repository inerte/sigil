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

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = repo_root().join("target").join(format!(
        "sigil-cli-init-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn external_temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = repo_root().join(format!(
        "sigil-cli-init-external-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

fn is_canonical_timestamp(value: &str) -> bool {
    if value.len() != 20 {
        return false;
    }
    let bytes = value.as_bytes();
    const DIGIT_POSITIONS: [usize; 14] = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    DIGIT_POSITIONS
        .iter()
        .all(|index| bytes[*index].is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b'-'
        && bytes[16] == b'-'
        && bytes[19] == b'Z'
}

#[test]
fn init_without_path_creates_neutral_project_in_current_directory() {
    let workspace = external_temp_dir("current");
    let target = workspace.join("neutral-project");
    fs::create_dir_all(&target).unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(&target)
        .arg("init")
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    let canonical_target = fs::canonicalize(&target).unwrap();
    assert_eq!(json["command"], "sigil init");
    assert_eq!(json["ok"], true);
    assert_eq!(json["phase"], "cli");
    assert_eq!(
        json["data"]["root"],
        canonical_target.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["manifest"]["name"], "neutralProject");
    let version = json["data"]["manifest"]["version"].as_str().unwrap();
    assert!(is_canonical_timestamp(version));

    let manifest_text = fs::read_to_string(target.join("sigil.json")).unwrap();
    let manifest: Value = serde_json::from_str(&manifest_text).unwrap();
    assert_eq!(manifest["name"], "neutralProject");
    assert_eq!(manifest["version"], version);
    assert_eq!(manifest.as_object().unwrap().len(), 2);

    assert!(target.join("src").is_dir());
    assert!(target.join("tests").is_dir());
    assert!(target.join(".local").is_dir());
}

#[test]
fn init_with_path_creates_target_directory_and_derives_name() {
    let workspace = temp_dir("path");
    let target = workspace.join("hello-world");

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("hello-world")
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let json = parse_json(&output.stdout);
    assert_eq!(json["data"]["manifest"]["name"], "helloWorld");
    assert_eq!(
        json["data"]["root"],
        fs::canonicalize(&target)
            .unwrap()
            .to_string_lossy()
            .to_string()
    );
    assert!(target.join("src").is_dir());
    assert!(target.join("tests").is_dir());
    assert!(target.join(".local").is_dir());
}

#[test]
fn init_rejects_invalid_target_name() {
    let workspace = temp_dir("invalid-name");

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("123-demo")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-PROJECT-INIT-INVALID-NAME");
    assert_eq!(json["error"]["details"]["rawName"], "123-demo");
    assert!(!workspace.join("123-demo").exists());
}

#[test]
fn init_rejects_non_empty_target_directory() {
    let workspace = temp_dir("non-empty");
    let target = workspace.join("existing-project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("README.md"), "existing\n").unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("existing-project")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-PROJECT-INIT-CONFLICT");
    assert_eq!(json["error"]["details"]["existingEntries"][0], "README.md");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("must be empty"));
    assert!(!target.join("sigil.json").exists());
}

#[test]
fn init_rejects_target_with_existing_manifest() {
    let workspace = temp_dir("existing-manifest");
    let target = workspace.join("existing-project");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join("sigil.json"),
        "{\n  \"name\": \"existingProject\",\n  \"version\": \"2026-04-15T00-00-00Z\"\n}\n",
    )
    .unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("existing-project")
        .output()
        .unwrap();

    assert!(!output.status.success());

    let json = parse_json(&output.stdout);
    assert_eq!(json["error"]["code"], "SIGIL-CLI-PROJECT-INIT-CONFLICT");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("sigil.json"));
    assert_eq!(json["error"]["details"]["existingEntries"][0], "sigil.json");
}
