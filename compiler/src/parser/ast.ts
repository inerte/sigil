/**
 * Mint Programming Language - Abstract Syntax Tree (AST) Definitions
 *
 * This file defines the AST node types that represent parsed Mint programs.
 * The AST is the intermediate representation between lexing and type checking.
 */

// ============================================================================
// SOURCE LOCATION
// ============================================================================

export interface SourceLocation {
  start: { line: number; column: number; offset: number };
  end: { line: number; column: number; offset: number };
}

// Helper to convert Token location to AST SourceLocation
export function tokenToLocation(start: { line: number; column: number; offset: number }, end: { line: number; column: number; offset: number }): SourceLocation {
  return { start, end };
}

// ============================================================================
// PROGRAM
// ============================================================================

export interface Program {
  type: 'Program';
  declarations: Declaration[];
  location: SourceLocation;
}

// ============================================================================
// DECLARATIONS
// ============================================================================

export type Declaration =
  | FunctionDecl
  | TypeDecl
  | ImportDecl
  | ConstDecl
  | TestDecl;

export interface FunctionDecl {
  type: 'FunctionDecl';
  name: string;
  params: Param[];
  returnType: Type | null;
  body: Expr;
  location: SourceLocation;
}

export interface Param {
  name: string;
  typeAnnotation: Type | null;
  location: SourceLocation;
}

export interface TypeDecl {
  type: 'TypeDecl';
  name: string;
  typeParams: string[];
  definition: TypeDef;
  location: SourceLocation;
}

export type TypeDef = SumType | ProductType | TypeAlias;

export interface SumType {
  type: 'SumType';
  variants: Variant[];
  location: SourceLocation;
}

export interface Variant {
  name: string;
  types: Type[];
  location: SourceLocation;
}

export interface ProductType {
  type: 'ProductType';
  fields: Field[];
  location: SourceLocation;
}

export interface Field {
  name: string;
  fieldType: Type;
  location: SourceLocation;
}

export interface TypeAlias {
  type: 'TypeAlias';
  aliasedType: Type;
  location: SourceLocation;
}

export interface ImportDecl {
  type: 'ImportDecl';
  modulePath: string[];
  imports: string[] | null; // null = import all
  location: SourceLocation;
}

export interface ConstDecl {
  type: 'ConstDecl';
  name: string;
  typeAnnotation: Type | null;
  value: Expr;
  location: SourceLocation;
}

export interface TestDecl {
  type: 'TestDecl';
  description: string;
  body: Expr;
  location: SourceLocation;
}

// ============================================================================
// TYPES
// ============================================================================

export type Type =
  | PrimitiveType
  | ListType
  | MapType
  | FunctionType
  | TypeConstructor
  | TypeVariable
  | TupleType;

export interface PrimitiveType {
  type: 'PrimitiveType';
  name: 'Int' | 'Float' | 'Bool' | 'String' | 'Char' | 'Unit';
  location: SourceLocation;
}

export interface ListType {
  type: 'ListType';
  elementType: Type;
  location: SourceLocation;
}

export interface MapType {
  type: 'MapType';
  keyType: Type;
  valueType: Type;
  location: SourceLocation;
}

export interface FunctionType {
  type: 'FunctionType';
  paramTypes: Type[];
  returnType: Type;
  location: SourceLocation;
}

export interface TypeConstructor {
  type: 'TypeConstructor';
  name: string;
  typeArgs: Type[];
  location: SourceLocation;
}

export interface TypeVariable {
  type: 'TypeVariable';
  name: string;
  location: SourceLocation;
}

export interface TupleType {
  type: 'TupleType';
  types: Type[];
  location: SourceLocation;
}

// ============================================================================
// EXPRESSIONS
// ============================================================================

export type Expr =
  | LiteralExpr
  | IdentifierExpr
  | LambdaExpr
  | ApplicationExpr
  | BinaryExpr
  | UnaryExpr
  | MatchExpr
  | LetExpr
  | IfExpr
  | ListExpr
  | RecordExpr
  | TupleExpr
  | FieldAccessExpr
  | IndexExpr
  | PipelineExpr;

export interface LiteralExpr {
  type: 'LiteralExpr';
  value: number | string | boolean | null; // null for Unit
  literalType: 'Int' | 'Float' | 'String' | 'Char' | 'Bool' | 'Unit';
  location: SourceLocation;
}

export interface IdentifierExpr {
  type: 'IdentifierExpr';
  name: string;
  location: SourceLocation;
}

export interface LambdaExpr {
  type: 'LambdaExpr';
  params: Param[];
  body: Expr;
  location: SourceLocation;
}

export interface ApplicationExpr {
  type: 'ApplicationExpr';
  func: Expr;
  args: Expr[];
  location: SourceLocation;
}

export interface BinaryExpr {
  type: 'BinaryExpr';
  left: Expr;
  operator: BinaryOperator;
  right: Expr;
  location: SourceLocation;
}

export type BinaryOperator =
  // Arithmetic
  | '+' | '-' | '*' | '/' | '%' | '^'
  // Comparison
  | '=' | '≠' | '<' | '>' | '≤' | '≥'
  // Logical
  | '∧' | '∨'
  // Pipeline
  | '|>' | '>>' | '<<'
  // Concatenation
  | '++';

export interface UnaryExpr {
  type: 'UnaryExpr';
  operator: UnaryOperator;
  operand: Expr;
  location: SourceLocation;
}

export type UnaryOperator = '-' | '¬';

export interface MatchExpr {
  type: 'MatchExpr';
  scrutinee: Expr;
  arms: MatchArm[];
  location: SourceLocation;
}

export interface MatchArm {
  pattern: Pattern;
  body: Expr;
  location: SourceLocation;
}

export interface LetExpr {
  type: 'LetExpr';
  pattern: Pattern;
  value: Expr;
  body: Expr;
  location: SourceLocation;
}

export interface IfExpr {
  type: 'IfExpr';
  condition: Expr;
  thenBranch: Expr;
  elseBranch: Expr | null;
  location: SourceLocation;
}

export interface ListExpr {
  type: 'ListExpr';
  elements: Expr[];
  location: SourceLocation;
}

export interface RecordExpr {
  type: 'RecordExpr';
  fields: RecordField[];
  location: SourceLocation;
}

export interface RecordField {
  name: string;
  value: Expr;
  location: SourceLocation;
}

export interface TupleExpr {
  type: 'TupleExpr';
  elements: Expr[];
  location: SourceLocation;
}

export interface FieldAccessExpr {
  type: 'FieldAccessExpr';
  object: Expr;
  field: string;
  location: SourceLocation;
}

export interface IndexExpr {
  type: 'IndexExpr';
  object: Expr;
  index: Expr;
  location: SourceLocation;
}

export interface PipelineExpr {
  type: 'PipelineExpr';
  left: Expr;
  operator: '|>' | '>>' | '<<';
  right: Expr;
  location: SourceLocation;
}

// ============================================================================
// PATTERNS
// ============================================================================

export type Pattern =
  | LiteralPattern
  | IdentifierPattern
  | WildcardPattern
  | ConstructorPattern
  | ListPattern
  | RecordPattern
  | TuplePattern;

export interface LiteralPattern {
  type: 'LiteralPattern';
  value: number | string | boolean | null;
  literalType: 'Int' | 'Float' | 'String' | 'Char' | 'Bool' | 'Unit';
  location: SourceLocation;
}

export interface IdentifierPattern {
  type: 'IdentifierPattern';
  name: string;
  location: SourceLocation;
}

export interface WildcardPattern {
  type: 'WildcardPattern';
  location: SourceLocation;
}

export interface ConstructorPattern {
  type: 'ConstructorPattern';
  name: string;
  patterns: Pattern[];
  location: SourceLocation;
}

export interface ListPattern {
  type: 'ListPattern';
  patterns: Pattern[];
  rest: string | null; // For [x,.xs] pattern, rest = "xs"
  location: SourceLocation;
}

export interface RecordPattern {
  type: 'RecordPattern';
  fields: RecordPatternField[];
  location: SourceLocation;
}

export interface RecordPatternField {
  name: string;
  pattern: Pattern | null; // null means just bind the field name
  location: SourceLocation;
}

export interface TuplePattern {
  type: 'TuplePattern';
  patterns: Pattern[];
  location: SourceLocation;
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/**
 * Create a source location from start and end positions
 */
export function createLocation(
  startLine: number,
  startColumn: number,
  startOffset: number,
  endLine: number,
  endColumn: number,
  endOffset: number
): SourceLocation {
  return {
    start: { line: startLine, column: startColumn, offset: startOffset },
    end: { line: endLine, column: endColumn, offset: endOffset },
  };
}

/**
 * Merge two source locations (from start of first to end of second)
 */
export function mergeLocations(start: SourceLocation, end: SourceLocation): SourceLocation {
  return {
    start: start.start,
    end: end.end,
  };
}

/**
 * Type guard for checking if a node is a specific type
 */
export function isNodeType<T extends { type: string }>(
  node: { type: string },
  type: T['type']
): node is T {
  return node.type === type;
}
