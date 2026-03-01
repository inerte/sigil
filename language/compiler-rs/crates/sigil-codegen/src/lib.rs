//! Sigil to TypeScript Code Generator
//!
//! Compiles Sigil AST to runnable TypeScript (ES2022-compatible output).
//!
//! Key transformations:
//! - All functions become `async function`
//! - All function calls use `await`
//! - Pattern matching compiles to if/else chains with __match variables
//! - Sum type constructors compile to objects with __tag and __fields
//! - Mock runtime helpers emitted at top of file

use sigil_ast::*;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("Codegen error: {0}")]
    General(String),
}

pub struct CodegenOptions {
    pub source_file: Option<String>,
    pub output_file: Option<String>,
    pub project_root: Option<String>,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            source_file: None,
            output_file: None,
            project_root: None,
        }
    }
}

pub struct TypeScriptGenerator {
    indent: usize,
    output: Vec<String>,
    source_file: Option<String>,
    output_file: Option<String>,
    project_root: Option<String>,
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
            project_root: options.project_root,
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

    fn emit_mock_runtime_helpers(&mut self) {
        self.emit("const __sigil_mocks = new Map();");
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
        self.emit("  return result === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to ⊥' } };");
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
        self.emit("async function __sigil_call(key, actualFn, args) {");
        self.emit("  const mockFn = __sigil_mocks.get(key);");
        self.emit("  const fn = mockFn ?? actualFn;");
        self.emit("  return await fn(...args);");
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
            "export async function"
        } else {
            "async function"
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
            self.emit(&format!("{}async function {}({}) {{", export_keyword, func.name, params_str));
            self.indent += 1;
            let args = if params.is_empty() {
                String::new()
            } else {
                format!(", {}", params_str)
            };
            self.emit(&format!("return await __sigil_call('{}', {}{});", func.name, impl_name, args));
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
                    "export async function"
                } else {
                    "async function"
                };

                self.emit(&format!("{} {}({}) {{", ctor_keyword, variant.name, params));
                self.indent += 1;
                self.emit(&format!(
                    "return {{ __tag: \"{}\", __fields: [{}] }};",
                    variant.name, params
                ));
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
        let namespace = import.module_path.join("_");

        // Calculate relative import path
        // Module path like src⋅utils becomes .local/src/utils.ts
        let target_path = import.module_path.join("/");

        // Calculate relative path from current output file
        let import_path = if let Some(ref output_file) = self.output_file {
            // Get directory of current output file
            let current_dir = std::path::Path::new(output_file)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".local");

            // Get directory of target module
            let target_dir = std::path::Path::new(&target_path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("");

            let target_file = std::path::Path::new(&target_path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(import.module_path.last().unwrap());

            // Calculate relative path
            if current_dir.ends_with(target_dir) || target_dir.is_empty() {
                // Same directory
                format!("./{}.js", target_file)
            } else if current_dir.contains(target_dir) {
                // Target is in parent directory
                format!("../{}.js", target_file)
            } else {
                // Different directory tree - use relative path segments
                format!("../{}.js", target_path)
            }
        } else {
            // Fallback: use module path directly
            format!("./{}.js", target_path)
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
            let namespace = extern_decl.module_path.last().unwrap();
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
            "{{ name: '{}', description: '{}', fn: __test_{} }}",
            test_name, description, test_name
        ));

        Ok(())
    }

    fn generate_expression(&mut self, expr: &Expr) -> Result<String, CodegenError> {
        match expr {
            Expr::Literal(lit) => self.generate_literal(lit),
            Expr::Identifier(id) => Ok(id.name.clone()),
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
        match &lit.value {
            LiteralValue::Int(n) => Ok(n.to_string()),
            LiteralValue::Float(f) => Ok(f.to_string()),
            LiteralValue::String(s) => {
                // Use JSON encoding to properly escape all special characters including newlines
                Ok(serde_json::to_string(s).unwrap())
            }
            LiteralValue::Char(c) => {
                // Chars are also strings in JavaScript, use JSON encoding
                Ok(serde_json::to_string(&c.to_string()).unwrap())
            }
            LiteralValue::Bool(b) => Ok(b.to_string()),
            LiteralValue::Unit => Ok("null".to_string()),
        }
    }

    fn generate_lambda(&mut self, lambda: &LambdaExpr) -> Result<String, CodegenError> {
        let params: Vec<String> = lambda.params.iter().map(|p| p.name.clone()).collect();
        let params_str = params.join(", ");
        let body = self.generate_expression(&lambda.body)?;
        Ok(format!("(async ({}) => {})", params_str, body))
    }

    fn generate_application(&mut self, app: &ApplicationExpr) -> Result<String, CodegenError> {
        // Check if this is a call to an imported function (MemberAccess)
        // If so, wrap with __sigil_call for mock support
        if let Expr::MemberAccess(member_access) = &app.func {
            // Generate the mock key: "extern:namespace/path.member"
            let mock_key = format!("extern:{}.{}",
                member_access.namespace.join("/"),
                member_access.member);

            // Generate the function reference
            let func_ref = format!("{}.{}",
                member_access.namespace.join("_"),
                member_access.member);

            // Generate arguments
            let args: Result<Vec<String>, CodegenError> = app.args.iter()
                .map(|arg| self.generate_expression(arg))
                .collect();
            let args_list = args?.join(", ");

            // Wrap in __sigil_call
            Ok(format!("await __sigil_call(\"{}\", {}, [{}])",
                mock_key, func_ref, args_list))
        } else {
            // Regular function call
            let func = self.generate_expression(&app.func)?;
            let args: Result<Vec<String>, CodegenError> = app.args.iter()
                .map(|arg| self.generate_expression(arg))
                .collect();
            let args_str = args?.join(", ");

            // All function calls use await
            Ok(format!("await {}({})", func, args_str))
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
            BinaryOperator::Equal => "===",
            BinaryOperator::NotEqual => "!==",
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
                return Ok(format!("(await {}({}))", right, left));
            }
            BinaryOperator::ComposeFwd | BinaryOperator::ComposeBwd => {
                // Function composition - defer to helper
                return Err(CodegenError::General("Function composition not yet implemented".to_string()));
            }
        };

        if bin.operator == BinaryOperator::ListAppend {
            Ok(format!("{}.concat({})", left, right))
        } else {
            Ok(format!("({} {} {})", left, op, right))
        }
    }

    fn generate_unary(&mut self, un: &UnaryExpr) -> Result<String, CodegenError> {
        let operand = self.generate_expression(&un.operand)?;

        match un.operator {
            UnaryOperator::Negate => Ok(format!("(-{})", operand)),
            UnaryOperator::Not => Ok(format!("(!{})", operand)),
            UnaryOperator::Length => Ok(format!("(await {}).length", operand)),
        }
    }

    fn generate_if(&mut self, if_expr: &IfExpr) -> Result<String, CodegenError> {
        let condition = self.generate_expression(&if_expr.condition)?;
        let then_branch = self.generate_expression(&if_expr.then_branch)?;

        if let Some(ref else_branch) = if_expr.else_branch {
            let else_code = self.generate_expression(else_branch)?;
            Ok(format!("({} ? {} : {})", condition, then_branch, else_code))
        } else {
            // No else branch - return null for the false case
            Ok(format!("({} ? {} : null)", condition, then_branch))
        }
    }

    fn generate_list(&mut self, list: &ListExpr) -> Result<String, CodegenError> {
        if list.elements.is_empty() {
            return Ok("[]".to_string());
        }

        if list.elements.len() == 1 {
            let elem = self.generate_expression(&list.elements[0])?;
            // Single element - check if it needs to be wrapped
            match &list.elements[0] {
                Expr::Application(_) | Expr::Identifier(_) => {
                    // Could be an array, could be a value - use concat to ensure proper handling
                    return Ok(format!("[].concat({})", elem));
                }
                _ => {
                    return Ok(format!("[{}]", elem));
                }
            }
        }

        // Multiple elements - use concat for all, wrapping literals in arrays
        let parts: Result<Vec<String>, CodegenError> = list.elements.iter()
            .map(|e| {
                let code = self.generate_expression(e)?;
                match e {
                    Expr::Application(_) | Expr::Identifier(_) => {
                        // Could be array or value, concat handles both
                        Ok(code)
                    }
                    _ => {
                        // Definitely a single value, wrap it
                        Ok(format!("[{}]", code))
                    }
                }
            })
            .collect();

        Ok(format!("[].concat({})", parts?.join(", ")))
    }

    fn generate_tuple(&mut self, tuple: &TupleExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = tuple.elements.iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        Ok(format!("[{}]", elements?.join(", ")))
    }

    fn generate_record(&mut self, record: &RecordExpr) -> Result<String, CodegenError> {
        let fields: Result<Vec<String>, CodegenError> = record.fields.iter()
            .map(|field| {
                let value = self.generate_expression(&field.value)?;
                Ok(format!("{}: {}", field.name, value))
            })
            .collect();
        Ok(format!("{{ {} }}", fields?.join(", ")))
    }

    fn generate_field_access(&mut self, field_access: &FieldAccessExpr) -> Result<String, CodegenError> {
        let object = self.generate_expression(&field_access.object)?;
        Ok(format!("{}.{}", object, field_access.field))
    }

    fn generate_index(&mut self, index: &IndexExpr) -> Result<String, CodegenError> {
        let object = self.generate_expression(&index.object)?;
        let idx = self.generate_expression(&index.index)?;
        Ok(format!("{}[{}]", object, idx))
    }

    fn generate_member_access(&mut self, member_access: &MemberAccessExpr) -> Result<String, CodegenError> {
        // Convert namespace⋅path to namespace_path.member
        let namespace = member_access.namespace.join("_");
        Ok(format!("{}.{}", namespace, member_access.member))
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

        for (i, arm) in match_expr.arms.iter().enumerate() {
            let condition = self.generate_pattern_condition(&arm.pattern, "__match")?;
            let body = self.generate_expression(&arm.body)?;
            let bindings = self.generate_pattern_bindings(&arm.pattern, "__match")?;

            if i == 0 {
                lines.push(format!("  if ({}) {{", condition));
            } else if matches!(arm.pattern, Pattern::Wildcard(_)) {
                lines.push("  else {".to_string());
            } else {
                lines.push(format!("  else if ({}) {{", condition));
            }

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
        Ok(format!("(await Promise.all({}.map(async (x) => await {}(x))))", list, func))
    }

    fn generate_filter(&mut self, filter: &FilterExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&filter.list)?;
        let predicate = self.generate_expression(&filter.predicate)?;
        // Inline filter expansion to match TypeScript compiler output
        Ok(format!(
            "(await Promise.all((await {}).map(async (x) => ({{ x, keep: await {}(x) }})))).filter(({{ keep }}) => keep).map(({{ x }}) => x)",
            list, predicate
        ))
    }

    fn generate_fold(&mut self, fold: &FoldExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&fold.list)?;
        let func = self.generate_expression(&fold.func)?;
        let init = self.generate_expression(&fold.init)?;
        // Inline fold expansion to match TypeScript compiler output
        Ok(format!(
            "(await {}).reduce(async (accPromise, x) => await {}(await accPromise, x), await {})",
            list, func, init
        ))
    }

    fn generate_pipeline(&mut self, pipeline: &PipelineExpr) -> Result<String, CodegenError> {
        let left = self.generate_expression(&pipeline.left)?;
        let right = self.generate_expression(&pipeline.right)?;

        match pipeline.operator {
            PipelineOperator::Pipe => {
                // a |> f becomes await f(a)
                Ok(format!("(await {}({}))", right, left))
            }
            PipelineOperator::ComposeFwd | PipelineOperator::ComposeBwd => {
                Err(CodegenError::General("Function composition not yet implemented".to_string()))
            }
        }
    }

    fn generate_with_mock(&mut self, with_mock: &WithMockExpr) -> Result<String, CodegenError> {
        let target = self.generate_expression(&with_mock.target)?;
        let replacement = self.generate_expression(&with_mock.replacement)?;
        let body = self.generate_expression(&with_mock.body)?;

        // Extract function name from target for mock registration
        // For now, simplified - assumes target is an identifier
        Ok(format!(
            "__sigil_with_mock('{}', {}, async () => {})",
            target, replacement, body
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;

    #[test]
    fn test_empty_program() {
        let program = Program {
            declarations: vec![],
            location: sigil_lexer::SourceLocation {
                start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
                end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
            },
        };

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

        // Should contain async function
        assert!(result.contains("async function add"));
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

        // Should contain constructor functions
        assert!(result.contains("async function Red"));
        assert!(result.contains("async function Green"));
        assert!(result.contains("async function Blue"));
        // Should use __tag pattern
        assert!(result.contains("__tag"));
    }
}
