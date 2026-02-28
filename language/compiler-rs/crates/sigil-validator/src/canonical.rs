//! Canonical form validation
//!
//! Enforces Sigil's "ONE WAY" principle by rejecting alternative patterns:
//! 1. No duplicate declarations
//! 2. No accumulator parameters (prevents tail-call optimization)
//! 3. Canonical pattern matching (most direct form)
//! 4. No CPS (continuation passing style)

use sigil_ast::*;
use std::collections::{HashMap, HashSet};
use crate::error::ValidationError;
use sigil_lexer::{SourceLocation, Position};

/// Validate EOF newline requirement
fn validate_eof_newline(source: &str, file_path: &str) -> Result<(), Vec<ValidationError>> {
    if source.is_empty() {
        return Ok(());
    }

    if !source.ends_with('\n') {
        return Err(vec![ValidationError::EOFNewline {
            filename: file_path.to_string(),
            location: SourceLocation {
                start: Position { line: 1, column: 1, offset: 0 },
                end: Position { line: 1, column: 1, offset: 0 },
            },
        }]);
    }

    Ok(())
}

/// Validate no trailing whitespace
fn validate_no_trailing_whitespace(source: &str, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let lines: Vec<&str> = source.split('\n').collect();

    for (i, line) in lines.iter().enumerate() {
        if line.ends_with(' ') || line.ends_with('\t') {
            return Err(vec![ValidationError::TrailingWhitespace {
                filename: file_path.to_string(),
                line: i + 1,
                location: SourceLocation {
                    start: Position { line: i + 1, column: 1, offset: 0 },
                    end: Position { line: i + 1, column: 1, offset: 0 },
                },
            }]);
        }
    }

    Ok(())
}

/// Validate maximum one blank line
fn validate_blank_lines(source: &str, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let lines: Vec<&str> = source.split('\n').collect();

    for i in 0..lines.len().saturating_sub(1) {
        if lines[i].is_empty() && lines[i + 1].is_empty() {
            return Err(vec![ValidationError::BlankLines {
                filename: file_path.to_string(),
                line: i + 2,
                location: SourceLocation {
                    start: Position { line: i + 2, column: 1, offset: 0 },
                    end: Position { line: i + 2, column: 1, offset: 0 },
                },
            }]);
        }
    }

    Ok(())
}

/// Validate that a program follows canonical form rules
pub fn validate_canonical_form(program: &Program, file_path: Option<&str>, source: Option<&str>) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Validate source formatting first (if source provided)
    if let (Some(path), Some(src)) = (file_path, source) {
        if let Err(e) = validate_eof_newline(src, path) {
            errors.extend(e);
        }
        if let Err(e) = validate_no_trailing_whitespace(src, path) {
            errors.extend(e);
        }
        if let Err(e) = validate_blank_lines(src, path) {
            errors.extend(e);
        }
    }

    // Rule 1: No duplicate declarations
    if let Err(e) = validate_no_duplicates(program) {
        errors.extend(e);
    }

    // Rule 2: File purpose - must be EITHER executable OR library
    if let Err(e) = validate_file_purpose(program, file_path) {
        errors.extend(e);
    }

    // Rule 3: Filename format - lowercase with hyphens only
    if let Some(path) = file_path {
        if let Err(e) = validate_filename_format(path) {
            errors.extend(e);
        }
    }

    // Rule 4: Test location - tests must be in tests/ directories
    if let Some(path) = file_path {
        if let Err(e) = validate_test_location(program, path) {
            errors.extend(e);
        }
    }

    // Rule 5: Declaration ordering - canonical alphabetical order
    if let Err(e) = validate_declaration_ordering(program) {
        errors.extend(e);
    }

    // Rule 6: Recursive functions must not use accumulators
    if let Err(e) = validate_recursive_functions(program) {
        errors.extend(e);
    }

    // Rule 7: Parameter and effect ordering - alphabetical
    if let Err(e) = validate_function_signature_ordering(program) {
        errors.extend(e);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate parameter alphabetical ordering
fn validate_parameter_ordering(
    params: &[Param],
    func_name: &str,
    location: SourceLocation
) -> Result<(), Vec<ValidationError>> {
    if params.len() <= 1 {
        return Ok(());
    }

    for i in 1..params.len() {
        let prev = &params[i - 1];
        let curr = &params[i];

        if curr.name < prev.name {
            let expected_order: Vec<String> = params.iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            let mut sorted_order = expected_order.clone();
            sorted_order.sort();

            return Err(vec![ValidationError::ParameterOrder {
                function_name: func_name.to_string(),
                param_name: curr.name.clone(),
                prev_param: prev.name.clone(),
                position: i + 1,
                expected_order: sorted_order,
                location,
            }]);
        }
    }

    Ok(())
}

/// Validate effect alphabetical ordering
fn validate_effect_ordering(
    effects: &[String],
    func_name: &str,
    location: SourceLocation
) -> Result<(), Vec<ValidationError>> {
    if effects.len() <= 1 {
        return Ok(());
    }

    for i in 1..effects.len() {
        if effects[i] < effects[i - 1] {
            let mut expected_order = effects.to_vec();
            expected_order.sort();

            return Err(vec![ValidationError::EffectOrder {
                function_name: func_name.to_string(),
                effect_name: effects[i].clone(),
                prev_effect: effects[i - 1].clone(),
                position: i + 1,
                expected_order,
                location,
            }]);
        }
    }

    Ok(())
}

/// Validate all function signatures in program
fn validate_function_signature_ordering(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for decl in &program.declarations {
        if let Declaration::Function(func) = decl {
            if let Err(e) = validate_parameter_ordering(&func.params, &func.name, func.location) {
                errors.extend(e);
            }
            if let Err(e) = validate_effect_ordering(&func.effects, &func.name, func.location) {
                errors.extend(e);
            }
        }
    }

    // TODO: Also walk lambda expressions in function bodies

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate that there are no duplicate declarations
fn validate_no_duplicates(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    let mut type_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut extern_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut import_paths: HashMap<String, SourceLocation> = HashMap::new();
    let mut const_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut function_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut test_names: HashMap<String, SourceLocation> = HashMap::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Type(TypeDecl { name, location, .. }) => {
                if let Some(first_loc) = type_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "TYPE".to_string(),
                        what: "type".to_string(),
                        name: name.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    type_names.insert(name.clone(), *location);
                }
            }

            Declaration::Extern(ExternDecl { module_path, location, .. }) => {
                let name = module_path.join("⋅");
                if let Some(first_loc) = extern_names.get(&name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "EXTERN".to_string(),
                        what: "extern".to_string(),
                        name,
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    extern_names.insert(name, *location);
                }
            }

            Declaration::Import(ImportDecl { module_path, location }) => {
                let path = module_path.join("⋅");
                if let Some(first_loc) = import_paths.get(&path) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "IMPORT".to_string(),
                        what: "import".to_string(),
                        name: path,
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    import_paths.insert(path, *location);
                }
            }

            Declaration::Const(ConstDecl { name, location, .. }) => {
                if let Some(first_loc) = const_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "CONST".to_string(),
                        what: "const".to_string(),
                        name: name.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    const_names.insert(name.clone(), *location);
                }
            }

            Declaration::Function(FunctionDecl { name, location, .. }) => {
                if let Some(first_loc) = function_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "FUNCTION".to_string(),
                        what: "function".to_string(),
                        name: name.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    function_names.insert(name.clone(), *location);
                }
            }

            Declaration::Test(TestDecl { description, location, .. }) => {
                if let Some(first_loc) = test_names.get(description) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "TEST".to_string(),
                        what: "test".to_string(),
                        name: description.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    test_names.insert(description.clone(), *location);
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate file purpose - must be EITHER executable OR library (exclusive)
///
/// Extension-based validation:
/// - `.lib.sigil` files CANNOT have main() function
/// - `.sigil` files (non-test) MUST have main() function
/// - Test files (`tests/` in path) can have either
fn validate_file_purpose(program: &Program, file_path: Option<&str>) -> Result<(), Vec<ValidationError>> {
    let mut has_main = false;
    let has_tests = program.declarations.iter().any(|d| matches!(d, Declaration::Test(_)));

    for decl in &program.declarations {
        if let Declaration::Function(FunctionDecl { name, .. }) = decl {
            if name == "main" {
                has_main = true;
            }
        }
    }

    // Extension-based validation
    if let Some(path) = file_path {
        let is_lib_file = path.ends_with(".lib.sigil");
        let is_test_file = path.contains("/tests/");

        if is_lib_file && has_main {
            return Err(vec![ValidationError::LibNoMain {
                message: format!(".lib.sigil files are libraries and cannot have main()\n\nFile: {}\nSolution: Remove main() or rename to .sigil executable", path),
            }]);
        }

        if !is_lib_file && !is_test_file && !has_main {
            return Err(vec![ValidationError::ExecNeedsMain {
                message: format!(".sigil executables must have main() function\n\nFile: {}\nSolution: Add λmain() or rename to .lib.sigil library", path),
            }]);
        }
    }

    // Test-specific validation
    if has_tests && !has_main {
        return Err(vec![ValidationError::TestNeedsMain {
            message: "Test files must have λmain()→𝕌=()\n\nHint: Test files are executables".to_string(),
        }]);
    }

    Ok(())
}

/// Validate filename format - lowercase, hyphens only
fn validate_filename_format(file_path: &str) -> Result<(), Vec<ValidationError>> {
    // Extract basename (without extension)
    let basename = file_path
        .strip_suffix(".lib.sigil")
        .or_else(|| file_path.strip_suffix(".sigil"))
        .and_then(|p| p.split('/').last())
        .unwrap_or("");

    let location = SourceLocation {
        start: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
        end: sigil_lexer::Position { line: 1, column: 1, offset: 0 },
    };

    // Check for uppercase
    if basename != basename.to_lowercase() {
        return Err(vec![ValidationError::FilenameCase {
            filename: file_path.to_string(),
            basename: basename.to_string(),
            suggested: format!("{}.{{sigil,lib.sigil}}", basename.to_lowercase()),
            location,
        }]);
    }

    // Check for underscores
    if basename.contains('_') {
        return Err(vec![ValidationError::FilenameInvalidChar {
            filename: file_path.to_string(),
            basename: basename.to_string(),
            suggested: format!("{}.{{sigil,lib.sigil}}", basename.replace('_', "-")),
            invalid_char: "underscores".to_string(),
            location,
        }]);
    }

    // Check for invalid characters
    let invalid_chars: Vec<char> = basename
        .chars()
        .filter(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && *c != '-')
        .collect();

    if !invalid_chars.is_empty() {
        return Err(vec![ValidationError::FilenameInvalidChar {
            filename: file_path.to_string(),
            basename: basename.to_string(),
            suggested: basename.to_string(),
            invalid_char: format!("{:?}", invalid_chars),
            location,
        }]);
    }

    // Check format
    if basename.is_empty() {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: "Filename cannot be empty".to_string(),
            location,
        }]);
    }

    if basename.starts_with('-') || basename.ends_with('-') {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: "Filename cannot start or end with hyphen".to_string(),
            location,
        }]);
    }

    if basename.contains("--") {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: "Filename cannot contain consecutive hyphens".to_string(),
            location,
        }]);
    }

    Ok(())
}

/// Validate that test blocks only appear in tests/ directories
fn validate_test_location(program: &Program, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let has_tests = program.declarations.iter().any(|d| matches!(d, Declaration::Test(_)));

    if !has_tests {
        return Ok(());
    }

    // Normalize path separators
    let normalized_path = file_path.replace('\\', "/");

    // Check if file is in a tests/ directory
    if !normalized_path.contains("/tests/") {
        return Err(vec![ValidationError::TestLocationInvalid {
            message: format!(
                "test blocks can only appear in files under tests/ directories.\n\n\
                This file contains test blocks but is not in a tests/ directory.\n\n\
                Move this file to a tests/ directory (e.g., tests/your-test.sigil).\n\n\
                Sigil enforces ONE way: tests live in tests/ directories."
            ),
            file_path: normalized_path,
        }]);
    }

    Ok(())
}

/// Validate recursive functions don't use accumulator parameters
fn validate_recursive_functions(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for decl in &program.declarations {
        if let Declaration::Function(func) = decl {
            // Check if function is recursive
            if !is_recursive(&func.body, &func.name) {
                continue;
            }

            // Check 1: Function with multiple parameters might be using accumulator pattern
            if func.params.len() > 1 {
                // Simplified check: Look for parameters that appear to grow
                // Full implementation would analyze parameter roles (STRUCTURAL vs ACCUMULATOR)
                let suspicious_params = detect_accumulator_params(func);

                if !suspicious_params.is_empty() {
                    errors.push(ValidationError::AccumulatorParameter {
                        function_name: func.name.clone(),
                        params: suspicious_params.join(", "),
                        location: func.location,
                    });
                }
            }

            // Check 2: Return type cannot be a function (blocks CPS)
            if let Some(Type::Function(_)) = &func.return_type {
                errors.push(ValidationError::ContinuationPassingStyle {
                    function_name: func.name.clone(),
                    location: func.location,
                });
            }

            // Check 3: Collection parameters must use structural recursion
            // Simplified: just check that collection params are destructured in patterns
            if func.params.len() == 1 {
                if let Some(Type::List(_)) = func.params[0].type_annotation.as_ref() {
                    if !uses_structural_recursion(&func.body) {
                        errors.push(ValidationError::NonStructuralRecursion {
                            function_name: func.name.clone(),
                            param_name: func.params[0].name.clone(),
                            location: func.location,
                        });
                    }
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check if an expression contains a recursive call to the given function
fn is_recursive(expr: &Expr, function_name: &str) -> bool {
    match expr {
        Expr::Application(app) => {
            // Check if calling itself
            if matches!(app.func, Expr::Identifier(IdentifierExpr { ref name, .. }) if name == function_name) {
                return true;
            }
            // Check function and args
            is_recursive(&app.func, function_name) ||
                app.args.iter().any(|arg| is_recursive(arg, function_name))
        }

        Expr::Identifier(_) | Expr::Literal(_) => false,

        Expr::Lambda(lambda) => is_recursive(&lambda.body, function_name),

        Expr::Binary(bin) => {
            is_recursive(&bin.left, function_name) || is_recursive(&bin.right, function_name)
        }

        Expr::Unary(un) => is_recursive(&un.operand, function_name),

        Expr::Match(m) => {
            is_recursive(&m.scrutinee, function_name) ||
                m.arms.iter().any(|arm| {
                    arm.guard.as_ref().map(|g| is_recursive(g, function_name)).unwrap_or(false) ||
                        is_recursive(&arm.body, function_name)
                })
        }

        Expr::Let(l) => {
            is_recursive(&l.value, function_name) || is_recursive(&l.body, function_name)
        }

        Expr::If(i) => {
            is_recursive(&i.condition, function_name) ||
                is_recursive(&i.then_branch, function_name) ||
                i.else_branch.as_ref().map(|e| is_recursive(e, function_name)).unwrap_or(false)
        }

        Expr::List(l) => l.elements.iter().any(|e| is_recursive(e, function_name)),

        Expr::Record(r) => r.fields.iter().any(|f| is_recursive(&f.value, function_name)),

        Expr::Tuple(t) => t.elements.iter().any(|e| is_recursive(e, function_name)),

        Expr::FieldAccess(f) => is_recursive(&f.object, function_name),

        Expr::Index(i) => {
            is_recursive(&i.object, function_name) || is_recursive(&i.index, function_name)
        }

        Expr::Pipeline(p) => {
            is_recursive(&p.left, function_name) || is_recursive(&p.right, function_name)
        }

        Expr::Map(m) => {
            is_recursive(&m.list, function_name) || is_recursive(&m.func, function_name)
        }

        Expr::Filter(f) => {
            is_recursive(&f.list, function_name) || is_recursive(&f.predicate, function_name)
        }

        Expr::Fold(f) => {
            is_recursive(&f.list, function_name) ||
                is_recursive(&f.func, function_name) ||
                is_recursive(&f.init, function_name)
        }

        Expr::MemberAccess(_) => false,

        Expr::WithMock(w) => {
            is_recursive(&w.target, function_name) ||
                is_recursive(&w.replacement, function_name) ||
                is_recursive(&w.body, function_name)
        }
    }
}

/// Detect parameters that might be accumulators (simplified heuristic)
fn detect_accumulator_params(func: &FunctionDecl) -> Vec<String> {
    // Simplified: In a real implementation, we would analyze how parameters
    // are used in recursive calls to classify them as STRUCTURAL, QUERY, or ACCUMULATOR

    // For now, we'll use a simple heuristic: if a parameter appears in a binary
    // operation in a recursive call argument, it might be an accumulator

    let mut suspicious = Vec::new();

    // This is a placeholder - full implementation would need data flow analysis
    // to determine if parameters grow during recursion

    suspicious
}

/// Check if function body uses structural recursion on a list
fn uses_structural_recursion(expr: &Expr) -> bool {
    // Simplified check: Look for match expressions that destructure lists
    match expr {
        Expr::Match(m) => {
            // Check if any arm uses list destructuring pattern
            m.arms.iter().any(|arm| matches_list_pattern(&arm.pattern))
        }
        _ => true, // Default to allowing if not a match expression
    }
}

/// Check if pattern destructures a list
fn matches_list_pattern(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::List(ListPattern { patterns, rest, .. }) => {
            // Structural recursion requires destructuring: [x, .xs]
            !patterns.is_empty() || rest.is_some()
        }
        Pattern::Constructor(_) => true, // Constructor patterns are OK
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;

    #[test]
    fn test_no_duplicate_functions() {
        let source = r#"λ bar(y: ℤ) → ℤ = y * 2
λ foo(x: ℤ) → ℤ = x + 1
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("test.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_duplicate_function_error() {
        let source = r#"λ foo(x: ℤ) → ℤ = x + 1
λ foo(y: ℤ) → ℤ = y * 2
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.lib.sigil"), Some(source));
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::DuplicateDeclaration { .. }));
    }

    #[test]
    fn test_simple_recursion_allowed() {
        // TODO: Parser bug - match expressions with scrutinee (≡n{...}) don't work yet
        // For now, test with a simple recursive function without pattern matching
        let source = r#"λfactorial(n:ℤ)→ℤ=factorial(n-1)"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        // Should pass - simple recursion is allowed
        assert!(validate_canonical_form(&program, None, None).is_ok());
    }
}

/// Validate canonical declaration ordering
fn validate_declaration_ordering(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    
    // Check category order (type → extern → import → const → function → test)
    if let Err(e) = validate_category_boundaries(&program.declarations) {
        errors.extend(e);
    }
    
    // Check alphabetical order within each category
    let functions: Vec<_> = program.declarations.iter()
        .filter_map(|d| if let Declaration::Function(f) = d { Some(f) } else { None })
        .collect();
    
    if let Err(e) = validate_alphabetical_order(&functions) {
        errors.extend(e);
    }
    
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check that declaration categories appear in correct order
fn validate_category_boundaries(declarations: &[Declaration]) -> Result<(), Vec<ValidationError>> {
    let get_category_index = |decl: &Declaration| -> usize {
        match decl {
            Declaration::Type(_) => 0,
            Declaration::Extern(_) => 1,
            Declaration::Import(_) => 2,
            Declaration::Const(_) => 3,
            Declaration::Function(_) => 4,
            Declaration::Test(_) => 5,
        }
    };
    
    let mut last_category_index: i32 = -1;
    
    for decl in declarations {
        let current_index = get_category_index(decl) as i32;
        
        if current_index < last_category_index {
            let category_names = ["type", "extern", "import", "const", "function", "test"];
            let category_symbols = ["t", "e", "i", "c", "λ", "test"];
            
            return Err(vec![ValidationError::DeclarationOrder {
                message: format!(
                    "SIGIL-CANON-DECL-CATEGORY-ORDER: Wrong category position\n\
                     Found: {} ({}) at line {}\n\
                     Category order: t → e → i → c → λ → test",
                    category_symbols[current_index as usize],
                    category_names[current_index as usize],
                    get_declaration_location(decl).start.line
                ),
            }]);
        }
        
        last_category_index = last_category_index.max(current_index);
    }
    
    Ok(())
}

/// Check alphabetical ordering within function declarations
fn validate_alphabetical_order(functions: &[&FunctionDecl]) -> Result<(), Vec<ValidationError>> {
    for i in 1..functions.len() {
        let prev = functions[i - 1];
        let curr = functions[i];
        
        if curr.name < prev.name {
            return Err(vec![ValidationError::DeclarationOrder {
                message: format!(
                    "SIGIL-CANON-DECL-ALPHABETICAL: Declaration out of alphabetical order\n\n\
                     Found: λ {} at line {}\n\
                     After: λ {} at line {}\n\n\
                     Within 'λ' category, non-exported declarations must be alphabetical.\n\
                     Expected '{}' to come before '{}'.\n\n\
                     Alphabetical order uses Unicode code point comparison (case-sensitive).\n\
                     Move '{}' to come before '{}'.\n\n\
                     Sigil enforces ONE way: strict alphabetical ordering within categories.",
                    curr.name,
                    curr.location.start.line,
                    prev.name,
                    prev.location.start.line,
                    curr.name,
                    prev.name,
                    curr.name,
                    prev.name
                ),
            }]);
        }
    }
    
    Ok(())
}

/// Get location from any declaration
fn get_declaration_location(decl: &Declaration) -> &SourceLocation {
    match decl {
        Declaration::Type(TypeDecl { location, .. }) => location,
        Declaration::Extern(ExternDecl { location, .. }) => location,
        Declaration::Import(ImportDecl { location, .. }) => location,
        Declaration::Const(ConstDecl { location, .. }) => location,
        Declaration::Function(FunctionDecl { location, .. }) => location,
        Declaration::Test(TestDecl { location, .. }) => location,
    }
}
