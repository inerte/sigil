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

use sigil_ast::*;
use std::collections::HashSet;
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
    mockable_functions: HashSet<String>,
}

impl TypeScriptGenerator {
    pub fn new(options: CodegenOptions) -> Self {
        Self {
            indent: 0,
            output: Vec::new(),
            source_file: options.source_file,
            output_file: options.output_file,
            test_meta_entries: Vec::new(),
            mockable_functions: HashSet::new(),
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

    pub fn generate(&mut self, program: &Program) -> Result<String, CodegenError> {
        self.output.clear();
        self.indent = 0;
        self.test_meta_entries.clear();
        self.mockable_functions.clear();

        // Collect mockable functions
        for decl in &program.declarations {
            if let Declaration::Function(func) = decl {
                if func.is_mockable {
                    self.mockable_functions.insert(func.name.clone());
                }
            }
        }

        // Emit mock runtime helpers first
        self.emit_mock_runtime_helpers();

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

    fn emit(&mut self, line: &str) {
        let indentation = "  ".repeat(self.indent);
        self.output.push(format!("{}{}\n", indentation, line));
    }

    fn js_ready(&self, expr: &str) -> String {
        format!("__sigil_ready({})", expr)
    }

    fn js_all(&self, exprs: &[String]) -> String {
        format!("__sigil_all([{}])", exprs.join(", "))
    }

    fn emit_mock_runtime_helpers(&mut self) {
        self.emit("const __sigil_mocks = new Map();");
        self.emit("function __sigil_ready(value) {");
        self.emit("  return Promise.resolve(value);");
        self.emit("}");
        self.emit("function __sigil_all(values) {");
        self.emit("  return Promise.all(values.map((value) => Promise.resolve(value)));");
        self.emit("}");
        self.emit("function __sigil_deep_equal(a, b) {");
        self.emit("  if (a === b) return true;");
        self.emit("  if (a == null || b == null) return false;");
        self.emit("  if (typeof a !== typeof b) return false;");
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
        self.emit("    default: throw new Error('Unsupported test comparison operator: ' + String(op));");
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
        self.emit("    if (had) { __sigil_mocks.set(key, prev); } else { __sigil_mocks.delete(key); }");
        self.emit("  }");
        self.emit("}");
        self.emit("async function __sigil_with_mock_extern(key, actualFn, mockFn, body) {");
        self.emit("  if (typeof actualFn !== 'function') { throw new Error('with_mock extern target is not callable'); }");
        self.emit("  if (typeof mockFn !== 'function') { throw new Error('with_mock replacement must be callable'); }");
        self.emit("  if (actualFn.length !== mockFn.length) { throw new Error(`with_mock extern arity mismatch for ${key}: expected ${actualFn.length}, got ${mockFn.length}`); }");
        self.emit("  return await __sigil_with_mock(key, mockFn, body);");
        self.emit("}");
    }

    fn generate_declaration(&mut self, decl: &Declaration) -> Result<(), CodegenError> {
        match decl {
            Declaration::Function(func) => self.generate_function(func),
            Declaration::Type(type_decl) => self.generate_type_decl(type_decl),
            Declaration::Const(const_decl) => self.generate_const(const_decl),
            Declaration::Import(import) => self.generate_import(import),
            Declaration::Extern(extern_decl) => self.generate_extern(extern_decl),
            Declaration::Test(test) => self.generate_test(test),
        }
    }

    fn generate_function(&mut self, func: &FunctionDecl) -> Result<(), CodegenError> {
        let params: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();
        let params_str = params.join(", ");

        let impl_name = if func.is_mockable {
            format!("__sigil_impl_{}", func.name)
        } else {
            func.name.clone()
        };

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

        self.emit(&format!("{} {}({}) {{", fn_keyword, impl_name, params_str));
        self.indent += 1;

        let body_code = self.generate_expression(&func.body)?;
        self.emit(&format!("return {};", body_code));

        self.indent -= 1;
        self.emit("}");

        // If mockable, emit wrapper
        if func.is_mockable {
            self.emit("");
            let export_keyword = if should_export { "export " } else { "" };
            self.emit(&format!("{}function {}({}) {{", export_keyword, func.name, params_str));
            self.indent += 1;
            let args = if params.is_empty() {
                String::new()
            } else {
                format!(", {}", params_str)
            };
            self.emit(&format!("return __sigil_call('{}', {}{});", func.name, impl_name, args));
            self.indent -= 1;
            self.emit("}");
        }

        Ok(())
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
                // Example: Some(x) → { __tag: "Some", __fields: [x] }
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

                self.emit(&format!("{} {}({}) {{", ctor_keyword, variant.name, params));
                self.indent += 1;
                if param_names.is_empty() {
                    self.emit(&format!(
                        "return __sigil_ready({{ __tag: \"{}\", __fields: [] }});",
                        variant.name
                    ));
                } else {
                    self.emit(&format!("return {}.then((__fields) => ({{ __tag: \"{}\", __fields }}));", self.js_all(&param_names), variant.name));
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

    fn generate_const(&mut self, const_decl: &ConstDecl) -> Result<(), CodegenError> {
        let value = self.generate_expression(&const_decl.value)?;
        // Export consts from .lib.sigil files
        let export_keyword = if self.should_export_from_lib() { "export " } else { "" };
        self.emit(&format!("{}const {} = {};", export_keyword, const_decl.name, value));
        Ok(())
    }

    fn generate_import(&mut self, import: &ImportDecl) -> Result<(), CodegenError> {
        // Convert Sigil import to ES module import (namespace style)
        // For src⋅utils, create:
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

        self.emit(&format!("import * as {} from '{}';", namespace, import_path));
        Ok(())
    }

    fn generate_extern(&mut self, extern_decl: &ExternDecl) -> Result<(), CodegenError> {
        // Extern declarations become ES module imports
        let module_path = extern_decl.module_path.join("/");

        if let Some(ref members) = extern_decl.members {
            let member_names: Vec<String> = members.iter().map(|m| m.name.clone()).collect();
            self.emit(&format!("import {{ {} }} from '{}';", member_names.join(", "), module_path));
        } else {
            // Import entire namespace
            let namespace = sanitize_js_identifier(&extern_decl.module_path.join("_"));
            self.emit(&format!("import * as {} from '{}';", namespace, module_path));
        }

        Ok(())
    }

    fn generate_test(&mut self, test: &TestDecl) -> Result<(), CodegenError> {
        // Generate a unique test name from the description
        let test_name = test.description
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
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

    fn generate_expression(&mut self, expr: &Expr) -> Result<String, CodegenError> {
        match expr {
            Expr::Literal(lit) => self.generate_literal(lit),
            Expr::Identifier(id) => Ok(self.js_ready(&id.name)),
            Expr::Lambda(lambda) => self.generate_lambda(lambda),
            Expr::Application(app) => self.generate_application(app),
            Expr::Binary(bin) => self.generate_binary(bin),
            Expr::Unary(un) => self.generate_unary(un),
            Expr::Match(match_expr) => self.generate_match(match_expr),
            Expr::Let(let_expr) => self.generate_let(let_expr),
            Expr::If(if_expr) => self.generate_if(if_expr),
            Expr::List(list) => self.generate_list(list),
            Expr::Tuple(tuple) => self.generate_tuple(tuple),
            Expr::Record(record) => self.generate_record(record),
            Expr::FieldAccess(field_access) => self.generate_field_access(field_access),
            Expr::Index(index) => self.generate_index(index),
            Expr::MemberAccess(member_access) => self.generate_member_access(member_access),
            Expr::Map(map) => self.generate_map(map),
            Expr::Filter(filter) => self.generate_filter(filter),
            Expr::Fold(fold) => self.generate_fold(fold),
            Expr::Pipeline(pipeline) => self.generate_pipeline(pipeline),
            Expr::WithMock(with_mock) => self.generate_with_mock(with_mock),
            // Type ascriptions are erased at runtime - emit only inner expression
            Expr::TypeAscription(t) => self.generate_expression(&t.expr),
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

    fn generate_lambda(&mut self, lambda: &LambdaExpr) -> Result<String, CodegenError> {
        let params: Vec<String> = lambda.params.iter().map(|p| p.name.clone()).collect();
        let params_str = params.join(", ");
        let body = self.generate_expression(&lambda.body)?;
        Ok(format!("(({}) => {})", params_str, body))
    }

    fn generate_application(&mut self, app: &ApplicationExpr) -> Result<String, CodegenError> {
        // Check for stdlib intrinsics first
        if let Expr::MemberAccess(member_access) = &app.func {
            if let Some(intrinsic) = self.try_generate_intrinsic(member_access, &app.args)? {
                return Ok(intrinsic);
            }
        }

        if let Expr::Identifier(name) = &app.func {
            if let Some(intrinsic) = self.try_generate_local_string_intrinsic(&name.name, &app.args)? {
                return Ok(intrinsic);
            }
        }

        if let Expr::MemberAccess(member_access) = &app.func {
            if member_access
                .member
                .chars()
                .next()
                .is_some_and(|ch| ch.is_uppercase())
            {
                let func = format!(
                    "{}.{}",
                    sanitize_js_identifier(&member_access.namespace.join("_")),
                    member_access.member
                );
                let args: Vec<String> = app.args.iter()
                    .map(|arg| self.generate_expression(arg))
                    .collect::<Result<_, _>>()?;
                let mut values = vec![func];
                values.extend(args);
                return Ok(format!(
                    "{}.then(([__sigil_fn, ...__sigil_args]) => __sigil_fn(...__sigil_args))",
                    self.js_all(&values)
                ));
            }
        }

        if let Expr::FieldAccess(field_access) = &app.func {
            let object = self.generate_expression(&field_access.object)?;
            let args: Vec<String> = app.args.iter()
                .map(|arg| self.generate_expression(arg))
                .collect::<Result<_, _>>()?;
            let mut values = vec![object];
            values.extend(args);
            return Ok(format!(
                "{}.then(([__sigil_object, ...__sigil_args]) => __sigil_object.{}.call(__sigil_object, ...__sigil_args))",
                self.js_all(&values),
                field_access.field
            ));
        }

        if let Expr::Index(index_expr) = &app.func {
            let object = self.generate_expression(&index_expr.object)?;
            let index = self.generate_expression(&index_expr.index)?;
            let args: Vec<String> = app.args.iter()
                .map(|arg| self.generate_expression(arg))
                .collect::<Result<_, _>>()?;
            let mut values = vec![object, index];
            values.extend(args);
            return Ok(format!(
                "{}.then(([__sigil_object, __sigil_index, ...__sigil_args]) => __sigil_object[__sigil_index].call(__sigil_object, ...__sigil_args))",
                self.js_all(&values)
            ));
        }

        // Check if this is a call to an imported function (MemberAccess)
        // If so, wrap with __sigil_call for mock support
        if let Expr::MemberAccess(member_access) = &app.func {
            // Generate the mock key: "extern:namespace/path.member"
            let mock_key = format!("extern:{}.{}",
                member_access.namespace.join("/"),
                member_access.member);

            // Generate the function reference
            let func_ref = format!(
                "{}.{}",
                sanitize_js_identifier(&member_access.namespace.join("_")),
                member_access.member
            );

            // Generate arguments
            let args: Vec<String> = app.args.iter()
                .map(|arg| self.generate_expression(arg))
                .collect::<Result<_, _>>()?;

            // Wrap in __sigil_call
            Ok(format!(
                "{}.then((__sigil_args) => __sigil_call(\"{}\", {}, __sigil_args))",
                self.js_all(&args),
                mock_key,
                func_ref
            ))
        } else {
            // Regular function call
            let func = self.generate_expression(&app.func)?;
            let args: Vec<String> = app.args.iter()
                .map(|arg| self.generate_expression(arg))
                .collect::<Result<_, _>>()?;
            let mut values = vec![func];
            values.extend(args);

            Ok(format!(
                "{}.then(([__sigil_fn, ...__sigil_args]) => __sigil_fn(...__sigil_args))",
                self.js_all(&values)
            ))
        }
    }

    fn try_generate_intrinsic(&mut self, func: &MemberAccessExpr, args: &[Expr]) -> Result<Option<String>, CodegenError> {
        let module = func.namespace.join("/");
        let member = &func.member;

        // String operations intrinsics for generated JavaScript output
        if module == "stdlib/string" {
            return self.generate_string_intrinsic(member, args);
        }

        Ok(None)
    }

    fn try_generate_local_string_intrinsic(&mut self, name: &str, args: &[Expr]) -> Result<Option<String>, CodegenError> {
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/string.lib.sigil"))
        {
            return self.generate_string_intrinsic(name, args);
        }

        Ok(None)
    }

    fn generate_string_intrinsic(&mut self, member: &str, args: &[Expr]) -> Result<Option<String>, CodegenError> {
        let generated_args: Result<Vec<String>, CodegenError> = args.iter()
            .map(|arg| self.generate_expression(arg))
            .collect();
        let generated_args = generated_args?;

        match member {
            "char_at" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__index, __string]) => __sigil_ready(__string.charAt(__index)))", self.js_all(&generated_args))))
            }
            "substring" if generated_args.len() == 3 => {
                Ok(Some(format!("{}.then(([__end, __string, __start]) => __sigil_ready(__string.substring(__start, __end)))", self.js_all(&generated_args))))
            }
            "to_upper" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.toUpperCase())", generated_args[0])))
            }
            "to_lower" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.toLowerCase())", generated_args[0])))
            }
            "trim" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.trim())", generated_args[0])))
            }
            "index_of" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __needle]) => __string.indexOf(__needle))", self.js_all(&generated_args))))
            }
            "split" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__separator, __string]) => __string.split(__separator))", self.js_all(&generated_args))))
            }
            "replace_all" if generated_args.len() == 3 => {
                Ok(Some(format!("{}.then(([__search, __replacement, __string]) => __string.replaceAll(__search, __replacement))", self.js_all(&generated_args))))
            }
            "int_to_string" if generated_args.len() == 1 => {
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
            "starts_with" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__prefix, __string]) => __string.startsWith(__prefix))", self.js_all(&generated_args))))
            }
            "ends_with" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __suffix]) => __string.endsWith(__suffix))", self.js_all(&generated_args))))
            }
            "is_digit" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => /^[0-9]$/.test(__value))", generated_args[0])))
            }
            _ => Ok(None),
        }
    }

    fn generate_binary(&mut self, bin: &BinaryExpr) -> Result<String, CodegenError> {
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
            BinaryOperator::Append => "+",  // String concatenation
            BinaryOperator::ListAppend => ".concat",  // Will need special handling
            BinaryOperator::Pipe => {
                // Pipeline operator - right(left)
                return Ok(format!(
                    "{}.then(([__sigil_fn, __sigil_value]) => __sigil_fn(__sigil_value))",
                    self.js_all(&[right, left])
                ));
            }
            BinaryOperator::ComposeFwd | BinaryOperator::ComposeBwd => {
                // Function composition - defer to helper
                return Err(CodegenError::General("Function composition not yet implemented".to_string()));
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

    fn generate_unary(&mut self, un: &UnaryExpr) -> Result<String, CodegenError> {
        let operand = self.generate_expression(&un.operand)?;

        match un.operator {
            UnaryOperator::Negate => Ok(format!("{}.then((__value) => (-__value))", operand)),
            UnaryOperator::Not => Ok(format!("{}.then((__value) => (!__value))", operand)),
            UnaryOperator::Length => Ok(format!("{}.then((__value) => __value.length)", operand)),
        }
    }

    fn generate_if(&mut self, if_expr: &IfExpr) -> Result<String, CodegenError> {
        let condition = self.generate_expression(&if_expr.condition)?;
        let then_branch = self.generate_expression(&if_expr.then_branch)?;

        if let Some(ref else_branch) = if_expr.else_branch {
            let else_code = self.generate_expression(else_branch)?;
            Ok(format!("{}.then((__cond) => (__cond ? {} : {}))", condition, then_branch, else_code))
        } else {
            // No else branch - return null for the false case
            Ok(format!("{}.then((__cond) => (__cond ? {} : __sigil_ready(null)))", condition, then_branch))
        }
    }

    fn generate_list(&mut self, list: &ListExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = list.elements.iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        let elements = elements?;
        Ok(format!("{}.then((__items) => __items)", self.js_all(&elements)))
    }

    fn generate_tuple(&mut self, tuple: &TupleExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = tuple.elements.iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        let elements = elements?;
        Ok(format!("{}.then((__items) => __items)", self.js_all(&elements)))
    }

    fn generate_record(&mut self, record: &RecordExpr) -> Result<String, CodegenError> {
        let field_names: Vec<String> = record.fields.iter().map(|field| field.name.clone()).collect();
        let values: Vec<String> = record.fields.iter()
            .map(|field| self.generate_expression(&field.value))
            .collect::<Result<_, _>>()?;

        let assignments: Result<Vec<String>, CodegenError> = field_names
            .iter()
            .enumerate()
            .map(|(index, field_name)| {
                let quoted_name = serde_json::to_string(field_name)
                    .map_err(|e| CodegenError::General(format!("Failed to JSON-encode field name: {}", e)))?;
                Ok(format!("{}: __values[{}]", quoted_name, index))
            })
            .collect();

        Ok(format!(
            "{}.then((__values) => ({{ {} }}))",
            self.js_all(&values),
            assignments?.join(", ")
        ))
    }

    fn generate_field_access(&mut self, field_access: &FieldAccessExpr) -> Result<String, CodegenError> {
        let object = self.generate_expression(&field_access.object)?;
        Ok(format!(
            "{}.then((__value) => __value.{} )",
            object,
            field_access.field
        ))
    }

    fn generate_index(&mut self, index: &IndexExpr) -> Result<String, CodegenError> {
        let object = self.generate_expression(&index.object)?;
        let idx = self.generate_expression(&index.index)?;
        Ok(format!(
            "{}.then(([__value, __index]) => __value[__index])",
            self.js_all(&[object, idx])
        ))
    }

    fn generate_member_access(&mut self, member_access: &MemberAccessExpr) -> Result<String, CodegenError> {
        // Convert namespace⋅path to namespace_path.member
        let namespace = sanitize_js_identifier(&member_access.namespace.join("_"));
        Ok(self.js_ready(&format!("{}.{}", namespace, member_access.member)))
    }

    fn generate_let(&mut self, let_expr: &LetExpr) -> Result<String, CodegenError> {
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

    fn generate_match(&mut self, match_expr: &MatchExpr) -> Result<String, CodegenError> {
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

    fn generate_pattern_condition(&mut self, pattern: &Pattern, scrutinee: &str) -> Result<String, CodegenError> {
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
                let length_check = format!("Array.isArray({}) && {}.length === {}",
                    scrutinee, scrutinee, tuple.patterns.len());
                // For now, just check length - could add element checks
                Ok(length_check)
            }
            Pattern::Record(_) => Ok("true".to_string()),
        }
    }

    fn generate_pattern_bindings(&mut self, pattern: &Pattern, scrutinee: &str) -> Result<Option<String>, CodegenError> {
        match pattern {
            Pattern::Identifier(id) => {
                Ok(Some(format!("const {} = {};", id.name, scrutinee)))
            }
            Pattern::Constructor(ctor) => {
                if ctor.patterns.is_empty() {
                    return Ok(None);
                }

                let mut bindings = Vec::new();
                for (i, p) in ctor.patterns.iter().enumerate() {
                    if let Some(b) = self.generate_pattern_bindings(p, &format!("{}.__fields[{}]", scrutinee, i))? {
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
                    if let Some(b) = self.generate_pattern_bindings(p, &format!("{}[{}]", scrutinee, i))? {
                        bindings.push(b);
                    }
                }

                if let Some(ref rest) = list.rest {
                    bindings.push(format!("const {} = {}.slice({});", rest, scrutinee, list.patterns.len()));
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
                    if let Some(b) = self.generate_pattern_bindings(p, &format!("{}[{}]", scrutinee, i))? {
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

    fn generate_map(&mut self, map: &MapExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&map.list)?;
        let func = self.generate_expression(&map.func)?;
        Ok(format!(
            "{}.then(([__items, __fn]) => Promise.all(__items.map((x) => __fn(x))))",
            self.js_all(&[list, func])
        ))
    }

    fn generate_filter(&mut self, filter: &FilterExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&filter.list)?;
        let predicate = self.generate_expression(&filter.predicate)?;
        // Inline filter expansion keeps generated output deterministic
        Ok(format!(
            "{}.then(([__items, __predicate]) => Promise.all(__items.map((x) => Promise.resolve(__predicate(x)).then((keep) => ({{ x, keep }})))).then((__pairs) => __pairs.filter(({{ keep }}) => keep).map(({{ x }}) => x)))",
            self.js_all(&[list, predicate])
        ))
    }

    fn generate_fold(&mut self, fold: &FoldExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&fold.list)?;
        let func = self.generate_expression(&fold.func)?;
        let init = self.generate_expression(&fold.init)?;
        // Inline fold expansion keeps generated output deterministic
        Ok(format!(
            "{}.then(([__items, __fn, __init]) => __items.reduce((__acc, x) => __acc.then((acc) => __fn(acc, x)), Promise.resolve(__init)))",
            self.js_all(&[list, func, init])
        ))
    }

    fn generate_pipeline(&mut self, pipeline: &PipelineExpr) -> Result<String, CodegenError> {
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
            PipelineOperator::ComposeFwd | PipelineOperator::ComposeBwd => {
                Err(CodegenError::General("Function composition not yet implemented".to_string()))
            }
        }
    }

    fn generate_with_mock(&mut self, with_mock: &WithMockExpr) -> Result<String, CodegenError> {
        let replacement = self.generate_expression(&with_mock.replacement)?;
        let body = self.generate_expression(&with_mock.body)?;
        match &with_mock.target {
            Expr::Identifier(id) => Ok(format!(
                "__sigil_with_mock('{}', {}, async () => {})",
                id.name,
                replacement,
                body
            )),
            Expr::MemberAccess(member_access) => {
                let key = format!("extern:{}.{}", member_access.namespace.join("/"), member_access.member);
                let func_ref = format!(
                    "{}.{}",
                    sanitize_js_identifier(&member_access.namespace.join("_")),
                    member_access.member
                );
                Ok(format!(
                    "__sigil_with_mock_extern('{}', {}, {}, async () => {})",
                    key,
                    func_ref,
                    replacement,
                    body
                ))
            }
            _ => Err(CodegenError::General(
                "with_mock target must be an identifier or imported member access".to_string(),
            )),
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

    if sanitized.is_empty() {
        "_".to_string()
    } else if sanitized.chars().next().unwrap().is_ascii_digit() {
        format!("_{}", sanitized)
    } else {
        sanitized
    }
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

    #[test]
    fn test_empty_program() {
        let program = Program::new(
            vec![],
            sigil_lexer::SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
        );

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_function() {
        let source = "λadd(x:ℤ,y:ℤ)→ℤ=x+y";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

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
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

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
    fn test_mockable_zero_arg_wrapper_uses_default_args() {
        let source = "mockable λping()→𝕊=\"real\"";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_call(key, actualFn, args = [])"));
        assert!(result.contains("return __sigil_call('ping', __sigil_impl_ping);"));
    }

    #[test]
    fn test_generate_import_sanitizes_alias_and_uses_relative_path() {
        let source = "i src⋅rot13-encoder\nλmain()→𝕌=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("projects/algorithms/tests/rot13-encoder.sigil".to_string()),
            output_file: Some("/tmp/projects/algorithms/.local/tests/rot13-encoder.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as src_rot13_encoder from '../src/rot13-encoder.js';"));
    }

    #[test]
    fn test_generate_import_uses_local_root_for_stdlib_test_outputs() {
        let source = "i stdlib⋅numeric\nλmain()→𝕌=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("language/stdlib-tests/tests/numeric-predicates.sigil".to_string()),
            output_file: Some("/tmp/language/stdlib-tests/.local/tests/numeric-predicates.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as stdlib_numeric from '../stdlib/numeric.js';"));
    }

    #[test]
    fn test_generate_extern_namespace_uses_full_sanitized_alias() {
        let source = "e fs⋅promises\nλmain()→𝕌=()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as fs_promises from 'fs/promises';"));
    }

    #[test]
    fn test_generate_match_with_guard_falls_through_to_later_arms() {
        let source = "λclassify(x:ℤ)→𝕊 match x{n when n>1→\"big\"|0→\"zero\"|_→\"other\"}";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("if (__match === 0)"));
        assert!(!result.contains("else if (__match === 0)"));
    }

    #[test]
    fn test_generate_list_preserves_nested_lists() {
        let source = "λwrap(xs:[ℤ])→[[ℤ]]=[xs]";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(".then((__items) => __items)"));
        assert!(!result.contains("[].concat(xs)"));
    }

    #[test]
    fn test_generate_list_append_parenthesizes_awaited_left_side() {
        let source = "λleft()→[ℤ]=[1]\nλright()→[ℤ]=[2]\nλmain()→[ℤ]=left()⧺right()";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(".concat("));
        assert!(!result.contains("await left().concat("));
    }

    #[test]
    fn test_generate_qualified_constructor_call_without_mock_wrapper() {
        let source = "i src⋅graph-types\nλmain()→𝕌=src⋅graph-types.Ordering([])";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("projects/algorithms/src/topological-sort.sigil".to_string()),
            output_file: Some("/tmp/projects/algorithms/.local/src/topological-sort.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("src_graph_types.Ordering"));
        assert!(!result.contains("__sigil_call(\"extern:src/graph-types.Ordering\""));
    }

    #[test]
    fn test_generate_test_metadata_includes_id_and_location() {
        let source = "λmain()→𝕌=()\n\ntest \"smoke\" { true }";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "tests/smoke.sigil").unwrap();

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            source_file: Some("tests/smoke.sigil".to_string()),
            output_file: Some("/tmp/tests/smoke.ts".to_string()),
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("id: 'tests/smoke.sigil::smoke'"));
        assert!(result.contains("location: { start: { line: 3, column: 1 } }"));
    }
}
