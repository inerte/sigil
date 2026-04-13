use super::legacy::CliError;
pub use super::legacy::DebugControlAction;
use std::path::Path;

pub fn debug_run_start_command(
    file: &Path,
    replay_path: &Path,
    watch_selectors: &[String],
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
) -> Result<(), CliError> {
    super::legacy::debug_run_start_command(
        file,
        replay_path,
        watch_selectors,
        breakpoint_lines,
        breakpoint_functions,
        breakpoint_spans,
    )
}

pub fn debug_run_session_command(
    action: DebugControlAction,
    session_path: &Path,
) -> Result<(), CliError> {
    super::legacy::debug_run_session_command(action, session_path)
}

pub fn debug_test_start_command(
    path: &Path,
    replay_path: &Path,
    test_id: Option<&str>,
    watch_selectors: &[String],
    breakpoint_lines: &[String],
    breakpoint_functions: &[String],
    breakpoint_spans: &[String],
) -> Result<(), CliError> {
    super::legacy::debug_test_start_command(
        path,
        replay_path,
        test_id,
        watch_selectors,
        breakpoint_lines,
        breakpoint_functions,
        breakpoint_spans,
    )
}

pub fn debug_test_session_command(
    action: DebugControlAction,
    session_path: &Path,
) -> Result<(), CliError> {
    super::legacy::debug_test_session_command(action, session_path)
}
