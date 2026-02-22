/**
 * Canonical Form Validator
 *
 * Enforces Mint's "ONE WAY" principle by making alternative patterns impossible:
 * 1. Recursive functions can only have ONE parameter (prevents accumulator pattern)
 * 2. No helper functions (functions only called by one other function)
 *
 * This ensures LLMs cannot generate multiple ways to solve the same problem.
 */

import * as AST from '../parser/ast.js';

export class CanonicalError extends Error {
  constructor(
    message: string,
    public location?: AST.SourceLocation
  ) {
    super(message);
    this.name = 'CanonicalError';
  }
}

/**
 * Validate that the program follows canonical form rules
 */
export function validateCanonicalForm(program: AST.Program): void {
  validateRecursiveFunctions(program);
  validateNoHelperFunctions(program);
}

/**
 * Rule 1: Recursive functions must have exactly ONE parameter
 *
 * This makes accumulator-style tail recursion impossible:
 * ❌ λfactorial(n:ℤ,acc:ℤ)→ℤ=... (2 parameters - rejected)
 * ✅ λfactorial(n:ℤ)→ℤ=...       (1 parameter - allowed)
 */
function validateRecursiveFunctions(program: AST.Program): void {
  for (const decl of program.declarations) {
    if (decl.type !== 'FunctionDecl') continue;

    // Check if function is recursive (calls itself)
    const isRecursive = containsRecursiveCall(decl.body, decl.name);

    if (isRecursive && decl.params.length > 1) {
      throw new CanonicalError(
        `Recursive function '${decl.name}' has ${decl.params.length} parameters.\n` +
        `Recursive functions must have exactly ONE parameter.\n` +
        `This prevents accumulator-style tail recursion.\n` +
        `\n` +
        `Example canonical form:\n` +
        `  λ${decl.name}(n:ℤ)→ℤ≡n{0→1|n→n*${decl.name}(n-1)}\n` +
        `\n` +
        `Mint enforces ONE way to write recursive functions.`,
        decl.location
      );
    }
  }
}

/**
 * Rule 2: No helper functions
 *
 * If a function is only called by one other function, it's a helper pattern.
 * This makes tail-recursion helpers impossible:
 * ❌ λhelper(n,acc)→... λfactorial(n)→helper(n,1)  (helper rejected)
 * ✅ λfactorial(n)→...                             (single function allowed)
 */
function validateNoHelperFunctions(program: AST.Program): void {
  const callGraph = buildCallGraph(program);

  for (const [funcName, callers] of callGraph.entries()) {
    // If function is only called by one other function → helper pattern
    if (callers.size === 1 && funcName !== 'main') {
      const caller = Array.from(callers)[0];
      throw new CanonicalError(
        `Function '${funcName}' is only called by '${caller}'.\n` +
        `Helper functions are not allowed.\n` +
        `\n` +
        `Options:\n` +
        `  1. Inline '${funcName}' into '${caller}'\n` +
        `  2. Export '${funcName}' and use it elsewhere\n` +
        `\n` +
        `Mint enforces ONE way: each function stands alone.`,
        getFunctionLocation(program, funcName)
      );
    }
  }
}

/**
 * Check if an expression contains a recursive call to the given function
 */
function containsRecursiveCall(expr: AST.Expr, functionName: string): boolean {
  switch (expr.type) {
    case 'ApplicationExpr':
      // Check if the function being called is itself
      if (expr.func.type === 'IdentifierExpr' && expr.func.name === functionName) {
        return true;
      }
      // Check function and arguments
      return containsRecursiveCall(expr.func, functionName) ||
        expr.args.some(arg => containsRecursiveCall(arg, functionName));

    case 'IdentifierExpr':
    case 'LiteralExpr':
      return false;

    case 'LambdaExpr':
      return containsRecursiveCall(expr.body, functionName);

    case 'BinaryExpr':
      return containsRecursiveCall(expr.left, functionName) ||
        containsRecursiveCall(expr.right, functionName);

    case 'UnaryExpr':
      return containsRecursiveCall(expr.operand, functionName);

    case 'MatchExpr':
      return containsRecursiveCall(expr.scrutinee, functionName) ||
        expr.arms.some(arm => containsRecursiveCall(arm.body, functionName));

    case 'LetExpr':
      return containsRecursiveCall(expr.value, functionName) ||
        containsRecursiveCall(expr.body, functionName);

    case 'IfExpr':
      return containsRecursiveCall(expr.condition, functionName) ||
        containsRecursiveCall(expr.thenBranch, functionName) ||
        (expr.elseBranch ? containsRecursiveCall(expr.elseBranch, functionName) : false);

    case 'ListExpr':
      return expr.elements.some(elem => containsRecursiveCall(elem, functionName));

    case 'RecordExpr':
      return expr.fields.some(field => containsRecursiveCall(field.value, functionName));

    case 'TupleExpr':
      return expr.elements.some(elem => containsRecursiveCall(elem, functionName));

    case 'FieldAccessExpr':
      return containsRecursiveCall(expr.object, functionName);

    case 'IndexExpr':
      return containsRecursiveCall(expr.object, functionName) ||
        containsRecursiveCall(expr.index, functionName);

    case 'PipelineExpr':
      return containsRecursiveCall(expr.left, functionName) ||
        containsRecursiveCall(expr.right, functionName);

    default:
      return false;
  }
}

/**
 * Build a call graph: Map<functionName, Set<callers>>
 *
 * For each function, track which other functions call it.
 */
function buildCallGraph(program: AST.Program): Map<string, Set<string>> {
  const callGraph = new Map<string, Set<string>>();

  // Initialize with all function names
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      callGraph.set(decl.name, new Set());
    }
  }

  // Track calls
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      const calledFunctions = findFunctionCalls(decl.body);
      for (const called of calledFunctions) {
        if (callGraph.has(called)) {
          callGraph.get(called)!.add(decl.name);
        }
      }
    }
  }

  return callGraph;
}

/**
 * Find all function names that are called in an expression
 */
function findFunctionCalls(expr: AST.Expr): Set<string> {
  const calls = new Set<string>();

  function visit(e: AST.Expr): void {
    switch (e.type) {
      case 'ApplicationExpr':
        if (e.func.type === 'IdentifierExpr') {
          calls.add(e.func.name);
        }
        visit(e.func);
        e.args.forEach(visit);
        break;

      case 'LambdaExpr':
        visit(e.body);
        break;

      case 'BinaryExpr':
        visit(e.left);
        visit(e.right);
        break;

      case 'UnaryExpr':
        visit(e.operand);
        break;

      case 'MatchExpr':
        visit(e.scrutinee);
        e.arms.forEach(arm => visit(arm.body));
        break;

      case 'LetExpr':
        visit(e.value);
        visit(e.body);
        break;

      case 'IfExpr':
        visit(e.condition);
        visit(e.thenBranch);
        if (e.elseBranch) visit(e.elseBranch);
        break;

      case 'ListExpr':
        e.elements.forEach(visit);
        break;

      case 'RecordExpr':
        e.fields.forEach(f => visit(f.value));
        break;

      case 'TupleExpr':
        e.elements.forEach(visit);
        break;

      case 'FieldAccessExpr':
        visit(e.object);
        break;

      case 'IndexExpr':
        visit(e.object);
        visit(e.index);
        break;

      case 'PipelineExpr':
        visit(e.left);
        visit(e.right);
        break;

      default:
        // Literals, identifiers - no calls
        break;
    }
  }

  visit(expr);
  return calls;
}

/**
 * Get the location of a function declaration
 */
function getFunctionLocation(program: AST.Program, functionName: string): AST.SourceLocation | undefined {
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl' && decl.name === functionName) {
      return decl.location;
    }
  }
  return undefined;
}
