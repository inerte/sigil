/**
 * Sigil Mutability Checker - Error Types
 *
 * Error classes for mutability violations
 */

import { SourceLocation } from '../parser/ast.js';
import { SigilDiagnosticError } from '../diagnostics/error.js';
import { diagnostic } from '../diagnostics/helpers.js';

export class MutabilityError extends SigilDiagnosticError {
  constructor(
    message: string,
    public location: SourceLocation
  ) {
    super(diagnostic('SIGIL-MUTABILITY-INVALID', 'mutability', message, {
      location: {
        file: '<unknown>',
        start: { line: location.start.line, column: location.start.column, offset: location.start.offset },
        end: { line: location.end.line, column: location.end.column, offset: location.end.offset },
      }
    }));
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
