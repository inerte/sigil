/**
 * Mint Type Checker - Type Environment
 *
 * Manages variable bindings and type schemes during type inference
 */

import { TypeScheme, InferenceType, Substitution, TPrimitive, TFunction, collectFreeVars, applySubstToScheme } from './types.js';

/**
 * Type environment (Γ in type theory notation)
 *
 * Maps variable names to their type schemes
 * Supports nested scopes via parent chaining
 */
export class TypeEnvironment {
  private bindings: Map<string, TypeScheme>;
  private parent?: TypeEnvironment;

  constructor(parent?: TypeEnvironment) {
    this.bindings = new Map();
    this.parent = parent;
  }

  /**
   * Look up a variable's type scheme
   *
   * Searches this environment and all parent environments
   */
  lookup(name: string): TypeScheme | undefined {
    const local = this.bindings.get(name);
    if (local) {
      return local;
    }

    // Search parent scope
    return this.parent?.lookup(name);
  }

  /**
   * Bind a variable to a type scheme
   *
   * Only affects the current scope
   */
  bind(name: string, scheme: TypeScheme): void {
    this.bindings.set(name, scheme);
  }

  /**
   * Create a child environment (for nested scopes)
   *
   * Example: when entering a lambda or let binding
   */
  extend(): TypeEnvironment {
    return new TypeEnvironment(this);
  }

  /**
   * Get all free type variables in the environment
   *
   * Used for generalization - we only generalize variables
   * that don't appear in the environment
   */
  getFreeVars(): Set<number> {
    const freeVars = new Set<number>();

    // Collect free vars from this scope
    for (const scheme of this.bindings.values()) {
      collectFreeVars(scheme.type, freeVars, scheme.quantifiedVars);
    }

    // Collect free vars from parent scopes
    if (this.parent) {
      for (const varId of this.parent.getFreeVars()) {
        freeVars.add(varId);
      }
    }

    return freeVars;
  }

  /**
   * Apply a substitution to all bindings in this environment
   *
   * This updates the environment in-place
   */
  apply(subst: Substitution): void {
    for (const [name, scheme] of this.bindings.entries()) {
      this.bindings.set(name, applySubstToScheme(subst, scheme));
    }

    // Don't apply to parent - that's a separate scope
  }

  /**
   * Get all bindings (for debugging/testing)
   */
  getBindings(): Map<string, TypeScheme> {
    return new Map(this.bindings);
  }
}

/**
 * Create the initial environment with built-in types and operators
 *
 * This includes:
 * - Primitive operators: +, -, *, /, %, ^, =, ≠, <, >, ≤, ≥
 * - Boolean operators: ∧, ∨, ¬
 * - List operators: ++
 * - Built-in functions
 */
export function createInitialEnvironment(): TypeEnvironment {
  const env = new TypeEnvironment();

  // Primitive types (just for reference - these are constructors, not values)
  // These are handled by the type checker directly

  // ========================================
  // ARITHMETIC OPERATORS
  // ========================================

  // + : ℤ → ℤ → ℤ (integer addition)
  env.bind('+', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: intType()
  }));

  // - : ℤ → ℤ → ℤ (integer subtraction)
  env.bind('-', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: intType()
  }));

  // * : ℤ → ℤ → ℤ (integer multiplication)
  env.bind('*', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: intType()
  }));

  // / : ℤ → ℤ → ℤ (integer division)
  env.bind('/', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: intType()
  }));

  // % : ℤ → ℤ → ℤ (modulo)
  env.bind('%', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: intType()
  }));

  // ^ : ℤ → ℤ → ℤ (exponentiation)
  env.bind('^', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: intType()
  }));

  // ========================================
  // COMPARISON OPERATORS
  // ========================================

  // = : ℤ → ℤ → 𝔹 (equality)
  env.bind('=', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: boolType()
  }));

  // ≠ : ℤ → ℤ → 𝔹 (inequality)
  env.bind('≠', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: boolType()
  }));

  // < : ℤ → ℤ → 𝔹
  env.bind('<', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: boolType()
  }));

  // > : ℤ → ℤ → 𝔹
  env.bind('>', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: boolType()
  }));

  // ≤ : ℤ → ℤ → 𝔹
  env.bind('≤', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: boolType()
  }));

  // ≥ : ℤ → ℤ → 𝔹
  env.bind('≥', monomorphicScheme({
    kind: 'function',
    params: [intType(), intType()],
    returnType: boolType()
  }));

  // ========================================
  // BOOLEAN OPERATORS
  // ========================================

  // ∧ : 𝔹 → 𝔹 → 𝔹 (and)
  env.bind('∧', monomorphicScheme({
    kind: 'function',
    params: [boolType(), boolType()],
    returnType: boolType()
  }));

  // ∨ : 𝔹 → 𝔹 → 𝔹 (or)
  env.bind('∨', monomorphicScheme({
    kind: 'function',
    params: [boolType(), boolType()],
    returnType: boolType()
  }));

  // ¬ : 𝔹 → 𝔹 (not)
  env.bind('¬', monomorphicScheme({
    kind: 'function',
    params: [boolType()],
    returnType: boolType()
  }));

  // ========================================
  // STRING OPERATORS
  // ========================================

  // ++ : 𝕊 → 𝕊 → 𝕊 (string concatenation)
  env.bind('++', monomorphicScheme({
    kind: 'function',
    params: [stringType(), stringType()],
    returnType: stringType()
  }));

  // TODO: Add polymorphic ++ for lists when we support ad-hoc polymorphism
  // ++ : [T] → [T] → [T] (list concatenation)

  return env;
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/**
 * Create a monomorphic type scheme (no quantified variables)
 */
function monomorphicScheme(type: InferenceType): TypeScheme {
  return {
    quantifiedVars: new Set(),
    type
  };
}

/**
 * Helper to create primitive types
 */
function intType(): TPrimitive {
  return { kind: 'primitive', name: 'Int' };
}

function floatType(): TPrimitive {
  return { kind: 'primitive', name: 'Float' };
}

function boolType(): TPrimitive {
  return { kind: 'primitive', name: 'Bool' };
}

function stringType(): TPrimitive {
  return { kind: 'primitive', name: 'String' };
}

function charType(): TPrimitive {
  return { kind: 'primitive', name: 'Char' };
}

function unitType(): TPrimitive {
  return { kind: 'primitive', name: 'Unit' };
}
