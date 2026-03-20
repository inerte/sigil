//! Token types for the Sigil language lexer
//!
//! This module defines all token types used in the Sigil language lexer.

use logos::Logos;

/// All token types in the Sigil language.
#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[logos(skip r"[ ]+")]  // Skip spaces (but not newlines or tabs)
pub enum TokenType {
    // ========================================================================
    // LITERALS
    // ========================================================================
    #[regex(r"[0-9]+", priority = 3)]
    INTEGER,

    #[regex(r"[0-9]+\.[0-9]+", priority = 4)]
    FLOAT,

    #[token("true")]
    TRUE,

    #[token("false")]
    FALSE,

    #[token("()")]
    UNIT,

    // STRING and CHAR are handled specially in lexer_impl.rs
    STRING,
    CHAR,

    // ========================================================================
    // IDENTIFIERS
    // ========================================================================
    #[regex(r"[a-z][a-zA-Z0-9_]*", priority = 1)]
    IDENTIFIER,

    #[regex(r"[A-Z][a-zA-Z0-9_]*", priority = 1)]
    UpperIdentifier,

    // ========================================================================
    // KEYWORDS (canonical symbols + words)
    // ========================================================================
    #[token("λ")]
    LAMBDA,

    #[token("=>")]
    ARROW,

    #[token("match", priority = 3)]
    MATCH,

    #[token("concurrent", priority = 3)]
    Concurrent,

    #[token("spawn", priority = 3)]
    Spawn,

    #[token("spawnEach", priority = 3)]
    SpawnEach,

    // ========================================================================
    // DECLARATION KEYWORDS
    // ========================================================================
    #[token("t", priority = 2)]
    TYPE,

    #[token("i", priority = 2)]
    IMPORT,

    #[token("e", priority = 2)]
    EXTERN,

    #[token("l", priority = 2)]
    LET,

    #[token("c", priority = 2)]
    CONST,

    #[token("mut")]
    MUT,

    #[token("withMock")]
    WithMock,

    #[token("when")]
    WHEN,

    // ========================================================================
    // TYPE SYMBOLS
    // ========================================================================
    #[token("Int", priority = 2)]
    TypeInt,

    #[token("Float", priority = 2)]
    TypeFloat,

    #[token("Bool", priority = 2)]
    TypeBool,

    #[token("String", priority = 2)]
    TypeString,

    #[token("Char", priority = 2)]
    TypeChar,

    #[token("Unit", priority = 2)]
    TypeUnit,

    #[token("Never", priority = 2)]
    TypeNever,

    // ========================================================================
    // OPERATORS
    // ========================================================================
    #[token("+")]
    PLUS,

    #[token("-")]
    MINUS,

    #[token("*")]
    STAR,

    #[token("/")]
    SLASH,

    #[token("%")]
    PERCENT,

    #[token("^")]
    CARET,

    #[token("=")]
    EQUAL,

    #[token("≠")]
    NotEqual,

    #[token("<")]
    LESS,

    #[token(">")]
    GREATER,

    #[token("≤")]
    LessEq,

    #[token("≥")]
    GreaterEq,

    #[token("and", priority = 3)]
    AND,

    #[token("or", priority = 3)]
    OR,

    #[token("¬")]
    NOT,

    #[token("|>")]
    PIPE,

    #[token(">>")]
    ComposeFwd,

    #[token("<<")]
    ComposeBwd,

    #[token("++")]
    APPEND,

    #[token("⧺")]
    ListAppend,

    // ========================================================================
    // LIST OPERATIONS (built-in language constructs)
    // ========================================================================
    #[token("↦")]
    MAP,

    #[token("⊳")]
    FILTER,

    #[token("⊕")]
    FOLD,

    // ========================================================================
    // DELIMITERS
    // ========================================================================
    #[token("(")]
    LPAREN,

    #[token(")")]
    RPAREN,

    #[token("[")]
    LBRACKET,

    #[token("]")]
    RBRACKET,

    #[token("{")]
    LBRACE,

    #[token("}")]
    RBRACE,

    // ========================================================================
    // PUNCTUATION
    // ========================================================================
    #[token(":")]
    COLON,

    #[token(";")]
    SEMICOLON,

    #[token(",")]
    COMMA,

    #[token("::")]
    NamespaceSep,

    #[token(".")]
    DOT,

    #[token("..")]
    DOTDOT,

    #[token("|")]
    PipeSep,

    #[token("_")]
    UNDERSCORE,

    #[token("!")]
    BANG,

    #[token("&")]
    AMPERSAND,

    #[token("#")]
    HASH,

    // ========================================================================
    // SPECIAL
    // ========================================================================
    #[regex(r"\n")]
    NEWLINE,

    EOF,

    // ========================================================================
    // ERROR TOKENS (handled by custom error type)
    // ========================================================================
    ERROR,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Position in source code (1-indexed line and column, 0-indexed byte offset)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self { line, column, offset }
    }
}

/// Source location (start and end positions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceLocation {
    pub start: Position,
    pub end: Position,
}

impl SourceLocation {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn single(pos: Position) -> Self {
        Self { start: pos, end: pos }
    }
}

/// Token with type, value, and source location
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub location: SourceLocation,
}

impl Token {
    pub fn new(token_type: TokenType, value: String, location: SourceLocation) -> Self {
        Self {
            token_type,
            value,
            location,
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({}) at {}:{}",
            self.token_type, self.value, self.location.start.line, self.location.start.column
        )
    }
}
