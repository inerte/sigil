//! Command implementations for CLI

use crate::module_graph::{load_project_effect_catalog_for, ModuleGraph, ModuleGraphError, LoadedModule};
use crate::project::{find_project_root, get_project_config};
use rayon::prelude::*;
use sigil_ast::{Declaration, Program, Type, TypeDef};
use sigil_diagnostics::codes;
use sigil_codegen::{CodegenOptions, TypeScriptGenerator};
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::{type_check, TypeError, TypeCheckOptions, TypeInfo, TypeScheme};
use sigil_typechecker::types::{InferenceType, TConstructor, TFunction, TList, TMap, TRecord, TTuple};
use sigil_validator::{
    validate_canonical_form_with_options,
    validate_typed_canonical_form,
    ValidationError,
    ValidationOptions,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use serde_json::json;
use thiserror::Error;

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
    Type(String),

    #[error("Codegen error: {0}")]
    Codegen(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Module graph error: {0}")]
    ModuleGraph(#[from] ModuleGraphError),
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
fn output_json_error(command: &str, phase: &str, error_code: &str, message: &str, details: serde_json::Value) {
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
    println!("{}", serde_json::to_string(&output).unwrap());
}

/// Lex command: tokenize a Sigil file
pub fn lex_command(file: &Path) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{}", e)))?;

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
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{}", e)))?;
    let token_count = tokens.len(); // Store token count for JSON output

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser.parse().map_err(|e| CliError::Parser(format!("{}", e)))?;

    let effect_catalog = load_project_effect_catalog_for(file)?;

    // Validate canonical form (includes formatting)
    validate_canonical_form_with_options(
        &ast,
        Some(&filename),
        Some(&source),
        ValidationOptions { effect_catalog },
    )
        .map_err(|errors: Vec<ValidationError>| CliError::Validation(format_validation_errors(&errors)))?;

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

/// Compile command: compile a Sigil file to TypeScript
pub fn compile_command(
    file: &Path,
    output: Option<&Path>,
    show_types: bool,
) -> Result<(), CliError> {
    // Build module graph
    let graph = match ModuleGraph::build(file) {
        Ok(g) => g,
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
                        "errors": errors.iter().map(|e| e.to_string()).collect::<Vec<_>>()
                    })
                );
            }
            return Err(ModuleGraphError::Validation(errors).into());
        }
        Err(e) => return Err(e.into()),
    };

    let mut compiled_modules = HashMap::new();
    let mut compiled_schemes = HashMap::new();
    let mut type_registries = HashMap::new();
    let mut output_files = Vec::new();
    let mut module_outputs: HashMap<String, PathBuf> = HashMap::new(); // Track module ID -> output path

    // Compile modules in topological order
    for module_id in &graph.topo_order {
        let module = &graph.modules[module_id];

        // Build imported namespaces from already-compiled dependencies
        let imported_namespaces = build_imported_namespaces(&module.ast, &compiled_modules);
        let imported_type_regs = build_imported_type_registries(&module.ast, &type_registries);
        let imported_value_schemes = build_imported_value_schemes(&module.ast, &compiled_schemes);
        let effect_catalog = load_project_effect_catalog_for(&module.file_path)?;

        // Type check with cross-module context
        let typecheck_result = type_check(
            &module.ast,
            &module.source,
            Some(TypeCheckOptions {
                effect_catalog,
                imported_namespaces: Some(imported_namespaces),
                imported_type_registries: Some(imported_type_regs),
                imported_value_schemes: Some(imported_value_schemes),
                source_file: Some(module.file_path.to_string_lossy().to_string()),
            }),
        )
        .map_err(|error: TypeError| CliError::Type(format!("{}", error)))?;

        validate_typed_canonical_form(&typecheck_result.typed_program)
            .map_err(|errors| CliError::Validation(format_validation_errors(&errors)))?;

        // Determine output path
        let output_path = if module_id == graph.topo_order.last().unwrap() && output.is_some() {
            // Entry module with explicit output path
            output.unwrap().to_path_buf()
        } else {
            // Use standard output path based on module ID
            get_module_output_path(module)
        };

        // Generate TypeScript
        let codegen_options = CodegenOptions {
            source_file: Some(module.file_path.to_string_lossy().to_string()),
            output_file: Some(output_path.to_string_lossy().to_string()),
        };
        let mut codegen = TypeScriptGenerator::new(codegen_options);
        let ts_code = codegen
            .generate(&typecheck_result.typed_program)
            .map_err(|e| CliError::Codegen(format!("{}", e)))?;

        // Create output directory
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write output file
        fs::write(&output_path, ts_code)?;
        output_files.push(output_path.clone());
        module_outputs.insert(module_id.clone(), output_path);

        // Track for dependents
        compiled_schemes.insert(module_id.clone(), typecheck_result.declaration_schemes.clone());
        compiled_modules.insert(module_id.clone(), typecheck_result.declaration_types);
        type_registries.insert(
            module_id.clone(),
            extract_type_registry(&module.ast, &module.file_path, module_id),
        );
    }

    // Find entry module output
    let entry_output = output_files.last().unwrap();
    let entry_module = graph.modules.get(graph.topo_order.last().unwrap()).unwrap();

    let all_modules: Vec<serde_json::Value> = graph.topo_order
        .iter()
        .map(|module_id| {
            let module = &graph.modules[module_id];
            let output_file = module_outputs.get(module_id)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            serde_json::json!({
                "moduleId": module_id,
                "sourceFile": module.file_path.to_string_lossy(),
                "outputFile": output_file
            })
        })
        .collect();

    let project_json = entry_module.project.as_ref().map(|proj| {
        serde_json::json!({
            "root": proj.root.to_string_lossy(),
            "layout": serde_json::to_value(&proj.layout).unwrap_or(serde_json::json!({}))
        })
    });

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

/// Run command: compile and execute a Sigil file
pub fn run_command(file: &Path, selected_env: Option<&str>, args: &[String]) -> Result<(), CliError> {
    let graph = ModuleGraph::build(file)?;
    let compiled = compile_module_graph(graph, None)?;
    let entry_output_path = compiled.entry_output_path;
    let topology_prelude = runner_prelude(file, selected_env)?.unwrap_or_default();

    // Create runner file
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

    // Execute the runner (use absolute path to avoid path resolution issues)
    let abs_runner_path = std::fs::canonicalize(&runner_path)?;
    let start_time = Instant::now();
    let output = Command::new("pnpm")
        .args(&["exec", "node", "--import", "tsx"])
        .arg(&abs_runner_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CliError::Runtime("pnpm not found. Please install pnpm to run Sigil programs.".to_string())
            } else {
                CliError::Runtime(format!("Failed to execute: {}", e))
            }
        })?;

    let duration_ms = start_time.elapsed().as_millis();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    if exit_code != 0 {
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc run",
            "ok": false,
            "phase": "runtime",
            "error": {
                "code": "SIGIL-RUNTIME-CHILD-EXIT",
                "phase": "runtime",
                "message": format!("child process exited with nonzero status: {}", exit_code),
                "details": {
                    "exitCode": exit_code,
                    "stdout": stdout.to_string(),
                    "stderr": stderr.to_string()
                }
            }
        });
        println!("{}", serde_json::to_string(&output_json).unwrap());
        return Err(CliError::Runtime(format!("Process exited with code {}", exit_code)));
    }

    let output_json = serde_json::json!({
        "formatVersion": 1,
        "command": "sigilc run",
        "ok": true,
        "phase": "runtime",
        "data": {
            "compile": {
                "input": file.to_string_lossy(),
                "output": entry_output_path.to_string_lossy(),
                "runnerFile": runner_path.to_string_lossy()
            },
            "runtime": {
                "engine": "node+tsx",
                "exitCode": exit_code,
                "durationMs": duration_ms,
                "stdout": stdout.to_string(),
                "stderr": stderr.to_string()
            }
        }
    });
    println!("{}", serde_json::to_string(&output_json).unwrap());

    Ok(())
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

    // Collect all .sigil files in test directory
    let test_files = collect_sigil_files(path)?;

    // Run test files in parallel
    let results: Vec<_> = test_files
        .par_iter()
        .map(|test_file| {
            compile_and_run_tests(test_file, selected_env, match_filter)
                .map_err(|e| {
                    eprintln!("Error running tests in {}: {}", test_file.display(), e);
                    e
                })
        })
        .collect();

    // Aggregate results from all files
    let mut all_results = Vec::new();
    let mut discovered = 0;
    let mut selected = 0;

    for result in results {
        if let Ok(test_result) = result {
            discovered += test_result.discovered;
            selected += test_result.selected;
            all_results.extend(test_result.results);
        }
    }

    // Sort results by file, then line, then column
    all_results.sort_by(|a, b| {
        a.file.cmp(&b.file)
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
    let project_root = find_project_root(path).ok_or_else(|| {
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
    let prelude = build_topology_runtime_prelude(&project_root, env)?;
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
}

struct CompiledGraphOutputs {
    entry_output_path: PathBuf,
}

fn compile_module_graph(
    graph: ModuleGraph,
    output_override: Option<&Path>,
) -> Result<CompiledGraphOutputs, CliError> {
    let mut compiled_modules = HashMap::new();
    let mut compiled_schemes = HashMap::new();
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
                imported_type_registries: Some(imported_type_regs),
                imported_value_schemes: Some(imported_value_schemes),
                source_file: Some(module.file_path.to_string_lossy().to_string()),
            }),
        )
        .map_err(|error: TypeError| CliError::Type(format!("{}", error)))?;

        validate_typed_canonical_form(&typecheck_result.typed_program)
            .map_err(|errors| CliError::Validation(format_validation_errors(&errors)))?;

        let output_path = if module_id == graph.topo_order.last().unwrap() && output_override.is_some() {
            output_override.unwrap().to_path_buf()
        } else {
            get_module_output_path(module)
        };

        let codegen_options = CodegenOptions {
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

        compiled_schemes.insert(module_id.clone(), typecheck_result.declaration_schemes.clone());
        compiled_modules.insert(module_id.clone(), typecheck_result.declaration_types);
        type_registries.insert(
            module_id.clone(),
            extract_type_registry(&module.ast, &module.file_path, module_id),
        );
    }

    Ok(CompiledGraphOutputs { entry_output_path })
}

fn topology_source_path(project_root: &Path) -> PathBuf {
    project_root.join("src/topology.lib.sigil")
}

fn config_source_path(project_root: &Path, env_name: &str) -> PathBuf {
    project_root.join("config").join(format!("{}.lib.sigil", env_name))
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

fn compile_config_module(project_root: &Path, env_name: &str) -> Result<CompiledGraphOutputs, CliError> {
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

fn build_topology_runtime_prelude(project_root: &Path, env_name: &str) -> Result<String, CliError> {
    let topology_outputs = compile_topology_module(project_root)?;
    let topology_output = topology_outputs.entry_output_path;
    let config_outputs = compile_config_module(project_root, env_name)?;
    let config_output = config_outputs.entry_output_path;
    let topology_url = format!("file://{}", fs::canonicalize(topology_output)?.display());
    let config_url = format!("file://{}", fs::canonicalize(config_output)?.display());
    let env_name_json = serde_json::to_string(env_name).unwrap();

    Ok(format!(
        r#"const __sigil_topology_module = await import("{topology_url}");
const __sigil_topology_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_topology_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);
const __sigil_config_module = await import("{config_url}");
const __sigil_config_exports = Object.fromEntries(
  await Promise.all(
    Object.entries(__sigil_config_module).map(async ([key, value]) => [key, await Promise.resolve(value)])
  )
);
const __sigil_topology_env_name = {env_name_json};

function __sigil_topology_fail(code, message) {{
  const error = new Error(`${{code}}: ${{message}}`);
  error.sigilCode = code;
  throw error;
}}

function __sigil_topology_dependency_name(dep, expectedTag) {{
  if (!dep || typeof dep !== 'object' || dep.__tag !== expectedTag) {{
    __sigil_topology_fail("{invalid_handle}", `expected ${{expectedTag}} handle`);
  }}
  const name = Array.isArray(dep.__fields) ? dep.__fields[0] : null;
  if (typeof name !== 'string' || name.length === 0) {{
    __sigil_topology_fail("{invalid_handle}", `invalid ${{expectedTag}} name`);
  }}
  return name;
}}

function __sigil_topology_environment_name(env) {{
  if (!env || typeof env !== 'object' || env.__tag !== 'Environment') {{
    __sigil_topology_fail("{invalid_handle}", 'expected Environment declaration');
  }}
  const name = Array.isArray(env.__fields) ? env.__fields[0] : null;
  if (typeof name !== 'string' || name.length === 0) {{
    __sigil_topology_fail("{invalid_handle}", 'invalid environment name');
  }}
  return name;
}}

function __sigil_topology_resolve_binding_value(bindingValue) {{
  if (!bindingValue || typeof bindingValue !== 'object') {{
    __sigil_topology_fail("{binding_kind}", 'invalid binding value');
  }}
  if (bindingValue.__tag === 'Literal') {{
    return String(bindingValue.__fields?.[0] ?? '');
  }}
  if (bindingValue.__tag === 'EnvVar') {{
    const envName = String(bindingValue.__fields?.[0] ?? '');
    const value = process.env[envName];
    if (typeof value !== 'string' || value.length === 0) {{
      __sigil_topology_fail("{missing_binding}", `environment variable ${{envName}} is required`);
    }}
    return value;
  }}
  __sigil_topology_fail("{binding_kind}", 'invalid string binding value');
}}

function __sigil_topology_resolve_port(bindingValue) {{
  if (!bindingValue || typeof bindingValue !== 'object') {{
    __sigil_topology_fail("{binding_kind}", 'invalid port binding value');
  }}
  if (bindingValue.__tag === 'LiteralPort') {{
    return Number(bindingValue.__fields?.[0] ?? 0);
  }}
  if (bindingValue.__tag === 'EnvVarPort') {{
    const envName = String(bindingValue.__fields?.[0] ?? '');
    const raw = process.env[envName];
    const port = Number(raw);
    if (!Number.isInteger(port) || port <= 0 || port > 65535) {{
      __sigil_topology_fail("{missing_binding}", `environment variable ${{envName}} must resolve to a valid TCP port`);
    }}
    return port;
  }}
  __sigil_topology_fail("{binding_kind}", 'invalid TCP port binding value');
}}

function __sigil_topology_collect_dependencies(moduleExports) {{
  const http = new Map();
  const tcp = new Map();
  for (const value of Object.values(moduleExports)) {{
    if (value?.__tag === 'HttpServiceDependency') {{
      const name = __sigil_topology_dependency_name(value, 'HttpServiceDependency');
      if (http.has(name) || tcp.has(name)) {{
        __sigil_topology_fail("{duplicate_dependency}", `duplicate dependency name '${{name}}'`);
      }}
      http.set(name, value);
    }}
    if (value?.__tag === 'TcpServiceDependency') {{
      const name = __sigil_topology_dependency_name(value, 'TcpServiceDependency');
      if (tcp.has(name) || http.has(name)) {{
        __sigil_topology_fail("{duplicate_dependency}", `duplicate dependency name '${{name}}'`);
      }}
      tcp.set(name, value);
    }}
  }}
  return {{ http, tcp }};
}}

function __sigil_topology_collect_environments(moduleExports) {{
  const envs = new Set();
  for (const value of Object.values(moduleExports)) {{
    if (value?.__tag === 'Environment') {{
      const name = __sigil_topology_environment_name(value);
      if (envs.has(name)) {{
        __sigil_topology_fail("{duplicate_dependency}", `duplicate environment name '${{name}}'`);
      }}
      envs.add(name);
    }}
  }}
  return envs;
}}

function __sigil_topology_read_config_bindings(configExports) {{
  const bindings = configExports.bindings;
  if (!bindings || typeof bindings !== 'object') {{
    __sigil_topology_fail("{invalid_config}", "config module must export a 'bindings' value");
  }}
  return bindings;
}}

function __sigil_topology_build_bindings(topologyExports, configExports, envName) {{
  const environments = __sigil_topology_collect_environments(topologyExports);
  if (!environments.has(envName)) {{
    __sigil_topology_fail("{env_not_found}", `environment '${{envName}}' not declared in src/topology.lib.sigil`);
  }}

  const dependencies = __sigil_topology_collect_dependencies(topologyExports);
  const configBindings = __sigil_topology_read_config_bindings(configExports);
  const resolved = {{ http: Object.create(null), tcp: Object.create(null) }};
  const seen = new Set();
  for (const binding of configBindings.httpBindings ?? []) {{
    const name = typeof binding?.dependencyName === 'string' ? binding.dependencyName : null;
    if (!name) {{
      __sigil_topology_fail("{binding_kind}", 'HTTP bindings must name a declared dependency');
    }}
    if (!dependencies.http.has(name)) {{
      __sigil_topology_fail("{invalid_handle}", `HTTP binding references undeclared dependency '${{name}}'`);
    }}
    if (seen.has(`http:${{name}}`) || seen.has(`tcp:${{name}}`)) {{
      __sigil_topology_fail("{duplicate_binding}", `duplicate binding for '${{name}}' in environment '${{envName}}'`);
    }}
    seen.add(`http:${{name}}`);
    resolved.http[name] = __sigil_topology_resolve_binding_value(binding.baseUrl);
  }}

  for (const binding of configBindings.tcpBindings ?? []) {{
    const name = typeof binding?.dependencyName === 'string' ? binding.dependencyName : null;
    if (!name) {{
      __sigil_topology_fail("{binding_kind}", 'TCP bindings must name a declared dependency');
    }}
    if (!dependencies.tcp.has(name)) {{
      __sigil_topology_fail("{invalid_handle}", `TCP binding references undeclared dependency '${{name}}'`);
    }}
    if (seen.has(`tcp:${{name}}`) || seen.has(`http:${{name}}`)) {{
      __sigil_topology_fail("{duplicate_binding}", `duplicate binding for '${{name}}' in environment '${{envName}}'`);
    }}
    seen.add(`tcp:${{name}}`);
    resolved.tcp[name] = {{
      host: __sigil_topology_resolve_binding_value(binding.host),
      port: __sigil_topology_resolve_port(binding.port)
    }};
  }}

  for (const name of dependencies.http.keys()) {{
    if (!(name in resolved.http)) {{
      __sigil_topology_fail("{missing_binding}", `missing HTTP binding for '${{name}}' in environment '${{envName}}'`);
    }}
  }}

  for (const name of dependencies.tcp.keys()) {{
    if (!(name in resolved.tcp)) {{
      __sigil_topology_fail("{missing_binding}", `missing TCP binding for '${{name}}' in environment '${{envName}}'`);
    }}
  }}

  return resolved;
}}

globalThis.__sigil_topology_env_name = __sigil_topology_env_name;
globalThis.__sigil_topology_bindings = __sigil_topology_build_bindings(__sigil_topology_exports, __sigil_config_exports, __sigil_topology_env_name);
"#,
        topology_url = topology_url,
        config_url = config_url,
        env_name_json = env_name_json,
        invalid_handle = codes::topology::INVALID_HANDLE,
        binding_kind = codes::topology::BINDING_KIND_MISMATCH,
        missing_binding = codes::topology::MISSING_BINDING,
        duplicate_dependency = codes::topology::DUPLICATE_DEPENDENCY,
        duplicate_binding = codes::topology::DUPLICATE_BINDING,
        env_not_found = codes::topology::ENV_NOT_FOUND,
        invalid_config = codes::topology::INVALID_CONFIG_MODULE,
    ))
}

fn project_root_and_topology(path: &Path) -> Option<(PathBuf, bool)> {
    let project = get_project_config(path)?;
    let topology_present = topology_source_path(&project.root).exists();
    Some((project.root, topology_present))
}

fn runner_prelude(
    path: &Path,
    selected_env: Option<&str>,
) -> Result<Option<String>, CliError> {
    let Some((project_root, topology_present)) = project_root_and_topology(path) else {
        return Ok(None);
    };

    if !topology_present {
        return Ok(None);
    }

    let env_name = selected_env.ok_or_else(|| {
        CliError::Validation(format!(
            "{}: topology-aware run/test/validate commands require --env <name>",
            codes::topology::ENV_REQUIRED
        ))
    })?;

    build_topology_runtime_prelude(&project_root, env_name).map(Some)
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
    let compiled = compile_module_graph(graph, None)?;
    let topology_prelude = runner_prelude(file, selected_env)?;
    run_test_module(
        &compiled.entry_output_path,
        match_filter,
        &file.to_string_lossy(),
        topology_prelude.as_deref(),
    )
}

fn run_test_module(
    ts_file: &Path,
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
    const freshMod = await import(moduleUrl + '?sigil_test=' + encodeURIComponent(String(t.id)) + '&ts=' + Date.now() + '_' + Math.random());
    const freshTests = Array.isArray(freshMod.__sigil_tests) ? freshMod.__sigil_tests : [];
    const freshTest = freshTests.find((x) => x.id === t.id);
    if (!freshTest) {{ throw new Error('Test not found in isolated module reload: ' + String(t.id)); }}
    const value = await freshTest.fn();
    if (value === true) {{
      results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'pass', durationMs: Date.now()-start, location: t.location }});
    }} else if (value && typeof value === 'object' && 'ok' in value) {{
      if (value.ok === true) {{
        results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'pass', durationMs: Date.now()-start, location: t.location }});
      }} else {{
        results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, failure: value.failure ?? {{ kind: 'assert_false', message: 'Test body evaluated to false' }} }});
      }}
    }} else {{
      results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, failure: {{ kind: 'assert_false', message: 'Test body evaluated to false' }} }});
    }}
  }} catch (e) {{
    results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'error', durationMs: Date.now()-start, location: t.location, failure: {{ kind: 'exception', message: e instanceof Error ? e.message : String(e) }} }});
  }}
}}
console.log(JSON.stringify({{ results, discovered: tests.length, selected: selected.length, durationMs: Date.now()-startSuite }}));
"#,
        topology_prelude = topology_prelude.unwrap_or(""),
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
        return Err(CliError::Runtime(format!(
            "Test runner failed: {}",
            stderr
        )));
    }

    // Parse test results
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| CliError::Runtime(format!("Failed to parse test output: {}", e)))?;

    let discovered = json["discovered"].as_u64().unwrap_or(0) as usize;
    let selected = json["selected"].as_u64().unwrap_or(0) as usize;

    let mut results = Vec::new();
    if let Some(test_results) = json["results"].as_array() {
        for result in test_results {
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
    })
}

// ============================================================================
// Multi-module Compilation Helpers
// ============================================================================

/// Build imported namespaces from already-compiled modules
///
/// For each import, creates a namespace type (record) containing exported functions/constants
fn build_imported_namespaces(
    ast: &Program,
    compiled_modules: &HashMap<String, HashMap<String, InferenceType>>,
) -> HashMap<String, InferenceType> {
    let mut imported = HashMap::new();

    for decl in &ast.declarations {
        if let Declaration::Import(import_decl) = decl {
            let module_id = import_decl.module_path.join("::");

            if let Some(types) = compiled_modules.get(&module_id) {
                // Build namespace type from exported functions/consts
                let mut fields = HashMap::new();
                for (name, typ) in types {
                    fields.insert(name.clone(), qualify_inference_type_in_context(typ, &module_id));
                }

                imported.insert(
                    module_id.clone(),
                    InferenceType::Record(TRecord {
                        fields,
                        name: Some(module_id.clone()),
                    }),
                );
            }
        }
    }

    imported
}

fn is_core_prelude_name(name: &str) -> bool {
    matches!(
        name,
        "ConcurrentOutcome" | "Option" | "Result" | "Aborted" | "Failure" | "Success" | "Some" | "None" | "Ok" | "Err"
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
    ast: &Program,
    type_registries: &HashMap<String, HashMap<String, TypeInfo>>,
) -> HashMap<String, HashMap<String, TypeInfo>> {
    let mut imported = HashMap::new();

    if let Some(registry) = type_registries.get("core::prelude") {
        imported.insert("core::prelude".to_string(), registry.clone());
    }

    for decl in &ast.declarations {
        if let Declaration::Import(import_decl) = decl {
            let module_id = import_decl.module_path.join("::");

            if let Some(registry) = type_registries.get(&module_id) {
                imported.insert(module_id.clone(), registry.clone());
            }
        }
    }

    imported
}

fn build_imported_value_schemes(
    ast: &Program,
    compiled_schemes: &HashMap<String, HashMap<String, TypeScheme>>,
) -> HashMap<String, HashMap<String, TypeScheme>> {
    let mut imported = HashMap::new();

    if let Some(schemes) = compiled_schemes.get("core::prelude") {
        imported.insert(
            "core::prelude".to_string(),
            schemes
                .iter()
                .map(|(name, scheme)| (name.clone(), scheme.clone()))
                .collect(),
        );
    }

    for decl in &ast.declarations {
        if let Declaration::Import(import_decl) = decl {
            let module_id = import_decl.module_path.join("::");

            if let Some(schemes) = compiled_schemes.get(&module_id) {
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
        }
    }

    imported
}

fn qualify_inference_type_for_module(
    module_id: &str,
    typ: &sigil_typechecker::InferenceType,
) -> sigil_typechecker::InferenceType {
    use sigil_typechecker::InferenceType;
    use sigil_typechecker::types::{TConstructor, TFunction, TList, TRecord, TTuple, TVar};

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

            if local_type_registry.contains_key(&constructor.name) && !type_params.contains(&constructor.name) {
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
                        .map(|ty| qualify_type_in_context(ty, module_id, local_type_registry, type_params))
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
                    },
                );
            }
        }
    }

    registry
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
    if let Some(project) = module.project.clone().or_else(|| crate::project::get_project_config(&module.file_path)) {
        // Use project's output directory
        let path_str = module.id.replace("::", "/");
        return project.root.join(&project.layout.out).join(format!("{}.ts", path_str));
    }

    // For non-project files, use repo root's .local/
    // Find repo root by walking up from source file
    let abs_source = fs::canonicalize(&module.file_path).unwrap_or_else(|_| module.file_path.clone());
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

    // Calculate relative path from repo root to source file
    let rel_source = abs_source.strip_prefix(&repo_root)
        .unwrap_or(&abs_source);

    // Build output path: <repo_root>/.local/<rel_path>.ts
    let mut output = repo_root.join(".local");
    if let Some(parent) = rel_source.parent() {
        output = output.join(parent);
    }
    if let Some(stem) = rel_source.file_stem() {
        output = output.join(format!("{}.ts", stem.to_string_lossy()));
    }

    output
}
