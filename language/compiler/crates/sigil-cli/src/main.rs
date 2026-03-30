//! Sigil Compiler CLI
//!
//! Command-line interface for the Sigil compiler.
//! Provides commands: compile, run, test, parse, lex

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod commands;
mod module_graph;
mod project;

use commands::{
    compile_command, inspect_command, lex_command, parse_command, run_command, test_command,
    validate_command,
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
        } => compile_command(
            &path,
            output.as_deref(),
            show_types,
            &ignore,
            ignore_from.as_deref(),
        ),
        Command::Inspect { command } => match command {
            InspectCommand::Types {
                path,
                ignore,
                ignore_from,
            } => inspect_command(
                commands::InspectMode::Types,
                &path,
                &ignore,
                ignore_from.as_deref(),
            ),
            InspectCommand::Validate {
                path,
                ignore,
                ignore_from,
            } => inspect_command(
                commands::InspectMode::Validate,
                &path,
                &ignore,
                ignore_from.as_deref(),
            ),
        },
        Command::Run {
            file,
            json,
            env,
            args,
        } => run_command(&file, json, env.as_deref(), &args),
        Command::Test { path, env, r#match } => {
            test_command(&path, env.as_deref(), r#match.as_deref())
        }
        Command::Validate { path, env } => validate_command(&path, &env),
    };

    if let Err(e) = result {
        if let Some(exit_code) = e.reported_exit_code() {
            process::exit(exit_code);
        }
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
