use super::legacy::CliError;
use crate::module_graph::ModuleGraphError;
use crate::project::{get_project_config, validate_project_default_entrypoint, ProjectConfigError};
use serde_json::json;
use sigil_diagnostics::codes;
use sigil_typechecker::TypeError;
use sigil_validator::ValidationError;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) const MACHINE_FORMAT_VERSION: u8 = 1;
pub(super) const COMPILER_VERSION: &str = match option_env!("SIGIL_VERSION") {
    Some(version) => version,
    None => "dev",
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct SourcePoint {
    pub line: usize,
    pub column: usize,
}

pub(super) fn extract_error_code(message: &str) -> String {
    if let Some(index) = message.find("SIGIL-") {
        let suffix = &message[index..];
        let end = suffix
            .char_indices()
            .find_map(|(index, character)| {
                (character == ':' || character.is_whitespace()).then_some(index)
            })
            .unwrap_or(suffix.len());
        return suffix[..end].to_string();
    }
    "SIGIL-CLI-UNEXPECTED".to_string()
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
        "formatVersion": MACHINE_FORMAT_VERSION,
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

pub(crate) fn output_json_value(output: &serde_json::Value, to_stderr: bool) {
    let normalized = canonical_machine_output(output.clone());
    let serialized = serde_json::to_string(&normalized).unwrap();
    if to_stderr {
        eprintln!("{}", serialized);
    } else {
        println!("{}", serialized);
    }
}

pub(super) fn canonical_machine_output(mut output: serde_json::Value) -> serde_json::Value {
    let Some(object) = output.as_object_mut() else {
        return json!({
            "formatVersion": MACHINE_FORMAT_VERSION,
            "compilerVersion": COMPILER_VERSION,
            "command": "sigil",
            "ok": false,
            "phase": "internal",
            "analysis": {"status": "failed", "level": "none"},
            "data": {},
            "diagnostics": [{
                "code": "SIGIL-CLI-UNEXPECTED",
                "phase": "internal",
                "severity": "error",
                "message": "command produced a non-object machine result",
                "details": {"value": output}
            }]
        });
    };

    object.insert("formatVersion".to_string(), json!(MACHINE_FORMAT_VERSION));
    object.insert("compilerVersion".to_string(), json!(COMPILER_VERSION));

    let command = object
        .get("command")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("sigil");
    let normalized_command = command
        .strip_prefix("sigilc")
        .map(|suffix| format!("sigil{suffix}"))
        .unwrap_or_else(|| command.to_string());
    object.insert("command".to_string(), json!(&normalized_command));

    let ok = object
        .get("ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    object.insert("ok".to_string(), json!(ok));

    let phase = object
        .get("phase")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| default_phase_for_command(&normalized_command, ok))
        .to_string();
    object.insert("phase".to_string(), json!(&phase));

    let mut data = object
        .remove("data")
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if !data.is_object() {
        data = json!({"value": data});
    }
    if object.contains_key("summary") || object.contains_key("results") {
        let data_object = data.as_object_mut().expect("data normalized to object");
        if let Some(summary) = object.remove("summary") {
            data_object.insert("summary".to_string(), summary);
        }
        if let Some(results) = object.remove("results") {
            data_object.insert("results".to_string(), results);
        }
    }
    remove_volatile_fields(&mut data);
    object.insert("data".to_string(), data);

    let mut diagnostics = object
        .remove("diagnostics")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if let Some(error) = object.remove("error") {
        diagnostics.insert(0, error);
    }
    if !ok && diagnostics.is_empty() {
        let (code, message, details) = if normalized_command == "sigil test" {
            (
                codes::cli::TESTS_FAILED,
                "one or more Sigil tests did not pass",
                json!({"summary": object["data"]["summary"]}),
            )
        } else if normalized_command == "sigil review" {
            (
                codes::cli::REVIEW_FAILED,
                "semantic review could not complete without errors",
                json!({"issues": object["data"]["issues"]}),
            )
        } else {
            (
                codes::cli::UNEXPECTED,
                "command reported failure without a structured diagnostic",
                json!({}),
            )
        };
        diagnostics.push(json!({
            "code": code,
            "phase": phase,
            "severity": "error",
            "message": message,
            "details": details
        }));
    }
    for diagnostic in &mut diagnostics {
        if let Some(diagnostic_object) = diagnostic.as_object_mut() {
            if let Some(related_locations) = diagnostic_object.remove("related_locations") {
                diagnostic_object.insert("relatedLocations".to_string(), related_locations);
            }
            diagnostic_object
                .entry("severity".to_string())
                .or_insert_with(|| json!("error"));
            diagnostic_object
                .entry("phase".to_string())
                .or_insert_with(|| json!(&phase));
        }
        remove_volatile_fields(diagnostic);
    }
    object.insert("diagnostics".to_string(), json!(diagnostics));

    if !object.contains_key("analysis") {
        object.insert(
            "analysis".to_string(),
            json!({
                "status": if ok && matches!(phase.as_str(), "docs" | "package") {
                    "notApplicable"
                } else if ok {
                    "complete"
                } else {
                    "failed"
                },
                "level": analysis_level_for_phase(&phase, ok)
            }),
        );
    }

    output
}

fn remove_volatile_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("durationMs");
            for nested in object.values_mut() {
                remove_volatile_fields(nested);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                remove_volatile_fields(item);
            }
        }
        _ => {}
    }
}

fn default_phase_for_command(command: &str, ok: bool) -> &'static str {
    if command.starts_with("sigil test")
        || command.starts_with("sigil run")
        || command.starts_with("sigil debug")
    {
        "runtime"
    } else if command.starts_with("sigil package") {
        "package"
    } else if ok {
        "cli"
    } else {
        "internal"
    }
}

fn analysis_level_for_phase(phase: &str, ok: bool) -> &'static str {
    match phase {
        "lexer" => {
            if ok {
                "lexed"
            } else {
                "none"
            }
        }
        "parser" => {
            if ok {
                "parsed"
            } else {
                "lexed"
            }
        }
        "canonical" => {
            if ok {
                "canonical"
            } else {
                "parsed"
            }
        }
        "typecheck" | "proof" | "topology" | "mutability" | "extern" => {
            if ok {
                "typed"
            } else {
                "canonical"
            }
        }
        "codegen" => {
            if ok {
                "generated"
            } else {
                "typed"
            }
        }
        "runtime" => {
            if ok {
                "executed"
            } else {
                "generated"
            }
        }
        "surface" => "mixed",
        _ => "none",
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
    } else if code.starts_with("SIGIL-PROOF-") {
        "proof"
    } else if code.starts_with("SIGIL-PACKAGE-") {
        "package"
    } else if code.starts_with("SIGIL-RUNTIME-") || code.starts_with("SIGIL-RUN-") {
        "runtime"
    } else if code.starts_with("SIGIL-MUTABILITY-") {
        "mutability"
    } else {
        "cli"
    }
}

#[allow(dead_code)] // Used by the binary target; the library target has no top-level dispatcher.
pub(crate) fn output_unhandled_error(error: &CliError) {
    if matches!(error, CliError::Reported(_)) {
        return;
    }
    let message = error.to_string();
    let extracted = extract_error_code(&message);
    let code = if extracted.starts_with("SIGIL-") {
        extracted
    } else {
        match error {
            CliError::Io(_) | CliError::ModuleGraph(ModuleGraphError::Io(_)) => {
                "SIGIL-CLI-IO".to_string()
            }
            _ => codes::cli::UNEXPECTED.to_string(),
        }
    };
    let phase = match error {
        CliError::Lexer(_) | CliError::ModuleGraph(ModuleGraphError::Lexer(_)) => "lexer",
        CliError::Parser(_) | CliError::ModuleGraph(ModuleGraphError::Parser(_)) => "parser",
        CliError::Validation(_) | CliError::ModuleGraph(ModuleGraphError::Validation(_)) => {
            "canonical"
        }
        CliError::Type(_) => "typecheck",
        CliError::Codegen(_) => "codegen",
        CliError::Runtime(_) | CliError::Breakpoint { .. } => "runtime",
        CliError::Io(_) | CliError::ModuleGraph(ModuleGraphError::Io(_)) => "io",
        _ => phase_for_code(&code),
    };
    let details = match error {
        CliError::Type(type_error) => type_error_json_details(type_error),
        CliError::Breakpoint { details, .. } => details.clone(),
        _ => json!({}),
    };
    output_json_error("sigil", phase, &code, &message, details);
}

#[cfg(test)]
mod tests {
    use super::{canonical_machine_output, extract_error_code};
    use serde_json::json;

    #[test]
    fn canonicalizes_legacy_failure_into_ordered_diagnostics() {
        let output = canonical_machine_output(json!({
            "formatVersion": 1,
            "command": "sigilc compile",
            "ok": false,
            "phase": "parser",
            "error": {
                "code": "SIGIL-PARSE-TEST",
                "phase": "parser",
                "message": "bad source"
            }
        }));
        assert_eq!(output["command"], "sigil compile");
        assert_eq!(output["analysis"]["level"], "lexed");
        assert_eq!(output["data"], json!({}));
        assert_eq!(output["diagnostics"][0]["severity"], "error");
        assert!(output.get("error").is_none());
    }

    #[test]
    fn moves_test_summary_and_results_under_data() {
        let output = canonical_machine_output(json!({
            "formatVersion": 1,
            "command": "sigilc test",
            "ok": true,
            "summary": {"passed": 1},
            "results": [{"status": "passed"}]
        }));
        assert_eq!(output["phase"], "runtime");
        assert_eq!(output["analysis"]["level"], "executed");
        assert_eq!(output["data"]["summary"]["passed"], 1);
        assert_eq!(output["data"]["results"][0]["status"], "passed");
    }

    #[test]
    fn extracts_only_the_stable_diagnostic_code() {
        assert_eq!(
            extract_error_code(
                "Parser error: SIGIL-PARSE-UNEXPECTED-TOKEN /tmp/main.sigil:2:3 unexpected token"
            ),
            "SIGIL-PARSE-UNEXPECTED-TOKEN"
        );
        assert_eq!(
            extract_error_code("Module graph error: module not found"),
            "SIGIL-CLI-UNEXPECTED"
        );
    }

    #[test]
    fn failed_test_results_receive_a_primary_diagnostic() {
        let output = canonical_machine_output(json!({
            "command": "sigilc test",
            "ok": false,
            "summary": {"failed": 1},
            "results": [{"status": "fail"}]
        }));
        assert_eq!(
            output["diagnostics"][0]["code"],
            sigil_diagnostics::codes::cli::TESTS_FAILED
        );
        assert_eq!(output["diagnostics"][0]["details"]["summary"]["failed"], 1);
    }
}
