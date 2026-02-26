/**
 * Canonical Form Validator
 *
 * Enforces Sigil's "ONE WAY" principle by making alternative patterns impossible:
 * 1. Accumulator parameters forbidden (prevents tail-call optimization)
 * 2. Canonical pattern matching (most direct form)
 * 3. CPS forbidden (function return types blocked for recursive functions)
 *
 * This ensures LLMs cannot generate multiple ways to solve the same problem.
 */

import * as AST from '../parser/ast.js';
import { SigilDiagnosticError } from '../diagnostics/error.js';
import { astLocationToSpan, diagnostic, suggestGeneric, suggestReorderDeclaration } from '../diagnostics/helpers.js';
import type { Fixit, Suggestion } from '../diagnostics/types.js';

/**
 * Parameter role classification for multi-parameter recursion validation
 *
 * STRUCTURAL: Decreases/decomposes during recursion (n-1, xs, a%b)
 * QUERY: Stays constant or swaps algorithmically (target, base)
 * ACCUMULATOR: Grows/builds up (n*acc, acc+x, [x,.acc]) - FORBIDDEN
 * UNKNOWN: Cannot determine role
 */
enum ParameterRole {
  STRUCTURAL,   // Decreases/decomposes - ALLOWED
  QUERY,        // Stays constant - ALLOWED
  ACCUMULATOR,  // Grows/builds up - FORBIDDEN
  UNKNOWN       // Cannot determine
}

export class CanonicalError extends SigilDiagnosticError {
  constructor(
    code: string,
    message: string,
    public location?: AST.SourceLocation,
    extras: {
      details?: Record<string, unknown>;
      fixits?: Fixit[];
      suggestions?: Suggestion[];
    } = {}
  ) {
    super(diagnostic(code, 'canonical', message, {
      location: astLocationToSpan('<unknown>', location),
      details: extras.details,
      fixits: extras.fixits,
      suggestions: extras.suggestions,
    }));
    this.name = 'CanonicalError';
  }
}

/**
 * Validate that there are no duplicate declarations
 *
 * Sigil enforces ONE canonical declaration per name.
 * No duplicate types, externs, imports, consts, or functions allowed.
 */
function validateNoDuplicateDeclarations(program: AST.Program): void {
  const typeNames = new Map<string, AST.SourceLocation>();
  const externNames = new Map<string, AST.SourceLocation>();
  const importPaths = new Map<string, AST.SourceLocation>();
  const constNames = new Map<string, AST.SourceLocation>();
  const functionNames = new Map<string, AST.SourceLocation>();
  const testNames = new Map<string, AST.SourceLocation>();

  for (const decl of program.declarations) {
    switch (decl.type) {
      case 'TypeDecl': {
        const name = decl.name;
        if (typeNames.has(name)) {
          throw new CanonicalError(
            'SIGIL-CANON-DUPLICATE-TYPE',
            `Duplicate type declaration: "${name}"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate type declaration.`,
            decl.location,
            {
              details: { typeName: name, firstLocation: typeNames.get(name) },
              suggestions: [suggestGeneric('remove duplicate type declaration', 'remove_duplicate')]
            }
          );
        }
        typeNames.set(name, decl.location);
        break;
      }

      case 'ExternDecl': {
        const name = decl.modulePath.join('⋅');
        if (externNames.has(name)) {
          throw new CanonicalError(
            'SIGIL-CANON-DUPLICATE-EXTERN',
            `Duplicate extern declaration: "${name}"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate extern declaration.`,
            decl.location,
            {
              details: { externName: name, firstLocation: externNames.get(name) },
              suggestions: [suggestGeneric('remove duplicate extern declaration', 'remove_duplicate')]
            }
          );
        }
        externNames.set(name, decl.location);
        break;
      }

      case 'ImportDecl': {
        const path = decl.modulePath.join('⋅');
        if (importPaths.has(path)) {
          throw new CanonicalError(
            'SIGIL-CANON-DUPLICATE-IMPORT',
            `Duplicate import declaration: "${path}"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate import declaration.`,
            decl.location,
            {
              details: { importPath: path, firstLocation: importPaths.get(path) },
              suggestions: [suggestGeneric('remove duplicate import declaration', 'remove_duplicate')]
            }
          );
        }
        importPaths.set(path, decl.location);
        break;
      }

      case 'ConstDecl': {
        const name = decl.name;
        if (constNames.has(name)) {
          throw new CanonicalError(
            'SIGIL-CANON-DUPLICATE-CONST',
            `Duplicate const declaration: "${name}"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate const declaration.`,
            decl.location,
            {
              details: { constName: name, firstLocation: constNames.get(name) },
              suggestions: [suggestGeneric('remove duplicate const declaration', 'remove_duplicate')]
            }
          );
        }
        constNames.set(name, decl.location);
        break;
      }

      case 'FunctionDecl': {
        const name = decl.name;
        if (functionNames.has(name)) {
          throw new CanonicalError(
            'SIGIL-CANON-DUPLICATE-FUNCTION',
            `Duplicate function declaration: "${name}"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate function declaration.`,
            decl.location,
            {
              details: { functionName: name, firstLocation: functionNames.get(name) },
              suggestions: [suggestGeneric('remove duplicate function declaration', 'remove_duplicate')]
            }
          );
        }
        functionNames.set(name, decl.location);
        break;
      }

      case 'TestDecl': {
        const name = decl.description;
        if (testNames.has(name)) {
          throw new CanonicalError(
            'SIGIL-CANON-DUPLICATE-TEST',
            `Duplicate test declaration: "${name}"\n\nSigil enforces ONE canonical declaration per name.\nRemove the duplicate test declaration.`,
            decl.location,
            {
              details: { testName: name, firstLocation: testNames.get(name) },
              suggestions: [suggestGeneric('remove duplicate test declaration', 'remove_duplicate')]
            }
          );
        }
        testNames.set(name, decl.location);
        break;
      }
    }
  }
}

/**
 * Validate that the program follows canonical form rules
 */
export function validateCanonicalForm(program: AST.Program, filename?: string): void {
  try {
    validateNoDuplicateDeclarations(program);
    validateRecursiveFunctions(program);
    validateCanonicalPatternMatching(program);
    validateDeclarationOrdering(program);
  } catch (error) {
    if (filename && error instanceof CanonicalError && error.diagnostic.location?.file === '<unknown>') {
      error.diagnostic.location.file = filename;
    }
    throw error;
  }
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

    // Check 1: If multiple parameters, classify each parameter's role
    if (decl.params.length > 1) {
      const recursiveCalls = findRecursiveCalls(decl.body, decl.name);

      // Classify each parameter's role (STRUCTURAL, QUERY, ACCUMULATOR)
      const paramRoles = classifyParameters(decl, recursiveCalls);

      // Check if any parameter is an accumulator (FORBIDDEN)
      const accumulatorParams: string[] = [];
      const paramRoleDescriptions: string[] = [];

      for (const param of decl.params) {
        const role = paramRoles.get(param.name);
        if (role === ParameterRole.ACCUMULATOR) {
          accumulatorParams.push(param.name);
        }

        // Build description for error message
        const roleStr = role === ParameterRole.ACCUMULATOR ? 'ACCUMULATOR (grows)' :
                       role === ParameterRole.STRUCTURAL ? 'structural (decreases)' :
                       role === ParameterRole.QUERY ? 'query (constant)' :
                       'unknown';
        paramRoleDescriptions.push(`  - ${param.name}: ${roleStr}`);
      }

      if (accumulatorParams.length > 0) {
        throw new CanonicalError(
          'SIGIL-CANON-RECURSION-ACCUMULATOR',
          `Accumulator-passing style detected in function '${decl.name}'.\n` +
          `\n` +
          `Parameter roles:\n${paramRoleDescriptions.join('\n')}\n` +
          `\n` +
          `The parameter(s) [${accumulatorParams.join(', ')}] are accumulators (grow during recursion).\n` +
          `Sigil does NOT support tail-call optimization or accumulator-passing style.\n` +
          `\n` +
          `Accumulator pattern (FORBIDDEN):\n` +
          `  λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→factorial(n-1,n*acc)}\n` +
          `  - Parameter 'acc' only grows (n*acc) → ACCUMULATOR\n` +
          `\n` +
          `Legitimate multi-parameter (ALLOWED):\n` +
          `  λgcd(a:ℤ,b:ℤ)→ℤ≡b{0→a|b→gcd(b,a%b)}\n` +
          `  - Both 'a' and 'b' transform algorithmically → structural\n` +
          `\n` +
          `Use simple recursion without accumulator parameters.`,
          decl.location,
          {
            details: { functionName: decl.name, accumulatorParams, kind: 'accumulator_passing_style' },
            suggestions: [suggestGeneric('rewrite recursive function without accumulator parameters', 'remove_accumulator_parameter')]
          }
        );
      }

      // If all params are STRUCTURAL or QUERY → ALLOW
      // This is the key change - allows GCD, binary search, nth, etc.
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

    // Multiple collection params - now allowed if they're all structural
    // Check 1 above already validated that none are accumulators
    // So if we're here with multiple collections, they're all decomposing (allowed)

    // Single collection param - check if structural recursion or accumulator pattern
    if (collectionParams.length === 1 && decl.params.length === 1) {
      // Single collection parameter only - validate structural recursion
      const collectionParam = collectionParams[0];

      if (!isStructuralRecursion(decl, collectionParam)) {
        throw new CanonicalError(
          'SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL',
          `Recursive function '${decl.name}' has collection parameter but doesn't use structural recursion.\n` +
          `Parameter: ${collectionParam.param.name}${collectionParam.param.typeAnnotation ? ':' + formatType(collectionParam.param.typeAnnotation) : ''}\n` +
          `\n` +
          `Structural recursion (ALLOWED):\n` +
          `  λreverse(lst:[T])→[T]≡lst{[]→[]|[x,.xs]→reverse(xs)⧺[x]}\n` +
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
          `Sigil enforces ONE way: structural recursion for collections.`,
          decl.location,
          {
            details: { functionName: decl.name, kind: 'non_structural_collection_recursion' },
            suggestions: [suggestGeneric('use structural recursion on the collection parameter', 'rewrite_recursive_form')]
          }
        );
      }
    }

    // Check 3: Return type cannot be a function (blocks CPS/continuation passing)
    // This closes the CPS loophole: λfactorial(n:ℤ)→λ(ℤ)→ℤ
    if (decl.returnType && decl.returnType.type === 'FunctionType') {
      throw new CanonicalError(
        'SIGIL-CANON-RECURSION-CPS',
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
        `Sigil enforces ONE way to write recursive functions.`,
        decl.location,
        {
          details: { functionName: decl.name, kind: 'recursive_function_returns_function' },
          suggestions: [suggestGeneric('recursive functions must return values, not function continuations', 'return_value_not_function')]
        }
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
      'SIGIL-CANON-MATCH-BOOLEAN',
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
      `Sigil enforces ONE way: use the most direct pattern matching form.`,
      match.location,
      {
        details: { kind: 'boolean_match_on_single_param', parameterName: params[0]?.name },
        suggestions: [suggestGeneric('match directly on the parameter value instead of a boolean comparison', 'use_direct_value_match')]
      }
    );
  }

  // Check if scrutinee is a tuple of boolean expressions on a single parameter
  if (scrutinee.type === 'TupleExpr' && isTupleSingleParamComparisons(scrutinee, params)) {
    throw new CanonicalError(
      'SIGIL-CANON-MATCH-TUPLE-BOOLEAN',
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
      `Sigil enforces ONE way: use the most direct pattern matching form.`,
      match.location,
      {
        details: { kind: 'tuple_boolean_match_on_single_param', parameterName: params[0]?.name },
        suggestions: [suggestGeneric('use direct value matching for a single parameter instead of tuple boolean matching', 'use_direct_value_match')]
      }
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
 * Classify each parameter's role across ALL recursive calls
 *
 * Returns map of parameter name → role (STRUCTURAL, QUERY, ACCUMULATOR, UNKNOWN)
 */
function classifyParameters(
  decl: AST.FunctionDecl,
  recursiveCalls: AST.ApplicationExpr[]
): Map<string, ParameterRole> {
  const paramRoles = new Map<string, ParameterRole>();

  // Initialize all params as UNKNOWN
  for (const param of decl.params) {
    paramRoles.set(param.name, ParameterRole.UNKNOWN);
  }

  // Analyze each parameter position across all recursive calls
  for (let i = 0; i < decl.params.length; i++) {
    const param = decl.params[i];
    const role = analyzeParameterAcrossCalls(param, i, recursiveCalls, decl.params);
    paramRoles.set(param.name, role);
  }

  return paramRoles;
}

/**
 * Analyze how a parameter is used across ALL recursive calls
 */
function analyzeParameterAcrossCalls(
  param: AST.Param,
  position: number,
  calls: AST.ApplicationExpr[],
  allParams: AST.Param[]
): ParameterRole {
  const paramName = param.name;
  const allParamNames = new Set(allParams.map(p => p.name));

  let seenStructural = false;
  let seenQuery = false;
  let seenAccumulator = false;

  for (const call of calls) {
    if (position >= call.args.length) continue;  // Safety check
    const arg = call.args[position];

    // Case 1: Passed unchanged (QUERY)
    if (isIdenticalToParam(arg, paramName)) {
      seenQuery = true;
      continue;
    }

    // Case 2: Decrement pattern (STRUCTURAL)
    if (isDecrementPattern(arg, paramName)) {
      seenStructural = true;
      continue;
    }

    // Case 3: List/collection decomposition (STRUCTURAL)
    if (isCollectionDecomposition(arg, paramName, allParamNames)) {
      seenStructural = true;
      continue;
    }

    // Case 4: Pure transformation (STRUCTURAL/QUERY)
    // Example: gcd(b, a%b) where a and b swap
    if (isPureTransformation(arg, allParamNames)) {
      seenStructural = true;
      continue;
    }

    // Case 5: Accumulation pattern (ACCUMULATOR) - FORBIDDEN
    if (isAccumulationExpression(arg, paramName, allParamNames)) {
      seenAccumulator = true;
      // Continue analyzing to provide comprehensive error info
    }
  }

  // Classification priority:
  // 1. If ANY call shows accumulation → ACCUMULATOR (forbidden)
  // 2. If decreases → STRUCTURAL
  // 3. If unchanged → QUERY
  // 4. Mixed structural/query → STRUCTURAL (conservative)

  if (seenAccumulator) {
    return ParameterRole.ACCUMULATOR;
  }

  if (seenStructural) {
    return ParameterRole.STRUCTURAL;
  }

  if (seenQuery) {
    return ParameterRole.QUERY;
  }

  return ParameterRole.UNKNOWN;
}

/**
 * Check if argument is identical to parameter (passed unchanged)
 */
function isIdenticalToParam(expr: AST.Expr, paramName: string): boolean {
  return expr.type === 'IdentifierExpr' && expr.name === paramName;
}

/**
 * Check if expression is a pure transformation of parameters
 * Examples: a%b (modulo), b (swap), base+1 (constant offset)
 * NOT pure: n*acc (accumulation)
 */
function isPureTransformation(expr: AST.Expr, paramNames: Set<string>): boolean {
  if (expr.type === 'BinaryExpr') {
    const { operator, left, right } = expr;

    // Modulo is always structural (decreases)
    if (operator === '%') {
      return true;
    }

    // Division is structural (decreases)
    if (operator === '/') {
      return true;
    }

    // Addition/subtraction with constants is structural
    if (operator === '+' || operator === '-') {
      const leftIsParam = left.type === 'IdentifierExpr' && paramNames.has((left as AST.IdentifierExpr).name);
      const rightIsConst = right.type === 'LiteralExpr';
      const leftIsConst = left.type === 'LiteralExpr';
      const rightIsParam = right.type === 'IdentifierExpr' && paramNames.has((right as AST.IdentifierExpr).name);

      // Param +/- constant or constant +/- param
      if ((leftIsParam && rightIsConst) || (leftIsConst && rightIsParam)) {
        return true;
      }
    }

    // Multiplication with params suggests accumulation (unless with constants)
    if (operator === '*') {
      const leftIsParam = left.type === 'IdentifierExpr' && paramNames.has((left as AST.IdentifierExpr).name);
      const rightIsParam = right.type === 'IdentifierExpr' && paramNames.has((right as AST.IdentifierExpr).name);

      // n*acc pattern → NOT pure (accumulation)
      if (leftIsParam && rightIsParam) {
        return false;
      }
    }
  }

  // If it's just a param reference or constant, consider it pure
  if (expr.type === 'IdentifierExpr') {
    return paramNames.has(expr.name);
  }

  return false;
}

/**
 * Check if expression is accumulation (multiplies/adds params together)
 */
function isAccumulationExpression(
  expr: AST.Expr,
  _paramName: string,  // Kept for signature compatibility
  allParamNames: Set<string>
): boolean {
  if (expr.type === 'BinaryExpr') {
    const { operator, left, right } = expr;

    // Multiplication or addition of two params → accumulation pattern
    if (operator === '*' || operator === '+') {
      const leftHasParam = containsParamReference(left, allParamNames);
      const rightHasParam = containsParamReference(right, allParamNames);

      // Both sides reference params → likely accumulation (n*acc, acc+n)
      if (leftHasParam && rightHasParam) {
        return true;
      }
    }

    // String concatenation with params
    if (operator === '⧺') {
      const leftHasParam = containsParamReference(left, allParamNames);
      const rightHasParam = containsParamReference(right, allParamNames);

      if (leftHasParam && rightHasParam) {
        return true;  // acc⧺xs pattern
      }
    }
  }

  if (expr.type === 'ListExpr') {
    // Check for list construction that might be accumulating
    // Pattern: [x,.acc] in source becomes a ListExpr in AST
    // Heuristic: if list contains identifiers that are params, might be accumulation
    // This is conservative - we'll primarily rely on the *+/ pattern above
    for (const elem of expr.elements) {
      // Check if any element references params (could indicate list building)
      if (containsParamReference(elem, allParamNames)) {
        // Conservative: if building a list with param references, might be accumulation
        // But we need to be careful not to block legitimate uses
        // For now, we'll rely on the multiplication/addition checks above
        // List accumulation is typically [x,.acc] which shows up as addition
      }
    }
  }

  return false;
}

/**
 * Check if argument is collection decomposition (structural recursion)
 * Example: xs (from pattern [x,.xs])
 *
 * Heuristic: if arg is an identifier that's not an original param,
 * it's likely from pattern destructuring (structural)
 */
function isCollectionDecomposition(
  expr: AST.Expr,
  paramName: string,
  allParamNames: Set<string>
): boolean {
  if (expr.type === 'IdentifierExpr') {
    const argName = expr.name;

    // If it's a binding from pattern (not an original param), likely decomposition
    // Common patterns: xs (from [x,.xs]), ys, rest, tail
    if (argName !== paramName && !allParamNames.has(argName)) {
      // It's a new binding, not an original parameter
      // This suggests pattern destructuring (structural recursion)
      return true;
    }
  }

  return false;
}

/**
 * OLD IMPLEMENTATION - Kept for reference, now replaced by classifyParameters()
 *
 * Check if a recursive call looks like accumulator-passing style
 *
 * Accumulator pattern: one param decrements (n-1), another accumulates (n*acc)
 * Legitimate multi-param: both params transform (gcd(b, a%b))
 *
 * Heuristic: If one argument is just "param - constant" and another argument
 * contains multiplication/addition with param names, likely accumulator.
 */
/*
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
*/

/**
 * Check if expression is a decrement pattern like n-1, n-2
 * Used by both old and new implementations
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
 * OLD HELPER - Kept for potential future use
 * Check if expression contains accumulation pattern
 * (multiplication or addition involving multiple param names)
 */
/*
function isAccumulationPattern(expr: AST.Expr, paramNames: Set<string>): boolean {
  if (expr.type === 'BinaryExpr' && (expr.operator === '*' || expr.operator === '+')) {
    // Check if both sides reference parameter names
    const leftHasParam = containsParamReference(expr.left, paramNames);
    const rightHasParam = containsParamReference(expr.right, paramNames);
    return leftHasParam && rightHasParam;
  }
  return false;
}
*/

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

/**
 * Rule 3: Canonical Declaration Ordering
 *
 * Enforces strict categorical and alphabetical ordering of module-level declarations.
 * This ensures "ONE WAY" to organize code, making it deterministic and machine-friendly.
 *
 * Category order (strict):
 *   1. Types (t)
 *   2. Externs (e)
 *   3. Imports (i)
 *   4. Consts (c)
 *   5. Functions (λ)
 *   6. Tests (test)
 *
 * Rationale for types-first:
 *   Types must come before externs to enable typed FFI declarations that
 *   reference named types (e.g., e fs⋅promises : { mkdir : λ(𝕊, MkdirOptions) → 𝕌 })
 *
 * Within each category:
 *   - Non-exported declarations first (alphabetically)
 *   - Exported declarations second (alphabetically)
 *
 * Note: Declaration order does NOT affect semantics in Sigil (forward references work).
 * This is purely for canonicality.
 */
function validateDeclarationOrdering(program: AST.Program): void {
  const decls = program.declarations;

  // Categorize all declarations
  const categories = categorizeDeclarations(decls);

  // Check category order (t → e → i → c → λ → test)
  validateCategoryBoundaries(decls);

  // Check alphabetical ordering within each category
  validateWithinCategoryOrder(categories.externs, 'extern', 'e');
  validateWithinCategoryOrder(categories.imports, 'import', 'i');
  validateWithinCategoryOrder(categories.types, 'type', 't');
  validateWithinCategoryOrder(categories.consts, 'const', 'c');
  validateWithinCategoryOrder(categories.functions, 'function', 'λ');
  validateWithinCategoryOrder(categories.tests, 'test', 'test');

  // Check alphabetical ordering of members within typed extern declarations
  validateExternMemberOrder(categories.externs);
}

/**
 * Categorize declarations by type
 */
function categorizeDeclarations(decls: AST.Declaration[]) {
  return {
    externs: decls.filter(d => d.type === 'ExternDecl') as AST.ExternDecl[],
    imports: decls.filter(d => d.type === 'ImportDecl') as AST.ImportDecl[],
    types: decls.filter(d => d.type === 'TypeDecl') as AST.TypeDecl[],
    consts: decls.filter(d => d.type === 'ConstDecl') as AST.ConstDecl[],
    functions: decls.filter(d => d.type === 'FunctionDecl') as AST.FunctionDecl[],
    tests: decls.filter(d => d.type === 'TestDecl') as AST.TestDecl[],
  };
}

/**
 * Check that categories appear in the correct order
 */
function validateCategoryBoundaries(decls: AST.Declaration[]): void {
  const categoryOrder = ['TypeDecl', 'ExternDecl', 'ImportDecl', 'ConstDecl', 'FunctionDecl', 'TestDecl'];
  const categoryNames = ['type', 'extern', 'import', 'const', 'function', 'test'];
  const categorySymbols = ['t', 'e', 'i', 'c', 'λ', 'test'];

  let lastCategoryIndex = -1;

  for (const decl of decls) {
    const currentIndex = categoryOrder.indexOf(decl.type);

    if (currentIndex < lastCategoryIndex) {
      // Found a declaration out of order
      const currentCategory = categoryNames[currentIndex];
      const currentSymbol = categorySymbols[currentIndex];

      throw new CanonicalError(
        'SIGIL-CANON-DECL-CATEGORY-ORDER',
        `Canonical Ordering Error: Wrong category position\n` +
        `\n` +
        `Found: ${currentSymbol} (${currentCategory}) at line ${decl.location.start.line}\n` +
        `Expected: ${currentCategory} declarations must come before ${categoryNames[lastCategoryIndex]} declarations\n` +
        `\n` +
        `Category order: t → e → i → c → λ → test\n` +
        `  t    = types\n` +
        `  e    = externs (FFI imports)\n` +
        `  i    = imports (Sigil modules)\n` +
        `  c    = consts\n` +
        `  λ    = functions\n` +
        `  test = tests\n` +
        `\n` +
        `Move all ${currentCategory} declarations to appear before ${categoryNames[lastCategoryIndex]} declarations.\n` +
        `\n` +
        `Sigil enforces ONE way: canonical declaration ordering.`,
        decl.location,
        {
          details: { category: currentCategory, categorySymbol: currentSymbol, expectedBeforeCategory: categoryNames[lastCategoryIndex] },
          suggestions: [suggestReorderDeclaration(`move ${currentCategory} declarations before ${categoryNames[lastCategoryIndex]} declarations`, currentCategory)]
        }
      );
    }

    lastCategoryIndex = Math.max(lastCategoryIndex, currentIndex);
  }
}

/**
 * Check alphabetical ordering within a category
 * Non-exported declarations come first, then exported declarations
 */
function validateWithinCategoryOrder(
  declarations: AST.Declaration[],
  categoryName: string,
  categorySymbol: string
): void {
  if (declarations.length === 0) return;

  // Separate exported and non-exported
  const nonExported: Array<{decl: AST.Declaration, name: string}> = [];
  const exported: Array<{decl: AST.Declaration, name: string}> = [];

  for (const decl of declarations) {
    const name = extractDeclarationName(decl);
    const isExport = isExportedDeclaration(decl);

    if (isExport) {
      exported.push({decl, name});
    } else {
      nonExported.push({decl, name});
    }
  }

  // Check non-exported are alphabetical
  checkAlphabeticalOrder(nonExported, categoryName, categorySymbol, false);

  // Check exported are alphabetical
  checkAlphabeticalOrder(exported, categoryName, categorySymbol, true);

  // Check that non-exported come before exported
  if (nonExported.length > 0 && exported.length > 0) {
    const lastNonExported = nonExported[nonExported.length - 1];
    const firstExported = exported[0];

    // Find positions in original declaration list
    const nonExportedIndex = declarations.indexOf(lastNonExported.decl);
    const exportedIndex = declarations.indexOf(firstExported.decl);

    if (exportedIndex < nonExportedIndex) {
      throw new CanonicalError(
        'SIGIL-CANON-DECL-EXPORT-ORDER',
        `Canonical Ordering Error: Exports must come after non-exports\n` +
        `\n` +
        `Found: export ${categorySymbol} ${firstExported.name} at line ${firstExported.decl.location.start.line}\n` +
        `Before: ${categorySymbol} ${lastNonExported.name} at line ${lastNonExported.decl.location.start.line}\n` +
        `\n` +
        `Within each category:\n` +
        `  1. Non-exported declarations (alphabetically)\n` +
        `  2. Exported declarations (alphabetically)\n` +
        `\n` +
        `Move all exported ${categoryName} declarations to come after non-exported ones.\n` +
        `\n` +
        `Sigil enforces ONE way: canonical declaration ordering.`,
        firstExported.decl.location,
        {
          details: { category: categoryName, name: firstExported.name, before: lastNonExported.name, exported: true },
          suggestions: [suggestReorderDeclaration('move exported declarations after non-exported declarations', categoryName, firstExported.name, lastNonExported.name)]
        }
      );
    }
  }
}

/**
 * Check that a list of declarations is in alphabetical order
 */
function checkAlphabeticalOrder(
  items: Array<{decl: AST.Declaration, name: string}>,
  _categoryName: string,
  categorySymbol: string,
  isExported: boolean
): void {
  for (let i = 1; i < items.length; i++) {
    const prev = items[i - 1];
    const curr = items[i];

    if (compareNames(curr.name, prev.name) < 0) {
      // Current should come before previous - wrong order
      const exportPrefix = isExported ? 'export ' : '';

      throw new CanonicalError(
        'SIGIL-CANON-DECL-ALPHABETICAL',
        `Canonical Ordering Error: Declaration out of alphabetical order\n` +
        `\n` +
        `Found: ${exportPrefix}${categorySymbol} ${curr.name} at line ${curr.decl.location.start.line}\n` +
        `After: ${exportPrefix}${categorySymbol} ${prev.name} at line ${prev.decl.location.start.line}\n` +
        `\n` +
        `Within '${categorySymbol}' category, ${isExported ? 'exported' : 'non-exported'} declarations must be alphabetical.\n` +
        `Expected '${curr.name}' to come before '${prev.name}'.\n` +
        `\n` +
        `Alphabetical order uses Unicode code point comparison (case-sensitive).\n` +
        `Move '${curr.name}' to come before '${prev.name}'.\n` +
        `\n` +
        `Sigil enforces ONE way: strict alphabetical ordering within categories.`,
        curr.decl.location,
        {
          details: { category: categorySymbol, name: curr.name, before: prev.name, exported: isExported },
          suggestions: [suggestReorderDeclaration(`move '${curr.name}' before '${prev.name}'`, categorySymbol, curr.name, prev.name)]
        }
      );
    }
  }
}

/**
 * Validate that members within typed extern declarations are alphabetically ordered
 */
function validateExternMemberOrder(externs: AST.ExternDecl[]): void {
  for (const externDecl of externs) {
    // Skip untyped externs (no members)
    if (!externDecl.members || externDecl.members.length === 0) {
      continue;
    }

    // Check that members are alphabetically ordered
    for (let i = 1; i < externDecl.members.length; i++) {
      const prev = externDecl.members[i - 1];
      const curr = externDecl.members[i];

      if (prev.name > curr.name) {
        const moduleName = formatModulePath(externDecl.modulePath);
        throw new CanonicalError(
          'SIGIL-CANON-EXTERN-MEMBER-ORDER',
          `Canonical Ordering Error: Extern member out of alphabetical order\n` +
          `\n` +
          `Module: e ${moduleName}\n` +
          `Found: ${curr.name} at line ${curr.location.start.line}\n` +
          `After: ${prev.name} at line ${prev.location.start.line}\n` +
          `\n` +
          `Typed extern members must be alphabetically ordered.\n` +
          `Expected '${curr.name}' to come before '${prev.name}'.\n` +
          `\n` +
          `Alphabetical order uses Unicode code point comparison (case-sensitive).\n` +
          `Move '${curr.name}' to come before '${prev.name}' in the member list.\n` +
          `\n` +
          `Sigil enforces ONE way: strict alphabetical ordering for extern members.`,
          curr.location,
          {
            details: { module: moduleName, member: curr.name, before: prev.name },
            suggestions: [suggestReorderDeclaration(`move '${curr.name}' before '${prev.name}' in extern ${moduleName}`, 'e', curr.name, prev.name)]
          }
        );
      }
    }
  }
}

/**
 * Extract the sortable name from a declaration
 */
function extractDeclarationName(decl: AST.Declaration): string {
  switch (decl.type) {
    case 'ExternDecl':
    case 'ImportDecl':
      return formatModulePath(decl.modulePath);
    case 'TypeDecl':
      return decl.name;
    case 'ConstDecl':
      return decl.name;
    case 'FunctionDecl':
      return decl.name;
    case 'TestDecl':
      return decl.description;
    default:
      return '';
  }
}

/**
 * Format module path for sorting (e.g., ['fs', 'promises'] → 'fs⋅promises')
 */
function formatModulePath(path: string[]): string {
  return path.join('⋅');
}

/**
 * Check if a declaration has export modifier
 */
function isExportedDeclaration(decl: AST.Declaration): boolean {
  switch (decl.type) {
    case 'TypeDecl':
    case 'ConstDecl':
    case 'FunctionDecl':
      return decl.isExported || false;
    default:
      return false;
  }
}

/**
 * Compare two names alphabetically (Unicode code point order, case-sensitive)
 */
function compareNames(a: string, b: string): number {
  return a.localeCompare(b, 'en', { sensitivity: 'case' });
}
