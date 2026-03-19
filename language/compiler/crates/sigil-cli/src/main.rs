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

use commands::{compile_command, lex_command, parse_command, run_command, test_command, validate_command};

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
        /// Input .sigil file
        file: PathBuf,

        /// Output file path
        #[arg(short = 'o')]
        output: Option<PathBuf>,

        /// Show inferred types in output
        #[arg(long)]
        show_types: bool,
    },

    /// Compile and run a Sigil file
    Run {
        /// Input .sigil file
        file: PathBuf,

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

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Lex { file } => lex_command(&file),
        Command::Parse { file } => parse_command(&file),
        Command::Compile {
            file,
            output,
            show_types,
        } => compile_command(&file, output.as_deref(), show_types),
        Command::Run { file, env, args } => run_command(&file, env.as_deref(), &args),
        Command::Test { path, env, r#match } => test_command(&path, env.as_deref(), r#match.as_deref()),
        Command::Validate { path, env } => validate_command(&path, &env),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
