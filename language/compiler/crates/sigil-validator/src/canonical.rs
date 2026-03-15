//! Canonical form validation
//!
//! Enforces Sigil's "ONE WAY" principle by rejecting alternative patterns:
//! 1. No duplicate declarations
//! 2. No accumulator parameters (prevents tail-call optimization)
//! 3. Canonical pattern matching (most direct form)
//! 4. No CPS (continuation passing style)

use sigil_ast::*;
use sigil_typechecker::{PurityClass, TypedDeclaration, TypedExpr, TypedExprKind, TypedProgram};
use std::collections::{HashMap, HashSet};
use crate::error::ValidationError;
use sigil_lexer::{tokenize, Position, SourceLocation, Token, TokenType};

fn is_lower_camel_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }

    chars.all(|ch| ch.is_ascii_alphanumeric())
}

fn is_upper_camel_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => {}
        _ => return false,
    }

    chars.all(|ch| ch.is_ascii_alphanumeric())
}

fn split_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_was_lower_or_digit = false;

    for ch in name.chars() {
        if matches!(ch, '_' | '-' | ' ' | '.') {
            if !current.is_empty() {
                words.push(current.to_ascii_lowercase());
                current.clear();
            }
            previous_was_lower_or_digit = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_was_lower_or_digit && !current.is_empty() {
            words.push(current.to_ascii_lowercase());
            current.clear();
        }

        current.push(ch);
        previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }

    if !current.is_empty() {
        words.push(current.to_ascii_lowercase());
    }

    words
}

fn to_lower_camel_case(name: &str) -> String {
    let words = split_words(name);
    if words.is_empty() {
        return name.to_string();
    }

    let mut result = String::new();
    result.push_str(&words[0]);
    for word in words.iter().skip(1) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            result.push(first.to_ascii_uppercase());
            result.extend(chars);
        }
    }
    result
}

fn to_upper_camel_case(name: &str) -> String {
    let words = split_words(name);
    if words.is_empty() {
        return name.to_string();
    }

    let mut result = String::new();
    for word in words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            result.push(first.to_ascii_uppercase());
            result.extend(chars);
        }
    }
    result
}

fn suggestion_suffix(found: &str, suggested: String) -> String {
    if suggested.is_empty() || suggested == found {
        String::new()
    } else {
        format!("\nSuggested: {}", suggested)
    }
}

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

fn slice_between<'a>(source: &'a str, start: usize, end: usize) -> &'a str {
    if start >= end || end > source.len() {
        ""
    } else {
        source.get(start..end).unwrap_or("")
    }
}

fn slice_at_line(source: &str, line: usize) -> &str {
    source.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

fn contains_space_outside_comments(slice: &str) -> bool {
    let mut chars = slice.chars().peekable();
    let mut in_comment = false;

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '⟧' {
                in_comment = false;
            }
            continue;
        }

        if ch == '⟦' {
            in_comment = true;
            continue;
        }

        if ch == ' ' {
            return true;
        }
    }

    false
}

fn has_space_gap(source: &str, left: &Token, right: &Token) -> bool {
    left.location.end.line == right.location.start.line
        && contains_space_outside_comments(slice_between(source, left.location.end.offset, right.location.start.offset))
}

fn char_before(source: &str, offset: usize) -> Option<char> {
    source.get(..offset)?.chars().next_back()
}

fn char_after(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().next()
}

fn expr_location(expr: &Expr) -> SourceLocation {
    match expr {
        Expr::Literal(expr) => expr.location,
        Expr::Identifier(expr) => expr.location,
        Expr::Lambda(expr) => expr.location,
        Expr::Application(expr) => expr.location,
        Expr::Binary(expr) => expr.location,
        Expr::Unary(expr) => expr.location,
        Expr::Match(expr) => expr.location,
        Expr::Let(expr) => expr.location,
        Expr::If(expr) => expr.location,
        Expr::List(expr) => expr.location,
        Expr::Record(expr) => expr.location,
        Expr::MapLiteral(expr) => expr.location,
        Expr::Tuple(expr) => expr.location,
        Expr::FieldAccess(expr) => expr.location,
        Expr::Index(expr) => expr.location,
        Expr::Pipeline(expr) => expr.location,
        Expr::Map(expr) => expr.location,
        Expr::Filter(expr) => expr.location,
        Expr::Fold(expr) => expr.location,
        Expr::MemberAccess(expr) => expr.location,
        Expr::WithMock(expr) => expr.location,
        Expr::TypeAscription(expr) => expr.location,
    }
}

fn type_location(ty: &Type) -> SourceLocation {
    match ty {
        Type::Primitive(ty) => ty.location,
        Type::List(ty) => ty.location,
        Type::Map(ty) => ty.location,
        Type::Function(ty) => ty.location,
        Type::Constructor(ty) => ty.location,
        Type::Variable(ty) => ty.location,
        Type::Tuple(ty) => ty.location,
        Type::Qualified(ty) => ty.location,
    }
}

fn token_is_open_delimiter(token_type: TokenType) -> bool {
    matches!(token_type, TokenType::LPAREN | TokenType::LBRACKET | TokenType::LBRACE)
}

fn token_is_close_delimiter(token_type: TokenType) -> bool {
    matches!(token_type, TokenType::RPAREN | TokenType::RBRACKET | TokenType::RBRACE)
}

fn token_forbids_surrounding_spaces(token_type: TokenType) -> bool {
    matches!(
        token_type,
        TokenType::COLON
            | TokenType::ARROW
            | TokenType::EQUAL
            | TokenType::PipeSep
            | TokenType::PLUS
            | TokenType::MINUS
            | TokenType::STAR
            | TokenType::SLASH
            | TokenType::PERCENT
    )
}

fn validate_token_spacing(source: &str, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let tokens = tokenize(source).map_err(|error| vec![ValidationError::FilenameFormat {
        filename: file_path.to_string(),
        message: error.to_string(),
        location: SourceLocation::new(Position::new(1, 1, 0), Position::new(1, 1, 0)),
    }])?;

    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|token| token.token_type != TokenType::NEWLINE && token.token_type != TokenType::EOF)
        .collect();

    for window in significant.windows(2) {
        let left = window[0];
        let right = window[1];

        if token_is_open_delimiter(left.token_type) && has_space_gap(source, left, right) {
            return Err(vec![ValidationError::DelimiterSpacing {
                location: SourceLocation::new(left.location.start, right.location.start),
            }]);
        }

        if token_is_close_delimiter(right.token_type) && has_space_gap(source, left, right) {
            return Err(vec![ValidationError::DelimiterSpacing {
                location: SourceLocation::new(left.location.end, right.location.end),
            }]);
        }

        if token_forbids_surrounding_spaces(left.token_type) && has_space_gap(source, left, right) {
            return Err(vec![ValidationError::OperatorSpacing {
                location: SourceLocation::new(left.location.start, right.location.start),
            }]);
        }

        if token_forbids_surrounding_spaces(right.token_type) && has_space_gap(source, left, right) {
            return Err(vec![ValidationError::OperatorSpacing {
                location: SourceLocation::new(left.location.end, right.location.end),
            }]);
        }
    }

    Ok(())
}

fn validate_function_body_layout(function: &FunctionDecl, source: &str) -> Result<(), Vec<ValidationError>> {
    let body_location = expr_location(&function.body);
    if function.location.start.line != body_location.start.line {
        return Err(vec![ValidationError::SignatureLayout {
            location: function.location,
        }]);
    }

    if matches!(function.body, Expr::Match(_)) {
        let between = slice_between(source, type_location(function.return_type.as_ref().unwrap()).end.offset, body_location.start.offset);
        if between.contains('{') {
            return Err(vec![ValidationError::MatchBodyBlock {
                location: function.location,
            }]);
        }
    }

    Ok(())
}

fn validate_lambda_body_layout(lambda: &LambdaExpr, source: &str) -> Result<(), Vec<ValidationError>> {
    let body_location = expr_location(&lambda.body);
    if lambda.location.start.line != body_location.start.line {
        return Err(vec![ValidationError::SignatureLayout {
            location: lambda.location,
        }]);
    }

    if matches!(lambda.body, Expr::Match(_)) {
        let between = slice_between(source, type_location(&lambda.return_type).end.offset, body_location.start.offset);
        if between.contains('{') {
            return Err(vec![ValidationError::MatchBodyBlock {
                location: lambda.location,
            }]);
        }
    }

    Ok(())
}

fn validate_match_layout(match_expr: &MatchExpr, source: &str) -> Result<(), Vec<ValidationError>> {
    let multiline = match_expr.location.start.line != match_expr.location.end.line;

    if match_expr.arms.len() > 1 && !multiline {
        return Err(vec![ValidationError::MatchLayout {
            location: match_expr.location,
        }]);
    }

    if !multiline {
        return Ok(());
    }

    for line in (match_expr.location.start.line + 1)..match_expr.location.end.line {
        let trimmed = slice_at_line(source, line).trim();
        if trimmed.is_empty() {
            return Err(vec![ValidationError::MatchArmLayout {
                location: match_expr.location,
            }]);
        }
        if trimmed.starts_with('|') {
            return Err(vec![ValidationError::MatchArmLayout {
                location: match_expr.location,
            }]);
        }
    }

    for arm in &match_expr.arms {
        let body_location = expr_location(&arm.body);
        if arm.location.start.line != body_location.start.line {
            return Err(vec![ValidationError::MatchArmLayout {
                location: arm.location,
            }]);
        }
    }

    Ok(())
}

fn validate_redundant_parens_in_body(expr: &Expr, source: &str) -> Result<(), Vec<ValidationError>> {
    let can_be_meaningfully_wrapped = matches!(
        expr,
        Expr::Application(_)
            | Expr::Binary(_)
            | Expr::Unary(_)
            | Expr::Match(_)
            | Expr::Let(_)
            | Expr::If(_)
            | Expr::List(_)
            | Expr::Record(_)
            | Expr::MapLiteral(_)
            | Expr::Tuple(_)
            | Expr::FieldAccess(_)
            | Expr::Index(_)
            | Expr::Pipeline(_)
            | Expr::Map(_)
            | Expr::Filter(_)
            | Expr::Fold(_)
            | Expr::TypeAscription(_)
    );
    if !can_be_meaningfully_wrapped {
        return Ok(());
    }

    let location = expr_location(expr);
    if char_before(source, location.start.offset) == Some('(')
        && char_after(source, location.end.offset) == Some(')')
    {
        return Err(vec![ValidationError::RedundantParens { location }]);
    }
    Ok(())
}

fn validate_expr_layout(expr: &Expr, source: &str, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::Lambda(lambda) => {
            if let Err(error) = validate_lambda_body_layout(lambda, source) {
                errors.extend(error);
            }
            validate_expr_layout(&lambda.body, source, errors);
        }
        Expr::Match(match_expr) => {
            if let Err(error) = validate_match_layout(match_expr, source) {
                errors.extend(error);
            }
            for arm in &match_expr.arms {
                if let Err(error) = validate_redundant_parens_in_body(&arm.body, source) {
                    errors.extend(error);
                }
                if let Some(guard) = &arm.guard {
                    validate_expr_layout(guard, source, errors);
                }
                validate_expr_layout(&arm.body, source, errors);
            }
            validate_expr_layout(&match_expr.scrutinee, source, errors);
        }
        Expr::Application(application) => {
            validate_expr_layout(&application.func, source, errors);
            for arg in &application.args {
                validate_expr_layout(arg, source, errors);
            }
        }
        Expr::Binary(binary) => {
            validate_expr_layout(&binary.left, source, errors);
            validate_expr_layout(&binary.right, source, errors);
        }
        Expr::Unary(unary) => validate_expr_layout(&unary.operand, source, errors),
        Expr::Let(let_expr) => {
            validate_expr_layout(&let_expr.value, source, errors);
            validate_expr_layout(&let_expr.body, source, errors);
        }
        Expr::If(if_expr) => {
            validate_expr_layout(&if_expr.condition, source, errors);
            validate_expr_layout(&if_expr.then_branch, source, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                validate_expr_layout(else_branch, source, errors);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                validate_expr_layout(element, source, errors);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                validate_expr_layout(&field.value, source, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_expr_layout(&entry.key, source, errors);
                validate_expr_layout(&entry.value, source, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_expr_layout(element, source, errors);
            }
        }
        Expr::FieldAccess(access) => validate_expr_layout(&access.object, source, errors),
        Expr::Index(index) => {
            validate_expr_layout(&index.object, source, errors);
            validate_expr_layout(&index.index, source, errors);
        }
        Expr::Pipeline(pipeline) => {
            validate_expr_layout(&pipeline.left, source, errors);
            validate_expr_layout(&pipeline.right, source, errors);
        }
        Expr::Map(map_expr) => {
            validate_expr_layout(&map_expr.list, source, errors);
            validate_expr_layout(&map_expr.func, source, errors);
        }
        Expr::Filter(filter) => {
            validate_expr_layout(&filter.list, source, errors);
            validate_expr_layout(&filter.predicate, source, errors);
        }
        Expr::Fold(fold) => {
            validate_expr_layout(&fold.list, source, errors);
            validate_expr_layout(&fold.func, source, errors);
            validate_expr_layout(&fold.init, source, errors);
        }
        Expr::WithMock(with_mock) => validate_expr_layout(&with_mock.body, source, errors),
        Expr::TypeAscription(ascription) => validate_expr_layout(&ascription.expr, source, errors),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => {}
    }
}

fn validate_source_layout(program: &Program, source: &str, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if let Err(error) = validate_token_spacing(source, file_path) {
        errors.extend(error);
    }

    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                if let Err(error) = validate_function_body_layout(function, source) {
                    errors.extend(error);
                }
                if let Err(error) = validate_redundant_parens_in_body(&function.body, source) {
                    errors.extend(error);
                }
                validate_expr_layout(&function.body, source, &mut errors);
            }
            Declaration::Const(const_decl) => {
                validate_expr_layout(&const_decl.value, source, &mut errors);
            }
            Declaration::Test(test_decl) => {
                validate_expr_layout(&test_decl.body, source, &mut errors);
            }
            Declaration::Type(_) | Declaration::Import(_) | Declaration::Extern(_) => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
        if let Err(e) = validate_source_layout(program, src, path) {
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

    // Rule 3: Filename format - lowerCamelCase only
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

    // Rule 5b: withMock is only valid inside test declaration bodies
    if let Err(e) = validate_with_mock_placement(program) {
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

    // Rule 8: Record fields - alphabetical everywhere
    if let Err(e) = validate_record_field_ordering(program) {
        errors.extend(e);
    }

    // Rule 9: One local name, one meaning
    if let Err(e) = validate_no_shadowing(program) {
        errors.extend(e);
    }

    // Rule 10: Naming forms - lowerCamelCase / UpperCamelCase only
    validate_naming_forms(program, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate canonical rules that require typed purity/effect information.
pub fn validate_typed_canonical_form(program: &TypedProgram) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for declaration in &program.declarations {
        match declaration {
            TypedDeclaration::Function(function) => {
                collect_single_use_pure_bindings(&function.body, &mut errors);
            }
            TypedDeclaration::Const(const_decl) => {
                collect_single_use_pure_bindings(&const_decl.value, &mut errors);
            }
            TypedDeclaration::Test(test_decl) => {
                collect_single_use_pure_bindings(&test_decl.body, &mut errors);
            }
            TypedDeclaration::Type(_)
            | TypedDeclaration::Import(_)
            | TypedDeclaration::Extern(_) => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn collect_single_use_pure_bindings(expr: &TypedExpr, errors: &mut Vec<ValidationError>) {
    match &expr.kind {
        TypedExprKind::Let(let_expr) => {
            if let Pattern::Identifier(identifier) = &let_expr.pattern {
                let usage_count = count_identifier_uses(&let_expr.body, &identifier.name);
                if usage_count == 1 && let_expr.value.purity == PurityClass::Pure {
                    errors.push(ValidationError::SingleUsePureBinding {
                        binding_name: identifier.name.clone(),
                        location: expr.location,
                    });
                }
            }

            collect_single_use_pure_bindings(&let_expr.value, errors);
            collect_single_use_pure_bindings(&let_expr.body, errors);
        }
        TypedExprKind::Lambda(lambda) => {
            collect_single_use_pure_bindings(&lambda.body, errors);
        }
        TypedExprKind::Call(call) => {
            collect_single_use_pure_bindings(&call.func, errors);
            for arg in &call.args {
                collect_single_use_pure_bindings(arg, errors);
            }
        }
        TypedExprKind::ConstructorCall(call) => {
            for arg in &call.args {
                collect_single_use_pure_bindings(arg, errors);
            }
        }
        TypedExprKind::ExternCall(call) => {
            for arg in &call.args {
                collect_single_use_pure_bindings(arg, errors);
            }
        }
        TypedExprKind::MethodCall(call) => {
            collect_single_use_pure_bindings(&call.receiver, errors);
            for arg in &call.args {
                collect_single_use_pure_bindings(arg, errors);
            }
        }
        TypedExprKind::Binary(binary) => {
            collect_single_use_pure_bindings(&binary.left, errors);
            collect_single_use_pure_bindings(&binary.right, errors);
        }
        TypedExprKind::Unary(unary) => {
            collect_single_use_pure_bindings(&unary.operand, errors);
        }
        TypedExprKind::Match(match_expr) => {
            collect_single_use_pure_bindings(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_single_use_pure_bindings(guard, errors);
                }
                collect_single_use_pure_bindings(&arm.body, errors);
            }
        }
        TypedExprKind::If(if_expr) => {
            collect_single_use_pure_bindings(&if_expr.condition, errors);
            collect_single_use_pure_bindings(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_single_use_pure_bindings(else_branch, errors);
            }
        }
        TypedExprKind::List(list) => {
            for element in &list.elements {
                collect_single_use_pure_bindings(element, errors);
            }
        }
        TypedExprKind::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_single_use_pure_bindings(element, errors);
            }
        }
        TypedExprKind::Record(record) => {
            for field in &record.fields {
                collect_single_use_pure_bindings(&field.value, errors);
            }
        }
        TypedExprKind::MapLiteral(map) => {
            for entry in &map.entries {
                collect_single_use_pure_bindings(&entry.key, errors);
                collect_single_use_pure_bindings(&entry.value, errors);
            }
        }
        TypedExprKind::FieldAccess(access) => {
            collect_single_use_pure_bindings(&access.object, errors);
        }
        TypedExprKind::Index(index) => {
            collect_single_use_pure_bindings(&index.object, errors);
            collect_single_use_pure_bindings(&index.index, errors);
        }
        TypedExprKind::Map(map_expr) => {
            collect_single_use_pure_bindings(&map_expr.list, errors);
            collect_single_use_pure_bindings(&map_expr.func, errors);
        }
        TypedExprKind::Filter(filter) => {
            collect_single_use_pure_bindings(&filter.list, errors);
            collect_single_use_pure_bindings(&filter.predicate, errors);
        }
        TypedExprKind::Fold(fold) => {
            collect_single_use_pure_bindings(&fold.list, errors);
            collect_single_use_pure_bindings(&fold.func, errors);
            collect_single_use_pure_bindings(&fold.init, errors);
        }
        TypedExprKind::Pipeline(pipeline) => {
            collect_single_use_pure_bindings(&pipeline.left, errors);
            collect_single_use_pure_bindings(&pipeline.right, errors);
        }
        TypedExprKind::WithMock(with_mock) => {
            collect_single_use_pure_bindings(&with_mock.body, errors);
        }
        TypedExprKind::Literal(_)
        | TypedExprKind::Identifier(_)
        | TypedExprKind::NamespaceMember { .. } => {}
    }
}

fn count_identifier_uses(expr: &TypedExpr, name: &str) -> usize {
    match &expr.kind {
        TypedExprKind::Identifier(identifier) => usize::from(identifier.name == name),
        TypedExprKind::Literal(_) | TypedExprKind::NamespaceMember { .. } => 0,
        TypedExprKind::Lambda(lambda) => count_identifier_uses(&lambda.body, name),
        TypedExprKind::Call(call) => {
            count_identifier_uses(&call.func, name)
                + call
                    .args
                    .iter()
                    .map(|arg| count_identifier_uses(arg, name))
                    .sum::<usize>()
        }
        TypedExprKind::ConstructorCall(call) => call
            .args
            .iter()
            .map(|arg| count_identifier_uses(arg, name))
            .sum(),
        TypedExprKind::ExternCall(call) => call
            .args
            .iter()
            .map(|arg| count_identifier_uses(arg, name))
            .sum(),
        TypedExprKind::MethodCall(call) => {
            count_identifier_uses(&call.receiver, name)
                + call
                    .args
                    .iter()
                    .map(|arg| count_identifier_uses(arg, name))
                    .sum::<usize>()
        }
        TypedExprKind::Binary(binary) => {
            count_identifier_uses(&binary.left, name) + count_identifier_uses(&binary.right, name)
        }
        TypedExprKind::Unary(unary) => count_identifier_uses(&unary.operand, name),
        TypedExprKind::Match(match_expr) => {
            count_identifier_uses(&match_expr.scrutinee, name)
                + match_expr
                    .arms
                    .iter()
                    .map(|arm| {
                        arm.guard
                            .as_ref()
                            .map(|guard| count_identifier_uses(guard, name))
                            .unwrap_or(0)
                            + count_identifier_uses(&arm.body, name)
                    })
                    .sum::<usize>()
        }
        TypedExprKind::Let(let_expr) => {
            count_identifier_uses(&let_expr.value, name) + count_identifier_uses(&let_expr.body, name)
        }
        TypedExprKind::If(if_expr) => {
            count_identifier_uses(&if_expr.condition, name)
                + count_identifier_uses(&if_expr.then_branch, name)
                + if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| count_identifier_uses(branch, name))
                    .unwrap_or(0)
        }
        TypedExprKind::List(list) => list
            .elements
            .iter()
            .map(|element| count_identifier_uses(element, name))
            .sum(),
        TypedExprKind::Tuple(tuple) => tuple
            .elements
            .iter()
            .map(|element| count_identifier_uses(element, name))
            .sum(),
        TypedExprKind::Record(record) => record
            .fields
            .iter()
            .map(|field| count_identifier_uses(&field.value, name))
            .sum(),
        TypedExprKind::MapLiteral(map) => map
            .entries
            .iter()
            .map(|entry| {
                count_identifier_uses(&entry.key, name) + count_identifier_uses(&entry.value, name)
            })
            .sum(),
        TypedExprKind::FieldAccess(access) => count_identifier_uses(&access.object, name),
        TypedExprKind::Index(index) => {
            count_identifier_uses(&index.object, name) + count_identifier_uses(&index.index, name)
        }
        TypedExprKind::Map(map_expr) => {
            count_identifier_uses(&map_expr.list, name) + count_identifier_uses(&map_expr.func, name)
        }
        TypedExprKind::Filter(filter) => {
            count_identifier_uses(&filter.list, name)
                + count_identifier_uses(&filter.predicate, name)
        }
        TypedExprKind::Fold(fold) => {
            count_identifier_uses(&fold.list, name)
                + count_identifier_uses(&fold.func, name)
                + count_identifier_uses(&fold.init, name)
        }
        TypedExprKind::Pipeline(pipeline) => {
            count_identifier_uses(&pipeline.left, name) + count_identifier_uses(&pipeline.right, name)
        }
        TypedExprKind::WithMock(with_mock) => count_identifier_uses(&with_mock.body, name),
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

fn first_out_of_order_field(fields: &[String]) -> Option<(usize, &str, &str, Vec<String>)> {
    if fields.len() <= 1 {
        return None;
    }

    for i in 1..fields.len() {
        if fields[i] < fields[i - 1] {
            let mut expected_order = fields.to_vec();
            expected_order.sort();
            return Some((i, &fields[i], &fields[i - 1], expected_order));
        }
    }

    None
}

fn validate_record_type_field_ordering(
    type_name: &str,
    fields: &[Field],
    location: SourceLocation,
) -> Result<(), Vec<ValidationError>> {
    let names: Vec<String> = fields.iter().map(|field| field.name.clone()).collect();

    if let Some((index, field_name, prev_field, expected_order)) = first_out_of_order_field(&names) {
        return Err(vec![ValidationError::RecordTypeFieldOrder {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
            prev_field: prev_field.to_string(),
            position: index + 1,
            expected_order,
            location,
        }]);
    }

    Ok(())
}

fn validate_record_literal_field_ordering(
    fields: &[RecordField],
    location: SourceLocation,
) -> Result<(), Vec<ValidationError>> {
    let names: Vec<String> = fields.iter().map(|field| field.name.clone()).collect();

    if let Some((index, field_name, prev_field, expected_order)) = first_out_of_order_field(&names) {
        return Err(vec![ValidationError::RecordLiteralFieldOrder {
            field_name: field_name.to_string(),
            prev_field: prev_field.to_string(),
            position: index + 1,
            expected_order,
            location,
        }]);
    }

    Ok(())
}

fn validate_record_pattern_field_ordering(
    fields: &[RecordPatternField],
    location: SourceLocation,
) -> Result<(), Vec<ValidationError>> {
    let names: Vec<String> = fields.iter().map(|field| field.name.clone()).collect();

    if let Some((index, field_name, prev_field, expected_order)) = first_out_of_order_field(&names) {
        return Err(vec![ValidationError::RecordPatternFieldOrder {
            field_name: field_name.to_string(),
            prev_field: prev_field.to_string(),
            position: index + 1,
            expected_order,
            location,
        }]);
    }

    Ok(())
}

fn validate_pattern_record_fields(pattern: &Pattern, errors: &mut Vec<ValidationError>) {
    match pattern {
        Pattern::Literal(_) | Pattern::Identifier(_) | Pattern::Wildcard(_) => {}
        Pattern::Constructor(constructor) => {
            for nested in &constructor.patterns {
                validate_pattern_record_fields(nested, errors);
            }
        }
        Pattern::List(list) => {
            for nested in &list.patterns {
                validate_pattern_record_fields(nested, errors);
            }
        }
        Pattern::Record(record) => {
            if let Err(e) = validate_record_pattern_field_ordering(&record.fields, record.location) {
                errors.extend(e);
            }

            for field in &record.fields {
                if let Some(pattern) = &field.pattern {
                    validate_pattern_record_fields(pattern, errors);
                }
            }
        }
        Pattern::Tuple(tuple) => {
            for nested in &tuple.patterns {
                validate_pattern_record_fields(nested, errors);
            }
        }
    }
}

fn validate_expr_record_fields(expr: &Expr, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => {}
        Expr::Lambda(lambda) => validate_expr_record_fields(&lambda.body, errors),
        Expr::Application(application) => {
            validate_expr_record_fields(&application.func, errors);
            for arg in &application.args {
                validate_expr_record_fields(arg, errors);
            }
        }
        Expr::Binary(binary) => {
            validate_expr_record_fields(&binary.left, errors);
            validate_expr_record_fields(&binary.right, errors);
        }
        Expr::Unary(unary) => validate_expr_record_fields(&unary.operand, errors),
        Expr::Match(match_expr) => {
            validate_expr_record_fields(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                validate_pattern_record_fields(&arm.pattern, errors);
                if let Some(guard) = &arm.guard {
                    validate_expr_record_fields(guard, errors);
                }
                validate_expr_record_fields(&arm.body, errors);
            }
        }
        Expr::Let(let_expr) => {
            validate_pattern_record_fields(&let_expr.pattern, errors);
            validate_expr_record_fields(&let_expr.value, errors);
            validate_expr_record_fields(&let_expr.body, errors);
        }
        Expr::If(if_expr) => {
            validate_expr_record_fields(&if_expr.condition, errors);
            validate_expr_record_fields(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                validate_expr_record_fields(else_branch, errors);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                validate_expr_record_fields(element, errors);
            }
        }
        Expr::Record(record) => {
            if let Err(e) = validate_record_literal_field_ordering(&record.fields, record.location) {
                errors.extend(e);
            }

            for field in &record.fields {
                validate_expr_record_fields(&field.value, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_expr_record_fields(&entry.key, errors);
                validate_expr_record_fields(&entry.value, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_expr_record_fields(element, errors);
            }
        }
        Expr::FieldAccess(field_access) => validate_expr_record_fields(&field_access.object, errors),
        Expr::Index(index) => {
            validate_expr_record_fields(&index.object, errors);
            validate_expr_record_fields(&index.index, errors);
        }
        Expr::Pipeline(pipeline) => {
            validate_expr_record_fields(&pipeline.left, errors);
            validate_expr_record_fields(&pipeline.right, errors);
        }
        Expr::Map(map) => {
            validate_expr_record_fields(&map.list, errors);
            validate_expr_record_fields(&map.func, errors);
        }
        Expr::Filter(filter) => {
            validate_expr_record_fields(&filter.list, errors);
            validate_expr_record_fields(&filter.predicate, errors);
        }
        Expr::Fold(fold) => {
            validate_expr_record_fields(&fold.list, errors);
            validate_expr_record_fields(&fold.func, errors);
            validate_expr_record_fields(&fold.init, errors);
        }
        Expr::WithMock(with_mock) => {
            validate_expr_record_fields(&with_mock.target, errors);
            validate_expr_record_fields(&with_mock.replacement, errors);
            validate_expr_record_fields(&with_mock.body, errors);
        }
        Expr::TypeAscription(type_ascription) => validate_expr_record_fields(&type_ascription.expr, errors),
    }
}

fn validate_record_field_ordering(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Type(type_decl) => {
                if let TypeDef::Product(product) = &type_decl.definition {
                    if let Err(e) = validate_record_type_field_ordering(&type_decl.name, &product.fields, product.location) {
                        errors.extend(e);
                    }
                }
            }
            Declaration::Function(function) => validate_expr_record_fields(&function.body, &mut errors),
            Declaration::Const(const_decl) => validate_expr_record_fields(&const_decl.value, &mut errors),
            Declaration::Test(test_decl) => validate_expr_record_fields(&test_decl.body, &mut errors),
            Declaration::Extern(_) | Declaration::Import(_) => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Clone, Copy)]
enum BindingKind {
    FunctionParam,
    LambdaParam,
    LocalBinding,
    PatternBinding,
    ListRestBinding,
}

impl BindingKind {
    fn as_str(&self) -> &'static str {
        match self {
            BindingKind::FunctionParam => "function parameter",
            BindingKind::LambdaParam => "lambda parameter",
            BindingKind::LocalBinding => "local binding",
            BindingKind::PatternBinding => "pattern binding",
            BindingKind::ListRestBinding => "list rest binding",
        }
    }
}

#[derive(Clone, Copy)]
struct BindingInfo {
    kind: BindingKind,
    location: SourceLocation,
}

type ScopeFrame = HashMap<String, BindingInfo>;

fn first_existing_binding(name: &str, local: &ScopeFrame, scopes: &[ScopeFrame]) -> Option<BindingInfo> {
    if let Some(info) = local.get(name) {
        return Some(*info);
    }

    for scope in scopes.iter().rev() {
        if let Some(info) = scope.get(name) {
            return Some(*info);
        }
    }

    None
}

fn try_bind_name(
    name: &str,
    kind: BindingKind,
    location: SourceLocation,
    local: &mut ScopeFrame,
    scopes: &[ScopeFrame],
    errors: &mut Vec<ValidationError>,
) {
    if let Some(previous) = first_existing_binding(name, local, scopes) {
        errors.push(ValidationError::NoShadowing {
            name: name.to_string(),
            current_kind: kind.as_str().to_string(),
            previous_kind: previous.kind.as_str().to_string(),
            location,
            previous_location: previous.location,
            previous_line: previous.location.start.line,
            previous_column: previous.location.start.column,
        });
        return;
    }

    local.insert(name.to_string(), BindingInfo { kind, location });
}

fn validate_pattern_no_shadowing(
    pattern: &Pattern,
    local: &mut ScopeFrame,
    scopes: &[ScopeFrame],
    errors: &mut Vec<ValidationError>,
) {
    match pattern {
        Pattern::Literal(_) | Pattern::Wildcard(_) => {}
        Pattern::Identifier(identifier) => {
            try_bind_name(
                &identifier.name,
                BindingKind::PatternBinding,
                identifier.location,
                local,
                scopes,
                errors,
            );
        }
        Pattern::Constructor(constructor) => {
            for nested in &constructor.patterns {
                validate_pattern_no_shadowing(nested, local, scopes, errors);
            }
        }
        Pattern::List(list) => {
            for nested in &list.patterns {
                validate_pattern_no_shadowing(nested, local, scopes, errors);
            }
            if let Some(rest) = &list.rest {
                try_bind_name(
                    rest,
                    BindingKind::ListRestBinding,
                    list.location,
                    local,
                    scopes,
                    errors,
                );
            }
        }
        Pattern::Record(record) => {
            let mut shorthand_seen = HashSet::new();

            for field in &record.fields {
                if let Some(pattern) = &field.pattern {
                    validate_pattern_no_shadowing(pattern, local, scopes, errors);
                } else if shorthand_seen.insert(field.name.clone()) {
                    try_bind_name(
                        &field.name,
                        BindingKind::PatternBinding,
                        field.location,
                        local,
                        scopes,
                        errors,
                    );
                }
            }
        }
        Pattern::Tuple(tuple) => {
            for nested in &tuple.patterns {
                validate_pattern_no_shadowing(nested, local, scopes, errors);
            }
        }
    }
}

fn validate_expr_no_shadowing(expr: &Expr, scopes: &mut Vec<ScopeFrame>, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => {}
        Expr::Lambda(lambda) => {
            let mut local = ScopeFrame::new();
            for param in &lambda.params {
                try_bind_name(
                    &param.name,
                    BindingKind::LambdaParam,
                    param.location,
                    &mut local,
                    scopes,
                    errors,
                );
            }
            scopes.push(local);
            validate_expr_no_shadowing(&lambda.body, scopes, errors);
            scopes.pop();
        }
        Expr::Application(application) => {
            validate_expr_no_shadowing(&application.func, scopes, errors);
            for arg in &application.args {
                validate_expr_no_shadowing(arg, scopes, errors);
            }
        }
        Expr::Binary(binary) => {
            validate_expr_no_shadowing(&binary.left, scopes, errors);
            validate_expr_no_shadowing(&binary.right, scopes, errors);
        }
        Expr::Unary(unary) => validate_expr_no_shadowing(&unary.operand, scopes, errors),
        Expr::Match(match_expr) => {
            validate_expr_no_shadowing(&match_expr.scrutinee, scopes, errors);
            for arm in &match_expr.arms {
                let mut local = ScopeFrame::new();
                validate_pattern_no_shadowing(&arm.pattern, &mut local, scopes, errors);
                scopes.push(local);
                if let Some(guard) = &arm.guard {
                    validate_expr_no_shadowing(guard, scopes, errors);
                }
                validate_expr_no_shadowing(&arm.body, scopes, errors);
                scopes.pop();
            }
        }
        Expr::Let(let_expr) => {
            validate_expr_no_shadowing(&let_expr.value, scopes, errors);
            let mut local = ScopeFrame::new();
            match &let_expr.pattern {
                Pattern::Identifier(identifier) => {
                    try_bind_name(
                        &identifier.name,
                        BindingKind::LocalBinding,
                        identifier.location,
                        &mut local,
                        scopes,
                        errors,
                    );
                }
                _ => validate_pattern_no_shadowing(&let_expr.pattern, &mut local, scopes, errors),
            }
            scopes.push(local);
            validate_expr_no_shadowing(&let_expr.body, scopes, errors);
            scopes.pop();
        }
        Expr::If(if_expr) => {
            validate_expr_no_shadowing(&if_expr.condition, scopes, errors);
            validate_expr_no_shadowing(&if_expr.then_branch, scopes, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                validate_expr_no_shadowing(else_branch, scopes, errors);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                validate_expr_no_shadowing(element, scopes, errors);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                validate_expr_no_shadowing(&field.value, scopes, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_expr_no_shadowing(&entry.key, scopes, errors);
                validate_expr_no_shadowing(&entry.value, scopes, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_expr_no_shadowing(element, scopes, errors);
            }
        }
        Expr::FieldAccess(field_access) => validate_expr_no_shadowing(&field_access.object, scopes, errors),
        Expr::Index(index) => {
            validate_expr_no_shadowing(&index.object, scopes, errors);
            validate_expr_no_shadowing(&index.index, scopes, errors);
        }
        Expr::Pipeline(pipeline) => {
            validate_expr_no_shadowing(&pipeline.left, scopes, errors);
            validate_expr_no_shadowing(&pipeline.right, scopes, errors);
        }
        Expr::Map(map) => {
            validate_expr_no_shadowing(&map.list, scopes, errors);
            validate_expr_no_shadowing(&map.func, scopes, errors);
        }
        Expr::Filter(filter) => {
            validate_expr_no_shadowing(&filter.list, scopes, errors);
            validate_expr_no_shadowing(&filter.predicate, scopes, errors);
        }
        Expr::Fold(fold) => {
            validate_expr_no_shadowing(&fold.list, scopes, errors);
            validate_expr_no_shadowing(&fold.func, scopes, errors);
            validate_expr_no_shadowing(&fold.init, scopes, errors);
        }
        Expr::WithMock(with_mock) => {
            validate_expr_no_shadowing(&with_mock.target, scopes, errors);
            validate_expr_no_shadowing(&with_mock.replacement, scopes, errors);
            validate_expr_no_shadowing(&with_mock.body, scopes, errors);
        }
        Expr::TypeAscription(type_ascription) => validate_expr_no_shadowing(&type_ascription.expr, scopes, errors),
    }
}

fn validate_no_shadowing(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Function(function) => {
                let mut function_scope = ScopeFrame::new();
                for param in &function.params {
                    try_bind_name(
                        &param.name,
                        BindingKind::FunctionParam,
                        param.location,
                        &mut function_scope,
                        &[],
                        &mut errors,
                    );
                }

                let mut scopes = vec![function_scope];
                validate_expr_no_shadowing(&function.body, &mut scopes, &mut errors);
            }
            Declaration::Const(const_decl) => {
                let mut scopes = Vec::new();
                validate_expr_no_shadowing(&const_decl.value, &mut scopes, &mut errors);
            }
            Declaration::Test(test_decl) => {
                let mut scopes = Vec::new();
                validate_expr_no_shadowing(&test_decl.body, &mut scopes, &mut errors);
            }
            Declaration::Type(_) | Declaration::Extern(_) | Declaration::Import(_) => {}
        }
    }

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
                let name = module_path.join("::");
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
                let path = module_path.join("::");
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
            message: "Test files must have λmain()=>Unit=()\n\nHint: Test files are executables".to_string(),
        }]);
    }

    Ok(())
}

/// Validate filename format - lowerCamelCase only
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

    if basename
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        return Err(vec![ValidationError::FilenameCase {
            filename: file_path.to_string(),
            basename: basename.to_string(),
            suggested: format!("{}.{{sigil,lib.sigil}}", to_lower_camel_case(basename)),
            location,
        }]);
    }

    if basename.is_empty() {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: "Filename cannot be empty".to_string(),
            location,
        }]);
    }

    if basename.starts_with(|ch: char| !ch.is_ascii_lowercase()) {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: format!(
                "Filenames must start with a lowercase ASCII letter.\nSuggested: {}",
                to_lower_camel_case(basename)
            ),
            location,
        }]);
    }

    if basename.contains('_') || basename.contains('-') {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: format!(
                "Filenames must be lowerCamelCase with no underscores or hyphens.\nSuggested: {}",
                to_lower_camel_case(basename)
            ),
            location,
        }]);
    }

    if !basename.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: format!(
                "Filenames may only contain ASCII letters and digits.\nSuggested: {}",
                to_lower_camel_case(basename)
            ),
            location,
        }]);
    }

    if !is_lower_camel_case(basename) {
        return Err(vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: format!(
                "Filenames must be lowerCamelCase.\nSuggested: {}",
                to_lower_camel_case(basename)
            ),
            location,
        }]);
    }

    Ok(())
}

fn validate_identifier_forms_in_type(ty: &Type, errors: &mut Vec<ValidationError>) {
    match ty {
        Type::Primitive(_) => {}
        Type::List(list) => validate_identifier_forms_in_type(&list.element_type, errors),
        Type::Map(map) => {
            validate_identifier_forms_in_type(&map.key_type, errors);
            validate_identifier_forms_in_type(&map.value_type, errors);
        }
        Type::Function(function) => {
            for param in &function.param_types {
                validate_identifier_forms_in_type(param, errors);
            }
            validate_identifier_forms_in_type(&function.return_type, errors);
        }
        Type::Constructor(constructor) => {
            if !is_upper_camel_case(&constructor.name) {
                errors.push(ValidationError::TypeNameForm {
                    found: constructor.name.clone(),
                    suggestion: suggestion_suffix(
                        &constructor.name,
                        to_upper_camel_case(&constructor.name),
                    ),
                    location: constructor.location,
                });
            }
            for arg in &constructor.type_args {
                validate_identifier_forms_in_type(arg, errors);
            }
        }
        Type::Variable(variable) => {
            if !is_upper_camel_case(&variable.name) {
                errors.push(ValidationError::TypeVarForm {
                    found: variable.name.clone(),
                    suggestion: suggestion_suffix(
                        &variable.name,
                        to_upper_camel_case(&variable.name),
                    ),
                    location: variable.location,
                });
            }
        }
        Type::Tuple(tuple) => {
            for item in &tuple.types {
                validate_identifier_forms_in_type(item, errors);
            }
        }
        Type::Qualified(qualified) => {
            for segment in &qualified.module_path {
                if !is_lower_camel_case(segment) {
                    errors.push(ValidationError::ModulePathForm {
                        found: segment.clone(),
                        suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                        location: qualified.location,
                    });
                }
            }
            if !is_upper_camel_case(&qualified.type_name) {
                errors.push(ValidationError::TypeNameForm {
                    found: qualified.type_name.clone(),
                    suggestion: suggestion_suffix(
                        &qualified.type_name,
                        to_upper_camel_case(&qualified.type_name),
                    ),
                    location: qualified.location,
                });
            }
            for arg in &qualified.type_args {
                validate_identifier_forms_in_type(arg, errors);
            }
        }
    }
}

fn validate_identifier_forms_in_pattern(pattern: &Pattern, errors: &mut Vec<ValidationError>) {
    match pattern {
        Pattern::Literal(_) | Pattern::Wildcard(_) => {}
        Pattern::Identifier(identifier) => {
            if !is_lower_camel_case(&identifier.name) {
                errors.push(ValidationError::IdentifierForm {
                    found: identifier.name.clone(),
                    suggestion: suggestion_suffix(
                        &identifier.name,
                        to_lower_camel_case(&identifier.name),
                    ),
                    location: identifier.location,
                });
            }
        }
        Pattern::Constructor(constructor) => {
            for segment in &constructor.module_path {
                if !is_lower_camel_case(segment) {
                    errors.push(ValidationError::ModulePathForm {
                        found: segment.clone(),
                        suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                        location: constructor.location,
                    });
                }
            }
            if !is_upper_camel_case(&constructor.name) {
                errors.push(ValidationError::ConstructorNameForm {
                    found: constructor.name.clone(),
                    suggestion: suggestion_suffix(
                        &constructor.name,
                        to_upper_camel_case(&constructor.name),
                    ),
                    location: constructor.location,
                });
            }
            for nested in &constructor.patterns {
                validate_identifier_forms_in_pattern(nested, errors);
            }
        }
        Pattern::List(list) => {
            for nested in &list.patterns {
                validate_identifier_forms_in_pattern(nested, errors);
            }
            if let Some(rest) = &list.rest {
                if !is_lower_camel_case(rest) {
                    errors.push(ValidationError::IdentifierForm {
                        found: rest.clone(),
                        suggestion: suggestion_suffix(rest, to_lower_camel_case(rest)),
                        location: list.location,
                    });
                }
            }
        }
        Pattern::Record(record) => {
            for field in &record.fields {
                if !is_lower_camel_case(&field.name) {
                    errors.push(ValidationError::RecordFieldForm {
                        found: field.name.clone(),
                        suggestion: suggestion_suffix(
                            &field.name,
                            to_lower_camel_case(&field.name),
                        ),
                        location: field.location,
                    });
                }
                if let Some(pattern) = &field.pattern {
                    validate_identifier_forms_in_pattern(pattern, errors);
                }
            }
        }
        Pattern::Tuple(tuple) => {
            for nested in &tuple.patterns {
                validate_identifier_forms_in_pattern(nested, errors);
            }
        }
    }
}

fn validate_identifier_forms_in_expr(expr: &Expr, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Identifier(identifier) => {
            let first = identifier.name.chars().next();
            let valid = match first {
                Some(ch) if ch.is_ascii_uppercase() => is_upper_camel_case(&identifier.name),
                _ => is_lower_camel_case(&identifier.name),
            };

            if !valid {
                let use_upper = matches!(first, Some(ch) if ch.is_ascii_uppercase());
                if use_upper {
                    errors.push(ValidationError::ConstructorNameForm {
                        found: identifier.name.clone(),
                        suggestion: suggestion_suffix(
                            &identifier.name,
                            to_upper_camel_case(&identifier.name),
                        ),
                        location: identifier.location,
                    });
                } else {
                    errors.push(ValidationError::IdentifierForm {
                        found: identifier.name.clone(),
                        suggestion: suggestion_suffix(
                            &identifier.name,
                            to_lower_camel_case(&identifier.name),
                        ),
                        location: identifier.location,
                    });
                }
            }
        }
        Expr::Lambda(lambda) => {
            for param in &lambda.params {
                if !is_lower_camel_case(&param.name) {
                    errors.push(ValidationError::IdentifierForm {
                        found: param.name.clone(),
                        suggestion: suggestion_suffix(&param.name, to_lower_camel_case(&param.name)),
                        location: param.location,
                    });
                }
                if let Some(type_annotation) = &param.type_annotation {
                    validate_identifier_forms_in_type(type_annotation, errors);
                }
            }
            validate_identifier_forms_in_type(&lambda.return_type, errors);
            validate_identifier_forms_in_expr(&lambda.body, errors);
        }
        Expr::Application(application) => {
            validate_identifier_forms_in_expr(&application.func, errors);
            for arg in &application.args {
                validate_identifier_forms_in_expr(arg, errors);
            }
        }
        Expr::Binary(binary) => {
            validate_identifier_forms_in_expr(&binary.left, errors);
            validate_identifier_forms_in_expr(&binary.right, errors);
        }
        Expr::Unary(unary) => validate_identifier_forms_in_expr(&unary.operand, errors),
        Expr::Match(match_expr) => {
            validate_identifier_forms_in_expr(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                validate_identifier_forms_in_pattern(&arm.pattern, errors);
                if let Some(guard) = &arm.guard {
                    validate_identifier_forms_in_expr(guard, errors);
                }
                validate_identifier_forms_in_expr(&arm.body, errors);
            }
        }
        Expr::Let(let_expr) => {
            validate_identifier_forms_in_pattern(&let_expr.pattern, errors);
            validate_identifier_forms_in_expr(&let_expr.value, errors);
            validate_identifier_forms_in_expr(&let_expr.body, errors);
        }
        Expr::If(if_expr) => {
            validate_identifier_forms_in_expr(&if_expr.condition, errors);
            validate_identifier_forms_in_expr(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                validate_identifier_forms_in_expr(else_branch, errors);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                validate_identifier_forms_in_expr(element, errors);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                if !is_lower_camel_case(&field.name) {
                    errors.push(ValidationError::RecordFieldForm {
                        found: field.name.clone(),
                        suggestion: suggestion_suffix(&field.name, to_lower_camel_case(&field.name)),
                        location: field.location,
                    });
                }
                validate_identifier_forms_in_expr(&field.value, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_identifier_forms_in_expr(&entry.key, errors);
                validate_identifier_forms_in_expr(&entry.value, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_identifier_forms_in_expr(element, errors);
            }
        }
        Expr::FieldAccess(field_access) => {
            validate_identifier_forms_in_expr(&field_access.object, errors);
            if !is_lower_camel_case(&field_access.field) {
                errors.push(ValidationError::RecordFieldForm {
                    found: field_access.field.clone(),
                    suggestion: suggestion_suffix(
                        &field_access.field,
                        to_lower_camel_case(&field_access.field),
                    ),
                    location: field_access.location,
                });
            }
        }
        Expr::Index(index) => {
            validate_identifier_forms_in_expr(&index.object, errors);
            validate_identifier_forms_in_expr(&index.index, errors);
        }
        Expr::Pipeline(pipeline) => {
            validate_identifier_forms_in_expr(&pipeline.left, errors);
            validate_identifier_forms_in_expr(&pipeline.right, errors);
        }
        Expr::Map(map) => {
            validate_identifier_forms_in_expr(&map.list, errors);
            validate_identifier_forms_in_expr(&map.func, errors);
        }
        Expr::Filter(filter) => {
            validate_identifier_forms_in_expr(&filter.list, errors);
            validate_identifier_forms_in_expr(&filter.predicate, errors);
        }
        Expr::Fold(fold) => {
            validate_identifier_forms_in_expr(&fold.list, errors);
            validate_identifier_forms_in_expr(&fold.func, errors);
            validate_identifier_forms_in_expr(&fold.init, errors);
        }
        Expr::MemberAccess(member_access) => {
            for segment in &member_access.namespace {
                if !is_lower_camel_case(segment) {
                    errors.push(ValidationError::ModulePathForm {
                        found: segment.clone(),
                        suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                        location: member_access.location,
                    });
                }
            }
            let first = member_access.member.chars().next();
            let valid = match first {
                Some(ch) if ch.is_ascii_uppercase() => is_upper_camel_case(&member_access.member),
                _ => is_lower_camel_case(&member_access.member),
            };
            if !valid {
                let use_upper = matches!(first, Some(ch) if ch.is_ascii_uppercase());
                if use_upper {
                    errors.push(ValidationError::ConstructorNameForm {
                        found: member_access.member.clone(),
                        suggestion: suggestion_suffix(
                            &member_access.member,
                            to_upper_camel_case(&member_access.member),
                        ),
                        location: member_access.location,
                    });
                } else {
                    errors.push(ValidationError::IdentifierForm {
                        found: member_access.member.clone(),
                        suggestion: suggestion_suffix(
                            &member_access.member,
                            to_lower_camel_case(&member_access.member),
                        ),
                        location: member_access.location,
                    });
                }
            }
        }
        Expr::WithMock(with_mock) => {
            validate_identifier_forms_in_expr(&with_mock.target, errors);
            validate_identifier_forms_in_expr(&with_mock.replacement, errors);
            validate_identifier_forms_in_expr(&with_mock.body, errors);
        }
        Expr::TypeAscription(type_ascription) => {
            validate_identifier_forms_in_expr(&type_ascription.expr, errors);
            validate_identifier_forms_in_type(&type_ascription.ascribed_type, errors);
        }
    }
}

fn validate_naming_forms(program: &Program, errors: &mut Vec<ValidationError>) {
    for decl in &program.declarations {
        match decl {
            Declaration::Function(function) => {
                if !is_lower_camel_case(&function.name) {
                    errors.push(ValidationError::IdentifierForm {
                        found: function.name.clone(),
                        suggestion: suggestion_suffix(
                            &function.name,
                            to_lower_camel_case(&function.name),
                        ),
                        location: function.location,
                    });
                }
                for type_param in &function.type_params {
                    if !is_upper_camel_case(type_param) {
                        errors.push(ValidationError::TypeVarForm {
                            found: type_param.clone(),
                            suggestion: suggestion_suffix(
                                type_param,
                                to_upper_camel_case(type_param),
                            ),
                            location: function.location,
                        });
                    }
                }
                for param in &function.params {
                    if !is_lower_camel_case(&param.name) {
                        errors.push(ValidationError::IdentifierForm {
                            found: param.name.clone(),
                            suggestion: suggestion_suffix(
                                &param.name,
                                to_lower_camel_case(&param.name),
                            ),
                            location: param.location,
                        });
                    }
                    if let Some(type_annotation) = &param.type_annotation {
                        validate_identifier_forms_in_type(type_annotation, errors);
                    }
                }
                if let Some(return_type) = &function.return_type {
                    validate_identifier_forms_in_type(return_type, errors);
                }
                validate_identifier_forms_in_expr(&function.body, errors);
            }
            Declaration::Type(type_decl) => {
                if !is_upper_camel_case(&type_decl.name) {
                    errors.push(ValidationError::TypeNameForm {
                        found: type_decl.name.clone(),
                        suggestion: suggestion_suffix(
                            &type_decl.name,
                            to_upper_camel_case(&type_decl.name),
                        ),
                        location: type_decl.location,
                    });
                }
                for type_param in &type_decl.type_params {
                    if !is_upper_camel_case(type_param) {
                        errors.push(ValidationError::TypeVarForm {
                            found: type_param.clone(),
                            suggestion: suggestion_suffix(
                                type_param,
                                to_upper_camel_case(type_param),
                            ),
                            location: type_decl.location,
                        });
                    }
                }
                match &type_decl.definition {
                    TypeDef::Sum(sum) => {
                        for variant in &sum.variants {
                            if !is_upper_camel_case(&variant.name) {
                                errors.push(ValidationError::ConstructorNameForm {
                                    found: variant.name.clone(),
                                    suggestion: suggestion_suffix(
                                        &variant.name,
                                        to_upper_camel_case(&variant.name),
                                    ),
                                    location: variant.location,
                                });
                            }
                            for ty in &variant.types {
                                validate_identifier_forms_in_type(ty, errors);
                            }
                        }
                    }
                    TypeDef::Product(product) => {
                        for field in &product.fields {
                            if !is_lower_camel_case(&field.name) {
                                errors.push(ValidationError::RecordFieldForm {
                                    found: field.name.clone(),
                                    suggestion: suggestion_suffix(
                                        &field.name,
                                        to_lower_camel_case(&field.name),
                                    ),
                                    location: field.location,
                                });
                            }
                            validate_identifier_forms_in_type(&field.field_type, errors);
                        }
                    }
                    TypeDef::Alias(alias) => {
                        validate_identifier_forms_in_type(&alias.aliased_type, errors);
                    }
                }
            }
            Declaration::Import(import_decl) => {
                for segment in &import_decl.module_path {
                    if !is_lower_camel_case(segment) {
                        errors.push(ValidationError::ModulePathForm {
                            found: segment.clone(),
                            suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                            location: import_decl.location,
                        });
                    }
                }
            }
            Declaration::Const(const_decl) => {
                if !is_lower_camel_case(&const_decl.name) {
                    errors.push(ValidationError::IdentifierForm {
                        found: const_decl.name.clone(),
                        suggestion: suggestion_suffix(
                            &const_decl.name,
                            to_lower_camel_case(&const_decl.name),
                        ),
                        location: const_decl.location,
                    });
                }
                if let Some(type_annotation) = &const_decl.type_annotation {
                    validate_identifier_forms_in_type(type_annotation, errors);
                }
                validate_identifier_forms_in_expr(&const_decl.value, errors);
            }
            Declaration::Test(test_decl) => validate_identifier_forms_in_expr(&test_decl.body, errors),
            Declaration::Extern(extern_decl) => {
                for segment in &extern_decl.module_path {
                    if !is_lower_camel_case(segment) {
                        errors.push(ValidationError::ModulePathForm {
                            found: segment.clone(),
                            suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                            location: extern_decl.location,
                        });
                    }
                }
                if let Some(members) = &extern_decl.members {
                    for member in members {
                        if !is_lower_camel_case(&member.name) {
                            errors.push(ValidationError::IdentifierForm {
                                found: member.name.clone(),
                                suggestion: suggestion_suffix(
                                    &member.name,
                                    to_lower_camel_case(&member.name),
                                ),
                                location: member.location,
                            });
                        }
                        validate_identifier_forms_in_type(&member.member_type, errors);
                    }
                }
            }
        }
    }
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

fn validate_with_mock_placement(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                collect_with_mock_outside_tests(&function.body, &mut errors);
            }
            Declaration::Const(const_decl) => {
                collect_with_mock_outside_tests(&const_decl.value, &mut errors);
            }
            Declaration::Test(test_decl) => {
                collect_with_mock_outside_tests_in_test_expr(&test_decl.body, true, &mut errors);
            }
            Declaration::Type(_)
            | Declaration::Import(_)
            | Declaration::Extern(_) => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn collect_with_mock_outside_tests(expr: &Expr, errors: &mut Vec<ValidationError>) {
    collect_with_mock_outside_tests_in_test_expr(expr, false, errors);
}

fn collect_with_mock_outside_tests_in_test_expr(
    expr: &Expr,
    in_test_body: bool,
    errors: &mut Vec<ValidationError>,
) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) => {}
        Expr::Lambda(lambda) => {
            collect_with_mock_outside_tests_in_test_expr(&lambda.body, false, errors);
        }
        Expr::Application(app) => {
            collect_with_mock_outside_tests_in_test_expr(&app.func, in_test_body, errors);
            for arg in &app.args {
                collect_with_mock_outside_tests_in_test_expr(arg, in_test_body, errors);
            }
        }
        Expr::Binary(binary) => {
            collect_with_mock_outside_tests_in_test_expr(&binary.left, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&binary.right, in_test_body, errors);
        }
        Expr::Unary(unary) => {
            collect_with_mock_outside_tests_in_test_expr(&unary.operand, in_test_body, errors);
        }
        Expr::Match(match_expr) => {
            collect_with_mock_outside_tests_in_test_expr(&match_expr.scrutinee, in_test_body, errors);
            for arm in &match_expr.arms {
                collect_with_mock_outside_tests_in_test_expr(&arm.body, in_test_body, errors);
                if let Some(guard) = &arm.guard {
                    collect_with_mock_outside_tests_in_test_expr(guard, in_test_body, errors);
                }
            }
        }
        Expr::Let(let_expr) => {
            collect_with_mock_outside_tests_in_test_expr(&let_expr.value, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&let_expr.body, in_test_body, errors);
        }
        Expr::If(if_expr) => {
            collect_with_mock_outside_tests_in_test_expr(&if_expr.condition, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&if_expr.then_branch, in_test_body, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_with_mock_outside_tests_in_test_expr(else_branch, in_test_body, errors);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                collect_with_mock_outside_tests_in_test_expr(element, in_test_body, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_with_mock_outside_tests_in_test_expr(element, in_test_body, errors);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                collect_with_mock_outside_tests_in_test_expr(&field.value, in_test_body, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                collect_with_mock_outside_tests_in_test_expr(&entry.key, in_test_body, errors);
                collect_with_mock_outside_tests_in_test_expr(&entry.value, in_test_body, errors);
            }
        }
        Expr::FieldAccess(field_access) => {
            collect_with_mock_outside_tests_in_test_expr(&field_access.object, in_test_body, errors);
        }
        Expr::Index(index) => {
            collect_with_mock_outside_tests_in_test_expr(&index.object, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&index.index, in_test_body, errors);
        }
        Expr::Map(map_expr) => {
            collect_with_mock_outside_tests_in_test_expr(&map_expr.list, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&map_expr.func, in_test_body, errors);
        }
        Expr::Filter(filter) => {
            collect_with_mock_outside_tests_in_test_expr(&filter.list, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&filter.predicate, in_test_body, errors);
        }
        Expr::Fold(fold) => {
            collect_with_mock_outside_tests_in_test_expr(&fold.list, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&fold.func, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&fold.init, in_test_body, errors);
        }
        Expr::Pipeline(pipeline) => {
            collect_with_mock_outside_tests_in_test_expr(&pipeline.left, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&pipeline.right, in_test_body, errors);
        }
        Expr::TypeAscription(type_ascription) => {
            collect_with_mock_outside_tests_in_test_expr(&type_ascription.expr, in_test_body, errors);
        }
        Expr::MemberAccess(_) => {}
        Expr::WithMock(with_mock) => {
            if !in_test_body {
                errors.push(ValidationError::WithMockTestOnly {
                    location: with_mock.location,
                });
            }
            collect_with_mock_outside_tests_in_test_expr(&with_mock.target, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&with_mock.replacement, in_test_body, errors);
            collect_with_mock_outside_tests_in_test_expr(&with_mock.body, in_test_body, errors);
        }
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

        Expr::MapLiteral(m) => {
            m.entries.iter().any(|entry| {
                is_recursive(&entry.key, function_name) || is_recursive(&entry.value, function_name)
            })
        }

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

        Expr::TypeAscription(t) => is_recursive(&t.expr, function_name),
    }
}

/// Detect parameters that might be accumulators (simplified heuristic)
fn detect_accumulator_params(_func: &FunctionDecl) -> Vec<String> {
    // Simplified: In a real implementation, we would analyze how parameters
    // are used in recursive calls to classify them as STRUCTURAL, QUERY, or ACCUMULATOR

    // For now, we'll use a simple heuristic: if a parameter appears in a binary
    // operation in a recursive call argument, it might be an accumulator

    let suspicious = Vec::new();

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
    use sigil_typechecker::type_check;

    #[test]
    fn test_no_duplicate_functions() {
        let source = r#"λbar(y:Int)=>Int=y*2
λfoo(x:Int)=>Int=x+1
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("test.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_duplicate_function_error() {
        let source = r#"λfoo(x:Int)=>Int=x+1
λfoo(y:Int)=>Int=y*2
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
        // TODO: Parser bug - match expressions with scrutinee (match n{...}) don't work yet
        // For now, test with a simple recursive function without pattern matching
        let source = r#"λfactorial(n:Int)=>Int=factorial(n-1)"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        // Should pass - simple recursion is allowed
        assert!(validate_canonical_form(&program, None, None).is_ok());
    }

    #[test]
    fn test_single_use_pure_binding_rejected() {
        let source = r#"λmain()=>String={
  l repo=(releaseRepo():String);
  {repo:repo}.repo
}

λreleaseRepo()=>String="inerte/sigil"
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        let result = validate_typed_canonical_form(&typed.typed_program);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(matches!(
            errors[0],
            ValidationError::SingleUsePureBinding { .. }
        ));
    }

    #[test]
    fn test_multi_use_pure_binding_allowed() {
        let source = r#"λmain()=>Int={
  l count=(releaseCount():Int);
  count+count
}

λreleaseCount()=>Int=2
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        assert!(validate_typed_canonical_form(&typed.typed_program).is_ok());
    }

    #[test]
    fn test_single_use_effectful_binding_allowed() {
        let source = r#"λemit()=>!IO String="x"
λmain()=>!IO String={
  l value=(emit():String);
  value
}"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        assert!(validate_typed_canonical_form(&typed.typed_program).is_ok());
    }

    #[test]
    fn test_multi_arm_match_must_be_multiline() {
        let source = r#"λfib(n:Int)=>Int match n{0=>0|1=>1|value=>fib(value-1)+fib(value-2)}
λmain()=>Int=fib(5)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err()[0], ValidationError::MatchLayout { .. }));
    }

    #[test]
    fn test_direct_match_body_canonical_layout_allowed() {
        let source = r#"λfib(n:Int)=>Int match n{
  0=>0|
  1=>1|
  value=>fib(value-1)+fib(value-2)
}
λmain()=>Int=fib(5)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_ok(), "{:?}", result.unwrap_err());
    }

    #[test]
    fn test_nested_match_arm_body_allowed() {
        let source = r#"λf(x:Int,y:Int)=>Int match x{
  0=>match y{
    0=>1|
    _=>2
  }|
  _=>3
}
λmain()=>Int=f(0,1)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_ok(), "{:?}", result.unwrap_err());
    }

    #[test]
    fn test_signature_split_across_lines_rejected() {
        let source = "λfib(n:Int)=>Int\nmatch n{0=>0}\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(error, ValidationError::SignatureLayout { .. })));
    }

}

/// Validate canonical declaration ordering
fn validate_declaration_ordering(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    
    // Check category order (type => extern => import => const => function => test)
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
            
            return Err(vec![ValidationError::DeclarationOrderOld {
                message: format!(
                    "SIGIL-CANON-DECL-CATEGORY-ORDER: Wrong category position\n\
                     Found: {} ({}) at line {}\n\
                     Category order: t => e => i => c => λ => test",
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
            return Err(vec![ValidationError::DeclarationOrderOld {
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
