//! Sigil project configuration and layout
//!
//! Handles detection and loading of sigil.json project configuration.
//! `src/` and `tests/` are canonical project directories; `sigil.json`
//! marks the project root and declares required project metadata.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish: Option<PublishConfig>,
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
    pub name: String,
    pub version: String,
    pub dependencies: BTreeMap<String, String>,
    pub publish: Option<PublishConfig>,
}

impl ProjectConfig {
    pub fn manifest(&self) -> ProjectManifest {
        ProjectManifest {
            name: self.name.clone(),
            version: self.version.clone(),
            dependencies: self.dependencies.clone(),
            publish: self.publish.clone(),
        }
    }

    pub fn is_publishable_package(&self) -> bool {
        self.publish.is_some()
    }

    pub fn package_store_root(&self) -> PathBuf {
        self.root.join(".sigil").join("packages")
    }
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

fn invalid_config(path: PathBuf, message: impl Into<String>) -> ProjectConfigError {
    ProjectConfigError::Invalid {
        path,
        message: message.into(),
    }
}

pub fn is_lower_camel_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric())
}

pub fn is_canonical_timestamp_version(version: &str) -> bool {
    if version.len() != 20 {
        return false;
    }

    let bytes = version.as_bytes();
    const DIGIT_POSITIONS: [usize; 14] = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if !DIGIT_POSITIONS
        .iter()
        .all(|index| bytes[*index].is_ascii_digit())
    {
        return false;
    }

    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b'-'
        && bytes[16] == b'-'
        && bytes[19] == b'Z'
}

pub fn sigil_version_to_npm_version(version: &str) -> Option<String> {
    if !is_canonical_timestamp_version(version) {
        return None;
    }

    Some(format!(
        "{}{}.{}.0",
        &version[0..4],
        version[5..7].to_string() + &version[8..10],
        version[11..13].to_string() + &version[14..16].to_string() + &version[17..19]
    ))
}

pub fn npm_version_to_sigil_version(version: &str) -> Option<String> {
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != "0" {
        return None;
    }
    if parts[0].len() != 8
        || parts[1].len() != 6
        || !parts[0].chars().all(|ch| ch.is_ascii_digit())
        || !parts[1].chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }

    Some(format!(
        "{}-{}-{}T{}-{}-{}Z",
        &parts[0][0..4],
        &parts[0][4..6],
        &parts[0][6..8],
        &parts[1][0..2],
        &parts[1][2..4],
        &parts[1][4..6]
    ))
}

pub fn sigil_name_to_npm_package_name(name: &str) -> Option<String> {
    if !is_lower_camel_name(name) {
        return None;
    }

    let mut result = String::new();
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            result.push('-');
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    Some(result)
}

pub fn package_version_fragment(version: &str) -> Option<String> {
    if !is_canonical_timestamp_version(version) {
        return None;
    }

    Some(format!("v{}_{}", &version[0..10].replace('-', ""), &version[11..19].replace('-', "")))
}

fn validate_name(
    path: &Path,
    field_name: &str,
    value: &str,
) -> Result<(), ProjectConfigError> {
    if value.trim().is_empty() {
        return Err(invalid_config(
            path.to_path_buf(),
            format!("field `{field_name}` must be a non-empty string"),
        ));
    }
    if !is_lower_camel_name(value) {
        return Err(invalid_config(
            path.to_path_buf(),
            format!(
                "field `{field_name}` must use lowerCamel with ASCII letters and digits only"
            ),
        ));
    }
    Ok(())
}

fn validate_version(
    path: &Path,
    field_name: &str,
    value: &str,
) -> Result<(), ProjectConfigError> {
    if value.trim().is_empty() {
        return Err(invalid_config(
            path.to_path_buf(),
            format!("field `{field_name}` must be a non-empty string"),
        ));
    }
    if !is_canonical_timestamp_version(value) {
        return Err(invalid_config(
            path.to_path_buf(),
            format!(
                "field `{field_name}` must use canonical UTC timestamp format YYYY-MM-DDTHH-mm-ssZ"
            ),
        ));
    }
    Ok(())
}

fn validate_manifest(
    config_path: &Path,
    root: &Path,
    manifest: &ProjectManifest,
) -> Result<(), ProjectConfigError> {
    validate_name(config_path, "name", manifest.name.trim())?;
    validate_version(config_path, "version", manifest.version.trim())?;

    for (dependency_name, dependency_version) in &manifest.dependencies {
        validate_name(config_path, "dependencies key", dependency_name)?;
        validate_version(
            config_path,
            &format!("dependencies.{dependency_name}"),
            dependency_version,
        )?;
    }

    let package_root_path = root.join("src/package.lib.sigil");
    let has_package_root = package_root_path.exists();
    let has_publish = manifest.publish.is_some();

    if has_package_root && !has_publish {
        return Err(invalid_config(
            config_path.to_path_buf(),
            "projects with src/package.lib.sigil must declare `publish` in sigil.json",
        ));
    }

    if has_publish && !has_package_root {
        return Err(invalid_config(
            config_path.to_path_buf(),
            "projects with `publish` in sigil.json must define src/package.lib.sigil",
        ));
    }

    Ok(())
}

fn parse_project_config(
    config_path: PathBuf,
    root: PathBuf,
    source: &str,
) -> Result<ProjectConfig, ProjectConfigError> {
    let manifest: ProjectManifest = serde_json::from_str(source)
        .map_err(|err| invalid_config(config_path.clone(), err.to_string()))?;

    validate_manifest(&config_path, &root, &manifest)?;

    Ok(ProjectConfig {
        root,
        layout: ProjectLayout::default(),
        name: manifest.name.trim().to_string(),
        version: manifest.version.trim().to_string(),
        dependencies: manifest.dependencies,
        publish: manifest.publish,
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

pub fn write_project_manifest(root: &Path, manifest: &ProjectManifest) -> Result<(), ProjectConfigError> {
    validate_manifest(&root.join("sigil.json"), root, manifest)?;
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|err| invalid_config(root.join("sigil.json"), err.to_string()))?;
    fs::write(root.join("sigil.json"), format!("{text}\n")).map_err(|source| ProjectConfigError::Io {
        path: root.join("sigil.json"),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        is_canonical_timestamp_version, is_lower_camel_name, npm_version_to_sigil_version,
        parse_project_config, package_version_fragment, sigil_name_to_npm_package_name,
        sigil_version_to_npm_version, ProjectConfigError,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("sigil-project-{label}-{unique}"));
        fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    fn parse(source: &str, label: &str, create_package_root: bool) -> Result<super::ProjectConfig, ProjectConfigError> {
        let root = temp_root(label);
        if create_package_root {
            fs::write(root.join("src/package.lib.sigil"), "λmain()=>Unit=()\n").unwrap();
        }
        parse_project_config(root.join("sigil.json"), root, source)
    }

    #[test]
    fn valid_config_requires_name_and_version_and_uses_canonical_layout() {
        let config = parse(
            r#"{"name":"demoApp","version":"2026-04-05T14-58-24Z"}"#,
            "valid",
            false,
        )
        .unwrap();

        assert_eq!(config.layout.src, "src");
        assert_eq!(config.layout.tests, "tests");
        assert_eq!(config.layout.out, ".local");
        assert_eq!(config.name, "demoApp");
        assert_eq!(config.version, "2026-04-05T14-58-24Z");
        assert!(config.dependencies.is_empty());
    }

    #[test]
    fn config_rejects_unknown_fields() {
        let err = parse(
            r#"{
  "name":"demoApp",
  "version":"2026-04-05T14-58-24Z",
  "extra":true
}"#,
            "unknown-field",
            false,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field `extra`"));
    }

    #[test]
    fn config_rejects_missing_name() {
        let err = parse(r#"{"version":"2026-04-05T14-58-24Z"}"#, "missing-name", false).unwrap_err();

        assert!(err.to_string().contains("missing field `name`"));
    }

    #[test]
    fn config_rejects_empty_version() {
        let err = parse(
            r#"{"name":"demoApp","version":"   "}"#,
            "empty-version",
            false,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("field `version` must be a non-empty string"));
    }

    #[test]
    fn config_rejects_non_lower_camel_names() {
        let err = parse(
            r#"{"name":"todo-app","version":"2026-04-05T14-58-24Z"}"#,
            "bad-name",
            false,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("field `name` must use lowerCamel"));
    }

    #[test]
    fn config_rejects_non_timestamp_versions() {
        let err = parse(r#"{"name":"demoApp","version":"0.1.0"}"#, "bad-version", false).unwrap_err();

        assert!(err.to_string().contains("YYYY-MM-DDTHH-mm-ssZ"));
    }

    #[test]
    fn config_rejects_package_root_without_publish() {
        let err = parse(
            r#"{"name":"router","version":"2026-04-05T14-58-24Z"}"#,
            "missing-publish",
            true,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("must declare `publish` in sigil.json"));
    }

    #[test]
    fn config_rejects_publish_without_package_root() {
        let err = parse(
            r#"{"name":"router","version":"2026-04-05T14-58-24Z","publish":{}}"#,
            "missing-package-root",
            false,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("must define src/package.lib.sigil"));
    }

    #[test]
    fn config_accepts_exact_dependencies() {
        let config = parse(
            r#"{
  "name":"demoApp",
  "version":"2026-04-05T14-58-24Z",
  "dependencies":{
    "router":"2026-04-05T14-57-00Z"
  }
}"#,
            "deps",
            false,
        )
        .unwrap();

        assert_eq!(
            config.dependencies.get("router"),
            Some(&"2026-04-05T14-57-00Z".to_string())
        );
    }

    #[test]
    fn helper_validates_lower_camel_names() {
        assert!(is_lower_camel_name("router"));
        assert!(is_lower_camel_name("todoApp2"));
        assert!(!is_lower_camel_name("todo-app"));
        assert!(!is_lower_camel_name("TodoApp"));
    }

    #[test]
    fn helper_validates_timestamp_versions() {
        assert!(is_canonical_timestamp_version("2026-04-05T14-58-24Z"));
        assert!(!is_canonical_timestamp_version("20260405.145824.0"));
        assert!(!is_canonical_timestamp_version("0.1.0"));
    }

    #[test]
    fn converts_versions_to_and_from_npm_transport() {
        let npm_version = sigil_version_to_npm_version("2026-04-05T14-58-24Z").unwrap();
        assert_eq!(npm_version, "20260405.145824.0");
        assert_eq!(
            npm_version_to_sigil_version(&npm_version).unwrap(),
            "2026-04-05T14-58-24Z"
        );
    }

    #[test]
    fn converts_names_to_npm_transport() {
        assert_eq!(
            sigil_name_to_npm_package_name("todoApp").unwrap(),
            "todo-app"
        );
        assert_eq!(sigil_name_to_npm_package_name("router").unwrap(), "router");
    }

    #[test]
    fn builds_package_version_fragments() {
        assert_eq!(
            package_version_fragment("2026-04-05T14-58-24Z").unwrap(),
            "v20260405_145824"
        );
    }
}
