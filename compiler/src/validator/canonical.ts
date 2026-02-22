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
 * Build a map of type names to their definitions for lookup
 */
function buildTypeDefinitionMap(program: AST.Program): Map<string, AST.TypeDef> {
  const typeMap = new Map<string, AST.TypeDef>();
  for (const decl of program.declarations) {
    if (decl.type === 'TypeDecl') {
      typeMap.set(decl.name, decl.definition);
    }
  }
  return typeMap;
}

/**
 * Rule 1: Recursive functions must have exactly ONE PRIMITIVE parameter
 *
 * This makes accumulator-style tail recursion impossible:
 * ❌ λfactorial(n:ℤ,acc:ℤ)→ℤ=...       (2 parameters - rejected)
 * ❌ λfactorial(state:[ℤ])→ℤ=...       (list parameter - rejected)
 * ❌ λfactorial(state:(ℤ,ℤ))→ℤ=...     (tuple parameter - rejected)
 * ✅ λfactorial(n:ℤ)→ℤ=...             (1 primitive parameter - allowed)
 */
function validateRecursiveFunctions(program: AST.Program): void {
  // Build type definition map for resolving user-defined types
  const typeMap = buildTypeDefinitionMap(program);

  for (const decl of program.declarations) {
    if (decl.type !== 'FunctionDecl') continue;

    // Check if function is recursive (calls itself)
    const isRecursive = containsRecursiveCall(decl.body, decl.name);

    if (!isRecursive) continue;

    // Check 1: No multiple parameters
    if (decl.params.length > 1) {
      throw new CanonicalError(
        `Recursive function '${decl.name}' has ${decl.params.length} parameters.\n` +
        `Recursive functions must have exactly ONE primitive parameter.\n` +
        `This prevents accumulator-style tail recursion.\n` +
        `\n` +
        `Example canonical form:\n` +
        `  λ${decl.name}(n:ℤ)→ℤ≡n{0→1|n→n*${decl.name}(n-1)}\n` +
        `\n` +
        `Mint enforces ONE way to write recursive functions.`,
        decl.location
      );
    }

    // Check 2: Parameter must be primitive type (not collection)
    // This closes the loophole: state:[ℤ] encodes multiple values
    if (decl.params.length === 1) {
      const param = decl.params[0];
      if (param.typeAnnotation && isCollectionType(param.typeAnnotation, typeMap)) {
        throw new CanonicalError(
          `Recursive function '${decl.name}' has a collection-type parameter.\n` +
          `Parameter type: ${formatType(param.typeAnnotation)}\n` +
          `\n` +
          `Recursive functions must have a PRIMITIVE parameter (ℤ, 𝕊, 𝔹, etc).\n` +
          `Collection types (lists, tuples, records) can encode multiple values,\n` +
          `which enables accumulator-style tail recursion.\n` +
          `\n` +
          `Example canonical form:\n` +
          `  λ${decl.name}(n:ℤ)→ℤ≡n{0→1|n→n*${decl.name}(n-1)}\n` +
          `\n` +
          `Mint enforces ONE way to write recursive functions.`,
          decl.location
        );
      }
    }

    // Check 3: Return type cannot be a function (blocks CPS/continuation passing)
    // This closes the CPS loophole: λfactorial(n:ℤ)→λ(ℤ)→ℤ
    if (decl.returnType && decl.returnType.type === 'FunctionType') {
      throw new CanonicalError(
        `Recursive function '${decl.name}' returns a function type.\n` +
        `Return type: ${formatType(decl.returnType)}\n` +
        `\n` +
        `This is Continuation Passing Style (CPS), which encodes\n` +
        `an accumulator in the returned function.\n` +
        `\n` +
        `Recursive functions must return a VALUE, not a FUNCTION.\n` +
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

/**
 * Check if a type is a collection type (can encode multiple values)
 *
 * Collection types enable the accumulator pattern loophole:
 * - Lists: [ℤ] can hold [n, acc]
 * - Tuples: (ℤ,ℤ) directly encodes (n, acc)
 * - Maps: {ℤ:ℤ} can encode multiple key-value pairs
 * - Records: {n:ℤ,acc:ℤ} directly encodes multiple values (LOOPHOLE CLOSED!)
 */
function isCollectionType(type: AST.Type, typeMap: Map<string, AST.TypeDef>): boolean {
  switch (type.type) {
    case 'ListType':
    case 'TupleType':
    case 'MapType':
      return true;

    case 'TypeConstructor':
    case 'TypeVariable':
      // Resolve user-defined types to check if they're record types
      // Note: Parser treats `State` as TypeVariable when used without args (State)
      // and as TypeConstructor when used with args (State[T])
      const typeDef = typeMap.get(type.name);
      if (typeDef && typeDef.type === 'ProductType') {
        // Record types with multiple fields can encode multiple values
        // This closes the loophole: t State={n:ℤ,acc:ℤ}
        return typeDef.fields.length > 1;
      }
      // Type aliases and sum types are OK (they don't encode multiple values directly)
      return false;

    case 'PrimitiveType':
    case 'FunctionType':
      return false;

    default:
      return false;
  }
}

/**
 * Format a type for error messages
 */
function formatType(type: AST.Type): string {
  switch (type.type) {
    case 'PrimitiveType':
      return type.name;
    case 'ListType':
      return `[${formatType(type.elementType)}]`;
    case 'TupleType':
      return `(tuple)`;
    case 'MapType':
      return `{${formatType(type.keyType)}:${formatType(type.valueType)}}`;
    case 'TypeVariable':
      return type.name;
    case 'TypeConstructor':
      return type.name;
    case 'FunctionType':
      return `function`;
    default:
      return 'unknown';
  }
}
