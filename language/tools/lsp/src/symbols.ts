/**
 * Document symbols provider for Mint Language Server
 *
 * Provides outline view of Mint source files:
 * - Functions with type signatures
 * - Type declarations
 * - Hierarchical structure
 */

import { TextDocuments } from 'vscode-languageserver/node.js';
import { TextDocument } from 'vscode-languageserver-textdocument';
import {
  DocumentSymbol,
  DocumentSymbolParams,
  SymbolKind,
} from 'vscode-languageserver/node.js';

/**
 * Handle document symbol requests
 */
export async function onDocumentSymbol(
  params: DocumentSymbolParams,
  documents: TextDocuments<TextDocument>
): Promise<DocumentSymbol[] | null> {
  const document = documents.get(params.textDocument.uri);
  if (!document) return null;

  const source = document.getText();

  try {
    // Dynamically import compiler modules
    const { tokenize } = await import('../../../compiler/dist/lexer/lexer.js');
    const { parse } = await import('../../../compiler/dist/parser/parser.js');

    // Parse the document
    const tokens = tokenize(source);
    const ast = parse(tokens);

    // Extract symbols from AST
    const symbols: DocumentSymbol[] = [];

    for (const decl of ast.declarations) {
      if (decl.type === 'FunctionDecl') {
        // Function symbol
        const funcSymbol: DocumentSymbol = {
          name: decl.name,
          kind: SymbolKind.Function,
          range: locationToRange(decl.location, document),
          selectionRange: locationToRange(decl.location, document),
          detail: formatFunctionSignature(decl),
        };

        symbols.push(funcSymbol);

      } else if (decl.type === 'TypeDecl') {
        // Type declaration symbol
        const typeSymbol: DocumentSymbol = {
          name: decl.name,
          kind: SymbolKind.Class,
          range: locationToRange(decl.location, document),
          selectionRange: locationToRange(decl.location, document),
          detail: 'Type definition',
        };

        symbols.push(typeSymbol);
      }
    }

    return symbols;

  } catch (error) {
    // Parse error - return empty symbols
    return [];
  }
}

/**
 * Convert AST location to LSP range
 */
function locationToRange(
  location: any,
  document: TextDocument
): { start: any; end: any } {
  return {
    start: document.positionAt(location.start.offset),
    end: document.positionAt(location.end.offset),
  };
}

/**
 * Format function signature for display
 */
function formatFunctionSignature(funcDecl: any): string {
  const params = funcDecl.params
    .map((p: any) => `${p.name}:${formatType(p.typeAnnotation)}`)
    .join(', ');

  const returnType = formatType(funcDecl.returnType);

  return `λ(${params})→${returnType}`;
}

/**
 * Format type annotation as Mint syntax
 */
function formatType(type: any): string {
  if (!type) return '?';

  switch (type.type) {
    case 'IntType':
      return 'ℤ';
    case 'StringType':
      return '𝕊';
    case 'BoolType':
      return '𝔹';
    case 'UnitType':
      return '𝕌';
    case 'RealType':
      return 'ℝ';
    case 'ListType':
      return `[${formatType(type.elementType)}]`;
    case 'TupleType':
      return `(${type.elements.map(formatType).join(',')})`;
    case 'FunctionType':
      const params = type.params.map(formatType).join(',');
      return `λ(${params})→${formatType(type.returnType)}`;
    case 'TypeReference':
      return type.name;
    default:
      return '?';
  }
}
