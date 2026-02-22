/**
 * Mint Language Lexer
 *
 * Tokenizes Mint source code with canonical formatting enforcement.
 * The lexer enforces formatting rules at tokenization time - incorrectly
 * formatted code produces lexical errors.
 */

import { Token, TokenType, SourceLocation, createToken } from './token.js';

export class LexerError extends Error {
  constructor(
    message: string,
    public location: SourceLocation
  ) {
    super(`Lexer error at ${location.line}:${location.column}: ${message}`);
    this.name = 'LexerError';
  }
}

export class Lexer {
  private chars: string[]; // Array of Unicode characters
  private pos: number = 0;
  private line: number = 1;
  private column: number = 1;
  private tokens: Token[] = [];

  constructor(source: string) {
    // Split into array of Unicode characters (handles multi-byte properly)
    this.chars = Array.from(source);
  }

  /**
   * Tokenize the entire source
   */
  public tokenize(): Token[] {
    while (!this.isAtEnd()) {
      this.scanToken();
    }

    // Add EOF token
    const eofLoc = this.currentLocation();
    this.tokens.push(createToken(TokenType.EOF, '', eofLoc, eofLoc));

    return this.tokens;
  }

  private scanToken(): void {
    const start = this.currentLocation();
    const char = this.advance();

    switch (char) {
      // Whitespace - only space and newline allowed
      case ' ':
        // Space is only significant for token separation
        break;

      case '\n':
        this.addToken(TokenType.NEWLINE, '\n', start);
        break;

      case '\r':
        if (this.peek() === '\n') {
          this.advance(); // Skip \r\n
          this.addToken(TokenType.NEWLINE, '\n', start);
        } else {
          this.error('Standalone \\r not allowed - use \\n for line breaks');
        }
        break;

      case '\t':
        this.error('Tab characters not allowed - use spaces');
        break;

      // Comments
      case '/':
        if (this.match('/')) {
          this.lineComment();
        } else {
          this.addToken(TokenType.SLASH, '/', start);
        }
        break;

      // Single-character tokens
      case '(':
        this.addToken(TokenType.LPAREN, '(', start);
        break;
      case ')':
        this.addToken(TokenType.RPAREN, ')', start);
        break;
      case '[':
        this.addToken(TokenType.LBRACKET, '[', start);
        break;
      case ']':
        this.addToken(TokenType.RBRACKET, ']', start);
        break;
      case '{':
        this.addToken(TokenType.LBRACE, '{', start);
        break;
      case '}':
        this.addToken(TokenType.RBRACE, '}', start);
        break;
      case ':':
        this.addToken(TokenType.COLON, ':', start);
        break;
      case ';':
        this.addToken(TokenType.SEMICOLON, ';', start);
        break;
      case ',':
        this.addToken(TokenType.COMMA, ',', start);
        break;
      case '_':
        this.addToken(TokenType.UNDERSCORE, '_', start);
        break;
      case '!':
        this.addToken(TokenType.BANG, '!', start);
        break;
      case '&':
        this.addToken(TokenType.AMPERSAND, '&', start);
        break;

      // Operators
      case '+':
        if (this.match('+')) {
          this.addToken(TokenType.APPEND, '++', start);
        } else {
          this.addToken(TokenType.PLUS, '+', start);
        }
        break;

      case '-':
        this.addToken(TokenType.MINUS, '-', start);
        break;

      case '*':
        this.addToken(TokenType.STAR, '*', start);
        break;

      case '%':
        this.addToken(TokenType.PERCENT, '%', start);
        break;

      case '^':
        this.addToken(TokenType.CARET, '^', start);
        break;

      case '=':
        this.addToken(TokenType.EQUAL, '=', start);
        break;

      case '<':
        if (this.match('<')) {
          this.addToken(TokenType.COMPOSE_BWD, '<<', start);
        } else {
          this.addToken(TokenType.LESS, '<', start);
        }
        break;

      case '>':
        if (this.match('>')) {
          this.addToken(TokenType.COMPOSE_FWD, '>>', start);
        } else {
          this.addToken(TokenType.GREATER, '>', start);
        }
        break;

      case '|':
        if (this.match('>')) {
          this.addToken(TokenType.PIPE, '|>', start);
        } else {
          this.addToken(TokenType.PIPE_SEP, '|', start);
        }
        break;

      case '.':
        if (this.match('.')) {
          this.addToken(TokenType.DOTDOT, '..', start);
        } else {
          this.addToken(TokenType.DOT, '.', start);
        }
        break;

      // Unicode operators
      case '≠':
        this.addToken(TokenType.NOT_EQUAL, '≠', start);
        break;
      case '≤':
        this.addToken(TokenType.LESS_EQ, '≤', start);
        break;
      case '≥':
        this.addToken(TokenType.GREATER_EQ, '≥', start);
        break;
      case '∧':
        this.addToken(TokenType.AND, '∧', start);
        break;
      case '∨':
        this.addToken(TokenType.OR, '∨', start);
        break;
      case '¬':
        this.addToken(TokenType.NOT, '¬', start);
        break;

      // List operations (built-in)
      case '↦':
        this.addToken(TokenType.MAP, '↦', start);
        break;
      case '⊳':
        this.addToken(TokenType.FILTER, '⊳', start);
        break;
      case '⊕':
        this.addToken(TokenType.FOLD, '⊕', start);
        break;

      // Unicode keywords
      case 'λ':
        this.addToken(TokenType.LAMBDA, 'λ', start);
        break;
      case '→':
        this.addToken(TokenType.ARROW, '→', start);
        break;
      case '≡':
        this.addToken(TokenType.MATCH, '≡', start);
        break;

      // Unicode type symbols
      case 'ℤ':
        this.addToken(TokenType.TYPE_INT, 'ℤ', start);
        break;
      case 'ℝ':
        this.addToken(TokenType.TYPE_FLOAT, 'ℝ', start);
        break;
      case '𝔹':
        this.addToken(TokenType.TYPE_BOOL, '𝔹', start);
        break;
      case '𝕊':
        this.addToken(TokenType.TYPE_STRING, '𝕊', start);
        break;
      case 'ℂ':
        this.addToken(TokenType.TYPE_CHAR, 'ℂ', start);
        break;
      case '𝕌':
        this.addToken(TokenType.TYPE_UNIT, '𝕌', start);
        break;
      case '∅':
        this.addToken(TokenType.TYPE_NEVER, '∅', start);
        break;

      // Boolean literals
      case '⊤':
        this.addToken(TokenType.TRUE, '⊤', start);
        break;
      case '⊥':
        this.addToken(TokenType.FALSE, '⊥', start);
        break;

      // String literals
      case '"':
        this.string();
        break;

      // Character literals
      case "'":
        this.character();
        break;

      default:
        if (this.isDigit(char)) {
          this.number();
        } else if (this.isAlpha(char)) {
          this.identifier();
        } else {
          this.error(`Unexpected character: ${char} (U+${char.codePointAt(0)?.toString(16).toUpperCase().padStart(4, '0')})`);
        }
    }
  }

  private lineComment(): void {
    // Consume until end of line
    while (this.peek() !== '\n' && !this.isAtEnd()) {
      this.advance();
    }
  }

  private string(): void {
    const start = this.currentLocation();
    start.offset--; // Include opening quote
    start.column--;

    let value = '';

    while (this.peek() !== '"' && !this.isAtEnd()) {
      if (this.peek() === '\n') {
        this.error('Unterminated string literal');
      }
      if (this.peek() === '\\') {
        this.advance();
        value += this.escapeSequence();
      } else {
        value += this.advance();
      }
    }

    if (this.isAtEnd()) {
      this.error('Unterminated string literal');
    }

    this.advance(); // Closing "

    this.addToken(TokenType.STRING, value, start);
  }

  private character(): void {
    const start = this.currentLocation();
    start.offset--; // Include opening quote
    start.column--;

    let value = '';

    if (this.peek() === '\\') {
      this.advance();
      value = this.escapeSequence();
    } else if (this.peek() === "'") {
      this.error('Empty character literal');
    } else {
      value = this.advance();
    }

    if (this.peek() !== "'") {
      this.error('Character literal must contain exactly one character');
    }

    this.advance(); // Closing '

    this.addToken(TokenType.CHAR, value, start);
  }

  private escapeSequence(): string {
    const char = this.advance();
    switch (char) {
      case 'n': return '\n';
      case 't': return '\t';
      case 'r': return '\r';
      case '\\': return '\\';
      case '"': return '"';
      case "'": return "'";
      default:
        this.error(`Invalid escape sequence: \\${char}`);
        return '';
    }
  }

  private number(): void {
    const start = this.currentLocation();
    start.offset--;
    start.column--;

    // Handle negative numbers (but - is already consumed)
    let value = this.chars[this.pos - 1];

    while (this.isDigit(this.peek())) {
      value += this.advance();
    }

    // Check for float
    if (this.peek() === '.' && this.isDigit(this.peekNext())) {
      value += this.advance(); // Consume .

      while (this.isDigit(this.peek())) {
        value += this.advance();
      }

      this.addToken(TokenType.FLOAT, value, start);
    } else {
      this.addToken(TokenType.INTEGER, value, start);
    }
  }

  private identifier(): void {
    const start = this.currentLocation();
    start.offset--;
    start.column--;

    const firstChar = this.chars[this.pos - 1];
    let value = firstChar;

    while (this.isAlphaNumeric(this.peek()) || this.peek() === '_') {
      value += this.advance();
    }

    // Check for single-char keywords
    const type = this.keywordType(value);

    if (type) {
      this.addToken(type, value, start);
    } else if (this.isUpperCase(firstChar)) {
      this.addToken(TokenType.UPPER_IDENTIFIER, value, start);
    } else {
      this.addToken(TokenType.IDENTIFIER, value, start);
    }
  }

  private keywordType(text: string): TokenType | null {
    switch (text) {
      case 't': return TokenType.TYPE;
      case 'i': return TokenType.IMPORT;
      case 'l': return TokenType.LET;
      case 'c': return TokenType.CONST;
      default: return null;
    }
  }

  private match(expected: string): boolean {
    if (this.isAtEnd()) return false;
    if (this.chars[this.pos] !== expected) return false;
    this.pos++;
    this.column++;
    return true;
  }

  private peek(): string {
    if (this.isAtEnd()) return '\0';
    return this.chars[this.pos];
  }

  private peekNext(): string {
    if (this.pos + 1 >= this.chars.length) return '\0';
    return this.chars[this.pos + 1];
  }

  private advance(): string {
    const char = this.chars[this.pos++];
    if (char === '\n') {
      this.line++;
      this.column = 1;
    } else {
      this.column++;
    }
    return char;
  }

  private isAtEnd(): boolean {
    return this.pos >= this.chars.length;
  }

  private isDigit(char: string): boolean {
    return char >= '0' && char <= '9';
  }

  private isAlpha(char: string): boolean {
    return (char >= 'a' && char <= 'z') ||
           (char >= 'A' && char <= 'Z');
  }

  private isUpperCase(char: string): boolean {
    return char >= 'A' && char <= 'Z';
  }

  private isAlphaNumeric(char: string): boolean {
    return this.isAlpha(char) || this.isDigit(char);
  }

  private currentLocation(): SourceLocation {
    return {
      line: this.line,
      column: this.column,
      offset: this.pos
    };
  }

  private addToken(type: TokenType, value: string, start: SourceLocation): void {
    const end = this.currentLocation();
    this.tokens.push(createToken(type, value, start, end));
  }

  private error(message: string): never {
    throw new LexerError(message, this.currentLocation());
  }
}

/**
 * Convenience function to tokenize source code
 */
export function tokenize(source: string): Token[] {
  const lexer = new Lexer(source);
  return lexer.tokenize();
}
