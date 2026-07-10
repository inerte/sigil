use super::legacy::CliError;
use super::shared::{output_json_value, COMPILER_VERSION, MACHINE_FORMAT_VERSION};
use serde_json::json;

const MACHINE_COMMANDS: &[(&str, &str)] = &[
    ("sigil capabilities", "json"),
    ("sigil init", "json"),
    ("sigil docs list", "json"),
    ("sigil docs search", "json"),
    ("sigil docs show", "json"),
    ("sigil docs context", "json"),
    ("sigil lex", "json"),
    ("sigil parse", "json"),
    ("sigil compile", "json"),
    ("sigil inspect types", "json"),
    ("sigil inspect proof", "json"),
    ("sigil inspect validate", "json"),
    ("sigil inspect codegen", "json"),
    ("sigil inspect world", "json"),
    ("sigil inspect trust", "json"),
    ("sigil run", "streaming-or-json"),
    ("sigil test", "json"),
    ("sigil validate", "json"),
    ("sigil review", "human-llm-or-json"),
    ("sigil featureFlag audit", "json"),
    ("sigil package add", "json"),
    ("sigil package install", "json"),
    ("sigil package update", "json"),
    ("sigil package remove", "json"),
    ("sigil package list", "json"),
    ("sigil package why", "json"),
    ("sigil package publish", "json"),
    ("sigil package validate", "json"),
    ("sigil debug run", "json"),
    ("sigil debug test", "json"),
];

pub fn capabilities_command() -> Result<(), CliError> {
    output_json_value(
        &json!({
            "formatVersion": MACHINE_FORMAT_VERSION,
            "command": "sigil capabilities",
            "ok": true,
            "phase": "cli",
            "analysis": {"status": "notApplicable", "level": "none"},
            "data": {
                "compiler": {"version": COMPILER_VERSION},
                "output": {
                    "formatVersion": MACHINE_FORMAT_VERSION,
                    "schema": "spec/cli-json"
                },
                "commands": MACHINE_COMMANDS.iter().map(|(name, mode)| json!({
                    "name": name,
                    "mode": mode
                })).collect::<Vec<_>>(),
                "phases": [
                    "cli", "io", "surface", "lexer", "parser", "canonical",
                    "typecheck", "proof", "topology", "mutability", "extern",
                    "codegen", "runtime", "docs", "package", "internal"
                ],
                "features": {
                    "inspect": ["types", "proof", "validate", "codegen", "world", "trust"],
                    "debug": ["run", "test"],
                    "replay": {"runFormatVersion": 2, "testFormatVersion": 1},
                    "trustReportFormatVersion": 1
                }
            }
        }),
        false,
    );
    Ok(())
}
