/**
 * Hover provider for Mint Language Server
 *
 * Shows semantic map content when hovering over Mint code:
 * - Function explanations with complexity analysis
 * - Type information
 * - Warnings and examples
 * - AI-generated documentation
 */

import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

import { TextDocuments } from 'vscode-languageserver/node.js';
import { TextDocument } from 'vscode-languageserver-textdocument';
import {
  Hover,
  HoverParams,
  MarkupKind,
} from 'vscode-languageserver/node.js';

import { SemanticMap, Mapping } from './types.js';

/**
 * Handle hover requests
 */
export function onHover(
  params: HoverParams,
  documents: TextDocuments<TextDocument>
): Hover | null {
  const document = documents.get(params.textDocument.uri);
  if (!document) return null;

  // Load semantic map for this document
  const semanticMap = loadSemanticMap(document.uri);
  if (!semanticMap) return null;

  // Find mapping at cursor position
  const offset = document.offsetAt(params.position);
  const mapping = findMappingAtOffset(semanticMap, offset);
  if (!mapping) return null;

  // Format mapping as markdown
  const markdown = formatMappingAsMarkdown(mapping);

  return {
    contents: {
      kind: MarkupKind.Markdown,
      value: markdown,
    },
  };
}

/**
 * Load semantic map file for a Mint document
 */
function loadSemanticMap(documentUri: string): SemanticMap | null {
  try {
    // Convert URI to file path
    const filePath = fileURLToPath(documentUri);

    // Replace .mint with .mint.map
    const mapPath = filePath.replace(/\.mint$/, '.mint.map');

    // Read and parse semantic map
    const mapContent = readFileSync(mapPath, 'utf-8');
    return JSON.parse(mapContent) as SemanticMap;

  } catch (error) {
    // Semantic map doesn't exist or is invalid
    return null;
  }
}

/**
 * Find mapping at a specific offset in the source file
 */
function findMappingAtOffset(
  semanticMap: SemanticMap,
  offset: number
): Mapping | null {
  // Search all mappings for one containing this offset
  for (const [id, mapping] of Object.entries(semanticMap.mappings)) {
    const [start, end] = mapping.range;
    if (offset >= start && offset <= end) {
      return mapping;
    }
  }

  return null;
}

/**
 * Format a mapping as markdown for display
 */
function formatMappingAsMarkdown(mapping: Mapping): string {
  let md = '';

  // Summary (bold header)
  md += `**${mapping.summary}**\n\n`;

  // Explanation
  md += `${mapping.explanation}\n\n`;

  // Type signature
  if (mapping.type) {
    md += `**Type:** \`${mapping.type}\`\n\n`;
  }

  // Complexity analysis
  if (mapping.complexity) {
    md += `**Complexity:** ${mapping.complexity}\n\n`;
  }

  // Warnings
  if (mapping.warnings && mapping.warnings.length > 0) {
    md += `**⚠️ Warnings:**\n`;
    for (const warning of mapping.warnings) {
      md += `- ${warning}\n`;
    }
    md += '\n';
  }

  // Examples
  if (mapping.examples && mapping.examples.length > 0) {
    md += `**Examples:**\n\`\`\`mint\n`;
    md += mapping.examples.join('\n');
    md += '\n\`\`\`\n\n';
  }

  // Related references
  if (mapping.related && mapping.related.length > 0) {
    md += `**Related:** ${mapping.related.join(', ')}\n`;
  }

  return md.trim();
}
