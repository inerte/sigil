/**
 * Mint Type Checker - Type Inference Engine
 *
 * Implements Algorithm W (Hindley-Milner type inference)
 * This is the main engine that infers types for all expressions
 */

import * as AST from '../parser/ast.js';
import {
  InferenceType,
  TVar,
  TPrimitive,
  TypeScheme,
  Substitution,
  applySubst,
  composeSubstitutions,
  collectFreeVars,
  astTypeToInferenceType
} from './types.js';
import { TypeEnvironment, createInitialEnvironment } from './environment.js';
import { unify } from './unification.js';
import { TypeError } from './errors.js';

/**
 * Type Inference Engine
 *
 * Implements Algorithm W for type inference
 */
export class TypeInferenceEngine {
  private nextVarId = 0;

  /**
   * Create a fresh type variable
   */
  freshVar(name?: string): TVar {
    return {
      kind: 'var',
      id: this.nextVarId++,
      name
    };
  }

  /**
   * Infer the type of an expression
   *
   * Returns [substitution, type]
   */
  infer(env: TypeEnvironment, expr: AST.Expr): [Substitution, InferenceType] {
    switch (expr.type) {
      case 'LiteralExpr':
        return this.inferLiteral(expr);

      case 'IdentifierExpr':
        return this.inferIdentifier(env, expr);

      case 'LambdaExpr':
        return this.inferLambda(env, expr);

      case 'ApplicationExpr':
        return this.inferApplication(env, expr);

      case 'BinaryExpr':
        return this.inferBinary(env, expr);

      case 'ListExpr':
        return this.inferList(env, expr);

      case 'TupleExpr':
        return this.inferTuple(env, expr);

      case 'MatchExpr':
        // TODO: Implement in Phase 4
        throw new Error('Pattern matching not yet implemented');

      case 'MapExpr':
        return this.inferMapOp(env, expr);

      case 'FilterExpr':
        return this.inferFilterOp(env, expr);

      case 'FoldExpr':
        return this.inferFoldOp(env, expr);

      default:
        throw new Error(`Unknown expression type: ${(expr as any).type}`);
    }
  }

  /**
   * Infer the type of a literal
   */
  private inferLiteral(expr: AST.LiteralExpr): [Substitution, InferenceType] {
    let type: TPrimitive;

    switch (expr.literalType) {
      case 'Int':
        type = { kind: 'primitive', name: 'Int' };
        break;

      case 'Float':
        type = { kind: 'primitive', name: 'Float' };
        break;

      case 'Bool':
        type = { kind: 'primitive', name: 'Bool' };
        break;

      case 'String':
        type = { kind: 'primitive', name: 'String' };
        break;

      case 'Char':
        type = { kind: 'primitive', name: 'Char' };
        break;

      case 'Unit':
        type = { kind: 'primitive', name: 'Unit' };
        break;

      default:
        throw new Error(`Unknown literal type: ${(expr as any).literalType}`);
    }

    return [new Map(), type];
  }

  /**
   * Infer the type of an identifier (variable lookup)
   */
  private inferIdentifier(
    env: TypeEnvironment,
    expr: AST.IdentifierExpr
  ): [Substitution, InferenceType] {
    const scheme = env.lookup(expr.name);

    if (!scheme) {
      throw new TypeError(
        `Undefined variable: ${expr.name}`,
        expr.location
      );
    }

    // Instantiate the type scheme with fresh type variables
    const type = this.instantiate(scheme);

    return [new Map(), type];
  }

  /**
   * Infer the type of a lambda expression
   */
  private inferLambda(
    env: TypeEnvironment,
    expr: AST.LambdaExpr
  ): [Substitution, InferenceType] {
    // Create a new environment for the lambda body
    const lambdaEnv = env.extend();

    // Process parameters
    const paramTypes: InferenceType[] = [];

    for (const param of expr.params) {
      // If parameter has type annotation, use it
      // Otherwise, create fresh type variable
      const paramType = param.typeAnnotation
        ? astTypeToInferenceType(param.typeAnnotation)
        : this.freshVar(param.name);

      paramTypes.push(paramType);

      // Bind parameter in lambda environment
      lambdaEnv.bind(param.name, {
        quantifiedVars: new Set(),
        type: paramType
      });
    }

    // Infer body type
    const [bodySubst, bodyType] = this.infer(lambdaEnv, expr.body);

    // Apply substitution to parameter types
    const finalParamTypes = paramTypes.map(t => applySubst(bodySubst, t));

    // Note: Lambda expressions in the AST don't have return type annotations
    // Return types are only on function declarations
    const returnType = bodyType;
    const subst = bodySubst;

    // Build function type
    const functionType: InferenceType = {
      kind: 'function',
      params: finalParamTypes.map(t => applySubst(subst, t)),
      returnType: applySubst(subst, returnType)
    };

    return [subst, functionType];
  }

  /**
   * Infer the type of a function application
   */
  private inferApplication(
    env: TypeEnvironment,
    expr: AST.ApplicationExpr
  ): [Substitution, InferenceType] {
    // Infer function type
    const [funcSubst, funcType] = this.infer(env, expr.func);

    // Infer argument types
    let subst = funcSubst;
    const argTypes: InferenceType[] = [];

    for (const arg of expr.args) {
      const [argSubst, argType] = this.infer(env, arg);
      subst = composeSubstitutions(subst, argSubst);
      argTypes.push(applySubst(subst, argType));
    }

    // Create fresh type variable for result
    const resultType = this.freshVar();

    // Expected function type: (arg1, arg2, ...) → result
    const expectedFuncType: InferenceType = {
      kind: 'function',
      params: argTypes,
      returnType: resultType
    };

    // Unify actual function type with expected
    const unificationSubst = unify(
      applySubst(subst, funcType),
      expectedFuncType,
      expr.location
    );

    const finalSubst = composeSubstitutions(subst, unificationSubst);
    const finalResultType = applySubst(unificationSubst, resultType);

    return [finalSubst, finalResultType];
  }

  /**
   * Infer the type of a binary expression
   *
   * Binary operators are looked up in the environment and treated as functions
   */
  private inferBinary(
    env: TypeEnvironment,
    expr: AST.BinaryExpr
  ): [Substitution, InferenceType] {
    // Look up operator as a function
    const operatorScheme = env.lookup(expr.operator);

    if (!operatorScheme) {
      throw new TypeError(
        `Undefined operator: ${expr.operator}`,
        expr.location
      );
    }

    // Instantiate operator type
    const operatorType = this.instantiate(operatorScheme);

    // Infer left and right operand types
    const [leftSubst, leftType] = this.infer(env, expr.left);
    const [rightSubst, rightType] = this.infer(env, expr.right);

    const subst = composeSubstitutions(leftSubst, rightSubst);

    // Operator must be a binary function: T → U → V
    if (operatorType.kind !== 'function' || operatorType.params.length !== 2) {
      throw new TypeError(
        `Operator ${expr.operator} is not a binary operator`,
        expr.location
      );
    }

    // Unify left operand with first parameter
    const leftUnifySubst = unify(
      applySubst(subst, leftType),
      operatorType.params[0],
      expr.left.location
    );

    const subst2 = composeSubstitutions(subst, leftUnifySubst);

    // Unify right operand with second parameter
    const rightUnifySubst = unify(
      applySubst(subst2, rightType),
      applySubst(leftUnifySubst, operatorType.params[1]),
      expr.right.location
    );

    const finalSubst = composeSubstitutions(subst2, rightUnifySubst);

    // Result type is the operator's return type
    const resultType = applySubst(finalSubst, operatorType.returnType);

    return [finalSubst, resultType];
  }

  /**
   * Infer the type of a list expression
   */
  private inferList(
    env: TypeEnvironment,
    expr: AST.ListExpr
  ): [Substitution, InferenceType] {
    if (expr.elements.length === 0) {
      // Empty list has type [α] for fresh α
      return [new Map(), { kind: 'list', elementType: this.freshVar() }];
    }

    // Infer type of first element
    const [firstSubst, firstType] = this.infer(env, expr.elements[0]);
    let subst = firstSubst;
    let elemType = firstType;

    // Unify all elements with the first element's type
    for (let i = 1; i < expr.elements.length; i++) {
      const [elemSubst, currentElemType] = this.infer(env, expr.elements[i]);
      subst = composeSubstitutions(subst, elemSubst);

      const unifySubst = unify(
        applySubst(subst, elemType),
        currentElemType,
        expr.elements[i].location
      );

      subst = composeSubstitutions(subst, unifySubst);
      elemType = applySubst(subst, elemType);
    }

    return [subst, { kind: 'list', elementType: elemType }];
  }

  /**
   * Infer the type of a tuple expression
   */
  private inferTuple(
    env: TypeEnvironment,
    expr: AST.TupleExpr
  ): [Substitution, InferenceType] {
    let subst: Substitution = new Map();
    const types: InferenceType[] = [];

    for (const elem of expr.elements) {
      const [elemSubst, elemType] = this.infer(env, elem);
      subst = composeSubstitutions(subst, elemSubst);
      types.push(applySubst(subst, elemType));
    }

    return [subst, { kind: 'tuple', types }];
  }

  /**
   * Infer the type of map operator (↦)
   *
   * list↦fn  where fn: T → U  produces [U]
   */
  private inferMapOp(
    env: TypeEnvironment,
    expr: AST.MapExpr
  ): [Substitution, InferenceType] {
    // Infer list type
    const [listSubst, listType] = this.infer(env, expr.list);

    // listType must be [T]
    const elemType = this.freshVar();
    const expectedListType: InferenceType = {
      kind: 'list',
      elementType: elemType
    };

    const listUnifySubst = unify(
      applySubst(listSubst, listType),
      expectedListType,
      expr.list.location
    );

    const subst1 = composeSubstitutions(listSubst, listUnifySubst);

    // Infer function type
    const [fnSubst, fnType] = this.infer(env, expr.fn);
    const subst2 = composeSubstitutions(subst1, fnSubst);

    // fnType must be T → U
    const resultType = this.freshVar();
    const expectedFnType: InferenceType = {
      kind: 'function',
      params: [applySubst(subst2, elemType)],
      returnType: resultType
    };

    const fnUnifySubst = unify(
      applySubst(subst2, fnType),
      expectedFnType,
      expr.fn.location
    );

    const finalSubst = composeSubstitutions(subst2, fnUnifySubst);

    // Result is [U]
    return [
      finalSubst,
      { kind: 'list', elementType: applySubst(finalSubst, resultType) }
    ];
  }

  /**
   * Infer the type of filter operator (⊳)
   *
   * list⊳predicate  where predicate: T → 𝔹  produces [T]
   */
  private inferFilterOp(
    env: TypeEnvironment,
    expr: AST.FilterExpr
  ): [Substitution, InferenceType] {
    // Infer list type
    const [listSubst, listType] = this.infer(env, expr.list);

    // listType must be [T]
    const elemType = this.freshVar();
    const expectedListType: InferenceType = {
      kind: 'list',
      elementType: elemType
    };

    const listUnifySubst = unify(
      applySubst(listSubst, listType),
      expectedListType,
      expr.list.location
    );

    const subst1 = composeSubstitutions(listSubst, listUnifySubst);

    // Infer predicate type
    const [predSubst, predType] = this.infer(env, expr.predicate);
    const subst2 = composeSubstitutions(subst1, predSubst);

    // predType must be T → 𝔹
    const expectedPredType: InferenceType = {
      kind: 'function',
      params: [applySubst(subst2, elemType)],
      returnType: { kind: 'primitive', name: 'Bool' }
    };

    const predUnifySubst = unify(
      applySubst(subst2, predType),
      expectedPredType,
      expr.predicate.location
    );

    const finalSubst = composeSubstitutions(subst2, predUnifySubst);

    // Result is [T] (same as input)
    return [
      finalSubst,
      { kind: 'list', elementType: applySubst(finalSubst, elemType) }
    ];
  }

  /**
   * Infer the type of fold operator (⊕)
   *
   * list⊕fn⊕init  where fn: (Acc, T) → Acc  produces Acc
   */
  private inferFoldOp(
    env: TypeEnvironment,
    expr: AST.FoldExpr
  ): [Substitution, InferenceType] {
    // Infer list type
    const [listSubst, listType] = this.infer(env, expr.list);

    // listType must be [T]
    const elemType = this.freshVar();
    const expectedListType: InferenceType = {
      kind: 'list',
      elementType: elemType
    };

    const listUnifySubst = unify(
      applySubst(listSubst, listType),
      expectedListType,
      expr.list.location
    );

    const subst1 = composeSubstitutions(listSubst, listUnifySubst);

    // Infer initial value type
    const [initSubst, initType] = this.infer(env, expr.init);
    const subst2 = composeSubstitutions(subst1, initSubst);

    // Infer function type
    const [fnSubst, fnType] = this.infer(env, expr.fn);
    const subst3 = composeSubstitutions(subst2, fnSubst);

    // fnType must be (Acc, T) → Acc
    const expectedFnType: InferenceType = {
      kind: 'function',
      params: [
        applySubst(subst3, initType),  // Accumulator type
        applySubst(subst3, elemType)   // Element type
      ],
      returnType: applySubst(subst3, initType)  // Returns accumulator type
    };

    const fnUnifySubst = unify(
      applySubst(subst3, fnType),
      expectedFnType,
      expr.fn.location
    );

    const finalSubst = composeSubstitutions(subst3, fnUnifySubst);

    // Result is Acc (same as init type)
    return [finalSubst, applySubst(finalSubst, initType)];
  }

  /**
   * Generalize a type into a type scheme
   *
   * Quantifies all free type variables that don't appear in the environment
   */
  private generalize(env: TypeEnvironment, type: InferenceType): TypeScheme {
    const envFreeVars = env.getFreeVars();
    const typeFreeVars = collectFreeVars(type);

    // Quantify variables that are free in the type but not in the environment
    const quantifiedVars = new Set<number>();
    for (const varId of typeFreeVars) {
      if (!envFreeVars.has(varId)) {
        quantifiedVars.add(varId);
      }
    }

    return {
      quantifiedVars,
      type
    };
  }

  /**
   * Instantiate a type scheme with fresh type variables
   *
   * Replaces all quantified variables with fresh ones
   */
  private instantiate(scheme: TypeScheme): InferenceType {
    if (scheme.quantifiedVars.size === 0) {
      return scheme.type;
    }

    // Create fresh variables for each quantified variable
    const subst = new Map<number, InferenceType>();
    for (const varId of scheme.quantifiedVars) {
      subst.set(varId, this.freshVar());
    }

    return applySubst(subst, scheme.type);
  }

  /**
   * Type check a program
   *
   * Returns a map of function names to their inferred type schemes
   */
  inferProgram(program: AST.Program): Map<string, TypeScheme> {
    const env = createInitialEnvironment();
    const types = new Map<string, TypeScheme>();

    // Two-pass approach for recursive functions:
    // Pass 1: Add all function declarations with placeholder types
    for (const decl of program.declarations) {
      if (decl.type === 'FunctionDecl') {
        // Get parameter types (from annotations or fresh vars)
        const paramTypes = decl.params.map(p =>
          p.typeAnnotation
            ? astTypeToInferenceType(p.typeAnnotation)
            : this.freshVar(p.name)
        );

        // Get return type (from annotation or fresh var)
        const returnType = decl.returnType
          ? astTypeToInferenceType(decl.returnType)
          : this.freshVar();

        // Create function type
        const funcType: InferenceType = {
          kind: 'function',
          params: paramTypes,
          returnType
        };

        // Bind function in environment (not generalized yet)
        env.bind(decl.name, {
          quantifiedVars: new Set(),
          type: funcType
        });
      }
    }

    // Pass 2: Infer function bodies and check consistency
    for (const decl of program.declarations) {
      if (decl.type === 'FunctionDecl') {
        // Get the function type from environment
        const declaredScheme = env.lookup(decl.name)!;
        const declaredType = declaredScheme.type as any;

        // Create environment for function body
        const funcEnv = env.extend();

        // Bind parameters
        for (let i = 0; i < decl.params.length; i++) {
          funcEnv.bind(decl.params[i].name, {
            quantifiedVars: new Set(),
            type: declaredType.params[i]
          });
        }

        // Infer body type
        const [bodySubst, bodyType] = this.infer(funcEnv, decl.body);

        // Unify with declared return type
        const returnUnifySubst = unify(
          applySubst(bodySubst, bodyType),
          applySubst(bodySubst, declaredType.returnType),
          decl.body.location
        );

        const finalSubst = composeSubstitutions(bodySubst, returnUnifySubst);

        // Apply substitution to get final function type
        const finalType = applySubst(finalSubst, declaredType);

        // Generalize the function type
        const scheme = this.generalize(env, finalType);

        // Update environment with generalized type
        env.bind(decl.name, scheme);
        types.set(decl.name, scheme);
      }
    }

    return types;
  }
}
