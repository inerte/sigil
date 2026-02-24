/**
 * Mint Semantic Map Extractor
 *
 * Extracts mappable nodes from AST
 */

import * as AST from '../parser/ast.js';
import { InferenceType } from '../typechecker/types.js';
import { MappableNode } from './types.js';

/**
 * Extract all mappable nodes from the program
 */
export function extractMappableNodes(
  program: AST.Program,
  typeMap: Map<string, InferenceType>
): MappableNode[] {
  const nodes: MappableNode[] = [];

  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      // Extract function itself
      nodes.push({
        id: decl.name,
        range: [decl.location.start.offset, decl.location.end.offset],
        nodeType: 'function',
        ast: decl,
        inferredType: typeMap.get(decl.name)
      });

      // Extract match arms if function body is a match expression
      if (decl.body.type === 'MatchExpr') {
        decl.body.arms.forEach((arm, idx) => {
          nodes.push({
            id: `${decl.name}_arm_${idx}`,
            range: [arm.location.start.offset, arm.location.end.offset],
            nodeType: 'match_arm',
            ast: arm
          });
        });
      }
    } else if (decl.type === 'TypeDecl') {
      // Extract type declaration
      nodes.push({
        id: decl.name,
        range: [decl.location.start.offset, decl.location.end.offset],
        nodeType: 'type',
        ast: decl
      });
    }
  }

  return nodes;
}
