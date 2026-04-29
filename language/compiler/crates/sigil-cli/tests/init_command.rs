use serde_json::Value;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(path: PathBuf) -> Self {
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl AsRef<Path> for TestDir {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

impl Deref for TestDir {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.path.as_path()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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

fn external_temp_dir(label: &str) -> TestDir {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let parent = repo_root().join("tmp").join("sigil-cli-tests");
    fs::create_dir_all(&parent).unwrap();
    let dir = parent.join(format!(
        "init-external-{label}-{}-{unique}",
        std::process::id()
    ));
    TestDir::new(dir)
}

fn parse_json(text: &[u8]) -> Value {
    serde_json::from_slice(text).unwrap()
}

fn gitignore_text(target: &std::path::Path) -> String {
    fs::read_to_string(target.join(".gitignore")).unwrap()
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
    assert_eq!(
        json["data"]["created"],
        serde_json::json!(["src", "tests", ".local", ".gitignore", "sigil.json"])
    );

    let manifest_text = fs::read_to_string(target.join("sigil.json")).unwrap();
    let manifest: Value = serde_json::from_str(&manifest_text).unwrap();
    assert_eq!(manifest["name"], "neutralProject");
    assert_eq!(manifest["version"], version);
    assert_eq!(manifest.as_object().unwrap().len(), 2);

    assert!(target.join("src").is_dir());
    assert!(target.join("tests").is_dir());
    assert!(target.join(".local").is_dir());
    assert_eq!(gitignore_text(&target), ".local/\n");
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
        json["data"]["created"],
        serde_json::json!(["src", "tests", ".local", ".gitignore", "sigil.json"])
    );
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
    assert_eq!(gitignore_text(&target), ".local/\n");
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
fn init_allows_non_empty_target_directory_with_unrelated_files() {
    let workspace = temp_dir("non-empty-allowed");
    let target = workspace.join("existing-project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("README.md"), "existing\n").unwrap();
    fs::create_dir_all(target.join(".git")).unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("existing-project")
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(
        json["data"]["created"],
        serde_json::json!(["src", "tests", ".local", ".gitignore", "sigil.json"])
    );
    assert!(target.join("README.md").is_file());
    assert!(target.join(".git").is_dir());
    assert!(target.join("sigil.json").is_file());
    assert!(target.join("src").is_dir());
    assert!(target.join("tests").is_dir());
    assert!(target.join(".local").is_dir());
    assert_eq!(gitignore_text(&target), ".local/\n");
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

#[test]
fn init_rejects_target_with_scaffold_file_conflict() {
    let workspace = temp_dir("scaffold-file-conflict");
    let target = workspace.join("existing-project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("src"), "not-a-directory\n").unwrap();

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
        .contains("non-directory scaffold path `src`"));
    assert_eq!(json["error"]["details"]["existingEntries"][0], "src");
}

#[test]
fn init_reuses_existing_scaffold_directories() {
    let workspace = temp_dir("reuse-scaffold-dirs");
    let target = workspace.join("existing-project");
    fs::create_dir_all(target.join("src")).unwrap();
    fs::create_dir_all(target.join("tests")).unwrap();
    fs::create_dir_all(target.join(".local")).unwrap();
    fs::write(target.join("README.md"), "existing\n").unwrap();
    fs::write(target.join(".gitignore"), "/.local/\n").unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("existing-project")
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["created"], serde_json::json!(["sigil.json"]));
    assert!(target.join("sigil.json").is_file());
    assert_eq!(gitignore_text(&target), "/.local/\n");
}

#[test]
fn init_appends_local_to_existing_gitignore_when_only_comment_or_negation_exists() {
    let workspace = temp_dir("append-gitignore");
    let target = workspace.join("existing-project");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join(".gitignore"), "# .local/\n!.local/\ndist/\n").unwrap();

    let output = Command::new(sigil_bin())
        .current_dir(&workspace)
        .arg("init")
        .arg("existing-project")
        .output()
        .unwrap();

    assert!(output.status.success(), "{:?}", output);

    let json = parse_json(&output.stdout);
    assert_eq!(json["ok"], true);
    assert_eq!(
        json["data"]["created"],
        serde_json::json!(["src", "tests", ".local", "sigil.json"])
    );
    assert_eq!(
        gitignore_text(&target),
        "# .local/\n!.local/\ndist/\n.local/\n"
    );
}

#[test]
fn init_rejects_target_with_directory_gitignore_conflict() {
    let workspace = temp_dir("gitignore-dir-conflict");
    let target = workspace.join("existing-project");
    fs::create_dir_all(target.join(".gitignore")).unwrap();

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
        .contains("non-file scaffold path `.gitignore`"));
    assert_eq!(json["error"]["details"]["existingEntries"][0], ".gitignore");
}
