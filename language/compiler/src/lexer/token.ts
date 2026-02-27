/**
 * Token types for Sigil language lexer
 */
export enum TokenType {
  // Literals
  INTEGER = 'INTEGER',
  FLOAT = 'FLOAT',
  STRING = 'STRING',
  CHAR = 'CHAR',
  TRUE = 'TRUE',
  FALSE = 'FALSE',
  UNIT = 'UNIT',

  // Identifiers
  IDENTIFIER = 'IDENTIFIER',
  UPPER_IDENTIFIER = 'UPPER_IDENTIFIER',

  // Keywords (Unicode symbols)
  LAMBDA = 'LAMBDA',           // λ
  ARROW = 'ARROW',             // →
  MATCH = 'MATCH',             // ≡

  // Declaration keywords
  TYPE = 'TYPE',               // t
  IMPORT = 'IMPORT',           // i
  EXTERN = 'EXTERN',           // e
  LET = 'LET',                 // l
  CONST = 'CONST',             // c
  MUT = 'MUT',                 // mut
  MOCKABLE = 'MOCKABLE',       // mockable
  WITH_MOCK = 'WITH_MOCK',     // with_mock
  WHEN = 'WHEN',               // when

  // Type symbols
  TYPE_INT = 'TYPE_INT',       // ℤ
  TYPE_FLOAT = 'TYPE_FLOAT',   // ℝ
  TYPE_BOOL = 'TYPE_BOOL',     // 𝔹
  TYPE_STRING = 'TYPE_STRING', // 𝕊
  TYPE_CHAR = 'TYPE_CHAR',     // ℂ
  TYPE_UNIT = 'TYPE_UNIT',     // 𝕌
  TYPE_NEVER = 'TYPE_NEVER',   // ∅

  // Operators
  PLUS = 'PLUS',               // +
  MINUS = 'MINUS',             // -
  STAR = 'STAR',               // *
  SLASH = 'SLASH',             // /
  PERCENT = 'PERCENT',         // %
  CARET = 'CARET',             // ^

  EQUAL = 'EQUAL',             // =
  NOT_EQUAL = 'NOT_EQUAL',     // ≠
  LESS = 'LESS',               // <
  GREATER = 'GREATER',         // >
  LESS_EQ = 'LESS_EQ',         // ≤
  GREATER_EQ = 'GREATER_EQ',   // ≥

  AND = 'AND',                 // ∧
  OR = 'OR',                   // ∨
  NOT = 'NOT',                 // ¬

  PIPE = 'PIPE',               // |>
  COMPOSE_FWD = 'COMPOSE_FWD', // >>
  COMPOSE_BWD = 'COMPOSE_BWD', // <<

  APPEND = 'APPEND',           // ++
  LIST_APPEND = 'LIST_APPEND', // ⧺

  // List operations (built-in language constructs)
  MAP = 'MAP',                 // ↦
  FILTER = 'FILTER',           // ⊳
  FOLD = 'FOLD',               // ⊕

  // Delimiters
  LPAREN = 'LPAREN',           // (
  RPAREN = 'RPAREN',           // )
  LBRACKET = 'LBRACKET',       // [
  RBRACKET = 'RBRACKET',       // ]
  LBRACE = 'LBRACE',           // {
  RBRACE = 'RBRACE',           // }

  // Punctuation
  COLON = 'COLON',             // :
  SEMICOLON = 'SEMICOLON',     // ;
  COMMA = 'COMMA',             // ,
  NAMESPACE_SEP = 'NAMESPACE_SEP', // ⋅
  DOT = 'DOT',                 // .
  DOTDOT = 'DOTDOT',           // ..
  PIPE_SEP = 'PIPE_SEP',       // | (in pattern matching)
  UNDERSCORE = 'UNDERSCORE',   // _
  BANG = 'BANG',               // ! (for effects)
  AMPERSAND = 'AMPERSAND',     // & (for borrows)
  HASH = 'HASH',               // # (length operator)

  // Special
  EOF = 'EOF',
  NEWLINE = 'NEWLINE',
}

export interface SourceLocation {
  line: number;
  column: number;
  offset: number;
}

export interface Token {
  type: TokenType;
  value: string;
  start: SourceLocation;
  end: SourceLocation;
}

export function createToken(
  type: TokenType,
  value: string,
  start: SourceLocation,
  end: SourceLocation
): Token {
  return { type, value, start, end };
}

export function tokenToString(token: Token): string {
  return `${token.type}(${token.value}) at ${token.start.line}:${token.start.column}`;
}
