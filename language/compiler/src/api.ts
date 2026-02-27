/**
 * Sigil Compiler API
 *
 * Public API for programmatic compilation of Sigil code.
 * Primarily used for testing invalid code patterns without creating .sigil files.
 */

import { tokenize } from './lexer/lexer.js';
import { parse } from './parser/parser.js';
import { validateCanonicalForm } from './validator/canonical.js';
import { validateSurfaceForm } from './validator/surface-form.js';
import { typeCheck } from './typechecker/index.js';
import { compile } from './codegen/javascript.js';
import { isSigilDiagnosticError } from './diagnostics/error.js';
import type { Diagnostic } from './diagnostics/types.js';
import { diagnostic } from './diagnostics/helpers.js';

/**
 * Result of a compilation attempt
 */
export type CompilationResult =
  | { ok: true; output: string; phase: 'codegen' }
  | { ok: false; error: Diagnostic; phase: Diagnostic['phase'] };

/**
 * Compile Sigil source code from a string
 *
 * Runs the full compilation pipeline:
 * 1. Surface form validation (formatting)
 * 2. Lexical analysis (tokenization)
 * 3. Parsing (AST construction)
 * 4. Canonical form validation
 * 5. Type checking
 * 6. Code generation
 *
 * @param code - Sigil source code to compile
 * @param filename - Optional filename for error messages (default: 'test.sigil')
 * @returns Compilation result with generated TypeScript code or error diagnostic
 *
 * @example
 * ```typescript
 * import { compileFromString } from '@sigil-lang/compiler';
 *
 * // Valid code compiles successfully
 * const result = compileFromString('λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}');
 * if (result.ok) {
 *   console.log(result.output); // Generated TypeScript
 * }
 *
 * // Invalid code returns diagnostic error
 * const invalid = compileFromString('λf(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→f(n-1,n*acc)}');
 * if (!invalid.ok) {
 *   console.log(invalid.error.code); // SIGIL-CANON-RECURSION-ACCUMULATOR
 * }
 * ```
 */
export function compileFromString(
  code: string,
  filename: string = 'test.sigil'
): CompilationResult {
  try {
    // Phase 1: Surface form validation
    validateSurfaceForm(code, filename);

    // Phase 2: Lexical analysis
    const tokens = tokenize(code);

    // Phase 3: Parsing
    const ast = parse(tokens, filename);

    // Phase 4: Canonical form validation
    validateCanonicalForm(ast, filename);

    // Phase 5: Type checking
    typeCheck(ast, code, {
      importedNamespaces: new Map(),
      importedTypeRegistries: new Map(),
      sourceFile: filename,
    });

    // Phase 6: Code generation
    const output = compile(ast, {
      sourceFile: filename,
      outputFile: filename.replace(/\.sigil$/, '.ts'),
      projectRoot: undefined,
    });

    return {
      ok: true,
      output,
      phase: 'codegen',
    };
  } catch (error) {
    // Convert error to diagnostic
    if (isSigilDiagnosticError(error)) {
      const diag = { ...error.diagnostic };
      if (diag.location && diag.location.file === '<unknown>') {
        diag.location = { ...diag.location, file: filename };
      }
      return {
        ok: false,
        error: diag,
        phase: diag.phase,
      };
    }

    if (error instanceof Error) {
      return {
        ok: false,
        error: diagnostic('SIGIL-CLI-UNEXPECTED', 'cli', error.message),
        phase: 'cli',
      };
    }

    return {
      ok: false,
      error: diagnostic('SIGIL-CLI-UNEXPECTED', 'cli', String(error)),
      phase: 'cli',
    };
  }
}
