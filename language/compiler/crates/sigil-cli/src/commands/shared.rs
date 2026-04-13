use super::legacy::CliError;
use crate::module_graph::ModuleGraphError;
use crate::project::{get_project_config, validate_project_default_entrypoint, ProjectConfigError};
use serde_json::json;
use sigil_diagnostics::codes;
use sigil_typechecker::TypeError;
use sigil_validator::ValidationError;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct SourcePoint {
    pub line: usize,
    pub column: usize,
}

pub(super) fn extract_error_code(message: &str) -> String {
    if let Some(index) = message.find("SIGIL-") {
        let suffix = &message[index..];
        if let Some(colon_pos) = suffix.find(':') {
            return suffix[..colon_pos].to_string();
        }
        return suffix
            .split_whitespace()
            .next()
            .unwrap_or("SIGIL-ERROR")
            .to_string();
    }
    if let Some(colon_pos) = message.find(':') {
        message[..colon_pos].to_string()
    } else {
        "SIGIL-ERROR".to_string()
    }
}

pub(super) fn format_validation_errors(errors: &[ValidationError]) -> String {
    if errors.is_empty() {
        "validation errors".to_string()
    } else {
        errors
            .iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

pub(super) fn output_json_error(
    command: &str,
    phase: &str,
    error_code: &str,
    message: &str,
    details: serde_json::Value,
) {
    output_json_error_to(command, phase, error_code, message, details, false);
}

pub(super) fn output_json_error_to(
    command: &str,
    phase: &str,
    error_code: &str,
    message: &str,
    details: serde_json::Value,
    to_stderr: bool,
) {
    let output = json!({
        "formatVersion": 1,
        "command": command,
        "ok": false,
        "phase": phase,
        "error": {
            "code": error_code,
            "phase": phase,
            "message": message,
            "details": details
        }
    });
    output_json_value(&output, to_stderr);
}

pub(super) fn output_json_value(output: &serde_json::Value, to_stderr: bool) {
    let serialized = serde_json::to_string(output).unwrap();
    if to_stderr {
        eprintln!("{}", serialized);
    } else {
        println!("{}", serialized);
    }
}

pub(super) fn type_error_json_details(error: &TypeError) -> serde_json::Value {
    let mut details = serde_json::Map::new();

    if let Some(source_file) = &error.source_file {
        details.insert("file".to_string(), json!(source_file));
    }

    if let Some(location) = error.location {
        details.insert(
            "location".to_string(),
            json!({
                "start": {
                    "line": location.start.line,
                    "column": location.start.column,
                    "offset": location.start.offset
                },
                "end": {
                    "line": location.end.line,
                    "column": location.end.column,
                    "offset": location.end.offset
                }
            }),
        );
    }

    if let Some(expected) = &error.expected {
        details.insert(
            "expected".to_string(),
            json!(sigil_typechecker::format_type(expected)),
        );
    }

    if let Some(actual) = &error.actual {
        details.insert(
            "found".to_string(),
            json!(sigil_typechecker::format_type(actual)),
        );
    }

    if let Some(extra) = &error.details {
        for (key, value) in extra {
            details.insert(key.clone(), value.clone());
        }
    }

    serde_json::Value::Object(details)
}

pub(super) fn merge_json_details(
    base: serde_json::Value,
    extra: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut merged = match base {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    merged.extend(extra);
    serde_json::Value::Object(merged)
}

pub(super) fn project_error_json_details(
    project_error: &ProjectConfigError,
    path_key: &str,
    path: &Path,
    extra: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut details = match project_error.details() {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    details.insert(
        path_key.to_string(),
        json!(path.to_string_lossy().to_string()),
    );
    details.extend(extra);
    serde_json::Value::Object(details)
}

pub(super) fn validate_project_entrypoint_for_path(path: &Path) -> Result<(), CliError> {
    if let Some(project) = get_project_config(path)? {
        validate_project_default_entrypoint(&project)?;
    }
    Ok(())
}

pub(super) fn validate_project_entrypoints_for_files(files: &[PathBuf]) -> Result<(), CliError> {
    let mut projects = BTreeMap::new();

    for file in files {
        if let Some(project) = get_project_config(file)? {
            projects.entry(project.root.clone()).or_insert(project);
        }
    }

    for project in projects.values() {
        validate_project_default_entrypoint(project)?;
    }

    Ok(())
}

pub(super) fn output_inspect_error(
    command: &str,
    file: &Path,
    error: &CliError,
    extra_details: serde_json::Map<String, serde_json::Value>,
) {
    match error {
        CliError::Type(type_error) => output_json_error(
            command,
            "typecheck",
            &type_error.code,
            &type_error.message,
            merge_json_details(type_error_json_details(type_error), extra_details),
        ),
        CliError::ModuleGraph(ModuleGraphError::Validation(errors)) => {
            let message = errors
                .first()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "validation errors".to_string());
            let error_code = extract_error_code(&message);
            output_json_error(
                command,
                "canonical",
                &error_code,
                &message,
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy(),
                        "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::ModuleGraph(ModuleGraphError::ImportNotFound {
            module_id,
            expected_path,
        }) => output_json_error(
            command,
            "cli",
            codes::cli::IMPORT_NOT_FOUND,
            &format!("module not found: {}", module_id),
            merge_json_details(
                json!({
                    "file": file.to_string_lossy(),
                    "moduleId": module_id,
                    "expectedPath": expected_path
                }),
                extra_details,
            ),
        ),
        CliError::ModuleGraph(ModuleGraphError::ImportCycle(cycle)) => output_json_error(
            command,
            "cli",
            codes::cli::IMPORT_CYCLE,
            "module import cycle detected",
            merge_json_details(
                json!({
                    "file": file.to_string_lossy(),
                    "cycle": cycle
                }),
                extra_details,
            ),
        ),
        CliError::ModuleGraph(ModuleGraphError::SelectedConfigEnvRequired)
        | CliError::ModuleGraph(ModuleGraphError::SelectedConfigModuleNotFound { .. }) => {
            let message = error.to_string();
            let error_code = extract_error_code(&message);
            output_json_error(
                command,
                phase_for_code(&error_code),
                &error_code,
                &message,
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::ModuleGraph(ModuleGraphError::Lexer(message))
        | CliError::Lexer(message)
        | CliError::ModuleGraph(ModuleGraphError::Parser(message))
        | CliError::Parser(message)
        | CliError::Validation(message)
        | CliError::Runtime(message) => {
            let error_code = extract_error_code(message);
            let phase = if error_code.starts_with("SIGIL-") {
                phase_for_code(&error_code)
            } else {
                match error {
                    CliError::ModuleGraph(ModuleGraphError::Lexer(_)) | CliError::Lexer(_) => {
                        "lexer"
                    }
                    CliError::ModuleGraph(ModuleGraphError::Parser(_)) | CliError::Parser(_) => {
                        "parser"
                    }
                    CliError::Validation(_) => "canonical",
                    CliError::Runtime(_) => "runtime",
                    _ => "cli",
                }
            };
            output_json_error(
                command,
                phase,
                if error_code.starts_with("SIGIL-") {
                    &error_code
                } else {
                    codes::cli::UNEXPECTED
                },
                message,
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
        | CliError::ProjectConfig(project_error) => output_json_error(
            command,
            phase_for_code(project_error.code()),
            project_error.code(),
            &project_error.to_string(),
            project_error_json_details(project_error, "file", file, extra_details),
        ),
        CliError::Io(error) | CliError::ModuleGraph(ModuleGraphError::Io(error)) => {
            output_json_error(
                command,
                "io",
                codes::cli::UNEXPECTED,
                &error.to_string(),
                merge_json_details(
                    json!({
                        "file": file.to_string_lossy()
                    }),
                    extra_details,
                ),
            );
        }
        CliError::Codegen(message) => output_json_error(
            command,
            "codegen",
            codes::cli::UNEXPECTED,
            message,
            merge_json_details(
                json!({
                    "file": file.to_string_lossy()
                }),
                extra_details,
            ),
        ),
        CliError::Breakpoint {
            code,
            message,
            details,
        } => output_json_error(
            command,
            phase_for_code(code),
            code,
            message,
            merge_json_details(details.clone(), extra_details),
        ),
        CliError::Reported(_) => {}
    }
}

pub(super) fn phase_for_code(code: &str) -> &'static str {
    if code.starts_with("SIGIL-LEX-") {
        "lexer"
    } else if code.starts_with("SIGIL-PARSE-") {
        "parser"
    } else if code.starts_with("SIGIL-CANON-") {
        "canonical"
    } else if code.starts_with("SIGIL-TYPE-") {
        "typecheck"
    } else if code.starts_with("SIGIL-TOPO-") {
        "topology"
    } else if code.starts_with("SIGIL-RUNTIME-") || code.starts_with("SIGIL-RUN-") {
        "runtime"
    } else if code.starts_with("SIGIL-MUTABILITY-") {
        "mutability"
    } else {
        "cli"
    }
}
