/**
 * Sigil Type Checker - Public API (Bidirectional)
 *
 * Main entry point for type checking Sigil programs
 */

import * as AST from '../parser/ast.js';
import { typeCheck as bidirectionalTypeCheck } from './bidirectional.js';
import { TypeError } from './errors.js';
import { InferenceType } from './types.js';

// Re-export types
export { TypeError } from './errors.js';
export type { InferenceType } from './types.js';

export interface TypeCheckOptions {
  importedNamespaces?: Map<string, InferenceType>;
}

/**
 * Type check a Mint program
 *
 * Returns a map of function names to their inferred types
 * Throws TypeError if type checking fails
 */
export function typeCheck(
  program: AST.Program,
  sourceCode?: string,
  options?: TypeCheckOptions
): Map<string, InferenceType> {
  try {
    return bidirectionalTypeCheck(program, sourceCode || '', options);
  } catch (error) {
    if (error instanceof TypeError && sourceCode) {
      // Format error with source context
      console.error(error.format(sourceCode));
    }
    throw error;
  }
}
