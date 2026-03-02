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

use commands::{compile_command, lex_command, parse_command, run_command, test_command};

#[derive(Parser)]
#[command(name = "sigil", version = "0.1.0", about = "Sigil Compiler")]
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

        /// Human-readable output (default: JSON)
        #[arg(long)]
        human: bool,
    },

    /// Parse a Sigil file to AST
    Parse {
        /// Input .sigil file
        file: PathBuf,

        /// Human-readable output (default: JSON)
        #[arg(long)]
        human: bool,
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

        /// Human-readable output (default: JSON)
        #[arg(long)]
        human: bool,
    },

    /// Compile and run a Sigil file
    Run {
        /// Input .sigil file
        file: PathBuf,

        /// Human-readable output (default: JSON)
        #[arg(long)]
        human: bool,
    },

    /// Run Sigil tests
    Test {
        /// Test directory (default: ./tests)
        #[arg(default_value = "tests")]
        path: PathBuf,

        /// Filter tests by substring match
        #[arg(long)]
        r#match: Option<String>,

        /// Human-readable output (default: JSON)
        #[arg(long)]
        human: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Lex { file, human } => lex_command(&file, human),
        Command::Parse { file, human } => parse_command(&file, human),
        Command::Compile {
            file,
            output,
            show_types,
            human,
        } => compile_command(&file, output.as_deref(), show_types, human),
        Command::Run { file, human } => run_command(&file, human),
        Command::Test { path, r#match, human } => test_command(&path, r#match.as_deref(), human),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
