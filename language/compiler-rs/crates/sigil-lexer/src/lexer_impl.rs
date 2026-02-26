//! Lexer implementation for Sigil
//!
//! This module provides the main `Lexer` struct that tokenizes Sigil source code.

use crate::token::{Position, SourceLocation, Token, TokenType};
use logos::Logos;
use thiserror::Error;

/// Lexer errors with source locations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum LexError {
    #[error("SIGIL-LEX-TAB: tab characters not allowed at {line}:{column}")]
    TabNotAllowed { line: usize, column: usize },

    #[error("SIGIL-LEX-CRLF: standalone carriage return not allowed at {line}:{column}")]
    StandaloneCarriageReturn { line: usize, column: usize },

    #[error("SIGIL-LEX-UNTERMINATED-STRING: unterminated string literal at {line}:{column}")]
    UnterminatedString { line: usize, column: usize },

    #[error("SIGIL-LEX-UNTERMINATED-COMMENT: unterminated multi-line comment at {line}:{column}")]
    UnterminatedComment { line: usize, column: usize },

    #[error("SIGIL-LEX-EMPTY-CHAR: empty character literal at {line}:{column}")]
    EmptyChar { line: usize, column: usize },

    #[error("SIGIL-LEX-CHAR-LENGTH: character literal must contain exactly one character at {line}:{column}")]
    CharLength { line: usize, column: usize },

    #[error("SIGIL-LEX-UNTERMINATED-CHAR: unterminated character literal at {line}:{column}")]
    UnterminatedChar { line: usize, column: usize },

    #[error("SIGIL-LEX-INVALID-ESCAPE: invalid escape sequence '\\{escape}' at {line}:{column}")]
    InvalidEscape {
        escape: char,
        line: usize,
        column: usize,
    },

    #[error("SIGIL-LEX-UNEXPECTED-CHAR: unexpected character '{ch}' (U+{code:04X}) at {line}:{column}")]
    UnexpectedChar {
        ch: char,
        code: u32,
        line: usize,
        column: usize,
    },
}

/// The Sigil lexer
pub struct Lexer {
    source: String,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    /// Create a new lexer for the given source code
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let chars: Vec<char> = source.chars().collect();

        Self {
            source,
            chars,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    /// Tokenize the entire source code
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        // First pass: use logos for most tokens
        let mut lex = TokenType::lexer(&self.source);
        let mut byte_offset = 0;
        let mut line = 1;
        let mut column = 1;

        while let Some(result) = lex.next() {
            let span = lex.span();
            let text = lex.slice();

            // Calculate position for this token
            let start_offset = span.start;
            let end_offset = span.end;

            // Skip ahead to the correct position
            while byte_offset < start_offset {
                if byte_offset < self.source.len() {
                    let ch = self.source[byte_offset..].chars().next().unwrap();
                    byte_offset += ch.len_utf8();
                    if ch == '\n' {
                        line += 1;
                        column = 1;
                    } else if ch == '\t' {
                        return Err(LexError::TabNotAllowed { line, column });
                    } else {
                        column += 1;
                    }
                }
            }

            let start_line = line;
            let start_column = column;

            match result {
                Ok(token_type) => {
                    // Handle special cases for STRING and CHAR
                    if text.starts_with('"') || text.starts_with('\'') {
                        // These need custom handling
                        continue;
                    }

                    // Update position based on token text
                    for ch in text.chars() {
                        column += 1;
                    }
                    byte_offset = end_offset;

                    let location = SourceLocation::new(
                        Position::new(start_line, start_column, start_offset),
                        Position::new(line, column - 1, end_offset),
                    );

                    tokens.push(Token::new(token_type, text.to_string(), location));
                }
                Err(_) => {
                    // Handle custom tokens or errors
                }
            }
        }

        // Second pass: manually handle strings, chars, and comments
        self.pos = 0;
        self.line = 1;
        self.column = 1;
        let mut final_tokens = Vec::new();

        while !self.is_at_end() {
            let start = self.current_position();
            let ch = self.peek();

            match ch {
                // Comments
                '⟦' => {
                    self.advance();
                    self.skip_multiline_comment()?;
                    continue;
                }
                // Strings
                '"' => {
                    let token = self.scan_string()?;
                    final_tokens.push(token);
                    continue;
                }
                // Characters
                '\'' => {
                    let token = self.scan_char()?;
                    final_tokens.push(token);
                    continue;
                }
                // Tab detection
                '\t' => {
                    return Err(LexError::TabNotAllowed {
                        line: self.line,
                        column: self.column,
                    });
                }
                // Carriage return detection
                '\r' => {
                    self.advance();
                    if self.peek() == '\n' {
                        self.advance();
                        let location = SourceLocation::new(start, self.current_position());
                        final_tokens.push(Token::new(TokenType::NEWLINE, "\n".to_string(), location));
                    } else {
                        return Err(LexError::StandaloneCarriageReturn {
                            line: self.line,
                            column: self.column,
                        });
                    }
                    continue;
                }
                _ => {
                    self.advance();
                }
            }
        }

        // Merge tokens from both passes (prioritizing manual tokens)
        // For simplicity, we'll use a simpler approach: just scan everything manually
        Ok(self.tokenize_manual()?)
    }

    /// Full manual tokenization (matching TypeScript exactly)
    fn tokenize_manual(&mut self) -> Result<Vec<Token>, LexError> {
        self.pos = 0;
        self.line = 1;
        self.column = 1;
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.scan_token(&mut tokens)?;
        }

        // Add EOF token
        let eof_pos = self.current_position();
        tokens.push(Token::new(
            TokenType::EOF,
            String::new(),
            SourceLocation::single(eof_pos),
        ));

        Ok(tokens)
    }

    fn scan_token(&mut self, tokens: &mut Vec<Token>) -> Result<(), LexError> {
        let start = self.current_position();
        let ch = self.advance();

        match ch {
            // Whitespace
            ' ' => {} // Skip spaces
            '\n' => {
                tokens.push(Token::new(TokenType::NEWLINE, "\n".to_string(), SourceLocation::new(start, self.current_position())));
            }
            '\r' => {
                if self.peek() == '\n' {
                    self.advance();
                    tokens.push(Token::new(TokenType::NEWLINE, "\n".to_string(), SourceLocation::new(start, self.current_position())));
                } else {
                    return Err(LexError::StandaloneCarriageReturn {
                        line: self.line,
                        column: self.column,
                    });
                }
            }
            '\t' => {
                return Err(LexError::TabNotAllowed {
                    line: self.line,
                    column: self.column - 1,
                });
            }

            // Single-character tokens
            '(' => self.add_token(tokens, TokenType::LPAREN, "(", start),
            ')' => self.add_token(tokens, TokenType::RPAREN, ")", start),
            '[' => self.add_token(tokens, TokenType::LBRACKET, "[", start),
            ']' => self.add_token(tokens, TokenType::RBRACKET, "]", start),
            '{' => self.add_token(tokens, TokenType::LBRACE, "{", start),
            '}' => self.add_token(tokens, TokenType::RBRACE, "}", start),
            ':' => self.add_token(tokens, TokenType::COLON, ":", start),
            ';' => self.add_token(tokens, TokenType::SEMICOLON, ";", start),
            ',' => self.add_token(tokens, TokenType::COMMA, ",", start),
            '⋅' => self.add_token(tokens, TokenType::NAMESPACE_SEP, "⋅", start),
            '_' => self.add_token(tokens, TokenType::UNDERSCORE, "_", start),
            '!' => self.add_token(tokens, TokenType::BANG, "!", start),
            '&' => self.add_token(tokens, TokenType::AMPERSAND, "&", start),
            '#' => self.add_token(tokens, TokenType::HASH, "#", start),
            '/' => self.add_token(tokens, TokenType::SLASH, "/", start),
            '%' => self.add_token(tokens, TokenType::PERCENT, "%", start),
            '^' => self.add_token(tokens, TokenType::CARET, "^", start),
            '*' => self.add_token(tokens, TokenType::STAR, "*", start),
            '=' => self.add_token(tokens, TokenType::EQUAL, "=", start),

            // Multi-character operators
            '+' => {
                if self.match_char('+') {
                    self.add_token(tokens, TokenType::APPEND, "++", start);
                } else {
                    self.add_token(tokens, TokenType::PLUS, "+", start);
                }
            }
            '-' => self.add_token(tokens, TokenType::MINUS, "-", start),
            '<' => {
                if self.match_char('<') {
                    self.add_token(tokens, TokenType::COMPOSE_BWD, "<<", start);
                } else {
                    self.add_token(tokens, TokenType::LESS, "<", start);
                }
            }
            '>' => {
                if self.match_char('>') {
                    self.add_token(tokens, TokenType::COMPOSE_FWD, ">>", start);
                } else {
                    self.add_token(tokens, TokenType::GREATER, ">", start);
                }
            }
            '|' => {
                if self.match_char('>') {
                    self.add_token(tokens, TokenType::PIPE, "|>", start);
                } else {
                    self.add_token(tokens, TokenType::PIPE_SEP, "|", start);
                }
            }
            '.' => {
                if self.match_char('.') {
                    self.add_token(tokens, TokenType::DOTDOT, "..", start);
                } else {
                    self.add_token(tokens, TokenType::DOT, ".", start);
                }
            }

            // Unicode operators
            '≠' => self.add_token(tokens, TokenType::NOT_EQUAL, "≠", start),
            '≤' => self.add_token(tokens, TokenType::LESS_EQ, "≤", start),
            '≥' => self.add_token(tokens, TokenType::GREATER_EQ, "≥", start),
            '∧' => self.add_token(tokens, TokenType::AND, "∧", start),
            '∨' => self.add_token(tokens, TokenType::OR, "∨", start),
            '¬' => self.add_token(tokens, TokenType::NOT, "¬", start),
            '↦' => self.add_token(tokens, TokenType::MAP, "↦", start),
            '⊳' => self.add_token(tokens, TokenType::FILTER, "⊳", start),
            '⊕' => self.add_token(tokens, TokenType::FOLD, "⊕", start),
            '⧺' => self.add_token(tokens, TokenType::LIST_APPEND, "⧺", start),

            // Unicode keywords
            'λ' => self.add_token(tokens, TokenType::LAMBDA, "λ", start),
            '→' => self.add_token(tokens, TokenType::ARROW, "→", start),
            '≡' => self.add_token(tokens, TokenType::MATCH, "≡", start),

            // Unicode type symbols
            'ℤ' => self.add_token(tokens, TokenType::TYPE_INT, "ℤ", start),
            'ℝ' => self.add_token(tokens, TokenType::TYPE_FLOAT, "ℝ", start),
            '𝔹' => self.add_token(tokens, TokenType::TYPE_BOOL, "𝔹", start),
            '𝕊' => self.add_token(tokens, TokenType::TYPE_STRING, "𝕊", start),
            'ℂ' => self.add_token(tokens, TokenType::TYPE_CHAR, "ℂ", start),
            '𝕌' => self.add_token(tokens, TokenType::TYPE_UNIT, "𝕌", start),
            '∅' => self.add_token(tokens, TokenType::TYPE_NEVER, "∅", start),

            // Boolean literals
            '⊤' => self.add_token(tokens, TokenType::TRUE, "⊤", start),
            '⊥' => self.add_token(tokens, TokenType::FALSE, "⊥", start),

            // Comments
            '⟦' => {
                self.skip_multiline_comment()?;
            }

            // String literals
            '"' => {
                let token = self.scan_string_from_quote(start)?;
                tokens.push(token);
            }

            // Character literals
            '\'' => {
                let token = self.scan_char_from_quote(start)?;
                tokens.push(token);
            }

            // Numbers
            '0'..='9' => {
                let token = self.scan_number(ch, start)?;
                tokens.push(token);
            }

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' => {
                let token = self.scan_identifier(ch, start)?;
                tokens.push(token);
            }

            _ => {
                return Err(LexError::UnexpectedChar {
                    ch,
                    code: ch as u32,
                    line: self.line,
                    column: self.column - 1,
                });
            }
        }

        Ok(())
    }

    fn scan_string(&mut self) -> Result<Token, LexError> {
        let start = self.current_position();
        self.advance(); // consume opening "
        self.scan_string_from_quote(start)
    }

    fn scan_string_from_quote(&mut self, start: Position) -> Result<Token, LexError> {
        let mut value = String::new();

        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                return Err(LexError::UnterminatedString {
                    line: self.line,
                    column: self.column,
                });
            }
            if self.peek() == '\\' {
                self.advance();
                value.push(self.scan_escape_sequence()?);
            } else {
                value.push(self.advance());
            }
        }

        if self.is_at_end() {
            return Err(LexError::UnterminatedString {
                line: self.line,
                column: self.column,
            });
        }

        self.advance(); // consume closing "

        Ok(Token::new(
            TokenType::STRING,
            value,
            SourceLocation::new(start, self.current_position()),
        ))
    }

    fn scan_char(&mut self) -> Result<Token, LexError> {
        let start = self.current_position();
        self.advance(); // consume opening '
        self.scan_char_from_quote(start)
    }

    fn scan_char_from_quote(&mut self, start: Position) -> Result<Token, LexError> {
        let value = if self.peek() == '\\' {
            self.advance();
            self.scan_escape_sequence()?
        } else if self.peek() == '\'' {
            return Err(LexError::EmptyChar {
                line: self.line,
                column: self.column,
            });
        } else {
            self.advance()
        };

        if self.peek() != '\'' {
            return Err(LexError::CharLength {
                line: self.line,
                column: self.column,
            });
        }

        self.advance(); // consume closing '

        Ok(Token::new(
            TokenType::CHAR,
            value.to_string(),
            SourceLocation::new(start, self.current_position()),
        ))
    }

    fn scan_escape_sequence(&mut self) -> Result<char, LexError> {
        let ch = self.advance();
        match ch {
            'n' => Ok('\n'),
            't' => Ok('\t'),
            'r' => Ok('\r'),
            '\\' => Ok('\\'),
            '"' => Ok('"'),
            '\'' => Ok('\''),
            _ => Err(LexError::InvalidEscape {
                escape: ch,
                line: self.line,
                column: self.column - 1,
            }),
        }
    }

    fn skip_multiline_comment(&mut self) -> Result<(), LexError> {
        while !self.is_at_end() && self.peek() != '⟧' {
            self.advance();
        }

        if self.is_at_end() {
            return Err(LexError::UnterminatedComment {
                line: self.line,
                column: self.column,
            });
        }

        self.advance(); // consume closing ⟧
        Ok(())
    }

    fn scan_number(&mut self, first_digit: char, start: Position) -> Result<Token, LexError> {
        let mut value = String::from(first_digit);

        while self.is_digit(self.peek()) {
            value.push(self.advance());
        }

        // Check for float
        if self.peek() == '.' && self.is_digit(self.peek_next()) {
            value.push(self.advance()); // consume .

            while self.is_digit(self.peek()) {
                value.push(self.advance());
            }

            Ok(Token::new(
                TokenType::FLOAT,
                value,
                SourceLocation::new(start, self.current_position()),
            ))
        } else {
            Ok(Token::new(
                TokenType::INTEGER,
                value,
                SourceLocation::new(start, self.current_position()),
            ))
        }
    }

    fn scan_identifier(&mut self, first_char: char, start: Position) -> Result<Token, LexError> {
        let mut value = String::from(first_char);

        while self.is_alphanumeric(self.peek()) || self.peek() == '_' {
            value.push(self.advance());
        }

        // Check for keywords
        let token_type = match value.as_str() {
            "t" => TokenType::TYPE,
            "i" => TokenType::IMPORT,
            "e" => TokenType::EXTERN,
            "l" => TokenType::LET,
            "c" => TokenType::CONST,
            "mut" => TokenType::MUT,
            "mockable" => TokenType::MOCKABLE,
            "with_mock" => TokenType::WITH_MOCK,
            "export" => TokenType::EXPORT,
            "when" => TokenType::WHEN,
            _ => {
                if first_char.is_uppercase() {
                    TokenType::UPPER_IDENTIFIER
                } else {
                    TokenType::IDENTIFIER
                }
            }
        };

        Ok(Token::new(
            token_type,
            value,
            SourceLocation::new(start, self.current_position()),
        ))
    }

    // Helper methods
    fn is_at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.pos]
        }
    }

    fn peek_next(&self) -> char {
        if self.pos + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.pos + 1]
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.chars[self.pos];
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        ch
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.chars[self.pos] != expected {
            false
        } else {
            self.pos += 1;
            self.column += 1;
            true
        }
    }

    fn is_digit(&self, ch: char) -> bool {
        ch >= '0' && ch <= '9'
    }

    fn is_alpha(&self, ch: char) -> bool {
        (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')
    }

    fn is_alphanumeric(&self, ch: char) -> bool {
        self.is_alpha(ch) || self.is_digit(ch)
    }

    fn current_position(&self) -> Position {
        Position::new(self.line, self.column, self.pos)
    }

    fn add_token(
        &self,
        tokens: &mut Vec<Token>,
        token_type: TokenType,
        value: &str,
        start: Position,
    ) {
        let end = self.current_position();
        tokens.push(Token::new(
            token_type,
            value.to_string(),
            SourceLocation::new(start, end),
        ));
    }
}

/// Convenience function to tokenize source code
pub fn tokenize(source: impl Into<String>) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_tokens() {
        let source = "λ → ≡";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens.len(), 4); // 3 tokens + EOF
        assert_eq!(tokens[0].token_type, TokenType::LAMBDA);
        assert_eq!(tokens[1].token_type, TokenType::ARROW);
        assert_eq!(tokens[2].token_type, TokenType::MATCH);
    }

    #[test]
    fn test_numbers() {
        let source = "42 3.14";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token_type, TokenType::INTEGER);
        assert_eq!(tokens[0].value, "42");
        assert_eq!(tokens[1].token_type, TokenType::FLOAT);
        assert_eq!(tokens[1].value, "3.14");
    }

    #[test]
    fn test_strings() {
        let source = r#""hello world""#;
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token_type, TokenType::STRING);
        assert_eq!(tokens[0].value, "hello world");
    }

    #[test]
    fn test_identifiers() {
        let source = "foo Bar mut export";
        let tokens = tokenize(source).unwrap();
        assert_eq!(tokens[0].token_type, TokenType::IDENTIFIER);
        assert_eq!(tokens[1].token_type, TokenType::UPPER_IDENTIFIER);
        assert_eq!(tokens[2].token_type, TokenType::MUT);
        assert_eq!(tokens[3].token_type, TokenType::EXPORT);
    }

    #[test]
    fn test_tab_error() {
        let source = "foo\tbar";
        let result = tokenize(source);
        assert!(matches!(result, Err(LexError::TabNotAllowed { .. })));
    }
}
