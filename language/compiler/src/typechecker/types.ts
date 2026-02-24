/**
 * Mint Type Checker - Core Type System
 *
 * Internal type representations used during type inference.
 * These are distinct from AST types and optimized for unification and substitution.
 */

import * as AST from '../parser/ast.js';

// ============================================================================
// INFERENCE TYPES
// ============================================================================

/**
 * Internal type representation for type inference
 *
 * This is separate from AST.Type to allow for features like:
 * - Type variables with instance tracking (for unification)
 * - Efficient substitution
 * - Type schemes for polymorphism
 */
export type InferenceType =
  | TPrimitive
  | TVar
  | TFunction
  | TList
  | TTuple
  | TRecord
  | TConstructor
  | TAny;

export interface TPrimitive {
  kind: 'primitive';
  name: 'Int' | 'Float' | 'Bool' | 'String' | 'Char' | 'Unit';
}

export interface TVar {
  kind: 'var';
  id: number;                    // Unique type variable ID
  name?: string;                 // Optional name for display (α, β, T, U)
  instance?: InferenceType;      // For unification - points to actual type when unified
}

export interface TFunction {
  kind: 'function';
  params: InferenceType[];
  returnType: InferenceType;
  effects?: EffectSet;           // Future: effect tracking (!IO, !Network, etc.)
}

export interface TList {
  kind: 'list';
  elementType: InferenceType;
}

export interface TTuple {
  kind: 'tuple';
  types: InferenceType[];
}

export interface TRecord {
  kind: 'record';
  fields: Map<string, InferenceType>;
  name?: string;                 // For user-defined types
}

export interface TConstructor {
  kind: 'constructor';
  name: string;                  // e.g., "Option", "Result", "Maybe"
  typeArgs: InferenceType[];     // Generic type arguments
}

export interface TAny {
  kind: 'any';
  // Used for FFI namespaces - trust mode, validated at link-time
}

// ============================================================================
// TYPE SCHEMES (Polymorphism)
// ============================================================================

/**
 * Type scheme for polymorphic types (∀α₁...αₙ.τ)
 *
 * Example:
 *   identity : ∀T. T → T
 *   map : ∀T,U. (T → U) → [T] → [U]
 */
export interface TypeScheme {
  quantifiedVars: Set<number>;  // Type variable IDs that are quantified
  type: InferenceType;           // The actual type with quantified variables
}

// ============================================================================
// SUBSTITUTIONS
// ============================================================================

/**
 * Type substitution (mapping from type variables to types)
 *
 * Example: [α₁ ↦ Int, α₂ ↦ String]
 */
export type Substitution = Map<number, InferenceType>;

// ============================================================================
// EFFECTS (Future)
// ============================================================================

/**
 * Effect system for tracking side effects
 * Tracks compile-time effects for function calls
 */
export type EffectSet = Set<'IO' | 'Network' | 'Async' | 'Error' | 'Mut'>;

// ============================================================================
// TYPE UTILITIES
// ============================================================================

/**
 * Apply a substitution to a type
 *
 * Recursively replaces type variables with their substituted types
 */
export function applySubst(subst: Substitution, type: InferenceType): InferenceType {
  switch (type.kind) {
    case 'primitive':
      return type;

    case 'var':
      // If this variable has a substitution, use it
      const substType = subst.get(type.id);
      if (substType) {
        // Recursively apply substitution (in case the substitution contains more variables)
        return applySubst(subst, substType);
      }
      // If variable has an instance (from unification), follow it
      if (type.instance) {
        return applySubst(subst, type.instance);
      }
      return type;

    case 'function':
      return {
        kind: 'function',
        params: type.params.map(p => applySubst(subst, p)),
        returnType: applySubst(subst, type.returnType),
        effects: type.effects
      };

    case 'list':
      return {
        kind: 'list',
        elementType: applySubst(subst, type.elementType)
      };

    case 'tuple':
      return {
        kind: 'tuple',
        types: type.types.map(t => applySubst(subst, t))
      };

    case 'record':
      const newFields = new Map<string, InferenceType>();
      for (const [fieldName, fieldType] of type.fields) {
        newFields.set(fieldName, applySubst(subst, fieldType));
      }
      return {
        kind: 'record',
        fields: newFields,
        name: type.name
      };

    case 'constructor':
      return {
        kind: 'constructor',
        name: type.name,
        typeArgs: type.typeArgs.map(arg => applySubst(subst, arg))
      };

    case 'any':
      // Any type is not affected by substitution
      return type;
  }
}

/**
 * Apply substitution to a type scheme
 *
 * Only substitute free variables (not quantified ones)
 */
export function applySubstToScheme(subst: Substitution, scheme: TypeScheme): TypeScheme {
  // Filter out quantified variables from substitution
  const filteredSubst = new Map<number, InferenceType>();
  for (const [varId, substType] of subst) {
    if (!scheme.quantifiedVars.has(varId)) {
      filteredSubst.set(varId, substType);
    }
  }

  return {
    quantifiedVars: scheme.quantifiedVars,
    type: applySubst(filteredSubst, scheme.type)
  };
}

/**
 * Compose two substitutions: s2 ∘ s1
 *
 * The result applies s1 first, then s2
 *
 * Example:
 *   s1 = [α ↦ β]
 *   s2 = [β ↦ Int]
 *   compose(s1, s2) = [α ↦ Int, β ↦ Int]
 */
export function composeSubstitutions(s1: Substitution, s2: Substitution): Substitution {
  const result = new Map<number, InferenceType>();

  // Apply s2 to all types in s1
  for (const [varId, type] of s1) {
    result.set(varId, applySubst(s2, type));
  }

  // Add all mappings from s2 that aren't in s1
  for (const [varId, type] of s2) {
    if (!result.has(varId)) {
      result.set(varId, type);
    }
  }

  return result;
}

/**
 * Collect all free type variables in a type
 *
 * Free variables are type variables that aren't quantified
 */
export function collectFreeVars(
  type: InferenceType,
  freeVars: Set<number> = new Set(),
  boundVars: Set<number> = new Set()
): Set<number> {
  switch (type.kind) {
    case 'primitive':
      return freeVars;

    case 'var':
      // Follow instance if it exists
      if (type.instance) {
        return collectFreeVars(type.instance, freeVars, boundVars);
      }
      // Add this variable if it's not bound
      if (!boundVars.has(type.id)) {
        freeVars.add(type.id);
      }
      return freeVars;

    case 'function':
      for (const param of type.params) {
        collectFreeVars(param, freeVars, boundVars);
      }
      collectFreeVars(type.returnType, freeVars, boundVars);
      return freeVars;

    case 'list':
      return collectFreeVars(type.elementType, freeVars, boundVars);

    case 'tuple':
      for (const t of type.types) {
        collectFreeVars(t, freeVars, boundVars);
      }
      return freeVars;

    case 'record':
      for (const fieldType of type.fields.values()) {
        collectFreeVars(fieldType, freeVars, boundVars);
      }
      return freeVars;

    case 'constructor':
      for (const arg of type.typeArgs) {
        collectFreeVars(arg, freeVars, boundVars);
      }
      return freeVars;

    case 'any':
      // Any type has no free variables
      return freeVars;
  }
}

/**
 * Convert AST type to inference type
 *
 * This is used when the user provides explicit type annotations
 */
export function astTypeToInferenceType(astType: AST.Type): InferenceType {
  switch (astType.type) {
    case 'PrimitiveType':
      return {
        kind: 'primitive',
        name: astType.name
      };

    case 'ListType':
      return {
        kind: 'list',
        elementType: astTypeToInferenceType(astType.elementType)
      };

    case 'TupleType':
      return {
        kind: 'tuple',
        types: astType.types.map(astTypeToInferenceType)
      };

    case 'FunctionType':
      return {
        kind: 'function',
        params: astType.paramTypes.map(astTypeToInferenceType),
        returnType: astTypeToInferenceType(astType.returnType),
        effects: new Set(astType.effects as Array<'IO' | 'Network' | 'Async' | 'Error' | 'Mut'>)
      };

    case 'TypeVariable':
      // Type variables in AST could be:
      // 1. Actual type parameters (T, U, E) - not yet supported
      // 2. User-defined types without type arguments (Color, Status)
      // For now, treat as a type constructor with no arguments
      // TODO: Proper handling requires tracking type parameters in context
      return {
        kind: 'constructor',
        name: astType.name,
        typeArgs: []
      };

    case 'TypeConstructor':
      return {
        kind: 'constructor',
        name: astType.name,
        typeArgs: astType.typeArgs.map(astTypeToInferenceType)
      };

    case 'MapType':
      // Map is just a constructor with two type arguments
      return {
        kind: 'constructor',
        name: 'Map',
        typeArgs: [
          astTypeToInferenceType(astType.keyType),
          astTypeToInferenceType(astType.valueType)
        ]
      };

    default:
      throw new Error(`Unknown AST type: ${(astType as any).type}`);
  }
}

/**
 * Check if two types are syntactically equal
 * (Used for testing and debugging)
 */
export function typesEqual(t1: InferenceType, t2: InferenceType): boolean {
  // Follow instances
  if (t1.kind === 'var' && t1.instance) {
    return typesEqual(t1.instance, t2);
  }
  if (t2.kind === 'var' && t2.instance) {
    return typesEqual(t1, t2.instance);
  }

  if (t1.kind !== t2.kind) return false;

  switch (t1.kind) {
    case 'primitive':
      return t1.name === (t2 as TPrimitive).name;

    case 'var':
      return t1.id === (t2 as TVar).id;

    case 'function': {
      const f2 = t2 as TFunction;
      if (t1.params.length !== f2.params.length) return false;
      for (let i = 0; i < t1.params.length; i++) {
        if (!typesEqual(t1.params[i], f2.params[i])) return false;
      }
      return typesEqual(t1.returnType, f2.returnType);
    }

    case 'list':
      return typesEqual(t1.elementType, (t2 as TList).elementType);

    case 'tuple': {
      const t2Tuple = t2 as TTuple;
      if (t1.types.length !== t2Tuple.types.length) return false;
      for (let i = 0; i < t1.types.length; i++) {
        if (!typesEqual(t1.types[i], t2Tuple.types[i])) return false;
      }
      return true;
    }

    case 'record': {
      const r2 = t2 as TRecord;
      if (t1.fields.size !== r2.fields.size) return false;
      for (const [name, type] of t1.fields) {
        const r2Type = r2.fields.get(name);
        if (!r2Type || !typesEqual(type, r2Type)) return false;
      }
      return true;
    }

    case 'constructor': {
      const c2 = t2 as TConstructor;
      if (t1.name !== c2.name) return false;
      if (t1.typeArgs.length !== c2.typeArgs.length) return false;
      for (let i = 0; i < t1.typeArgs.length; i++) {
        if (!typesEqual(t1.typeArgs[i], c2.typeArgs[i])) return false;
      }
      return true;
    }

    case 'any':
      // Any is equal to any
      return true;
  }
}
