//! Canonical form validation
//!
//! Enforces Sigil's "ONE WAY" principle by rejecting alternative patterns:
//! 1. No duplicate declarations
//! 2. Canonical recursive list-plumbing shapes
//! 3. Canonical pattern matching (most direct form)
//! 4. No CPS (continuation passing style)
#![allow(dead_code)]

use crate::error::ValidationError;
use crate::printer::print_canonical_program_with_effects;
use sigil_ast::*;
use sigil_lexer::{tokenize, Position, SourceLocation, Token, TokenType};
use sigil_typechecker::json_codec::derive_json_surface_info_for_type;
use sigil_typechecker::typed_ir::TypedConcurrentStep;
use sigil_typechecker::types::{ast_type_to_inference_type, InferenceType, TPrimitive};
use sigil_typechecker::{
    EffectCatalog, PurityClass, TypeInfo, TypedDeclaration, TypedExpr, TypedExprKind, TypedProgram,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ValidationOptions {
    pub effect_catalog: Option<EffectCatalog>,
}

#[derive(Debug, Clone, Default)]
pub struct TypedValidationOptions {
    pub local_type_registry: HashMap<String, TypeInfo>,
    pub imported_type_registries: HashMap<String, HashMap<String, TypeInfo>>,
    pub module_id: Option<String>,
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalHelperWrapper {
    canonical_helper: String,
    canonical_surface: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectJsonCodecSurface {
    Encode,
    Decode,
    Parse,
    Stringify,
}

impl DirectJsonCodecSurface {
    fn label(self) -> &'static str {
        match self {
            Self::Encode => "encode",
            Self::Decode => "decode",
            Self::Parse => "parse",
            Self::Stringify => "stringify",
        }
    }
}

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

fn first_source_difference(source: &str, canonical_source: &str) -> SourceLocation {
    let mut line = 1;
    let mut column = 1;
    let mut offset = 0;

    for (left, right) in source.chars().zip(canonical_source.chars()) {
        if left != right {
            return SourceLocation {
                start: Position {
                    line,
                    column,
                    offset,
                },
                end: Position {
                    line,
                    column,
                    offset,
                },
            };
        }

        offset += left.len_utf8();
        if left == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    SourceLocation {
        start: Position {
            line,
            column,
            offset,
        },
        end: Position {
            line,
            column,
            offset,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentNormalizedLineKind {
    Blank,
    Code,
    CommentOnly,
}

#[derive(Debug, Clone)]
struct CommentNormalizedLine {
    had_comment: bool,
    has_newline: bool,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentScanState {
    Code,
    Comment,
    String { escaped: bool },
    Char { escaped: bool },
}

impl CommentNormalizedLine {
    fn kind(&self) -> CommentNormalizedLineKind {
        if self.text.trim().is_empty() {
            if self.had_comment {
                CommentNormalizedLineKind::CommentOnly
            } else {
                CommentNormalizedLineKind::Blank
            }
        } else {
            CommentNormalizedLineKind::Code
        }
    }
}

fn comment_normalized_lines(source: &str) -> Vec<CommentNormalizedLine> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut line_had_comment = false;
    let mut state = CommentScanState::Code;

    let push_line =
        |lines: &mut Vec<CommentNormalizedLine>, current: &mut String, line_had_comment: bool| {
            lines.push(CommentNormalizedLine {
                had_comment: line_had_comment,
                has_newline: true,
                text: current.clone(),
            });
            current.clear();
        };

    for ch in source.chars() {
        match state {
            CommentScanState::Code => match ch {
                '⟦' => {
                    state = CommentScanState::Comment;
                    line_had_comment = true;
                }
                '"' => {
                    current.push(ch);
                    state = CommentScanState::String { escaped: false };
                }
                '\'' => {
                    current.push(ch);
                    state = CommentScanState::Char { escaped: false };
                }
                '\n' => {
                    push_line(&mut lines, &mut current, line_had_comment);
                    line_had_comment = false;
                }
                _ => current.push(ch),
            },
            CommentScanState::Comment => match ch {
                '⟧' => {
                    state = CommentScanState::Code;
                    line_had_comment = true;
                }
                '\n' => {
                    push_line(&mut lines, &mut current, true);
                    line_had_comment = true;
                }
                _ => {
                    line_had_comment = true;
                }
            },
            CommentScanState::String { escaped } => match ch {
                '\n' => {
                    push_line(&mut lines, &mut current, line_had_comment);
                    line_had_comment = false;
                    state = CommentScanState::String { escaped: false };
                }
                _ => {
                    current.push(ch);
                    state = if escaped {
                        CommentScanState::String { escaped: false }
                    } else if ch == '\\' {
                        CommentScanState::String { escaped: true }
                    } else if ch == '"' {
                        CommentScanState::Code
                    } else {
                        CommentScanState::String { escaped: false }
                    };
                }
            },
            CommentScanState::Char { escaped } => match ch {
                '\n' => {
                    push_line(&mut lines, &mut current, line_had_comment);
                    line_had_comment = false;
                    state = CommentScanState::Char { escaped: false };
                }
                _ => {
                    current.push(ch);
                    state = if escaped {
                        CommentScanState::Char { escaped: false }
                    } else if ch == '\\' {
                        CommentScanState::Char { escaped: true }
                    } else if ch == '\'' {
                        CommentScanState::Code
                    } else {
                        CommentScanState::Char { escaped: false }
                    };
                }
            },
        }
    }

    if !current.is_empty() || line_had_comment || !source.ends_with('\n') {
        lines.push(CommentNormalizedLine {
            had_comment: line_had_comment,
            has_newline: false,
            text: current,
        });
    }

    for line in &mut lines {
        if line.had_comment {
            line.text = line.text.trim_end_matches([' ', '\t']).to_string();
        }
    }

    lines
}

fn normalize_source_for_canonical_compare(source: &str) -> String {
    let lines = comment_normalized_lines(source);
    if lines.is_empty() {
        return String::new();
    }

    let mut kept = Vec::new();
    let mut idx = 0;
    while idx < lines.len() {
        match lines[idx].kind() {
            CommentNormalizedLineKind::Code => {
                kept.push(lines[idx].clone());
                idx += 1;
            }
            _ => {
                let run_start = idx;
                let mut has_comment_only = false;
                while idx < lines.len() {
                    match lines[idx].kind() {
                        CommentNormalizedLineKind::Code => break,
                        CommentNormalizedLineKind::CommentOnly => {
                            has_comment_only = true;
                            idx += 1;
                        }
                        CommentNormalizedLineKind::Blank => idx += 1,
                    }
                }

                if has_comment_only {
                    if !kept.is_empty() && idx < lines.len() {
                        kept.push(CommentNormalizedLine {
                            had_comment: false,
                            has_newline: true,
                            text: String::new(),
                        });
                    }
                } else {
                    kept.extend(lines[run_start..idx].iter().cloned());
                }
            }
        }
    }

    let mut normalized = String::new();
    for line in kept {
        normalized.push_str(&line.text);
        if line.has_newline {
            normalized.push('\n');
        }
    }
    normalized
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
                start: Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
                end: Position {
                    line: 1,
                    column: 1,
                    offset: 0,
                },
            },
        }]);
    }

    Ok(())
}

/// Validate no trailing whitespace
fn validate_no_trailing_whitespace(
    source: &str,
    file_path: &str,
) -> Result<(), Vec<ValidationError>> {
    let lines: Vec<&str> = source.split('\n').collect();

    for (i, line) in lines.iter().enumerate() {
        if line.ends_with(' ') || line.ends_with('\t') {
            return Err(vec![ValidationError::TrailingWhitespace {
                filename: file_path.to_string(),
                line: i + 1,
                location: SourceLocation {
                    start: Position {
                        line: i + 1,
                        column: 1,
                        offset: 0,
                    },
                    end: Position {
                        line: i + 1,
                        column: 1,
                        offset: 0,
                    },
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
                    start: Position {
                        line: i + 2,
                        column: 1,
                        offset: 0,
                    },
                    end: Position {
                        line: i + 2,
                        column: 1,
                        offset: 0,
                    },
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
        && contains_space_outside_comments(slice_between(
            source,
            left.location.end.offset,
            right.location.start.offset,
        ))
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
        Expr::Using(expr) => expr.location,
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
        Expr::Concurrent(expr) => expr.location,
        Expr::MemberAccess(expr) => expr.location,
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
    matches!(
        token_type,
        TokenType::LPAREN | TokenType::LBRACKET | TokenType::LBRACE
    )
}

fn token_is_close_delimiter(token_type: TokenType) -> bool {
    matches!(
        token_type,
        TokenType::RPAREN | TokenType::RBRACKET | TokenType::RBRACE
    )
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
    let tokens = tokenize(source).map_err(|error| {
        vec![ValidationError::FilenameFormat {
            filename: file_path.to_string(),
            message: error.to_string(),
            location: SourceLocation::new(Position::new(1, 1, 0), Position::new(1, 1, 0)),
        }]
    })?;

    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|token| {
            token.token_type != TokenType::NEWLINE && token.token_type != TokenType::EOF
        })
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

        if token_forbids_surrounding_spaces(right.token_type) && has_space_gap(source, left, right)
        {
            return Err(vec![ValidationError::OperatorSpacing {
                location: SourceLocation::new(left.location.end, right.location.end),
            }]);
        }
    }

    Ok(())
}

fn validate_function_body_layout(
    function: &FunctionDecl,
    source: &str,
) -> Result<(), Vec<ValidationError>> {
    let body_location = expr_location(&function.body);

    if matches!(function.body, Expr::Match(_)) {
        let between = slice_between(
            source,
            type_location(function.return_type.as_ref().unwrap())
                .end
                .offset,
            body_location.start.offset,
        );
        if between.contains('{') {
            return Err(vec![ValidationError::MatchBodyBlock {
                location: function.location,
            }]);
        }
    }

    Ok(())
}

fn validate_lambda_body_layout(
    lambda: &LambdaExpr,
    source: &str,
) -> Result<(), Vec<ValidationError>> {
    let body_location = expr_location(&lambda.body);

    if matches!(lambda.body, Expr::Match(_)) {
        let between = slice_between(
            source,
            type_location(&lambda.return_type).end.offset,
            body_location.start.offset,
        );
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

fn validate_redundant_parens_in_body(
    expr: &Expr,
    source: &str,
) -> Result<(), Vec<ValidationError>> {
    let can_be_meaningfully_wrapped = matches!(
        expr,
        Expr::Application(_)
            | Expr::Binary(_)
            | Expr::Unary(_)
            | Expr::Match(_)
            | Expr::Let(_)
            | Expr::Using(_)
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
            | Expr::Concurrent(_)
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
        Expr::Using(using_expr) => {
            validate_expr_layout(&using_expr.value, source, errors);
            validate_expr_layout(&using_expr.body, source, errors);
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
        Expr::Concurrent(concurrent) => {
            validate_expr_layout(&concurrent.width, source, errors);
            if let Some(policy) = &concurrent.policy {
                for field in &policy.fields {
                    validate_expr_layout(&field.value, source, errors);
                }
            }
            for step in &concurrent.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => {
                        validate_expr_layout(&spawn.expr, source, errors)
                    }
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_expr_layout(&spawn_each.list, source, errors);
                        validate_expr_layout(&spawn_each.func, source, errors);
                    }
                }
            }
        }
        Expr::TypeAscription(ascription) => validate_expr_layout(&ascription.expr, source, errors),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => {}
    }
}

fn validate_source_layout(
    program: &Program,
    source: &str,
    file_path: &str,
) -> Result<(), Vec<ValidationError>> {
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
            Declaration::FeatureFlag(feature_flag_decl) => {
                validate_expr_layout(&feature_flag_decl.default, source, &mut errors);
            }
            Declaration::Test(test_decl) => {
                validate_expr_layout(&test_decl.body, source, &mut errors);
            }
            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate that a program follows canonical form rules
pub fn validate_canonical_form(
    program: &Program,
    file_path: Option<&str>,
    source: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    validate_canonical_form_with_options(program, file_path, source, ValidationOptions::default())
}

pub fn validate_canonical_form_with_options(
    program: &Program,
    file_path: Option<&str>,
    source: Option<&str>,
    options: ValidationOptions,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    if let (Some(_path), Some(src)) = (file_path, source) {
        let normalized_source = normalize_source_for_canonical_compare(src);
        let canonical_source =
            print_canonical_program_with_effects(program, options.effect_catalog.as_ref());
        if normalized_source != canonical_source {
            let location = first_source_difference(&normalized_source, &canonical_source);
            errors.push(ValidationError::SourceForm {
                canonical_source,
                location,
            });
        }
    }

    // Rule 1: No duplicate declarations
    if let Err(e) = validate_no_duplicates(program) {
        errors.extend(e);
    }

    if let Err(e) = validate_effect_declaration_placement(program, file_path) {
        errors.extend(e);
    }

    if let Err(e) = validate_project_type_declaration_placement(program, file_path) {
        errors.extend(e);
    }

    if let Err(e) = validate_project_policy_declaration_placement(program, file_path) {
        errors.extend(e);
    }

    if let Err(e) = validate_project_feature_flag_declaration_placement(program, file_path) {
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

    // Rule 6: Recursive functions must not use accumulators
    if let Err(e) = validate_direct_canonical_helper_wrappers(program, file_path) {
        errors.extend(e);
    }

    // Rule 6b: Recursive functions must not use accumulators
    if let Err(e) = validate_recursive_functions(program) {
        errors.extend(e);
    }

    // Rule 6c: No non-canonical branching self-recursion
    if let Err(e) = crate::branching_recursion::validate_branching_recursion(program) {
        errors.extend(e);
    }

    // Rule 6d: No mutual recursion (per-file scope)
    if let Err(e) = validate_mutual_recursion(program) {
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

    // Rule 11: No dead executable externs/declarations
    if let Err(e) = validate_no_unused_items(program, file_path) {
        errors.extend(e);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate canonical rules that require typed purity/effect information.
pub fn validate_typed_canonical_form(
    program: &TypedProgram,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    validate_typed_canonical_form_with_options(
        program,
        file_path,
        TypedValidationOptions::default(),
    )
}

pub fn validate_typed_canonical_form_with_options(
    program: &TypedProgram,
    file_path: Option<&str>,
    options: TypedValidationOptions,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for declaration in &program.declarations {
        match declaration {
            TypedDeclaration::Function(function) => {
                collect_unused_named_bindings(&function.body, &mut errors);
                collect_single_use_pure_bindings(&function.body, &mut errors);
                collect_dead_pure_discards(&function.body, &mut errors);
            }
            TypedDeclaration::Const(const_decl) => {
                collect_unused_named_bindings(&const_decl.value, &mut errors);
                collect_single_use_pure_bindings(&const_decl.value, &mut errors);
                collect_dead_pure_discards(&const_decl.value, &mut errors);
            }
            TypedDeclaration::Test(test_decl) => {
                collect_unused_named_bindings(&test_decl.body, &mut errors);
                collect_single_use_pure_bindings(&test_decl.body, &mut errors);
                collect_dead_pure_discards(&test_decl.body, &mut errors);
            }
            TypedDeclaration::Type(_)
            | TypedDeclaration::JsonCodec(_)
            | TypedDeclaration::Extern(_) => {}
        }
    }

    collect_direct_json_codec_surfaces(program, file_path, &options, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn collect_direct_json_codec_surfaces(
    program: &TypedProgram,
    file_path: Option<&str>,
    options: &TypedValidationOptions,
    errors: &mut Vec<ValidationError>,
) {
    for declaration in &program.declarations {
        match declaration {
            TypedDeclaration::Function(function) => {
                let Some(param_types) = function
                    .params
                    .iter()
                    .map(|param| param.type_annotation.as_ref().map(ast_type_to_inference_type))
                    .collect::<Option<Vec<_>>>()
                else {
                    continue;
                };
                let param_names = function
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                if let Some(error) = direct_json_codec_error_for_surface(
                    &function.name,
                    &param_types,
                    &function.return_type,
                    &function.body,
                    &param_names,
                    function.location,
                    file_path,
                    options,
                ) {
                    errors.push(error);
                }
            }
            TypedDeclaration::Const(const_decl) => {
                let InferenceType::Function(function_type) = &const_decl.typ else {
                    continue;
                };
                let param_names = match &const_decl.value.kind {
                    TypedExprKind::Lambda(lambda) => {
                        lambda.params.iter().map(|param| param.name.clone()).collect()
                    }
                    _ => Vec::new(),
                };
                if let Some(error) = direct_json_codec_error_for_surface(
                    &const_decl.name,
                    &function_type.params,
                    &function_type.return_type,
                    &const_decl.value,
                    &param_names,
                    const_decl.location,
                    file_path,
                    options,
                ) {
                    errors.push(error);
                }
            }
            TypedDeclaration::Type(_)
            | TypedDeclaration::JsonCodec(_)
            | TypedDeclaration::Test(_)
            | TypedDeclaration::Extern(_) => {}
        }
    }
}

fn direct_json_codec_error_for_surface(
    declaration_name: &str,
    params: &[InferenceType],
    return_type: &InferenceType,
    body: &TypedExpr,
    wrapper_param_names: &[String],
    location: SourceLocation,
    file_path: Option<&str>,
    options: &TypedValidationOptions,
) -> Option<ValidationError> {
    if params.len() != 1 {
        return None;
    }

    let source_file = options.source_file.as_deref().or(file_path);

    if is_json_value_type(return_type) {
        let surface_info = derive_json_surface_info_for_type(
            &params[0],
            options.module_id.as_deref(),
            source_file,
            &options.local_type_registry,
            &options.imported_type_registries,
        )
        .ok()?;
        return Some(ValidationError::DirectJsonCodec {
            declaration_name: declaration_name.to_string(),
            target_name: surface_info.target_name.clone(),
            surface_kind: DirectJsonCodecSurface::Encode.label().to_string(),
            encode_helper: surface_info.helper_names.encode,
            decode_helper: surface_info.helper_names.decode,
            parse_helper: surface_info.helper_names.parse,
            stringify_helper: surface_info.helper_names.stringify,
            location,
        });
    }

    if is_string_type(return_type) {
        let surface_info = derive_json_surface_info_for_type(
            &params[0],
            options.module_id.as_deref(),
            source_file,
            &options.local_type_registry,
            &options.imported_type_registries,
        )
        .ok()?;
        if !expr_uses_json_stringify_surface(
            body,
            &surface_info.helper_names.stringify,
            wrapper_param_names,
        ) {
            return None;
        }
        return Some(ValidationError::DirectJsonCodec {
            declaration_name: declaration_name.to_string(),
            target_name: surface_info.target_name.clone(),
            surface_kind: DirectJsonCodecSurface::Stringify.label().to_string(),
            encode_helper: surface_info.helper_names.encode,
            decode_helper: surface_info.helper_names.decode,
            parse_helper: surface_info.helper_names.parse,
            stringify_helper: surface_info.helper_names.stringify,
            location,
        });
    }

    let Some(decoded_root_type) = decode_result_ok_type(return_type) else {
        return None;
    };
    let surface_kind = if is_json_value_type(&params[0]) {
        DirectJsonCodecSurface::Decode
    } else if is_string_type(&params[0]) {
        DirectJsonCodecSurface::Parse
    } else {
        return None;
    };

    let surface_info = derive_json_surface_info_for_type(
        decoded_root_type,
        options.module_id.as_deref(),
        source_file,
        &options.local_type_registry,
        &options.imported_type_registries,
    )
    .ok()?;
    Some(ValidationError::DirectJsonCodec {
        declaration_name: declaration_name.to_string(),
        target_name: surface_info.target_name,
        surface_kind: surface_kind.label().to_string(),
        encode_helper: surface_info.helper_names.encode,
        decode_helper: surface_info.helper_names.decode,
        parse_helper: surface_info.helper_names.parse,
        stringify_helper: surface_info.helper_names.stringify,
        location,
    })
}

fn is_json_value_type(typ: &InferenceType) -> bool {
    matches!(
        typ,
        InferenceType::Constructor(constructor)
            if constructor.name == "stdlib::json.JsonValue" && constructor.type_args.is_empty()
    )
}

fn is_string_type(typ: &InferenceType) -> bool {
    matches!(
        typ,
        InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::String,
        })
    )
}

fn decode_result_ok_type(return_type: &InferenceType) -> Option<&InferenceType> {
    let InferenceType::Constructor(constructor) = return_type else {
        return None;
    };
    if constructor.name != "Result" || constructor.type_args.len() != 2 {
        return None;
    }
    if !is_decode_error_type(&constructor.type_args[1]) {
        return None;
    }
    Some(&constructor.type_args[0])
}

fn is_decode_error_type(typ: &InferenceType) -> bool {
    match typ {
        InferenceType::Constructor(error_type) => {
            error_type.name == "stdlib::decode.DecodeError" && error_type.type_args.is_empty()
        }
        InferenceType::Record(record) => {
            record.name.as_deref() == Some("stdlib::decode.DecodeError")
        }
        _ => false,
    }
}

fn expr_uses_json_stringify_surface(
    expr: &TypedExpr,
    stringify_helper: &str,
    wrapper_param_names: &[String],
) -> bool {
    expr_contains_named_call(expr, &["stdlib", "json"], "stringify")
        || expr_is_identifier_named(expr, stringify_helper)
        || expr_is_direct_call_wrapper(expr, stringify_helper, wrapper_param_names)
}

fn expr_is_identifier_named(expr: &TypedExpr, helper_name: &str) -> bool {
    matches!(
        &expr.kind,
        TypedExprKind::Identifier(identifier) if identifier.name == helper_name
    )
}

fn expr_is_direct_call_wrapper(
    expr: &TypedExpr,
    helper_name: &str,
    wrapper_param_names: &[String],
) -> bool {
    let TypedExprKind::Call(call) = &expr.kind else {
        return false;
    };
    if wrapper_param_names.len() != call.args.len() {
        return false;
    }
    match &call.func.kind {
        TypedExprKind::Identifier(identifier) if identifier.name == helper_name => {}
        _ => return false,
    }
    call.args.iter().zip(wrapper_param_names).all(|(arg, param_name)| {
        matches!(
            &arg.kind,
            TypedExprKind::Identifier(identifier) if identifier.name == *param_name
        )
    })
}

fn expr_contains_named_call(expr: &TypedExpr, namespace: &[&str], member: &str) -> bool {
    match &expr.kind {
        TypedExprKind::Call(call) => {
            is_namespace_member(&call.func, namespace, member)
                || expr_contains_named_call(&call.func, namespace, member)
                || call
                    .args
                    .iter()
                    .any(|arg| expr_contains_named_call(arg, namespace, member))
        }
        TypedExprKind::ConstructorCall(call) => call
            .args
            .iter()
            .any(|arg| expr_contains_named_call(arg, namespace, member)),
        TypedExprKind::ExternCall(call) => call
            .args
            .iter()
            .any(|arg| expr_contains_named_call(arg, namespace, member)),
        TypedExprKind::MethodCall(call) => {
            expr_contains_named_call(&call.receiver, namespace, member)
                || call
                    .args
                    .iter()
                    .any(|arg| expr_contains_named_call(arg, namespace, member))
        }
        TypedExprKind::Binary(binary) => {
            expr_contains_named_call(&binary.left, namespace, member)
                || expr_contains_named_call(&binary.right, namespace, member)
        }
        TypedExprKind::Unary(unary) => expr_contains_named_call(&unary.operand, namespace, member),
        TypedExprKind::Match(match_expr) => {
            expr_contains_named_call(&match_expr.scrutinee, namespace, member)
                || match_expr.arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expr_contains_named_call(guard, namespace, member))
                        || expr_contains_named_call(&arm.body, namespace, member)
                })
        }
        TypedExprKind::Let(let_expr) => {
            expr_contains_named_call(&let_expr.value, namespace, member)
                || expr_contains_named_call(&let_expr.body, namespace, member)
        }
        TypedExprKind::Using(using_expr) => {
            expr_contains_named_call(&using_expr.value, namespace, member)
                || expr_contains_named_call(&using_expr.body, namespace, member)
        }
        TypedExprKind::If(if_expr) => {
            expr_contains_named_call(&if_expr.condition, namespace, member)
                || expr_contains_named_call(&if_expr.then_branch, namespace, member)
                || if_expr
                    .else_branch
                    .as_ref()
                    .is_some_and(|else_branch| expr_contains_named_call(else_branch, namespace, member))
        }
        TypedExprKind::List(list) => list
            .elements
            .iter()
            .any(|element| expr_contains_named_call(element, namespace, member)),
        TypedExprKind::Tuple(tuple) => tuple
            .elements
            .iter()
            .any(|element| expr_contains_named_call(element, namespace, member)),
        TypedExprKind::Record(record) => record
            .fields
            .iter()
            .any(|field| expr_contains_named_call(&field.value, namespace, member)),
        TypedExprKind::MapLiteral(map) => map.entries.iter().any(|entry| {
            expr_contains_named_call(&entry.key, namespace, member)
                || expr_contains_named_call(&entry.value, namespace, member)
        }),
        TypedExprKind::FieldAccess(access) => {
            expr_contains_named_call(&access.object, namespace, member)
        }
        TypedExprKind::Index(index) => {
            expr_contains_named_call(&index.object, namespace, member)
                || expr_contains_named_call(&index.index, namespace, member)
        }
        TypedExprKind::Map(map_expr) => {
            expr_contains_named_call(&map_expr.list, namespace, member)
                || expr_contains_named_call(&map_expr.func, namespace, member)
        }
        TypedExprKind::Filter(filter) => {
            expr_contains_named_call(&filter.list, namespace, member)
                || expr_contains_named_call(&filter.predicate, namespace, member)
        }
        TypedExprKind::Fold(fold) => {
            expr_contains_named_call(&fold.list, namespace, member)
                || expr_contains_named_call(&fold.func, namespace, member)
                || expr_contains_named_call(&fold.init, namespace, member)
        }
        TypedExprKind::Pipeline(pipeline) => {
            expr_contains_named_call(&pipeline.left, namespace, member)
                || expr_contains_named_call(&pipeline.right, namespace, member)
        }
        TypedExprKind::Concurrent(concurrent) => {
            expr_contains_named_call(&concurrent.config.width, namespace, member)
                || concurrent
                    .config
                    .jitter_ms
                    .as_ref()
                    .is_some_and(|expr| expr_contains_named_call(expr, namespace, member))
                || concurrent
                    .config
                    .stop_on
                    .as_ref()
                    .is_some_and(|expr| expr_contains_named_call(expr, namespace, member))
                || concurrent
                    .config
                    .window_ms
                    .as_ref()
                    .is_some_and(|expr| expr_contains_named_call(expr, namespace, member))
                || concurrent.steps.iter().any(|step| match step {
                    TypedConcurrentStep::Spawn(spawn) => {
                        expr_contains_named_call(&spawn.expr, namespace, member)
                    }
                    TypedConcurrentStep::SpawnEach(spawn_each) => {
                        expr_contains_named_call(&spawn_each.list, namespace, member)
                            || expr_contains_named_call(&spawn_each.func, namespace, member)
                    }
                })
        }
        TypedExprKind::Lambda(lambda) => expr_contains_named_call(&lambda.body, namespace, member),
        TypedExprKind::Literal(_)
        | TypedExprKind::Identifier(_)
        | TypedExprKind::NamespaceMember { .. } => false,
    }
}

fn is_namespace_member(expr: &TypedExpr, namespace: &[&str], member: &str) -> bool {
    matches!(
        &expr.kind,
        TypedExprKind::NamespaceMember {
            namespace: actual_namespace,
            member: actual_member,
        } if actual_member == member
            && actual_namespace.len() == namespace.len()
            && actual_namespace
                .iter()
                .map(String::as_str)
                .zip(namespace.iter().copied())
                .all(|(left, right)| left == right)
    )
}

fn collect_unused_named_bindings(expr: &TypedExpr, errors: &mut Vec<ValidationError>) {
    match &expr.kind {
        TypedExprKind::Let(let_expr) => {
            for binding_name in pattern_binding_frame(&let_expr.pattern) {
                if count_identifier_uses(&let_expr.body, &binding_name) == 0 {
                    errors.push(ValidationError::UnusedBinding {
                        binding_name,
                        location: expr.location,
                    });
                }
            }

            collect_unused_named_bindings(&let_expr.value, errors);
            collect_unused_named_bindings(&let_expr.body, errors);
        }
        TypedExprKind::Using(using_expr) => {
            if count_identifier_uses(&using_expr.body, &using_expr.name) == 0 {
                errors.push(ValidationError::UnusedBinding {
                    binding_name: using_expr.name.clone(),
                    location: expr.location,
                });
            }
            collect_unused_named_bindings(&using_expr.value, errors);
            collect_unused_named_bindings(&using_expr.body, errors);
        }
        TypedExprKind::Lambda(lambda) => {
            collect_unused_named_bindings(&lambda.body, errors);
        }
        TypedExprKind::Call(call) => {
            collect_unused_named_bindings(&call.func, errors);
            for arg in &call.args {
                collect_unused_named_bindings(arg, errors);
            }
        }
        TypedExprKind::ConstructorCall(call) => {
            for arg in &call.args {
                collect_unused_named_bindings(arg, errors);
            }
        }
        TypedExprKind::ExternCall(call) => {
            for arg in &call.args {
                collect_unused_named_bindings(arg, errors);
            }
        }
        TypedExprKind::MethodCall(call) => {
            collect_unused_named_bindings(&call.receiver, errors);
            for arg in &call.args {
                collect_unused_named_bindings(arg, errors);
            }
        }
        TypedExprKind::Binary(binary) => {
            collect_unused_named_bindings(&binary.left, errors);
            collect_unused_named_bindings(&binary.right, errors);
        }
        TypedExprKind::Unary(unary) => collect_unused_named_bindings(&unary.operand, errors),
        TypedExprKind::Match(match_expr) => {
            collect_unused_named_bindings(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_unused_named_bindings(guard, errors);
                }
                collect_unused_named_bindings(&arm.body, errors);
            }
        }
        TypedExprKind::If(if_expr) => {
            collect_unused_named_bindings(&if_expr.condition, errors);
            collect_unused_named_bindings(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_unused_named_bindings(else_branch, errors);
            }
        }
        TypedExprKind::List(list) => {
            for element in &list.elements {
                collect_unused_named_bindings(element, errors);
            }
        }
        TypedExprKind::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_unused_named_bindings(element, errors);
            }
        }
        TypedExprKind::Record(record) => {
            for field in &record.fields {
                collect_unused_named_bindings(&field.value, errors);
            }
        }
        TypedExprKind::MapLiteral(map) => {
            for entry in &map.entries {
                collect_unused_named_bindings(&entry.key, errors);
                collect_unused_named_bindings(&entry.value, errors);
            }
        }
        TypedExprKind::FieldAccess(access) => {
            collect_unused_named_bindings(&access.object, errors);
        }
        TypedExprKind::Index(index) => {
            collect_unused_named_bindings(&index.object, errors);
            collect_unused_named_bindings(&index.index, errors);
        }
        TypedExprKind::Map(map_expr) => {
            collect_unused_named_bindings(&map_expr.list, errors);
            collect_unused_named_bindings(&map_expr.func, errors);
        }
        TypedExprKind::Filter(filter) => {
            collect_unused_named_bindings(&filter.list, errors);
            collect_unused_named_bindings(&filter.predicate, errors);
        }
        TypedExprKind::Fold(fold) => {
            collect_unused_named_bindings(&fold.list, errors);
            collect_unused_named_bindings(&fold.func, errors);
            collect_unused_named_bindings(&fold.init, errors);
        }
        TypedExprKind::Pipeline(pipeline) => {
            collect_unused_named_bindings(&pipeline.left, errors);
            collect_unused_named_bindings(&pipeline.right, errors);
        }
        TypedExprKind::Concurrent(concurrent) => {
            collect_unused_named_bindings(&concurrent.config.width, errors);
            if let Some(jitter_ms) = &concurrent.config.jitter_ms {
                collect_unused_named_bindings(jitter_ms, errors);
            }
            if let Some(stop_on) = &concurrent.config.stop_on {
                collect_unused_named_bindings(stop_on, errors);
            }
            if let Some(window_ms) = &concurrent.config.window_ms {
                collect_unused_named_bindings(window_ms, errors);
            }
            for step in &concurrent.steps {
                match step {
                    TypedConcurrentStep::Spawn(spawn) => {
                        collect_unused_named_bindings(&spawn.expr, errors)
                    }
                    TypedConcurrentStep::SpawnEach(spawn_each) => {
                        collect_unused_named_bindings(&spawn_each.list, errors);
                        collect_unused_named_bindings(&spawn_each.func, errors);
                    }
                }
            }
        }
        TypedExprKind::Literal(_)
        | TypedExprKind::Identifier(_)
        | TypedExprKind::NamespaceMember { .. } => {}
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
        TypedExprKind::Using(using_expr) => {
            collect_single_use_pure_bindings(&using_expr.value, errors);
            collect_single_use_pure_bindings(&using_expr.body, errors);
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
        TypedExprKind::Concurrent(concurrent) => {
            collect_single_use_pure_bindings(&concurrent.config.width, errors);
            if let Some(jitter_ms) = &concurrent.config.jitter_ms {
                collect_single_use_pure_bindings(jitter_ms, errors);
            }
            if let Some(stop_on) = &concurrent.config.stop_on {
                collect_single_use_pure_bindings(stop_on, errors);
            }
            if let Some(window_ms) = &concurrent.config.window_ms {
                collect_single_use_pure_bindings(window_ms, errors);
            }
            for step in &concurrent.steps {
                match step {
                    TypedConcurrentStep::Spawn(spawn) => {
                        collect_single_use_pure_bindings(&spawn.expr, errors)
                    }
                    TypedConcurrentStep::SpawnEach(spawn_each) => {
                        collect_single_use_pure_bindings(&spawn_each.list, errors);
                        collect_single_use_pure_bindings(&spawn_each.func, errors);
                    }
                }
            }
        }
        TypedExprKind::Literal(_)
        | TypedExprKind::Identifier(_)
        | TypedExprKind::NamespaceMember { .. } => {}
    }
}

fn collect_dead_pure_discards(expr: &TypedExpr, errors: &mut Vec<ValidationError>) {
    match &expr.kind {
        TypedExprKind::Let(let_expr) => {
            if matches!(&let_expr.pattern, Pattern::Wildcard(_))
                && let_expr.value.purity == PurityClass::Pure
            {
                errors.push(ValidationError::DeadPureDiscard {
                    location: expr.location,
                });
            }

            collect_dead_pure_discards(&let_expr.value, errors);
            collect_dead_pure_discards(&let_expr.body, errors);
        }
        TypedExprKind::Using(using_expr) => {
            collect_dead_pure_discards(&using_expr.value, errors);
            collect_dead_pure_discards(&using_expr.body, errors);
        }
        TypedExprKind::Lambda(lambda) => {
            collect_dead_pure_discards(&lambda.body, errors);
        }
        TypedExprKind::Call(call) => {
            collect_dead_pure_discards(&call.func, errors);
            for arg in &call.args {
                collect_dead_pure_discards(arg, errors);
            }
        }
        TypedExprKind::ConstructorCall(call) => {
            for arg in &call.args {
                collect_dead_pure_discards(arg, errors);
            }
        }
        TypedExprKind::ExternCall(call) => {
            for arg in &call.args {
                collect_dead_pure_discards(arg, errors);
            }
        }
        TypedExprKind::MethodCall(call) => {
            collect_dead_pure_discards(&call.receiver, errors);
            for arg in &call.args {
                collect_dead_pure_discards(arg, errors);
            }
        }
        TypedExprKind::Binary(binary) => {
            collect_dead_pure_discards(&binary.left, errors);
            collect_dead_pure_discards(&binary.right, errors);
        }
        TypedExprKind::Unary(unary) => {
            collect_dead_pure_discards(&unary.operand, errors);
        }
        TypedExprKind::Match(match_expr) => {
            collect_dead_pure_discards(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_dead_pure_discards(guard, errors);
                }
                collect_dead_pure_discards(&arm.body, errors);
            }
        }
        TypedExprKind::If(if_expr) => {
            collect_dead_pure_discards(&if_expr.condition, errors);
            collect_dead_pure_discards(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_dead_pure_discards(else_branch, errors);
            }
        }
        TypedExprKind::List(list_expr) => {
            for element in &list_expr.elements {
                collect_dead_pure_discards(element, errors);
            }
        }
        TypedExprKind::Tuple(tuple_expr) => {
            for element in &tuple_expr.elements {
                collect_dead_pure_discards(element, errors);
            }
        }
        TypedExprKind::Record(record) => {
            for field in &record.fields {
                collect_dead_pure_discards(&field.value, errors);
            }
        }
        TypedExprKind::MapLiteral(map) => {
            for entry in &map.entries {
                collect_dead_pure_discards(&entry.key, errors);
                collect_dead_pure_discards(&entry.value, errors);
            }
        }
        TypedExprKind::FieldAccess(access) => {
            collect_dead_pure_discards(&access.object, errors);
        }
        TypedExprKind::Index(index) => {
            collect_dead_pure_discards(&index.object, errors);
            collect_dead_pure_discards(&index.index, errors);
        }
        TypedExprKind::Map(map_expr) => {
            collect_dead_pure_discards(&map_expr.list, errors);
            collect_dead_pure_discards(&map_expr.func, errors);
        }
        TypedExprKind::Filter(filter) => {
            collect_dead_pure_discards(&filter.list, errors);
            collect_dead_pure_discards(&filter.predicate, errors);
        }
        TypedExprKind::Fold(fold) => {
            collect_dead_pure_discards(&fold.list, errors);
            collect_dead_pure_discards(&fold.func, errors);
            collect_dead_pure_discards(&fold.init, errors);
        }
        TypedExprKind::Pipeline(pipeline) => {
            collect_dead_pure_discards(&pipeline.left, errors);
            collect_dead_pure_discards(&pipeline.right, errors);
        }
        TypedExprKind::Concurrent(concurrent) => {
            collect_dead_pure_discards(&concurrent.config.width, errors);
            if let Some(jitter_ms) = &concurrent.config.jitter_ms {
                collect_dead_pure_discards(jitter_ms, errors);
            }
            if let Some(stop_on) = &concurrent.config.stop_on {
                collect_dead_pure_discards(stop_on, errors);
            }
            if let Some(window_ms) = &concurrent.config.window_ms {
                collect_dead_pure_discards(window_ms, errors);
            }
            for spawn in &concurrent.steps {
                match spawn {
                    TypedConcurrentStep::Spawn(spawn) => {
                        collect_dead_pure_discards(&spawn.expr, errors);
                    }
                    TypedConcurrentStep::SpawnEach(spawn_each) => {
                        collect_dead_pure_discards(&spawn_each.list, errors);
                        collect_dead_pure_discards(&spawn_each.func, errors);
                    }
                }
            }
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
            count_identifier_uses(&let_expr.value, name)
                + count_identifier_uses(&let_expr.body, name)
        }
        TypedExprKind::Using(using_expr) => {
            count_identifier_uses(&using_expr.value, name)
                + count_identifier_uses(&using_expr.body, name)
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
            count_identifier_uses(&map_expr.list, name)
                + count_identifier_uses(&map_expr.func, name)
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
            count_identifier_uses(&pipeline.left, name)
                + count_identifier_uses(&pipeline.right, name)
        }
        TypedExprKind::Concurrent(concurrent) => {
            count_identifier_uses(&concurrent.config.width, name)
                + concurrent
                    .config
                    .jitter_ms
                    .as_ref()
                    .map(|expr| count_identifier_uses(expr, name))
                    .unwrap_or(0)
                + concurrent
                    .config
                    .stop_on
                    .as_ref()
                    .map(|expr| count_identifier_uses(expr, name))
                    .unwrap_or(0)
                + concurrent
                    .config
                    .window_ms
                    .as_ref()
                    .map(|expr| count_identifier_uses(expr, name))
                    .unwrap_or(0)
                + concurrent
                    .steps
                    .iter()
                    .map(|step| match step {
                        TypedConcurrentStep::Spawn(spawn) => {
                            count_identifier_uses(&spawn.expr, name)
                        }
                        TypedConcurrentStep::SpawnEach(spawn_each) => {
                            count_identifier_uses(&spawn_each.list, name)
                                + count_identifier_uses(&spawn_each.func, name)
                        }
                    })
                    .sum::<usize>()
        }
    }
}

/// Validate parameter alphabetical ordering
fn validate_parameter_ordering(
    params: &[Param],
    func_name: &str,
    location: SourceLocation,
) -> Result<(), Vec<ValidationError>> {
    if params.len() <= 1 {
        return Ok(());
    }

    for i in 1..params.len() {
        let prev = &params[i - 1];
        let curr = &params[i];

        if curr.name < prev.name {
            let expected_order: Vec<String> = params
                .iter()
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
    location: SourceLocation,
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
        match decl {
            Declaration::Function(func) => {
                if let Err(e) = validate_parameter_ordering(&func.params, &func.name, func.location)
                {
                    errors.extend(e);
                }
                if let Err(e) = validate_effect_ordering(&func.effects, &func.name, func.location) {
                    errors.extend(e);
                }
            }
            Declaration::Effect(effect_decl) => {
                if let Err(e) = validate_effect_ordering(
                    &effect_decl.effects,
                    &effect_decl.name,
                    effect_decl.location,
                ) {
                    errors.extend(e);
                }
            }
            _ => {}
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

    if let Some((index, field_name, prev_field, expected_order)) = first_out_of_order_field(&names)
    {
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

    if let Some((index, field_name, prev_field, expected_order)) = first_out_of_order_field(&names)
    {
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

    if let Some((index, field_name, prev_field, expected_order)) = first_out_of_order_field(&names)
    {
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
            if let Err(e) = validate_record_pattern_field_ordering(&record.fields, record.location)
            {
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
        Expr::Using(using_expr) => {
            validate_expr_record_fields(&using_expr.value, errors);
            validate_expr_record_fields(&using_expr.body, errors);
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
            if let Err(e) = validate_record_literal_field_ordering(&record.fields, record.location)
            {
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
        Expr::FieldAccess(field_access) => {
            validate_expr_record_fields(&field_access.object, errors)
        }
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
        Expr::Concurrent(concurrent) => {
            validate_expr_record_fields(&concurrent.width, errors);
            if let Some(policy) = &concurrent.policy {
                if let Err(e) =
                    validate_record_literal_field_ordering(&policy.fields, policy.location)
                {
                    errors.extend(e);
                }
                for field in &policy.fields {
                    validate_expr_record_fields(&field.value, errors);
                }
            }
            for step in &concurrent.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => {
                        validate_expr_record_fields(&spawn.expr, errors)
                    }
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_expr_record_fields(&spawn_each.list, errors);
                        validate_expr_record_fields(&spawn_each.func, errors);
                    }
                }
            }
        }
        Expr::TypeAscription(type_ascription) => {
            validate_expr_record_fields(&type_ascription.expr, errors)
        }
    }
}

fn validate_record_field_ordering(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Type(type_decl) => {
                if let TypeDef::Product(product) = &type_decl.definition {
                    if let Err(e) = validate_record_type_field_ordering(
                        &type_decl.name,
                        &product.fields,
                        product.location,
                    ) {
                        errors.extend(e);
                    }
                }
            }
            Declaration::Function(function) => {
                validate_expr_record_fields(&function.body, &mut errors)
            }
            Declaration::Const(const_decl) => {
                validate_expr_record_fields(&const_decl.value, &mut errors)
            }
            Declaration::FeatureFlag(feature_flag_decl) => {
                validate_expr_record_fields(&feature_flag_decl.default, &mut errors)
            }
            Declaration::Test(test_decl) => {
                validate_expr_record_fields(&test_decl.body, &mut errors)
            }
            _ => {}
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

fn first_existing_binding(
    name: &str,
    local: &ScopeFrame,
    scopes: &[ScopeFrame],
) -> Option<BindingInfo> {
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

fn validate_expr_no_shadowing(
    expr: &Expr,
    scopes: &mut Vec<ScopeFrame>,
    errors: &mut Vec<ValidationError>,
) {
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
        Expr::Using(using_expr) => {
            validate_expr_no_shadowing(&using_expr.value, scopes, errors);
            let mut local = ScopeFrame::new();
            try_bind_name(
                &using_expr.name,
                BindingKind::LocalBinding,
                using_expr.location,
                &mut local,
                scopes,
                errors,
            );
            scopes.push(local);
            validate_expr_no_shadowing(&using_expr.body, scopes, errors);
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
        Expr::FieldAccess(field_access) => {
            validate_expr_no_shadowing(&field_access.object, scopes, errors)
        }
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
        Expr::Concurrent(concurrent) => {
            validate_expr_no_shadowing(&concurrent.width, scopes, errors);
            if let Some(policy) = &concurrent.policy {
                for field in &policy.fields {
                    validate_expr_no_shadowing(&field.value, scopes, errors);
                }
            }
            for step in &concurrent.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => {
                        validate_expr_no_shadowing(&spawn.expr, scopes, errors)
                    }
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_expr_no_shadowing(&spawn_each.list, scopes, errors);
                        validate_expr_no_shadowing(&spawn_each.func, scopes, errors);
                    }
                }
            }
        }
        Expr::TypeAscription(type_ascription) => {
            validate_expr_no_shadowing(&type_ascription.expr, scopes, errors)
        }
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
            Declaration::FeatureFlag(feature_flag_decl) => {
                let mut scopes = Vec::new();
                validate_expr_no_shadowing(&feature_flag_decl.default, &mut scopes, &mut errors);
            }
            Declaration::Test(test_decl) => {
                let mut scopes = Vec::new();
                validate_expr_no_shadowing(&test_decl.body, &mut scopes, &mut errors);
            }
            _ => {}
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
    let mut protocol_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut effect_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut extern_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut feature_flag_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut const_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut function_names: HashMap<String, SourceLocation> = HashMap::new();
    let mut label_names: HashMap<String, SourceLocation> = HashMap::new();
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

            Declaration::Derive(_) => {}

            Declaration::Effect(EffectDecl { name, location, .. }) => {
                if let Some(first_loc) = effect_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "EFFECT".to_string(),
                        what: "effect".to_string(),
                        name: name.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    effect_names.insert(name.clone(), *location);
                }
            }

            Declaration::Extern(ExternDecl {
                module_path,
                location,
                ..
            }) => {
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

            Declaration::FeatureFlag(FeatureFlagDecl { name, location, .. }) => {
                if let Some(first_loc) = feature_flag_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "FEATURE-FLAG".to_string(),
                        what: "feature flag".to_string(),
                        name: name.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    feature_flag_names.insert(name.clone(), *location);
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

            Declaration::Transform(transform_decl) => {
                let name = &transform_decl.function.name;
                let location = transform_decl.function.location;
                if let Some(first_loc) = function_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "TRANSFORM".to_string(),
                        what: "transform".to_string(),
                        name: name.clone(),
                        location,
                        first_location: *first_loc,
                    });
                } else {
                    function_names.insert(name.clone(), location);
                }
            }

            Declaration::Label(label_decl) => {
                let name = &label_decl.name;
                let location = label_decl.location;
                if let Some(first_loc) = label_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "LABEL".to_string(),
                        what: "label".to_string(),
                        name: name.clone(),
                        location,
                        first_location: *first_loc,
                    });
                } else {
                    label_names.insert(name.clone(), location);
                }
            }

            Declaration::Rule(_) => {}

            Declaration::Protocol(ProtocolDecl { name, location, .. }) => {
                if let Some(first_loc) = protocol_names.get(name) {
                    errors.push(ValidationError::DuplicateDeclaration {
                        kind: "PROTOCOL".to_string(),
                        what: "protocol".to_string(),
                        name: name.clone(),
                        location: *location,
                        first_location: *first_loc,
                    });
                } else {
                    protocol_names.insert(name.clone(), *location);
                }
            }

            Declaration::Test(TestDecl {
                description,
                location,
                ..
            }) => {
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
fn validate_file_purpose(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let mut has_main = false;
    let has_tests = program
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Test(_)));

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
            message: "Test files must have λmain()=>Unit=()\n\nHint: Test files are executables"
                .to_string(),
        }]);
    }

    Ok(())
}

fn validate_effect_declaration_placement(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let Some(path) = file_path else {
        return Ok(());
    };

    if find_project_root(Path::new(path)).is_none() {
        return Ok(());
    }

    let normalized_path = Some(path.replace('\\', "/"));
    let is_effects_file = normalized_path
        .as_deref()
        .is_some_and(|path| path.ends_with("/src/effects.lib.sigil"));

    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Effect(effect_decl) => {
                if !is_effects_file {
                    errors.push(ValidationError::EffectDeclarationPlacement {
                        message: "Project-defined effects must live in src/effects.lib.sigil"
                            .to_string(),
                        location: effect_decl.location,
                    });
                }
            }
            _ => {
                if is_effects_file {
                    errors.push(ValidationError::EffectDeclarationPlacement {
                        message: "src/effects.lib.sigil may only contain effect declarations"
                            .to_string(),
                        location: *get_declaration_location(decl),
                    });
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

fn validate_project_type_declaration_placement(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let Some(path) = file_path else {
        return Ok(());
    };

    if find_project_root(Path::new(path)).is_none() {
        return Ok(());
    }

    let normalized_path = path.replace('\\', "/");
    let is_types_file = normalized_path.ends_with("/src/types.lib.sigil");
    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Label(label_decl) => {
                if !is_types_file {
                    errors.push(ValidationError::TypeDeclarationPlacement {
                        message: "Project-defined labels must live in src/types.lib.sigil"
                            .to_string(),
                        location: label_decl.location,
                    });
                }
            }
            Declaration::Type(type_decl) => {
                if !is_types_file {
                    errors.push(ValidationError::TypeDeclarationPlacement {
                        message: "Project-defined types must live in src/types.lib.sigil"
                            .to_string(),
                        location: type_decl.location,
                    });
                } else {
                    validate_types_file_type_decl(type_decl, &mut errors);
                }
            }
            _ => {
                if is_types_file {
                    errors.push(ValidationError::TypeDeclarationPlacement {
                        message: "src/types.lib.sigil may only contain type and label declarations"
                            .to_string(),
                        location: *get_declaration_location(decl),
                    });
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

fn validate_project_policy_declaration_placement(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let Some(path) = file_path else {
        return Ok(());
    };

    if find_project_root(Path::new(path)).is_none() {
        return Ok(());
    }

    let normalized_path = path.replace('\\', "/");
    let is_policies_file = normalized_path.ends_with("/src/policies.lib.sigil");
    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Rule(rule_decl) => {
                if !is_policies_file {
                    errors.push(ValidationError::PolicyDeclarationPlacement {
                        message:
                            "Project-defined boundary rules must live in src/policies.lib.sigil"
                                .to_string(),
                        location: rule_decl.location,
                    });
                }
            }
            Declaration::Transform(transform_decl) => {
                if !is_policies_file {
                    errors.push(ValidationError::PolicyDeclarationPlacement {
                        message: "Project-defined transforms must live in src/policies.lib.sigil"
                            .to_string(),
                        location: transform_decl.function.location,
                    });
                }
            }
            _ => {
                if is_policies_file {
                    errors.push(ValidationError::PolicyDeclarationPlacement {
                        message: "src/policies.lib.sigil may only contain rule and transform declarations"
                            .to_string(),
                        location: *get_declaration_location(decl),
                    });
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

fn validate_project_feature_flag_declaration_placement(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let Some(path) = file_path else {
        return Ok(());
    };

    if find_project_root(Path::new(path)).is_none() {
        return Ok(());
    }

    let normalized_path = path.replace('\\', "/");
    let is_flags_file = normalized_path.ends_with("/src/flags.lib.sigil");
    let mut errors = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::FeatureFlag(feature_flag_decl) => {
                if !is_flags_file {
                    errors.push(ValidationError::FeatureFlagDeclaration {
                        message: "Project-defined feature flags must live in src/flags.lib.sigil"
                            .to_string(),
                        location: feature_flag_decl.location,
                    });
                } else if !is_canonical_timestamp(feature_flag_decl.created_at.as_str()) {
                    errors.push(ValidationError::FeatureFlagDeclaration {
                        message: format!(
                            "featureFlag {} createdAt must use canonical UTC timestamp format YYYY-MM-DDTHH-mm-ssZ",
                            feature_flag_decl.name
                        ),
                        location: feature_flag_decl.created_at_location,
                    });
                }
            }
            _ => {
                if is_flags_file {
                    errors.push(ValidationError::FeatureFlagDeclaration {
                        message: "src/flags.lib.sigil may only contain featureFlag declarations"
                            .to_string(),
                        location: *get_declaration_location(decl),
                    });
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

fn find_project_root(start_path: &Path) -> Option<PathBuf> {
    let mut current = if start_path.is_file() {
        start_path.parent()?.to_path_buf()
    } else {
        start_path.to_path_buf()
    };

    loop {
        if current.join("sigil.json").exists() {
            return Some(current);
        }

        current = current.parent()?.to_path_buf();
    }
}

fn validate_types_file_type_decl(type_decl: &TypeDecl, errors: &mut Vec<ValidationError>) {
    validate_types_file_type_roots_in_def(&type_decl.definition, errors);

    if let Some(constraint) = &type_decl.constraint {
        validate_types_file_expr_roots(constraint, errors);
    }
}

fn validate_types_file_type_roots_in_def(type_def: &TypeDef, errors: &mut Vec<ValidationError>) {
    match type_def {
        TypeDef::Alias(alias) => validate_types_file_type_roots(&alias.aliased_type, errors),
        TypeDef::Product(product) => {
            for field in &product.fields {
                validate_types_file_type_roots(&field.field_type, errors);
            }
        }
        TypeDef::Sum(sum) => {
            for variant in &sum.variants {
                for typ in &variant.types {
                    validate_types_file_type_roots(typ, errors);
                }
            }
        }
    }
}

fn validate_types_file_type_roots(typ: &Type, errors: &mut Vec<ValidationError>) {
    match typ {
        Type::Primitive(_) | Type::Variable(_) => {}
        Type::List(list) => validate_types_file_type_roots(&list.element_type, errors),
        Type::Map(map) => {
            validate_types_file_type_roots(&map.key_type, errors);
            validate_types_file_type_roots(&map.value_type, errors);
        }
        Type::Function(function) => {
            for param in &function.param_types {
                validate_types_file_type_roots(param, errors);
            }
            validate_types_file_type_roots(&function.return_type, errors);
        }
        Type::Constructor(constructor) => {
            for arg in &constructor.type_args {
                validate_types_file_type_roots(arg, errors);
            }
        }
        Type::Tuple(tuple) => {
            for elem in &tuple.types {
                validate_types_file_type_roots(elem, errors);
            }
        }
        Type::Qualified(qualified) => {
            if !qualified
                .module_path
                .first()
                .is_some_and(|root| root == "stdlib" || root == "core")
            {
                errors.push(ValidationError::TypeDeclarationPlacement {
                    message:
                        "src/types.lib.sigil may only reference § and ¶ roots inside type declarations"
                            .to_string(),
                    location: qualified.location,
                });
            }

            for arg in &qualified.type_args {
                validate_types_file_type_roots(arg, errors);
            }
        }
    }
}

fn validate_types_file_expr_roots(expr: &Expr, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) => {}
        Expr::Lambda(lambda) => {
            for param in &lambda.params {
                if let Some(type_annotation) = &param.type_annotation {
                    validate_types_file_type_roots(type_annotation, errors);
                }
            }
            validate_types_file_type_roots(&lambda.return_type, errors);
            validate_types_file_expr_roots(&lambda.body, errors);
        }
        Expr::Application(app) => {
            validate_types_file_expr_roots(&app.func, errors);
            for arg in &app.args {
                validate_types_file_expr_roots(arg, errors);
            }
        }
        Expr::Binary(binary) => {
            validate_types_file_expr_roots(&binary.left, errors);
            validate_types_file_expr_roots(&binary.right, errors);
        }
        Expr::Unary(unary) => validate_types_file_expr_roots(&unary.operand, errors),
        Expr::Match(match_expr) => {
            validate_types_file_expr_roots(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                validate_types_file_pattern_roots(&arm.pattern, errors);
                if let Some(guard) = &arm.guard {
                    validate_types_file_expr_roots(guard, errors);
                }
                validate_types_file_expr_roots(&arm.body, errors);
            }
        }
        Expr::Let(let_expr) => {
            validate_types_file_pattern_roots(&let_expr.pattern, errors);
            validate_types_file_expr_roots(&let_expr.value, errors);
            validate_types_file_expr_roots(&let_expr.body, errors);
        }
        Expr::Using(using_expr) => {
            validate_types_file_expr_roots(&using_expr.value, errors);
            validate_types_file_expr_roots(&using_expr.body, errors);
        }
        Expr::If(if_expr) => {
            validate_types_file_expr_roots(&if_expr.condition, errors);
            validate_types_file_expr_roots(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                validate_types_file_expr_roots(else_branch, errors);
            }
        }
        Expr::List(list) => {
            for elem in &list.elements {
                validate_types_file_expr_roots(elem, errors);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                validate_types_file_expr_roots(&field.value, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                validate_types_file_expr_roots(&entry.key, errors);
                validate_types_file_expr_roots(&entry.value, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for elem in &tuple.elements {
                validate_types_file_expr_roots(elem, errors);
            }
        }
        Expr::FieldAccess(field_access) => {
            validate_types_file_expr_roots(&field_access.object, errors);
        }
        Expr::Index(index) => {
            validate_types_file_expr_roots(&index.object, errors);
            validate_types_file_expr_roots(&index.index, errors);
        }
        Expr::Pipeline(pipeline) => {
            validate_types_file_expr_roots(&pipeline.left, errors);
            validate_types_file_expr_roots(&pipeline.right, errors);
        }
        Expr::Map(map_expr) => {
            validate_types_file_expr_roots(&map_expr.list, errors);
            validate_types_file_expr_roots(&map_expr.func, errors);
        }
        Expr::Filter(filter_expr) => {
            validate_types_file_expr_roots(&filter_expr.list, errors);
            validate_types_file_expr_roots(&filter_expr.predicate, errors);
        }
        Expr::Fold(fold_expr) => {
            validate_types_file_expr_roots(&fold_expr.list, errors);
            validate_types_file_expr_roots(&fold_expr.func, errors);
            validate_types_file_expr_roots(&fold_expr.init, errors);
        }
        Expr::Concurrent(concurrent_expr) => {
            validate_types_file_expr_roots(&concurrent_expr.width, errors);
            if let Some(policy) = &concurrent_expr.policy {
                for field in &policy.fields {
                    validate_types_file_expr_roots(&field.value, errors);
                }
            }
            for step in &concurrent_expr.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => {
                        validate_types_file_expr_roots(&spawn.expr, errors)
                    }
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_types_file_expr_roots(&spawn_each.list, errors);
                        validate_types_file_expr_roots(&spawn_each.func, errors);
                    }
                }
            }
        }
        Expr::MemberAccess(member_access) => {
            if !member_access
                .namespace
                .first()
                .is_some_and(|root| root == "stdlib" || root == "core")
            {
                errors.push(ValidationError::TypeDeclarationPlacement {
                    message:
                        "src/types.lib.sigil may only reference § and ¶ roots inside constraints"
                            .to_string(),
                    location: member_access.location,
                });
            }
        }
        Expr::TypeAscription(type_asc) => {
            validate_types_file_expr_roots(&type_asc.expr, errors);
            validate_types_file_type_roots(&type_asc.ascribed_type, errors);
        }
    }
}

fn validate_types_file_pattern_roots(pattern: &Pattern, errors: &mut Vec<ValidationError>) {
    match pattern {
        Pattern::Literal(_) | Pattern::Identifier(_) | Pattern::Wildcard(_) => {}
        Pattern::Constructor(constructor) => {
            if !constructor.module_path.is_empty()
                && !constructor
                    .module_path
                    .first()
                    .is_some_and(|root| root == "stdlib" || root == "core")
            {
                errors.push(ValidationError::TypeDeclarationPlacement {
                    message:
                        "src/types.lib.sigil may only reference § and ¶ roots inside constraint patterns"
                            .to_string(),
                    location: constructor.location,
                });
            }

            for nested in &constructor.patterns {
                validate_types_file_pattern_roots(nested, errors);
            }
        }
        Pattern::List(list) => {
            for nested in &list.patterns {
                validate_types_file_pattern_roots(nested, errors);
            }
        }
        Pattern::Record(record) => {
            for field in &record.fields {
                if let Some(pattern) = &field.pattern {
                    validate_types_file_pattern_roots(pattern, errors);
                }
            }
        }
        Pattern::Tuple(tuple) => {
            for nested in &tuple.patterns {
                validate_types_file_pattern_roots(nested, errors);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct UsageSummary {
    imports: HashSet<String>,
    externs: HashSet<String>,
    value_refs: HashSet<String>,
    type_refs: HashSet<String>,
    effect_refs: HashSet<String>,
}

impl UsageSummary {
    fn merge_from(&mut self, other: &UsageSummary) {
        self.imports.extend(other.imports.iter().cloned());
        self.externs.extend(other.externs.iter().cloned());
        self.value_refs.extend(other.value_refs.iter().cloned());
        self.type_refs.extend(other.type_refs.iter().cloned());
        self.effect_refs.extend(other.effect_refs.iter().cloned());
    }
}

#[derive(Debug, Clone)]
struct NamedUsage {
    kind: &'static str,
    exported_from_executable: bool,
    location: SourceLocation,
    summary: UsageSummary,
}

fn executable_runtime_export_type(typ: &Type) -> bool {
    match typ {
        Type::Qualified(qualified) => {
            let module_id = qualified.module_path.join("::");
            matches!(
                (module_id.as_str(), qualified.type_name.as_str()),
                ("world::runtime", "World")
                    | ("stdlib::topology", "Environment")
                    | ("stdlib::topology", "FsRoot")
                    | ("stdlib::topology", "HttpServiceDependency")
                    | ("stdlib::topology", "LogSink")
                    | ("stdlib::topology", "PtyHandle")
                    | ("stdlib::topology", "ProcessHandle")
                    | ("stdlib::topology", "SqlHandle")
                    | ("stdlib::topology", "TcpServiceDependency")
                    | ("stdlib::topology", "WebSocketHandle")
            )
        }
        _ => false,
    }
}

fn const_is_exported_from_executable(const_decl: &ConstDecl) -> bool {
    const_decl.name == "world"
        || const_decl
            .type_annotation
            .as_ref()
            .is_some_and(executable_runtime_export_type)
}

fn validate_no_unused_items(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    let is_lib_file = file_path.is_some_and(|path| path.ends_with(".lib.sigil"));

    let import_paths: HashSet<String> = HashSet::new();
    let extern_paths: HashSet<String> = program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            Declaration::Extern(extern_decl) => Some(extern_decl.module_path.join("::")),
            _ => None,
        })
        .collect();
    let top_level_values: HashSet<String> = program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            Declaration::Function(function_decl) => Some(function_decl.name.clone()),
            Declaration::Transform(transform_decl) => Some(transform_decl.function.name.clone()),
            Declaration::FeatureFlag(feature_flag_decl) => Some(feature_flag_decl.name.clone()),
            Declaration::Const(const_decl) => Some(const_decl.name.clone()),
            _ => None,
        })
        .collect();
    let top_level_types: HashSet<String> = program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            Declaration::Type(type_decl) => Some(type_decl.name.clone()),
            Declaration::Label(label_decl) => Some(label_decl.name.clone()),
            _ => None,
        })
        .collect();
    let top_level_effects: HashSet<String> = program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            Declaration::Effect(effect_decl) => Some(effect_decl.name.clone()),
            _ => None,
        })
        .collect();
    let constructor_to_type: HashMap<String, String> = program
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            Declaration::Type(type_decl) => match &type_decl.definition {
                TypeDef::Sum(sum_type) => Some(
                    sum_type
                        .variants
                        .iter()
                        .map(|variant| (variant.name.clone(), type_decl.name.clone()))
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .collect();

    let mut externs = HashMap::new();
    let mut values = HashMap::new();
    let mut types = HashMap::new();
    let mut effects = HashMap::new();
    let mut rules = Vec::new();
    let mut tests = Vec::new();

    for decl in &program.declarations {
        match decl {
            Declaration::Extern(extern_decl) => {
                let path = extern_decl.module_path.join("::");
                externs.insert(
                    path,
                    NamedUsage {
                        kind: "extern",
                        exported_from_executable: false,
                        location: extern_decl.location,
                        summary: collect_extern_usage_summary(
                            extern_decl,
                            &top_level_types,
                            &top_level_effects,
                        ),
                    },
                );
            }
            Declaration::Derive(_) => {}
            Declaration::Function(function_decl) => {
                values.insert(
                    function_decl.name.clone(),
                    NamedUsage {
                        kind: "function",
                        exported_from_executable: false,
                        location: function_decl.location,
                        summary: collect_function_usage_summary(
                            function_decl,
                            &top_level_values,
                            &top_level_types,
                            &top_level_effects,
                            &constructor_to_type,
                            &import_paths,
                            &extern_paths,
                        ),
                    },
                );
            }
            Declaration::Transform(transform_decl) => {
                let function_decl = &transform_decl.function;
                values.insert(
                    function_decl.name.clone(),
                    NamedUsage {
                        kind: "transform",
                        exported_from_executable: false,
                        location: function_decl.location,
                        summary: collect_function_usage_summary(
                            function_decl,
                            &top_level_values,
                            &top_level_types,
                            &top_level_effects,
                            &constructor_to_type,
                            &import_paths,
                            &extern_paths,
                        ),
                    },
                );
            }
            Declaration::Const(const_decl) => {
                values.insert(
                    const_decl.name.clone(),
                    NamedUsage {
                        kind: "const",
                        exported_from_executable: const_is_exported_from_executable(const_decl),
                        location: const_decl.location,
                        summary: collect_const_usage_summary(
                            const_decl,
                            &[],
                            &top_level_values,
                            &top_level_types,
                            &top_level_effects,
                            &constructor_to_type,
                            &import_paths,
                            &extern_paths,
                        ),
                    },
                );
            }
            Declaration::FeatureFlag(feature_flag_decl) => {
                let synthetic_const = ConstDecl {
                    name: feature_flag_decl.name.clone(),
                    type_annotation: Some(feature_flag_decl.flag_type.clone()),
                    value: feature_flag_decl.default.clone(),
                    location: feature_flag_decl.location,
                };
                values.insert(
                    feature_flag_decl.name.clone(),
                    NamedUsage {
                        kind: "feature flag",
                        exported_from_executable: false,
                        location: feature_flag_decl.location,
                        summary: collect_const_usage_summary(
                            &synthetic_const,
                            &[],
                            &top_level_values,
                            &top_level_types,
                            &top_level_effects,
                            &constructor_to_type,
                            &import_paths,
                            &extern_paths,
                        ),
                    },
                );
            }
            Declaration::Type(type_decl) => {
                types.insert(
                    type_decl.name.clone(),
                    NamedUsage {
                        kind: "type",
                        exported_from_executable: false,
                        location: type_decl.location,
                        summary: collect_type_decl_usage_summary(
                            type_decl,
                            &top_level_types,
                            &top_level_effects,
                            &import_paths,
                        ),
                    },
                );
            }
            Declaration::Effect(effect_decl) => {
                effects.insert(
                    effect_decl.name.clone(),
                    NamedUsage {
                        kind: "effect",
                        exported_from_executable: false,
                        location: effect_decl.location,
                        summary: collect_effect_decl_usage_summary(effect_decl, &top_level_effects),
                    },
                );
            }
            Declaration::Label(label_decl) => {
                types.insert(
                    label_decl.name.clone(),
                    NamedUsage {
                        kind: "label",
                        exported_from_executable: false,
                        location: label_decl.location,
                        summary: collect_label_decl_usage_summary(label_decl, &top_level_types),
                    },
                );
            }
            Declaration::Test(test_decl) => {
                tests.push(collect_test_usage_summary(
                    test_decl,
                    &top_level_values,
                    &top_level_types,
                    &top_level_effects,
                    &constructor_to_type,
                    &import_paths,
                    &extern_paths,
                ));
            }
            Declaration::Rule(rule_decl) => {
                rules.push(collect_rule_usage_summary(
                    rule_decl,
                    &top_level_values,
                    &top_level_types,
                    &import_paths,
                ));
            }
            Declaration::Protocol(_) => {}
        }
    }

    let reachability = if is_lib_file {
        collect_library_usage(&values, &types, &effects, &externs, &tests, &rules)
    } else {
        collect_executable_usage(&values, &types, &externs, &tests, &rules)
    };

    if !is_lib_file {
        for (extern_path, extern_usage) in &externs {
            if !reachability.externs.contains(extern_path) {
                errors.push(ValidationError::UnusedExtern {
                    extern_path: extern_path.clone(),
                    location: extern_usage.location,
                });
            }
        }
    }

    if !is_lib_file {
        for (name, value_usage) in &values {
            if name != "main"
                && !value_usage.exported_from_executable
                && !reachability.values.contains(name)
            {
                errors.push(ValidationError::UnusedDeclaration {
                    decl_kind: value_usage.kind.to_string(),
                    decl_name: name.clone(),
                    location: value_usage.location,
                });
            }
        }

        for (name, type_usage) in &types {
            if !reachability.types.contains(name) {
                errors.push(ValidationError::UnusedDeclaration {
                    decl_kind: type_usage.kind.to_string(),
                    decl_name: name.clone(),
                    location: type_usage.location,
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, Default)]
struct Reachability {
    imports: HashSet<String>,
    externs: HashSet<String>,
    values: HashSet<String>,
    types: HashSet<String>,
}

fn collect_library_usage(
    values: &HashMap<String, NamedUsage>,
    types: &HashMap<String, NamedUsage>,
    effects: &HashMap<String, NamedUsage>,
    externs: &HashMap<String, NamedUsage>,
    tests: &[UsageSummary],
    rules: &[UsageSummary],
) -> Reachability {
    let mut aggregate = UsageSummary::default();
    for usage in values.values() {
        aggregate.merge_from(&usage.summary);
    }
    for usage in types.values() {
        aggregate.merge_from(&usage.summary);
    }
    for usage in effects.values() {
        aggregate.merge_from(&usage.summary);
    }
    for test in tests {
        aggregate.merge_from(test);
    }
    for rule in rules {
        aggregate.merge_from(rule);
    }

    let mut reachability = Reachability {
        imports: aggregate.imports.clone(),
        externs: aggregate.externs.clone(),
        values: values.keys().cloned().collect(),
        types: types.keys().cloned().collect(),
    };

    accumulate_extern_usage(&mut aggregate, externs);
    reachability.imports = aggregate.imports;
    reachability.externs = aggregate.externs;
    reachability
}

fn collect_executable_usage(
    values: &HashMap<String, NamedUsage>,
    types: &HashMap<String, NamedUsage>,
    externs: &HashMap<String, NamedUsage>,
    tests: &[UsageSummary],
    rules: &[UsageSummary],
) -> Reachability {
    let mut aggregate = UsageSummary::default();
    let mut reachable_values = HashSet::new();
    let mut pending_values = Vec::new();

    if let Some(main_usage) = values.get("main") {
        reachable_values.insert("main".to_string());
        pending_values.push("main".to_string());
        aggregate.merge_from(&main_usage.summary);
    }

    for test in tests {
        aggregate.merge_from(test);
        for name in &test.value_refs {
            if values.contains_key(name) && reachable_values.insert(name.clone()) {
                pending_values.push(name.clone());
            }
        }
    }
    for rule in rules {
        aggregate.merge_from(rule);
        for name in &rule.value_refs {
            if values.contains_key(name) && reachable_values.insert(name.clone()) {
                pending_values.push(name.clone());
            }
        }
    }

    while let Some(name) = pending_values.pop() {
        if let Some(usage) = values.get(&name) {
            aggregate.merge_from(&usage.summary);
            for dependency in &usage.summary.value_refs {
                if values.contains_key(dependency) && reachable_values.insert(dependency.clone()) {
                    pending_values.push(dependency.clone());
                }
            }
        }
    }

    accumulate_extern_usage(&mut aggregate, externs);

    let mut reachable_types = HashSet::new();
    let mut pending_types = Vec::new();
    for type_name in &aggregate.type_refs {
        if types.contains_key(type_name) && reachable_types.insert(type_name.clone()) {
            pending_types.push(type_name.clone());
        }
    }
    while let Some(name) = pending_types.pop() {
        if let Some(usage) = types.get(&name) {
            aggregate.merge_from(&usage.summary);
            for dependency in &usage.summary.type_refs {
                if types.contains_key(dependency) && reachable_types.insert(dependency.clone()) {
                    pending_types.push(dependency.clone());
                }
            }
        }
    }

    Reachability {
        imports: aggregate.imports,
        externs: aggregate.externs,
        values: reachable_values,
        types: reachable_types,
    }
}

fn accumulate_extern_usage(aggregate: &mut UsageSummary, externs: &HashMap<String, NamedUsage>) {
    let mut pending = aggregate.externs.iter().cloned().collect::<Vec<_>>();
    let mut seen = aggregate.externs.clone();

    while let Some(path) = pending.pop() {
        if let Some(usage) = externs.get(&path) {
            aggregate
                .imports
                .extend(usage.summary.imports.iter().cloned());
            aggregate
                .type_refs
                .extend(usage.summary.type_refs.iter().cloned());
            aggregate
                .effect_refs
                .extend(usage.summary.effect_refs.iter().cloned());
            for dependency in &usage.summary.externs {
                if seen.insert(dependency.clone()) {
                    aggregate.externs.insert(dependency.clone());
                    pending.push(dependency.clone());
                }
            }
        }
    }
}

fn collect_function_usage_summary(
    function_decl: &FunctionDecl,
    top_level_values: &HashSet<String>,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
    constructor_to_type: &HashMap<String, String>,
    import_paths: &HashSet<String>,
    extern_paths: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    for param in &function_decl.params {
        if let Some(type_annotation) = &param.type_annotation {
            collect_type_usage(
                type_annotation,
                &mut summary,
                top_level_types,
                top_level_effects,
            );
        }
    }
    if let Some(return_type) = &function_decl.return_type {
        collect_type_usage(
            return_type,
            &mut summary,
            top_level_types,
            top_level_effects,
        );
    }
    collect_effect_usage(&function_decl.effects, &mut summary, top_level_effects);
    let mut scopes = vec![function_decl
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>()];
    collect_expr_usage(
        &function_decl.body,
        &mut scopes,
        &mut summary,
        top_level_values,
        top_level_types,
        top_level_effects,
        constructor_to_type,
        import_paths,
        extern_paths,
    );
    summary
}

fn collect_const_usage_summary(
    const_decl: &ConstDecl,
    initial_scopes: &[HashSet<String>],
    top_level_values: &HashSet<String>,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
    constructor_to_type: &HashMap<String, String>,
    import_paths: &HashSet<String>,
    extern_paths: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    if let Some(type_annotation) = &const_decl.type_annotation {
        collect_type_usage(
            type_annotation,
            &mut summary,
            top_level_types,
            top_level_effects,
        );
    }
    let mut scopes = initial_scopes.to_vec();
    collect_expr_usage(
        &const_decl.value,
        &mut scopes,
        &mut summary,
        top_level_values,
        top_level_types,
        top_level_effects,
        constructor_to_type,
        import_paths,
        extern_paths,
    );
    summary
}

fn collect_test_usage_summary(
    test_decl: &TestDecl,
    top_level_values: &HashSet<String>,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
    constructor_to_type: &HashMap<String, String>,
    import_paths: &HashSet<String>,
    extern_paths: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    collect_effect_usage(&test_decl.effects, &mut summary, top_level_effects);

    let mut world_scope = HashSet::new();
    for binding in &test_decl.world_bindings {
        let scope_frames = vec![world_scope.clone()];
        summary.merge_from(&collect_const_usage_summary(
            binding,
            &scope_frames,
            top_level_values,
            top_level_types,
            top_level_effects,
            constructor_to_type,
            import_paths,
            extern_paths,
        ));
        world_scope.insert(binding.name.clone());
    }

    let mut scopes = vec![world_scope];
    collect_expr_usage(
        &test_decl.body,
        &mut scopes,
        &mut summary,
        top_level_values,
        top_level_types,
        top_level_effects,
        constructor_to_type,
        import_paths,
        extern_paths,
    );
    summary
}

fn collect_type_decl_usage_summary(
    type_decl: &TypeDecl,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
    import_paths: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    for label_ref in &type_decl.labels {
        collect_label_ref_usage(label_ref, &mut summary, top_level_types);
    }
    match &type_decl.definition {
        TypeDef::Sum(sum_type) => {
            for variant in &sum_type.variants {
                for typ in &variant.types {
                    collect_type_usage(typ, &mut summary, top_level_types, top_level_effects);
                }
            }
        }
        TypeDef::Product(product_type) => {
            for field in &product_type.fields {
                collect_type_usage(
                    &field.field_type,
                    &mut summary,
                    top_level_types,
                    top_level_effects,
                );
            }
        }
        TypeDef::Alias(alias) => {
            collect_type_usage(
                &alias.aliased_type,
                &mut summary,
                top_level_types,
                top_level_effects,
            );
        }
    }
    summary.imports.retain(|path| import_paths.contains(path));
    summary
}

fn collect_label_decl_usage_summary(
    label_decl: &LabelDecl,
    top_level_types: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    for label_ref in &label_decl.combines {
        collect_label_ref_usage(label_ref, &mut summary, top_level_types);
    }
    summary
}

fn collect_rule_usage_summary(
    rule_decl: &RuleDecl,
    top_level_values: &HashSet<String>,
    top_level_types: &HashSet<String>,
    import_paths: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    for label_ref in &rule_decl.labels {
        collect_label_ref_usage(label_ref, &mut summary, top_level_types);
    }
    if !rule_decl.boundary.module_path.is_empty() {
        summary
            .imports
            .insert(rule_decl.boundary.module_path.join("::"));
    }
    if let RuleAction::Through { transform, .. } = &rule_decl.action {
        collect_member_ref_usage(transform, &mut summary, top_level_values);
    }
    summary.imports.retain(|path| import_paths.contains(path));
    summary
}

fn collect_effect_decl_usage_summary(
    effect_decl: &EffectDecl,
    top_level_effects: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    collect_effect_usage(&effect_decl.effects, &mut summary, top_level_effects);
    summary
}

fn collect_extern_usage_summary(
    extern_decl: &ExternDecl,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    if let Some(members) = &extern_decl.members {
        for member in members {
            collect_type_usage(
                &member.member_type,
                &mut summary,
                top_level_types,
                top_level_effects,
            );
        }
    }
    summary
}

fn collect_effect_usage(
    effects: &[String],
    summary: &mut UsageSummary,
    top_level_effects: &HashSet<String>,
) {
    for effect in effects {
        if top_level_effects.contains(effect) {
            summary.effect_refs.insert(effect.clone());
        }
    }
}

fn collect_label_ref_usage(
    label_ref: &LabelRef,
    summary: &mut UsageSummary,
    top_level_types: &HashSet<String>,
) {
    if label_ref.module_path.is_empty() && top_level_types.contains(&label_ref.name) {
        summary.type_refs.insert(label_ref.name.clone());
    } else if !label_ref.module_path.is_empty() {
        summary.imports.insert(label_ref.module_path.join("::"));
    }
}

fn collect_member_ref_usage(
    member_ref: &MemberRef,
    summary: &mut UsageSummary,
    top_level_values: &HashSet<String>,
) {
    if member_ref.module_path.is_empty() && top_level_values.contains(&member_ref.member) {
        summary.value_refs.insert(member_ref.member.clone());
    } else if !member_ref.module_path.is_empty() {
        summary.imports.insert(member_ref.module_path.join("::"));
    }
}

fn collect_type_usage(
    typ: &Type,
    summary: &mut UsageSummary,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
) {
    match typ {
        Type::Primitive(_) => {}
        Type::Variable(variable) => {
            if top_level_types.contains(&variable.name) {
                summary.type_refs.insert(variable.name.clone());
            }
        }
        Type::List(list_type) => {
            collect_type_usage(
                &list_type.element_type,
                summary,
                top_level_types,
                top_level_effects,
            );
        }
        Type::Map(map_type) => {
            collect_type_usage(
                &map_type.key_type,
                summary,
                top_level_types,
                top_level_effects,
            );
            collect_type_usage(
                &map_type.value_type,
                summary,
                top_level_types,
                top_level_effects,
            );
        }
        Type::Function(function_type) => {
            for param_type in &function_type.param_types {
                collect_type_usage(param_type, summary, top_level_types, top_level_effects);
            }
            collect_effect_usage(&function_type.effects, summary, top_level_effects);
            collect_type_usage(
                &function_type.return_type,
                summary,
                top_level_types,
                top_level_effects,
            );
        }
        Type::Constructor(constructor) => {
            if top_level_types.contains(&constructor.name) {
                summary.type_refs.insert(constructor.name.clone());
            }
            for type_arg in &constructor.type_args {
                collect_type_usage(type_arg, summary, top_level_types, top_level_effects);
            }
        }
        Type::Tuple(tuple_type) => {
            for nested in &tuple_type.types {
                collect_type_usage(nested, summary, top_level_types, top_level_effects);
            }
        }
        Type::Qualified(qualified) => {
            summary.imports.insert(qualified.module_path.join("::"));
            for type_arg in &qualified.type_args {
                collect_type_usage(type_arg, summary, top_level_types, top_level_effects);
            }
        }
    }
}

fn collect_expr_usage(
    expr: &Expr,
    scopes: &mut Vec<HashSet<String>>,
    summary: &mut UsageSummary,
    top_level_values: &HashSet<String>,
    top_level_types: &HashSet<String>,
    top_level_effects: &HashSet<String>,
    constructor_to_type: &HashMap<String, String>,
    import_paths: &HashSet<String>,
    extern_paths: &HashSet<String>,
) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Identifier(identifier) => {
            if !is_locally_bound(&identifier.name, scopes) {
                if top_level_values.contains(&identifier.name) {
                    summary.value_refs.insert(identifier.name.clone());
                }
                if let Some(type_name) = constructor_to_type.get(&identifier.name) {
                    summary.type_refs.insert(type_name.clone());
                }
            }
        }
        Expr::Lambda(lambda) => {
            for param in &lambda.params {
                if let Some(type_annotation) = &param.type_annotation {
                    collect_type_usage(
                        type_annotation,
                        summary,
                        top_level_types,
                        top_level_effects,
                    );
                }
            }
            collect_effect_usage(&lambda.effects, summary, top_level_effects);
            collect_type_usage(
                &lambda.return_type,
                summary,
                top_level_types,
                top_level_effects,
            );
            let mut child_scopes = scopes.clone();
            child_scopes.push(
                lambda
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<HashSet<_>>(),
            );
            collect_expr_usage(
                &lambda.body,
                &mut child_scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Application(application) => {
            collect_expr_usage(
                &application.func,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            for arg in &application.args {
                collect_expr_usage(
                    arg,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::Binary(binary) => {
            collect_expr_usage(
                &binary.left,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &binary.right,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Unary(unary) => {
            collect_expr_usage(
                &unary.operand,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Match(match_expr) => {
            collect_expr_usage(
                &match_expr.scrutinee,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            for arm in &match_expr.arms {
                collect_pattern_usage(
                    &arm.pattern,
                    summary,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
                let mut child_scopes = scopes.clone();
                child_scopes.push(pattern_binding_frame(&arm.pattern));
                if let Some(guard) = &arm.guard {
                    collect_expr_usage(
                        guard,
                        &mut child_scopes,
                        summary,
                        top_level_values,
                        top_level_types,
                        top_level_effects,
                        constructor_to_type,
                        import_paths,
                        extern_paths,
                    );
                }
                collect_expr_usage(
                    &arm.body,
                    &mut child_scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::Let(let_expr) => {
            collect_expr_usage(
                &let_expr.value,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_pattern_usage(
                &let_expr.pattern,
                summary,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            let mut child_scopes = scopes.clone();
            child_scopes.push(pattern_binding_frame(&let_expr.pattern));
            collect_expr_usage(
                &let_expr.body,
                &mut child_scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Using(using_expr) => {
            collect_expr_usage(
                &using_expr.value,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            let mut child_scopes = scopes.clone();
            child_scopes.push(HashSet::from([using_expr.name.clone()]));
            collect_expr_usage(
                &using_expr.body,
                &mut child_scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::If(if_expr) => {
            collect_expr_usage(
                &if_expr.condition,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &if_expr.then_branch,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            if let Some(else_branch) = &if_expr.else_branch {
                collect_expr_usage(
                    else_branch,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::List(list_expr) => {
            for element in &list_expr.elements {
                collect_expr_usage(
                    element,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::Record(record_expr) => {
            for field in &record_expr.fields {
                collect_expr_usage(
                    &field.value,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::MapLiteral(map_expr) => {
            for entry in &map_expr.entries {
                collect_expr_usage(
                    &entry.key,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
                collect_expr_usage(
                    &entry.value,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::Tuple(tuple_expr) => {
            for element in &tuple_expr.elements {
                collect_expr_usage(
                    element,
                    scopes,
                    summary,
                    top_level_values,
                    top_level_types,
                    top_level_effects,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Expr::FieldAccess(field_access) => {
            collect_expr_usage(
                &field_access.object,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            if let Expr::Identifier(identifier) = &field_access.object {
                if !is_locally_bound(&identifier.name, scopes)
                    && extern_paths.contains(&identifier.name)
                {
                    summary.externs.insert(identifier.name.clone());
                }
            }
        }
        Expr::Index(index_expr) => {
            collect_expr_usage(
                &index_expr.object,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &index_expr.index,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Pipeline(pipeline) => {
            collect_expr_usage(
                &pipeline.left,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &pipeline.right,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Map(map_expr) => {
            collect_expr_usage(
                &map_expr.list,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &map_expr.func,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Filter(filter_expr) => {
            collect_expr_usage(
                &filter_expr.list,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &filter_expr.predicate,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Fold(fold_expr) => {
            collect_expr_usage(
                &fold_expr.list,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &fold_expr.func,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_expr_usage(
                &fold_expr.init,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
        }
        Expr::Concurrent(concurrent_expr) => {
            collect_expr_usage(
                &concurrent_expr.width,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            if let Some(policy) = &concurrent_expr.policy {
                for field in &policy.fields {
                    collect_expr_usage(
                        &field.value,
                        scopes,
                        summary,
                        top_level_values,
                        top_level_types,
                        top_level_effects,
                        constructor_to_type,
                        import_paths,
                        extern_paths,
                    );
                }
            }
            for step in &concurrent_expr.steps {
                match step {
                    ConcurrentStep::Spawn(spawn_step) => collect_expr_usage(
                        &spawn_step.expr,
                        scopes,
                        summary,
                        top_level_values,
                        top_level_types,
                        top_level_effects,
                        constructor_to_type,
                        import_paths,
                        extern_paths,
                    ),
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        collect_expr_usage(
                            &spawn_each.list,
                            scopes,
                            summary,
                            top_level_values,
                            top_level_types,
                            top_level_effects,
                            constructor_to_type,
                            import_paths,
                            extern_paths,
                        );
                        collect_expr_usage(
                            &spawn_each.func,
                            scopes,
                            summary,
                            top_level_values,
                            top_level_types,
                            top_level_effects,
                            constructor_to_type,
                            import_paths,
                            extern_paths,
                        );
                    }
                }
            }
        }
        Expr::MemberAccess(member_access) => {
            let namespace = member_access.namespace.join("::");
            if import_paths.contains(&namespace) {
                summary.imports.insert(namespace);
            } else if extern_paths.contains(&namespace) {
                summary.externs.insert(namespace);
            }
        }
        Expr::TypeAscription(type_ascription) => {
            collect_expr_usage(
                &type_ascription.expr,
                scopes,
                summary,
                top_level_values,
                top_level_types,
                top_level_effects,
                constructor_to_type,
                import_paths,
                extern_paths,
            );
            collect_type_usage(
                &type_ascription.ascribed_type,
                summary,
                top_level_types,
                top_level_effects,
            );
        }
    }
}

fn collect_pattern_usage(
    pattern: &Pattern,
    summary: &mut UsageSummary,
    constructor_to_type: &HashMap<String, String>,
    import_paths: &HashSet<String>,
    extern_paths: &HashSet<String>,
) {
    match pattern {
        Pattern::Literal(_) | Pattern::Identifier(_) | Pattern::Wildcard(_) => {}
        Pattern::Constructor(constructor_pattern) => {
            if !constructor_pattern.module_path.is_empty() {
                let namespace = constructor_pattern.module_path.join("::");
                if import_paths.contains(&namespace) {
                    summary.imports.insert(namespace);
                } else if extern_paths.contains(&namespace) {
                    summary.externs.insert(namespace);
                }
            } else if let Some(type_name) = constructor_to_type.get(&constructor_pattern.name) {
                summary.type_refs.insert(type_name.clone());
            }
            for nested in &constructor_pattern.patterns {
                collect_pattern_usage(
                    nested,
                    summary,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Pattern::List(list_pattern) => {
            for nested in &list_pattern.patterns {
                collect_pattern_usage(
                    nested,
                    summary,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
        Pattern::Record(record_pattern) => {
            for field in &record_pattern.fields {
                if let Some(nested) = &field.pattern {
                    collect_pattern_usage(
                        nested,
                        summary,
                        constructor_to_type,
                        import_paths,
                        extern_paths,
                    );
                }
            }
        }
        Pattern::Tuple(tuple_pattern) => {
            for nested in &tuple_pattern.patterns {
                collect_pattern_usage(
                    nested,
                    summary,
                    constructor_to_type,
                    import_paths,
                    extern_paths,
                );
            }
        }
    }
}

fn pattern_binding_frame(pattern: &Pattern) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_pattern_binding_names(pattern, &mut names);
    names
}

fn collect_pattern_binding_names(pattern: &Pattern, names: &mut HashSet<String>) {
    match pattern {
        Pattern::Identifier(identifier_pattern) => {
            names.insert(identifier_pattern.name.clone());
        }
        Pattern::Wildcard(_) | Pattern::Literal(_) => {}
        Pattern::Constructor(constructor_pattern) => {
            for nested in &constructor_pattern.patterns {
                collect_pattern_binding_names(nested, names);
            }
        }
        Pattern::List(list_pattern) => {
            for nested in &list_pattern.patterns {
                collect_pattern_binding_names(nested, names);
            }
            if let Some(rest_name) = &list_pattern.rest {
                names.insert(rest_name.clone());
            }
        }
        Pattern::Record(record_pattern) => {
            for field in &record_pattern.fields {
                match &field.pattern {
                    Some(nested) => collect_pattern_binding_names(nested, names),
                    None => {
                        names.insert(field.name.clone());
                    }
                }
            }
        }
        Pattern::Tuple(tuple_pattern) => {
            for nested in &tuple_pattern.patterns {
                collect_pattern_binding_names(nested, names);
            }
        }
    }
}

fn is_locally_bound(name: &str, scopes: &[HashSet<String>]) -> bool {
    scopes.iter().rev().any(|scope| scope.contains(name))
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
        start: sigil_lexer::Position {
            line: 1,
            column: 1,
            offset: 0,
        },
        end: sigil_lexer::Position {
            line: 1,
            column: 1,
            offset: 0,
        },
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
        Expr::Using(using_expr) => {
            if !is_lower_camel_case(&using_expr.name) {
                errors.push(ValidationError::IdentifierForm {
                    found: using_expr.name.clone(),
                    suggestion: suggestion_suffix(
                        &using_expr.name,
                        to_lower_camel_case(&using_expr.name),
                    ),
                    location: using_expr.location,
                });
            }
            validate_identifier_forms_in_expr(&using_expr.value, errors);
            validate_identifier_forms_in_expr(&using_expr.body, errors);
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
                        suggestion: suggestion_suffix(
                            &field.name,
                            to_lower_camel_case(&field.name),
                        ),
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
        Expr::Concurrent(concurrent) => {
            if !is_lower_camel_case(&concurrent.name) {
                errors.push(ValidationError::IdentifierForm {
                    found: concurrent.name.clone(),
                    suggestion: suggestion_suffix(
                        &concurrent.name,
                        to_lower_camel_case(&concurrent.name),
                    ),
                    location: concurrent.location,
                });
            }
            validate_identifier_forms_in_expr(&concurrent.width, errors);
            if let Some(policy) = &concurrent.policy {
                for field in &policy.fields {
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
                    validate_identifier_forms_in_expr(&field.value, errors);
                }
            }
            for step in &concurrent.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => {
                        validate_identifier_forms_in_expr(&spawn.expr, errors)
                    }
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        validate_identifier_forms_in_expr(&spawn_each.list, errors);
                        validate_identifier_forms_in_expr(&spawn_each.func, errors);
                    }
                }
            }
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
        Expr::TypeAscription(type_ascription) => {
            validate_identifier_forms_in_expr(&type_ascription.expr, errors);
            validate_identifier_forms_in_type(&type_ascription.ascribed_type, errors);
        }
    }
}

fn validate_identifier_forms_in_label_ref(label_ref: &LabelRef, errors: &mut Vec<ValidationError>) {
    for segment in &label_ref.module_path {
        if !is_lower_camel_case(segment) {
            errors.push(ValidationError::ModulePathForm {
                found: segment.clone(),
                suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                location: label_ref.location,
            });
        }
    }
    if !is_upper_camel_case(&label_ref.name) {
        errors.push(ValidationError::TypeNameForm {
            found: label_ref.name.clone(),
            suggestion: suggestion_suffix(&label_ref.name, to_upper_camel_case(&label_ref.name)),
            location: label_ref.location,
        });
    }
}

fn validate_identifier_forms_in_member_ref(
    member_ref: &MemberRef,
    errors: &mut Vec<ValidationError>,
) {
    for segment in &member_ref.module_path {
        if !is_lower_camel_case(segment) {
            errors.push(ValidationError::ModulePathForm {
                found: segment.clone(),
                suggestion: suggestion_suffix(segment, to_lower_camel_case(segment)),
                location: member_ref.location,
            });
        }
    }
    if !is_lower_camel_case(&member_ref.member) {
        errors.push(ValidationError::IdentifierForm {
            found: member_ref.member.clone(),
            suggestion: suggestion_suffix(
                &member_ref.member,
                to_lower_camel_case(&member_ref.member),
            ),
            location: member_ref.location,
        });
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
            Declaration::Derive(derive_decl) => {
                validate_identifier_forms_in_type(&derive_decl.target, errors);
            }
            Declaration::Transform(transform_decl) => {
                let function = &transform_decl.function;
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
            Declaration::Label(label_decl) => {
                if !is_upper_camel_case(&label_decl.name) {
                    errors.push(ValidationError::TypeNameForm {
                        found: label_decl.name.clone(),
                        suggestion: suggestion_suffix(
                            &label_decl.name,
                            to_upper_camel_case(&label_decl.name),
                        ),
                        location: label_decl.location,
                    });
                }
                for label_ref in &label_decl.combines {
                    validate_identifier_forms_in_label_ref(label_ref, errors);
                }
            }
            Declaration::Effect(effect_decl) => {
                if !is_upper_camel_case(&effect_decl.name) {
                    errors.push(ValidationError::TypeNameForm {
                        found: effect_decl.name.clone(),
                        suggestion: suggestion_suffix(
                            &effect_decl.name,
                            to_upper_camel_case(&effect_decl.name),
                        ),
                        location: effect_decl.location,
                    });
                }
            }
            Declaration::FeatureFlag(feature_flag_decl) => {
                if !is_upper_camel_case(&feature_flag_decl.name) {
                    errors.push(ValidationError::TypeNameForm {
                        found: feature_flag_decl.name.clone(),
                        suggestion: suggestion_suffix(
                            &feature_flag_decl.name,
                            to_upper_camel_case(&feature_flag_decl.name),
                        ),
                        location: feature_flag_decl.location,
                    });
                }
                validate_identifier_forms_in_type(&feature_flag_decl.flag_type, errors);
                validate_identifier_forms_in_expr(&feature_flag_decl.default, errors);
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
            Declaration::Test(test_decl) => {
                validate_identifier_forms_in_expr(&test_decl.body, errors)
            }
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
            Declaration::Rule(rule_decl) => {
                for label_ref in &rule_decl.labels {
                    validate_identifier_forms_in_label_ref(label_ref, errors);
                }
                validate_identifier_forms_in_member_ref(&rule_decl.boundary, errors);
                if let RuleAction::Through { transform, .. } = &rule_decl.action {
                    validate_identifier_forms_in_member_ref(transform, errors);
                }
            }
            Declaration::Protocol(protocol_decl) => {
                if !is_upper_camel_case(&protocol_decl.name) {
                    errors.push(ValidationError::TypeNameForm {
                        found: protocol_decl.name.clone(),
                        suggestion: suggestion_suffix(
                            &protocol_decl.name,
                            to_upper_camel_case(&protocol_decl.name),
                        ),
                        location: protocol_decl.location,
                    });
                }
                for state in std::iter::once(&protocol_decl.initial)
                    .chain(std::iter::once(&protocol_decl.terminal))
                    .chain(
                        protocol_decl
                            .transitions
                            .iter()
                            .flat_map(|t| [&t.from, &t.to]),
                    )
                {
                    if !is_upper_camel_case(state) {
                        errors.push(ValidationError::TypeNameForm {
                            found: state.clone(),
                            suggestion: suggestion_suffix(state, to_upper_camel_case(state)),
                            location: protocol_decl.location,
                        });
                    }
                }
            }
        }
    }
}

/// Validate that project test blocks only appear in tests/ directories
fn validate_test_location(program: &Program, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let has_tests = program
        .declarations
        .iter()
        .any(|d| matches!(d, Declaration::Test(_)));

    if !has_tests {
        return Ok(());
    }

    // Normalize path separators
    let normalized_path = file_path.replace('\\', "/");

    if find_project_root(Path::new(file_path)).is_none() {
        return Ok(());
    }

    // Check if file is in a tests/ directory
    if !normalized_path.contains("/tests/") {
        return Err(vec![ValidationError::TestLocationInvalid {
            message: format!(
                "project test blocks can only appear in files under tests/ directories.\n\n\
                This file belongs to a Sigil project and contains test blocks but is not in a tests/ directory.\n\n\
                Move this file to a tests/ directory (e.g., tests/your-test.sigil).\n\n\
                Standalone non-project files may keep tests inline, but project mode keeps tests in tests/ directories."
            ),
            file_path: normalized_path,
        }]);
    }

    Ok(())
}

fn validate_direct_canonical_helper_wrappers(
    program: &Program,
    file_path: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    if file_path.is_some_and(is_stdlib_file_path) {
        return Ok(());
    }

    let mut errors = Vec::new();

    for declaration in &program.declarations {
        let Declaration::Function(function) = declaration else {
            continue;
        };
        let Some(wrapper) = detect_direct_canonical_helper_wrapper(function) else {
            continue;
        };
        errors.push(ValidationError::HelperDirectWrapper {
            function_name: function.name.clone(),
            canonical_helper: wrapper.canonical_helper,
            canonical_surface: wrapper.canonical_surface,
            location: function.location,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn is_stdlib_file_path(file_path: &str) -> bool {
    let normalized = file_path.replace('\\', "/");
    normalized.starts_with("language/stdlib/")
        || normalized.starts_with("stdlib/")
        || normalized.contains("/language/stdlib/")
}

fn detect_direct_canonical_helper_wrapper(func: &FunctionDecl) -> Option<CanonicalHelperWrapper> {
    detect_direct_stdlib_wrapper(func).or_else(|| detect_direct_operator_wrapper(func))
}

fn detect_direct_stdlib_wrapper(func: &FunctionDecl) -> Option<CanonicalHelperWrapper> {
    if func.params.is_empty() {
        return None;
    }

    let Expr::Application(application) = strip_type_ascriptions(&func.body) else {
        return None;
    };
    let Expr::MemberAccess(member) = strip_type_ascriptions(&application.func) else {
        return None;
    };
    let canonical_helper = stdlib_helper_name(member)?;
    if application.args.len() != func.params.len() {
        return None;
    }

    let param_names: Vec<&str> = func
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    if !application
        .args
        .iter()
        .zip(param_names.iter())
        .all(|(arg, param_name)| identifier_name(strip_type_ascriptions(arg)) == Some(*param_name))
    {
        return None;
    }

    Some(CanonicalHelperWrapper {
        canonical_surface: format!("{}({})", canonical_helper, param_names.join(",")),
        canonical_helper,
    })
}

fn detect_direct_operator_wrapper(func: &FunctionDecl) -> Option<CanonicalHelperWrapper> {
    match strip_type_ascriptions(&func.body) {
        Expr::Map(map_expr) => {
            let list_name = identifier_name(strip_type_ascriptions(&map_expr.list))?;
            let func_name = identifier_name(strip_type_ascriptions(&map_expr.func))?;
            operator_wrapper(
                func,
                &[list_name, func_name],
                "map",
                format!("{} map {}", list_name, func_name),
            )
        }
        Expr::Filter(filter_expr) => {
            let list_name = identifier_name(strip_type_ascriptions(&filter_expr.list))?;
            let predicate_name = identifier_name(strip_type_ascriptions(&filter_expr.predicate))?;
            operator_wrapper(
                func,
                &[list_name, predicate_name],
                "filter",
                format!("{} filter {}", list_name, predicate_name),
            )
        }
        Expr::Fold(fold_expr) => {
            let list_name = identifier_name(strip_type_ascriptions(&fold_expr.list))?;
            let func_name = identifier_name(strip_type_ascriptions(&fold_expr.func))?;
            let init_name = identifier_name(strip_type_ascriptions(&fold_expr.init))?;
            operator_wrapper(
                func,
                &[list_name, func_name, init_name],
                "reduce ... from ...",
                format!("{} reduce {} from {}", list_name, func_name, init_name),
            )
        }
        _ => None,
    }
}

fn operator_wrapper(
    func: &FunctionDecl,
    names: &[&str],
    canonical_helper: &str,
    canonical_surface: String,
) -> Option<CanonicalHelperWrapper> {
    if func.params.is_empty() || names.len() != func.params.len() {
        return None;
    }

    let param_names: HashSet<&str> = func
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect();
    let wrapper_names: HashSet<&str> = names.iter().copied().collect();
    if param_names.len() != func.params.len()
        || wrapper_names.len() != names.len()
        || param_names != wrapper_names
    {
        return None;
    }

    Some(CanonicalHelperWrapper {
        canonical_helper: canonical_helper.to_string(),
        canonical_surface,
    })
}

fn strip_type_ascriptions(expr: &Expr) -> &Expr {
    match expr {
        Expr::TypeAscription(type_ascription) => strip_type_ascriptions(&type_ascription.expr),
        other => other,
    }
}

fn identifier_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Identifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn stdlib_helper_name(member: &MemberAccessExpr) -> Option<String> {
    if member.namespace.first().map(String::as_str) != Some("stdlib") || member.namespace.len() < 2
    {
        return None;
    }
    Some(format!(
        "§{}.{}",
        member.namespace[1..].join("."),
        member.member
    ))
}

/// Validate recursive functions don't use accumulator parameters
fn validate_recursive_functions(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for decl in &program.declarations {
        let func = match decl {
            Declaration::Function(func) => func,
            Declaration::Transform(transform) => &transform.function,
            _ => continue,
        };

        // Check if function is recursive
        if !is_recursive(&func.body, &func.name) {
            if func.mode == FunctionMode::Ordinary && func.decreases.is_some() {
                errors.push(ValidationError::OrdinaryFunctionDecreases {
                    function_name: func.name.clone(),
                    location: func.location,
                });
            }
            continue;
        }

        // Every self-recursive function must declare a termination measure,
        // EXCEPT functions whose return type is Never. A Never-returning
        // function intentionally does not return normally (it diverges,
        // throws, or hands off to the runtime), so the decreases obligation
        // does not apply.
        let returns_never = matches!(
            &func.return_type,
            Some(Type::Primitive(PrimitiveType {
                name: PrimitiveName::Never,
                ..
            }))
        );
        if func.mode == FunctionMode::Ordinary {
            if func.decreases.is_some() {
                errors.push(ValidationError::OrdinaryFunctionDecreases {
                    function_name: func.name.clone(),
                    location: func.location,
                });
            }
        } else if !returns_never && func.decreases.is_none() {
            errors.push(ValidationError::RecursionMissingDecreases {
                function_name: func.name.clone(),
                location: func.location,
            });
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

        if detect_exact_recursive_reverse_clone(func) {
            errors.push(ValidationError::RecursiveReverseClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_all_clone(func) {
            errors.push(ValidationError::RecursiveAllClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_any_clone(func) {
            errors.push(ValidationError::RecursiveAnyClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_map_clone(func) {
            errors.push(ValidationError::RecursiveMapClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_filter_clone(func) {
            errors.push(ValidationError::RecursiveFilterClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_find_clone(func) {
            errors.push(ValidationError::RecursiveFindClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_flat_map_clone(func) {
            errors.push(ValidationError::RecursiveFlatMapClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if detect_exact_recursive_fold_clone(func) {
            errors.push(ValidationError::RecursiveFoldClone {
                function_name: func.name.clone(),
                location: func.location,
            });
        } else if contains_recursive_append_result(&func.body, &func.name) {
            errors.push(ValidationError::RecursiveAppendResult {
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

    collect_filter_then_count_errors(program, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Reject mutual recursion between top-level functions in the same file/module.
///
/// Cross-module cycles can't exist in Sigil today: rooted module references like
/// `•other.bar` use a different name space, and module cycles are already
/// rejected by an earlier pass. So a name-only call graph over local top-level
/// function declarations is sufficient given Sigil's no-shadowing invariant.
///
/// Self-recursion (size-1 SCC) is allowed and handled by the missing-decreases
/// check; this pass only rejects SCCs of size >= 2.
fn validate_mutual_recursion(program: &Program) -> Result<(), Vec<ValidationError>> {
    use std::collections::{BTreeMap, HashMap};

    let mut errors = Vec::new();

    // Map of top-level function name to its declaration location and body.
    let mut local_functions: BTreeMap<String, (SourceLocation, &Expr)> = BTreeMap::new();
    for decl in &program.declarations {
        let func = match decl {
            Declaration::Function(func) => func,
            Declaration::Transform(transform) => &transform.function,
            _ => continue,
        };
        local_functions.insert(func.name.clone(), (func.location, &func.body));
    }

    // Build the call graph: for each function, the set of other local functions
    // it calls (excluding itself; self-edges are handled by missing-decreases).
    let mut call_graph: HashMap<String, Vec<String>> = HashMap::new();
    for (name, (_, body)) in &local_functions {
        let mut callees = Vec::new();
        collect_local_callees(body, name, &local_functions, &mut callees);
        callees.sort();
        callees.dedup();
        call_graph.insert(name.clone(), callees);
    }

    // Find SCCs via Tarjan's algorithm.
    let sccs = strongly_connected_components(&call_graph);

    for scc in sccs {
        if scc.len() < 2 {
            continue;
        }
        // Sort cycle members alphabetically for deterministic error text.
        let mut cycle_names = scc.clone();
        cycle_names.sort();
        // Report the error at the location of the alphabetically-first function.
        let location = cycle_names
            .iter()
            .filter_map(|name| local_functions.get(name).map(|(loc, _)| *loc))
            .next()
            .unwrap_or_else(|| SourceLocation {
                start: sigil_lexer::Position::new(1, 1, 0),
                end: sigil_lexer::Position::new(1, 1, 0),
            });
        errors.push(ValidationError::MutualRecursion {
            cycle: cycle_names.join(", "),
            location,
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Collect identifier-shaped calls in `expr` whose callee is a known local
/// top-level function (excluding `self_name`, which is the function whose
/// body we're walking; self-edges are excluded so SCC analysis only flags
/// cycles of size >= 2).
fn collect_local_callees(
    expr: &Expr,
    self_name: &str,
    local_functions: &std::collections::BTreeMap<String, (SourceLocation, &Expr)>,
    out: &mut Vec<String>,
) {
    match expr {
        Expr::Application(app) => {
            if let Expr::Identifier(ident) = &app.func {
                if ident.name != self_name && local_functions.contains_key(&ident.name) {
                    out.push(ident.name.clone());
                }
            }
            collect_local_callees(&app.func, self_name, local_functions, out);
            for arg in &app.args {
                collect_local_callees(arg, self_name, local_functions, out);
            }
        }
        Expr::Identifier(_) | Expr::Literal(_) | Expr::MemberAccess(_) => {}
        Expr::Lambda(lambda) => {
            collect_local_callees(&lambda.body, self_name, local_functions, out);
        }
        Expr::Binary(bin) => {
            collect_local_callees(&bin.left, self_name, local_functions, out);
            collect_local_callees(&bin.right, self_name, local_functions, out);
        }
        Expr::Unary(un) => collect_local_callees(&un.operand, self_name, local_functions, out),
        Expr::Match(m) => {
            collect_local_callees(&m.scrutinee, self_name, local_functions, out);
            for arm in &m.arms {
                if let Some(g) = &arm.guard {
                    collect_local_callees(g, self_name, local_functions, out);
                }
                collect_local_callees(&arm.body, self_name, local_functions, out);
            }
        }
        Expr::Let(l) => {
            collect_local_callees(&l.value, self_name, local_functions, out);
            collect_local_callees(&l.body, self_name, local_functions, out);
        }
        Expr::Using(using_expr) => {
            collect_local_callees(&using_expr.value, self_name, local_functions, out);
            collect_local_callees(&using_expr.body, self_name, local_functions, out);
        }
        Expr::If(i) => {
            collect_local_callees(&i.condition, self_name, local_functions, out);
            collect_local_callees(&i.then_branch, self_name, local_functions, out);
            if let Some(else_branch) = &i.else_branch {
                collect_local_callees(else_branch, self_name, local_functions, out);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                collect_local_callees(element, self_name, local_functions, out);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                collect_local_callees(&field.value, self_name, local_functions, out);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                collect_local_callees(&entry.key, self_name, local_functions, out);
                collect_local_callees(&entry.value, self_name, local_functions, out);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_local_callees(element, self_name, local_functions, out);
            }
        }
        Expr::FieldAccess(field_access) => {
            collect_local_callees(&field_access.object, self_name, local_functions, out);
        }
        Expr::Index(index) => {
            collect_local_callees(&index.object, self_name, local_functions, out);
            collect_local_callees(&index.index, self_name, local_functions, out);
        }
        Expr::Pipeline(pipeline) => {
            collect_local_callees(&pipeline.left, self_name, local_functions, out);
            collect_local_callees(&pipeline.right, self_name, local_functions, out);
        }
        Expr::Map(m) => {
            collect_local_callees(&m.list, self_name, local_functions, out);
            collect_local_callees(&m.func, self_name, local_functions, out);
        }
        Expr::Filter(f) => {
            collect_local_callees(&f.list, self_name, local_functions, out);
            collect_local_callees(&f.predicate, self_name, local_functions, out);
        }
        Expr::Fold(f) => {
            collect_local_callees(&f.list, self_name, local_functions, out);
            collect_local_callees(&f.init, self_name, local_functions, out);
            collect_local_callees(&f.func, self_name, local_functions, out);
        }
        Expr::Concurrent(concurrent) => {
            for step in &concurrent.steps {
                match step {
                    sigil_ast::ConcurrentStep::Spawn(spawn) => {
                        collect_local_callees(&spawn.expr, self_name, local_functions, out);
                    }
                    sigil_ast::ConcurrentStep::SpawnEach(spawn_each) => {
                        collect_local_callees(&spawn_each.list, self_name, local_functions, out);
                        collect_local_callees(&spawn_each.func, self_name, local_functions, out);
                    }
                }
            }
        }
        Expr::TypeAscription(type_ascription) => {
            collect_local_callees(&type_ascription.expr, self_name, local_functions, out);
        }
    }
}

/// Tarjan's strongly-connected components algorithm.
fn strongly_connected_components(
    graph: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    use std::collections::HashMap;

    struct State<'a> {
        graph: &'a HashMap<String, Vec<String>>,
        index_counter: usize,
        stack: Vec<String>,
        on_stack: HashMap<String, bool>,
        index: HashMap<String, usize>,
        lowlink: HashMap<String, usize>,
        sccs: Vec<Vec<String>>,
    }

    fn strong_connect(state: &mut State, node: &str) {
        state.index.insert(node.to_string(), state.index_counter);
        state.lowlink.insert(node.to_string(), state.index_counter);
        state.index_counter += 1;
        state.stack.push(node.to_string());
        state.on_stack.insert(node.to_string(), true);

        if let Some(neighbors) = state.graph.get(node) {
            for neighbor in neighbors.clone() {
                if !state.index.contains_key(&neighbor) {
                    strong_connect(state, &neighbor);
                    let neighbor_low = state.lowlink[&neighbor];
                    let node_low = state.lowlink[node];
                    state
                        .lowlink
                        .insert(node.to_string(), node_low.min(neighbor_low));
                } else if *state.on_stack.get(&neighbor).unwrap_or(&false) {
                    let neighbor_index = state.index[&neighbor];
                    let node_low = state.lowlink[node];
                    state
                        .lowlink
                        .insert(node.to_string(), node_low.min(neighbor_index));
                }
            }
        }

        if state.lowlink[node] == state.index[node] {
            let mut scc = Vec::new();
            while let Some(member) = state.stack.pop() {
                state.on_stack.insert(member.clone(), false);
                let is_root = member == node;
                scc.push(member);
                if is_root {
                    break;
                }
            }
            state.sccs.push(scc);
        }
    }

    let mut state = State {
        graph,
        index_counter: 0,
        stack: Vec::new(),
        on_stack: HashMap::new(),
        index: HashMap::new(),
        lowlink: HashMap::new(),
        sccs: Vec::new(),
    };

    let mut nodes: Vec<&String> = graph.keys().collect();
    nodes.sort();
    for node in nodes {
        if !state.index.contains_key(node) {
            strong_connect(&mut state, node);
        }
    }

    state.sccs
}

/// Check if an expression contains a recursive call to the given function
fn is_recursive(expr: &Expr, function_name: &str) -> bool {
    match expr {
        Expr::Application(app) => {
            // Check if calling itself
            if matches!(app.func, Expr::Identifier(IdentifierExpr { ref name, .. }) if name == function_name)
            {
                return true;
            }
            // Check function and args
            is_recursive(&app.func, function_name)
                || app.args.iter().any(|arg| is_recursive(arg, function_name))
        }

        Expr::Identifier(_) | Expr::Literal(_) => false,

        Expr::Lambda(lambda) => is_recursive(&lambda.body, function_name),

        Expr::Binary(bin) => {
            is_recursive(&bin.left, function_name) || is_recursive(&bin.right, function_name)
        }

        Expr::Unary(un) => is_recursive(&un.operand, function_name),

        Expr::Match(m) => {
            is_recursive(&m.scrutinee, function_name)
                || m.arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .map(|g| is_recursive(g, function_name))
                        .unwrap_or(false)
                        || is_recursive(&arm.body, function_name)
                })
        }

        Expr::Let(l) => {
            is_recursive(&l.value, function_name) || is_recursive(&l.body, function_name)
        }
        Expr::Using(using_expr) => {
            is_recursive(&using_expr.value, function_name)
                || is_recursive(&using_expr.body, function_name)
        }

        Expr::If(i) => {
            is_recursive(&i.condition, function_name)
                || is_recursive(&i.then_branch, function_name)
                || i.else_branch
                    .as_ref()
                    .map(|e| is_recursive(e, function_name))
                    .unwrap_or(false)
        }

        Expr::List(l) => l.elements.iter().any(|e| is_recursive(e, function_name)),

        Expr::Record(r) => r
            .fields
            .iter()
            .any(|f| is_recursive(&f.value, function_name)),

        Expr::MapLiteral(m) => m.entries.iter().any(|entry| {
            is_recursive(&entry.key, function_name) || is_recursive(&entry.value, function_name)
        }),

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
            is_recursive(&f.list, function_name)
                || is_recursive(&f.func, function_name)
                || is_recursive(&f.init, function_name)
        }

        Expr::MemberAccess(_) => false,

        Expr::Concurrent(concurrent) => concurrent.steps.iter().any(|step| match step {
            ConcurrentStep::Spawn(spawn) => is_recursive(&spawn.expr, function_name),
            ConcurrentStep::SpawnEach(spawn_each) => {
                is_recursive(&spawn_each.list, function_name)
                    || is_recursive(&spawn_each.func, function_name)
            }
        }),

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

fn function_match_on_single_list_param<'a>(
    func: &'a FunctionDecl,
) -> Option<(&'a MatchExpr, &'a str)> {
    if func.params.len() != 1 {
        return None;
    }
    let param = &func.params[0];
    if !matches!(param.type_annotation.as_ref(), Some(Type::List(_))) {
        return None;
    }
    let Expr::Match(match_expr) = &func.body else {
        return None;
    };
    let Expr::Identifier(scrutinee) = &match_expr.scrutinee else {
        return None;
    };
    if scrutinee.name != param.name {
        return None;
    }
    Some((match_expr, param.name.as_str()))
}

fn empty_and_cons_arms<'a>(
    match_expr: &'a MatchExpr,
) -> Option<(&'a MatchArm, &'a MatchArm, &'a str, &'a str)> {
    if match_expr.arms.len() != 2 {
        return None;
    }
    let empty_arm = &match_expr.arms[0];
    let cons_arm = &match_expr.arms[1];
    if empty_arm.guard.is_some() || cons_arm.guard.is_some() {
        return None;
    }
    let Pattern::List(empty_pat) = &empty_arm.pattern else {
        return None;
    };
    if !empty_pat.patterns.is_empty() || empty_pat.rest.is_some() {
        return None;
    }
    let Pattern::List(cons_pat) = &cons_arm.pattern else {
        return None;
    };
    if cons_pat.patterns.len() != 1 || cons_pat.rest.is_none() {
        return None;
    }
    let Pattern::Identifier(head_pat) = &cons_pat.patterns[0] else {
        return None;
    };
    Some((
        empty_arm,
        cons_arm,
        head_pat.name.as_str(),
        cons_pat.rest.as_deref()?,
    ))
}

fn is_empty_list_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::List(ListExpr { elements, .. }) if elements.is_empty())
}

fn is_singleton_list_expr<'a>(expr: &'a Expr) -> Option<&'a Expr> {
    match expr {
        Expr::List(ListExpr { elements, .. }) if elements.len() == 1 => elements.first(),
        _ => None,
    }
}

fn is_self_call(expr: &Expr, function_name: &str) -> bool {
    matches!(
        expr,
        Expr::Application(app)
            if matches!(&app.func, Expr::Identifier(identifier) if identifier.name == function_name)
    )
}

fn is_self_call_with_rest(expr: &Expr, function_name: &str, rest_name: &str) -> bool {
    let Expr::Application(app) = expr else {
        return false;
    };
    if !matches!(&app.func, Expr::Identifier(identifier) if identifier.name == function_name) {
        return false;
    }
    if app.args.len() != 1 {
        return false;
    }
    matches!(&app.args[0], Expr::Identifier(identifier) if identifier.name == rest_name)
}

fn is_binary_list_append<'a>(expr: &'a Expr) -> Option<(&'a Expr, &'a Expr)> {
    match expr {
        Expr::Binary(binary) if binary.operator == BinaryOperator::ListAppend => {
            Some((&binary.left, &binary.right))
        }
        _ => None,
    }
}

fn is_binary_operator<'a>(
    expr: &'a Expr,
    operator: BinaryOperator,
) -> Option<(&'a Expr, &'a Expr)> {
    match expr {
        Expr::Binary(binary) if binary.operator == operator => Some((&binary.left, &binary.right)),
        _ => None,
    }
}

fn is_unary_length(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Unary(unary) if unary.operator == UnaryOperator::Length => Some(&unary.operand),
        _ => None,
    }
}

fn is_nullary_constructor_expr(expr: &Expr, name: &str) -> bool {
    matches!(
        expr,
        Expr::Application(app)
            if matches!(&app.func, Expr::Identifier(identifier) if identifier.name == name)
                && app.args.is_empty()
    )
}

fn is_unary_constructor_expr<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    match expr {
        Expr::Application(app)
            if matches!(&app.func, Expr::Identifier(identifier) if identifier.name == name)
                && app.args.len() == 1 =>
        {
            app.args.first()
        }
        _ => None,
    }
}

fn contains_recursive_append_result(expr: &Expr, function_name: &str) -> bool {
    match expr {
        Expr::Binary(binary) => {
            (binary.operator == BinaryOperator::ListAppend
                && is_self_call(&binary.left, function_name))
                || contains_recursive_append_result(&binary.left, function_name)
                || contains_recursive_append_result(&binary.right, function_name)
        }
        Expr::Application(app) => {
            contains_recursive_append_result(&app.func, function_name)
                || app
                    .args
                    .iter()
                    .any(|arg| contains_recursive_append_result(arg, function_name))
        }
        Expr::Lambda(lambda) => contains_recursive_append_result(&lambda.body, function_name),
        Expr::Match(match_expr) => {
            contains_recursive_append_result(&match_expr.scrutinee, function_name)
                || match_expr.arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .map(|guard| contains_recursive_append_result(guard, function_name))
                        .unwrap_or(false)
                        || contains_recursive_append_result(&arm.body, function_name)
                })
        }
        Expr::Let(let_expr) => {
            contains_recursive_append_result(&let_expr.value, function_name)
                || contains_recursive_append_result(&let_expr.body, function_name)
        }
        Expr::Using(using_expr) => {
            contains_recursive_append_result(&using_expr.value, function_name)
                || contains_recursive_append_result(&using_expr.body, function_name)
        }
        Expr::If(if_expr) => {
            contains_recursive_append_result(&if_expr.condition, function_name)
                || contains_recursive_append_result(&if_expr.then_branch, function_name)
                || if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| contains_recursive_append_result(branch, function_name))
                    .unwrap_or(false)
        }
        Expr::List(list) => list
            .elements
            .iter()
            .any(|element| contains_recursive_append_result(element, function_name)),
        Expr::Record(record) => record
            .fields
            .iter()
            .any(|field| contains_recursive_append_result(&field.value, function_name)),
        Expr::MapLiteral(map) => map.entries.iter().any(|entry| {
            contains_recursive_append_result(&entry.key, function_name)
                || contains_recursive_append_result(&entry.value, function_name)
        }),
        Expr::Tuple(tuple) => tuple
            .elements
            .iter()
            .any(|element| contains_recursive_append_result(element, function_name)),
        Expr::FieldAccess(field_access) => {
            contains_recursive_append_result(&field_access.object, function_name)
        }
        Expr::Index(index) => {
            contains_recursive_append_result(&index.object, function_name)
                || contains_recursive_append_result(&index.index, function_name)
        }
        Expr::Pipeline(pipeline) => {
            contains_recursive_append_result(&pipeline.left, function_name)
                || contains_recursive_append_result(&pipeline.right, function_name)
        }
        Expr::Map(map) => {
            contains_recursive_append_result(&map.list, function_name)
                || contains_recursive_append_result(&map.func, function_name)
        }
        Expr::Filter(filter) => {
            contains_recursive_append_result(&filter.list, function_name)
                || contains_recursive_append_result(&filter.predicate, function_name)
        }
        Expr::Fold(fold) => {
            contains_recursive_append_result(&fold.list, function_name)
                || contains_recursive_append_result(&fold.func, function_name)
                || contains_recursive_append_result(&fold.init, function_name)
        }
        Expr::Concurrent(concurrent) => concurrent.steps.iter().any(|step| match step {
            ConcurrentStep::Spawn(spawn) => {
                contains_recursive_append_result(&spawn.expr, function_name)
            }
            ConcurrentStep::SpawnEach(spawn_each) => {
                contains_recursive_append_result(&spawn_each.list, function_name)
                    || contains_recursive_append_result(&spawn_each.func, function_name)
            }
        }),
        Expr::TypeAscription(type_ascription) => {
            contains_recursive_append_result(&type_ascription.expr, function_name)
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Unary(_) | Expr::MemberAccess(_) => false,
    }
}

fn detect_exact_recursive_reverse_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    if !is_empty_list_expr(&empty_arm.body) {
        return false;
    }
    let Some((left, right)) = is_binary_list_append(&cons_arm.body) else {
        return false;
    };
    is_self_call_with_rest(left, &func.name, rest_name)
        && matches!(is_singleton_list_expr(right), Some(Expr::Identifier(identifier)) if identifier.name == head_name)
}

fn is_binary_predicate_with_self(
    expr: &Expr,
    operator: BinaryOperator,
    function_name: &str,
    head_name: &str,
    rest_name: &str,
) -> bool {
    let Some((left, right)) = is_binary_operator(expr, operator) else {
        return false;
    };
    (is_predicate_application(left, head_name)
        && is_self_call_with_rest(right, function_name, rest_name))
        || (is_self_call_with_rest(left, function_name, rest_name)
            && is_predicate_application(right, head_name))
}

fn detect_exact_recursive_all_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    matches!(&empty_arm.body, Expr::Literal(lit) if lit.literal_type == LiteralType::Bool && lit.value == LiteralValue::Bool(true))
        && is_binary_predicate_with_self(
            &cons_arm.body,
            BinaryOperator::And,
            &func.name,
            head_name,
            rest_name,
        )
}

fn detect_exact_recursive_any_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    matches!(&empty_arm.body, Expr::Literal(lit) if lit.literal_type == LiteralType::Bool && lit.value == LiteralValue::Bool(false))
        && is_binary_predicate_with_self(
            &cons_arm.body,
            BinaryOperator::Or,
            &func.name,
            head_name,
            rest_name,
        )
}

fn detect_exact_recursive_map_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    if !is_empty_list_expr(&empty_arm.body) {
        return false;
    }
    let Some((left, right)) = is_binary_list_append(&cons_arm.body) else {
        return false;
    };
    is_self_call_with_rest(right, &func.name, rest_name)
        && matches!(is_singleton_list_expr(left), Some(element) if !matches!(element, Expr::Identifier(identifier) if identifier.name == head_name))
}

fn is_predicate_application(expr: &Expr, head_name: &str) -> bool {
    matches!(
        expr,
        Expr::Application(app)
            if app.args.len() == 1
                && matches!(&app.args[0], Expr::Identifier(identifier) if identifier.name == head_name)
    )
}

fn is_keep_branch(expr: &Expr, function_name: &str, head_name: &str, rest_name: &str) -> bool {
    let Some((left, right)) = is_binary_list_append(expr) else {
        return false;
    };
    matches!(is_singleton_list_expr(left), Some(Expr::Identifier(identifier)) if identifier.name == head_name)
        && is_self_call_with_rest(right, function_name, rest_name)
}

fn is_drop_branch(expr: &Expr, function_name: &str, rest_name: &str) -> bool {
    is_self_call_with_rest(expr, function_name, rest_name)
}

fn detect_exact_recursive_filter_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    if !is_empty_list_expr(&empty_arm.body) {
        return false;
    }
    match &cons_arm.body {
        Expr::If(if_expr) => {
            is_predicate_application(&if_expr.condition, head_name)
                && is_keep_branch(&if_expr.then_branch, &func.name, head_name, rest_name)
                && if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| is_drop_branch(branch, &func.name, rest_name))
                    .unwrap_or(false)
        }
        Expr::Match(nested_match) => {
            if !is_predicate_application(&nested_match.scrutinee, head_name)
                || nested_match.arms.len() != 2
            {
                return false;
            }
            let keep_arm = &nested_match.arms[0];
            let drop_arm = &nested_match.arms[1];
            keep_arm.guard.is_none()
                && drop_arm.guard.is_none()
                && matches!(&keep_arm.pattern, Pattern::Literal(lit) if lit.literal_type == PatternLiteralType::Bool && lit.value == PatternLiteralValue::Bool(true))
                && matches!(&drop_arm.pattern, Pattern::Literal(lit) if lit.literal_type == PatternLiteralType::Bool && lit.value == PatternLiteralValue::Bool(false))
                && is_keep_branch(&keep_arm.body, &func.name, head_name, rest_name)
                && is_drop_branch(&drop_arm.body, &func.name, rest_name)
        }
        _ => false,
    }
}

fn detect_exact_recursive_find_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    if !is_nullary_constructor_expr(&empty_arm.body, "None") {
        return false;
    }
    match &cons_arm.body {
        Expr::If(if_expr) => {
            is_predicate_application(&if_expr.condition, head_name)
                && matches!(
                    is_unary_constructor_expr(&if_expr.then_branch, "Some"),
                    Some(Expr::Identifier(identifier)) if identifier.name == head_name
                )
                && if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| is_self_call_with_rest(branch, &func.name, rest_name))
                    .unwrap_or(false)
        }
        Expr::Match(nested_match) => {
            if !is_predicate_application(&nested_match.scrutinee, head_name)
                || nested_match.arms.len() != 2
            {
                return false;
            }
            let some_arm = &nested_match.arms[0];
            let none_arm = &nested_match.arms[1];
            some_arm.guard.is_none()
                && none_arm.guard.is_none()
                && matches!(&some_arm.pattern, Pattern::Literal(lit) if lit.literal_type == PatternLiteralType::Bool && lit.value == PatternLiteralValue::Bool(true))
                && matches!(&none_arm.pattern, Pattern::Literal(lit) if lit.literal_type == PatternLiteralType::Bool && lit.value == PatternLiteralValue::Bool(false))
                && matches!(
                    is_unary_constructor_expr(&some_arm.body, "Some"),
                    Some(Expr::Identifier(identifier)) if identifier.name == head_name
                )
                && is_self_call_with_rest(&none_arm.body, &func.name, rest_name)
        }
        _ => false,
    }
}

fn detect_exact_recursive_flat_map_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    if !is_empty_list_expr(&empty_arm.body) {
        return false;
    }
    let Some((left, right)) = is_binary_list_append(&cons_arm.body) else {
        return false;
    };
    is_self_call_with_rest(right, &func.name, rest_name)
        && !is_self_call(left, &func.name)
        && expr_contains_identifier(left, head_name)
}

fn count_self_calls(expr: &Expr, function_name: &str) -> usize {
    match expr {
        Expr::Application(app) => {
            usize::from(
                matches!(&app.func, Expr::Identifier(identifier) if identifier.name == function_name),
            ) + count_self_calls(&app.func, function_name)
                + app
                    .args
                    .iter()
                    .map(|arg| count_self_calls(arg, function_name))
                    .sum::<usize>()
        }
        Expr::Binary(binary) => {
            count_self_calls(&binary.left, function_name)
                + count_self_calls(&binary.right, function_name)
        }
        Expr::Unary(unary) => count_self_calls(&unary.operand, function_name),
        Expr::Match(match_expr) => {
            count_self_calls(&match_expr.scrutinee, function_name)
                + match_expr
                    .arms
                    .iter()
                    .map(|arm| {
                        arm.guard
                            .as_ref()
                            .map(|guard| count_self_calls(guard, function_name))
                            .unwrap_or(0)
                            + count_self_calls(&arm.body, function_name)
                    })
                    .sum::<usize>()
        }
        Expr::Let(let_expr) => {
            count_self_calls(&let_expr.value, function_name)
                + count_self_calls(&let_expr.body, function_name)
        }
        Expr::Using(using_expr) => {
            count_self_calls(&using_expr.value, function_name)
                + count_self_calls(&using_expr.body, function_name)
        }
        Expr::If(if_expr) => {
            count_self_calls(&if_expr.condition, function_name)
                + count_self_calls(&if_expr.then_branch, function_name)
                + if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| count_self_calls(branch, function_name))
                    .unwrap_or(0)
        }
        Expr::List(list) => list
            .elements
            .iter()
            .map(|element| count_self_calls(element, function_name))
            .sum(),
        Expr::Record(record) => record
            .fields
            .iter()
            .map(|field| count_self_calls(&field.value, function_name))
            .sum(),
        Expr::MapLiteral(map) => map
            .entries
            .iter()
            .map(|entry| {
                count_self_calls(&entry.key, function_name)
                    + count_self_calls(&entry.value, function_name)
            })
            .sum(),
        Expr::Tuple(tuple) => tuple
            .elements
            .iter()
            .map(|element| count_self_calls(element, function_name))
            .sum(),
        Expr::FieldAccess(field_access) => count_self_calls(&field_access.object, function_name),
        Expr::Index(index) => {
            count_self_calls(&index.object, function_name)
                + count_self_calls(&index.index, function_name)
        }
        Expr::Pipeline(pipeline) => {
            count_self_calls(&pipeline.left, function_name)
                + count_self_calls(&pipeline.right, function_name)
        }
        Expr::Map(map) => {
            count_self_calls(&map.list, function_name) + count_self_calls(&map.func, function_name)
        }
        Expr::Filter(filter) => {
            count_self_calls(&filter.list, function_name)
                + count_self_calls(&filter.predicate, function_name)
        }
        Expr::Fold(fold) => {
            count_self_calls(&fold.list, function_name)
                + count_self_calls(&fold.func, function_name)
                + count_self_calls(&fold.init, function_name)
        }
        Expr::Concurrent(concurrent) => concurrent
            .steps
            .iter()
            .map(|step| match step {
                ConcurrentStep::Spawn(spawn) => count_self_calls(&spawn.expr, function_name),
                ConcurrentStep::SpawnEach(spawn_each) => {
                    count_self_calls(&spawn_each.list, function_name)
                        + count_self_calls(&spawn_each.func, function_name)
                }
            })
            .sum(),
        Expr::TypeAscription(type_ascription) => {
            count_self_calls(&type_ascription.expr, function_name)
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) | Expr::Lambda(_) => 0,
    }
}

fn expr_contains_identifier(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Identifier(identifier) => identifier.name == name,
        Expr::Application(app) => {
            expr_contains_identifier(&app.func, name)
                || app
                    .args
                    .iter()
                    .any(|arg| expr_contains_identifier(arg, name))
        }
        Expr::Binary(binary) => {
            expr_contains_identifier(&binary.left, name)
                || expr_contains_identifier(&binary.right, name)
        }
        Expr::Unary(unary) => expr_contains_identifier(&unary.operand, name),
        Expr::Match(match_expr) => {
            expr_contains_identifier(&match_expr.scrutinee, name)
                || match_expr.arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .map(|guard| expr_contains_identifier(guard, name))
                        .unwrap_or(false)
                        || expr_contains_identifier(&arm.body, name)
                })
        }
        Expr::Let(let_expr) => {
            expr_contains_identifier(&let_expr.value, name)
                || expr_contains_identifier(&let_expr.body, name)
        }
        Expr::Using(using_expr) => {
            expr_contains_identifier(&using_expr.value, name)
                || expr_contains_identifier(&using_expr.body, name)
        }
        Expr::If(if_expr) => {
            expr_contains_identifier(&if_expr.condition, name)
                || expr_contains_identifier(&if_expr.then_branch, name)
                || if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| expr_contains_identifier(branch, name))
                    .unwrap_or(false)
        }
        Expr::List(list) => list
            .elements
            .iter()
            .any(|element| expr_contains_identifier(element, name)),
        Expr::Record(record) => record
            .fields
            .iter()
            .any(|field| expr_contains_identifier(&field.value, name)),
        Expr::MapLiteral(map) => map.entries.iter().any(|entry| {
            expr_contains_identifier(&entry.key, name)
                || expr_contains_identifier(&entry.value, name)
        }),
        Expr::Tuple(tuple) => tuple
            .elements
            .iter()
            .any(|element| expr_contains_identifier(element, name)),
        Expr::FieldAccess(field_access) => expr_contains_identifier(&field_access.object, name),
        Expr::Index(index) => {
            expr_contains_identifier(&index.object, name)
                || expr_contains_identifier(&index.index, name)
        }
        Expr::Pipeline(pipeline) => {
            expr_contains_identifier(&pipeline.left, name)
                || expr_contains_identifier(&pipeline.right, name)
        }
        Expr::Map(map) => {
            expr_contains_identifier(&map.list, name) || expr_contains_identifier(&map.func, name)
        }
        Expr::Filter(filter) => {
            expr_contains_identifier(&filter.list, name)
                || expr_contains_identifier(&filter.predicate, name)
        }
        Expr::Fold(fold) => {
            expr_contains_identifier(&fold.list, name)
                || expr_contains_identifier(&fold.func, name)
                || expr_contains_identifier(&fold.init, name)
        }
        Expr::Concurrent(concurrent) => concurrent.steps.iter().any(|step| match step {
            ConcurrentStep::Spawn(spawn) => expr_contains_identifier(&spawn.expr, name),
            ConcurrentStep::SpawnEach(spawn_each) => {
                expr_contains_identifier(&spawn_each.list, name)
                    || expr_contains_identifier(&spawn_each.func, name)
            }
        }),
        Expr::TypeAscription(type_ascription) => {
            expr_contains_identifier(&type_ascription.expr, name)
        }
        Expr::Literal(_) | Expr::Lambda(_) | Expr::MemberAccess(_) => false,
    }
}

fn is_obvious_fold_step(
    expr: &Expr,
    function_name: &str,
    head_name: &str,
    rest_name: &str,
) -> bool {
    match expr {
        Expr::Binary(binary) => {
            (expr_contains_identifier(&binary.left, head_name)
                && is_self_call_with_rest(&binary.right, function_name, rest_name))
                || (is_self_call_with_rest(&binary.left, function_name, rest_name)
                    && expr_contains_identifier(&binary.right, head_name))
        }
        Expr::Application(app) => {
            count_self_calls(expr, function_name) == 1
                && app
                    .args
                    .iter()
                    .any(|arg| is_self_call_with_rest(arg, function_name, rest_name))
                && app
                    .args
                    .iter()
                    .any(|arg| expr_contains_identifier(arg, head_name))
        }
        _ => false,
    }
}

fn detect_exact_recursive_fold_clone(func: &FunctionDecl) -> bool {
    let Some((match_expr, _param_name)) = function_match_on_single_list_param(func) else {
        return false;
    };
    let Some((empty_arm, cons_arm, head_name, rest_name)) = empty_and_cons_arms(match_expr) else {
        return false;
    };
    if matches!(&func.return_type, Some(Type::List(_))) || is_empty_list_expr(&empty_arm.body) {
        return false;
    }
    is_obvious_fold_step(&cons_arm.body, &func.name, head_name, rest_name)
}

fn collect_filter_then_count_errors(program: &Program, errors: &mut Vec<ValidationError>) {
    for declaration in &program.declarations {
        match declaration {
            Declaration::Function(function) => {
                collect_filter_then_count_in_expr(&function.body, errors)
            }
            Declaration::Const(const_decl) => {
                collect_filter_then_count_in_expr(&const_decl.value, errors)
            }
            Declaration::FeatureFlag(feature_flag_decl) => {
                collect_filter_then_count_in_expr(&feature_flag_decl.default, errors)
            }
            Declaration::Test(test_decl) => {
                collect_filter_then_count_in_expr(&test_decl.body, errors)
            }
            _ => {}
        }
    }
}

fn collect_filter_then_count_in_expr(expr: &Expr, errors: &mut Vec<ValidationError>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => {}
        Expr::Lambda(lambda) => collect_filter_then_count_in_expr(&lambda.body, errors),
        Expr::Application(app) => {
            collect_filter_then_count_in_expr(&app.func, errors);
            for arg in &app.args {
                collect_filter_then_count_in_expr(arg, errors);
            }
        }
        Expr::Binary(binary) => {
            collect_filter_then_count_in_expr(&binary.left, errors);
            collect_filter_then_count_in_expr(&binary.right, errors);
        }
        Expr::Unary(unary) => {
            if unary.operator == UnaryOperator::Length && matches!(&unary.operand, Expr::Filter(_))
            {
                errors.push(ValidationError::FilterThenCount {
                    location: unary.location,
                });
            }
            collect_filter_then_count_in_expr(&unary.operand, errors);
        }
        Expr::Match(match_expr) => {
            collect_filter_then_count_in_expr(&match_expr.scrutinee, errors);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_filter_then_count_in_expr(guard, errors);
                }
                collect_filter_then_count_in_expr(&arm.body, errors);
            }
        }
        Expr::Let(let_expr) => {
            collect_filter_then_count_in_expr(&let_expr.value, errors);
            collect_filter_then_count_in_expr(&let_expr.body, errors);
        }
        Expr::Using(using_expr) => {
            collect_filter_then_count_in_expr(&using_expr.value, errors);
            collect_filter_then_count_in_expr(&using_expr.body, errors);
        }
        Expr::If(if_expr) => {
            collect_filter_then_count_in_expr(&if_expr.condition, errors);
            collect_filter_then_count_in_expr(&if_expr.then_branch, errors);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_filter_then_count_in_expr(else_branch, errors);
            }
        }
        Expr::List(list) => {
            for element in &list.elements {
                collect_filter_then_count_in_expr(element, errors);
            }
        }
        Expr::Record(record) => {
            for field in &record.fields {
                collect_filter_then_count_in_expr(&field.value, errors);
            }
        }
        Expr::MapLiteral(map) => {
            for entry in &map.entries {
                collect_filter_then_count_in_expr(&entry.key, errors);
                collect_filter_then_count_in_expr(&entry.value, errors);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_filter_then_count_in_expr(element, errors);
            }
        }
        Expr::FieldAccess(field_access) => {
            collect_filter_then_count_in_expr(&field_access.object, errors)
        }
        Expr::Index(index) => {
            collect_filter_then_count_in_expr(&index.object, errors);
            collect_filter_then_count_in_expr(&index.index, errors);
        }
        Expr::Pipeline(pipeline) => {
            collect_filter_then_count_in_expr(&pipeline.left, errors);
            collect_filter_then_count_in_expr(&pipeline.right, errors);
        }
        Expr::Map(map) => {
            collect_filter_then_count_in_expr(&map.list, errors);
            collect_filter_then_count_in_expr(&map.func, errors);
        }
        Expr::Filter(filter) => {
            collect_filter_then_count_in_expr(&filter.list, errors);
            collect_filter_then_count_in_expr(&filter.predicate, errors);
        }
        Expr::Fold(fold) => {
            collect_filter_then_count_in_expr(&fold.list, errors);
            collect_filter_then_count_in_expr(&fold.func, errors);
            collect_filter_then_count_in_expr(&fold.init, errors);
        }
        Expr::Concurrent(concurrent) => {
            collect_filter_then_count_in_expr(&concurrent.width, errors);
            if let Some(policy) = &concurrent.policy {
                for field in &policy.fields {
                    collect_filter_then_count_in_expr(&field.value, errors);
                }
            }
            for step in &concurrent.steps {
                match step {
                    ConcurrentStep::Spawn(spawn) => {
                        collect_filter_then_count_in_expr(&spawn.expr, errors)
                    }
                    ConcurrentStep::SpawnEach(spawn_each) => {
                        collect_filter_then_count_in_expr(&spawn_each.list, errors);
                        collect_filter_then_count_in_expr(&spawn_each.func, errors);
                    }
                }
            }
        }
        Expr::TypeAscription(type_ascription) => {
            collect_filter_then_count_in_expr(&type_ascription.expr, errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;
    use sigil_typechecker::{type_check, TypeCheckOptions, TypeInfo};

    fn type_registry_for(program: &Program) -> HashMap<String, TypeInfo> {
        program
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                Declaration::Type(type_decl) => Some((
                    type_decl.name.clone(),
                    TypeInfo {
                        type_params: type_decl.type_params.clone(),
                        definition: type_decl.definition.clone(),
                        constraint: type_decl.constraint.clone(),
                        labels: std::collections::BTreeSet::new(),
                    },
                )),
                _ => None,
            })
            .collect()
    }

    fn validator_test_imported_type_registries() -> HashMap<String, HashMap<String, TypeInfo>> {
        let json_source = "t JsonValue=JsonArray([JsonValue])|JsonBool(Bool)|JsonNull()|JsonNumber(Float)|JsonObject({String↦JsonValue})|JsonString(String)\n";
        let json_program = parse(tokenize(json_source).unwrap(), "json.lib.sigil").unwrap();
        let decode_source = "t DecodeError={message:String,path:[String]}\n";
        let decode_program = parse(tokenize(decode_source).unwrap(), "decode.lib.sigil").unwrap();
        HashMap::from([
            ("stdlib::json".to_string(), type_registry_for(&json_program)),
            ("stdlib::decode".to_string(), type_registry_for(&decode_program)),
        ])
    }

    fn validator_test_imported_namespaces() -> HashMap<String, InferenceType> {
        let empty_namespace = |name: &str| {
            InferenceType::Record(sigil_typechecker::types::TRecord {
                fields: HashMap::new(),
                name: Some(name.to_string()),
            })
        };
        HashMap::from([
            ("stdlib::json".to_string(), empty_namespace("stdlib::json")),
            ("stdlib::decode".to_string(), empty_namespace("stdlib::decode")),
        ])
    }

    fn type_check_for_typed_validation(
        program: &Program,
        source: &str,
    ) -> sigil_typechecker::TypeCheckResult {
        type_check(
            program,
            source,
            Some(TypeCheckOptions {
                imported_namespaces: Some(validator_test_imported_namespaces()),
                imported_type_registries: Some(validator_test_imported_type_registries()),
                ..TypeCheckOptions::default()
            }),
        )
        .unwrap()
    }

    fn typed_validation_options_for(program: &Program, file_path: &str) -> TypedValidationOptions {
        TypedValidationOptions {
            local_type_registry: type_registry_for(program),
            imported_type_registries: validator_test_imported_type_registries(),
            module_id: None,
            source_file: Some(file_path.to_string()),
        }
    }

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
        assert!(errors
            .iter()
            .any(|error| matches!(error, ValidationError::DuplicateDeclaration { .. })));
    }

    #[test]
    fn test_simple_recursion_allowed() {
        // This stays minimal because the test is about recursion validation, not match coverage.
        let source = "total λfactorial(n:Int)=>Int\nrequires n≥0\ndecreases n\n=factorial(n-1)\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();

        // Should pass - simple recursion is allowed
        assert!(validate_canonical_form(&program, Some("test.lib.sigil"), Some(source)).is_ok());
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

        let result = validate_typed_canonical_form(&typed.typed_program, Some("test.sigil"));
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

        assert!(validate_typed_canonical_form(&typed.typed_program, Some("test.sigil")).is_ok());
    }

    #[test]
    fn test_single_use_effectful_binding_allowed() {
        let source = r#"e console:{log:λ(String)=>!Log Unit}
λemit()=>!Log String={
  l _=(console.log("x"):Unit);
  "x"
}
λmain()=>!Log String={
  l value=(emit():String);
  value
}"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        assert!(validate_typed_canonical_form(&typed.typed_program, Some("test.sigil")).is_ok());
    }

    #[test]
    fn test_dead_pure_discard_rejected() {
        let source = r#"λmain()=>Unit={
  l _=(releaseCount():Int);
  ()
}

λreleaseCount()=>Int=2
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        let result = validate_typed_canonical_form(&typed.typed_program, Some("test.sigil"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::DeadPureDiscard { .. })));
    }

    #[test]
    fn test_dead_pure_unit_discard_rejected() {
        let source = r#"λmain()=>Unit={
  l _=(touch():Unit);
  ()
}

λtouch()=>Unit=()
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        let result = validate_typed_canonical_form(&typed.typed_program, Some("test.sigil"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::DeadPureDiscard { .. })));
    }

    #[test]
    fn test_unused_named_binding_rejected() {
        let source = r#"λmain()=>Unit={
  l unused=("x":String);
  ()
}
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let typed = type_check(&program, source, None).unwrap();

        let result = validate_typed_canonical_form(&typed.typed_program, Some("test.sigil"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::UnusedBinding { binding_name, .. } if binding_name == "unused")));
    }

    #[test]
    fn test_manual_direct_json_encoder_rejected_for_derivable_type() {
        let source = r#"t User={name:String}

derive json User

λuserToJson(user:User)=>§json.JsonValue=encodeUser(user)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        let result = validate_typed_canonical_form_with_options(
            &typed.typed_program,
            Some("test.lib.sigil"),
            typed_validation_options_for(&program, "test.lib.sigil"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::DirectJsonCodec {
                declaration_name,
                target_name,
                surface_kind,
                encode_helper,
                ..
            } if declaration_name == "userToJson"
                && target_name == "User"
                && surface_kind == "encode"
                && encode_helper == "encodeUser"
        )));
    }

    #[test]
    fn test_manual_direct_json_decoder_rejected_for_derivable_type() {
        let source = r#"t Result[T,E]=Err(E)|Ok(T)

t User={name:String}

λfromJsonUser(value:§json.JsonValue)=>Result[
  User,
  §decode.DecodeError
]=Ok({name:"sigil"})
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        let result = validate_typed_canonical_form_with_options(
            &typed.typed_program,
            Some("test.lib.sigil"),
            typed_validation_options_for(&program, "test.lib.sigil"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::DirectJsonCodec {
                declaration_name,
                target_name,
                surface_kind,
                decode_helper,
                ..
            } if declaration_name == "fromJsonUser"
                && target_name == "User"
                && surface_kind == "decode"
                && decode_helper == "decodeUser"
        )));
    }

    #[test]
    fn test_manual_direct_json_parser_rejected_for_derivable_type() {
        let source = r#"t Result[T,E]=Err(E)|Ok(T)

t User={name:String}

λfromJsonText(input:String)=>Result[
  User,
  §decode.DecodeError
]=Err({
  message:"bad json",
  path:[]
})
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        let result = validate_typed_canonical_form_with_options(
            &typed.typed_program,
            Some("test.lib.sigil"),
            typed_validation_options_for(&program, "test.lib.sigil"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::DirectJsonCodec {
                declaration_name,
                target_name,
                surface_kind,
                parse_helper,
                ..
            } if declaration_name == "fromJsonText"
                && target_name == "User"
                && surface_kind == "parse"
                && parse_helper == "parseUser"
        )));
    }

    #[test]
    fn test_manual_direct_json_stringifier_rejected_for_derivable_type() {
        let source = r#"t User={name:String}

derive json User

λuserToJsonText(user:User)=>String=stringifyUser(user)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        let result = validate_typed_canonical_form_with_options(
            &typed.typed_program,
            Some("test.lib.sigil"),
            typed_validation_options_for(&program, "test.lib.sigil"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::DirectJsonCodec {
                declaration_name,
                target_name,
                surface_kind,
                stringify_helper,
                ..
            } if declaration_name == "userToJsonText"
                && target_name == "User"
                && surface_kind == "stringify"
                && stringify_helper == "stringifyUser"
        )));
    }

    #[test]
    fn test_function_valued_const_direct_json_encoder_rejected() {
        let source = r#"t User={name:String}

derive json User

c userToJson=(encodeUser:λ(User)=>§json.JsonValue)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        let result = validate_typed_canonical_form_with_options(
            &typed.typed_program,
            Some("test.lib.sigil"),
            typed_validation_options_for(&program, "test.lib.sigil"),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::DirectJsonCodec {
                declaration_name,
                target_name,
                surface_kind,
                ..
            } if declaration_name == "userToJson"
                && target_name == "User"
                && surface_kind == "encode"
        )));
    }

    #[test]
    fn test_non_json_string_function_allowed_for_derivable_type() {
        let source = r#"t User={name:String}

λdisplayName(user:User)=>String=user.name
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        assert!(
            validate_typed_canonical_form_with_options(
                &typed.typed_program,
                Some("test.lib.sigil"),
                typed_validation_options_for(&program, "test.lib.sigil"),
            )
            .is_ok()
        );
    }

    #[test]
    fn test_manual_decoder_allowed_for_non_derivable_payload_type() {
        let source = r#"t Result[T,E]=Err(E)|Ok(T)

t LegacyUserPayload=LegacyUserPayload(Char)

λdecodeLegacyUser(value:§json.JsonValue)=>Result[
  LegacyUserPayload,
  §decode.DecodeError
]=Ok(LegacyUserPayload('x'))
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let typed = type_check_for_typed_validation(&program, source);

        assert!(
            validate_typed_canonical_form_with_options(
                &typed.typed_program,
                Some("test.lib.sigil"),
                typed_validation_options_for(&program, "test.lib.sigil"),
            )
            .is_ok()
        );
    }

    #[test]
    fn test_unused_executable_function_rejected() {
        let source = r#"λhelper()=>Int=1

λmain()=>Unit=()
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::UnusedDeclaration { decl_kind, decl_name, .. } if decl_kind == "function" && decl_name == "helper")));
    }

    #[test]
    fn test_unused_library_export_allowed() {
        let source = r#"λhelper()=>Int=1
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "helper.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("helper.lib.sigil"), Some(source)).is_ok());
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
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::SourceForm { .. })));
    }

    #[test]
    fn test_source_form_ignores_comment_only_lines_between_declarations() {
        let source = r#"λalpha()=>Int=1

⟦ keep this explanation in checked docs ⟧
λbeta()=>Int=2
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "helpers.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("helpers.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_source_form_ignores_inline_comments() {
        let source = r#"λalpha()=>Int=1 ⟦ comment after code ⟧

λbeta()=>Int=2 ⟦ another note ⟧
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "helpers.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("helpers.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_source_form_keeps_comment_delimiters_inside_strings() {
        let source = "λclose()=>String=\"⟧\"\n\nλopen()=>String=\"⟦\"\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "helpers.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("helpers.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_source_form_keeps_multiline_strings_with_comment_delimiters() {
        let source = "λtemplate()=>String=\"first ⟦ not a comment\nsecond ⟧ still string\"\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "helpers.lib.sigil").unwrap();

        assert!(validate_canonical_form(&program, Some("helpers.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_normalized_source_keeps_real_extra_blank_lines_without_comments() {
        let source = r#"λalpha()=>Int=1


λbeta()=>Int=2
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "helpers.lib.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("helpers.lib.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::SourceForm { .. })));
    }

    #[test]
    fn test_direct_match_body_canonical_layout_allowed() {
        let source = r#"total λcountdown(n:Int)=>Int
requires n≥0
decreases n
match n{
  0=>0|
  value=>countdown(value+-1)
}

λmain()=>Int=countdown(5)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_ok(), "{:?}", result.unwrap_err());
    }

    #[test]
    fn test_concurrent_config_fields_must_be_alphabetical() {
        let source = r#"e clock:{tick:λ()=>!Timer Unit}
λmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@1:{windowMs:None(),jitterMs:None(),stopOn:stopOn}{
  spawn one()
}

λone()=>!Timer Result[Int,String]={
  l _=(clock.tick():Unit);
  Ok(1)
}

λstopOn(err:String)=>Bool=false
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecordLiteralFieldOrder { .. })));
    }

    #[test]
    fn test_concurrent_default_policy_fields_must_be_omitted() {
        let source = r#"e clock:{tick:λ()=>!Timer Unit}
λmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@1:{jitterMs:None(),windowMs:None()}{
  spawn one()
}

λone()=>!Timer Result[Int,String]={
  l _=(clock.tick():Unit);
  Ok(1)
}
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::SourceForm { .. })));
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

λmain()=>Int=f(
  0,
  1
)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_ok(), "{:?}", result.unwrap_err());
    }

    #[test]
    fn test_non_canonical_match_after_signature_rejected_as_source_form() {
        let source = "λfib(n:Int)=>Int\nmatch n{0=>0}\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::SourceForm { .. })));
    }

    #[test]
    fn test_recursive_append_result_rejected() {
        let source = r#"λmain()=>[Int]=suffix([1,2,3])

λsuffix(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>suffix(rest)⧺[x+1,x]
}
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveAppendResult { .. })));
    }

    #[test]
    fn test_non_recursive_append_allowed() {
        let source = r#"λaddFooter(lines:[String])=>[String]=lines⧺["-- end --"]

λmain()=>[String]=addFooter(["a"])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        assert!(validate_canonical_form(&program, Some("test.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_recursive_map_clone_rejected() {
        let source = r#"λdouble(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>[x*2]⧺double(rest)
}

λmain()=>[Int]=double([1,2,3])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveMapClone { .. })));
    }

    #[test]
    fn test_recursive_all_clone_rejected() {
        let source = r#"λallPositive(xs:[Int])=>Bool match xs{
  []=>true|
  [x,.rest]=>isPositive(x) and allPositive(rest)
}

λisPositive(x:Int)=>Bool=x>0

λmain()=>Bool=allPositive([1,2,3])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveAllClone { .. })));
    }

    #[test]
    fn test_recursive_any_clone_rejected() {
        let source = r#"λanyEven(xs:[Int])=>Bool match xs{
  []=>false|
  [x,.rest]=>isEven(x) or anyEven(rest)
}

λisEven(x:Int)=>Bool=x%2=0

λmain()=>Bool=anyEven([1,2,3])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveAnyClone { .. })));
    }

    #[test]
    fn test_recursive_filter_clone_rejected() {
        let source = r#"λevens(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>match isEven(x){
    true=>[x]⧺evens(rest)|
    false=>evens(rest)
  }
}

λisEven(x:Int)=>Bool=x%2=0

λmain()=>[Int]=evens([1,2,3,4])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveFilterClone { .. })));
    }

    #[test]
    fn test_recursive_reverse_clone_rejected() {
        let source = r#"λmain()=>[Int]=reverse([1,2,3])

λreverse(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>reverse(rest)⧺[x]
}
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveReverseClone { .. })));
    }

    #[test]
    fn test_recursive_fold_clone_rejected() {
        let source = r#"λmain()=>Int=sum([1,2,3])

λsum(xs:[Int])=>Int match xs{
  []=>0|
  [x,.rest]=>x+sum(rest)
}
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveFoldClone { .. })));
    }

    #[test]
    fn test_recursive_find_clone_rejected() {
        let source = r#"λfindEven(xs:[Int])=>Option[Int] match xs{
  []=>None()|
  [x,.rest]=>match isEven(x){
    true=>Some(x)|
    false=>findEven(rest)
  }
}

λisEven(x:Int)=>Bool=x%2=0

λmain()=>Option[Int]=findEven([1,2,3,4])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveFindClone { .. })));
    }

    #[test]
    fn test_recursive_flat_map_clone_rejected() {
        let source = r#"λexplode(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>digits(x)⧺explode(rest)
}

λdigits(x:Int)=>[Int]=[x,x]

λmain()=>[Int]=explode([1,2,3])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::RecursiveFlatMapClone { .. })));
    }

    #[test]
    fn test_filter_then_count_rejected() {
        let source = r#"λcountEven(xs:[Int])=>Int=#(xs filter (λ(x:Int)=>Bool=x%2=0))

λmain()=>Int=countEven([1,2,3,4])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|error| matches!(error, ValidationError::FilterThenCount { .. })));
    }

    #[test]
    fn test_canonical_map_operator_allowed() {
        let source = r#"λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)

λmain()=>[Int]=double([
  1,
  2,
  3
])
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        assert!(validate_canonical_form(&program, Some("test.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_canonical_any_all_find_allowed() {
        let source = r#"λallPositive(xs:[Int])=>Bool=§list.all(
  λ(x:Int)=>Bool=x>0,
  xs
)

λanyEven(xs:[Int])=>Bool=§list.any(
  λ(x:Int)=>Bool=x%2=0,
  xs
)

λfindEven(xs:[Int])=>Option[Int]=§list.find(
  λ(x:Int)=>Bool=x%2=0,
  xs
)

λmain()=>Bool=allPositive([
  1,
  2,
  3
])
  and anyEven([
    1,
    2,
    3
  ])
  and findEven([
    1,
    2,
    3,
    4
  ])=Some(2)
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        assert!(validate_canonical_form(&program, Some("test.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_canonical_flat_map_and_count_if_allowed() {
        let source = r#"λcountEven(xs:[Int])=>Int=§list.countIf(
  λ(x:Int)=>Bool=x%2=0,
  xs
)

λexplode(xs:[Int])=>[Int]=§list.flatMap(
  λ(x:Int)=>[Int]=[
    x,
    x
  ],
  xs
)

λmain()=>Bool=countEven([
  1,
  2,
  3,
  4
])=2 and explode([
  1,
  2
])=[
  1,
  1,
  2,
  2
]
"#;
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        assert!(validate_canonical_form(&program, Some("test.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_direct_stdlib_wrapper_rejected() {
        let source = "λsum1(xs:[Int])=>Int=§list.sum(xs)\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.lib.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::HelperDirectWrapper {
                canonical_helper,
                canonical_surface,
                function_name,
                ..
            } if canonical_helper == "§list.sum"
                && canonical_surface == "§list.sum(xs)"
                && function_name == "sum1"
        )));
    }

    #[test]
    fn test_direct_map_wrapper_rejected() {
        let source = "λproject[T,U](fn:λ(T)=>U,xs:[T])=>[U]=xs map fn\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        let result = validate_canonical_form(&program, Some("test.lib.sigil"), Some(source));
        assert!(result.is_err());
        assert!(result.unwrap_err().iter().any(|error| matches!(
            error,
            ValidationError::HelperDirectWrapper {
                canonical_helper,
                canonical_surface,
                function_name,
                ..
            } if canonical_helper == "map"
                && canonical_surface == "xs map fn"
                && function_name == "project"
        )));
    }

    #[test]
    fn test_wrapper_with_extra_logic_allowed() {
        let source = "λsum1(xs:[Int])=>Int=§list.sum(§list.reverse(xs))\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.lib.sigil").unwrap();
        assert!(validate_canonical_form(&program, Some("test.lib.sigil"), Some(source)).is_ok());
    }

    #[test]
    fn test_stdlib_file_direct_wrapper_allowed() {
        let source = "λsum(xs:[Int])=>Int=§list.sum(xs)\n";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "language/stdlib/list.lib.sigil").unwrap();
        assert!(validate_canonical_form(
            &program,
            Some("language/stdlib/list.lib.sigil"),
            Some(source)
        )
        .is_ok());
    }
}

/// Validate canonical declaration ordering
fn validate_declaration_ordering(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check category order (type => extern => const => function => test)
    if let Err(e) = validate_category_boundaries(&program.declarations) {
        errors.extend(e);
    }

    // Check alphabetical order within each category
    let functions: Vec<_> = program
        .declarations
        .iter()
        .filter_map(|d| {
            if let Declaration::Function(f) = d {
                Some(f)
            } else {
                None
            }
        })
        .collect();

    let effects: Vec<_> = program
        .declarations
        .iter()
        .filter_map(|d| {
            if let Declaration::Effect(e) = d {
                Some(e)
            } else {
                None
            }
        })
        .collect();

    if let Err(e) = validate_alphabetical_order(&functions) {
        errors.extend(e);
    }

    if let Err(e) = validate_effect_alphabetical_order(&effects) {
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
            Declaration::Label(_) => 0,
            Declaration::Type(_) => 1,
            Declaration::Derive(_) => 2,
            Declaration::Protocol(_) => 3,
            Declaration::Effect(_) => 4,
            Declaration::Extern(_) => 5,
            Declaration::FeatureFlag(_) => 6,
            Declaration::Const(_) => 7,
            Declaration::Transform(_) => 8,
            Declaration::Function(_) => 9,
            Declaration::Rule(_) => 10,
            Declaration::Test(_) => 11,
        }
    };

    let mut last_category_index: i32 = -1;

    for decl in declarations {
        let current_index = get_category_index(decl) as i32;

        if current_index < last_category_index {
            let category_names = [
                "label",
                "type",
                "derive",
                "protocol",
                "effect",
                "extern",
                "featureFlag",
                "const",
                "transform",
                "function",
                "rule",
                "test",
            ];
            let category_symbols = [
                "label",
                "t",
                "derive",
                "protocol",
                "effect",
                "e",
                "featureFlag",
                "c",
                "transform",
                "λ",
                "rule",
                "test",
            ];

            return Err(vec![ValidationError::DeclarationOrderOld {
                message: format!(
                    "SIGIL-CANON-DECL-CATEGORY-ORDER: Wrong category position\n\
                     Found: {} ({}) at line {}\n\
                     Category order: label => t => derive => protocol => effect => e => featureFlag => c => transform => λ => rule => test",
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

fn validate_effect_alphabetical_order(effects: &[&EffectDecl]) -> Result<(), Vec<ValidationError>> {
    for i in 1..effects.len() {
        let prev = effects[i - 1];
        let curr = effects[i];

        if curr.name < prev.name {
            return Err(vec![ValidationError::DeclarationOrderOld {
                message: format!(
                    "SIGIL-CANON-DECL-ALPHABETICAL: Declaration out of alphabetical order\n\n\
                     Found: effect {} at line {}\n\
                     After: effect {} at line {}\n\n\
                     Within 'effect' category, declarations must be alphabetical.\n\
                     Solution: Move effect {} before effect {}",
                    curr.name,
                    curr.location.start.line,
                    prev.name,
                    prev.location.start.line,
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
        Declaration::Label(LabelDecl { location, .. }) => location,
        Declaration::Type(TypeDecl { location, .. }) => location,
        Declaration::Derive(DeriveDecl { location, .. }) => location,
        Declaration::Protocol(ProtocolDecl { location, .. }) => location,
        Declaration::Rule(RuleDecl { location, .. }) => location,
        Declaration::Effect(EffectDecl { location, .. }) => location,
        Declaration::Extern(ExternDecl { location, .. }) => location,
        Declaration::FeatureFlag(FeatureFlagDecl { location, .. }) => location,
        Declaration::Const(ConstDecl { location, .. }) => location,
        Declaration::Transform(TransformDecl { function, .. }) => &function.location,
        Declaration::Function(FunctionDecl { location, .. }) => location,
        Declaration::Test(TestDecl { location, .. }) => location,
    }
}

fn is_canonical_timestamp(value: &str) -> bool {
    if value.len() != 20 {
        return false;
    }

    let bytes = value.as_bytes();
    const DIGIT_POSITIONS: [usize; 14] = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if !DIGIT_POSITIONS
        .iter()
        .all(|index| bytes[*index].is_ascii_digit())
    {
        return false;
    }

    bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b'-'
        && bytes[16] == b'-'
        && bytes[19] == b'Z'
}
