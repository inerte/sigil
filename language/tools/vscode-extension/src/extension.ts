/**
 * VS Code Extension for Mint Language
 *
 * Activates the Mint Language Server when opening .mint files.
 * Provides syntax highlighting, diagnostics, hover, completion, and more.
 */

import * as path from 'path';
import { workspace, ExtensionContext, window } from 'vscode';
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

/**
 * Extension activation
 * Called when a .mint file is opened
 */
export function activate(context: ExtensionContext) {
  console.log('Mint language extension is now active');

  // Get LSP server path
  const serverModule = getServerPath(context);

  if (!serverModule) {
    window.showErrorMessage(
      'Mint Language Server not found. Please install or configure mint.lsp.path in settings.'
    );
    return;
  }

  // Server options - run the LSP server
  const serverOptions: ServerOptions = {
    run: {
      module: serverModule,
      transport: TransportKind.stdio,
      options: {
        env: process.env,
      },
    },
    debug: {
      module: serverModule,
      transport: TransportKind.stdio,
      options: {
        env: process.env,
        execArgv: ['--nolazy', '--inspect=6009'],
      },
    },
  };

  // Client options - configure language client
  const clientOptions: LanguageClientOptions = {
    // Register for .mint files
    documentSelector: [
      {
        scheme: 'file',
        language: 'mint',
      },
    ],

    // Synchronize file events
    synchronize: {
      // Watch .mint and .mint.map files
      fileEvents: workspace.createFileSystemWatcher('**/*.{mint,mint.map}'),
    },

    // Output channel for debugging
    outputChannelName: 'Mint Language Server',
  };

  // Create and start the language client
  client = new LanguageClient(
    'mintLanguageServer',
    'Mint Language Server',
    serverOptions,
    clientOptions
  );

  // Start the client (this will also launch the server)
  client.start();

  console.log('Mint Language Server started');
}

/**
 * Extension deactivation
 * Clean up resources
 */
export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

/**
 * Get the path to the LSP server
 * Checks custom path first, then bundled server
 */
function getServerPath(context: ExtensionContext): string | null {
  // Check for custom path in settings
  const config = workspace.getConfiguration('mint');
  const customPath = config.get<string>('lsp.path');

  if (customPath) {
    return customPath;
  }

  // Use bundled server (relative to extension root)
  // In development: ../lsp/dist/server.js
  // In packaged extension: bundled in extension
  const bundledPath = context.asAbsolutePath(
    path.join('..', 'lsp', 'dist', 'server.js')
  );

  try {
    // Check if bundled server exists
    require.resolve(bundledPath);
    return bundledPath;
  } catch {
    // Server not found
    return null;
  }
}
