/**
 * Surface Form Validator
 *
 * Enforces canonical surface forms (formatting) for Sigil programs.
 *
 * Sigil's philosophy: **byte-for-byte reproducibility**.
 * Every program has exactly ONE valid textual representation.
 *
 * This ensures:
 * - Training data quality (no syntactic variations)
 * - Deterministic generation (LLMs generate exactly one form)
 * - Zero ambiguity (canonical forms extend to formatting)
 */

import { SigilDiagnosticError } from '../diagnostics/error.js';
import { diagnostic } from '../diagnostics/helpers.js';

export class SurfaceFormError extends SigilDiagnosticError {
  constructor(
    code: string,
    message: string,
    public readonly filename: string,
    public readonly line?: number,
    public readonly column?: number
  ) {
    super(diagnostic(code, 'surface', message, {
      location: line !== undefined ? {
        file: filename,
        start: { line, column: column ?? 0 },
      } : { file: filename, start: { line: 1, column: column ?? 0 } }
    }));
    this.name = 'SurfaceFormError';
  }
}

/**
 * Validates that source code follows canonical surface form rules.
 *
 * Enforced rules:
 * 1. File must end with a newline
 * 2. No trailing whitespace at line ends
 * 3. Maximum one consecutive blank line between declarations
 *
 * @param source - Source code to validate
 * @param filename - Filename for error reporting
 * @throws {SurfaceFormError} if validation fails
 */
export function validateSurfaceForm(source: string, filename: string): void {
  // Rule 1: File must end with newline
  if (source.length === 0) {
    // Empty file is technically valid (ends with implicit newline)
    return;
  }

  if (!source.endsWith('\n')) {
    throw new SurfaceFormError(
      'SIGIL-SURFACE-EOF-NEWLINE',
      'file must end with newline',
      filename,
      undefined,
      source.length
    );
  }

  // Split into lines for line-by-line validation
  const lines = source.split('\n');

  // Rule 2: No trailing whitespace
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Check for trailing spaces or tabs
    if (line.endsWith(' ') || line.endsWith('\t')) {
      throw new SurfaceFormError(
        'SIGIL-SURFACE-TRAILING-WHITESPACE',
        'trailing whitespace',
        filename,
        i + 1,
        line.length
      );
    }
  }

  // Rule 3: Maximum one consecutive blank line
  for (let i = 0; i < lines.length - 1; i++) {
    if (lines[i] === '' && lines[i + 1] === '') {
      throw new SurfaceFormError(
        'SIGIL-SURFACE-BLANK-LINES',
        'multiple consecutive blank lines',
        filename,
        i + 1,
        0
      );
    }
  }
}
