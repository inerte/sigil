/**
 * Sigil Semantic Map Generator
 *
 * Generates basic semantic mappings from AST nodes
 */

import * as AST from '../parser/ast.js';
import { InferenceType } from '../typechecker/types.js';
import { Mapping, MappableNode } from './types.js';

/**
 * Generate basic mapping for a node
 */
export function generateBasicMapping(
  node: MappableNode,
  _source: string  // Reserved for future use (extracting code snippets)
): Mapping {
  let summary: string;
  let explanation: string;
  let type: string | undefined;

  switch (node.nodeType) {
    case 'function': {
      const func = node.ast as AST.FunctionDecl;
      summary = `Function: ${func.name}`;
      explanation = `Function with ${func.params.length} parameter(s)`;

      if (node.inferredType) {
        type = formatType(node.inferredType);
        const returnType = extractReturnType(node.inferredType);
        explanation += `, returns ${returnType}`;
      }
      break;
    }

    case 'match_arm': {
      const arm = node.ast as AST.MatchArm;
      const pattern = formatPattern(arm.pattern);
      summary = `Match arm: ${pattern}`;
      explanation = `Pattern matches ${pattern}`;
      break;
    }

    case 'type': {
      const typeDecl = node.ast as AST.TypeDecl;
      summary = `Type definition: ${typeDecl.name}`;
      explanation = `Type ${typeDecl.name}`;
      break;
    }

    default:
      summary = 'Unknown';
      explanation = 'Unknown node type';
  }

  return {
    range: node.range,
    summary,
    explanation,
    type
  };
}

/**
 * Format type as Sigil syntax
 */
function formatType(type: InferenceType): string {
  switch (type.kind) {
    case 'primitive':
      switch (type.name) {
        case 'Int': return 'ℤ';
        case 'Float': return 'ℝ';
        case 'Bool': return '𝔹';
        case 'String': return '𝕊';
        case 'Char': return 'ℂ';
        case 'Unit': return '𝕌';
      }
      break;
    case 'function': {
      const params = type.params.map(p => formatType(p)).join(',');
      const ret = formatType(type.returnType);
      return `λ(${params})→${ret}`;
    }
    case 'list':
      return `[${formatType(type.elementType)}]`;
    case 'tuple':
      return `(${type.types.map(t => formatType(t)).join(',')})`;
    case 'var':
      return type.name || `T${type.id}`;
    case 'record':
      return type.name || 'Record';
    case 'constructor':
      return type.name;
    default:
      return 'unknown';
  }
}

/**
 * Extract return type from function type
 */
function extractReturnType(type: InferenceType): string {
  if (type.kind === 'function') {
    return formatType(type.returnType);
  }
  return 'unknown';
}

/**
 * Format pattern for display
 */
function formatPattern(pattern: AST.Pattern): string {
  switch (pattern.type) {
    case 'LiteralPattern':
      return String(pattern.value);
    case 'IdentifierPattern':
      return pattern.name;
    case 'WildcardPattern':
      return '_';
    case 'ListPattern':
      if (pattern.patterns.length === 0) {
        return '[]';
      }
      return `[${pattern.patterns.map(formatPattern).join(',')}]`;
    case 'TuplePattern':
      return `(${pattern.patterns.map(formatPattern).join(',')})`;
    case 'ConstructorPattern':
      if (pattern.patterns.length > 0) {
        return `${pattern.name}(${pattern.patterns.map(formatPattern).join(',')})`;
      }
      return pattern.name;
    default:
      return '?';
  }
}
