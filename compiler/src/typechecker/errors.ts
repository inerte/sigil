/**
 * Mint Type Checker - Error Reporting
 *
 * Type error messages optimized for clarity (both for LLMs and humans)
 */

import * as AST from '../parser/ast.js';
import { InferenceType, TVar } from './types.js';

/**
 * Type error with source location information
 */
export class TypeError extends Error {
  constructor(
    message: string,
    public location?: AST.SourceLocation,
    public expected?: InferenceType,
    public actual?: InferenceType
  ) {
    super(message);
    this.name = 'TypeError';
  }

  /**
   * Format error message with source context
   */
  format(sourceCode?: string): string {
    let output = `Type Error: ${this.message}\n`;

    // Show source location context
    if (this.location && sourceCode) {
      const lines = sourceCode.split('\n');
      const line = lines[this.location.start.line - 1];

      if (line) {
        output += `\n`;
        output += `  ${this.location.start.line} | ${line}\n`;

        // Add caret pointing to error location
        const lineNumStr = String(this.location.start.line);
        const padding = ' '.repeat(lineNumStr.length + 3 + this.location.start.column);
        output += `  ${' '.repeat(lineNumStr.length)} | ${padding}^\n`;
      }
    }

    // Show expected vs actual types
    if (this.expected && this.actual) {
      output += `\n`;
      output += `Expected: ${formatType(this.expected)}\n`;
      output += `Actual:   ${formatType(this.actual)}\n`;
    }

    return output;
  }
}

/**
 * Format a type for display in error messages
 *
 * Uses Mint Unicode symbols (ℤ, 𝔹, 𝕊) for readability
 */
export function formatType(type: InferenceType): string {
  // Follow instances (dereferencing)
  type = prune(type);

  switch (type.kind) {
    case 'primitive': {
      // Use Mint Unicode symbols
      const nameMap: Record<string, string> = {
        'Int': 'ℤ',
        'Float': 'ℝ',
        'Bool': '𝔹',
        'String': '𝕊',
        'Char': 'ℂ',
        'Unit': '𝕌'
      };
      return nameMap[type.name] || type.name;
    }

    case 'var':
      // Use Greek letters for type variables
      return type.name || `α${type.id}`;

    case 'function': {
      const params = type.params.map(formatType).join(', ');
      const ret = formatType(type.returnType);

      // Use Mint syntax: λ(T1, T2) → R
      if (type.params.length === 0) {
        return `λ() → ${ret}`;
      }
      return `λ(${params}) → ${ret}`;
    }

    case 'list':
      return `[${formatType(type.elementType)}]`;

    case 'tuple': {
      if (type.types.length === 0) {
        return '()';
      }
      const types = type.types.map(formatType).join(', ');
      return `(${types})`;
    }

    case 'record': {
      if (type.name) {
        return type.name;
      }

      const fields = Array.from(type.fields.entries())
        .map(([name, t]) => `${name}: ${formatType(t)}`)
        .join(', ');

      return `{${fields}}`;
    }

    case 'constructor': {
      if (type.typeArgs.length === 0) {
        return type.name;
      }

      const args = type.typeArgs.map(formatType).join(', ');
      return `${type.name}[${args}]`;
    }

    case 'any':
      return 'any';
  }
}

/**
 * Prune type variables to get the actual type
 *
 * Follows instance chain until we reach a non-variable type
 */
function prune(type: InferenceType): InferenceType {
  if (type.kind === 'var' && type.instance) {
    // Follow the chain and update the instance for efficiency
    type.instance = prune(type.instance);
    return type.instance;
  }
  return type;
}

/**
 * Create a user-friendly type mismatch error
 */
export function typeMismatchError(
  message: string,
  expected: InferenceType,
  actual: InferenceType,
  location?: AST.SourceLocation
): TypeError {
  return new TypeError(message, location, expected, actual);
}

/**
 * Create an undefined variable error
 */
export function undefinedVariableError(
  varName: string,
  location?: AST.SourceLocation
): TypeError {
  return new TypeError(
    `Undefined variable: ${varName}`,
    location
  );
}

/**
 * Create a non-exhaustive pattern match error
 */
export function nonExhaustiveMatchError(
  missingPatterns: string[],
  location?: AST.SourceLocation
): TypeError {
  const patterns = missingPatterns.map(p => `  - ${p}`).join('\n');
  return new TypeError(
    `Non-exhaustive pattern match.\n\nMissing cases:\n${patterns}`,
    location
  );
}

/**
 * Create an occurs check failure error (infinite type)
 */
export function occursCheckError(
  typeVar: TVar,
  type: InferenceType,
  location?: AST.SourceLocation
): TypeError {
  return new TypeError(
    `Occurs check failed: ${formatType(typeVar)} occurs in ${formatType(type)}.\n` +
    `This would create an infinite type.`,
    location
  );
}

/**
 * Create a function arity mismatch error
 */
export function arityMismatchError(
  expected: number,
  actual: number,
  location?: AST.SourceLocation
): TypeError {
  return new TypeError(
    `Function arity mismatch: expected ${expected} argument${expected !== 1 ? 's' : ''}, ` +
    `but got ${actual}`,
    location
  );
}
