//! Sigil to TypeScript Code Generator
//!
//! Compiles Sigil AST to runnable TypeScript (ES2022-compatible output).
//!
//! Key transformations:
//! - All functions return promise-shaped values
//! - Ordinary function calls compose without eager `await`
//! - Pattern matching compiles to if/else chains with `__match` variables
//! - Sum type constructors compile to objects with __tag and __fields
//! - Mock runtime helpers emitted at top of file

use sigil_ast::{
    BinaryOperator, ExternDecl, ImportDecl, LiteralExpr, LiteralValue, Pattern,
    PatternLiteralValue, PipelineOperator, TypeDecl, TypeDef, UnaryOperator,
};
use sigil_typechecker::typed_ir::{
    MethodSelector, TypedBinaryExpr, TypedCallExpr, TypedConcurrentExpr, TypedConcurrentStep,
    TypedConstDecl, TypedConstructorCallExpr, TypedDeclaration, TypedExpr, TypedExprKind,
    TypedExternCallExpr, TypedFieldAccessExpr, TypedFilterExpr, TypedFoldExpr, TypedFunctionDecl,
    TypedIfExpr, TypedIndexExpr, TypedLambdaExpr, TypedLetExpr, TypedListExpr, TypedMapExpr,
    TypedMapLiteralExpr, TypedMatchExpr, TypedMethodCallExpr, TypedPipelineExpr, TypedProgram,
    TypedRecordExpr, TypedTestDecl, TypedTupleExpr, TypedUnaryExpr, TypedWithMockExpr,
    WithMockTarget,
};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("Codegen error: {0}")]
    General(String),
}

pub struct CodegenOptions {
    pub source_file: Option<String>,
    pub output_file: Option<String>,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            source_file: None,
            output_file: None,
        }
    }
}

pub struct TypeScriptGenerator {
    indent: usize,
    output: Vec<String>,
    source_file: Option<String>,
    output_file: Option<String>,
    test_meta_entries: Vec<String>,
}

impl TypeScriptGenerator {
    pub fn new(options: CodegenOptions) -> Self {
        Self {
            indent: 0,
            output: Vec::new(),
            source_file: options.source_file,
            output_file: options.output_file,
            test_meta_entries: Vec::new(),
        }
    }

    /// Determine if declarations should be exported based on source file extension
    /// - .lib.sigil files: export ALL top-level declarations
    /// - .sigil files: export NOTHING (executables)
    fn should_export_from_lib(&self) -> bool {
        if let Some(ref source_file) = self.source_file {
            source_file.ends_with(".lib.sigil")
        } else {
            false
        }
    }

    pub fn generate(&mut self, program: &TypedProgram) -> Result<String, CodegenError> {
        self.output.clear();
        self.indent = 0;
        self.test_meta_entries.clear();
        // Emit mock runtime helpers first
        self.emit_mock_runtime_helpers();

        // Implicit core prelude constructors are available unqualified in every
        // module except the prelude module itself, so runtime code needs the same
        // bindings even though the typechecker injected them implicitly.
        self.emit_core_prelude_runtime_import()?;

        // Generate code for all declarations
        for decl in &program.declarations {
            self.generate_declaration(decl)?;
            self.output.push("\n".to_string());
        }

        // Emit test metadata if any tests were found
        if !self.test_meta_entries.is_empty() {
            self.emit("export const __sigil_tests = [");
            self.indent += 1;
            let entries = self.test_meta_entries.clone();
            for entry in &entries {
                self.emit(&format!("{},", entry));
            }
            self.indent -= 1;
            self.emit("];");
            self.output.push("\n".to_string());
        }

        Ok(self.output.join(""))
    }

    fn emit_core_prelude_runtime_import(&mut self) -> Result<(), CodegenError> {
        let Some(source_file) = self.source_file.as_deref() else {
            return Ok(());
        };

        if source_file.ends_with("language/core/prelude.lib.sigil") {
            return Ok(());
        }

        let import_path = if let Some(ref output_file) = self.output_file {
            let output_path = Path::new(output_file);
            if let Some(local_root) = find_output_root(output_path) {
                let target_abs = local_root.join("core/prelude.js");
                relative_import_path(
                    output_path.parent().unwrap_or_else(|| Path::new(".")),
                    &target_abs,
                )
            } else {
                "./core/prelude.js".to_string()
            }
        } else {
            "./core/prelude.js".to_string()
        };

        self.emit(&format!(
            "import {{ Some, None, Ok, Err, Aborted, Failure, Success }} from '{}';",
            import_path
        ));
        self.output.push("\n".to_string());
        Ok(())
    }

    fn emit(&mut self, line: &str) {
        let indentation = "  ".repeat(self.indent);
        self.output.push(format!("{}{}\n", indentation, line));
    }

    fn js_ready(&self, expr: &str) -> String {
        format!("__sigil_ready({})", expr)
    }

    fn js_all(&self, exprs: &[String]) -> String {
        format!(
            "__sigil_all([{}])",
            exprs
                .iter()
                .map(|expr| format!("() => {}", expr))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn emit_mock_runtime_helpers(&mut self) {
        self.emit("const __sigil_mocks = new Map();");
        self.emit("function __sigil_ready(value) {");
        self.emit("  return Promise.resolve(value);");
        self.emit("}");
        self.emit("function __sigil_all(values) {");
        self.emit("  return values.reduce(async (__sigil_acc_promise, __sigil_thunk) => {");
        self.emit("    const __sigil_acc = await __sigil_acc_promise;");
        self.emit("    __sigil_acc.push(await __sigil_thunk());");
        self.emit("    return __sigil_acc;");
        self.emit("  }, Promise.resolve([]));");
        self.emit("}");
        self.emit("function __sigil_sleep(ms) {");
        self.emit("  return new Promise((resolve) => setTimeout(resolve, Math.max(0, ms)));");
        self.emit("}");
        self.emit("function __sigil_option_value(option) {");
        self.emit("  return option && option.__tag === 'Some' ? option.__fields[0] : null;");
        self.emit("}");
        self.emit("async function __sigil_map_list(items, fn) {");
        self.emit("  const results = [];");
        self.emit("  for (const item of items) {");
        self.emit("    results.push(await fn(item));");
        self.emit("  }");
        self.emit("  return results;");
        self.emit("}");
        self.emit("async function __sigil_filter_list(items, predicate) {");
        self.emit("  const results = [];");
        self.emit("  for (const item of items) {");
        self.emit("    if (await predicate(item)) {");
        self.emit("      results.push(item);");
        self.emit("    }");
        self.emit("  }");
        self.emit("  return results;");
        self.emit("}");
        self.emit("async function __sigil_concurrent_region(name, config, tasks) {");
        self.emit("  const concurrency = Math.max(1, Number(config.concurrency));");
        self.emit("  const jitter = __sigil_option_value(config.jitterMs);");
        self.emit("  const stopOn = config.stopOn;");
        self.emit("  const windowMs = __sigil_option_value(config.windowMs);");
        self.emit("  const outcomes = new Array(tasks.length);");
        self.emit("  const startTimes = [];");
        self.emit("  let nextIndex = 0;");
        self.emit("  let stopRequested = false;");
        self.emit("  function abortedOutcome() { return { __tag: 'Aborted', __fields: [] }; }");
        self.emit("  function failureOutcome(errorValue) { return { __tag: 'Failure', __fields: [errorValue] }; }");
        self.emit(
            "  function successOutcome(value) { return { __tag: 'Success', __fields: [value] }; }",
        );
        self.emit("  async function waitForWindowSlot() {");
        self.emit("    if (windowMs === null) return;");
        self.emit("    while (true) {");
        self.emit("      const now = Date.now();");
        self.emit("      while (startTimes.length > 0 && now - startTimes[0] >= windowMs) {");
        self.emit("        startTimes.shift();");
        self.emit("      }");
        self.emit("      if (startTimes.length < concurrency) return;");
        self.emit("      await __sigil_sleep(startTimes[0] + windowMs - now);");
        self.emit("    }");
        self.emit("  }");
        self.emit("  function jitterDelayMs() {");
        self.emit("    if (jitter === null) return 0;");
        self.emit("    const min = Number(jitter.min);");
        self.emit("    const max = Number(jitter.max);");
        self.emit("    if (!Number.isFinite(min) || !Number.isFinite(max)) return 0;");
        self.emit("    if (max <= min) return Math.max(0, min);");
        self.emit("    return Math.floor(Math.random() * (max - min + 1)) + min;");
        self.emit("  }");
        self.emit("  async function worker() {");
        self.emit("    while (true) {");
        self.emit("      const index = nextIndex;");
        self.emit("      if (index >= tasks.length) return;");
        self.emit("      nextIndex += 1;");
        self.emit("      if (stopRequested) {");
        self.emit("        outcomes[index] = abortedOutcome();");
        self.emit("        continue;");
        self.emit("      }");
        self.emit("      await waitForWindowSlot();");
        self.emit("      if (stopRequested) {");
        self.emit("        outcomes[index] = abortedOutcome();");
        self.emit("        continue;");
        self.emit("      }");
        self.emit("      const delay = jitterDelayMs();");
        self.emit("      if (delay > 0) {");
        self.emit("        await __sigil_sleep(delay);");
        self.emit("      }");
        self.emit("      startTimes.push(Date.now());");
        self.emit("      const result = await tasks[index]();");
        self.emit("      if (result && result.__tag === 'Ok') {");
        self.emit("        outcomes[index] = successOutcome(result.__fields[0]);");
        self.emit("        continue;");
        self.emit("      }");
        self.emit("      if (!result || result.__tag !== 'Err') {");
        self.emit("        throw new Error(`Concurrent region ${name} child returned a non-Result value`);");
        self.emit("      }");
        self.emit("      const errorValue = result.__fields[0];");
        self.emit("      outcomes[index] = failureOutcome(errorValue);");
        self.emit("      if (await stopOn(errorValue)) {");
        self.emit("        stopRequested = true;");
        self.emit("      }");
        self.emit("    }");
        self.emit("  }");
        self.emit("  await Promise.all(Array.from({ length: concurrency }, () => worker()));");
        self.emit("  return outcomes.map((outcome) => outcome ?? abortedOutcome());");
        self.emit("}");
        self.emit("function __sigil_map_empty() {");
        self.emit("  return { __sigil_map: [] };");
        self.emit("}");
        self.emit("function __sigil_map_from_entries(entries) {");
        self.emit("  let current = __sigil_map_empty();");
        self.emit("  for (const [key, value] of entries) { current = __sigil_map_insert(current, key, value); }");
        self.emit("  return current;");
        self.emit("}");
        self.emit("function __sigil_map_get(map, key) {");
        self.emit("  for (const [entryKey, entryValue] of map.__sigil_map) { if (__sigil_deep_equal(entryKey, key)) return { __tag: \"Some\", __fields: [entryValue] }; }");
        self.emit("  return { __tag: \"None\", __fields: [] };");
        self.emit("}");
        self.emit("function __sigil_map_has(map, key) {");
        self.emit("  for (const [entryKey] of map.__sigil_map) { if (__sigil_deep_equal(entryKey, key)) return true; }");
        self.emit("  return false;");
        self.emit("}");
        self.emit("function __sigil_map_insert(map, key, value) {");
        self.emit("  const next = [];");
        self.emit("  let replaced = false;");
        self.emit("  for (const [entryKey, entryValue] of map.__sigil_map) {");
        self.emit("    if (__sigil_deep_equal(entryKey, key)) { if (!replaced) { next.push([key, value]); replaced = true; } } else { next.push([entryKey, entryValue]); }");
        self.emit("  }");
        self.emit("  if (!replaced) next.push([key, value]);");
        self.emit("  return { __sigil_map: next };");
        self.emit("}");
        self.emit("function __sigil_map_remove(map, key) {");
        self.emit("  return { __sigil_map: map.__sigil_map.filter(([entryKey]) => !__sigil_deep_equal(entryKey, key)) };");
        self.emit("}");
        self.emit("function __sigil_map_entries(map) {");
        self.emit("  return map.__sigil_map.slice();");
        self.emit("}");
        self.emit("function __sigil_json_from_js(value) {");
        self.emit("  if (value === null) return { __tag: \"JsonNull\", __fields: [] };");
        self.emit("  if (Array.isArray(value)) return { __tag: \"JsonArray\", __fields: [value.map(__sigil_json_from_js)] };");
        self.emit(
            "  if (typeof value === 'boolean') return { __tag: \"JsonBool\", __fields: [value] };",
        );
        self.emit(
            "  if (typeof value === 'number') return { __tag: \"JsonNumber\", __fields: [value] };",
        );
        self.emit(
            "  if (typeof value === 'string') return { __tag: \"JsonString\", __fields: [value] };",
        );
        self.emit("  if (typeof value === 'object') {");
        self.emit("    return { __tag: \"JsonObject\", __fields: [__sigil_map_from_entries(Object.entries(value).map(([k, v]) => [k, __sigil_json_from_js(v)]))] };");
        self.emit("  }");
        self.emit("  return { __tag: \"JsonNull\", __fields: [] };");
        self.emit("}");
        self.emit("function __sigil_json_to_js(value) {");
        self.emit("  if (!value || typeof value !== 'object') return null;");
        self.emit("  switch (value.__tag) {");
        self.emit(
            "    case 'JsonArray': return (value.__fields[0] ?? []).map(__sigil_json_to_js);",
        );
        self.emit("    case 'JsonBool': return !!value.__fields[0];");
        self.emit("    case 'JsonNull': return null;");
        self.emit("    case 'JsonNumber': return Number(value.__fields[0]);");
        self.emit("    case 'JsonObject': {");
        self.emit("      const result = {};");
        self.emit("      for (const [k, v] of __sigil_map_entries(value.__fields[0] ?? __sigil_map_empty())) { result[String(k)] = __sigil_json_to_js(v); }");
        self.emit("      return result;");
        self.emit("    }");
        self.emit("    case 'JsonString': return String(value.__fields[0] ?? '');");
        self.emit("    default: return null;");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_json_parse_result(input) {");
        self.emit("  try {");
        self.emit(
            "    return { __tag: \"Ok\", __fields: [__sigil_json_from_js(JSON.parse(input))] };",
        );
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_json_stringify_value(value) {");
        self.emit("  return JSON.stringify(__sigil_json_to_js(value));");
        self.emit("}");
        self.emit("function __sigil_time_is_iso(input) {");
        self.emit("  return /^\\d{4}-\\d{2}-\\d{2}(?:T\\d{2}:\\d{2}:\\d{2}(?:\\.\\d{3})?(?:Z|[+-]\\d{2}:\\d{2}))?$/.test(input);");
        self.emit("}");
        self.emit("function __sigil_time_parse_iso_result(input) {");
        self.emit("  if (!__sigil_time_is_iso(input)) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: \"invalid ISO-8601 timestamp\" }] };");
        self.emit("  }");
        self.emit("  const millis = Date.parse(input);");
        self.emit("  if (Number.isNaN(millis)) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: \"invalid ISO-8601 timestamp\" }] };");
        self.emit("  }");
        self.emit("  return { __tag: \"Ok\", __fields: [{ epochMillis: millis }] };");
        self.emit("}");
        self.emit("function __sigil_time_format_iso(instant) {");
        self.emit("  return new Date(instant.epochMillis).toISOString();");
        self.emit("}");
        self.emit("function __sigil_time_now_instant() {");
        self.emit("  return { epochMillis: Date.now() };");
        self.emit("}");
        self.emit("const __sigil_processes = new Map();");
        self.emit("function __sigil_process_env_to_object(envMap) {");
        self.emit("  const out = {};");
        self.emit(
            "  for (const [key, value] of __sigil_map_entries(envMap ?? __sigil_map_empty())) {",
        );
        self.emit("    out[String(key)] = String(value);");
        self.emit("  }");
        self.emit("  return out;");
        self.emit("}");
        self.emit("function __sigil_process_command_cwd(command) {");
        self.emit("  const cwd = command?.cwd;");
        self.emit("  return cwd && cwd.__tag === 'Some' ? cwd.__fields[0] : undefined;");
        self.emit("}");
        self.emit("function __sigil_process_result(code, stderr, stdout) {");
        self.emit("  return { code, stderr, stdout };");
        self.emit("}");
        self.emit("async function __sigil_process_spawn(command) {");
        self.emit("  const { spawn } = await import('child_process');");
        self.emit("  const argv = Array.isArray(command?.argv) ? command.argv : [];");
        self.emit("  if (argv.length === 0) { return { pid: -1 }; }");
        self.emit("  const child = spawn(argv[0], argv.slice(1), {");
        self.emit("    cwd: __sigil_process_command_cwd(command),");
        self.emit("    env: { ...process.env, ...__sigil_process_env_to_object(command?.env) },");
        self.emit("    stdio: ['ignore', 'pipe', 'pipe'],");
        self.emit("  });");
        self.emit("  const pid = typeof child.pid === 'number' ? child.pid : Math.floor(Math.random() * 2147483647);");
        self.emit("  const state = { child, stdout: '', stderr: '', done: null };");
        self.emit("  if (child.stdout) { child.stdout.on('data', (chunk) => { state.stdout += String(chunk); }); }");
        self.emit("  if (child.stderr) { child.stderr.on('data', (chunk) => { state.stderr += String(chunk); }); }");
        self.emit("  state.done = new Promise((resolve) => {");
        self.emit("    child.once('error', (error) => {");
        self.emit("      resolve(__sigil_process_result(-1, state.stderr + String(error?.message ?? error), state.stdout));");
        self.emit("    });");
        self.emit("    child.once('close', (code) => {");
        self.emit("      resolve(__sigil_process_result(typeof code === 'number' ? code : -1, state.stderr, state.stdout));");
        self.emit("    });");
        self.emit("  });");
        self.emit("  __sigil_processes.set(pid, state);");
        self.emit("  return { pid };");
        self.emit("}");
        self.emit("async function __sigil_process_wait(processHandle) {");
        self.emit("  const pid = Number(processHandle?.pid ?? -1);");
        self.emit("  const state = __sigil_processes.get(pid);");
        self.emit("  if (!state) {");
        self.emit("    return __sigil_process_result(-1, 'unknown process', '');");
        self.emit("  }");
        self.emit("  const result = await state.done;");
        self.emit("  __sigil_processes.delete(pid);");
        self.emit("  return result;");
        self.emit("}");
        self.emit("async function __sigil_process_kill(processHandle) {");
        self.emit("  const pid = Number(processHandle?.pid ?? -1);");
        self.emit("  const state = __sigil_processes.get(pid);");
        self.emit("  if (state) {");
        self.emit("    try { state.child.kill(); } catch (_) {}");
        self.emit("  }");
        self.emit("  return null;");
        self.emit("}");
        self.emit("async function __sigil_process_run(command) {");
        self.emit("  const handle = await __sigil_process_spawn(command);");
        self.emit("  return __sigil_process_wait(handle);");
        self.emit("}");
        self.emit("async function __sigil_process_argv() {");
        self.emit("  return process.argv.slice(2);");
        self.emit("}");
        self.emit("async function __sigil_process_exit(code) {");
        self.emit("  process.exit(Number(code));");
        self.emit("  return null;");
        self.emit("}");
        self.emit("function __sigil_regex_compile_result(flags, pattern) {");
        self.emit("  try {");
        self.emit("    const normalizedFlags = String(flags ?? '');");
        self.emit("    const normalizedPattern = String(pattern ?? '');");
        self.emit("    new RegExp(normalizedPattern, normalizedFlags);");
        self.emit("    return { __tag: \"Ok\", __fields: [{ flags: normalizedFlags, pattern: normalizedPattern }] };");
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_regex_find(regex, input) {");
        self.emit("  try {");
        self.emit("    const compiled = new RegExp(String(regex?.pattern ?? ''), String(regex?.flags ?? ''));");
        self.emit("    const source = String(input ?? '');");
        self.emit("    const match = compiled.exec(source);");
        self.emit("    if (!match) { return { __tag: \"None\", __fields: [] }; }");
        self.emit("    return { __tag: \"Some\", __fields: [{ captures: match.slice(1).map((value) => value ?? ''), end: match.index + match[0].length, full: match[0], start: match.index }] };");
        self.emit("  } catch (_) {");
        self.emit("    return { __tag: \"None\", __fields: [] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_regex_is_match(regex, input) {");
        self.emit("  try {");
        self.emit("    return new RegExp(String(regex?.pattern ?? ''), String(regex?.flags ?? '')).test(String(input ?? ''));");
        self.emit("  } catch (_) {");
        self.emit("    return false;");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_url_query_map_from_search(search) {");
        self.emit("  const params = new URLSearchParams(search);");
        self.emit("  return __sigil_map_from_entries(Array.from(params.entries()));");
        self.emit("}");
        self.emit("function __sigil_url_from_absolute(absolute) {");
        self.emit("  const protocol = absolute.protocol.endsWith(':') ? absolute.protocol.slice(0, -1) : absolute.protocol;");
        self.emit("  const port = absolute.port.length > 0 ? { __tag: \"Some\", __fields: [Number(absolute.port)] } : { __tag: \"None\", __fields: [] };");
        self.emit("  return {");
        self.emit("    fragment: absolute.hash || '',");
        self.emit("    host: absolute.hostname || '',");
        self.emit("    path: absolute.pathname || '',");
        self.emit("    port,");
        self.emit("    protocol,");
        self.emit("    query: __sigil_url_query_map_from_search(absolute.search || ''),");
        self.emit("    query_string: absolute.search || ''");
        self.emit("  };");
        self.emit("}");
        self.emit("function __sigil_url_from_relative(input) {");
        self.emit("  const fragmentIndex = input.indexOf('#');");
        self.emit("  const fragment = fragmentIndex >= 0 ? input.slice(fragmentIndex) : '';");
        self.emit(
            "  const withoutFragment = fragmentIndex >= 0 ? input.slice(0, fragmentIndex) : input;",
        );
        self.emit("  const queryIndex = withoutFragment.indexOf('?');");
        self.emit("  const path = queryIndex >= 0 ? withoutFragment.slice(0, queryIndex) : withoutFragment;");
        self.emit(
            "  const queryString = queryIndex >= 0 ? withoutFragment.slice(queryIndex) : '';",
        );
        self.emit("  return {");
        self.emit("    fragment,");
        self.emit("    host: '',");
        self.emit("    path,");
        self.emit("    port: { __tag: \"None\", __fields: [] },");
        self.emit("    protocol: '',");
        self.emit("    query: __sigil_url_query_map_from_search(queryString),");
        self.emit("    query_string: queryString");
        self.emit("  };");
        self.emit("}");
        self.emit("function __sigil_url_parse_result(input) {");
        self.emit("  try {");
        self.emit("    const absolutePattern = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;");
        self.emit("    if (absolutePattern.test(input)) {");
        self.emit("      return { __tag: \"Ok\", __fields: [__sigil_url_from_absolute(new URL(input))] };");
        self.emit("    }");
        self.emit("    return { __tag: \"Ok\", __fields: [__sigil_url_from_relative(input)] };");
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_http_error(kind, message) {");
        self.emit("  return { kind: { __tag: kind, __fields: [] }, message: String(message) };");
        self.emit("}");
        self.emit("function __sigil_http_headers_from_entries(entries) {");
        self.emit("  return __sigil_map_from_entries(entries.map(([key, value]) => [String(key).toLowerCase(), String(value)]));");
        self.emit("}");
        self.emit("function __sigil_http_header_value(value) {");
        self.emit(
            "  if (Array.isArray(value)) return value.map((item) => String(item)).join(', ');",
        );
        self.emit("  if (value === undefined || value === null) return null;");
        self.emit("  return String(value);");
        self.emit("}");
        self.emit("function __sigil_http_headers_from_node(headers) {");
        self.emit("  return __sigil_http_headers_from_entries(Object.entries(headers ?? {}).flatMap(([key, value]) => {");
        self.emit("    const normalized = __sigil_http_header_value(value);");
        self.emit("    return normalized === null ? [] : [[key, normalized]];");
        self.emit("  }));");
        self.emit("}");
        self.emit("function __sigil_http_headers_from_web(headers) {");
        self.emit("  return __sigil_http_headers_from_entries(Array.from(headers.entries()));");
        self.emit("}");
        self.emit("function __sigil_http_headers_to_js(headers) {");
        self.emit("  const result = {};");
        self.emit("  for (const [key, value] of __sigil_map_entries(headers ?? __sigil_map_empty())) { result[String(key)] = String(value); }");
        self.emit("  return result;");
        self.emit("}");
        self.emit("function __sigil_http_method_to_string(method) {");
        self.emit("  switch (method?.__tag) {");
        self.emit("    case 'Delete': return 'DELETE';");
        self.emit("    case 'Get': return 'GET';");
        self.emit("    case 'Patch': return 'PATCH';");
        self.emit("    case 'Post': return 'POST';");
        self.emit("    case 'Put': return 'PUT';");
        self.emit("    default: return 'GET';");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_topology_dependency_name(dep, expectedTag) {");
        self.emit("  if (!dep || typeof dep !== 'object' || dep.__tag !== expectedTag) {");
        self.emit("    throw new Error(`expected ${expectedTag} dependency handle`);");
        self.emit("  }");
        self.emit("  const name = Array.isArray(dep.__fields) ? dep.__fields[0] : null;");
        self.emit("  if (typeof name !== 'string' || name.length === 0) {");
        self.emit("    throw new Error(`invalid ${expectedTag} dependency handle`);");
        self.emit("  }");
        self.emit("  return name;");
        self.emit("}");
        self.emit("function __sigil_topology_http_base_url(dep) {");
        self.emit("  const name = __sigil_topology_dependency_name(dep, 'HttpServiceDependency');");
        self.emit("  const bindings = globalThis.__sigil_topology_bindings?.http;");
        self.emit("  const baseUrl = bindings ? bindings[name] : undefined;");
        self.emit("  if (typeof baseUrl !== 'string' || baseUrl.length === 0) {");
        self.emit("    throw new Error(`missing HTTP topology binding for '${name}'`);");
        self.emit("  }");
        self.emit("  return baseUrl;");
        self.emit("}");
        self.emit("function __sigil_topology_http_url(dep, path) {");
        self.emit("  const baseUrl = __sigil_topology_http_base_url(dep);");
        self.emit("  const relativePath = String(path ?? '/');");
        self.emit("  return new URL(relativePath, baseUrl).toString();");
        self.emit("}");
        self.emit("function __sigil_topology_tcp_target(dep) {");
        self.emit("  const name = __sigil_topology_dependency_name(dep, 'TcpServiceDependency');");
        self.emit("  const bindings = globalThis.__sigil_topology_bindings?.tcp;");
        self.emit("  const target = bindings ? bindings[name] : undefined;");
        self.emit(
            "  if (!target || typeof target.host !== 'string' || !Number.isInteger(target.port)) {",
        );
        self.emit("    throw new Error(`missing TCP topology binding for '${name}'`);");
        self.emit("  }");
        self.emit("  return target;");
        self.emit("}");
        self.emit("async function __sigil_http_request(request) {");
        self.emit("  try {");
        self.emit("    const parsed = new URL(__sigil_topology_http_url(request.dependency, request.path));");
        self.emit("    const init = { headers: __sigil_http_headers_to_js(request.headers), method: __sigil_http_method_to_string(request.method) };");
        self.emit(
            "    if (request.body?.__tag === 'Some') { init.body = request.body.__fields[0]; }",
        );
        self.emit("    const response = await fetch(parsed, init);");
        self.emit("    const body = await response.text();");
        self.emit("    return { __tag: 'Ok', __fields: [{ body, headers: __sigil_http_headers_from_web(response.headers), status: response.status, url: response.url }] };");
        self.emit("  } catch (error) {");
        self.emit("    const message = error instanceof Error ? error.message : String(error);");
        self.emit("    if (message.includes('topology binding') || message.includes('dependency handle')) {");
        self.emit(
            "      return { __tag: 'Err', __fields: [__sigil_http_error('Topology', message)] };",
        );
        self.emit("    }");
        self.emit("    if (message.includes('Invalid URL')) {");
        self.emit(
            "      return { __tag: 'Err', __fields: [__sigil_http_error('InvalidUrl', message)] };",
        );
        self.emit("    }");
        self.emit(
            "    return { __tag: 'Err', __fields: [__sigil_http_error('Network', message)] };",
        );
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_http_request_path(url) {");
        self.emit("  try {");
        self.emit("    const parsed = new URL(String(url ?? '/'), 'http://127.0.0.1');");
        self.emit("    return parsed.pathname || '/';");
        self.emit("  } catch (_) {");
        self.emit("    return '/';");
        self.emit("  }");
        self.emit("}");
        self.emit("async function __sigil_http_serve(handler, port) {");
        self.emit("  const { createServer } = await import('node:http');");
        self.emit("  const { text } = await import('stream/consumers');");
        self.emit("  const server = createServer(async (req, res) => {");
        self.emit("    try {");
        self.emit("      const request = {");
        self.emit("        body: await text(req),");
        self.emit("        headers: __sigil_http_headers_from_node(req.headers),");
        self.emit("        method: String(req.method ?? 'GET'),");
        self.emit("        path: __sigil_http_request_path(req.url)");
        self.emit("      };");
        self.emit("      const response = await Promise.resolve(handler(request));");
        self.emit(
            "      res.writeHead(response.status, __sigil_http_headers_to_js(response.headers));",
        );
        self.emit("      res.end(String(response.body));");
        self.emit("    } catch (error) {");
        self.emit("      const message = error instanceof Error ? error.message : String(error);");
        self.emit("      res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' });");
        self.emit("      res.end(message);");
        self.emit("    }");
        self.emit("  });");
        self.emit("  await new Promise((resolve, reject) => {");
        self.emit("    server.once('error', reject);");
        self.emit("    server.listen(port, () => resolve(undefined));");
        self.emit("  });");
        self.emit("  console.log(`Server running at http://localhost:${String(port)}`);");
        self.emit("  await new Promise(() => {});");
        self.emit("}");
        self.emit("function __sigil_tcp_error(kind, message) {");
        self.emit("  return { kind: { __tag: kind, __fields: [] }, message: String(message) };");
        self.emit("}");
        self.emit("function __sigil_tcp_is_valid_host(host) {");
        self.emit("  return typeof host === 'string' && host.length > 0;");
        self.emit("}");
        self.emit("function __sigil_tcp_is_valid_port(port) {");
        self.emit("  return Number.isInteger(port) && port > 0 && port <= 65535;");
        self.emit("}");
        self.emit("function __sigil_tcp_first_line(buffer) {");
        self.emit("  const index = buffer.indexOf('\\n');");
        self.emit("  return index === -1 ? null : buffer.slice(0, index).replace(/\\r$/, '');");
        self.emit("}");
        self.emit("async function __sigil_tcp_request(request) {");
        self.emit("  let target;");
        self.emit("  try {");
        self.emit("    target = __sigil_topology_tcp_target(request?.dependency);");
        self.emit("  } catch (error) {");
        self.emit("    const message = error instanceof Error ? error.message : String(error);");
        self.emit(
            "    return { __tag: 'Err', __fields: [__sigil_tcp_error('Topology', message)] };",
        );
        self.emit("  }");
        self.emit("  if (!__sigil_tcp_is_valid_host(target?.host) || !__sigil_tcp_is_valid_port(target?.port)) {");
        self.emit("    return { __tag: 'Err', __fields: [__sigil_tcp_error('InvalidAddress', 'TCP requests require a valid host and port')] };");
        self.emit("  }");
        self.emit("  const { Socket } = await import('node:net');");
        self.emit("  return await new Promise((resolve) => {");
        self.emit("    const socket = new Socket();");
        self.emit("    let settled = false;");
        self.emit("    let received = '';");
        self.emit("    const finish = (value) => {");
        self.emit("      if (settled) return;");
        self.emit("      settled = true;");
        self.emit("      socket.destroy();");
        self.emit("      resolve(value);");
        self.emit("    };");
        self.emit("    socket.setEncoding('utf8');");
        self.emit("    socket.setTimeout(5000);");
        self.emit("    socket.once('connect', () => {");
        self.emit("      socket.write(`${String(request.message)}\\n`);");
        self.emit("    });");
        self.emit("    socket.on('data', (chunk) => {");
        self.emit("      received += String(chunk);");
        self.emit("      const line = __sigil_tcp_first_line(received);");
        self.emit("      if (line !== null) {");
        self.emit("        finish({ __tag: 'Ok', __fields: [{ message: line }] });");
        self.emit("      }");
        self.emit("    });");
        self.emit("    socket.once('timeout', () => {");
        self.emit("      finish({ __tag: 'Err', __fields: [__sigil_tcp_error('Timeout', 'TCP request timed out')] });");
        self.emit("    });");
        self.emit("    socket.once('error', (error) => {");
        self.emit("      finish({ __tag: 'Err', __fields: [__sigil_tcp_error('Connection', error instanceof Error ? error.message : String(error))] });");
        self.emit("    });");
        self.emit("    socket.once('close', () => {");
        self.emit("      if (!settled) {");
        self.emit("        finish({ __tag: 'Err', __fields: [__sigil_tcp_error('Protocol', 'TCP response closed before a newline-delimited message was received')] });");
        self.emit("      }");
        self.emit("    });");
        self.emit("    socket.connect(target.port, target.host);");
        self.emit("  });");
        self.emit("}");
        self.emit("async function __sigil_tcp_serve(handler, port) {");
        self.emit("  const { createServer } = await import('node:net');");
        self.emit("  const server = createServer((socket) => {");
        self.emit("    socket.setEncoding('utf8');");
        self.emit("    let received = '';");
        self.emit("    let handled = false;");
        self.emit("    socket.on('data', async (chunk) => {");
        self.emit("      if (handled) return;");
        self.emit("      received += String(chunk);");
        self.emit("      const line = __sigil_tcp_first_line(received);");
        self.emit("      if (line === null) return;");
        self.emit("      handled = true;");
        self.emit("      try {");
        self.emit("        const request = {");
        self.emit("          host: String(socket.remoteAddress ?? ''),");
        self.emit("          message: line,");
        self.emit("          port: Number(port)");
        self.emit("        };");
        self.emit("        const response = await Promise.resolve(handler(request));");
        self.emit("        socket.write(`${String(response.message)}\\n`, () => socket.end());");
        self.emit("      } catch (error) {");
        self.emit(
            "        const message = error instanceof Error ? error.message : String(error);",
        );
        self.emit("        socket.write(`${message}\\n`, () => socket.end());");
        self.emit("      }");
        self.emit("    });");
        self.emit("    socket.once('end', () => {");
        self.emit("      if (!handled) {");
        self.emit("        socket.write('protocol error: missing newline-delimited request\\n', () => socket.end());");
        self.emit("      }");
        self.emit("    });");
        self.emit("    socket.once('error', () => {");
        self.emit("      socket.destroy();");
        self.emit("    });");
        self.emit("  });");
        self.emit("  await new Promise((resolve, reject) => {");
        self.emit("    server.once('error', reject);");
        self.emit("    server.listen(port, () => resolve(undefined));");
        self.emit("  });");
        self.emit("  console.log(`TCP server running at tcp://127.0.0.1:${String(port)}`);");
        self.emit("  await new Promise(() => {});");
        self.emit("}");
        self.emit("function __sigil_is_map(value) {");
        self.emit(
            "  return !!value && typeof value === 'object' && Array.isArray(value.__sigil_map);",
        );
        self.emit("}");
        self.emit("function __sigil_deep_equal(a, b) {");
        self.emit("  if (a === b) return true;");
        self.emit("  if (a == null || b == null) return false;");
        self.emit("  if (typeof a !== typeof b) return false;");
        self.emit("  if (__sigil_is_map(a) && __sigil_is_map(b)) {");
        self.emit("    if (a.__sigil_map.length !== b.__sigil_map.length) return false;");
        self.emit("    for (const [aKey, aValue] of a.__sigil_map) {");
        self.emit("      let matched = false;");
        self.emit("      for (const [bKey, bValue] of b.__sigil_map) {");
        self.emit("        if (__sigil_deep_equal(aKey, bKey)) {");
        self.emit("          if (!__sigil_deep_equal(aValue, bValue)) return false;");
        self.emit("          matched = true;");
        self.emit("          break;");
        self.emit("        }");
        self.emit("      }");
        self.emit("      if (!matched) return false;");
        self.emit("    }");
        self.emit("    return true;");
        self.emit("  }");
        self.emit("  if (Array.isArray(a) && Array.isArray(b)) {");
        self.emit("    if (a.length !== b.length) return false;");
        self.emit("    for (let i = 0; i < a.length; i++) {");
        self.emit("      if (!__sigil_deep_equal(a[i], b[i])) return false;");
        self.emit("    }");
        self.emit("    return true;");
        self.emit("  }");
        self.emit("  if (typeof a === 'object' && typeof b === 'object') {");
        self.emit("    const aKeys = Object.keys(a).sort();");
        self.emit("    const bKeys = Object.keys(b).sort();");
        self.emit("    if (aKeys.length !== bKeys.length) return false;");
        self.emit("    for (let i = 0; i < aKeys.length; i++) {");
        self.emit("      if (aKeys[i] !== bKeys[i]) return false;");
        self.emit("      if (!__sigil_deep_equal(a[aKeys[i]], b[bKeys[i]])) return false;");
        self.emit("    }");
        self.emit("    return true;");
        self.emit("  }");
        self.emit("  return false;");
        self.emit("}");
        self.emit("function __sigil_preview(value) {");
        self.emit("  try { return JSON.stringify(value); } catch { return String(value); }");
        self.emit("}");
        self.emit("function __sigil_diff_hint(actual, expected) {");
        self.emit("  if (Array.isArray(actual) && Array.isArray(expected)) {");
        self.emit("    if (actual.length !== expected.length) { return { kind: 'array_length', actualLength: actual.length, expectedLength: expected.length }; }");
        self.emit("    for (let i = 0; i < actual.length; i++) { if (actual[i] !== expected[i]) { return { kind: 'array_first_diff', index: i, actual: __sigil_preview(actual[i]), expected: __sigil_preview(expected[i]) }; } }");
        self.emit("    return null;");
        self.emit("  }");
        self.emit("  if (actual && expected && typeof actual === 'object' && typeof expected === 'object') {");
        self.emit("    const actualKeys = Object.keys(actual).sort();");
        self.emit("    const expectedKeys = Object.keys(expected).sort();");
        self.emit("    if (actualKeys.join('|') !== expectedKeys.join('|')) { return { kind: 'object_keys', actualKeys, expectedKeys }; }");
        self.emit("    for (const k of actualKeys) { if (actual[k] !== expected[k]) { return { kind: 'object_field', field: k, actual: __sigil_preview(actual[k]), expected: __sigil_preview(expected[k]) }; } }");
        self.emit("    return null;");
        self.emit("  }");
        self.emit("  return null;");
        self.emit("}");
        self.emit("async function __sigil_test_bool_result(ok) {");
        self.emit("  const result = await ok;");
        self.emit("  return result === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to false' } };");
        self.emit("}");
        self.emit("async function __sigil_test_compare_result(op, leftFn, rightFn) {");
        self.emit("  const actual = await leftFn();");
        self.emit("  const expected = await rightFn();");
        self.emit("  let ok = false;");
        self.emit("  switch (op) {");
        self.emit("    case '=': ok = __sigil_deep_equal(actual, expected); break;");
        self.emit("    case '≠': ok = !__sigil_deep_equal(actual, expected); break;");
        self.emit("    case '<': ok = actual < expected; break;");
        self.emit("    case '>': ok = actual > expected; break;");
        self.emit("    case '≤': ok = actual <= expected; break;");
        self.emit("    case '≥': ok = actual >= expected; break;");
        self.emit(
            "    default: throw new Error('Unsupported test comparison operator: ' + String(op));",
        );
        self.emit("  }");
        self.emit("  if (ok) { return { ok: true }; }");
        self.emit("  return { ok: false, failure: { kind: 'comparison_mismatch', message: 'Comparison test failed', operator: op, actual: __sigil_preview(actual), expected: __sigil_preview(expected), diffHint: __sigil_diff_hint(actual, expected) } };");
        self.emit("}");
        self.emit("function __sigil_call(key, actualFn, args = []) {");
        self.emit("  const mockFn = __sigil_mocks.get(key);");
        self.emit("  const fn = mockFn ?? actualFn;");
        self.emit("  return Promise.resolve().then(() => fn(...args));");
        self.emit("}");
        self.emit("async function __sigil_with_mock(key, mockFn, body) {");
        self.emit("  const had = __sigil_mocks.has(key);");
        self.emit("  const prev = __sigil_mocks.get(key);");
        self.emit("  __sigil_mocks.set(key, mockFn);");
        self.emit("  try {");
        self.emit("    return await body();");
        self.emit("  } finally {");
        self.emit(
            "    if (had) { __sigil_mocks.set(key, prev); } else { __sigil_mocks.delete(key); }",
        );
        self.emit("  }");
        self.emit("}");
        self.emit("async function __sigil_with_mock_extern(key, actualFn, mockFn, body) {");
        self.emit("  if (typeof actualFn !== 'function') { throw new Error('withMock extern target is not callable'); }");
        self.emit("  if (typeof mockFn !== 'function') { throw new Error('withMock replacement must be callable'); }");
        self.emit("  if (actualFn.length !== mockFn.length) { throw new Error(`withMock extern arity mismatch for ${key}: expected ${actualFn.length}, got ${mockFn.length}`); }");
        self.emit("  return await __sigil_with_mock(key, mockFn, body);");
        self.emit("}");
    }

    fn generate_declaration(&mut self, decl: &TypedDeclaration) -> Result<(), CodegenError> {
        match decl {
            TypedDeclaration::Function(func) => self.generate_function(func),
            TypedDeclaration::Type(type_decl) => self.generate_type_decl(&type_decl.ast),
            TypedDeclaration::Const(const_decl) => self.generate_const(const_decl),
            TypedDeclaration::Import(import) => self.generate_import(&import.ast),
            TypedDeclaration::Extern(extern_decl) => self.generate_extern(&extern_decl.ast),
            TypedDeclaration::Test(test) => self.generate_test(test),
        }
    }

    fn generate_function(&mut self, func: &TypedFunctionDecl) -> Result<(), CodegenError> {
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/core/map.lib.sigil"))
        {
            if self.generate_core_map_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/httpClient.lib.sigil"))
        {
            if self.generate_stdlib_http_client_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/httpServer.lib.sigil"))
        {
            if self.generate_stdlib_http_server_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/tcpClient.lib.sigil"))
        {
            if self.generate_stdlib_tcp_client_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/tcpServer.lib.sigil"))
        {
            if self.generate_stdlib_tcp_server_function(func)? {
                return Ok(());
            }
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let func_name = sanitize_js_identifier(&func.name);

        // Export logic:
        // - .lib.sigil files: export all functions
        // - .sigil files: export main() only (for executables)
        let should_export = if self.should_export_from_lib() {
            true
        } else {
            func.name == "main"
        };

        let fn_keyword = if should_export {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!("{} {}({}) {{", fn_keyword, func_name, params_str));
        self.indent += 1;

        let body_code = self.generate_expression(&func.body)?;
        self.emit(&format!("return {};", body_code));

        self.indent -= 1;
        self.emit("}");

        Ok(())
    }

    fn generate_core_map_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        let body = match (func.name.as_str(), params.as_slice()) {
            ("empty", []) => Some("__sigil_ready(__sigil_map_empty())".to_string()),
            ("entries", [map]) => Some(format!(
                "{}.then((__map) => __sigil_map_entries(__map).map(([__key, __value]) => ({{ key: __key, value: __value }})))",
                self.js_ready(map)
            )),
            ("filter", [map, pred]) => Some(format!(
                "{}.then(async ([__map, __fn]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ if (await Promise.resolve(__fn(__key, __value))) {{ __current = __sigil_map_insert(__current, __key, __value); }} }} return __current; }})",
                self.js_all(&[self.js_ready(map), self.js_ready(pred)])
            )),
            ("fold", [fn_name, init, map]) => Some(format!(
                "{}.then(async ([__fn, __acc, __map]) => {{ let __current = __acc; for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = await Promise.resolve(__fn(__current, __key, __value)); }} return __current; }})",
                self.js_all(&[self.js_ready(fn_name), self.js_ready(init), self.js_ready(map)])
            )),
            ("fromList", [entries]) => Some(format!(
                "{}.then((__entries) => __sigil_map_from_entries(__entries.map((__entry) => [__entry.key, __entry.value])))",
                self.js_ready(entries)
            )),
            ("get", [key, map]) => Some(format!(
                "{}.then(([__key, __map]) => __sigil_map_get(__map, __key))",
                self.js_all(&[self.js_ready(key), self.js_ready(map)])
            )),
            ("has", [key, map]) => Some(format!(
                "{}.then(([__key, __map]) => __sigil_map_has(__map, __key))",
                self.js_all(&[self.js_ready(key), self.js_ready(map)])
            )),
            ("insert", [key, map, value]) => Some(format!(
                "{}.then(([__key, __map, __value]) => __sigil_map_insert(__map, __key, __value))",
                self.js_all(&[
                    self.js_ready(key),
                    self.js_ready(map),
                    self.js_ready(value),
                ])
            )),
            ("keys", [map]) => Some(format!(
                "{}.then((__map) => __sigil_map_entries(__map).map(([__key]) => __key))",
                self.js_ready(map)
            )),
            ("mapValues", [fn_name, map]) => Some(format!(
                "{}.then(async ([__fn, __map]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = __sigil_map_insert(__current, __key, await Promise.resolve(__fn(__value))); }} return __current; }})",
                self.js_all(&[self.js_ready(fn_name), self.js_ready(map)])
            )),
            ("merge", [left, right]) => Some(format!(
                "{}.then(([__left, __right]) => {{ let __current = __sigil_map_from_entries(__sigil_map_entries(__left)); for (const [__key, __value] of __sigil_map_entries(__right)) {{ __current.__sigil_map = __sigil_map_insert(__current, __key, __value).__sigil_map; }} return __current; }})",
                self.js_all(&[self.js_ready(left), self.js_ready(right)])
            )),
            ("remove", [key, map]) => Some(format!(
                "{}.then(([__key, __map]) => __sigil_map_remove(__map, __key))",
                self.js_all(&[self.js_ready(key), self.js_ready(map)])
            )),
            ("singleton", [key, value]) => Some(format!(
                "{}.then(([__key, __value]) => __sigil_map_insert(__sigil_map_empty(), __key, __value))",
                self.js_all(&[self.js_ready(key), self.js_ready(value)])
            )),
            ("size", [map]) => Some(format!(
                "{}.then((__map) => __map.__sigil_map.length)",
                self.js_ready(map)
            )),
            ("values", [map]) => Some(format!(
                "{}.then((__map) => __sigil_map_entries(__map).map(([_, __value]) => __value))",
                self.js_ready(map)
            )),
            _ => None,
        };

        let Some(body) = body else {
            return Ok(false);
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!("return {};", body));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_http_client_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "request" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then((__request) => __sigil_http_request(__request));",
            self.js_ready(&params[0])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_http_server_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "serve" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then(([__handler, __port]) => __sigil_http_serve(__handler, __port));",
            self.js_all(&[self.js_ready(&params[0]), self.js_ready(&params[1])])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_tcp_client_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "request" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then((__request) => __sigil_tcp_request(__request));",
            self.js_ready(&params[0])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_tcp_server_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "serve" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then(([__handler, __port]) => __sigil_tcp_serve(__handler, __port));",
            self.js_all(&[self.js_ready(&params[0]), self.js_ready(&params[1])])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_type_decl(&mut self, type_decl: &TypeDecl) -> Result<(), CodegenError> {
        // Generate constructor functions for sum types
        if let TypeDef::Sum(sum_type) = &type_decl.definition {
            let type_params = if type_decl.type_params.is_empty() {
                String::new()
            } else {
                format!("[{}]", type_decl.type_params.join(","))
            };
            self.emit(&format!("// type {}{}", type_decl.name, type_params));

            for variant in &sum_type.variants {
                // Generate constructor function
                // Example: Some(x) => { __tag: "Some", __fields: [x] }
                let param_names: Vec<String> = (0..variant.types.len())
                    .map(|i| format!("_{}", i))
                    .collect();
                let params = param_names.join(", ");

                // Export constructors from .lib.sigil files
                let ctor_keyword = if self.should_export_from_lib() {
                    "export function"
                } else {
                    "function"
                };

                self.emit(&format!(
                    "{} {}({}) {{",
                    ctor_keyword,
                    sanitize_js_identifier(&variant.name),
                    params
                ));
                self.indent += 1;
                if param_names.is_empty() {
                    self.emit(&format!(
                        "return __sigil_ready({{ __tag: \"{}\", __fields: [] }});",
                        variant.name
                    ));
                } else {
                    self.emit(&format!(
                        "return {}.then((__fields) => ({{ __tag: \"{}\", __fields }}));",
                        self.js_all(&param_names),
                        variant.name
                    ));
                }
                self.indent -= 1;
                self.emit("}");
            }
        } else {
            // Product types and type aliases are erased for now
            self.emit(&format!("// type {} (erased)", type_decl.name));
        }

        Ok(())
    }

    fn generate_const(&mut self, const_decl: &TypedConstDecl) -> Result<(), CodegenError> {
        let value = self.generate_expression(&const_decl.value)?;
        // Export consts from .lib.sigil files
        let export_keyword = if self.should_export_from_lib() {
            "export "
        } else {
            ""
        };
        self.emit(&format!(
            "{}const {} = {};",
            export_keyword,
            sanitize_js_identifier(&const_decl.name),
            value
        ));
        Ok(())
    }

    fn generate_import(&mut self, import: &ImportDecl) -> Result<(), CodegenError> {
        // Convert Sigil import to ES module import (namespace style)
        // For src::utils, create:
        //   - namespace: src_utils (matches member access generation)
        //   - import path: relative to current output file
        let namespace = sanitize_js_identifier(&import.module_path.join("_"));
        let import_path = if let Some(ref output_file) = self.output_file {
            let output_path = Path::new(output_file);
            if let Some(local_root) = find_output_root(output_path) {
                let target_abs = local_root
                    .join(import.module_path.join("/"))
                    .with_extension("js");
                relative_import_path(
                    output_path.parent().unwrap_or_else(|| Path::new(".")),
                    &target_abs,
                )
            } else {
                format!("./{}.js", import.module_path.join("/"))
            }
        } else {
            format!("./{}.js", import.module_path.join("/"))
        };

        self.emit(&format!(
            "import * as {} from '{}';",
            namespace, import_path
        ));
        Ok(())
    }

    fn generate_extern(&mut self, extern_decl: &ExternDecl) -> Result<(), CodegenError> {
        // Extern declarations become ES module imports
        let module_path = extern_decl.module_path.join("/");
        if extern_decl
            .members
            .as_ref()
            .map(|members: &Vec<_>| !members.is_empty())
            .unwrap_or(false)
        {
            // Typed extern with declared members: import only those members
            if let Some(members) = &extern_decl.members {
                let imports = members
                    .iter()
                    .map(|member| member.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.emit(&format!("import {{ {} }} from '{}';", imports, module_path));
            }
        } else {
            // Untyped extern or no declared members: namespace import
            let namespace = sanitize_js_identifier(&extern_decl.module_path.join("_"));
            self.emit(&format!(
                "import * as {} from '{}';",
                namespace, module_path
            ));
        }

        Ok(())
    }

    fn generate_test(&mut self, test: &TypedTestDecl) -> Result<(), CodegenError> {
        // Generate a unique test name from the description
        let test_name = test
            .description
            .chars()
            .filter(|c: &char| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase();
        let test_name = if test_name.is_empty() {
            format!("test_{}", self.test_meta_entries.len())
        } else {
            test_name
        };

        // Generate test function
        self.emit(&format!("async function __test_{}() {{", test_name));
        self.indent += 1;
        let body = self.generate_expression(&test.body)?;
        self.emit(&format!("return {};", body));
        self.indent -= 1;
        self.emit("}");

        // Add to test metadata
        let description = test.description.replace('\"', "\\\"");
        self.test_meta_entries.push(format!(
            "{{ id: '{}::{}', name: '{}', description: '{}', location: {{ start: {{ line: {}, column: {} }} }}, fn: __test_{} }}",
            self.source_file.as_deref().unwrap_or("<unknown>"),
            test_name,
            test_name,
            description,
            test.location.start.line,
            test.location.start.column,
            test_name
        ));

        Ok(())
    }

    fn generate_expression(&mut self, expr: &TypedExpr) -> Result<String, CodegenError> {
        match &expr.kind {
            TypedExprKind::Literal(lit) => self.generate_literal(lit),
            TypedExprKind::Identifier(id) => Ok(self.js_ready(&sanitize_js_identifier(&id.name))),
            TypedExprKind::NamespaceMember { namespace, member } => Ok(self.js_ready(&format!(
                "{}.{}",
                sanitize_js_identifier(&namespace.join("_")),
                sanitize_js_identifier(member)
            ))),
            TypedExprKind::Lambda(lambda) => self.generate_lambda(lambda),
            TypedExprKind::Call(call) => self.generate_call(call),
            TypedExprKind::ConstructorCall(call) => self.generate_constructor_call(call),
            TypedExprKind::ExternCall(call) => self.generate_extern_call(call),
            TypedExprKind::MethodCall(call) => self.generate_method_call(call),
            TypedExprKind::Binary(bin) => self.generate_binary(bin),
            TypedExprKind::Unary(un) => self.generate_unary(un),
            TypedExprKind::Match(match_expr) => self.generate_match(match_expr),
            TypedExprKind::Let(let_expr) => self.generate_let(let_expr),
            TypedExprKind::If(if_expr) => self.generate_if(if_expr),
            TypedExprKind::List(list) => self.generate_list(list),
            TypedExprKind::Tuple(tuple) => self.generate_tuple(tuple),
            TypedExprKind::Record(record) => self.generate_record(record),
            TypedExprKind::MapLiteral(map) => self.generate_map_literal(map),
            TypedExprKind::FieldAccess(field_access) => self.generate_field_access(field_access),
            TypedExprKind::Index(index) => self.generate_index(index),
            TypedExprKind::Map(map) => self.generate_map(map),
            TypedExprKind::Filter(filter) => self.generate_filter(filter),
            TypedExprKind::Fold(fold) => self.generate_fold(fold),
            TypedExprKind::Concurrent(concurrent) => self.generate_concurrent(concurrent),
            TypedExprKind::Pipeline(pipeline) => self.generate_pipeline(pipeline),
            TypedExprKind::WithMock(with_mock) => self.generate_with_mock(with_mock),
        }
    }

    fn generate_literal(&mut self, lit: &LiteralExpr) -> Result<String, CodegenError> {
        let value = match &lit.value {
            LiteralValue::Int(n) => n.to_string(),
            LiteralValue::Float(f) => f.to_string(),
            LiteralValue::String(s) => serde_json::to_string(s).unwrap(),
            LiteralValue::Char(c) => serde_json::to_string(&c.to_string()).unwrap(),
            LiteralValue::Bool(b) => b.to_string(),
            LiteralValue::Unit => "null".to_string(),
        };
        Ok(self.js_ready(&value))
    }

    fn generate_lambda(&mut self, lambda: &TypedLambdaExpr) -> Result<String, CodegenError> {
        let params: Vec<String> = lambda
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let body = self.generate_expression(&lambda.body)?;
        Ok(format!("(({}) => {})", params_str, body))
    }

    fn generate_call(&mut self, call: &TypedCallExpr) -> Result<String, CodegenError> {
        if let Some(intrinsic) = self.try_generate_typed_intrinsic(&call.func, &call.args)? {
            return Ok(intrinsic);
        }

        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;
        match &call.func.kind {
            TypedExprKind::Identifier(id) => Ok(format!(
                "{}.then((__sigil_args) => __sigil_call(\"{}\", {}, __sigil_args))",
                self.js_all(&args),
                id.name,
                sanitize_js_identifier(&id.name)
            )),
            TypedExprKind::NamespaceMember { namespace, member } => {
                let func_ref = format!(
                    "{}.{}",
                    sanitize_js_identifier(&namespace.join("_")),
                    sanitize_js_identifier(member)
                );
                Ok(format!(
                    "{}.then((__sigil_args) => __sigil_call(\"extern:{}.{}\", {}, __sigil_args))",
                    self.js_all(&args),
                    namespace.join("/"),
                    member,
                    func_ref
                ))
            }
            _ => {
                let func = self.generate_expression(&call.func)?;
                let mut values = vec![func];
                values.extend(args);
                Ok(format!(
                    "{}.then(([__sigil_fn, ...__sigil_args]) => __sigil_fn(...__sigil_args))",
                    self.js_all(&values)
                ))
            }
        }
    }

    fn try_generate_typed_intrinsic(
        &mut self,
        func: &TypedExpr,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        match &func.kind {
            TypedExprKind::NamespaceMember { namespace, member } => {
                let module = namespace.join("/");
                if module == "stdlib/string" {
                    return self.generate_string_intrinsic(member, args);
                }
                if module == "stdlib/json" {
                    return self.generate_json_intrinsic(member, args);
                }
                if module == "stdlib/httpClient" {
                    return self.generate_http_client_intrinsic(member, args);
                }
                if module == "stdlib/httpServer" {
                    return self.generate_http_server_intrinsic(member, args);
                }
                if module == "stdlib/tcpClient" {
                    return self.generate_tcp_client_intrinsic(member, args);
                }
                if module == "stdlib/tcpServer" {
                    return self.generate_tcp_server_intrinsic(member, args);
                }
                if module == "stdlib/time" {
                    return self.generate_time_intrinsic(member, args);
                }
                if module == "stdlib/process" {
                    return self.generate_process_intrinsic(member, args);
                }
                if module == "stdlib/url" {
                    return self.generate_url_intrinsic(member, args);
                }
                if module == "core/map" {
                    return self.generate_map_intrinsic(member, args);
                }
                Ok(None)
            }
            TypedExprKind::Identifier(name) => {
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/string.lib.sigil"))
                {
                    return self.generate_string_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/json.lib.sigil"))
                {
                    return self.generate_json_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/httpClient.lib.sigil"))
                {
                    return self.generate_http_client_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/httpServer.lib.sigil"))
                {
                    return self.generate_http_server_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/tcpClient.lib.sigil"))
                {
                    return self.generate_tcp_client_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/tcpServer.lib.sigil"))
                {
                    return self.generate_tcp_server_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/time.lib.sigil"))
                {
                    return self.generate_time_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/process.lib.sigil"))
                {
                    return self.generate_process_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/url.lib.sigil"))
                {
                    return self.generate_url_intrinsic(&name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/core/map.lib.sigil"))
                {
                    return self.generate_map_intrinsic(&name.name, args);
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn generate_string_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args: Result<Vec<String>, CodegenError> = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect();
        let generated_args = generated_args?;

        match member {
            "charAt" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__index, __string]) => __sigil_ready(__string.charAt(__index)))", self.js_all(&generated_args))))
            }
            "substring" if generated_args.len() == 3 => {
                Ok(Some(format!("{}.then(([__end, __string, __start]) => __sigil_ready(__string.substring(__start, __end)))", self.js_all(&generated_args))))
            }
            "toUpper" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.toUpperCase())", generated_args[0])))
            }
            "toLower" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.toLowerCase())", generated_args[0])))
            }
            "trim" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.trim())", generated_args[0])))
            }
            "indexOf" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __needle]) => __string.indexOf(__needle))", self.js_all(&generated_args))))
            }
            "split" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__separator, __string]) => __string.split(__separator))", self.js_all(&generated_args))))
            }
            "reverse" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __sigil_ready(__value.split(\"\").reverse().join(\"\")))", generated_args[0])))
            }
            "replaceAll" if generated_args.len() == 3 => {
                Ok(Some(format!("{}.then(([__search, __replacement, __string]) => __string.replaceAll(__search, __replacement))", self.js_all(&generated_args))))
            }
            "intToString" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => String(__value))", generated_args[0])))
            }
            "join" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__separator, __items]) => __items.join(__separator))", self.js_all(&generated_args))))
            }
            "take" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__count, __string]) => __string.substring(0, __count))", self.js_all(&generated_args))))
            }
            "drop" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__count, __string]) => __string.substring(__count))", self.js_all(&generated_args))))
            }
            "startsWith" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__prefix, __string]) => __string.startsWith(__prefix))", self.js_all(&generated_args))))
            }
            "endsWith" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __suffix]) => __string.endsWith(__suffix))", self.js_all(&generated_args))))
            }
            "isDigit" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => /^[0-9]$/.test(__value))", generated_args[0])))
            }
            _ => Ok(None),
        }
    }

    fn generate_map_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "empty" if generated_args.is_empty() => {
                Ok(Some("__sigil_ready(__sigil_map_empty())".to_string()))
            }
            "entries" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __sigil_map_entries(__map).map(([__key, __value]) => ({{ key: __key, value: __value }})))",
                    generated_args[0]
                )))
            }
            "filter" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(async ([__map, __fn]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ if (await Promise.resolve(__fn(__key, __value))) {{ __current = __sigil_map_insert(__current, __key, __value); }} }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "fold" if generated_args.len() == 3 => {
                Ok(Some(format!(
                    "{}.then(async ([__fn, __acc, __map]) => {{ let __current = __acc; for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = await Promise.resolve(__fn(__current, __key, __value)); }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "fromList" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__entries) => __sigil_map_from_entries(__entries.map((__entry) => [__entry.key, __entry.value])))",
                    generated_args[0]
                )))
            }
            "get" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map]) => __sigil_map_get(__map, __key))",
                    self.js_all(&generated_args)
                )))
            }
            "has" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map]) => __sigil_map_has(__map, __key))",
                    self.js_all(&generated_args)
                )))
            }
            "insert" if generated_args.len() == 3 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map, __value]) => __sigil_map_insert(__map, __key, __value))",
                    self.js_all(&generated_args)
                )))
            }
            "keys" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __sigil_map_entries(__map).map(([__key]) => __key))",
                    generated_args[0]
                )))
            }
            "mapValues" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(async ([__fn, __map]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = __sigil_map_insert(__current, __key, await Promise.resolve(__fn(__value))); }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "merge" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__left, __right]) => {{ let __current = __left; for (const [__key, __value] of __sigil_map_entries(__right)) {{ __current = __sigil_map_insert(__current, __key, __value); }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "remove" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map]) => __sigil_map_remove(__map, __key))",
                    self.js_all(&generated_args)
                )))
            }
            "singleton" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __value]) => __sigil_map_insert(__sigil_map_empty(), __key, __value))",
                    self.js_all(&generated_args)
                )))
            }
            "size" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __map.__sigil_map.length)",
                    generated_args[0]
                )))
            }
            "values" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __sigil_map_entries(__map).map(([_, __value]) => __value))",
                    generated_args[0]
                )))
            }
            _ => Ok(None),
        }
    }

    fn generate_json_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "asArray" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonArray' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asBool" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonBool' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asNumber" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonNumber' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asObject" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonObject' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asString" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonString' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "getField" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __obj]) => __sigil_map_get(__obj, __key))",
                self.js_all(&generated_args)
            ))),
            "getIndex" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__idx, __arr]) => (__idx >= 0 && __idx < __arr.length) ? {{ __tag: \"Some\", __fields: [__arr[__idx]] }} : {{ __tag: \"None\", __fields: [] }})",
                self.js_all(&generated_args)
            ))),
            "isNull" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonNull')",
                generated_args[0]
            ))),
            "parse" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_json_parse_result(__input))",
                generated_args[0]
            ))),
            "stringify" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __sigil_json_stringify_value(__value))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_time_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "compare" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => (__left.epochMillis < __right.epochMillis ? -1 : (__left.epochMillis > __right.epochMillis ? 1 : 0)))",
                self.js_all(&generated_args)
            ))),
            "formatIso" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__instant) => __sigil_time_format_iso(__instant))",
                generated_args[0]
            ))),
            "fromEpochMillis" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__millis) => ({{ epochMillis: __millis }}))",
                generated_args[0]
            ))),
            "isAfter" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => __left.epochMillis > __right.epochMillis)",
                self.js_all(&generated_args)
            ))),
            "isBefore" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => __left.epochMillis < __right.epochMillis)",
                self.js_all(&generated_args)
            ))),
            "now" if generated_args.is_empty() => Ok(Some("__sigil_ready(__sigil_time_now_instant())".to_string())),
            "parseIso" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_time_parse_iso_result(__input))",
                generated_args[0]
            ))),
            "sleepMs" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__ms) => new Promise((resolve) => setTimeout(() => resolve(null), Math.max(0, Number(__ms)))))",
                generated_args[0]
            ))),
            "toEpochMillis" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__instant) => __instant.epochMillis)",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_process_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "argv" if generated_args.is_empty() => Ok(Some("__sigil_process_argv()".to_string())),
            "kill" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__process) => __sigil_process_kill(__process))",
                generated_args[0]
            ))),
            "run" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__command) => __sigil_process_run(__command))",
                generated_args[0]
            ))),
            "exit" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__code) => __sigil_process_exit(__code))",
                generated_args[0]
            ))),
            "start" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__command) => __sigil_process_spawn(__command))",
                generated_args[0]
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__process) => __sigil_process_wait(__process))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_http_client_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "request" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__request) => __sigil_http_request(__request))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_regex_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "compile" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__flags, __pattern]) => __sigil_regex_compile_result(__flags, __pattern))",
                self.js_all(&generated_args)
            ))),
            "find" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __regex]) => __sigil_regex_find(__regex, __input))",
                self.js_all(&generated_args)
            ))),
            "isMatch" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __regex]) => __sigil_regex_is_match(__regex, __input))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_http_server_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "serve" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handler, __port]) => __sigil_http_serve(__handler, __port))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_tcp_client_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "request" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__request) => __sigil_tcp_request(__request))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_tcp_server_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "serve" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handler, __port]) => __sigil_tcp_serve(__handler, __port))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_url_intrinsic(
        &mut self,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "get_query" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __url]) => __sigil_map_get(__url.query, __key))",
                self.js_all(&generated_args)
            ))),
            "has_query" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __url]) => __sigil_map_has(__url.query, __key))",
                self.js_all(&generated_args)
            ))),
            "is_absolute" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__url) => __url.protocol.length > 0)",
                generated_args[0]
            ))),
            "is_anchor" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__url) => __url.path.length === 0 && __url.fragment.length > 0)",
                generated_args[0]
            ))),
            "parse" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_url_parse_result(__input))",
                generated_args[0]
            ))),
            "suffix" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__url) => __url.query_string + __url.fragment)",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_constructor_call(
        &mut self,
        call: &TypedConstructorCallExpr,
    ) -> Result<String, CodegenError> {
        let func = match &call.module_path {
            Some(module_path) => {
                let namespace = module_path
                    .iter()
                    .cloned()
                    .collect::<Vec<String>>()
                    .join("_");
                format!(
                    "{}.{}",
                    sanitize_js_identifier(&namespace),
                    sanitize_js_identifier(&call.constructor)
                )
            }
            None => sanitize_js_identifier(&call.constructor),
        };
        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;
        let mut values = vec![self.js_ready(&func)];
        values.extend(args);
        Ok(format!(
            "{}.then(([__sigil_fn, ...__sigil_args]) => __sigil_fn(...__sigil_args))",
            self.js_all(&values)
        ))
    }

    fn generate_extern_call(&mut self, call: &TypedExternCallExpr) -> Result<String, CodegenError> {
        if call.namespace.join("/") == "stdlib/string" {
            if let Some(intrinsic) = self.generate_string_intrinsic(&call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/json" {
            if let Some(intrinsic) = self.generate_json_intrinsic(&call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/time" {
            if let Some(intrinsic) = self.generate_time_intrinsic(&call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/process" {
            if let Some(intrinsic) = self.generate_process_intrinsic(&call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/regex" {
            if let Some(intrinsic) = self.generate_regex_intrinsic(&call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/url" {
            if let Some(intrinsic) = self.generate_url_intrinsic(&call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }

        let func_ref = format!(
            "{}.{}",
            sanitize_js_identifier(&call.namespace.join("_")),
            sanitize_js_identifier(&call.member)
        );
        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;
        Ok(format!(
            "{}.then((__sigil_args) => __sigil_call(\"{}\", {}, __sigil_args))",
            self.js_all(&args),
            call.mock_key,
            func_ref
        ))
    }

    fn generate_method_call(&mut self, call: &TypedMethodCallExpr) -> Result<String, CodegenError> {
        let receiver = self.generate_expression(&call.receiver)?;
        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;

        match &call.selector {
            MethodSelector::Field(field) => {
                let mut values = vec![receiver];
                values.extend(args);
                Ok(format!(
                    "{}.then(([__sigil_object, ...__sigil_args]) => __sigil_object.{}.call(__sigil_object, ...__sigil_args))",
                    self.js_all(&values),
                    field
                ))
            }
            MethodSelector::Index(index) => {
                let index = self.generate_expression(index)?;
                let mut values = vec![receiver, index];
                values.extend(args);
                Ok(format!(
                    "{}.then(([__sigil_object, __sigil_index, ...__sigil_args]) => __sigil_object[__sigil_index].call(__sigil_object, ...__sigil_args))",
                    self.js_all(&values)
                ))
            }
        }
    }

    fn generate_binary(&mut self, bin: &TypedBinaryExpr) -> Result<String, CodegenError> {
        let left = self.generate_expression(&bin.left)?;
        let right = self.generate_expression(&bin.right)?;

        let op = match bin.operator {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Power => "**",
            BinaryOperator::Equal => "",
            BinaryOperator::NotEqual => "",
            BinaryOperator::Less => "<",
            BinaryOperator::Greater => ">",
            BinaryOperator::LessEq => "<=",
            BinaryOperator::GreaterEq => ">=",
            BinaryOperator::And => "&&",
            BinaryOperator::Or => "||",
            BinaryOperator::Append => "+", // String concatenation
            BinaryOperator::ListAppend => ".concat", // Will need special handling
            BinaryOperator::Pipe => {
                // Pipeline operator - right(left)
                return Ok(format!(
                    "{}.then(([__sigil_fn, __sigil_value]) => __sigil_fn(__sigil_value))",
                    self.js_all(&[right, left])
                ));
            }
            BinaryOperator::ComposeFwd | BinaryOperator::ComposeBwd => {
                // Function composition - defer to helper
                return Err(CodegenError::General(
                    "Function composition not yet implemented".to_string(),
                ));
            }
        };

        match bin.operator {
            BinaryOperator::Equal => Ok(format!(
                "{}.then(([__left, __right]) => __sigil_deep_equal(__left, __right))",
                self.js_all(&[left, right])
            )),
            BinaryOperator::NotEqual => Ok(format!(
                "{}.then(([__left, __right]) => !__sigil_deep_equal(__left, __right))",
                self.js_all(&[left, right])
            )),
            BinaryOperator::ListAppend => Ok(format!(
                "{}.then(([__left, __right]) => __left.concat(__right))",
                self.js_all(&[left, right])
            )),
            BinaryOperator::And => Ok(format!(
                "{}.then((__left) => __left ? {}.then((__right) => (__left && __right)) : false)",
                left, right
            )),
            BinaryOperator::Or => Ok(format!(
                "{}.then((__left) => __left ? true : {}.then((__right) => (__left || __right)))",
                left, right
            )),
            _ => Ok(format!(
                "{}.then(([__left, __right]) => (__left {} __right))",
                self.js_all(&[left, right]),
                op
            )),
        }
    }

    fn generate_unary(&mut self, un: &TypedUnaryExpr) -> Result<String, CodegenError> {
        let operand = self.generate_expression(&un.operand)?;

        match un.operator {
            UnaryOperator::Negate => Ok(format!("{}.then((__value) => (-__value))", operand)),
            UnaryOperator::Not => Ok(format!("{}.then((__value) => (!__value))", operand)),
            UnaryOperator::Length => Ok(format!("{}.then((__value) => (__sigil_is_map(__value) ? __value.__sigil_map.length : __value.length))", operand)),
        }
    }

    fn generate_if(&mut self, if_expr: &TypedIfExpr) -> Result<String, CodegenError> {
        let condition = self.generate_expression(&if_expr.condition)?;
        let then_branch = self.generate_expression(&if_expr.then_branch)?;

        if let Some(ref else_branch) = if_expr.else_branch {
            let else_code = self.generate_expression(else_branch)?;
            Ok(format!(
                "{}.then((__cond) => (__cond ? {} : {}))",
                condition, then_branch, else_code
            ))
        } else {
            // No else branch - return null for the false case
            Ok(format!(
                "{}.then((__cond) => (__cond ? {} : __sigil_ready(null)))",
                condition, then_branch
            ))
        }
    }

    fn generate_list(&mut self, list: &TypedListExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = list
            .elements
            .iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        let elements = elements?;
        Ok(format!(
            "{}.then((__items) => __items)",
            self.js_all(&elements)
        ))
    }

    fn generate_tuple(&mut self, tuple: &TypedTupleExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = tuple
            .elements
            .iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        let elements = elements?;
        Ok(format!(
            "{}.then((__items) => __items)",
            self.js_all(&elements)
        ))
    }

    fn generate_record(&mut self, record: &TypedRecordExpr) -> Result<String, CodegenError> {
        let field_names: Vec<String> = record
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect();
        let values: Vec<String> = record
            .fields
            .iter()
            .map(|field| self.generate_expression(&field.value))
            .collect::<Result<_, _>>()?;

        let assignments: Result<Vec<String>, CodegenError> = field_names
            .iter()
            .enumerate()
            .map(|(index, field_name)| {
                let quoted_name = serde_json::to_string(field_name).map_err(|e| {
                    CodegenError::General(format!("Failed to JSON-encode field name: {}", e))
                })?;
                Ok(format!("{}: __values[{}]", quoted_name, index))
            })
            .collect();

        Ok(format!(
            "{}.then((__values) => ({{ {} }}))",
            self.js_all(&values),
            assignments?.join(", ")
        ))
    }

    fn generate_map_literal(&mut self, map: &TypedMapLiteralExpr) -> Result<String, CodegenError> {
        let entries = map
            .entries
            .iter()
            .map(|entry| {
                let key = self.generate_expression(&entry.key)?;
                let value = self.generate_expression(&entry.value)?;
                Ok(format!(
                    "{}.then(([__sigil_key, __sigil_value]) => [__sigil_key, __sigil_value])",
                    self.js_all(&[key, value])
                ))
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;

        Ok(format!(
            "{}.then((__entries) => __sigil_map_from_entries(__entries))",
            self.js_all(&entries)
        ))
    }

    fn generate_field_access(
        &mut self,
        field_access: &TypedFieldAccessExpr,
    ) -> Result<String, CodegenError> {
        let object = self.generate_expression(&field_access.object)?;
        Ok(format!(
            "{}.then((__value) => __value.{} )",
            object, field_access.field
        ))
    }

    fn generate_index(&mut self, index: &TypedIndexExpr) -> Result<String, CodegenError> {
        let object = self.generate_expression(&index.object)?;
        let idx = self.generate_expression(&index.index)?;
        Ok(format!(
            "{}.then(([__value, __index]) => __value[__index])",
            self.js_all(&[object, idx])
        ))
    }

    fn generate_let(&mut self, let_expr: &TypedLetExpr) -> Result<String, CodegenError> {
        // Generate async IIFE for let binding
        let value = self.generate_expression(&let_expr.value)?;
        let body = self.generate_expression(&let_expr.body)?;
        let bindings = self.generate_pattern_bindings(&let_expr.pattern, "__let_value")?;

        let mut lines = Vec::new();
        lines.push("(async () => {".to_string());
        lines.push(format!("  const __let_value = await {};", value));
        if let Some(binding) = bindings {
            lines.push(format!("  {}", binding));
        }
        lines.push(format!("  return {};", body));
        lines.push("})()".to_string());

        Ok(lines.join("\n"))
    }

    fn generate_match(&mut self, match_expr: &TypedMatchExpr) -> Result<String, CodegenError> {
        // Generate an async IIFE that implements pattern matching
        let scrutinee = self.generate_expression(&match_expr.scrutinee)?;

        let mut lines = Vec::new();
        lines.push("(async () => {".to_string());
        lines.push(format!("  const __match = await {};", scrutinee));

        for arm in &match_expr.arms {
            let condition = self.generate_pattern_condition(&arm.pattern, "__match")?;
            let body = self.generate_expression(&arm.body)?;
            let bindings = self.generate_pattern_bindings(&arm.pattern, "__match")?;

            lines.push(format!("  if ({}) {{", condition));

            if let Some(binding) = bindings {
                lines.push(format!("    {}", binding));
            }

            // Add guard check if present
            if let Some(ref guard) = arm.guard {
                let guard_expr = self.generate_expression(guard)?;
                lines.push(format!("    if (await {}) {{", guard_expr));
                lines.push(format!("      return {};", body));
                lines.push("    }".to_string());
            } else {
                lines.push(format!("    return {};", body));
            }

            lines.push("  }".to_string());
        }

        lines.push("  throw new Error('Match failed: no pattern matched');".to_string());
        lines.push("})()".to_string());

        Ok(lines.join("\n"))
    }

    fn generate_pattern_condition(
        &mut self,
        pattern: &Pattern,
        scrutinee: &str,
    ) -> Result<String, CodegenError> {
        match pattern {
            Pattern::Literal(lit) => {
                let value = match &lit.value {
                    PatternLiteralValue::Int(n) => n.to_string(),
                    PatternLiteralValue::Float(f) => f.to_string(),
                    PatternLiteralValue::String(s) => {
                        // Use JSON encoding to properly escape all special characters
                        serde_json::to_string(s).unwrap()
                    }
                    PatternLiteralValue::Char(c) => {
                        // Chars are also strings in JavaScript, use JSON encoding
                        serde_json::to_string(&c.to_string()).unwrap()
                    }
                    PatternLiteralValue::Bool(b) => b.to_string(),
                    PatternLiteralValue::Unit => "null".to_string(),
                };
                Ok(format!("{} === {}", scrutinee, value))
            }
            Pattern::Identifier(_) => Ok("true".to_string()),
            Pattern::Wildcard(_) => Ok("true".to_string()),
            Pattern::Constructor(ctor) => {
                // Check constructor tag
                Ok(format!("{}?.__tag === \"{}\"", scrutinee, ctor.name))
            }
            Pattern::List(list) => {
                if list.patterns.is_empty() {
                    Ok(format!("{}.length === 0", scrutinee))
                } else {
                    Ok(format!("{}.length >= {}", scrutinee, list.patterns.len()))
                }
            }
            Pattern::Tuple(tuple) => {
                let length_check = format!(
                    "Array.isArray({}) && {}.length === {}",
                    scrutinee,
                    scrutinee,
                    tuple.patterns.len()
                );
                // For now, just check length - could add element checks
                Ok(length_check)
            }
            Pattern::Record(_) => Ok("true".to_string()),
        }
    }

    fn generate_pattern_bindings(
        &mut self,
        pattern: &Pattern,
        scrutinee: &str,
    ) -> Result<Option<String>, CodegenError> {
        match pattern {
            Pattern::Identifier(id) => Ok(Some(format!(
                "const {} = {};",
                sanitize_js_identifier(&id.name),
                scrutinee
            ))),
            Pattern::Constructor(ctor) => {
                if ctor.patterns.is_empty() {
                    return Ok(None);
                }

                let mut bindings = Vec::new();
                for (i, p) in ctor.patterns.iter().enumerate() {
                    if let Some(b) = self
                        .generate_pattern_bindings(p, &format!("{}.__fields[{}]", scrutinee, i))?
                    {
                        bindings.push(b);
                    }
                }

                if bindings.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bindings.join(" ")))
                }
            }
            Pattern::List(list) => {
                let mut bindings = Vec::new();

                for (i, p) in list.patterns.iter().enumerate() {
                    if let Some(b) =
                        self.generate_pattern_bindings(p, &format!("{}[{}]", scrutinee, i))?
                    {
                        bindings.push(b);
                    }
                }

                if let Some(ref rest) = list.rest {
                    bindings.push(format!(
                        "const {} = {}.slice({});",
                        sanitize_js_identifier(rest),
                        scrutinee,
                        list.patterns.len()
                    ));
                }

                if bindings.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bindings.join(" ")))
                }
            }
            Pattern::Tuple(tuple) => {
                let mut bindings = Vec::new();
                for (i, p) in tuple.patterns.iter().enumerate() {
                    if let Some(b) =
                        self.generate_pattern_bindings(p, &format!("{}[{}]", scrutinee, i))?
                    {
                        bindings.push(b);
                    }
                }

                if bindings.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bindings.join(" ")))
                }
            }
            _ => Ok(None),
        }
    }

    fn generate_map(&mut self, map: &TypedMapExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&map.list)?;
        let func = self.generate_expression(&map.func)?;
        Ok(format!(
            "{}.then(([__items, __fn]) => __sigil_map_list(__items, __fn))",
            self.js_all(&[list, func])
        ))
    }

    fn generate_filter(&mut self, filter: &TypedFilterExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&filter.list)?;
        let predicate = self.generate_expression(&filter.predicate)?;
        Ok(format!(
            "{}.then(([__items, __predicate]) => __sigil_filter_list(__items, __predicate))",
            self.js_all(&[list, predicate])
        ))
    }

    fn generate_fold(&mut self, fold: &TypedFoldExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&fold.list)?;
        let func = self.generate_expression(&fold.func)?;
        let init = self.generate_expression(&fold.init)?;
        // Inline fold expansion keeps generated output deterministic
        Ok(format!(
            "{}.then(([__items, __fn, __init]) => __items.reduce((__acc, x) => __acc.then((acc) => __fn(acc, x)), Promise.resolve(__init)))",
            self.js_all(&[list, func, init])
        ))
    }

    fn generate_concurrent(
        &mut self,
        concurrent: &TypedConcurrentExpr,
    ) -> Result<String, CodegenError> {
        let width = self.generate_expression(&concurrent.config.width)?;
        let jitter_ms = concurrent
            .config
            .jitter_ms
            .as_ref()
            .map(|expr| self.generate_expression(expr))
            .transpose()?
            .unwrap_or_else(|| self.js_ready("{ __tag: \"None\", __fields: [] }"));
        let stop_on = concurrent
            .config
            .stop_on
            .as_ref()
            .map(|expr| self.generate_expression(expr))
            .transpose()?
            .unwrap_or_else(|| self.js_ready("(__sigil_error) => false"));
        let window_ms = concurrent
            .config
            .window_ms
            .as_ref()
            .map(|expr| self.generate_expression(expr))
            .transpose()?
            .unwrap_or_else(|| self.js_ready("{ __tag: \"None\", __fields: [] }"));

        let mut body_lines = Vec::new();
        body_lines.push("const __sigil_tasks = [];".to_string());

        for (index, step) in concurrent.steps.iter().enumerate() {
            match step {
                TypedConcurrentStep::Spawn(spawn) => {
                    let expr = self.generate_expression(&spawn.expr)?;
                    body_lines.push(format!("__sigil_tasks.push(() => {});", expr));
                }
                TypedConcurrentStep::SpawnEach(spawn_each) => {
                    let list = self.generate_expression(&spawn_each.list)?;
                    let func = self.generate_expression(&spawn_each.func)?;
                    body_lines.push(format!(
                        "const [__sigil_items_{index}, __sigil_fn_{index}] = await {};",
                        self.js_all(&[list, func])
                    ));
                    body_lines.push(format!(
                        "for (const __sigil_item_{index} of __sigil_items_{index}) {{"
                    ));
                    body_lines.push(format!(
                        "  __sigil_tasks.push(() => __sigil_fn_{index}(__sigil_item_{index}));"
                    ));
                    body_lines.push("}".to_string());
                }
            }
        }

        let body = body_lines
            .into_iter()
            .map(|line| format!("    {}", line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(format!(
            "(async () => {{\n  const [__sigil_concurrency, __sigil_jitterMs, __sigil_stopOn, __sigil_windowMs] = await {};\n{}\n  return __sigil_concurrent_region({}, {{ concurrency: __sigil_concurrency, jitterMs: __sigil_jitterMs, stopOn: __sigil_stopOn, windowMs: __sigil_windowMs }}, __sigil_tasks);\n}})()",
            self.js_all(&[width, jitter_ms, stop_on, window_ms]),
            body,
            serde_json::to_string(&concurrent.name).unwrap()
        ))
    }

    fn generate_pipeline(&mut self, pipeline: &TypedPipelineExpr) -> Result<String, CodegenError> {
        let left = self.generate_expression(&pipeline.left)?;
        let right = self.generate_expression(&pipeline.right)?;

        match pipeline.operator {
            PipelineOperator::Pipe => {
                // a |> f becomes f(a) without eager await
                Ok(format!(
                    "{}.then(([__sigil_value, __sigil_fn]) => __sigil_fn(__sigil_value))",
                    self.js_all(&[left, right])
                ))
            }
            PipelineOperator::ComposeFwd | PipelineOperator::ComposeBwd => Err(
                CodegenError::General("Function composition not yet implemented".to_string()),
            ),
        }
    }

    fn generate_with_mock(
        &mut self,
        with_mock: &TypedWithMockExpr,
    ) -> Result<String, CodegenError> {
        let replacement = self.generate_expression(&with_mock.replacement)?;
        let body = self.generate_expression(&with_mock.body)?;
        match &with_mock.target {
            WithMockTarget::LocalFunction(name) => Ok(format!(
                "__sigil_with_mock('{}', {}, async () => {})",
                name, replacement, body
            )),
            WithMockTarget::ExternMember {
                namespace,
                member,
                mock_key,
            } => {
                let func_ref = format!(
                    "{}.{}",
                    sanitize_js_identifier(&namespace.join("_")),
                    sanitize_js_identifier(member)
                );
                Ok(format!(
                    "__sigil_with_mock_extern('{}', {}, {}, async () => {})",
                    mock_key, func_ref, replacement, body
                ))
            }
        }
    }
}

fn sanitize_js_identifier(raw: &str) -> String {
    let mut sanitized = String::with_capacity(raw.len());

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    let sanitized = if sanitized.is_empty() {
        "_".to_string()
    } else if sanitized.chars().next().unwrap().is_ascii_digit() {
        format!("_{}", sanitized)
    } else {
        sanitized
    };

    if is_reserved_js_identifier(&sanitized) {
        format!("_{}", sanitized)
    } else {
        sanitized
    }
}

fn is_reserved_js_identifier(name: &str) -> bool {
    matches!(
        name,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn find_output_root(output_path: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();

    for component in output_path.components() {
        root.push(component.as_os_str());
        if matches!(component, Component::Normal(name) if name == ".local") {
            return Some(root);
        }
    }

    None
}

fn relative_import_path(from_dir: &Path, target_file: &Path) -> String {
    let from_components: Vec<_> = from_dir.components().collect();
    let target_components: Vec<_> = target_file.components().collect();
    let common_len = from_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();

    for _ in common_len..from_components.len() {
        relative.push("..");
    }

    for component in &target_components[common_len..] {
        relative.push(component.as_os_str());
    }

    let relative_str = relative.to_string_lossy().replace('\\', "/");
    if relative_str.starts_with("../") {
        relative_str
    } else {
        format!("./{}", relative_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;
    use sigil_typechecker::type_check;

    fn typed_program_for(source: &str, path: &str) -> TypedProgram {
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, path).unwrap();
        type_check(&program, source, None).unwrap().typed_program
    }

    #[test]
    fn test_empty_program() {
        let program = TypedProgram {
            declarations: vec![],
        };

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_function() {
        let source = "λadd(x:Int,y:Int)=>Int=x+y";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        // Should contain a plain function that returns promise-shaped values
        assert!(result.contains("function add"));
        assert!(!result.contains("async function add"));
        // Should contain return statement
        assert!(result.contains("return"));
        // Should contain parameters
        assert!(result.contains("x, y"));
    }

    #[test]
    fn test_sum_type_constructors() {
        let source = "t Color=Red|Green|Blue";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        // Should contain constructor functions without eager async wrappers
        assert!(result.contains("function Red"));
        assert!(result.contains("function Green"));
        assert!(result.contains("function Blue"));
        assert!(!result.contains("async function Red"));
        // Should use __tag pattern
        assert!(result.contains("__tag"));
    }

    #[test]
    fn test_core_prelude_result_helper_codegen() {
        let source = "t Result[T,E]=Ok(T)|Err(E)\nλnormalize[T,E](res:Result[T,E])=>Result[T,E] match res{Ok(value)=>Ok(value)|Err(error)=>Err(error)}";
        let program = typed_program_for(source, "test.lib.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function normalize"));
        assert!(result.contains("__tag"));
    }

    #[test]
    fn test_regular_function_calls_route_through_mock_runtime() {
        let source = "λping()=>String=\"real\"\nλmain()=>String=ping()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_call(key, actualFn, args = [])"));
        assert!(result.contains("__sigil_call(\"ping\", ping, __sigil_args)"));
    }

    #[test]
    fn test_generate_import_sanitizes_alias_and_uses_relative_path() {
        let source = "i src::rot13Encoder\nλmain()=>Unit=()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("projects/algorithms/tests/rot13Encoder.sigil".to_string()),
            output_file: Some("/tmp/projects/algorithms/.local/tests/rot13Encoder.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as src_rot13Encoder from '../src/rot13Encoder.js';"));
    }

    #[test]
    fn test_generate_import_uses_local_root_for_stdlib_test_outputs() {
        let source = "i stdlib::numeric\nλmain()=>Unit=()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("language/stdlib-tests/tests/numericPredicates.sigil".to_string()),
            output_file: Some(
                "/tmp/language/stdlib-tests/.local/tests/numericPredicates.ts".to_string(),
            ),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as stdlib_numeric from '../stdlib/numeric.js';"));
    }

    #[test]
    fn test_generate_extern_namespace_uses_full_sanitized_alias() {
        let source = "e fs::promises\nλmain()=>Unit=()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as fs_promises from 'fs/promises';"));
    }

    #[test]
    fn test_generate_match_with_guard_falls_through_to_later_arms() {
        let source =
            "λclassify(x:Int)=>String match x{n when n>1=>\"big\"|0=>\"zero\"|_=>\"other\"}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("if (__match === 0)"));
        assert!(!result.contains("else if (__match === 0)"));
    }

    #[test]
    fn test_generate_list_preserves_nested_lists() {
        let source = "λwrap(xs:[Int])=>[[Int]]=[xs]";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(".then((__items) => __items)"));
        assert!(!result.contains("[].concat(xs)"));
    }

    #[test]
    fn test_generate_list_append_parenthesizes_awaited_left_side() {
        let source = "λleft()=>[Int]=[1]\nλright()=>[Int]=[2]\nλmain()=>[Int]=left()⧺right()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(".concat("));
        assert!(!result.contains("await left().concat("));
    }

    #[test]
    fn test_generate_qualified_constructor_call_without_mock_wrapper() {
        let source = "i src::graphTypes\nλmain()=>Unit=src::graphTypes.Ordering([])";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("projects/algorithms/src/topologicalSort.sigil".to_string()),
            output_file: Some("/tmp/projects/algorithms/.local/src/topologicalSort.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("src_graphTypes.Ordering"));
        assert!(!result.contains("__sigil_call(\"extern:src/graphTypes.Ordering\""));
    }

    #[test]
    fn test_generate_test_metadata_includes_id_and_location() {
        let source = "λmain()=>Unit=()\n\ntest \"smoke\" { true }";
        let program = typed_program_for(source, "tests/smoke.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("tests/smoke.sigil".to_string()),
            output_file: Some("/tmp/tests/smoke.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("id: 'tests/smoke.sigil::smoke'"));
        assert!(result.contains("location: { start: { line: 3, column: 1 } }"));
    }

    #[test]
    fn test_generate_map_uses_ordered_helper_not_promise_all_map() {
        let source = "λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_map_list"));
        assert!(!result.contains("Promise.all(__items.map"));
    }

    #[test]
    fn test_generate_concurrent_region_uses_scheduler_helper() {
        let source = "e clock:{tick:λ()=>!Timer Unit}\nt ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)\nt Option[T]=Some(T)|None()\nt Result[T,E]=Ok(T)|Err(E)\nλmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@2{spawnEach [1,2] process}\nλprocess(value:Int)=>!Timer Result[Int,String]={l _=(clock.tick():Unit);Ok(value)}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_concurrent_region(\"urlAudit\""));
        assert!(result.contains("__sigil_tasks.push(() => __sigil_fn_0(__sigil_item_0));"));
    }
}
