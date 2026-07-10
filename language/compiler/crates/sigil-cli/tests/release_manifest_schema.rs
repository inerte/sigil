use jsonschema::Validator;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .unwrap()
        .to_path_buf()
}

fn schema() -> Validator {
    let text =
        fs::read_to_string(repo_root().join("language/spec/release-manifest.schema.json")).unwrap();
    let value = serde_json::from_str(&text).unwrap();
    jsonschema::validator_for(&value).unwrap()
}

#[test]
fn release_manifest_v1_accepts_the_canonical_shape() {
    let capabilities = json!({
        "commands": [],
        "compiler": {"version": "2026-07-10T16-00-00Z"},
        "features": {},
        "output": {"formatVersion": 1, "schema": "spec/cli-json"},
        "phases": []
    });
    let platforms = [
        (
            "darwin-arm64",
            "darwin",
            "arm64",
            "aarch64-apple-darwin",
            "tar.gz",
        ),
        (
            "darwin-x64",
            "darwin",
            "x64",
            "x86_64-apple-darwin",
            "tar.gz",
        ),
        (
            "linux-arm64",
            "linux",
            "arm64",
            "aarch64-unknown-linux-gnu",
            "tar.gz",
        ),
        (
            "linux-x64",
            "linux",
            "x64",
            "x86_64-unknown-linux-gnu",
            "tar.gz",
        ),
        (
            "windows-x64",
            "windows",
            "x64",
            "x86_64-pc-windows-msvc",
            "zip",
        ),
    ];
    let artifacts = platforms
        .into_iter()
        .map(|(id, os, architecture, target, archive_format)| {
            json!({
                "archiveFormat": archive_format,
                "file": format!("sigil-2026-07-10T16-00-00Z-{id}.{archive_format}"),
                "platform": {
                    "architecture": architecture,
                    "id": id,
                    "os": os,
                    "target": target
                },
                "sha256": "a".repeat(64),
                "sizeBytes": 42
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "artifacts": artifacts,
        "compiler": {
            "capabilities": capabilities,
            "version": "2026-07-10T16-00-00Z"
        },
        "formatVersion": 1,
        "source": {
            "commit": "b".repeat(40),
            "repository": "inerte/sigil"
        },
        "version": "2026-07-10T16-00-00Z"
    });
    let errors = schema()
        .iter_errors(&manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {}", errors.join("\n"));
}

#[test]
fn release_manifest_v1_rejects_volatile_or_unknown_fields() {
    let manifest = json!({
        "artifacts": [],
        "compiler": {"capabilities": {}, "version": "2026-07-10T16-00-00Z"},
        "formatVersion": 1,
        "generatedAt": "2026-07-10T16:00:01Z",
        "source": {"commit": "b".repeat(40), "repository": "inerte/sigil"},
        "version": "2026-07-10T16-00-00Z"
    });
    assert!(!schema().is_valid(&manifest));
}
