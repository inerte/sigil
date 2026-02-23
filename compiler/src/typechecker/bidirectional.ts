/**
 * Bidirectional Type Checking for Mint
 *
 * Uses two complementary modes:
 * - Synthesis (⇒): Infer type from expression structure (bottom-up)
 * - Checking (⇐): Verify expression matches expected type (top-down)
 *
 * This is simpler than Hindley-Milner because Mint requires mandatory
 * type annotations everywhere, making the inference burden much lighter.
 */

import { InferenceType, astTypeToInferenceType } from './types.js';
import { TypeEnvironment } from './environment.js';
import { TypeError, formatType } from './errors.js';
import * as AST from '../parser/ast.js';
import { checkProgramMutability, MutabilityError } from '../mutability/index.js';

/**
 * Synthesize (infer) type from expression
 * Returns the inferred type
 */
function synthesize(env: TypeEnvironment, expr: AST.Expr): InferenceType {
  switch (expr.type) {
    case 'LiteralExpr':
      return synthesizeLiteral(expr);

    case 'IdentifierExpr':
      return synthesizeIdentifier(env, expr);

    case 'ApplicationExpr':
      return synthesizeApplication(env, expr);

    case 'BinaryExpr':
      return synthesizeBinary(env, expr);

    case 'UnaryExpr':
      return synthesizeUnary(env, expr);

    case 'MatchExpr':
      return synthesizeMatch(env, expr);

    case 'ListExpr':
      return synthesizeList(env, expr);

    case 'TupleExpr':
      return synthesizeTuple(env, expr);

    case 'RecordExpr':
      return synthesizeRecord(env, expr);

    case 'FieldAccessExpr':
      return synthesizeFieldAccess(env, expr);

    case 'MemberAccessExpr':
      return synthesizeMemberAccess(env, expr);

    // List operations (language constructs)
    case 'MapExpr':
      return synthesizeMap(env, expr);

    case 'FilterExpr':
      return synthesizeFilter(env, expr);

    case 'FoldExpr':
      return synthesizeFold(env, expr);

    case 'LambdaExpr':
      return synthesizeLambda(env, expr);

    case 'IfExpr':
      return synthesizeIf(env, expr);

    case 'LetExpr':
      return synthesizeLet(env, expr);

    default:
      throw new TypeError(
        `Cannot synthesize type for ${(expr as any).type}`,
        expr.location
      );
  }
}

/**
 * Check expression against expected type
 * Throws TypeError if expression doesn't match
 */
function check(env: TypeEnvironment, expr: AST.Expr, expectedType: InferenceType): void {
  // Special case: checking against 'any' type always succeeds (FFI trust mode)
  if (expectedType.kind === 'any') {
    return;
  }

  switch (expr.type) {
    case 'LambdaExpr':
      checkLambda(env, expr, expectedType);
      return;

    case 'LiteralExpr':
      checkLiteral(expr, expectedType);
      return;

    // For most expressions: synthesize then verify equality
    default:
      const actualType = synthesize(env, expr);

      // Special case: 'any' type matches anything (FFI trust mode)
      if (actualType.kind === 'any') {
        return;
      }

      if (!typesEqual(actualType, expectedType)) {
        throw new TypeError(
          `Type mismatch: expected ${formatType(expectedType)}, got ${formatType(actualType)}`,
          expr.location
        );
      }
  }
}

/**
 * Type equality (structural)
 */
function typesEqual(t1: InferenceType, t2: InferenceType): boolean {
  if (t1.kind !== t2.kind) {
    return false;
  }

  switch (t1.kind) {
    case 'primitive':
      return t2.kind === 'primitive' && t1.name === t2.name;

    case 'function':
      if (t2.kind !== 'function') return false;
      if (t1.params.length !== t2.params.length) return false;
      for (let i = 0; i < t1.params.length; i++) {
        if (!typesEqual(t1.params[i], t2.params[i])) return false;
      }
      return typesEqual(t1.returnType, t2.returnType);

    case 'list':
      return t2.kind === 'list' && typesEqual(t1.elementType, t2.elementType);

    case 'tuple':
      if (t2.kind !== 'tuple') return false;
      if (t1.types.length !== t2.types.length) return false;
      for (let i = 0; i < t1.types.length; i++) {
        if (!typesEqual(t1.types[i], t2.types[i])) return false;
      }
      return true;

    case 'record':
      if (t2.kind !== 'record') return false;
      if (t1.fields.size !== t2.fields.size) return false;
      for (const [key, type1] of t1.fields) {
        const type2 = t2.fields.get(key);
        if (!type2 || !typesEqual(type1, type2)) return false;
      }
      return true;

    case 'constructor':
      if (t2.kind !== 'constructor') return false;
      if (t1.name !== t2.name) return false;
      if (t1.typeArgs.length !== t2.typeArgs.length) return false;
      for (let i = 0; i < t1.typeArgs.length; i++) {
        if (!typesEqual(t1.typeArgs[i], t2.typeArgs[i])) return false;
      }
      return true;

    default:
      return false;
  }
}

// ============================================================================
// SYNTHESIS FUNCTIONS
// ============================================================================

function synthesizeLiteral(expr: AST.LiteralExpr): InferenceType {
  switch (expr.literalType) {
    case 'Int':
      return { kind: 'primitive', name: 'Int' };
    case 'String':
      return { kind: 'primitive', name: 'String' };
    case 'Bool':
      return { kind: 'primitive', name: 'Bool' };
    case 'Unit':
      return { kind: 'primitive', name: 'Unit' };
    default:
      throw new TypeError(`Unknown literal type: ${expr.literalType}`, expr.location);
  }
}

function synthesizeIdentifier(env: TypeEnvironment, expr: AST.IdentifierExpr): InferenceType {
  const type = env.lookup(expr.name);
  if (!type) {
    throw new TypeError(`Unbound variable: ${expr.name}`, expr.location);
  }
  return type;
}

function synthesizeApplication(env: TypeEnvironment, expr: AST.ApplicationExpr): InferenceType {
  const fnType = synthesize(env, expr.func);

  // Special case: applying 'any' type (FFI function call)
  // No type checking - trust mode, validated at link-time
  if (fnType.kind === 'any') {
    return { kind: 'any' };
  }

  if (fnType.kind !== 'function') {
    throw new TypeError(
      `Expected function type, got ${formatType(fnType)}`,
      expr.func.location
    );
  }

  // Check argument count
  if (expr.args.length !== fnType.params.length) {
    throw new TypeError(
      `Function expects ${fnType.params.length} arguments, got ${expr.args.length}`,
      expr.location
    );
  }

  // Check each argument against parameter type
  for (let i = 0; i < expr.args.length; i++) {
    check(env, expr.args[i], fnType.params[i]);
  }

  return fnType.returnType;
}

function synthesizeBinary(env: TypeEnvironment, expr: AST.BinaryExpr): InferenceType {
  // Synthesize operand types
  const leftType = synthesize(env, expr.left);
  const rightType = synthesize(env, expr.right);

  // Determine result type based on operator
  const op = expr.operator;

  // Arithmetic operators: ℤ → ℤ → ℤ
  // Exception: + can also do string concatenation with coercion
  if (['+', '-', '*', '/', '%'].includes(op)) {
    // Special case: + with string operands does concatenation with coercion
    if (op === '+' && (leftType.kind === 'primitive' && leftType.name === 'String' ||
                        rightType.kind === 'primitive' && rightType.name === 'String')) {
      // At least one operand is a string, so this is string concatenation
      // The other operand will be coerced to string (handled by codegen)
      return { kind: 'primitive', name: 'String' };
    }

    // Otherwise, require both operands to be integers
    check(env, expr.left, { kind: 'primitive', name: 'Int' });
    check(env, expr.right, { kind: 'primitive', name: 'Int' });
    return { kind: 'primitive', name: 'Int' };
  }

  // Comparison operators: ℤ → ℤ → 𝔹
  // Support both ASCII (<= >=) and Unicode (≤ ≥) forms
  if (['<', '>', '<=', '>=', '≤', '≥'].includes(op)) {
    check(env, expr.left, { kind: 'primitive', name: 'Int' });
    check(env, expr.right, { kind: 'primitive', name: 'Int' });
    return { kind: 'primitive', name: 'Bool' };
  }

  // Equality operators: T → T → 𝔹 (polymorphic)
  // Support both ASCII (!= ) and Unicode (≠) forms
  if (['=', '!=', '≠'].includes(op)) {
    if (!typesEqual(leftType, rightType)) {
      throw new TypeError(
        `Cannot compare ${formatType(leftType)} with ${formatType(rightType)}`,
        expr.location
      );
    }
    return { kind: 'primitive', name: 'Bool' };
  }

  // Logical operators: 𝔹 → 𝔹 → 𝔹
  // Support both ASCII (&& ||) and Unicode (∧ ∨) forms
  if (['&&', '||', '∧', '∨'].includes(op)) {
    check(env, expr.left, { kind: 'primitive', name: 'Bool' });
    check(env, expr.right, { kind: 'primitive', name: 'Bool' });
    return { kind: 'primitive', name: 'Bool' };
  }

  // String concatenation: 𝕊 → 𝕊 → 𝕊
  if (op === '++') {
    check(env, expr.left, { kind: 'primitive', name: 'String' });
    check(env, expr.right, { kind: 'primitive', name: 'String' });
    return { kind: 'primitive', name: 'String' };
  }

  throw new TypeError(`Unknown operator: ${op}`, expr.location);
}

function synthesizeUnary(env: TypeEnvironment, expr: AST.UnaryExpr): InferenceType {
  switch (expr.operator) {
    case '-':
      check(env, expr.operand, { kind: 'primitive', name: 'Int' });
      return { kind: 'primitive', name: 'Int' };

    case '¬':
      check(env, expr.operand, { kind: 'primitive', name: 'Bool' });
      return { kind: 'primitive', name: 'Bool' };

    default:
      throw new TypeError(`Unknown unary operator: ${expr.operator}`, expr.location);
  }
}

function synthesizeMatch(env: TypeEnvironment, expr: AST.MatchExpr): InferenceType {
  // Synthesize scrutinee type
  const scrutineeType = synthesize(env, expr.scrutinee);

  // Process each arm
  const armTypes: InferenceType[] = [];
  for (const arm of expr.arms) {
    // Check pattern against scrutinee type, get bindings
    const bindings = checkPatternAndGetBindings(arm.pattern, scrutineeType);

    // Extend environment with bindings
    const armEnv = env.extend(bindings);

    // Synthesize arm body type
    const armType = synthesize(armEnv, arm.body);
    armTypes.push(armType);
  }

  // All arms must have same type
  for (let i = 1; i < armTypes.length; i++) {
    if (!typesEqual(armTypes[0], armTypes[i])) {
      throw new TypeError(
        `Pattern match arms have different types: ${formatType(armTypes[0])} vs ${formatType(armTypes[i])}`,
        expr.arms[i].location
      );
    }
  }

  return armTypes[0];
}

function synthesizeList(env: TypeEnvironment, expr: AST.ListExpr): InferenceType {
  if (expr.elements.length === 0) {
    // Empty list - we'd need type annotation in full system
    // For now, default to [ℤ] (this is a limitation of monomorphic system)
    throw new TypeError(
      'Cannot infer type of empty list. Please use type annotation.',
      expr.location
    );
  }

  // Synthesize first element type
  const firstType = synthesize(env, expr.elements[0]);

  // Check all other elements have same type
  for (let i = 1; i < expr.elements.length; i++) {
    check(env, expr.elements[i], firstType);
  }

  return { kind: 'list', elementType: firstType };
}

function synthesizeTuple(env: TypeEnvironment, expr: AST.TupleExpr): InferenceType {
  const types = expr.elements.map(elem => synthesize(env, elem));
  return { kind: 'tuple', types };
}

function synthesizeRecord(env: TypeEnvironment, expr: AST.RecordExpr): InferenceType {
  const fields = new Map<string, InferenceType>();
  for (const field of expr.fields) {
    fields.set(field.name, synthesize(env, field.value));
  }
  return { kind: 'record', fields };
}

function synthesizeFieldAccess(env: TypeEnvironment, expr: AST.FieldAccessExpr): InferenceType {
  const objType = synthesize(env, expr.object);

  // Special case: field access on 'any' type (FFI namespace)
  // This happens when accessing extern namespace members like console.log
  if (objType.kind === 'any') {
    // Return any type - member validation happens at link-time
    return { kind: 'any' };
  }

  if (objType.kind !== 'record') {
    throw new TypeError(
      `Cannot access field on non-record type ${formatType(objType)}`,
      expr.location
    );
  }

  const fieldType = objType.fields.get(expr.field);
  if (!fieldType) {
    throw new TypeError(
      `Record does not have field '${expr.field}'`,
      expr.location
    );
  }

  return fieldType;
}

function synthesizeMemberAccess(env: TypeEnvironment, expr: AST.MemberAccessExpr): InferenceType {
  const namespaceName = expr.namespace.join('/');

  // Check namespace exists (should be registered from extern declaration)
  const namespaceType = env.lookup(namespaceName);
  if (!namespaceType) {
    throw new TypeError(
      `Unknown namespace '${namespaceName}'. Did you forget 'e ${namespaceName}'?`,
      expr.location
    );
  }

  // Return any type for member access
  // Actual validation happens at link-time (extern-validator.ts)
  return { kind: 'any' };
}

function synthesizeMap(env: TypeEnvironment, expr: AST.MapExpr): InferenceType {
  const listType = synthesize(env, expr.list);

  if (listType.kind !== 'list') {
    throw new TypeError(
      `Map (↦) requires a list, got ${formatType(listType)}`,
      expr.location
    );
  }

  const fnType = synthesize(env, expr.fn);

  if (fnType.kind !== 'function') {
    throw new TypeError(
      `Map (↦) requires a function, got ${formatType(fnType)}`,
      expr.location
    );
  }

  // Function should take element type and return some type
  if (fnType.params.length !== 1) {
    throw new TypeError(
      `Map (↦) function should take 1 parameter, got ${fnType.params.length}`,
      expr.location
    );
  }

  // Check function parameter matches list element type
  if (!typesEqual(fnType.params[0], listType.elementType)) {
    throw new TypeError(
      `Map (↦) function parameter type ${formatType(fnType.params[0])} doesn't match list element type ${formatType(listType.elementType)}`,
      expr.location
    );
  }

  // Result is list of return type
  return { kind: 'list', elementType: fnType.returnType };
}

function synthesizeFilter(env: TypeEnvironment, expr: AST.FilterExpr): InferenceType {
  const listType = synthesize(env, expr.list);

  if (listType.kind !== 'list') {
    throw new TypeError(
      `Filter (⊳) requires a list, got ${formatType(listType)}`,
      expr.location
    );
  }

  const predicateType = synthesize(env, expr.predicate);

  if (predicateType.kind !== 'function') {
    throw new TypeError(
      `Filter (⊳) requires a predicate function, got ${formatType(predicateType)}`,
      expr.location
    );
  }

  // Predicate should be T → 𝔹
  if (predicateType.params.length !== 1) {
    throw new TypeError(
      `Filter (⊳) predicate should take 1 parameter, got ${predicateType.params.length}`,
      expr.location
    );
  }

  if (!typesEqual(predicateType.params[0], listType.elementType)) {
    throw new TypeError(
      `Filter (⊳) predicate parameter type ${formatType(predicateType.params[0])} doesn't match list element type ${formatType(listType.elementType)}`,
      expr.location
    );
  }

  if (!typesEqual(predicateType.returnType, { kind: 'primitive', name: 'Bool' })) {
    throw new TypeError(
      `Filter (⊳) predicate must return 𝔹, got ${formatType(predicateType.returnType)}`,
      expr.location
    );
  }

  // Result is same list type
  return listType;
}

function synthesizeFold(env: TypeEnvironment, expr: AST.FoldExpr): InferenceType {
  const listType = synthesize(env, expr.list);

  if (listType.kind !== 'list') {
    throw new TypeError(
      `Fold (⊕) requires a list, got ${formatType(listType)}`,
      expr.location
    );
  }

  const fnType = synthesize(env, expr.fn);

  if (fnType.kind !== 'function') {
    throw new TypeError(
      `Fold (⊕) requires a function, got ${formatType(fnType)}`,
      expr.location
    );
  }

  // Function should be (Acc, T) → Acc
  if (fnType.params.length !== 2) {
    throw new TypeError(
      `Fold (⊕) function should take 2 parameters, got ${fnType.params.length}`,
      expr.location
    );
  }

  const initType = synthesize(env, expr.init);

  // Check function signature matches (Acc, T) → Acc
  if (!typesEqual(fnType.params[0], initType)) {
    throw new TypeError(
      `Fold (⊕) function first parameter type ${formatType(fnType.params[0])} doesn't match initial value type ${formatType(initType)}`,
      expr.location
    );
  }

  if (!typesEqual(fnType.params[1], listType.elementType)) {
    throw new TypeError(
      `Fold (⊕) function second parameter type ${formatType(fnType.params[1])} doesn't match list element type ${formatType(listType.elementType)}`,
      expr.location
    );
  }

  if (!typesEqual(fnType.returnType, initType)) {
    throw new TypeError(
      `Fold (⊕) function return type ${formatType(fnType.returnType)} doesn't match accumulator type ${formatType(initType)}`,
      expr.location
    );
  }

  return initType;
}

function synthesizeLambda(env: TypeEnvironment, expr: AST.LambdaExpr): InferenceType {
  // Lambda has mandatory type annotations (enforced by parser)
  const paramTypes = expr.params.map(p => astTypeToInferenceType(p.typeAnnotation!));
  const returnType = astTypeToInferenceType(expr.returnType);

  // Check body against declared return type
  const bodyEnv = env.extend(
    new Map(expr.params.map((p, i) => [p.name, paramTypes[i]]))
  );
  check(bodyEnv, expr.body, returnType);

  return {
    kind: 'function',
    params: paramTypes,
    returnType
  };
}

function synthesizeIf(env: TypeEnvironment, expr: AST.IfExpr): InferenceType {
  // Check condition is boolean
  check(env, expr.condition, { kind: 'primitive', name: 'Bool' });

  // Synthesize then branch
  const thenType = synthesize(env, expr.thenBranch);

  // If no else branch, then branch must be Unit
  if (!expr.elseBranch) {
    if (!typesEqual(thenType, { kind: 'primitive', name: 'Unit' })) {
      throw new TypeError(
        `If expression without else must have Unit type, got ${formatType(thenType)}`,
        expr.location
      );
    }
    return thenType;
  }

  // Synthesize else branch
  const elseType = synthesize(env, expr.elseBranch);

  // Both branches must have same type
  if (!typesEqual(thenType, elseType)) {
    throw new TypeError(
      `If branches have different types: then is ${formatType(thenType)}, else is ${formatType(elseType)}`,
      expr.location
    );
  }

  return thenType;
}

function synthesizeLet(env: TypeEnvironment, expr: AST.LetExpr): InferenceType {
  // Synthesize binding value type
  const valueType = synthesize(env, expr.value);

  // Check pattern and get bindings
  const bindings = new Map<string, InferenceType>();
  checkPattern(expr.pattern, valueType, bindings);

  // Extend environment and synthesize body
  const bodyEnv = env.extend(bindings);
  return synthesize(bodyEnv, expr.body);
}

// ============================================================================
// CHECKING FUNCTIONS
// ============================================================================

function checkLambda(env: TypeEnvironment, expr: AST.LambdaExpr, expectedType: InferenceType): void {
  if (expectedType.kind !== 'function') {
    throw new TypeError(
      `Expected ${formatType(expectedType)}, but lambda needs function type`,
      expr.location
    );
  }

  // Lambda must have type annotations (enforced by parser)
  const paramTypes = expr.params.map(p => astTypeToInferenceType(p.typeAnnotation!));
  const returnType = astTypeToInferenceType(expr.returnType);

  // Verify annotations match expected type
  if (paramTypes.length !== expectedType.params.length) {
    throw new TypeError(
      `Lambda parameter count mismatch: expected ${expectedType.params.length}, got ${paramTypes.length}`,
      expr.location
    );
  }

  for (let i = 0; i < paramTypes.length; i++) {
    if (!typesEqual(paramTypes[i], expectedType.params[i])) {
      throw new TypeError(
        `Lambda parameter ${i} type mismatch: expected ${formatType(expectedType.params[i])}, got ${formatType(paramTypes[i])}`,
        expr.params[i].location
      );
    }
  }

  if (!typesEqual(returnType, expectedType.returnType)) {
    throw new TypeError(
      `Lambda return type mismatch: expected ${formatType(expectedType.returnType)}, got ${formatType(returnType)}`,
      expr.location
    );
  }

  // Check body against declared return type
  const bodyEnv = env.extend(
    new Map(expr.params.map((p, i) => [p.name, paramTypes[i]]))
  );
  check(bodyEnv, expr.body, returnType);
}

function checkLiteral(expr: AST.LiteralExpr, expectedType: InferenceType): void {
  const actualType = synthesizeLiteral(expr);
  if (!typesEqual(actualType, expectedType)) {
    throw new TypeError(
      `Literal type mismatch: expected ${formatType(expectedType)}, got ${formatType(actualType)}`,
      expr.location
    );
  }
}

// ============================================================================
// PATTERN CHECKING
// ============================================================================

function checkPatternAndGetBindings(
  pattern: AST.Pattern,
  scrutineeType: InferenceType
): Map<string, InferenceType> {
  const bindings = new Map<string, InferenceType>();

  checkPattern(pattern, scrutineeType, bindings);

  return bindings;
}

function checkPattern(
  pattern: AST.Pattern,
  scrutineeType: InferenceType,
  bindings: Map<string, InferenceType>
): void {
  switch (pattern.type) {
    case 'WildcardPattern':
      // Wildcard matches anything
      return;

    case 'IdentifierPattern':
      // Bind variable to scrutinee type
      bindings.set(pattern.name, scrutineeType);
      return;

    case 'LiteralPattern':
      // Check literal type matches scrutinee
      const litType: InferenceType = {
        kind: 'primitive',
        name: pattern.literalType
      };
      if (!typesEqual(litType, scrutineeType)) {
        throw new TypeError(
          `Pattern type mismatch: expected ${formatType(scrutineeType)}, got ${formatType(litType)}`,
          pattern.location
        );
      }
      return;

    case 'ListPattern':
      if (scrutineeType.kind !== 'list') {
        throw new TypeError(
          `List pattern requires list type, got ${formatType(scrutineeType)}`,
          pattern.location
        );
      }

      // Check each element pattern
      for (const elem of pattern.patterns) {
        // Regular pattern gets element type
        checkPattern(elem, scrutineeType.elementType, bindings);
      }

      // Handle rest pattern if present
      if (pattern.rest) {
        bindings.set(pattern.rest, scrutineeType);
      }
      return;

    case 'TuplePattern':
      if (scrutineeType.kind !== 'tuple') {
        throw new TypeError(
          `Tuple pattern requires tuple type, got ${formatType(scrutineeType)}`,
          pattern.location
        );
      }

      if (pattern.patterns.length !== scrutineeType.types.length) {
        throw new TypeError(
          `Tuple pattern has ${pattern.patterns.length} elements, but type has ${scrutineeType.types.length}`,
          pattern.location
        );
      }

      for (let i = 0; i < pattern.patterns.length; i++) {
        checkPattern(pattern.patterns[i], scrutineeType.types[i], bindings);
      }
      return;

    case 'ConstructorPattern':
      if (scrutineeType.kind !== 'constructor') {
        throw new TypeError(
          `Constructor pattern requires constructor type, got ${formatType(scrutineeType)}`,
          pattern.location
        );
      }

      if (pattern.name !== scrutineeType.name) {
        throw new TypeError(
          `Constructor pattern '${pattern.name}' doesn't match type '${scrutineeType.name}'`,
          pattern.location
        );
      }

      // Check argument patterns
      if (pattern.patterns && scrutineeType.typeArgs.length > 0) {
        if (pattern.patterns.length !== scrutineeType.typeArgs.length) {
          throw new TypeError(
            `Constructor pattern has ${pattern.patterns.length} arguments, but type has ${scrutineeType.typeArgs.length}`,
            pattern.location
          );
        }

        for (let i = 0; i < pattern.patterns.length; i++) {
          checkPattern(pattern.patterns[i], scrutineeType.typeArgs[i], bindings);
        }
      }
      return;

    default:
      throw new TypeError(
        `Unknown pattern type: ${(pattern as any).type}`,
        pattern.location
      );
  }
}

// ============================================================================
// PUBLIC API
// ============================================================================

/**
 * Type check a program
 * Returns map of function names to their types
 */
export function typeCheck(program: AST.Program, _source: string): Map<string, InferenceType> {
  const env = TypeEnvironment.createInitialEnvironment();
  const types = new Map<string, InferenceType>();

  // First pass: Add all function declarations and extern namespaces to environment
  // (for mutual recursion support and FFI)
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      const params = decl.params.map(p => astTypeToInferenceType(p.typeAnnotation!));
      const returnType = astTypeToInferenceType(decl.returnType!);
      const funcType: InferenceType = {
        kind: 'function',
        params,
        returnType
      };
      env.bind(decl.name, funcType);
      types.set(decl.name, funcType);
    } else if (decl.type === 'ExternDecl') {
      // Register namespace as "any" type (trust mode)
      // Member validation happens at link-time, not type-check time
      const namespaceName = decl.modulePath.join('/');
      const anyType: InferenceType = { kind: 'any' };
      env.bind(namespaceName, anyType);
    }
  }

  // Second pass: Check function bodies
  for (const decl of program.declarations) {
    if (decl.type === 'FunctionDecl') {
      checkFunctionDecl(env, decl);
    } else if (decl.type === 'ConstDecl') {
      checkConstDecl(env, decl, types);
    }
    // TypeDecl doesn't need runtime checking
  }

  // Third pass: Check mutability constraints
  try {
    checkProgramMutability(program);
  } catch (error) {
    if (error instanceof MutabilityError && _source) {
      console.error(error.format(_source));
    }
    throw error;
  }

  return types;
}

function checkFunctionDecl(env: TypeEnvironment, decl: AST.FunctionDecl): void {
  // All type annotations are mandatory (enforced by parser)
  const paramTypes = decl.params.map(p => astTypeToInferenceType(p.typeAnnotation!));
  const returnType = astTypeToInferenceType(decl.returnType!);

  // Add parameters to environment
  const bodyEnv = env.extend(
    new Map(decl.params.map((p, i) => [p.name, paramTypes[i]]))
  );

  // Check body against declared return type
  check(bodyEnv, decl.body, returnType);
}

function checkConstDecl(env: TypeEnvironment, decl: AST.ConstDecl, types: Map<string, InferenceType>): void {
  // Type annotation is mandatory (enforced by parser)
  const annotatedType = astTypeToInferenceType(decl.typeAnnotation!);

  // Synthesize value type
  const valueType = synthesize(env, decl.value);

  // Check they match
  if (!typesEqual(valueType, annotatedType)) {
    throw new TypeError(
      `Const declaration type mismatch: declared as ${formatType(annotatedType)}, but value has type ${formatType(valueType)}`,
      decl.location
    );
  }

  // Add to environment
  env.bind(decl.name, annotatedType);
  types.set(decl.name, annotatedType);
}
