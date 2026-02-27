//! Command implementations for CLI

use sigil_codegen::{CodegenOptions, TypeScriptGenerator};
use sigil_lexer::Lexer;
use sigil_parser::Parser;
use sigil_typechecker::{type_check, TypeError, TypeCheckOptions};
use sigil_validator::{validate_canonical_form, validate_surface_form, ValidationError};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use std::io::Write;
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
}

/// Lex command: tokenize a Sigil file
pub fn lex_command(file: &Path, human: bool) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{:?}", e)))?;

    if human {
        println!("sigilc lex OK phase=lexer");
        for token in &tokens {
            println!(
                "{}({}) at {}:{}",
                format!("{:?}", token.token_type),
                &token.value,
                token.location.start.line,
                token.location.start.column
            );
        }
    } else {
        // JSON output
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
                        }
                    })
                }).collect::<Vec<_>>()
            }
        });
        println!("{}", serde_json::to_string(&output).unwrap());
    }

    Ok(())
}

/// Parse command: parse a Sigil file to AST
pub fn parse_command(file: &Path, human: bool) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{:?}", e)))?;

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser.parse().map_err(|e| CliError::Parser(format!("{:?}", e)))?;

    // Validate surface form
    validate_surface_form(&ast).map_err(|e: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", e.len()))
    })?;

    if human {
        println!("sigilc parse OK phase=parser");
        println!("{:#?}", ast);
    } else {
        // JSON output
        let output = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc parse",
            "ok": true,
            "phase": "parser",
            "data": {
                "file": filename,
                "summary": {
                    "declarations": ast.declarations.len()
                },
                "ast": format!("{:#?}", ast) // Simplified for now
            }
        });
        println!("{}", serde_json::to_string(&output).unwrap());
    }

    Ok(())
}

/// Compile command: compile a Sigil file to TypeScript
pub fn compile_command(
    file: &Path,
    output: Option<&Path>,
    show_types: bool,
    human: bool,
) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{:?}", e)))?;

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser.parse().map_err(|e| CliError::Parser(format!("{:?}", e)))?;

    // Validate surface form
    validate_surface_form(&ast).map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", errors.len()))
    })?;

    // Validate canonical form
    validate_canonical_form(&ast).map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", errors.len()))
    })?;

    // Type check
    let _inferred_types = type_check(&ast, &source, None)
        .map_err(|error: TypeError| CliError::Type(format!("Type error: {:?}", error)))?;

    // Generate TypeScript
    let codegen_options = CodegenOptions {
        source_file: Some(filename.clone()),
        output_file: output.map(|p| p.to_string_lossy().to_string()),
        project_root: None,
    };
    let mut codegen = TypeScriptGenerator::new(codegen_options);
    let ts_code = codegen
        .generate(&ast)
        .map_err(|e| CliError::Codegen(format!("{:?}", e)))?;

    // Determine output file
    let output_file_owned: std::path::PathBuf;
    let output_file = if let Some(out) = output {
        out
    } else {
        let input_str = file.to_string_lossy();
        output_file_owned = if input_str.ends_with(".sigil") {
            std::path::PathBuf::from(format!(
                ".local/{}",
                input_str.replace(".sigil", ".ts")
            ))
        } else {
            std::path::PathBuf::from(format!("{}.ts", input_str))
        };
        &output_file_owned
    };

    // Create output directory if needed
    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write output file
    fs::write(output_file, ts_code)?;

    if human {
        println!("sigilc compile OK phase=codegen");
        println!("Output: {}", output_file.display());
    } else {
        // JSON output
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc compile",
            "ok": true,
            "phase": "codegen",
            "data": {
                "input": filename,
                "outputs": {
                    "rootTs": output_file.to_string_lossy()
                },
                "typecheck": {
                    "ok": true,
                    "inferred": if show_types { vec![] as Vec<serde_json::Value> } else { vec![] }
                }
            }
        });
        println!("{}", serde_json::to_string(&output_json).unwrap());
    }

    Ok(())
}

/// Run command: compile and execute a Sigil file
pub fn run_command(file: &Path, human: bool) -> Result<(), CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{:?}", e)))?;

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser.parse().map_err(|e| CliError::Parser(format!("{:?}", e)))?;

    // Validate surface form
    validate_surface_form(&ast).map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", errors.len()))
    })?;

    // Validate canonical form
    validate_canonical_form(&ast).map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", errors.len()))
    })?;

    // Type check
    let _inferred_types = type_check(&ast, &source, None)
        .map_err(|error: TypeError| CliError::Type(format!("Type error: {:?}", error)))?;

    // Generate TypeScript
    let input_str = file.to_string_lossy();
    let output_file_path = if input_str.ends_with(".sigil") {
        PathBuf::from(format!(".local/{}", input_str.replace(".sigil", ".ts")))
    } else {
        PathBuf::from(format!("{}.ts", input_str))
    };

    let codegen_options = CodegenOptions {
        source_file: Some(filename.clone()),
        output_file: Some(output_file_path.to_string_lossy().to_string()),
        project_root: None,
    };
    let mut codegen = TypeScriptGenerator::new(codegen_options);
    let ts_code = codegen
        .generate(&ast)
        .map_err(|e| CliError::Codegen(format!("{:?}", e)))?;

    // Create output directory if needed
    if let Some(parent) = output_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write output file
    fs::write(&output_file_path, ts_code)?;

    // Create runner file
    let runner_path = output_file_path.with_extension("run.ts");
    let module_name = output_file_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let runner_code = format!(
        r#"import {{ main }} from './{module_name}';

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
"#
    );

    fs::write(&runner_path, runner_code)?;

    // Execute the runner (use absolute path to avoid path resolution issues)
    let abs_runner_path = std::fs::canonicalize(&runner_path)?;
    let start_time = Instant::now();
    let output = Command::new("pnpm")
        .args(&["exec", "node", "--import", "tsx"])
        .arg(&abs_runner_path)
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
        if human {
            eprintln!("{}", stderr);
            eprintln!("sigilc run FAIL (exit code: {})", exit_code);
        } else {
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
        }
        return Err(CliError::Runtime(format!("Process exited with code {}", exit_code)));
    }

    if human {
        print!("{}", stdout);
        eprint!("{}", stderr);
        println!("sigilc run OK phase=runtime");
    } else {
        let output_json = serde_json::json!({
            "formatVersion": 1,
            "command": "sigilc run",
            "ok": true,
            "phase": "runtime",
            "data": {
                "compile": {
                    "input": filename,
                    "output": output_file_path.to_string_lossy(),
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
    }

    Ok(())
}

/// Test command: run Sigil tests from a directory
pub fn test_command(path: &Path, match_filter: Option<&str>, human: bool) -> Result<(), CliError> {
    // Check if tests directory exists
    if !path.exists() {
        if human {
            println!("No tests found ({} does not exist).", path.display());
        } else {
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
        }
        return Ok(());
    }

    let start_time = Instant::now();

    // Collect all .sigil files in test directory
    let test_files = collect_sigil_files(path)?;

    let mut all_results = Vec::new();
    let mut discovered = 0;
    let mut selected = 0;

    // Compile and run each test file
    for test_file in &test_files {
        match compile_and_run_tests(test_file, match_filter) {
            Ok(result) => {
                discovered += result.discovered;
                selected += result.selected;
                all_results.extend(result.results);
            }
            Err(e) => {
                eprintln!("Error running tests in {}: {}", test_file.display(), e);
            }
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

    if human {
        if ok {
            println!("PASS {}/{} tests passed", passed, selected);
        } else {
            println!("FAIL {}/{} tests passed", passed, selected);
            for result in &all_results {
                if result.status != "pass" {
                    println!(
                        "{}: {} ({})",
                        result.status.to_uppercase(),
                        result.name,
                        result.file
                    );
                    if let Some(ref failure) = result.failure {
                        println!("  {}", failure);
                    }
                }
            }
        }
    } else {
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
    }

    if !ok {
        return Err(CliError::Runtime("Tests failed".to_string()));
    }

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
    match_filter: Option<&str>,
) -> Result<TestRunResult, CliError> {
    let source = fs::read_to_string(file)?;
    let filename = file.to_string_lossy().to_string();

    // Tokenize
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize().map_err(|e| CliError::Lexer(format!("{:?}", e)))?;

    // Parse
    let mut parser = Parser::new(tokens, &filename);
    let ast = parser.parse().map_err(|e| CliError::Parser(format!("{:?}", e)))?;

    // Validate
    validate_surface_form(&ast).map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", errors.len()))
    })?;

    validate_canonical_form(&ast).map_err(|errors: Vec<ValidationError>| {
        CliError::Validation(format!("{} validation errors", errors.len()))
    })?;

    // Type check
    let _inferred_types = type_check(&ast, &source, None)
        .map_err(|error: TypeError| CliError::Type(format!("Type error: {:?}", error)))?;

    // Generate TypeScript
    let input_str = file.to_string_lossy();
    let output_file_path = if input_str.ends_with(".sigil") {
        PathBuf::from(format!(".local/{}", input_str.replace(".sigil", ".ts")))
    } else {
        PathBuf::from(format!("{}.ts", input_str))
    };

    let codegen_options = CodegenOptions {
        source_file: Some(filename.clone()),
        output_file: Some(output_file_path.to_string_lossy().to_string()),
        project_root: None,
    };
    let mut codegen = TypeScriptGenerator::new(codegen_options);
    let ts_code = codegen
        .generate(&ast)
        .map_err(|e| CliError::Codegen(format!("{:?}", e)))?;

    // Create output directory
    if let Some(parent) = output_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write TypeScript file
    fs::write(&output_file_path, ts_code)?;

    // Run test runner
    run_test_module(&output_file_path, match_filter, &filename)
}

fn run_test_module(
    ts_file: &Path,
    match_filter: Option<&str>,
    source_file: &str,
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
        r#"const moduleUrl = "{}";
const discoverMod = await import(moduleUrl);
const tests = Array.isArray(discoverMod.__sigil_tests) ? discoverMod.__sigil_tests : [];
const matchText = {};
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
        results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, failure: value.failure ?? {{ kind: 'assert_false', message: 'Test body evaluated to ⊥' }} }});
      }}
    }} else {{
      results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, failure: {{ kind: 'assert_false', message: 'Test body evaluated to ⊥' }} }});
    }}
  }} catch (e) {{
    results.push({{ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'error', durationMs: Date.now()-start, location: t.location, failure: {{ kind: 'exception', message: e instanceof Error ? e.message : String(e) }} }});
  }}
}}
console.log(JSON.stringify({{ results, discovered: tests.length, selected: selected.length, durationMs: Date.now()-startSuite }}));
"#,
        module_url, match_text_json
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
