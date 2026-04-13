//! Sigil Compiler CLI
//!
//! Command-line interface for the Sigil compiler.
//! Provides commands: compile, run, test, parse, lex

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process;

mod commands;
mod module_graph;
mod package_manager;
mod project;

use commands::{
    compile_command, debug_run_session_command, debug_run_start_command,
    debug_test_session_command, debug_test_start_command, feature_flag_audit_command,
    inspect_command, lex_command, parse_command, run_command, test_command, validate_command,
    DebugControlAction,
};
use package_manager::{
    package_add_command, package_install_command, package_list_command, package_publish_command,
    package_remove_command, package_update_command, package_validate_command, package_why_command,
};

const SIGIL_VERSION: &str = match option_env!("SIGIL_VERSION") {
    Some(version) => version,
    None => "dev",
};

#[derive(Parser)]
#[command(name = "sigil", version = SIGIL_VERSION, about = "Sigil Compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BreakModeArg {
    Stop,
    Collect,
}

#[derive(Subcommand)]
enum Command {
    /// Tokenize a Sigil file
    Lex {
        /// Input .sigil file
        file: PathBuf,
    },

    /// Parse a Sigil file to AST
    Parse {
        /// Input .sigil file
        file: PathBuf,
    },

    /// Compile a Sigil file to TypeScript
    Compile {
        /// Input .sigil file or directory
        path: PathBuf,

        /// Output file path (single-file compile only)
        #[arg(short = 'o')]
        output: Option<PathBuf>,

        /// Show inferred types in output
        #[arg(long)]
        show_types: bool,

        /// Ignore an additional path while compiling a directory
        #[arg(long)]
        ignore: Vec<PathBuf>,

        /// Load gitignore-style ignore rules from a file while compiling a directory
        #[arg(long = "ignore-from")]
        ignore_from: Option<PathBuf>,

        /// Selected config environment for code that reads •config.<name>
        #[arg(long)]
        env: Option<String>,
    },

    /// Inspect compiler state for a Sigil file or directory
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },

    /// Compile and run a Sigil file
    Run {
        /// Input .sigil file
        file: PathBuf,

        /// Emit a machine-readable JSON result envelope even on success
        #[arg(long)]
        json: bool,

        /// Capture a bounded structured execution trace (requires --json)
        #[arg(long)]
        trace: bool,

        /// Include fine-grained expression enter/return/throw events in the trace (requires --trace and --json)
        #[arg(long = "trace-expr")]
        trace_expr: bool,

        /// Break when execution reaches a specific source line
        #[arg(long = "break", value_name = "FILE:LINE")]
        breakpoint: Vec<String>,

        /// Break when a specific top-level function is entered
        #[arg(long = "break-fn", value_name = "NAME")]
        break_fn: Vec<String>,

        /// Break when a specific span id is reached
        #[arg(long = "break-span", value_name = "SPAN")]
        break_span: Vec<String>,

        /// How breakpoint hits should affect the run
        #[arg(long = "break-mode", default_value = "stop")]
        break_mode: BreakModeArg,

        /// Maximum breakpoint hits returned inline
        #[arg(long = "break-max-hits", default_value_t = 32)]
        break_max_hits: usize,

        /// Record replayable external effect activity to a file
        #[arg(long, conflicts_with = "replay")]
        record: Option<PathBuf>,

        /// Replay a prior recorded run from a file
        #[arg(long, conflicts_with = "record")]
        replay: Option<PathBuf>,

        /// Runtime topology environment name (required for topology-aware projects)
        #[arg(long)]
        env: Option<String>,

        /// Arguments passed through to the Sigil program
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Run Sigil tests
    Test {
        /// Test directory (default: ./tests)
        #[arg(default_value = "tests")]
        path: PathBuf,

        /// Runtime topology environment name (required for topology-aware projects)
        #[arg(long)]
        env: Option<String>,

        /// Filter tests by substring match
        #[arg(long)]
        r#match: Option<String>,

        /// Capture a bounded structured execution trace for each selected test
        #[arg(long)]
        trace: bool,

        /// Include fine-grained expression enter/return/throw events in the trace (requires --trace)
        #[arg(long = "trace-expr")]
        trace_expr: bool,

        /// Break when execution reaches a specific source line
        #[arg(long = "break", value_name = "FILE:LINE")]
        breakpoint: Vec<String>,

        /// Break when a specific top-level function is entered
        #[arg(long = "break-fn", value_name = "NAME")]
        break_fn: Vec<String>,

        /// Break when a specific span id is reached
        #[arg(long = "break-span", value_name = "SPAN")]
        break_span: Vec<String>,

        /// How breakpoint hits should affect the current test
        #[arg(long = "break-mode", default_value = "stop")]
        break_mode: BreakModeArg,

        /// Maximum breakpoint hits returned inline per test
        #[arg(long = "break-max-hits", default_value_t = 32)]
        break_max_hits: usize,

        /// Record replayable external effect activity for the test run
        #[arg(long, conflicts_with = "replay")]
        record: Option<PathBuf>,

        /// Replay a prior recorded test run
        #[arg(long, conflicts_with = "record")]
        replay: Option<PathBuf>,
    },

    /// Validate project topology for one environment
    Validate {
        /// Project path or file within the project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Runtime topology environment name
        #[arg(long)]
        env: String,
    },

    /// Query first-class feature flag declarations
    #[command(name = "featureFlag")]
    FeatureFlag {
        #[command(subcommand)]
        command: FeatureFlagCommand,
    },

    /// Manage Sigil packages
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },

    /// Replay-backed machine-first debugging
    Debug {
        #[command(subcommand)]
        command: DebugCommand,
    },
}

#[derive(Subcommand)]
enum InspectCommand {
    /// Inspect top-level solved types
    Types {
        /// Input .sigil file or directory
        path: PathBuf,

        /// Ignore an additional path while inspecting a directory
        #[arg(long)]
        ignore: Vec<PathBuf>,

        /// Load gitignore-style ignore rules from a file while inspecting a directory
        #[arg(long = "ignore-from")]
        ignore_from: Option<PathBuf>,

        /// Selected config environment for code that reads •config.<name>
        #[arg(long)]
        env: Option<String>,
    },

    /// Inspect declared proof surfaces and branch facts
    Proof {
        /// Input .sigil file or directory
        path: PathBuf,

        /// Ignore an additional path while inspecting a directory
        #[arg(long)]
        ignore: Vec<PathBuf>,

        /// Load gitignore-style ignore rules from a file while inspecting a directory
        #[arg(long = "ignore-from")]
        ignore_from: Option<PathBuf>,

        /// Selected config environment for code that reads •config.<name>
        #[arg(long)]
        env: Option<String>,
    },

    /// Inspect canonical validation and printer output
    Validate {
        /// Input .sigil file or directory
        path: PathBuf,

        /// Ignore an additional path while inspecting a directory
        #[arg(long)]
        ignore: Vec<PathBuf>,

        /// Load gitignore-style ignore rules from a file while inspecting a directory
        #[arg(long = "ignore-from")]
        ignore_from: Option<PathBuf>,

        /// Selected config environment for code that reads •config.<name>
        #[arg(long)]
        env: Option<String>,
    },

    /// Inspect generated TypeScript and derived codegen outputs
    Codegen {
        /// Input .sigil file or directory
        path: PathBuf,

        /// Ignore an additional path while inspecting a directory
        #[arg(long)]
        ignore: Vec<PathBuf>,

        /// Load gitignore-style ignore rules from a file while inspecting a directory
        #[arg(long = "ignore-from")]
        ignore_from: Option<PathBuf>,

        /// Selected config environment for code that reads •config.<name>
        #[arg(long)]
        env: Option<String>,
    },

    /// Inspect the resolved runtime world for one environment
    World {
        /// Project path or file within the project (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Runtime topology environment name
        #[arg(long)]
        env: String,
    },
}

#[derive(Subcommand)]
enum DebugCommand {
    /// Debug a replayed program run
    Run {
        #[command(subcommand)]
        command: DebugRunCommand,
    },

    /// Debug one replayed test by exact id
    Test {
        #[command(subcommand)]
        command: DebugTestCommand,
    },
}

#[derive(Subcommand)]
enum PackageCommand {
    /// Add one direct dependency at the latest exact version
    Add {
        /// Direct dependency name
        name: String,
    },

    /// Install exact dependencies from sigil.json
    Install,

    /// Update one or all direct dependencies
    Update {
        /// Direct dependency name
        name: Option<String>,

        /// Keep updated dependencies even when project tests fail
        #[arg(long)]
        keep_failing: bool,
    },

    /// Remove one direct dependency
    Remove {
        /// Direct dependency name
        name: String,
    },

    /// List direct dependencies
    List,

    /// Show why one package is present
    Why {
        /// Package name
        name: String,
    },

    /// Publish the current package using npm transport
    Publish,

    /// Validate the current package's local publishability
    Validate,
}

#[derive(Subcommand)]
enum FeatureFlagCommand {
    /// Audit feature flag declarations under one file or directory
    Audit {
        /// Input .sigil file or directory (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Filter for flags older than Nd, for example 180d
        #[arg(long = "older-than")]
        older_than: Option<String>,
    },
}

#[derive(Subcommand)]
enum DebugRunCommand {
    /// Start a replay-backed debug session for one program
    Start {
        /// Replay artifact recorded from `sigil run --record`
        #[arg(long)]
        replay: PathBuf,

        /// Input .sigil file
        file: PathBuf,

        /// Add a watched local or record path to every snapshot
        #[arg(long = "watch", value_name = "SELECTOR")]
        watch: Vec<String>,

        /// Stop when execution reaches a specific source line
        #[arg(long = "break", value_name = "FILE:LINE")]
        breakpoint: Vec<String>,

        /// Stop when a specific top-level function is entered
        #[arg(long = "break-fn", value_name = "NAME")]
        break_fn: Vec<String>,

        /// Stop when a specific span id is reached
        #[arg(long = "break-span", value_name = "SPAN")]
        break_span: Vec<String>,
    },

    /// Read the latest stored snapshot for a debug session
    Snapshot {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Step into the next source-level event
    StepInto {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Step over the current entered expression or frame
    StepOver {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Step out of the current frame
    StepOut {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Continue until the next breakpoint, uncaught exception, or exit
    Continue {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Close a debug session and remove its session file
    Close {
        /// Debug session file returned by `start`
        session: PathBuf,
    },
}

#[derive(Subcommand)]
enum DebugTestCommand {
    /// Start a replay-backed debug session for one exact test id
    Start {
        /// Replay artifact recorded from `sigil test --record`
        #[arg(long)]
        replay: PathBuf,

        /// Exact `results[].id` from the recorded test run
        #[arg(long = "test")]
        test_id: String,

        /// Test file or directory
        path: PathBuf,

        /// Add a watched local or record path to every snapshot
        #[arg(long = "watch", value_name = "SELECTOR")]
        watch: Vec<String>,

        /// Stop when execution reaches a specific source line
        #[arg(long = "break", value_name = "FILE:LINE")]
        breakpoint: Vec<String>,

        /// Stop when a specific top-level function is entered
        #[arg(long = "break-fn", value_name = "NAME")]
        break_fn: Vec<String>,

        /// Stop when a specific span id is reached
        #[arg(long = "break-span", value_name = "SPAN")]
        break_span: Vec<String>,
    },

    /// Read the latest stored snapshot for a debug session
    Snapshot {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Step into the next source-level event
    StepInto {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Step over the current entered expression or frame
    StepOver {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Step out of the current frame
    StepOut {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Continue until the next breakpoint, uncaught exception, or test exit
    Continue {
        /// Debug session file returned by `start`
        session: PathBuf,
    },

    /// Close a debug session and remove its session file
    Close {
        /// Debug session file returned by `start`
        session: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Lex { file } => lex_command(&file),
        Command::Parse { file } => parse_command(&file),
        Command::Compile {
            path,
            output,
            show_types,
            ignore,
            ignore_from,
            env,
        } => compile_command(
            &path,
            output.as_deref(),
            show_types,
            &ignore,
            ignore_from.as_deref(),
            env.as_deref(),
        ),
        Command::Inspect { command } => match command {
            InspectCommand::Types {
                path,
                ignore,
                ignore_from,
                env,
            } => inspect_command(
                commands::InspectMode::Types,
                &path,
                env.as_deref(),
                &ignore,
                ignore_from.as_deref(),
            ),
            InspectCommand::Proof {
                path,
                ignore,
                ignore_from,
                env,
            } => inspect_command(
                commands::InspectMode::Proof,
                &path,
                env.as_deref(),
                &ignore,
                ignore_from.as_deref(),
            ),
            InspectCommand::Validate {
                path,
                ignore,
                ignore_from,
                env,
            } => inspect_command(
                commands::InspectMode::Validate,
                &path,
                env.as_deref(),
                &ignore,
                ignore_from.as_deref(),
            ),
            InspectCommand::Codegen {
                path,
                ignore,
                ignore_from,
                env,
            } => inspect_command(
                commands::InspectMode::Codegen,
                &path,
                env.as_deref(),
                &ignore,
                ignore_from.as_deref(),
            ),
            InspectCommand::World { path, env } => {
                inspect_command(commands::InspectMode::World, &path, Some(&env), &[], None)
            }
        },
        Command::Run {
            file,
            json,
            trace,
            trace_expr,
            breakpoint,
            break_fn,
            break_span,
            break_mode,
            break_max_hits,
            record,
            replay,
            env,
            args,
        } => run_command(
            &file,
            json,
            trace,
            trace_expr,
            &breakpoint,
            &break_fn,
            &break_span,
            matches!(break_mode, BreakModeArg::Collect),
            break_max_hits,
            record.as_deref(),
            replay.as_deref(),
            env.as_deref(),
            &args,
        ),
        Command::Test {
            path,
            env,
            r#match,
            trace,
            trace_expr,
            breakpoint,
            break_fn,
            break_span,
            break_mode,
            break_max_hits,
            record,
            replay,
        } => test_command(
            &path,
            env.as_deref(),
            r#match.as_deref(),
            trace,
            trace_expr,
            &breakpoint,
            &break_fn,
            &break_span,
            matches!(break_mode, BreakModeArg::Collect),
            break_max_hits,
            record.as_deref(),
            replay.as_deref(),
        ),
        Command::Validate { path, env } => validate_command(&path, &env),
        Command::FeatureFlag { command } => match command {
            FeatureFlagCommand::Audit { path, older_than } => {
                feature_flag_audit_command(&path, older_than.as_deref())
            }
        },
        Command::Package { command } => match command {
            PackageCommand::Add { name } => {
                package_add_command(&std::env::current_dir().unwrap(), &name)
            }
            PackageCommand::Install => package_install_command(&std::env::current_dir().unwrap()),
            PackageCommand::Update { name, keep_failing } => package_update_command(
                &std::env::current_dir().unwrap(),
                name.as_deref(),
                keep_failing,
            ),
            PackageCommand::Remove { name } => {
                package_remove_command(&std::env::current_dir().unwrap(), &name)
            }
            PackageCommand::List => package_list_command(&std::env::current_dir().unwrap()),
            PackageCommand::Why { name } => {
                package_why_command(&std::env::current_dir().unwrap(), &name)
            }
            PackageCommand::Publish => package_publish_command(&std::env::current_dir().unwrap()),
            PackageCommand::Validate => package_validate_command(&std::env::current_dir().unwrap()),
        },
        Command::Debug { command } => match command {
            DebugCommand::Run { command } => match command {
                DebugRunCommand::Start {
                    replay,
                    file,
                    watch,
                    breakpoint,
                    break_fn,
                    break_span,
                } => debug_run_start_command(
                    &file,
                    &replay,
                    &watch,
                    &breakpoint,
                    &break_fn,
                    &break_span,
                ),
                DebugRunCommand::Snapshot { session } => {
                    debug_run_session_command(DebugControlAction::Snapshot, &session)
                }
                DebugRunCommand::StepInto { session } => {
                    debug_run_session_command(DebugControlAction::StepInto, &session)
                }
                DebugRunCommand::StepOver { session } => {
                    debug_run_session_command(DebugControlAction::StepOver, &session)
                }
                DebugRunCommand::StepOut { session } => {
                    debug_run_session_command(DebugControlAction::StepOut, &session)
                }
                DebugRunCommand::Continue { session } => {
                    debug_run_session_command(DebugControlAction::Continue, &session)
                }
                DebugRunCommand::Close { session } => {
                    debug_run_session_command(DebugControlAction::Close, &session)
                }
            },
            DebugCommand::Test { command } => match command {
                DebugTestCommand::Start {
                    replay,
                    test_id,
                    path,
                    watch,
                    breakpoint,
                    break_fn,
                    break_span,
                } => debug_test_start_command(
                    &path,
                    &replay,
                    Some(&test_id),
                    &watch,
                    &breakpoint,
                    &break_fn,
                    &break_span,
                ),
                DebugTestCommand::Snapshot { session } => {
                    debug_test_session_command(DebugControlAction::Snapshot, &session)
                }
                DebugTestCommand::StepInto { session } => {
                    debug_test_session_command(DebugControlAction::StepInto, &session)
                }
                DebugTestCommand::StepOver { session } => {
                    debug_test_session_command(DebugControlAction::StepOver, &session)
                }
                DebugTestCommand::StepOut { session } => {
                    debug_test_session_command(DebugControlAction::StepOut, &session)
                }
                DebugTestCommand::Continue { session } => {
                    debug_test_session_command(DebugControlAction::Continue, &session)
                }
                DebugTestCommand::Close { session } => {
                    debug_test_session_command(DebugControlAction::Close, &session)
                }
            },
        },
    };

    if let Err(e) = result {
        if let Some(exit_code) = e.reported_exit_code() {
            process::exit(exit_code);
        }
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
