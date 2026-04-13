use super::legacy::CliError;
use std::path::{Path, PathBuf};

pub fn compile_command(
    path: &Path,
    output: Option<&Path>,
    show_types: bool,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
    selected_env: Option<&str>,
) -> Result<(), CliError> {
    super::compile_support::compile_command(
        path,
        output,
        show_types,
        ignore_paths,
        ignore_from,
        selected_env,
    )
}
