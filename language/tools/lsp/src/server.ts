/**
 * Mint Language Server
 *
 * Provides LSP features for the Mint programming language:
 * - Real-time diagnostics (syntax, type, canonical form errors)
 * - Hover tooltips with semantic map content
 * - Unicode symbol completion
 * - Document symbols (outline view)
 */

import {
  createConnection,
  TextDocuments,
  ProposedFeatures,
  InitializeParams,
  TextDocumentSyncKind,
  InitializeResult,
  ServerCapabilities,
} from 'vscode-languageserver/node.js';

import { TextDocument } from 'vscode-languageserver-textdocument';

import { validateDocument } from './diagnostics.js';
import { onHover } from './hover.js';
import { onCompletion } from './completion.js';
import { onDocumentSymbol } from './symbols.js';

// Create LSP connection using stdio
const connection = createConnection(ProposedFeatures.all);

// Document manager - tracks open .mint files
const documents = new TextDocuments(TextDocument);

// Server initialization
connection.onInitialize((params: InitializeParams): InitializeResult => {
  const capabilities: ServerCapabilities = {
    // Incremental document sync
    textDocumentSync: TextDocumentSyncKind.Incremental,

    // Hover provider (show semantic maps)
    hoverProvider: true,

    // Completion provider (Unicode symbols)
    completionProvider: {
      triggerCharacters: ['l', 'a', 'r', 't', 'f', 'i', 'm', 'e', 'b'],
    },

    // Document symbols (outline view)
    documentSymbolProvider: true,
  };

  return {
    capabilities,
    serverInfo: {
      name: 'Mint Language Server',
      version: '0.1.0',
    },
  };
});

connection.onInitialized(() => {
  connection.console.log('Mint Language Server initialized');
});

// Validate document when content changes
documents.onDidChangeContent(change => {
  validateDocument(change.document, connection);
});

// Validate document when first opened
documents.onDidOpen(event => {
  validateDocument(event.document, connection);
});

// Register LSP feature handlers
connection.onHover(params => onHover(params, documents));
connection.onCompletion(params => onCompletion(params, documents));
connection.onDocumentSymbol(params => onDocumentSymbol(params, documents));

// Start listening for LSP messages
documents.listen(connection);
connection.listen();

connection.console.log('Mint Language Server started');
