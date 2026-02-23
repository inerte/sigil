/**
 * Diagnostics provider for Mint Language Server
 *
 * Integrates with the Mint compiler to provide real-time error checking:
 * - Lexer errors (invalid tokens, Unicode issues)
 * - Parser errors (syntax errors, malformed AST)
 * - Type checker errors (type mismatches, undefined functions)
 * - Canonical form violations (accumulator parameters, helper functions, etc.)
 */

import { TextDocument } from 'vscode-languageserver-textdocument';
import {
  Connection,
  Diagnostic,
  DiagnosticSeverity,
} from 'vscode-languageserver/node.js';

import { MintError } from './types.js';

/**
 * Validate a Mint document and send diagnostics to the client
 */
export async function validateDocument(
  document: TextDocument,
  connection: Connection
): Promise<void> {
  const diagnostics: Diagnostic[] = [];
  const source = document.getText();

  try {
    // Dynamically import compiler modules
    const { tokenize } = await import('../../../compiler/dist/lexer/lexer.js');
    const { parse } = await import('../../../compiler/dist/parser/parser.js');
    const { typeCheck } = await import('../../../compiler/dist/typechecker/index.js');
    const { checkProgramMutability } = await import('../../../compiler/dist/mutability/index.js');

    try {
      // 1. Lex the source code
      const tokens = tokenize(source);

      // 2. Parse into AST
      const ast = parse(tokens);

      // 3. Type check
      const typeMap = typeCheck(ast, source);

      // 4. Check mutability rules
      checkProgramMutability(ast);

      // Success - no errors

    } catch (error: any) {
      // Convert compiler error to LSP diagnostic
      const diagnostic = errorToDiagnostic(error, document);
      if (diagnostic) {
        diagnostics.push(diagnostic);
      }
    }

  } catch (importError: any) {
    // Compiler not available - log warning but don't fail
    connection.console.warn(
      `Failed to load Mint compiler: ${importError.message}`
    );
  }

  // Send diagnostics to client
  connection.sendDiagnostics({
    uri: document.uri,
    diagnostics,
  });
}

/**
 * Convert a Mint compiler error to an LSP diagnostic
 */
function errorToDiagnostic(
  error: MintError,
  document: TextDocument
): Diagnostic | null {
  // Extract error location
  const location = error.location;

  let range;
  if (location) {
    // Use precise location from error
    range = {
      start: document.positionAt(location.start.offset),
      end: document.positionAt(location.end.offset),
    };
  } else {
    // Fallback: highlight first line if no location
    range = {
      start: { line: 0, character: 0 },
      end: { line: 0, character: Number.MAX_VALUE },
    };
  }

  return {
    severity: DiagnosticSeverity.Error,
    range,
    message: error.message || 'Unknown error',
    source: 'mint',
  };
}
