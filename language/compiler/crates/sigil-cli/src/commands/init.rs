use super::legacy::CliError;
use super::shared::{output_json_error, output_json_value, phase_for_code};
use crate::project::{
    is_lower_camel_name, write_project_manifest, ProjectConfigError, ProjectLayout, ProjectManifest,
};
use serde_json::json;
use sigil_diagnostics::codes;
use std::fs;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

const COMMAND_NAME: &str = "sigil init";

#[derive(Debug)]
enum InitError {
    Conflict {
        target_root: PathBuf,
        reason: String,
        existing_entries: Vec<String>,
    },
    InvalidName {
        target_root: PathBuf,
        raw_name: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    ProjectConfig(ProjectConfigError),
}

impl From<ProjectConfigError> for InitError {
    fn from(error: ProjectConfigError) -> Self {
        Self::ProjectConfig(error)
    }
}

struct InitSuccess {
    root: PathBuf,
    manifest: ProjectManifest,
    layout: ProjectLayout,
}

pub fn init_command(path: Option<&Path>) -> Result<(), CliError> {
    match init_project(path) {
        Ok(result) => {
            output_json_value(
                &json!({
                    "formatVersion": 1,
                    "command": COMMAND_NAME,
                    "ok": true,
                    "phase": "cli",
                    "data": {
                        "root": result.root.to_string_lossy(),
                        "manifest": result.manifest,
                        "layout": result.layout,
                        "created": [
                            "sigil.json",
                            "src",
                            "tests",
                            ".local"
                        ]
                    }
                }),
                false,
            );
            Ok(())
        }
        Err(error) => {
            output_init_error(&error);
            Err(CliError::Reported(1))
        }
    }
}

fn init_project(path: Option<&Path>) -> Result<InitSuccess, InitError> {
    let target_root = resolve_target_root(path)?;
    let raw_name = target_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| InitError::InvalidName {
            target_root: target_root.clone(),
            raw_name: target_root.to_string_lossy().to_string(),
        })?
        .to_string();
    let name = derive_project_name(&raw_name).ok_or_else(|| InitError::InvalidName {
        target_root: target_root.clone(),
        raw_name: raw_name.clone(),
    })?;

    ensure_target_is_safe(&target_root)?;

    fs::create_dir_all(&target_root).map_err(|source| InitError::Io {
        path: target_root.clone(),
        source,
    })?;

    for relative in ["src", "tests", ".local"] {
        let path = target_root.join(relative);
        fs::create_dir(&path).map_err(|source| InitError::Io {
            path: path.clone(),
            source,
        })?;
    }

    let manifest = ProjectManifest {
        name,
        version: current_utc_timestamp(),
        dependencies: Default::default(),
        publish: None,
    };
    write_project_manifest(&target_root, &manifest)?;

    let canonical_root = fs::canonicalize(&target_root).map_err(|source| InitError::Io {
        path: target_root.clone(),
        source,
    })?;

    Ok(InitSuccess {
        root: canonical_root,
        manifest,
        layout: ProjectLayout::default(),
    })
}

fn resolve_target_root(path: Option<&Path>) -> Result<PathBuf, InitError> {
    let cwd = std::env::current_dir().map_err(|source| InitError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    let requested = match path {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => cwd.join(path),
        None => cwd,
    };

    if requested.exists() {
        fs::canonicalize(&requested).map_err(|source| InitError::Io {
            path: requested,
            source,
        })
    } else {
        Ok(requested)
    }
}

fn derive_project_name(raw_name: &str) -> Option<String> {
    if is_lower_camel_name(raw_name) {
        return Some(raw_name.to_string());
    }

    let segments = raw_name
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    let first = *segments.first()?;
    if !first.chars().next()?.is_ascii_alphabetic() {
        return None;
    }

    let mut result = String::new();
    for (index, segment) in segments.into_iter().enumerate() {
        let mut chars = segment.chars();
        let first = chars.next()?;
        if index == 0 {
            result.push(first.to_ascii_lowercase());
        } else {
            result.push(first.to_ascii_uppercase());
        }
        for ch in chars {
            result.push(ch.to_ascii_lowercase());
        }
    }

    is_lower_camel_name(&result).then_some(result)
}

fn ensure_target_is_safe(target_root: &Path) -> Result<(), InitError> {
    if !target_root.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(target_root).map_err(|source| InitError::Io {
        path: target_root.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(InitError::Conflict {
            target_root: target_root.to_path_buf(),
            reason: "target exists and is not a directory".to_string(),
            existing_entries: Vec::new(),
        });
    }

    let entries = fs::read_dir(target_root)
        .map_err(|source| InitError::Io {
            path: target_root.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|source| InitError::Io {
            path: target_root.to_path_buf(),
            source,
        })?
        .into_iter()
        .map(|entry| {
            (
                entry.path(),
                entry.file_name().to_string_lossy().to_string(),
            )
        })
        .collect::<Vec<_>>();

    let mut ignored_internal_entries = Vec::new();
    let mut blocking_entries = Vec::new();

    for (path, name) in entries {
        if name == ".z3-trace" && path.is_file() {
            ignored_internal_entries.push(path);
        } else {
            blocking_entries.push(name);
        }
    }

    blocking_entries.sort();

    if blocking_entries.is_empty() {
        for path in ignored_internal_entries {
            fs::remove_file(&path).map_err(|source| InitError::Io { path, source })?;
        }
        return Ok(());
    }

    let reason = if blocking_entries.iter().any(|entry| entry == "sigil.json") {
        "target already contains sigil.json".to_string()
    } else {
        "target directory must be empty before initialization".to_string()
    };

    Err(InitError::Conflict {
        target_root: target_root.to_path_buf(),
        reason,
        existing_entries: blocking_entries,
    })
}

fn current_utc_timestamp() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}-{:02}-{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

fn output_init_error(error: &InitError) {
    match error {
        InitError::Conflict {
            target_root,
            reason,
            existing_entries,
        } => output_json_error(
            COMMAND_NAME,
            "cli",
            codes::cli::PROJECT_INIT_CONFLICT,
            reason,
            json!({
                "targetRoot": target_root.to_string_lossy(),
                "reason": reason,
                "existingEntries": existing_entries,
            }),
        ),
        InitError::InvalidName {
            target_root,
            raw_name,
        } => output_json_error(
            COMMAND_NAME,
            "cli",
            codes::cli::PROJECT_INIT_INVALID_NAME,
            &format!(
                "target directory name `{raw_name}` cannot be converted into a lowerCamel Sigil project name"
            ),
            json!({
                "targetRoot": target_root.to_string_lossy(),
                "rawName": raw_name,
                "expected": "lowerCamel ASCII letters and digits"
            }),
        ),
        InitError::Io { path, source } => output_json_error(
            COMMAND_NAME,
            "cli",
            codes::cli::UNEXPECTED,
            &format!("failed to initialize project at {}: {}", path.display(), source),
            json!({
                "path": path.to_string_lossy()
            }),
        ),
        InitError::ProjectConfig(project_error) => output_json_error(
            COMMAND_NAME,
            phase_for_code(project_error.code()),
            project_error.code(),
            &project_error.to_string(),
            project_error.details(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::derive_project_name;

    #[test]
    fn derive_project_name_preserves_lower_camel_input() {
        assert_eq!(
            derive_project_name("helloSigil"),
            Some("helloSigil".to_string())
        );
    }

    #[test]
    fn derive_project_name_converts_common_directory_forms() {
        assert_eq!(
            derive_project_name("hello-sigil"),
            Some("helloSigil".to_string())
        );
        assert_eq!(
            derive_project_name("Hello World"),
            Some("helloWorld".to_string())
        );
        assert_eq!(
            derive_project_name("hello_sigil_2"),
            Some("helloSigil2".to_string())
        );
    }

    #[test]
    fn derive_project_name_rejects_invalid_leading_digits() {
        assert_eq!(derive_project_name("123-demo"), None);
        assert_eq!(derive_project_name("---"), None);
    }
}
