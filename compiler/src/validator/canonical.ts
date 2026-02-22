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
  validateCanonicalPatternMatching(program);
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
 * Rule 1: Recursive functions must use canonical parameter patterns
 *
 * This blocks accumulator-style tail recursion while allowing legitimate
 * multi-parameter algorithms (like GCD, power, ackermann).
 *
 * ❌ λfactorial(n:ℤ,acc:ℤ)→ℤ=...       (accumulator pattern - rejected)
 * ❌ λfactorial(state:[ℤ])→ℤ=...       (collection encoding - rejected)
 * ✅ λfactorial(n:ℤ)→ℤ=...             (single primitive - allowed)
 * ✅ λgcd(a:ℤ,b:ℤ)→ℤ=...               (multi-param algorithm - allowed)
 */
function validateRecursiveFunctions(program: AST.Program): void {
  // Build type definition map for resolving user-defined types
  const typeMap = buildTypeDefinitionMap(program);

  for (const decl of program.declarations) {
    if (decl.type !== 'FunctionDecl') continue;

    // Check if function is recursive (calls itself)
    const isRecursive = containsRecursiveCall(decl.body, decl.name);

    if (!isRecursive) continue;

    // Check 1: If multiple parameters, detect accumulator pattern
    if (decl.params.length > 1) {
      // Check if this looks like accumulator-passing style
      const recursiveCalls = findRecursiveCalls(decl.body, decl.name);

      for (const call of recursiveCalls) {
        if (looksLikeAccumulatorPattern(call, decl.params)) {
          throw new CanonicalError(
            `Recursive function '${decl.name}' uses accumulator-passing style.\n` +
            `\n` +
            `Detected pattern: one parameter decrements while another accumulates.\n` +
            `This is tail-call optimization, which Mint blocks.\n` +
            `\n` +
            `Use direct recursion instead:\n` +
            `  λ${decl.name}(n:ℤ)→ℤ≡n{0→1|n→n*${decl.name}(n-1)}\n` +
            `\n` +
            `Note: Multi-parameter recursion IS allowed for algorithms like GCD\n` +
            `where both parameters are legitimately transformed:\n` +
            `  λgcd(a:ℤ,b:ℤ)→ℤ≡b{0→a|b→gcd(b,a%b)}\n` +
            `\n` +
            `Mint enforces ONE way to write each algorithm.`,
            decl.location
          );
        }
      }
    }

    // Check 2: Collection parameters - distinguish structural recursion from accumulator
    // Find collection-type parameters
    const collectionParams: {index: number, param: AST.Param}[] = [];
    for (let i = 0; i < decl.params.length; i++) {
      const typeAnnotation = decl.params[i].typeAnnotation;
      if (typeAnnotation && isCollectionType(typeAnnotation, typeMap)) {
        collectionParams.push({index: i, param: decl.params[i]});
      }
    }

    // If multiple collection params, likely accumulator pattern
    if (collectionParams.length > 1) {
      throw new CanonicalError(
        `Recursive function '${decl.name}' has multiple collection parameters.\n` +
        `This pattern can encode accumulator-passing style.\n` +
        `\n` +
        `Use single collection parameter with structural recursion instead:\n` +
        `  λreverse(lst:[T])→[T]≡lst{[]→[]|[x,.xs]→reverse(xs)++[x]}\n` +
        `\n` +
        `Mint enforces ONE way: structural recursion for collections.`,
        decl.location
      );
    }

    // Single collection param - check if structural recursion or accumulator pattern
    if (collectionParams.length === 1) {
      const collectionParam = collectionParams[0];

      // If there are other (non-collection) parameters alongside the collection,
      // this is likely accumulator pattern (collection + accumulators)
      if (decl.params.length > 1) {
        throw new CanonicalError(
          `Recursive function '${decl.name}' has collection parameter plus additional parameters.\n` +
          `This pattern enables accumulator-passing style (tail-call optimization).\n` +
          `\n` +
          `Example of what's blocked:\n` +
          `  λfold_sum(xs:[T],acc:ℤ,count:ℤ)→ℤ≡xs{...}\n` +
          `  - Collection parameter: xs\n` +
          `  - Accumulator parameters: acc, count\n` +
          `\n` +
          `Use pure structural recursion instead (single parameter only):\n` +
          `  λsum(xs:[T])→ℤ≡xs{[]→0|[x,.rest]→x+sum(rest)}\n` +
          `  λlength(xs:[T])→ℤ≡xs{[]→0|[_,.rest]→1+length(rest)}\n` +
          `\n` +
          `Mint enforces ONE way: structural recursion with single collection parameter.`,
          decl.location
        );
      }

      if (!isStructuralRecursion(decl, collectionParam)) {
        throw new CanonicalError(
          `Recursive function '${decl.name}' has collection parameter but doesn't use structural recursion.\n` +
          `Parameter: ${collectionParam.param.name}${collectionParam.param.typeAnnotation ? ':' + formatType(collectionParam.param.typeAnnotation) : ''}\n` +
          `\n` +
          `Structural recursion (ALLOWED):\n` +
          `  λreverse(lst:[T])→[T]≡lst{[]→[]|[x,.xs]→reverse(xs)++[x]}\n` +
          `  - Pattern matches on the collection\n` +
          `  - Destructures into pieces ([x,.xs])\n` +
          `  - Recurses on smaller piece (xs)\n` +
          `  - Single collection parameter only\n` +
          `\n` +
          `Blocked patterns:\n` +
          `  λfactorial(state:[ℤ])→ℤ≡state{[n,acc]→factorial([n-1,n*acc])}\n` +
          `  - Uses list to encode multiple values\n` +
          `  - Pattern [n,acc] extracts state, not structure\n` +
          `\n` +
          `Mint enforces ONE way: structural recursion for collections.`,
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

/**
 * Rule 3: Canonical Pattern Matching
 *
 * Pattern matches must use the most direct form possible:
 * - ✅ Match on parameter value directly: ≡n{0→...|n→...}
 * - ❌ Match on boolean when value matching works: ≡(n=0){⊤→...|⊥→...}
 *
 * Boolean/tuple matching allowed ONLY when value matching impossible:
 * - ✅ Complex conditions: ≡(x>0,y>0){(⊤,⊤)→...}
 * - ✅ Multiple parameters: ≡(x,y){...}
 */
function validateCanonicalPatternMatching(program: AST.Program): void {
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      validatePatternMatchingInExpr(decl.body, decl.params);
    }
  }
}

/**
 * Check if an expression uses non-canonical pattern matching
 */
function validatePatternMatchingInExpr(expr: AST.Expr, params: AST.Param[]): void {
  switch (expr.type) {
    case 'MatchExpr':
      validateMatchExpr(expr, params);
      // Recursively check match arms
      for (const arm of expr.arms) {
        validatePatternMatchingInExpr(arm.body, params);
      }
      // Check scrutinee
      validatePatternMatchingInExpr(expr.scrutinee, params);
      break;

    case 'LambdaExpr':
      validatePatternMatchingInExpr(expr.body, expr.params);
      break;

    case 'ApplicationExpr':
      validatePatternMatchingInExpr(expr.func, params);
      for (const arg of expr.args) {
        validatePatternMatchingInExpr(arg, params);
      }
      break;

    case 'BinaryExpr':
      validatePatternMatchingInExpr(expr.left, params);
      validatePatternMatchingInExpr(expr.right, params);
      break;

    case 'UnaryExpr':
      validatePatternMatchingInExpr(expr.operand, params);
      break;

    case 'LetExpr':
      validatePatternMatchingInExpr(expr.value, params);
      validatePatternMatchingInExpr(expr.body, params);
      break;

    case 'ListExpr':
      for (const elem of expr.elements) {
        validatePatternMatchingInExpr(elem, params);
      }
      break;

    case 'RecordExpr':
      for (const field of expr.fields) {
        validatePatternMatchingInExpr(field.value, params);
      }
      break;

    case 'TupleExpr':
      for (const elem of expr.elements) {
        validatePatternMatchingInExpr(elem, params);
      }
      break;

    case 'FieldAccessExpr':
      validatePatternMatchingInExpr(expr.object, params);
      break;

    case 'IndexExpr':
      validatePatternMatchingInExpr(expr.object, params);
      validatePatternMatchingInExpr(expr.index, params);
      break;

    case 'PipelineExpr':
      validatePatternMatchingInExpr(expr.left, params);
      validatePatternMatchingInExpr(expr.right, params);
      break;

    case 'MapExpr':
      validatePatternMatchingInExpr(expr.list, params);
      validatePatternMatchingInExpr(expr.fn, params);
      break;

    case 'FilterExpr':
      validatePatternMatchingInExpr(expr.list, params);
      validatePatternMatchingInExpr(expr.predicate, params);
      break;

    case 'FoldExpr':
      validatePatternMatchingInExpr(expr.list, params);
      validatePatternMatchingInExpr(expr.fn, params);
      validatePatternMatchingInExpr(expr.init, params);
      break;

    // Literals and identifiers don't contain pattern matches
    case 'LiteralExpr':
    case 'IdentifierExpr':
      break;
  }
}

/**
 * Check if a match expression uses canonical pattern matching
 */
function validateMatchExpr(match: AST.MatchExpr, params: AST.Param[]): void {
  const scrutinee = match.scrutinee;

  // Check if scrutinee is a single parameter reference
  if (scrutinee.type === 'IdentifierExpr' && params.length === 1 && scrutinee.name === params[0].name) {
    // This is matching on the function parameter directly - CANONICAL
    // ≡n{0→...|n→...}
    return;
  }

  // Check if scrutinee is a boolean/comparison expression on a single parameter
  if (isSingleParamComparison(scrutinee, params)) {
    throw new CanonicalError(
      `Non-canonical pattern matching: matching on boolean expression.\n` +
      `\n` +
      `Found: ≡(${formatScrutinee(scrutinee)}){...}\n` +
      `\n` +
      `Use direct value matching instead:\n` +
      `  ≡${params[0].name}{0→...|${params[0].name}→...}\n` +
      `\n` +
      `Boolean matching is only allowed when value matching is impossible\n` +
      `(e.g., complex conditions like ≡(x>0,y>0){...}).\n` +
      `\n` +
      `Mint enforces ONE way: use the most direct pattern matching form.`,
      match.location
    );
  }

  // Check if scrutinee is a tuple of boolean expressions on a single parameter
  if (scrutinee.type === 'TupleExpr' && isTupleSingleParamComparisons(scrutinee, params)) {
    throw new CanonicalError(
      `Non-canonical pattern matching: tuple of boolean expressions on single parameter.\n` +
      `\n` +
      `Found: ≡(${formatTupleScrutinee(scrutinee)}){...}\n` +
      `\n` +
      `Use direct value matching instead:\n` +
      `  ≡${params[0].name}{0→...|1→...|${params[0].name}→...}\n` +
      `\n` +
      `Tuple boolean matching is only allowed for multiple independent conditions\n` +
      `(e.g., ≡(x>0,y>0){...} for two different variables).\n` +
      `\n` +
      `Mint enforces ONE way: use the most direct pattern matching form.`,
      match.location
    );
  }
}

/**
 * Check if expression is a comparison on a single parameter
 * E.g., n=0, n>5, etc.
 */
function isSingleParamComparison(expr: AST.Expr, params: AST.Param[]): boolean {
  if (params.length !== 1) return false;

  if (expr.type === 'BinaryExpr') {
    const isComparison = ['=', '≠', '<', '>', '≤', '≥'].includes(expr.operator);
    if (!isComparison) return false;

    // Check if either side is the parameter
    const leftIsParam = expr.left.type === 'IdentifierExpr' && expr.left.name === params[0].name;
    const rightIsParam = expr.right.type === 'IdentifierExpr' && expr.right.name === params[0].name;

    return leftIsParam || rightIsParam;
  }

  return false;
}

/**
 * Check if tuple contains comparisons all on the same single parameter
 */
function isTupleSingleParamComparisons(tuple: AST.TupleExpr, params: AST.Param[]): boolean {
  if (params.length !== 1) return false;

  return tuple.elements.every(elem => isSingleParamComparison(elem, params));
}

/**
 * Format scrutinee for error message
 */
function formatScrutinee(expr: AST.Expr): string {
  if (expr.type === 'BinaryExpr') {
    return `${formatExpr(expr.left)}${expr.operator}${formatExpr(expr.right)}`;
  }
  return formatExpr(expr);
}

/**
 * Format tuple scrutinee for error message
 */
function formatTupleScrutinee(tuple: AST.TupleExpr): string {
  return tuple.elements.map(formatScrutinee).join(',');
}

/**
 * Format expression for error message
 */
function formatExpr(expr: AST.Expr): string {
  switch (expr.type) {
    case 'IdentifierExpr':
      return expr.name;
    case 'LiteralExpr':
      return String(expr.value);
    case 'BinaryExpr':
      return `${formatExpr(expr.left)}${expr.operator}${formatExpr(expr.right)}`;
    default:
      return '...';
  }
}

/**
 * Find all recursive calls in an expression
 */
function findRecursiveCalls(expr: AST.Expr, functionName: string): AST.ApplicationExpr[] {
  const calls: AST.ApplicationExpr[] = [];

  function visit(e: AST.Expr): void {
    if (e.type === 'ApplicationExpr' && e.func.type === 'IdentifierExpr' && e.func.name === functionName) {
      calls.push(e);
    }

    // Recursively visit sub-expressions
    switch (e.type) {
      case 'BinaryExpr':
        visit(e.left);
        visit(e.right);
        break;
      case 'UnaryExpr':
        visit(e.operand);
        break;
      case 'ApplicationExpr':
        visit(e.func);
        e.args.forEach(visit);
        break;
      case 'MatchExpr':
        visit(e.scrutinee);
        e.arms.forEach(arm => visit(arm.body));
        break;
      case 'LetExpr':
        visit(e.value);
        visit(e.body);
        break;
      case 'LambdaExpr':
        visit(e.body);
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
      case 'MapExpr':
        visit(e.list);
        visit(e.fn);
        break;
      case 'FilterExpr':
        visit(e.list);
        visit(e.predicate);
        break;
      case 'FoldExpr':
        visit(e.list);
        visit(e.fn);
        visit(e.init);
        break;
    }
  }

  visit(expr);
  return calls;
}

/**
 * Check if a recursive call looks like accumulator-passing style
 *
 * Accumulator pattern: one param decrements (n-1), another accumulates (n*acc)
 * Legitimate multi-param: both params transform (gcd(b, a%b))
 *
 * Heuristic: If one argument is just "param - constant" and another argument
 * contains multiplication/addition with param names, likely accumulator.
 */
function looksLikeAccumulatorPattern(call: AST.ApplicationExpr, params: AST.Param[]): boolean {
  if (call.args.length !== params.length) return false;
  if (params.length < 2) return false;

  const paramNames = new Set(params.map(p => p.name));

  let hasDecrement = false;
  let hasAccumulation = false;

  for (let i = 0; i < call.args.length; i++) {
    const arg = call.args[i];
    const paramName = params[i].name;

    // Check if this arg looks like a decrement: n-1, n-2, etc.
    if (isDecrementPattern(arg, paramName)) {
      hasDecrement = true;
    }

    // Check if this arg looks like accumulation: n*acc, acc+n, etc.
    if (isAccumulationPattern(arg, paramNames)) {
      hasAccumulation = true;
    }
  }

  // If one param decrements and another accumulates, it's likely accumulator pattern
  return hasDecrement && hasAccumulation;
}

/**
 * Check if expression is a decrement pattern like n-1, n-2
 */
function isDecrementPattern(expr: AST.Expr, paramName: string): boolean {
  if (expr.type === 'BinaryExpr' && expr.operator === '-') {
    // Check if left side is the param and right side is a constant
    return expr.left.type === 'IdentifierExpr' &&
           expr.left.name === paramName &&
           expr.right.type === 'LiteralExpr';
  }
  return false;
}

/**
 * Check if expression contains accumulation pattern
 * (multiplication or addition involving multiple param names)
 */
function isAccumulationPattern(expr: AST.Expr, paramNames: Set<string>): boolean {
  if (expr.type === 'BinaryExpr' && (expr.operator === '*' || expr.operator === '+')) {
    // Check if both sides reference parameter names
    const leftHasParam = containsParamReference(expr.left, paramNames);
    const rightHasParam = containsParamReference(expr.right, paramNames);
    return leftHasParam && rightHasParam;
  }
  return false;
}

/**
 * Check if expression contains a reference to any of the parameter names
 */
function containsParamReference(expr: AST.Expr, paramNames: Set<string>): boolean {
  if (expr.type === 'IdentifierExpr') {
    return paramNames.has(expr.name);
  }
  if (expr.type === 'BinaryExpr') {
    return containsParamReference(expr.left, paramNames) ||
           containsParamReference(expr.right, paramNames);
  }
  return false;
}

/**
 * Check if a function uses structural recursion on a collection parameter
 *
 * Structural recursion (ALLOWED):
 *   - Pattern matches on collection: ≡lst{[]→...|[x,.xs]→...}
 *   - Destructures into smaller pieces: [x,.xs]
 *   - Recursive calls use the smaller pieces: xs (not lst)
 *
 * Accumulator pattern (BLOCKED):
 *   - Multiple params with one being collection accumulator
 *   - Collection passed unchanged or grown
 */
function isStructuralRecursion(
  decl: AST.FunctionDecl,
  collectionParam: {index: number, param: AST.Param}
): boolean {
  const paramName = collectionParam.param.name;

  // Find match expression that matches on the collection parameter
  const matchExpr = findPatternMatchOnParam(decl.body, paramName);

  if (!matchExpr) {
    // No pattern match on collection - not structural recursion
    return false;
  }

  // Check if any pattern arm destructures the collection
  const hasDestructuring = matchExpr.arms.some(arm =>
    isDestructuringPattern(arm.pattern)
  );

  if (!hasDestructuring) {
    // Pattern match exists but no destructuring - not structural
    return false;
  }

  // Check if list patterns are encoding state rather than structure
  // E.g., [n,acc] or [[n,acc]] - fixed-size patterns that extract values
  for (const arm of matchExpr.arms) {
    if (arm.pattern.type === 'ListPattern') {
      // If all elements are identifier patterns (no rest), it's encoding state
      if (arm.pattern.patterns.length >= 2 && !arm.pattern.rest) {
        // Pattern like [n, acc] or [x, y, z] - encoding multiple values, not structure
        return false;
      }

      // Check for nested list patterns that encode state: [[n,acc]]
      if (arm.pattern.patterns.length === 1 && arm.pattern.patterns[0].type === 'ListPattern') {
        const innerPattern = arm.pattern.patterns[0] as AST.ListPattern;
        if (innerPattern.patterns.length >= 2 && !innerPattern.rest) {
          // Pattern like [[n, acc]] - nested encoding of multiple values
          return false;
        }
      }
    }
  }

  // Check that recursive calls use smaller pieces from destructuring
  const recursiveCalls = findRecursiveCalls(decl.body, decl.name);

  for (const call of recursiveCalls) {
    // Get the argument passed for the collection parameter position
    const collectionArg = call.args[collectionParam.index];

    // Check if this argument is a reference to a destructured piece
    // (like 'xs' from pattern [x,.xs]) or the original parameter unchanged
    if (collectionArg.type === 'IdentifierExpr' &&
        collectionArg.name === paramName) {
      // Passing the original parameter unchanged - not structural!
      return false;
    }
  }

  return true;
}

/**
 * Find a match expression that matches on the given parameter
 */
function findPatternMatchOnParam(expr: AST.Expr, paramName: string): AST.MatchExpr | null {
  if (expr.type === 'MatchExpr') {
    // Check if scrutinee is the parameter
    if (expr.scrutinee.type === 'IdentifierExpr' && expr.scrutinee.name === paramName) {
      return expr;
    }
  }

  // Recursively search in sub-expressions
  switch (expr.type) {
    case 'LambdaExpr':
      return findPatternMatchOnParam(expr.body, paramName);
    case 'LetExpr':
      return findPatternMatchOnParam(expr.body, paramName) ||
             findPatternMatchOnParam(expr.value, paramName);
    case 'IfExpr':
      return findPatternMatchOnParam(expr.thenBranch, paramName) ||
             (expr.elseBranch ? findPatternMatchOnParam(expr.elseBranch, paramName) : null);
    case 'MatchExpr':
      // Check scrutinee first
      if (expr.scrutinee.type === 'IdentifierExpr' && expr.scrutinee.name === paramName) {
        return expr;
      }
      // Check in match arms
      for (const arm of expr.arms) {
        const found = findPatternMatchOnParam(arm.body, paramName);
        if (found) return found;
      }
      return null;
    default:
      return null;
  }
}

/**
 * Check if a pattern destructures a collection
 * Examples: [x,.xs], [x,y,.rest], {field1, field2}
 */
function isDestructuringPattern(pattern: AST.Pattern): boolean {
  switch (pattern.type) {
    case 'ListPattern':
      // List patterns with at least one element or rest are destructuring
      return pattern.patterns.length > 0 || pattern.rest !== null;

    case 'RecordPattern':
      // Record patterns with fields are destructuring
      return pattern.fields.length > 0;

    case 'ConstructorPattern':
      // Constructor patterns with fields are destructuring
      return pattern.patterns.length > 0;

    case 'TuplePattern':
      // Tuple patterns are destructuring
      return pattern.patterns.length > 0;

    default:
      return false;
  }
}
