/**
 * Completion provider for Mint Language Server
 *
 * Provides autocomplete for Unicode symbols used in Mint:
 * - λ (lambda) for functions
 * - → (arrow) for function returns
 * - ≡ (equivalent) for pattern matching
 * - ℤ ℝ 𝔹 𝕊 𝕌 (type symbols)
 * - ⊤ ⊥ (true/false)
 * - ↦ ⊳ ⊕ (map, filter, fold)
 */

import { TextDocuments } from 'vscode-languageserver/node.js';
import { TextDocument } from 'vscode-languageserver-textdocument';
import {
  CompletionItem,
  CompletionItemKind,
  CompletionParams,
} from 'vscode-languageserver/node.js';

/**
 * Unicode symbol completions
 */
const UNICODE_COMPLETIONS: Array<{
  triggers: string[];
  symbol: string;
  label: string;
  detail: string;
}> = [
  {
    triggers: ['lambda', 'lam', 'fn'],
    symbol: 'λ',
    label: 'λ (lambda)',
    detail: 'Lambda function symbol',
  },
  {
    triggers: ['arrow', '->', 'returns'],
    symbol: '→',
    label: '→ (arrow)',
    detail: 'Function return type arrow',
  },
  {
    triggers: ['match', 'equiv', '=='],
    symbol: '≡',
    label: '≡ (match)',
    detail: 'Pattern matching operator',
  },
  {
    triggers: ['int', 'integer'],
    symbol: 'ℤ',
    label: 'ℤ (Int)',
    detail: 'Integer type',
  },
  {
    triggers: ['real', 'float', 'double'],
    symbol: 'ℝ',
    label: 'ℝ (Real)',
    detail: 'Real number type',
  },
  {
    triggers: ['bool', 'boolean'],
    symbol: '𝔹',
    label: '𝔹 (Bool)',
    detail: 'Boolean type',
  },
  {
    triggers: ['string', 'str'],
    symbol: '𝕊',
    label: '𝕊 (String)',
    detail: 'String type',
  },
  {
    triggers: ['unit', 'void'],
    symbol: '𝕌',
    label: '𝕌 (Unit)',
    detail: 'Unit type (void)',
  },
  {
    triggers: ['true', 'top'],
    symbol: '⊤',
    label: '⊤ (true)',
    detail: 'Boolean true literal',
  },
  {
    triggers: ['false', 'bottom', 'bot'],
    symbol: '⊥',
    label: '⊥ (false)',
    detail: 'Boolean false literal',
  },
  {
    triggers: ['map', '|>'],
    symbol: '↦',
    label: '↦ (map)',
    detail: 'List map operation',
  },
  {
    triggers: ['filter', 'select'],
    symbol: '⊳',
    label: '⊳ (filter)',
    detail: 'List filter operation',
  },
  {
    triggers: ['fold', 'reduce'],
    symbol: '⊕',
    label: '⊕ (fold)',
    detail: 'List fold/reduce operation',
  },
  {
    triggers: ['in', 'element', 'elem'],
    symbol: '∈',
    label: '∈ (in)',
    detail: 'Element membership operator',
  },
  {
    triggers: ['empty', 'none', 'null'],
    symbol: '∅',
    label: '∅ (empty)',
    detail: 'Empty set / None value',
  },
];

/**
 * Handle completion requests
 */
export function onCompletion(
  params: CompletionParams,
  documents: TextDocuments<TextDocument>
): CompletionItem[] | null {
  const document = documents.get(params.textDocument.uri);
  if (!document) return null;

  const position = params.position;

  // Get text from start of line to cursor
  const lineText = document.getText({
    start: { line: position.line, character: 0 },
    end: position,
  });

  // Find matching Unicode completions
  const completions: CompletionItem[] = [];

  for (const completion of UNICODE_COMPLETIONS) {
    for (const trigger of completion.triggers) {
      if (lineText.endsWith(trigger)) {
        completions.push({
          label: completion.label,
          kind: CompletionItemKind.Text,
          insertText: completion.symbol,
          detail: completion.detail,
          // Remove the trigger text when accepting completion
          textEdit: {
            range: {
              start: {
                line: position.line,
                character: position.character - trigger.length,
              },
              end: position,
            },
            newText: completion.symbol,
          },
        });
      }
    }
  }

  return completions.length > 0 ? completions : null;
}
