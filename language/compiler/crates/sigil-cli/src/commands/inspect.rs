use super::legacy::CliError;
pub use super::legacy::InspectMode;
use std::path::{Path, PathBuf};

pub fn inspect_command(
    mode: InspectMode,
    path: &Path,
    selected_env: Option<&str>,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    super::legacy::inspect_command(mode, path, selected_env, ignore_paths, ignore_from)
}
