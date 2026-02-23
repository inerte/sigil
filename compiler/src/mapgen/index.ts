/**
 * Mint Semantic Map Generator
 *
 * Main entry point for semantic map generation
 */

import * as path from 'path';
import * as AST from '../parser/ast.js';
import { InferenceType } from '../typechecker/types.js';
import { SemanticMap } from './types.js';
import { extractMappableNodes } from './extractor.js';
import { generateBasicMapping } from './generator.js';
import { writeSemanticMap } from './writer.js';

/**
 * Generate semantic map from AST and type information
 */
export function generateSemanticMap(
  program: AST.Program,
  typeMap: Map<string, InferenceType>,
  source: string,
  outputFile: string
): void {
  // 1. Extract mappable nodes from AST
  const nodes = extractMappableNodes(program, typeMap);

  // 2. Generate basic mappings for each node
  const mappings: Record<string, any> = {};
  for (const node of nodes) {
    mappings[node.id] = generateBasicMapping(node, source);
  }

  // 3. Build complete semantic map
  const map: SemanticMap = {
    version: 1,
    file: path.basename(outputFile.replace('.mint.map', '.mint')),
    generated_by: 'mintc@0.1.0',
    generated_at: new Date().toISOString(),
    mappings,
    metadata: {
      category: 'unknown',
      tested: false
    }
  };

  // 4. Write to file
  writeSemanticMap(map, outputFile);
}

export { enhanceWithClaude } from './enhance.js';
