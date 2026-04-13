mod compile;
mod compile_support;
mod debug;
mod feature_flag;
mod inspect;
mod legacy;
mod lex_parse;
mod run;
mod shared;
mod test;
mod validate;

pub use compile::compile_command;
pub use debug::{
    debug_run_session_command, debug_run_start_command, debug_test_session_command,
    debug_test_start_command, DebugControlAction,
};
pub use feature_flag::feature_flag_audit_command;
pub use inspect::{inspect_command, InspectMode};
pub use legacy::CliError;
pub use lex_parse::{lex_command, parse_command};
pub use run::run_command;
pub use test::test_command;
pub use validate::validate_command;
