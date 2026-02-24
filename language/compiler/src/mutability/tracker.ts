/**
 * Sigil Mutability Checker - Tracking
 *
 * Tracks mutability state and prevents invalid mutations
 */

import * as AST from '../parser/ast.js';
import { MutabilityError } from './errors.js';

/**
 * Tracks mutability state of variables in current scope
 */
interface MutabilityContext {
  // Variables that are immutable
  immutable: Set<string>;

  // Variables that are mutable
  mutable: Set<string>;

  // Parent scope (for nested contexts)
  parent?: MutabilityContext;
}

/**
 * Create initial mutability context
 */
function createInitialContext(): MutabilityContext {
  return {
    immutable: new Set(),
    mutable: new Set()
  };
}

/**
 * Extend context with new scope
 */
function extendContext(
  parent: MutabilityContext,
  bindings: Map<string, boolean>  // name -> isMutable
): MutabilityContext {
  const ctx: MutabilityContext = {
    immutable: new Set(),
    mutable: new Set(),
    parent
  };

  for (const [name, isMutable] of bindings) {
    if (isMutable) {
      ctx.mutable.add(name);
    } else {
      ctx.immutable.add(name);
    }
  }

  return ctx;
}

/**
 * Check if variable is mutable
 */
function isMutable(ctx: MutabilityContext, name: string): boolean {
  if (ctx.mutable.has(name)) return true;
  if (ctx.immutable.has(name)) return false;
  if (ctx.parent) return isMutable(ctx.parent, name);
  return false;  // Unknown variables treated as immutable
}

/**
 * Get pattern binding name (simple version - just handles identifiers)
 */
function getPatternName(pattern: AST.Pattern): string | null {
  if (pattern.type === 'IdentifierPattern') {
    return pattern.name;
  }
  // For complex patterns, we'd need to extract all bindings
  // For now, just return null
  return null;
}

/**
 * Check mutability constraints in expression
 */
function checkMutability(expr: AST.Expr, ctx: MutabilityContext): void {
  switch (expr.type) {
    case 'LiteralExpr':
    case 'IdentifierExpr':
      // Reading is always OK
      break;

    case 'ApplicationExpr':
      // Check arguments
      for (const arg of expr.args) {
        checkMutability(arg, ctx);
      }
      break;

    case 'BinaryExpr':
      checkMutability(expr.left, ctx);
      checkMutability(expr.right, ctx);
      break;

    case 'UnaryExpr':
      checkMutability(expr.operand, ctx);
      break;

    case 'MatchExpr':
      checkMutability(expr.scrutinee, ctx);
      for (const arm of expr.arms) {
        // Each arm creates its own scope with pattern bindings
        // For now, treat all pattern bindings as immutable
        checkMutability(arm.body, ctx);
      }
      break;

    case 'LetExpr':
      checkMutability(expr.value, ctx);

      // ERROR: Cannot create alias of mutable value
      if (expr.value.type === 'IdentifierExpr' && isMutable(ctx, expr.value.name)) {
        throw new MutabilityError(
          `Cannot create alias of mutable value '${expr.value.name}'`,
          expr.location
        );
      }

      // Add binding to context (let bindings are always immutable)
      const patternName = getPatternName(expr.pattern);
      if (patternName) {
        const newCtx = extendContext(ctx, new Map([[patternName, false]]));
        checkMutability(expr.body, newCtx);
      } else {
        // Complex pattern - just check body with same context
        checkMutability(expr.body, ctx);
      }
      break;

    case 'IfExpr':
      checkMutability(expr.condition, ctx);
      checkMutability(expr.thenBranch, ctx);
      if (expr.elseBranch) {
        checkMutability(expr.elseBranch, ctx);
      }
      break;

    case 'LambdaExpr':
      // Lambda parameters create new scope
      const paramBindings = new Map(
        expr.params.map(p => [p.name, p.isMutable])
      );
      const lambdaCtx = extendContext(ctx, paramBindings);
      checkMutability(expr.body, lambdaCtx);
      break;

    case 'ListExpr':
      for (const elem of expr.elements) {
        checkMutability(elem, ctx);
      }
      break;

    case 'TupleExpr':
      for (const elem of expr.elements) {
        checkMutability(elem, ctx);
      }
      break;

    case 'RecordExpr':
      for (const field of expr.fields) {
        checkMutability(field.value, ctx);
      }
      break;

    case 'FieldAccessExpr':
      checkMutability(expr.object, ctx);
      break;

    case 'MapExpr':
      checkMutability(expr.list, ctx);
      checkMutability(expr.fn, ctx);
      break;

    case 'FilterExpr':
      checkMutability(expr.list, ctx);
      checkMutability(expr.predicate, ctx);
      break;

    case 'FoldExpr':
      checkMutability(expr.list, ctx);
      checkMutability(expr.fn, ctx);
      checkMutability(expr.init, ctx);
      break;

    default:
      // For any expression types we don't handle, just skip
      break;
  }
}

/**
 * Check function declaration
 */
function checkFunctionDecl(decl: AST.FunctionDecl): void {
  // Create context with function parameters
  const paramBindings = new Map(
    decl.params.map(p => [p.name, p.isMutable])
  );

  const ctx = extendContext(createInitialContext(), paramBindings);

  // Check function body
  checkMutability(decl.body, ctx);
}

/**
 * Check mutability for entire program
 */
export function checkProgramMutability(program: AST.Program): void {
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      checkFunctionDecl(decl);
    }
    // Other declaration types don't have bodies to check
  }
}
