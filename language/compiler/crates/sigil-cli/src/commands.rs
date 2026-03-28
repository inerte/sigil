//! Command implementations for CLI

use crate::module_graph::{
    entry_module_key, load_project_effect_catalog_for, LoadedModule, ModuleGraph, ModuleGraphError,
};
use crate::project::{get_project_config, ProjectConfigError};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use rayon::{prelude::*, ThreadPoolBuilder};
use serde_json::json;
use sigil_ast::{Declaration, Program, Type, TypeDef};
use sigil_codegen::{CodegenOptions, TypeScriptGenerator};
use sigil_diagnostics::codes;
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::types::{
    InferenceType, TConstructor, TFunction, TList, TMap, TRecord, TTuple,
};
use sigil_typechecker::{
    type_check, TypeCheckOptions, TypeError, TypeInfo, TypeScheme, TypedDeclaration, TypedProgram,
};
use sigil_validator::{
    validate_canonical_form_with_options, validate_typed_canonical_form, ValidationError,
    ValidationOptions,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Instant;
use thiserror::Error;

const TEST_WORKER_STACK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Lexer error: {0}")]
    Lexer(String),

    #[error("Parser error: {0}")]
    Parser(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Type error: {0}")]
    Type(#[from] TypeError),

    #[error("Codegen error: {0}")]
    Codegen(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Module graph error: {0}")]
    ModuleGraph(#[from] ModuleGraphError),

    #[error("Project config error: {0}")]
    ProjectConfig(#[from] ProjectConfigError),

    #[error("reported")]
    Reported(i32),
}

impl CliError {
    pub fn reported_exit_code(&self) -> Option<i32> {
        match self {
            CliError::Reported(exit_code) => Some(*exit_code),
            _ => None,
        }
    }
}

/// Extract error code from error message (format: "SIGIL-CANON-XXX: message")
fn extract_error_code(message: &str) -> String {
    if let Some(colon_pos) = message.find(':') {
        message[..colon_pos].to_string()
    } else {
        "SIGIL-ERROR".to_string()
    }
}

fn format_validation_errors(errors: &[ValidationError]) -> String {
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

/// Output a JSON error message matching the Sigil CLI format
fn output_json_error(
    command: &str,
    phase: &str,
    error_code: &str,
    message: &str,
    details: serde_json::Value,
) {
    output_json_error_to(command, phase, error_code, message, details, false);
}

fn output_json_error_to(
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

fn output_json_value(output: &serde_json::Value, to_stderr: bool) {
    let serialized = serde_json::to_string(output).unwrap();
    if to_stderr {
        eprintln!("{}", serialized);
    } else {
        println!("{}", serialized);
    }
}

fn type_error_json_details(error: &TypeError) -> serde_json::Value {
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

struct CompileDirectoryIgnore {
    root: PathBuf,
    explicit_paths: Vec<PathBuf>,
    gitignore: Option<Gitignore>,
}

impl CompileDirectoryIgnore {
    fn new(
        root: &Path,
        ignore_paths: &[PathBuf],
        ignore_from: Option<&Path>,
    ) -> Result<Self, CliError> {
        let root = fs::canonicalize(root)?;
        let explicit_paths = ignore_paths
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    Ok(path.to_path_buf())
                } else {
                    Ok(root.join(path))
                }
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;

        let gitignore = if let Some(ignore_from) = ignore_from {
            let resolved_ignore_from = if ignore_from.is_absolute() {
                ignore_from.to_path_buf()
            } else {
                std::env::current_dir()?.join(ignore_from)
            };
            let mut builder = GitignoreBuilder::new(&root);
            if let Some(error) = builder.add(&resolved_ignore_from) {
                return Err(CliError::Validation(format!(
                    "failed to load ignore rules from '{}': {}",
                    resolved_ignore_from.display(),
                    error
                )));
            }
            Some(builder.build().map_err(|error| {
                CliError::Validation(format!(
                    "failed to parse ignore rules from '{}': {}",
                    resolved_ignore_from.display(),
                    error
                ))
            })?)
        } else {
            None
        };

        Ok(Self {
            root,
            explicit_paths,
            gitignore,
        })
    }

    fn should_ignore(&self, path: &Path, is_dir: bool) -> bool {
        if path
            .components()
            .any(|component| component.as_os_str() == ".local")
        {
            return true;
        }

        if self
            .explicit_paths
            .iter()
            .any(|ignore| path.starts_with(ignore))
        {
            return true;
        }

        self.gitignore.as_ref().is_some_and(|gitignore| {
            gitignore
                .matched_path_or_any_parents(path, is_dir)
                .is_ignore()
        })
    }
}

#[derive(Clone)]
struct CompileBatchGroup {
    first_index: usize,
    files: Vec<PathBuf>,
}

struct CompileEntryOutput {
    input: PathBuf,
    output: PathBuf,
    project_root: Option<PathBuf>,
}

struct CompileBatchOutputs {
    compiled_modules: usize,
    entries: Vec<CompileEntryOutput>,
}

/// Lex command: tokenize a Sigil file
pub fn lex_command(file: &Path) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| CliError::Lexer(format!("{}", e)))?;

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc lex",
        "ok": true,
        "phase": "lexer",
        "data": {
            "file": filename,
            "summary": {
                "tokens": tokens.len()
            },
            "tokens": tokens.iter().map(|t| {
                serde_json::json!({
                    "type": format!("{:?}", t.token_type),
                    "lexeme": &t.value,
                    "start": {
                        "line": t.location.start.line,
                        "column": t.location.start.column,
                        "offset": t.location.start.offset
                    },
                    "end": {
                        "line": t.location.end.line,
                        "column": t.location.end.column,
                        "offset": t.location.end.offset
                    },
                    "text": format!("{}({}) at {}:{}", format!("{:?}", t.token_type), &t.value, t.location.start.line, t.location.start.column)
                })
            }).collect::<Vec<_>>()
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());

    Ok(())
}

/// Parse command: parse a Sigil file to AST
pub fn parse_command(file: &Path) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| CliError::Lexer(format!("{}", e)))?;
    let token_count = tokens.len(); // Store token count for JSON output

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser
        .parse()
        .map_err(|e| CliError::Parser(format!("{}", e)))?;

    let effect_catalog = load_project_effect_catalog_for(file)?;

    // Validate canonical form (includes formatting)
    validate_canonical_form_with_options(
        &ast,
        Some(&filename),
        Some(&source),
        ValidationOptions { effect_catalog },
    )
    .map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format_validation_errors(&errors))
    })?;

    let ast_json = serde_json::to_value(&ast).unwrap_or_else(|e| {
        eprintln!("Warning: AST serialization failed: {}", e);
        serde_json::json!(format!("{:#?}", ast))
    });

    let output = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc parse",
        "ok": true,
        "phase": "parser",
        "data": {
            "file": filename,
            "summary": {
                "tokens": token_count,
                "declarations": ast.declarations.len()
            },
            "ast": ast_json
        }
    });
    println!("{}", serde_json::to_string(&output).unwrap());

    Ok(())
}

fn is_sigil_source_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("sigil")
}

fn walk_compile_directory(
    dir: &Path,
    ignore: &CompileDirectoryIgnore,
    files: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let mut entries = fs::read_dir(dir)?
        .collect::<Result<Vec<_>, std::io::Error>>()?
        .into_iter()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        let is_dir = path.is_dir();
        if ignore.should_ignore(&path, is_dir) {
            continue;
        }

        if is_dir {
            walk_compile_directory(&path, ignore, files)?;
        } else if is_sigil_source_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn collect_compile_targets(
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<Vec<PathBuf>, CliError> {
    if is_sigil_source_file(path) {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_file() {
        return Err(CliError::Validation(format!(
            "compile expects a .sigil file or directory, got '{}'",
            path.display()
        )));
    }

    let ignore = CompileDirectoryIgnore::new(path, ignore_paths, ignore_from)?;
    let mut files = Vec::new();
    walk_compile_directory(&ignore.root, &ignore, &mut files)?;
    files.sort();
    Ok(files)
}

fn group_compile_targets(files: &[PathBuf]) -> Result<Vec<CompileBatchGroup>, CliError> {
    let mut project_buckets: HashMap<PathBuf, Vec<(usize, PathBuf, String)>> = HashMap::new();
    let mut standalone_bucket: Vec<(usize, PathBuf, String)> = Vec::new();

    for (index, file) in files.iter().enumerate() {
        let module_key = entry_module_key(file)?;
        if let Some(project) = get_project_config(file)? {
            project_buckets
                .entry(project.root.clone())
                .or_default()
                .push((index, file.clone(), module_key));
        } else {
            standalone_bucket.push((index, file.clone(), module_key));
        }
    }

    let mut groups = Vec::new();

    let mut project_roots = project_buckets.keys().cloned().collect::<Vec<_>>();
    project_roots.sort();
    for root in project_roots {
        let mut bucket = project_buckets.remove(&root).unwrap_or_default();
        bucket.sort_by(|a, b| a.1.cmp(&b.1));
        let mut packed_groups: Vec<(CompileBatchGroup, HashSet<String>)> = Vec::new();
        for (index, file, module_key) in bucket {
            if let Some((group, seen_keys)) = packed_groups
                .iter_mut()
                .find(|(_, seen_keys)| !seen_keys.contains(&module_key))
            {
                group.files.push(file);
                seen_keys.insert(module_key);
            } else {
                let mut seen_keys = HashSet::new();
                seen_keys.insert(module_key);
                packed_groups.push((
                    CompileBatchGroup {
                        first_index: index,
                        files: vec![file],
                    },
                    seen_keys,
                ));
            }
        }
        groups.extend(packed_groups.into_iter().map(|(group, _)| group));
    }

    if !standalone_bucket.is_empty() {
        standalone_bucket.sort_by(|a, b| a.1.cmp(&b.1));
        groups.push(CompileBatchGroup {
            first_index: standalone_bucket
                .iter()
                .map(|(index, _, _)| *index)
                .min()
                .unwrap_or(0),
            files: standalone_bucket
                .into_iter()
                .map(|(_, file, _)| file)
                .collect(),
        });
    }

    groups.sort_by_key(|group| group.first_index);
    for group in &mut groups {
        group.files.sort();
    }
    Ok(groups)
}

fn compile_group(group: &CompileBatchGroup) -> Result<CompileBatchOutputs, CliError> {
    let graph = ModuleGraph::build_many(&group.files)?;
    let compiled_modules = graph.topo_order.len();
    let entry_modules = group
        .files
        .iter()
        .map(|file| {
            let module_key = entry_module_key(file)?;
            let module = graph.modules.get(&module_key).ok_or_else(|| {
                CliError::Codegen(format!(
                    "batch compile could not resolve entry module '{}'",
                    file.display()
                ))
            })?;
            Ok((
                file.clone(),
                module_key,
                module.project.as_ref().map(|project| project.root.clone()),
            ))
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    let compiled = compile_module_graph(graph, None)?;
    let entries = entry_modules
        .into_iter()
        .map(|(input, module_id, project_root)| {
            let output = compiled
                .module_outputs
                .get(&module_id)
                .cloned()
                .ok_or_else(|| {
                    CliError::Codegen(format!(
                        "batch compile did not produce output for '{}'",
                        input.display()
                    ))
                })?;
            Ok(CompileEntryOutput {
                input,
                output,
                project_root,
            })
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    Ok(CompileBatchOutputs {
        compiled_modules,
        entries,
    })
}

fn compile_single_file_command(
    file: &Path,
    output: Option<&Path>,
    show_types: bool,
) -> Result<(), CliError> {
    let graph = match ModuleGraph::build(file) {
        Ok(graph) => graph,
        Err(ModuleGraphError::Validation(errors)) => {
            if let Some(first_error) = errors.first() {
                let error_msg = first_error.to_string();
                let error_code = extract_error_code(&error_msg);

                output_json_error(
                    "sigilc compile",
                    "canonical",
                    &error_code,
                    &error_msg,
                    json!({
                        "file": file.to_string_lossy(),
                        "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                    }),
                );
            }
            return Err(ModuleGraphError::Validation(errors).into());
        }
        Err(error) => return Err(error.into()),
    };

    let entry_module_id = graph.topo_order.last().unwrap().clone();
    let entry_module = graph.modules.get(&entry_module_id).unwrap();
    let all_module_sources = graph
        .topo_order
        .iter()
        .map(|module_id| {
            (
                module_id.clone(),
                graph.modules[module_id]
                    .file_path
                    .to_string_lossy()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let project_json = entry_module.project.as_ref().map(|project| {
        serde_json::json!({
            "root": project.root.to_string_lossy(),
            "layout": serde_json::to_value(&project.layout).unwrap_or(serde_json::json!({}))
        })
    });

    let compiled = match compile_module_graph(graph, output) {
        Ok(compiled) => compiled,
        Err(CliError::Type(type_error)) => {
            output_json_error(
                "sigilc compile",
                "typecheck",
                &type_error.code,
                &type_error.message,
                type_error_json_details(&type_error),
            );
            return Err(CliError::Type(type_error));
        }
        Err(error) => return Err(error),
    };
    let entry_output = compiled.entry_output_path.clone();

    let all_modules: Vec<serde_json::Value> = all_module_sources
        .into_iter()
        .map(|(module_id, source_file)| {
            let output_file = compiled
                .module_outputs
                .get(&module_id)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default();

            serde_json::json!({
                "moduleId": module_id,
                "sourceFile": source_file,
                "outputFile": output_file
            })
        })
        .collect();

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc compile",
        "ok": true,
        "phase": "codegen",
        "data": {
            "input": file.to_string_lossy(),
            "outputs": {
                "rootTs": entry_output.to_string_lossy(),
                "allModules": all_modules
            },
            "project": project_json,
            "typecheck": {
                "ok": true,
                "inferred": if show_types { vec![] as Vec<serde_json::Value> } else { vec![] }
            }
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

fn compile_directory_command(
    path: &Path,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    let start_time = Instant::now();
    let files = collect_compile_targets(path, ignore_paths, ignore_from)?;
    let groups = group_compile_targets(&files)?;
    let group_count = groups.len();
    let file_order = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<HashMap<_, _>>();

    let mut compiled_file_count = 0usize;
    let mut compiled_module_count = 0usize;
    let mut compiled_entries = Vec::new();

    for group in groups {
        let first_file = group.files.first().cloned();
        let batch = match compile_group(&group) {
            Ok(batch) => batch,
            Err(error) => {
                match &error {
                    CliError::Type(type_error) => {
                        let mut details = match type_error_json_details(type_error) {
                            serde_json::Value::Object(map) => map,
                            _ => serde_json::Map::new(),
                        };
                        details.insert(
                            "input".to_string(),
                            json!(path.to_string_lossy().to_string()),
                        );
                        details.insert(
                            "file".to_string(),
                            json!(type_error.source_file.clone().or_else(|| first_file
                                .as_ref()
                                .map(|file| file.to_string_lossy().to_string()))),
                        );
                        details.insert("discovered".to_string(), json!(files.len()));
                        details.insert("compiled".to_string(), json!(compiled_file_count));
                        details.insert(
                            "durationMs".to_string(),
                            json!(start_time.elapsed().as_millis()),
                        );
                        output_json_error(
                            "sigilc compile",
                            "typecheck",
                            &type_error.code,
                            &type_error.message,
                            serde_json::Value::Object(details),
                        );
                    }
                    _ => {
                        let message = error.to_string();
                        let error_code = extract_error_code(&message);
                        output_json_error(
                            "sigilc compile",
                            "codegen",
                            &error_code,
                            &message,
                            json!({
                                "input": path.to_string_lossy(),
                                "file": first_file.map(|file| file.to_string_lossy().to_string()),
                                "discovered": files.len(),
                                "compiled": compiled_file_count,
                                "durationMs": start_time.elapsed().as_millis()
                            }),
                        );
                    }
                }
                return Err(error);
            }
        };

        compiled_module_count += batch.compiled_modules;
        compiled_file_count += batch.entries.len();
        compiled_entries.extend(batch.entries);
    }

    compiled_entries
        .sort_by_key(|entry| file_order.get(&entry.input).copied().unwrap_or(usize::MAX));
    let file_results = compiled_entries
        .into_iter()
        .map(|entry| {
            serde_json::json!({
                "input": entry.input.to_string_lossy(),
                "rootTs": entry.output.to_string_lossy(),
                "projectRoot": entry.project_root.map(|root| root.to_string_lossy().to_string())
            })
        })
        .collect::<Vec<_>>();

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc compile",
        "ok": true,
        "phase": "codegen",
        "data": {
            "input": path.to_string_lossy(),
            "summary": {
                "discovered": files.len(),
                "compiled": compiled_file_count,
                "groups": group_count,
                "modules": compiled_module_count,
                "durationMs": start_time.elapsed().as_millis()
            },
            "files": file_results
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

/// Compile command: compile a Sigil file to TypeScript
pub fn compile_command(
    path: &Path,
    output: Option<&Path>,
    show_types: bool,
    ignore_paths: &[PathBuf],
    ignore_from: Option<&Path>,
) -> Result<(), CliError> {
    if path.is_dir() {
        if output.is_some() {
            return Err(CliError::Validation(
                "compile -o is only valid when compiling a single file".to_string(),
            ));
        }
        compile_directory_command(path, ignore_paths, ignore_from)
    } else {
        compile_single_file_command(path, output, show_types)
    }
}

/// Run command: compile and execute a Sigil file
pub fn run_command(
    file: &Path,
    json_output: bool,
    selected_env: Option<&str>,
    args: &[String],
) -> Result<(), CliError> {
    let run_target = match build_run_target(file, selected_env) {
        Ok(run_target) => run_target,
        Err(error) => {
            output_run_error(file, &error, !json_output);
            return Err(CliError::Reported(1));
        }
    };

    let runtime_output = match execute_runner(&run_target.runner_path, args, !json_output) {
        Ok(runtime_output) => runtime_output,
        Err(error) => {
            output_run_error(file, &error, !json_output);
            return Err(CliError::Reported(1));
        }
    };

    if runtime_output.exit_code != 0 {
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc run",
            "ok": false,
            "phase": "runtime",
            "error": {
                "code": codes::runtime::CHILD_EXIT,
                "phase": "runtime",
                "message": format!("child process exited with nonzero status: {}", runtime_output.exit_code),
                "details": {
                    "exitCode": runtime_output.exit_code,
                    "stdout": runtime_output.stdout,
                    "stderr": runtime_output.stderr
                }
            }
        });
        output_json_value(&output_json, !json_output);
        return Err(CliError::Reported(1));
    }

    if json_output {
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc run",
            "ok": true,
            "phase": "runtime",
            "data": {
                "compile": {
                    "input": file.to_string_lossy(),
                    "output": run_target.entry_output_path.to_string_lossy(),
                    "runnerFile": run_target.runner_path.to_string_lossy()
                },
                "runtime": {
                    "engine": "node+tsx",
                    "exitCode": runtime_output.exit_code,
                    "durationMs": runtime_output.duration_ms,
                    "stdout": runtime_output.stdout,
                    "stderr": runtime_output.stderr
                }
            }
        });
        output_json_value(&output_json, false);
    }

    Ok(())
}

struct RunTarget {
    entry_output_path: PathBuf,
    runner_path: PathBuf,
}

struct RuntimeOutput {
    exit_code: i32,
    duration_ms: u128,
    stdout: String,
    stderr: String,
}

fn build_run_target(file: &Path, selected_env: Option<&str>) -> Result<RunTarget, CliError> {
    let graph = ModuleGraph::build(file)?;
    let topology_prelude = runner_prelude(file, &graph, selected_env)?.unwrap_or_default();
    let compiled = compile_module_graph(graph, None)?;
    let entry_output_path = compiled.entry_output_path;

    let runner_path = entry_output_path.with_extension("run.ts");
    let module_name = entry_output_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let runner_code = format!(
        r#"{topology_prelude}
import {{ main }} from './{module_name}';

if (typeof main !== 'function') {{
  console.error('Error: No main() function found in {filename}');
  console.error('Add a main() function to make this program runnable.');
  process.exit(1);
}}

// Call main and handle the result (all Sigil functions are async)
const result = await main();

// If main returns a value (not Unit/undefined), show it
if (result !== undefined) {{
  console.log(result);
}}
"#,
        topology_prelude = topology_prelude,
        filename = file.to_string_lossy()
    );

    fs::write(&runner_path, runner_code)?;
    Ok(RunTarget {
        entry_output_path,
        runner_path,
    })
}

fn execute_runner(
    runner_path: &Path,
    args: &[String],
    stream_output: bool,
) -> Result<RuntimeOutput, CliError> {
    let abs_runner_path = std::fs::canonicalize(runner_path)?;
    let start_time = Instant::now();
    if stream_output {
        if io::stdout().is_terminal() || io::stderr().is_terminal() {
            let status = Command::new("pnpm")
                .args(["exec", "node", "--import", "tsx"])
                .arg(&abs_runner_path)
                .args(args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .map_err(map_runner_launch_error)?;

            return Ok(RuntimeOutput {
                exit_code: status.code().unwrap_or(-1),
                duration_ms: start_time.elapsed().as_millis(),
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        let mut child = Command::new("pnpm")
            .args(["exec", "node", "--import", "tsx"])
            .arg(&abs_runner_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(map_runner_launch_error)?;

        let stdout = child.stdout.take().ok_or_else(|| {
            CliError::Runtime(format!(
                "{}: failed to capture child stdout",
                codes::cli::UNEXPECTED
            ))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            CliError::Runtime(format!(
                "{}: failed to capture child stderr",
                codes::cli::UNEXPECTED
            ))
        })?;

        let stdout_handle = thread::spawn(move || tee_reader(stdout, io::stdout()));
        let stderr_handle = thread::spawn(move || tee_reader(stderr, io::stderr()));

        let status = child.wait()?;
        let stdout_bytes = join_tee_output(stdout_handle, "stdout")?;
        let stderr_bytes = join_tee_output(stderr_handle, "stderr")?;

        return Ok(RuntimeOutput {
            exit_code: status.code().unwrap_or(-1),
            duration_ms: start_time.elapsed().as_millis(),
            stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
            stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        });
    }

    let output = Command::new("pnpm")
        .args(["exec", "node", "--import", "tsx"])
        .arg(&abs_runner_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(map_runner_launch_error)?;

    Ok(RuntimeOutput {
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: start_time.elapsed().as_millis(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn tee_reader<R: Read, W: Write>(mut reader: R, mut writer: W) -> io::Result<Vec<u8>> {
    let mut capture = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        writer.flush()?;
        capture.extend_from_slice(&buffer[..read]);
    }
    Ok(capture)
}

fn join_tee_output(
    handle: thread::JoinHandle<io::Result<Vec<u8>>>,
    stream_name: &str,
) -> Result<Vec<u8>, CliError> {
    match handle.join() {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(error)) => Err(CliError::Io(error)),
        Err(_) => Err(CliError::Runtime(format!(
            "{}: run {} forwarding thread panicked",
            codes::cli::UNEXPECTED,
            stream_name
        ))),
    }
}

fn map_runner_launch_error(error: io::Error) -> CliError {
    if error.kind() == io::ErrorKind::NotFound {
        CliError::Runtime(format!(
            "{}: pnpm not found. Please install pnpm to run Sigil programs.",
            codes::runtime::ENGINE_NOT_FOUND
        ))
    } else {
        CliError::Runtime(format!(
            "{}: failed to execute run target: {}",
            codes::cli::UNEXPECTED,
            error
        ))
    }
}

fn output_run_error(file: &Path, error: &CliError, to_stderr: bool) {
    match error {
        CliError::Type(type_error) => output_json_error_to(
            "sigilc run",
            "typecheck",
            &type_error.code,
            &type_error.message,
            type_error_json_details(type_error),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::Validation(errors)) => {
            let message = errors
                .first()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "validation errors".to_string());
            let error_code = extract_error_code(&message);
            output_json_error_to(
                "sigilc run",
                "canonical",
                &error_code,
                &message,
                json!({
                    "file": file.to_string_lossy(),
                    "errors": errors.iter().map(|error| error.to_string()).collect::<Vec<_>>()
                }),
                to_stderr,
            );
        }
        CliError::ModuleGraph(ModuleGraphError::ImportNotFound {
            module_id,
            expected_path,
        }) => output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::IMPORT_NOT_FOUND,
            &format!("module not found: {}", module_id),
            json!({
                "file": file.to_string_lossy(),
                "moduleId": module_id,
                "expectedPath": expected_path
            }),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::ImportCycle(cycle)) => output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::IMPORT_CYCLE,
            "module import cycle detected",
            json!({
                "file": file.to_string_lossy(),
                "cycle": cycle
            }),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::Io(error)) => output_json_error_to(
            "sigilc run",
            "io",
            codes::cli::UNEXPECTED,
            &error.to_string(),
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::ModuleGraph(ModuleGraphError::Lexer(message))
        | CliError::ModuleGraph(ModuleGraphError::Parser(message))
        | CliError::Lexer(message)
        | CliError::Parser(message)
        | CliError::Validation(message)
        | CliError::Runtime(message) => {
            output_run_message_error(file, message, to_stderr);
        }
        CliError::ModuleGraph(ModuleGraphError::ProjectConfig(project_error))
        | CliError::ProjectConfig(project_error) => output_json_error_to(
            "sigilc run",
            "cli",
            codes::cli::UNEXPECTED,
            &project_error.to_string(),
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::Io(error) => output_json_error_to(
            "sigilc run",
            "io",
            codes::cli::UNEXPECTED,
            &error.to_string(),
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::Codegen(message) => output_json_error_to(
            "sigilc run",
            "codegen",
            codes::cli::UNEXPECTED,
            message,
            json!({
                "file": file.to_string_lossy()
            }),
            to_stderr,
        ),
        CliError::Reported(_) => {}
    }
}

fn output_run_message_error(file: &Path, message: &str, to_stderr: bool) {
    let error_code = extract_error_code(message);
    let (code, phase) = if error_code.starts_with("SIGIL-") {
        let phase = phase_for_code(&error_code);
        (error_code, phase)
    } else {
        (codes::cli::UNEXPECTED.to_string(), "cli")
    };

    output_json_error_to(
        "sigilc run",
        phase,
        &code,
        message,
        json!({
            "file": file.to_string_lossy()
        }),
        to_stderr,
    );
}

fn phase_for_code(code: &str) -> &'static str {
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

/// Test command: run Sigil tests from a directory
pub fn test_command(
    path: &Path,
    selected_env: Option<&str>,
    match_filter: Option<&str>,
) -> Result<(), CliError> {
    // Check if tests directory exists
    if !path.exists() {
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc test",
            "ok": true,
            "summary": {
                "files": 0,
                "discovered": 0,
                "selected": 0,
                "passed": 0,
                "failed": 0,
                "errored": 0,
                "skipped": 0,
                "durationMs": 0
            },
            "results": []
        });
        println!("{}", serde_json::to_string(&output_json).unwrap());
        return Ok(());
    }

    let start_time = Instant::now();
    let enforce_project_coverage = match_filter.is_none() && !path.is_file();

    // Collect all .sigil files in test directory
    let test_files = collect_sigil_files(path)?;

    let run_test_file = |test_file: &PathBuf| {
        compile_and_run_tests(test_file, selected_env, match_filter).map_err(|e| {
            eprintln!("Error running tests in {}: {}", test_file.display(), e);
            e
        })
    };

    let results: Vec<_> = if test_files.len() <= 1 {
        test_files.iter().map(run_test_file).collect()
    } else {
        // The SSG integration suite overflows Rayon’s default worker stack on Linux.
        // Use an explicit pool so `sigil test <dir>` is stable in CI without env hacks.
        let thread_pool = ThreadPoolBuilder::new()
            .thread_name(|index| format!("sigil-test-{index}"))
            .stack_size(TEST_WORKER_STACK_BYTES)
            .build()
            .map_err(|err| {
                CliError::Runtime(format!("Failed to configure test worker pool: {}", err))
            })?;

        thread_pool.install(|| test_files.par_iter().map(run_test_file).collect())
    };

    // Aggregate results from all files
    let mut all_results = Vec::new();
    let mut observed_calls = HashSet::new();
    let mut observed_variants: HashMap<String, HashSet<String>> = HashMap::new();
    let mut coverage_targets = HashMap::new();
    let mut discovered = 0;
    let mut selected = 0;

    for result in results {
        if let Ok(test_result) = result {
            discovered += test_result.discovered;
            selected += test_result.selected;
            observed_calls.extend(test_result.coverage_observation.calls);
            for (key, tags) in test_result.coverage_observation.variants {
                observed_variants.entry(key).or_default().extend(tags);
            }
            for target in test_result.coverage_targets {
                coverage_targets.entry(target.id.clone()).or_insert(target);
            }
            all_results.extend(test_result.results);
        }
    }

    if enforce_project_coverage {
        for target in coverage_targets.into_values() {
            if !observed_calls.contains(&target.id) {
                all_results.push(TestResult {
                    id: format!("{}::coverage", target.id),
                    file: target.file.clone(),
                    name: format!("coverage {}", target.function_name),
                    status: "fail".to_string(),
                    duration_ms: 0,
                    location: target.location.clone(),
                    failure: Some(format!(
                        "sigil test requires '{}' to be executed by the test suite",
                        target.id
                    )),
                });
                continue;
            }

            if !target.expected_variants.is_empty() {
                let observed = observed_variants.get(&target.id);
                let mut missing = target
                    .expected_variants
                    .iter()
                    .filter(|variant| {
                        observed.is_none_or(|tags| !tags.contains((*variant).as_str()))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                missing.sort();

                if !missing.is_empty() {
                    all_results.push(TestResult {
                        id: format!("{}::coverage-variants", target.id),
                        file: target.file.clone(),
                        name: format!("coverage variants {}", target.function_name),
                        status: "fail".to_string(),
                        duration_ms: 0,
                        location: target.location.clone(),
                        failure: Some(format!(
                            "sigil test requires '{}' to observe variants [{}]",
                            target.id,
                            missing.join(", ")
                        )),
                    });
                }
            }
        }
    }

    // Sort results by file, then line, then column
    all_results.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.location.line.cmp(&b.location.line))
            .then_with(|| a.location.column.cmp(&b.location.column))
    });

    let passed = all_results.iter().filter(|r| r.status == "pass").count();
    let failed = all_results.iter().filter(|r| r.status == "fail").count();
    let errored = all_results.iter().filter(|r| r.status == "error").count();
    let duration_ms = start_time.elapsed().as_millis();

    let ok = failed == 0 && errored == 0;

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc test",
        "ok": ok,
        "summary": {
            "files": test_files.len(),
            "discovered": discovered,
            "selected": selected,
            "passed": passed,
            "failed": failed,
            "errored": errored,
            "skipped": 0,
            "durationMs": duration_ms
        },
        "results": all_results
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    if !ok {
        return Err(CliError::Runtime("Tests failed".to_string()));
    }

    Ok(())
}

pub fn validate_command(path: &Path, env: &str) -> Result<(), CliError> {
    let project_root = get_project_config(path)?
        .map(|project| project.root)
        .ok_or_else(|| {
            CliError::Validation(format!(
                "{}: no Sigil project found while validating topology",
                codes::topology::MISSING_MODULE
            ))
        })?;

    if !topology_source_path(&project_root).exists() {
        return Err(CliError::Validation(format!(
            "{}: topology-aware projects require src/topology.lib.sigil",
            codes::topology::MISSING_MODULE
        )));
    }

    let _compiled = compile_topology_module(&project_root)?;
    let prelude = build_world_runtime_prelude(&project_root, env, true)?;
    let runner_path = project_root.join(".local/topology.validate.run.ts");
    if let Some(parent) = runner_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(
        &runner_path,
        format!(
            r#"{prelude}
console.log(JSON.stringify({{
  ok: true,
  environment: {env_json}
}}));
"#,
            prelude = prelude,
            env_json = serde_json::to_string(env).unwrap()
        ),
    )?;

    let abs_runner = fs::canonicalize(&runner_path)?;
    let output = Command::new("pnpm")
        .args(&["exec", "node", "--import", "tsx"])
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime(
                    "pnpm not found. Please install pnpm to validate Sigil topology.".to_string(),
                )
            } else {
                CliError::Runtime(format!("Failed to execute topology validation: {}", e))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr.trim();
        return Err(CliError::Validation(if message.is_empty() {
            "topology validation failed".to_string()
        } else {
            message.to_string()
        }));
    }

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc validate",
        "ok": true,
        "phase": "topology",
        "data": {
            "environment": env,
            "projectRoot": project_root.to_string_lossy()
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
struct TestResult {
    id: String,
    file: String,
    name: String,
    status: String,
    #[serde(rename = "durationMs")]
    duration_ms: u128,
    location: TestLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TestLocation {
    line: usize,
    column: usize,
}

struct TestRunResult {
    discovered: usize,
    selected: usize,
    results: Vec<TestResult>,
    coverage_observation: CoverageObservation,
    coverage_targets: Vec<CoverageTarget>,
}

#[derive(Debug, Clone)]
struct CoverageTarget {
    id: String,
    expected_variants: Vec<String>,
    file: String,
    function_name: String,
    location: TestLocation,
}

#[derive(Debug, Clone, Default)]
struct CoverageObservation {
    calls: HashSet<String>,
    variants: HashMap<String, HashSet<String>>,
}

struct CompiledGraphOutputs {
    entry_output_path: PathBuf,
    module_outputs: HashMap<String, PathBuf>,
    coverage_targets: Vec<CoverageTarget>,
}

fn compile_module_graph(
    graph: ModuleGraph,
    output_override: Option<&Path>,
) -> Result<CompiledGraphOutputs, CliError> {
    let mut compiled_modules = HashMap::new();
    let mut compiled_schemes = HashMap::new();
    let mut coverage_targets = Vec::new();
    let mut type_registries = HashMap::new();
    let mut module_outputs = HashMap::new();
    let mut entry_output_path = PathBuf::new();

    for module_id in &graph.topo_order {
        let module = &graph.modules[module_id];

        let imported_namespaces = build_imported_namespaces(&module.ast, &compiled_modules);
        let imported_type_regs = build_imported_type_registries(&module.ast, &type_registries);
        let imported_value_schemes = build_imported_value_schemes(&module.ast, &compiled_schemes);
        let effect_catalog = load_project_effect_catalog_for(&module.file_path)?;

        let typecheck_result = type_check(
            &module.ast,
            &module.source,
            Some(TypeCheckOptions {
                effect_catalog,
                imported_namespaces: Some(imported_namespaces),
                imported_type_registries: Some(imported_type_regs.clone()),
                imported_value_schemes: Some(imported_value_schemes),
                source_file: Some(module.file_path.to_string_lossy().to_string()),
            }),
        )
        .map_err(CliError::Type)?;

        validate_typed_canonical_form(
            &typecheck_result.typed_program,
            Some(module.file_path.to_string_lossy().as_ref()),
        )
        .map_err(|errors| CliError::Validation(format_validation_errors(&errors)))?;

        coverage_targets.extend(collect_module_coverage_targets(
            module,
            &typecheck_result.typed_program,
            &imported_type_regs,
            &typecheck_result
                .typed_program
                .declarations
                .iter()
                .filter_map(|decl| match decl {
                    TypedDeclaration::Type(type_decl) => Some((
                        type_decl.ast.name.clone(),
                        TypeInfo {
                            type_params: type_decl.ast.type_params.clone(),
                            definition: type_decl.ast.definition.clone(),
                            constraint: type_decl.ast.constraint.clone(),
                        },
                    )),
                    _ => None,
                })
                .collect(),
        ));

        let output_path =
            if module_id == graph.topo_order.last().unwrap() && output_override.is_some() {
                output_override.unwrap().to_path_buf()
            } else {
                get_module_output_path(module)
            };

        let codegen_options = CodegenOptions {
            module_id: Some(module_id.clone()),
            source_file: Some(module.file_path.to_string_lossy().to_string()),
            output_file: Some(output_path.to_string_lossy().to_string()),
        };
        let mut codegen = TypeScriptGenerator::new(codegen_options);
        let ts_code = codegen
            .generate(&typecheck_result.typed_program)
            .map_err(|e| CliError::Codegen(format!("{}", e)))?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&output_path, ts_code)?;
        module_outputs.insert(module_id.clone(), output_path.clone());

        if module_id == graph.topo_order.last().unwrap() {
            entry_output_path = output_path;
        }

        compiled_schemes.insert(
            module_id.clone(),
            typecheck_result.declaration_schemes.clone(),
        );
        compiled_modules.insert(module_id.clone(), typecheck_result.declaration_types);
        type_registries.insert(
            module_id.clone(),
            extract_type_registry(&module.ast, &module.file_path, module_id),
        );
    }

    Ok(CompiledGraphOutputs {
        entry_output_path,
        module_outputs,
        coverage_targets,
    })
}

fn topology_source_path(project_root: &Path) -> PathBuf {
    project_root.join("src/topology.lib.sigil")
}

fn config_source_path(project_root: &Path, env_name: &str) -> PathBuf {
    project_root
        .join("config")
        .join(format!("{}.lib.sigil", env_name))
}

fn compile_topology_module(project_root: &Path) -> Result<CompiledGraphOutputs, CliError> {
    let topology_source = topology_source_path(project_root);
    if !topology_source.exists() {
        return Err(CliError::Validation(format!(
            "{}: topology-aware projects require src/topology.lib.sigil",
            codes::topology::MISSING_MODULE
        )));
    }

    let graph = ModuleGraph::build(&topology_source)?;
    compile_module_graph(graph, None)
}

fn compile_config_module(
    project_root: &Path,
    env_name: &str,
) -> Result<CompiledGraphOutputs, CliError> {
    let config_source = config_source_path(project_root, env_name);
    if !config_source.exists() {
        return Err(CliError::Validation(format!(
            "{}: topology environment '{}' requires config/{}.lib.sigil",
            codes::topology::MISSING_CONFIG_MODULE,
            env_name,
            env_name
        )));
    }

    let graph = ModuleGraph::build(&config_source)?;
    compile_module_graph(graph, None)
}

fn build_world_runtime_prelude(
    project_root: &Path,
    env_name: &str,
    topology_present: bool,
) -> Result<String, CliError> {
    let topology_url = if topology_present {
        let topology_outputs = compile_topology_module(project_root)?;
        let topology_output = topology_outputs.entry_output_path;
        Some(format!(
            "file://{}",
            fs::canonicalize(topology_output)?.display()
        ))
    } else {
        None
    };
    let config_outputs = compile_config_module(project_root, env_name)?;
    let config_output = config_outputs.entry_output_path;
    let config_url = format!("file://{}", fs::canonicalize(config_output)?.display());
    let env_name_json = serde_json::to_string(env_name).unwrap();

    Ok(format!(
        r#"{topology_import}
const __sigil_config_module = await import("{config_url}");
const __sigil_config_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_config_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);
const __sigil_world_env_name = {env_name_json};

function __sigil_runtime_fail(code, message) {{
  const error = new Error(`${{code}}: ${{message}}`);
  error.sigilCode = code;
  throw error;
}}

function __sigil_runtime_collect_topology(moduleExports) {{
  const envs = new Set();
  const http = new Set();
  const tcp = new Set();
  for (const value of Object.values(moduleExports ?? {{}})) {{
    if (value?.__tag === 'Environment') {{
      envs.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'HttpServiceDependency') {{
      http.add(String(value.__fields?.[0] ?? ''));
    }} else if (value?.__tag === 'TcpServiceDependency') {{
      tcp.add(String(value.__fields?.[0] ?? ''));
    }}
  }}
  return {{ envs, http, tcp }};
}}

function __sigil_runtime_collect_world_dependency_names(entries, expectedTag) {{
  if (!Array.isArray(entries)) {{
    __sigil_runtime_fail("{binding_kind}", `world ${{
      expectedTag === 'HttpEntry' ? 'http' : 'tcp'
    }} entries must be a list`);
  }}
  const seen = new Set();
  for (const entry of entries) {{
    if (!entry || typeof entry !== 'object' || entry.__tag !== expectedTag) {{
      __sigil_runtime_fail("{binding_kind}", `world entries must be tagged as ${{expectedTag}}`);
    }}
    const dependencyName = String(entry.__fields?.[0]?.dependencyName ?? '');
    if (!dependencyName) {{
      __sigil_runtime_fail("{binding_kind}", 'world entries must include dependencyName');
    }}
    if (seen.has(dependencyName)) {{
      __sigil_runtime_fail("{duplicate_binding}", `duplicate world entry for '${{dependencyName}}'`);
    }}
    seen.add(dependencyName);
  }}
  return seen;
}}

function __sigil_runtime_read_world(configExports) {{
  const world = configExports.world;
  if (!world || typeof world !== 'object') {{
    __sigil_runtime_fail("{invalid_config}", "config module must export a 'world' value");
  }}
  for (const field of ['clock', 'fs', 'http', 'log', 'process', 'random', 'tcp', 'timer']) {{
    if (!(field in world)) {{
      __sigil_runtime_fail("{invalid_config}", `world is missing '${{field}}'`);
    }}
  }}
  return world;
}}

const __sigil_world_value = __sigil_runtime_read_world(__sigil_config_exports);
const __sigil_topology_info = __sigil_runtime_collect_topology(globalThis.__sigil_topology_exports ?? {{}});
if (__sigil_topology_info.envs.size > 0 && !__sigil_topology_info.envs.has(__sigil_world_env_name)) {{
  __sigil_runtime_fail("{env_not_found}", `environment '${{__sigil_world_env_name}}' not declared in src/topology.lib.sigil`);
}}
const __sigil_http_world_names = __sigil_runtime_collect_world_dependency_names(__sigil_world_value.http, 'HttpEntry');
const __sigil_tcp_world_names = __sigil_runtime_collect_world_dependency_names(__sigil_world_value.tcp, 'TcpEntry');
for (const dependencyName of __sigil_topology_info.http) {{
  if (!__sigil_http_world_names.has(dependencyName)) {{
    __sigil_runtime_fail("{missing_binding}", `missing HTTP world entry for '${{dependencyName}}' in environment '${{__sigil_world_env_name}}'`);
  }}
}}
for (const dependencyName of __sigil_topology_info.tcp) {{
  if (!__sigil_tcp_world_names.has(dependencyName)) {{
    __sigil_runtime_fail("{missing_binding}", `missing TCP world entry for '${{dependencyName}}' in environment '${{__sigil_world_env_name}}'`);
  }}
}}
for (const dependencyName of __sigil_http_world_names) {{
  if (__sigil_topology_info.http.size > 0 && !__sigil_topology_info.http.has(dependencyName)) {{
    __sigil_runtime_fail("{invalid_handle}", `HTTP world entry references undeclared dependency '${{dependencyName}}'`);
  }}
}}
for (const dependencyName of __sigil_tcp_world_names) {{
  if (__sigil_topology_info.tcp.size > 0 && !__sigil_topology_info.tcp.has(dependencyName)) {{
    __sigil_runtime_fail("{invalid_handle}", `TCP world entry references undeclared dependency '${{dependencyName}}'`);
  }}
}}
globalThis.__sigil_world_env_name = __sigil_world_env_name;
globalThis.__sigil_world_value = __sigil_world_value;
globalThis.__sigil_world_template_cache = undefined;
globalThis.__sigil_world_current = undefined;
"#,
        topology_import = topology_url.map_or_else(
            || "globalThis.__sigil_topology_exports = null;".to_string(),
            |topology_url| {
                format!(
                    r#"const __sigil_topology_module = await import("{topology_url}");
globalThis.__sigil_topology_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_topology_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);"#
                )
            }
        ),
        config_url = config_url,
        env_name_json = env_name_json,
        invalid_handle = codes::topology::INVALID_HANDLE,
        binding_kind = codes::topology::BINDING_KIND_MISMATCH,
        missing_binding = codes::topology::MISSING_BINDING,
        duplicate_binding = codes::topology::DUPLICATE_BINDING,
        env_not_found = codes::topology::ENV_NOT_FOUND,
        invalid_config = codes::topology::INVALID_CONFIG_MODULE,
    ))
}

fn project_root_and_runtime(
    path: &Path,
    graph: &ModuleGraph,
) -> Result<Option<(PathBuf, bool, bool)>, ProjectConfigError> {
    let Some(project) = get_project_config(path)? else {
        return Ok(None);
    };
    let topology_present = topology_source_path(&project.root).exists();
    let config_imported = graph
        .modules
        .keys()
        .any(|module_id| module_id.starts_with("config::"));
    Ok(Some((project.root, topology_present, config_imported)))
}

fn runner_prelude(
    path: &Path,
    graph: &ModuleGraph,
    selected_env: Option<&str>,
) -> Result<Option<String>, CliError> {
    let Some((project_root, topology_present, config_imported)) =
        project_root_and_runtime(path, graph)?
    else {
        return Ok(None);
    };

    if !topology_present && !config_imported {
        return Ok(None);
    }

    let env_name = selected_env.ok_or_else(|| {
        CliError::Validation(format!(
            "{}: runtime-world projects require --env <name>",
            codes::topology::ENV_REQUIRED
        ))
    })?;

    build_world_runtime_prelude(&project_root, env_name, topology_present).map(Some)
}

fn collect_sigil_files(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();

    if dir.is_file() && dir.extension().and_then(|s| s.to_str()) == Some("sigil") {
        files.push(dir.to_path_buf());
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(collect_sigil_files(&path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("sigil") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn compile_and_run_tests(
    file: &Path,
    selected_env: Option<&str>,
    match_filter: Option<&str>,
) -> Result<TestRunResult, CliError> {
    let graph = ModuleGraph::build(file)?;
    let topology_prelude = runner_prelude(file, &graph, selected_env)?;
    let compiled = compile_module_graph(graph, None)?;
    run_test_module(
        &compiled.entry_output_path,
        &compiled.coverage_targets,
        match_filter,
        &file.to_string_lossy(),
        topology_prelude.as_deref(),
    )
}

fn run_test_module(
    ts_file: &Path,
    coverage_targets: &[CoverageTarget],
    match_filter: Option<&str>,
    source_file: &str,
    topology_prelude: Option<&str>,
) -> Result<TestRunResult, CliError> {
    // Create test runner directory
    let test_dir = ts_file.parent().unwrap().join("__sigil_test");
    fs::create_dir_all(&test_dir)?;

    // Create unique runner file
    let unique = format!(
        "{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let runner_file = test_dir.join(format!(
        "{}.{}.runner.ts",
        ts_file.file_stem().unwrap().to_string_lossy(),
        unique
    ));

    // Canonicalize the TypeScript file path for import
    let abs_ts_file = fs::canonicalize(ts_file)?;
    let module_url = format!("file://{}", abs_ts_file.display());

    // Generate test runner code
    let match_text_json = match match_filter {
        Some(m) => format!("\"{}\"", m.replace('"', "\\\"")),
        None => "null".to_string(),
    };
    let coverage_targets_json = serde_json::to_string(
        &coverage_targets
            .iter()
            .map(|target| &target.id)
            .collect::<Vec<_>>(),
    )
    .unwrap();

    let runner_code = format!(
        r#"{topology_prelude}
const moduleUrl = "{module_url}";
const discoverMod = await import(moduleUrl);
const tests = Array.isArray(discoverMod.__sigil_tests) ? discoverMod.__sigil_tests : [];
const matchText = {match_text_json};
const selected = matchText ? tests.filter((t) => String(t.name).includes(matchText)) : tests;
const results = [];
const startSuite = Date.now();
for (const t of selected) {{
  const start = Date.now();
  try {{
    globalThis.__sigil_coverage_current = {{ calls: Object.create(null), variants: Object.create(null) }};
    const freshMod = await import(moduleUrl + '?sigil_test=' + encodeURIComponent(String(t.id)) + '&ts=' + Date.now() + '_' + Math.random());
    const freshTests = Array.isArray(freshMod.__sigil_tests) ? freshMod.__sigil_tests : [];
    const freshTest = freshTests.find((x) => x.id === t.id);
    if (!freshTest) {{ throw new Error('Test not found in isolated module reload: ' + String(t.id)); }}
    const value = await freshTest.fn();
    const coverageState = globalThis.__sigil_coverage_current ?? {{ calls: {{}}, variants: {{}} }};
    const coverage = {{
      calls: Object.entries(coverageState.calls ?? {{}})
        .filter(([, count]) => Number(count ?? 0) > 0)
        .map(([key]) => key),
      variants: Object.fromEntries(
        Object.entries(coverageState.variants ?? {{}}).map(([key, tags]) => [key, Array.isArray(tags) ? tags : []])
      )
    }};
    delete globalThis.__sigil_coverage_current;
    if (value === true) {{
      results.push({{ coverage, id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'pass', durationMs: Date.now()-start, location: t.location }});
    }} else if (value && typeof value === 'object' && 'ok' in value) {{
      if (value.ok === true) {{
        results.push({{ coverage, id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'pass', durationMs: Date.now()-start, location: t.location }});
      }} else {{
        results.push({{ coverage, id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, failure: value.failure ?? {{ kind: 'assert_false', message: 'Test body evaluated to false' }} }});
      }}
    }} else {{
      results.push({{ coverage, id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, failure: {{ kind: 'assert_false', message: 'Test body evaluated to false' }} }});
    }}
  }} catch (e) {{
    delete globalThis.__sigil_coverage_current;
    results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'error', durationMs: Date.now()-start, location: t.location, failure: {{ kind: 'exception', message: e instanceof Error ? e.message : String(e) }} }});
  }}
}}
console.log(JSON.stringify({{ coverageTargets: {coverage_targets_json}, results, discovered: tests.length, selected: selected.length, durationMs: Date.now()-startSuite }}));
"#,
        topology_prelude = topology_prelude.unwrap_or(""),
        coverage_targets_json = coverage_targets_json,
        module_url = module_url,
        match_text_json = match_text_json
    );

    fs::write(&runner_file, runner_code)?;

    // Execute runner
    let abs_runner = fs::canonicalize(&runner_file)?;
    let output = Command::new("pnpm")
        .args(&["exec", "node", "--import", "tsx"])
        .arg(&abs_runner)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime("pnpm not found".to_string())
            } else {
                CliError::Runtime(format!("Failed to execute test runner: {}", e))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError::Runtime(format!("Test runner failed: {}", stderr)));
    }

    // Parse test results
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| CliError::Runtime(format!("Failed to parse test output: {}", e)))?;

    let discovered = json["discovered"].as_u64().unwrap_or(0) as usize;
    let selected = json["selected"].as_u64().unwrap_or(0) as usize;

    let mut coverage_observation = CoverageObservation::default();
    let mut runner_coverage_targets = coverage_targets.to_vec();
    if let Some(targets) = json["coverageTargets"].as_array() {
        let selected_ids = targets
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<HashSet<_>>();
        runner_coverage_targets.retain(|target| selected_ids.contains(target.id.as_str()));
    }

    let mut results = Vec::new();
    if let Some(test_results) = json["results"].as_array() {
        for result in test_results {
            if let Some(call_keys) = result["coverage"]["calls"].as_array() {
                for key in call_keys.iter().filter_map(|value| value.as_str()) {
                    coverage_observation.calls.insert(key.to_string());
                }
            }
            if let Some(variant_map) = result["coverage"]["variants"].as_object() {
                for (key, tags) in variant_map {
                    let observed = coverage_observation
                        .variants
                        .entry(key.clone())
                        .or_default();
                    if let Some(tag_values) = tags.as_array() {
                        for tag in tag_values.iter().filter_map(|value| value.as_str()) {
                            observed.insert(tag.to_string());
                        }
                    }
                }
            }
            let test_result = TestResult {
                id: result["id"].as_str().unwrap_or("").to_string(),
                file: source_file.to_string(),
                name: result["name"].as_str().unwrap_or("").to_string(),
                status: result["status"].as_str().unwrap_or("unknown").to_string(),
                duration_ms: result["durationMs"].as_u64().unwrap_or(0) as u128,
                location: TestLocation {
                    line: result["location"]["start"]["line"].as_u64().unwrap_or(0) as usize,
                    column: result["location"]["start"]["column"].as_u64().unwrap_or(0) as usize,
                },
                failure: result["failure"]["message"].as_str().map(|s| s.to_string()),
            };
            results.push(test_result);
        }
    }

    Ok(TestRunResult {
        discovered,
        selected,
        results,
        coverage_observation,
        coverage_targets: runner_coverage_targets,
    })
}

// ============================================================================
// Multi-module Compilation Helpers
// ============================================================================

/// Build imported namespaces from already-compiled modules
///
/// For each import, creates a namespace type (record) containing exported functions/constants
fn build_imported_namespaces(
    _ast: &Program,
    compiled_modules: &HashMap<String, HashMap<String, InferenceType>>,
) -> HashMap<String, InferenceType> {
    let mut imported = HashMap::new();

    for (module_id, types) in compiled_modules {
        let mut fields = HashMap::new();
        for (name, typ) in types {
            fields.insert(
                name.clone(),
                qualify_inference_type_in_context(typ, module_id),
            );
        }

        imported.insert(
            module_id.clone(),
            InferenceType::Record(TRecord {
                fields,
                name: Some(module_id.clone()),
            }),
        );
    }

    imported
}

fn is_core_prelude_name(name: &str) -> bool {
    matches!(
        name,
        "ConcurrentOutcome"
            | "Option"
            | "Result"
            | "Aborted"
            | "Failure"
            | "Success"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
    )
}

fn qualify_inference_type_in_context(typ: &InferenceType, module_id: &str) -> InferenceType {
    match typ {
        InferenceType::Primitive(_) | InferenceType::Var(_) | InferenceType::Any => typ.clone(),
        InferenceType::Function(func) => InferenceType::Function(Box::new(TFunction {
            params: func
                .params
                .iter()
                .map(|param| qualify_inference_type_in_context(param, module_id))
                .collect(),
            return_type: qualify_inference_type_in_context(&func.return_type, module_id),
            effects: func.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: qualify_inference_type_in_context(&list.element_type, module_id),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: qualify_inference_type_in_context(&map.key_type, module_id),
            value_type: qualify_inference_type_in_context(&map.value_type, module_id),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| qualify_inference_type_in_context(item, module_id))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        qualify_inference_type_in_context(field_type, module_id),
                    )
                })
                .collect(),
            name: record.name.as_ref().map(|name| {
                if is_core_prelude_name(name) {
                    name.clone()
                } else if name.contains("::") {
                    name.clone()
                } else if name.contains('.') {
                    name.clone()
                } else {
                    format!("{}.{}", module_id, name)
                }
            }),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: if is_core_prelude_name(&constructor.name) {
                constructor.name.clone()
            } else if constructor.name.contains("::") || constructor.name.contains('.') {
                constructor.name.clone()
            } else {
                format!("{}.{}", module_id, constructor.name)
            },
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| qualify_inference_type_in_context(arg, module_id))
                .collect(),
        }),
    }
}

/// Build imported type registries from dependencies
///
/// Extracts type definitions (sum types, product types) from imported modules
fn build_imported_type_registries(
    _ast: &Program,
    type_registries: &HashMap<String, HashMap<String, TypeInfo>>,
) -> HashMap<String, HashMap<String, TypeInfo>> {
    type_registries.clone()
}

fn build_imported_value_schemes(
    _ast: &Program,
    compiled_schemes: &HashMap<String, HashMap<String, TypeScheme>>,
) -> HashMap<String, HashMap<String, TypeScheme>> {
    let mut imported = HashMap::new();

    for (module_id, schemes) in compiled_schemes {
        imported.insert(
            module_id.clone(),
            schemes
                .iter()
                .map(|(name, scheme)| {
                    (
                        name.clone(),
                        qualify_scheme_for_module(module_id.as_str(), scheme),
                    )
                })
                .collect(),
        );
    }

    imported
}

fn qualify_inference_type_for_module(
    module_id: &str,
    typ: &sigil_typechecker::InferenceType,
) -> sigil_typechecker::InferenceType {
    use sigil_typechecker::types::{TConstructor, TFunction, TList, TRecord, TTuple, TVar};
    use sigil_typechecker::InferenceType;

    match typ {
        InferenceType::Primitive(_) | InferenceType::Any => typ.clone(),
        InferenceType::Var(var) => InferenceType::Var(Box::new(TVar {
            id: var.id,
            name: var.name.clone(),
            instance: var
                .instance
                .as_ref()
                .map(|instance| qualify_inference_type_for_module(module_id, instance)),
        })),
        InferenceType::Function(function) => InferenceType::Function(Box::new(TFunction {
            params: function
                .params
                .iter()
                .map(|param| qualify_inference_type_for_module(module_id, param))
                .collect(),
            return_type: qualify_inference_type_for_module(module_id, &function.return_type),
            effects: function.effects.clone(),
        })),
        InferenceType::List(list) => InferenceType::List(Box::new(TList {
            element_type: qualify_inference_type_for_module(module_id, &list.element_type),
        })),
        InferenceType::Map(map) => InferenceType::Map(Box::new(TMap {
            key_type: qualify_inference_type_for_module(module_id, &map.key_type),
            value_type: qualify_inference_type_for_module(module_id, &map.value_type),
        })),
        InferenceType::Tuple(tuple) => InferenceType::Tuple(TTuple {
            types: tuple
                .types
                .iter()
                .map(|item| qualify_inference_type_for_module(module_id, item))
                .collect(),
        }),
        InferenceType::Record(record) => InferenceType::Record(TRecord {
            fields: record
                .fields
                .iter()
                .map(|(name, field_type)| {
                    (
                        name.clone(),
                        qualify_inference_type_for_module(module_id, field_type),
                    )
                })
                .collect(),
            name: record.name.as_ref().map(|name| {
                if is_core_prelude_name(name) {
                    name.clone()
                } else if name.contains('.') {
                    name.clone()
                } else {
                    format!("{}.{}", module_id, name)
                }
            }),
        }),
        InferenceType::Constructor(constructor) => InferenceType::Constructor(TConstructor {
            name: if is_core_prelude_name(&constructor.name) {
                constructor.name.clone()
            } else if constructor.name.contains('.') {
                constructor.name.clone()
            } else {
                format!("{}.{}", module_id, constructor.name)
            },
            type_args: constructor
                .type_args
                .iter()
                .map(|arg| qualify_inference_type_for_module(module_id, arg))
                .collect(),
        }),
    }
}

fn qualify_scheme_for_module(module_id: &str, scheme: &TypeScheme) -> TypeScheme {
    TypeScheme {
        quantified_vars: scheme.quantified_vars.clone(),
        typ: qualify_inference_type_for_module(module_id, &scheme.typ),
    }
}

fn qualify_type_in_context(
    ast_type: &Type,
    module_id: &str,
    local_type_registry: &HashMap<String, TypeInfo>,
    type_params: &[String],
) -> Type {
    match ast_type {
        Type::Primitive(_) => ast_type.clone(),
        Type::Qualified(qualified) => Type::Qualified(sigil_ast::QualifiedType {
            module_path: qualified.module_path.clone(),
            type_name: qualified.type_name.clone(),
            type_args: qualified
                .type_args
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            location: qualified.location,
        }),
        Type::List(list_type) => Type::List(Box::new(sigil_ast::ListType {
            element_type: qualify_type_in_context(
                &list_type.element_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: list_type.location,
        })),
        Type::Map(map_type) => Type::Map(Box::new(sigil_ast::MapType {
            key_type: qualify_type_in_context(
                &map_type.key_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            value_type: qualify_type_in_context(
                &map_type.value_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: map_type.location,
        })),
        Type::Function(func_type) => Type::Function(Box::new(sigil_ast::FunctionType {
            param_types: func_type
                .param_types
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            effects: func_type.effects.clone(),
            return_type: qualify_type_in_context(
                &func_type.return_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: func_type.location,
        })),
        Type::Tuple(tuple_type) => Type::Tuple(sigil_ast::TupleType {
            types: tuple_type
                .types
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect(),
            location: tuple_type.location,
        }),
        Type::Variable(var_type) => {
            if is_core_prelude_name(&var_type.name)
                || type_params.contains(&var_type.name)
                || !local_type_registry.contains_key(&var_type.name)
            {
                return ast_type.clone();
            }

            Type::Qualified(sigil_ast::QualifiedType {
                module_path: module_id.split("::").map(|s| s.to_string()).collect(),
                type_name: var_type.name.clone(),
                type_args: vec![],
                location: var_type.location,
            })
        }
        Type::Constructor(constructor) => {
            let qualified_args = constructor
                .type_args
                .iter()
                .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
                .collect();

            if local_type_registry.contains_key(&constructor.name)
                && !type_params.contains(&constructor.name)
            {
                Type::Qualified(sigil_ast::QualifiedType {
                    module_path: module_id.split("::").map(|s| s.to_string()).collect(),
                    type_name: constructor.name.clone(),
                    type_args: qualified_args,
                    location: constructor.location,
                })
            } else {
                Type::Constructor(sigil_ast::TypeConstructor {
                    name: constructor.name.clone(),
                    type_args: qualified_args,
                    location: constructor.location,
                })
            }
        }
    }
}

fn qualify_type_def(
    type_def: &TypeDef,
    module_id: &str,
    local_type_registry: &HashMap<String, TypeInfo>,
    type_params: &[String],
) -> TypeDef {
    match type_def {
        TypeDef::Product(product) => TypeDef::Product(sigil_ast::ProductType {
            fields: product
                .fields
                .iter()
                .map(|field| sigil_ast::Field {
                    name: field.name.clone(),
                    field_type: qualify_type_in_context(
                        &field.field_type,
                        module_id,
                        local_type_registry,
                        type_params,
                    ),
                    location: field.location,
                })
                .collect(),
            location: product.location,
        }),
        TypeDef::Alias(alias) => TypeDef::Alias(sigil_ast::TypeAlias {
            aliased_type: qualify_type_in_context(
                &alias.aliased_type,
                module_id,
                local_type_registry,
                type_params,
            ),
            location: alias.location,
        }),
        TypeDef::Sum(sum) => TypeDef::Sum(sigil_ast::SumType {
            variants: sum
                .variants
                .iter()
                .map(|variant| sigil_ast::Variant {
                    name: variant.name.clone(),
                    types: variant
                        .types
                        .iter()
                        .map(|ty| {
                            qualify_type_in_context(ty, module_id, local_type_registry, type_params)
                        })
                        .collect(),
                    location: variant.location,
                })
                .collect(),
            location: sum.location,
        }),
    }
}

/// Extract type registry from a module's AST
///
/// Collects all exported type definitions for use by dependent modules
fn extract_type_registry(
    ast: &Program,
    file_path: &std::path::Path,
    module_id: &str,
) -> HashMap<String, TypeInfo> {
    let mut registry = HashMap::new();

    // Only .lib.sigil files export types
    let is_lib_file = file_path.to_string_lossy().ends_with(".lib.sigil");

    let mut local_type_registry = HashMap::new();
    for decl in &ast.declarations {
        if let Declaration::Type(type_decl) = decl {
            local_type_registry.insert(
                type_decl.name.clone(),
                TypeInfo {
                    type_params: type_decl.type_params.clone(),
                    definition: type_decl.definition.clone(),
                    constraint: type_decl.constraint.clone(),
                },
            );
        }
    }

    for decl in &ast.declarations {
        if let Declaration::Type(type_decl) = decl {
            if is_lib_file {
                registry.insert(
                    type_decl.name.clone(),
                    TypeInfo {
                        type_params: type_decl.type_params.clone(),
                        definition: qualify_type_def(
                            &type_decl.definition,
                            module_id,
                            &local_type_registry,
                            &type_decl.type_params,
                        ),
                        constraint: type_decl.constraint.clone(),
                    },
                );
            }
        }
    }

    registry
}

fn coverage_variant_names_for_type_def(type_def: &TypeDef) -> Vec<String> {
    match type_def {
        TypeDef::Sum(sum) => sum
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn coverage_expected_variants(
    return_type: &InferenceType,
    local_type_registry: &HashMap<String, TypeInfo>,
    imported_type_regs: &HashMap<String, HashMap<String, TypeInfo>>,
) -> Vec<String> {
    let InferenceType::Constructor(constructor) = return_type else {
        return Vec::new();
    };

    match constructor.name.as_str() {
        "Option" => return vec!["None".to_string(), "Some".to_string()],
        "Result" => return vec!["Err".to_string(), "Ok".to_string()],
        _ => {}
    }

    if let Some(info) = local_type_registry.get(&constructor.name) {
        let variants = coverage_variant_names_for_type_def(&info.definition);
        if !variants.is_empty() {
            return variants;
        }
    }

    let mut imported_matches = imported_type_regs
        .values()
        .filter_map(|registry| registry.get(&constructor.name))
        .map(|info| coverage_variant_names_for_type_def(&info.definition))
        .filter(|variants| !variants.is_empty())
        .collect::<Vec<_>>();

    if imported_matches.len() == 1 {
        return imported_matches.pop().unwrap();
    }

    Vec::new()
}

fn collect_module_coverage_targets(
    module: &LoadedModule,
    typed_program: &TypedProgram,
    imported_type_regs: &HashMap<String, HashMap<String, TypeInfo>>,
    local_type_registry: &HashMap<String, TypeInfo>,
) -> Vec<CoverageTarget> {
    let Some(project) = &module.project else {
        return Vec::new();
    };

    let normalized_path = module.file_path.to_string_lossy().replace('\\', "/");
    let normalized_root = project.root.to_string_lossy().replace('\\', "/");
    if !normalized_path.starts_with(&normalized_root) || !normalized_path.contains("/src/") {
        return Vec::new();
    }
    if normalized_path.contains("/tests/") {
        return Vec::new();
    }

    let is_lib_file = normalized_path.ends_with(".lib.sigil");
    let is_exec_file = normalized_path.ends_with(".sigil") && !is_lib_file;
    if !is_lib_file && !is_exec_file {
        return Vec::new();
    }

    let mut targets = Vec::new();

    for decl in &typed_program.declarations {
        let TypedDeclaration::Function(function) = decl else {
            continue;
        };

        let expected_variants = coverage_expected_variants(
            &function.return_type,
            local_type_registry,
            imported_type_regs,
        );
        let id = format!("{}::{}", module.id, function.name);
        targets.push(CoverageTarget {
            id,
            expected_variants,
            file: normalized_path.clone(),
            function_name: function.name.clone(),
            location: TestLocation {
                line: function.location.start.line,
                column: function.location.start.column,
            },
        });
    }

    targets
}

/// Get output path for a compiled module
///
/// Converts module ID to file path, using repo root's .local directory:
/// - stdlib::list => <repo_root>/.local/language/stdlib/list.ts
/// - src::utils => <repo_root>/.local/path/to/src/utils.ts
fn get_module_output_path(module: &LoadedModule) -> PathBuf {
    use std::env;
    use std::fs;

    // Check if this is a project file
    if let Some(project) = module.project.clone() {
        // Use project's output directory
        let path_str = module.id.replace("::", "/");
        return project
            .root
            .join(&project.layout.out)
            .join(format!("{}.ts", path_str));
    }

    // For non-project files, use repo root's .local/
    // Find repo root by walking up from source file
    let abs_source =
        fs::canonicalize(&module.file_path).unwrap_or_else(|_| module.file_path.clone());
    let mut repo_root = abs_source.parent().unwrap().to_path_buf();

    // Walk up to find .git directory (repo root marker)
    while !repo_root.join(".git").exists() {
        if let Some(parent) = repo_root.parent() {
            repo_root = parent.to_path_buf();
        } else {
            // If we can't find .git, fall back to current directory
            repo_root = env::current_dir().unwrap();
            break;
        }
    }

    if module.id.contains("::") {
        return repo_root
            .join(".local")
            .join(format!("{}.ts", module.id.replace("::", "/")));
    }

    // Calculate relative path from repo root to source file
    let rel_source = abs_source.strip_prefix(&repo_root).unwrap_or(&abs_source);

    // Build output path: <repo_root>/.local/<rel_path>.ts
    let mut output = repo_root.join(".local");
    if let Some(parent) = rel_source.parent() {
        output = output.join(parent);
    }
    if let Some(file_name) = rel_source.file_name().and_then(|name| name.to_str()) {
        let stem = file_name
            .strip_suffix(".lib.sigil")
            .or_else(|| file_name.strip_suffix(".sigil"))
            .unwrap_or(file_name);
        output = output.join(format!("{}.ts", stem));
    }

    output
}
