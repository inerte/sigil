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
        "sigil-cli-feature-flag-{label}-{}-{unique}",
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
fn feature_flag_audit_lists_flags_and_filters_by_age() {
    let dir = temp_dir("audit");
    write_program(
        &dir,
        "src/flags.lib.sigil",
        concat!(
            "featureFlag CheckoutColorChoice:Bool\n",
            "  createdAt \"2099-01-01T00-00-00Z\"\n",
            "  default false\n\n",
            "featureFlag NewCheckout:Bool\n",
            "  createdAt \"2020-01-01T00-00-00Z\"\n",
            "  default false\n",
        ),
    );

    let output = Command::new(sigil_bin())
        .current_dir(repo_root())
        .arg("featureFlag")
        .arg("audit")
        .arg(&dir)
        .arg("--older-than")
        .arg("1000d")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let json = parse_json(&output.stdout);
    assert_eq!(json["command"], "sigil featureFlag audit");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["summary"]["flags"], 2);
    assert_eq!(json["data"]["summary"]["matched"], 1);
    assert_eq!(json["data"]["summary"]["olderThanDays"], 1000);
    let flags = json["data"]["flags"].as_array().unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0]["name"], "NewCheckout");
}
