//! Sigil project configuration and layout
//!
//! Handles detection and loading of sigil.json project configuration.
//! `src/` and `tests/` are canonical project directories; `sigil.json`
//! marks the project root and declares required project metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Effective project layout used by the compiler.
///
/// `src/`, `tests/`, and `.local/` are fixed by the compiler.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectLayout {
    pub src: String,
    pub tests: String,
    pub out: String,
}

fn default_src() -> String {
    "src".to_string()
}

fn default_tests() -> String {
    "tests".to_string()
}

fn default_out() -> String {
    ".local".to_string()
}

impl ProjectLayout {
    fn canonical(out: String) -> Self {
        Self {
            src: default_src(),
            tests: default_tests(),
            out,
        }
    }
}

impl Default for ProjectLayout {
    fn default() -> Self {
        Self::canonical(default_out())
    }
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub layout: ProjectLayout,
}

#[derive(Debug, Error)]
pub enum ProjectConfigError {
    #[error("failed to read sigil.json at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid sigil.json at {}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjectConfig {
    name: String,
    version: String,
}

fn invalid_config(path: PathBuf, message: impl Into<String>) -> ProjectConfigError {
    ProjectConfigError::Invalid {
        path,
        message: message.into(),
    }
}

fn parse_project_config(
    config_path: PathBuf,
    root: PathBuf,
    source: &str,
) -> Result<ProjectConfig, ProjectConfigError> {
    let raw: RawProjectConfig = serde_json::from_str(source)
        .map_err(|err| invalid_config(config_path.clone(), err.to_string()))?;
    let name = raw.name.trim();
    let version = raw.version.trim();

    if name.is_empty() {
        return Err(invalid_config(
            config_path,
            "field `name` must be a non-empty string",
        ));
    }

    if version.is_empty() {
        return Err(invalid_config(
            config_path,
            "field `version` must be a non-empty string",
        ));
    }

    Ok(ProjectConfig {
        root,
        layout: ProjectLayout::default(),
    })
}

/// Find the Sigil project root by searching for sigil.json
pub fn find_project_root(start_path: &Path) -> Option<PathBuf> {
    let mut current = start_path.to_path_buf();

    if current.is_file() {
        current = current.parent()?.to_path_buf();
    }

    loop {
        let config_path = current.join("sigil.json");
        if config_path.exists() {
            return Some(current);
        }

        current = current.parent()?.to_path_buf();
    }
}

/// Get Sigil project configuration
pub fn get_project_config(start_path: &Path) -> Result<Option<ProjectConfig>, ProjectConfigError> {
    let Some(root) = find_project_root(start_path) else {
        return Ok(None);
    };
    let config_path = root.join("sigil.json");
    let source = fs::read_to_string(&config_path).map_err(|source| ProjectConfigError::Io {
        path: config_path.clone(),
        source,
    })?;

    parse_project_config(config_path, root, &source).map(Some)
}

#[cfg(test)]
mod tests {
    use super::{parse_project_config, ProjectConfigError};
    use std::path::PathBuf;

    fn parse(source: &str) -> Result<super::ProjectConfig, ProjectConfigError> {
        parse_project_config(
            PathBuf::from("/tmp/demo/sigil.json"),
            PathBuf::from("/tmp/demo"),
            source,
        )
    }

    #[test]
    fn valid_config_requires_name_and_version_and_uses_canonical_layout() {
        let config = parse(r#"{"name":"demo","version":"0.1.0"}"#).unwrap();

        assert_eq!(config.layout.src, "src");
        assert_eq!(config.layout.tests, "tests");
        assert_eq!(config.layout.out, ".local");
    }

    #[test]
    fn config_rejects_unknown_fields() {
        let err = parse(
            r#"{
  "name":"demo",
  "version":"0.1.0",
  "extra":true
}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field `extra`"));
    }

    #[test]
    fn config_rejects_missing_name() {
        let err = parse(r#"{"version":"0.1.0"}"#).unwrap_err();

        assert!(err.to_string().contains("missing field `name`"));
    }

    #[test]
    fn config_rejects_empty_version() {
        let err = parse(r#"{"name":"demo","version":"   "}"#).unwrap_err();

        assert!(err
            .to_string()
            .contains("field `version` must be a non-empty string"));
    }
}
