//! Sigil project configuration and layout
//!
//! Handles detection and loading of sigil.json project configuration

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLayout {
    #[serde(default = "default_src")]
    pub src: String,

    #[serde(default = "default_tests")]
    pub tests: String,

    #[serde(default = "default_out")]
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

impl Default for ProjectLayout {
    fn default() -> Self {
        Self {
            src: default_src(),
            tests: default_tests(),
            out: default_out(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub layout: ProjectLayout,
}

#[derive(Debug, Deserialize)]
struct RawProjectConfig {
    #[serde(default)]
    layout: Option<ProjectLayout>,
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
pub fn get_project_config(start_path: &Path) -> Option<ProjectConfig> {
    let root = find_project_root(start_path)?;
    let config_path = root.join("sigil.json");

    let raw_config: RawProjectConfig = serde_json::from_str(&fs::read_to_string(config_path).ok()?).ok()?;

    Some(ProjectConfig {
        root,
        layout: raw_config.layout.unwrap_or_default(),
    })
}
