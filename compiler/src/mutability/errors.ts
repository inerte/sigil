/**
 * Mint Mutability Checker - Error Types
 *
 * Error classes for mutability violations
 */

import { SourceLocation } from '../parser/ast.js';

export class MutabilityError extends Error {
  constructor(
    message: string,
    public location: SourceLocation
  ) {
    super(message);
    this.name = 'MutabilityError';
  }

  /**
   * Format error with source context
   */
  format(source: string): string {
    const lines = source.split('\n');
    const line = lines[this.location.start.line - 1];

    if (!line) {
      return `Mutability Error: ${this.message}`;
    }

    const errorLine = `  ${this.location.start.line} | ${line}`;
    const pointer = ' '.repeat(this.location.start.line.toString().length + 3 + this.location.start.column) + '^';

    return `Mutability Error: ${this.message}\n\n${errorLine}\n${pointer}`;
  }
}
