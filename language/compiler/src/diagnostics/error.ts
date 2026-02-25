import type { Diagnostic } from './types.js';

export class SigilDiagnosticError extends Error {
  constructor(public readonly diagnostic: Diagnostic) {
    super(diagnostic.message);
    this.name = 'SigilDiagnosticError';
  }
}

export function isSigilDiagnosticError(value: unknown): value is SigilDiagnosticError {
  return value instanceof SigilDiagnosticError;
}
