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
export type { TypeInfo } from './environment.js';

export interface TypeCheckOptions {
  importedNamespaces?: Map<string, InferenceType>;
  importedTypeRegistries?: Map<string, Map<string, import('./environment.js').TypeInfo>>;
  sourceFile?: string;
}

/**
 * Type check a Sigil program
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
    if (error instanceof TypeError && options?.sourceFile && error.diagnostic.location?.file === '<unknown>') {
      error.diagnostic.location.file = options.sourceFile;
    }
    throw error;
  }
}
