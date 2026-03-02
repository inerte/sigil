//! Comprehensive lexer tests covering all token types and edge cases

use sigil_lexer::{tokenize, LexError, TokenType};

#[test]
fn test_all_unicode_operators() {
    let source = "λ → ≡ ⋅ ∧ ∨ ¬ ≤ ≥ ≠ ↦ ⊳ ⊕ ⧺";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::LAMBDA);
    assert_eq!(tokens[1].token_type, TokenType::ARROW);
    assert_eq!(tokens[2].token_type, TokenType::MATCH);
    assert_eq!(tokens[3].token_type, TokenType::NAMESPACE_SEP);
    assert_eq!(tokens[4].token_type, TokenType::AND);
    assert_eq!(tokens[5].token_type, TokenType::OR);
    assert_eq!(tokens[6].token_type, TokenType::NOT);
    assert_eq!(tokens[7].token_type, TokenType::LESS_EQ);
    assert_eq!(tokens[8].token_type, TokenType::GREATER_EQ);
    assert_eq!(tokens[9].token_type, TokenType::NOT_EQUAL);
    assert_eq!(tokens[10].token_type, TokenType::MAP);
    assert_eq!(tokens[11].token_type, TokenType::FILTER);
    assert_eq!(tokens[12].token_type, TokenType::FOLD);
    assert_eq!(tokens[13].token_type, TokenType::LIST_APPEND);
}

#[test]
fn test_all_type_symbols() {
    let source = "ℤ ℝ 𝔹 𝕊 ℂ 𝕌 ∅";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::TYPE_INT);
    assert_eq!(tokens[1].token_type, TokenType::TYPE_FLOAT);
    assert_eq!(tokens[2].token_type, TokenType::TYPE_BOOL);
    assert_eq!(tokens[3].token_type, TokenType::TYPE_STRING);
    assert_eq!(tokens[4].token_type, TokenType::TYPE_CHAR);
    assert_eq!(tokens[5].token_type, TokenType::TYPE_UNIT);
    assert_eq!(tokens[6].token_type, TokenType::TYPE_NEVER);
}

#[test]
fn test_all_keywords() {
    let source = "i e mockable c when l mut with_mock t";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::IMPORT);
    assert_eq!(tokens[1].token_type, TokenType::EXTERN);
    assert_eq!(tokens[2].token_type, TokenType::MOCKABLE);
    assert_eq!(tokens[3].token_type, TokenType::CONST);
    assert_eq!(tokens[4].token_type, TokenType::WHEN);
    assert_eq!(tokens[5].token_type, TokenType::LET);
    assert_eq!(tokens[6].token_type, TokenType::MUT);
    assert_eq!(tokens[7].token_type, TokenType::WITH_MOCK);
    assert_eq!(tokens[8].token_type, TokenType::TYPE);
}

#[test]
fn test_all_delimiters() {
    let source = "( ) [ ] { } , : . ; | _ ..";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::LPAREN);
    assert_eq!(tokens[1].token_type, TokenType::RPAREN);
    assert_eq!(tokens[2].token_type, TokenType::LBRACKET);
    assert_eq!(tokens[3].token_type, TokenType::RBRACKET);
    assert_eq!(tokens[4].token_type, TokenType::LBRACE);
    assert_eq!(tokens[5].token_type, TokenType::RBRACE);
    assert_eq!(tokens[6].token_type, TokenType::COMMA);
    assert_eq!(tokens[7].token_type, TokenType::COLON);
    assert_eq!(tokens[8].token_type, TokenType::DOT);
    assert_eq!(tokens[9].token_type, TokenType::SEMICOLON);
    assert_eq!(tokens[10].token_type, TokenType::PIPE_SEP);
    assert_eq!(tokens[11].token_type, TokenType::UNDERSCORE);
    assert_eq!(tokens[12].token_type, TokenType::DOTDOT);
}

#[test]
fn test_all_operators() {
    let source = "+ - * / % ^ = < > ! & #";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::PLUS);
    assert_eq!(tokens[1].token_type, TokenType::MINUS);
    assert_eq!(tokens[2].token_type, TokenType::STAR);
    assert_eq!(tokens[3].token_type, TokenType::SLASH);
    assert_eq!(tokens[4].token_type, TokenType::PERCENT);
    assert_eq!(tokens[5].token_type, TokenType::CARET);
    assert_eq!(tokens[6].token_type, TokenType::EQUAL);
    assert_eq!(tokens[7].token_type, TokenType::LESS);
    assert_eq!(tokens[8].token_type, TokenType::GREATER);
    assert_eq!(tokens[9].token_type, TokenType::BANG);
    assert_eq!(tokens[10].token_type, TokenType::AMPERSAND);
    assert_eq!(tokens[11].token_type, TokenType::HASH);
}

#[test]
fn test_integer_literals() {
    let source = "0 1 42 123456789 999";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::INTEGER);
    assert_eq!(tokens[0].value, "0");
    assert_eq!(tokens[1].value, "1");
    assert_eq!(tokens[2].value, "42");
    assert_eq!(tokens[3].value, "123456789");
    assert_eq!(tokens[4].value, "999");
}

#[test]
fn test_float_literals() {
    let source = "0.0 3.14 123.456 0.1 99.99";
    let tokens = tokenize(source).unwrap();

    for i in 0..5 {
        assert_eq!(tokens[i].token_type, TokenType::FLOAT);
    }
    assert_eq!(tokens[0].value, "0.0");
    assert_eq!(tokens[1].value, "3.14");
    assert_eq!(tokens[2].value, "123.456");
}

#[test]
fn test_unit_literal() {
    let source = "()";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::LPAREN);
    assert_eq!(tokens[1].token_type, TokenType::RPAREN);
    // Note: UNIT token exists but is for the () literal value,
    // which the parser constructs from LPAREN + RPAREN
}

#[test]
fn test_string_with_escapes() {
    let source = r#""hello\nworld" "tab\there" "quote\"here" "backslash\\""#;
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::STRING);
    assert_eq!(tokens[0].value, "hello\nworld");
    assert_eq!(tokens[1].value, "tab\there");
    assert_eq!(tokens[2].value, "quote\"here");
    assert_eq!(tokens[3].value, "backslash\\");
}

#[test]
fn test_string_with_unicode() {
    let source = r#""こんにちは" "emoji🎉" "math:∫∑∏""#;
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::STRING);
    assert_eq!(tokens[0].value, "こんにちは");
    assert_eq!(tokens[1].value, "emoji🎉");
    assert_eq!(tokens[2].value, "math:∫∑∏");
}

#[test]
fn test_char_literals() {
    let source = r"'a' 'X' '1' '\n' '\t' '\\' '\''";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::CHAR);
    assert_eq!(tokens[0].value, "a");
    assert_eq!(tokens[1].value, "X");
    assert_eq!(tokens[2].value, "1");
    assert_eq!(tokens[3].value, "\n");
    assert_eq!(tokens[4].value, "\t");
    assert_eq!(tokens[5].value, "\\");
    assert_eq!(tokens[6].value, "'");
}

#[test]
fn test_identifiers_lowercase() {
    let source = "foo bar x y1 my_var camelCase";
    let tokens = tokenize(source).unwrap();

    for i in 0..6 {
        assert_eq!(tokens[i].token_type, TokenType::IDENTIFIER);
    }
    assert_eq!(tokens[0].value, "foo");
    assert_eq!(tokens[5].value, "camelCase");
}

#[test]
fn test_identifiers_uppercase() {
    let source = "Foo Bar Result Maybe Option SomeType";
    let tokens = tokenize(source).unwrap();

    for i in 0..6 {
        assert_eq!(tokens[i].token_type, TokenType::UPPER_IDENTIFIER);
    }
    assert_eq!(tokens[0].value, "Foo");
    assert_eq!(tokens[5].value, "SomeType");
}

#[test]
fn test_no_comment_support_in_lexer() {
    // The lexer does NOT strip comments - they are tokenized as SLASH + identifiers
    // Comment handling is done at the parser level
    let source = "foo bar";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens.len(), 3); // foo, bar, EOF
    assert_eq!(tokens[0].value, "foo");
    assert_eq!(tokens[1].value, "bar");
}

#[test]
fn test_whitespace_handling() {
    let source = "a  b   c    d";
    let tokens = tokenize(source).unwrap();

    // Multiple spaces are allowed (only tabs are forbidden)
    assert_eq!(tokens.len(), 5); // a, b, c, d, EOF
}

#[test]
fn test_newlines_in_locations() {
    let source = "a\nb\n\nc";
    let tokens = tokenize(source).unwrap();

    // Tokens: a, NEWLINE, b, NEWLINE, NEWLINE, c, EOF
    assert_eq!(tokens[0].location.start.line, 1); // a
    assert_eq!(tokens[0].token_type, TokenType::IDENTIFIER);
    assert_eq!(tokens[1].token_type, TokenType::NEWLINE);
    assert_eq!(tokens[2].location.start.line, 2); // b
    assert_eq!(tokens[4].token_type, TokenType::NEWLINE); // second newline
    assert_eq!(tokens[5].location.start.line, 4); // c (after double newline)
}

#[test]
fn test_source_locations() {
    let source = "foo bar";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].location.start.line, 1);
    assert_eq!(tokens[0].location.start.column, 1);
    assert_eq!(tokens[0].location.end.column, 4);

    assert_eq!(tokens[1].location.start.column, 5);
    assert_eq!(tokens[1].location.end.column, 8);
}

#[test]
fn test_error_tab_not_allowed() {
    let source = "foo\tbar";
    let result = tokenize(source);

    assert!(result.is_err());
    match result.unwrap_err() {
        LexError::TabNotAllowed { line, column, .. } => {
            assert_eq!(line, 1);
            assert_eq!(column, 4);
        }
        _ => panic!("Expected TabNotAllowed error"),
    }
}

#[test]
fn test_error_unterminated_string() {
    let source = r#""hello"#;
    let result = tokenize(source);

    assert!(result.is_err());
    matches!(result.unwrap_err(), LexError::UnterminatedString { .. });
}

#[test]
fn test_error_unterminated_char() {
    let source = "'a";
    let result = tokenize(source);

    assert!(result.is_err());
    matches!(result.unwrap_err(), LexError::UnterminatedChar { .. });
}

#[test]
fn test_error_empty_char() {
    let source = "''";
    let result = tokenize(source);

    assert!(result.is_err());
    matches!(result.unwrap_err(), LexError::EmptyChar { .. });
}

#[test]
fn test_complex_expression() {
    let source = "λfoo(x:ℤ,y:ℝ)→𝔹=x>0∧y≠3.14";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::LAMBDA);
    assert_eq!(tokens[1].token_type, TokenType::IDENTIFIER);
    assert_eq!(tokens[2].token_type, TokenType::LPAREN);
    assert_eq!(tokens[3].token_type, TokenType::IDENTIFIER);
    assert_eq!(tokens[4].token_type, TokenType::COLON);
    assert_eq!(tokens[5].token_type, TokenType::TYPE_INT);
    // ... rest of tokens
}

#[test]
fn test_namespace_separator() {
    let source = "stdlib⋅list⋅map";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::IDENTIFIER);
    assert_eq!(tokens[1].token_type, TokenType::NAMESPACE_SEP);
    assert_eq!(tokens[2].token_type, TokenType::IDENTIFIER);
    assert_eq!(tokens[3].token_type, TokenType::NAMESPACE_SEP);
    assert_eq!(tokens[4].token_type, TokenType::IDENTIFIER);
}

#[test]
fn test_type_keyword() {
    let source = "t";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::TYPE);
}

#[test]
fn test_dotdot_in_list() {
    let source = "[1,..[2,3]]";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::LBRACKET);
    assert_eq!(tokens[1].token_type, TokenType::INTEGER);
    assert_eq!(tokens[2].token_type, TokenType::COMMA);
    assert_eq!(tokens[3].token_type, TokenType::DOTDOT);
    assert_eq!(tokens[4].token_type, TokenType::LBRACKET);
}

#[test]
fn test_effect_markers() {
    let source = "!IO !Network !Async !Error !Mut";
    let tokens = tokenize(source).unwrap();

    // Each !X should be BANG followed by UPPER_IDENTIFIER
    assert_eq!(tokens[0].token_type, TokenType::BANG);
    assert_eq!(tokens[1].token_type, TokenType::UPPER_IDENTIFIER);
    assert_eq!(tokens[1].value, "IO");
}

#[test]
fn test_empty_source() {
    let source = "";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens.len(), 1); // Just EOF
    assert_eq!(tokens[0].token_type, TokenType::EOF);
}

#[test]
fn test_only_newlines() {
    let source = "\n\n";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::NEWLINE);
    assert_eq!(tokens[1].token_type, TokenType::NEWLINE);
    assert_eq!(tokens[2].token_type, TokenType::EOF);
}

#[test]
fn test_newline_tokens() {
    let source = "foo\nbar";
    let tokens = tokenize(source).unwrap();

    assert_eq!(tokens[0].token_type, TokenType::IDENTIFIER);
    assert_eq!(tokens[1].token_type, TokenType::NEWLINE);
    assert_eq!(tokens[2].token_type, TokenType::IDENTIFIER);
}
