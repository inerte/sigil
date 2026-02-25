/**
 * Sigil Type Checker - Type Environment (Bidirectional)
 *
 * Manages variable bindings during type checking.
 * Simplified from HM version - no type schemes, direct InferenceType bindings.
 */

import { InferenceType } from './types.js';
import * as AST from '../parser/ast.js';

/**
 * Type information for user-defined types
 */
export interface TypeInfo {
  typeParams: string[];        // Generic type parameters (e.g., ['T', 'E'] for Result[T,E])
  definition: AST.TypeDef;     // The type definition (SumType, ProductType, or TypeAlias)
}

export interface BindingMeta {
  isMockableFunction?: boolean;
  isExternNamespace?: boolean;
}

/**
 * Type environment (Γ in type theory notation)
 *
 * Maps variable names to their types
 * Supports nested scopes via parent chaining
 */
export class TypeEnvironment {
  private bindings: Map<string, InferenceType>;
  private bindingMeta: Map<string, BindingMeta>;
  private typeRegistry: Map<string, TypeInfo>;  // NEW: user-defined types
  private importedTypeRegistries: Map<string, Map<string, TypeInfo>>;  // NEW: types from imported modules
  private parent?: TypeEnvironment;

  constructor(parent?: TypeEnvironment) {
    this.bindings = new Map();
    this.bindingMeta = new Map();
    this.typeRegistry = new Map();
    this.importedTypeRegistries = new Map();
    this.parent = parent;
  }

  /**
   * Look up a variable's type
   *
   * Searches this environment and all parent environments
   */
  lookup(name: string): InferenceType | undefined {
    const local = this.bindings.get(name);
    if (local) {
      return local;
    }

    // Search parent scope
    return this.parent?.lookup(name);
  }

  /**
   * Bind a variable to a type
   *
   * Only affects the current scope
   */
  bind(name: string, type: InferenceType): void {
    this.bindings.set(name, type);
  }

  bindWithMeta(name: string, type: InferenceType, meta: BindingMeta): void {
    this.bindings.set(name, type);
    this.bindingMeta.set(name, meta);
  }

  lookupMeta(name: string): BindingMeta | undefined {
    const local = this.bindingMeta.get(name);
    if (local) {
      return local;
    }
    return this.parent?.lookupMeta(name);
  }

  /**
   * Register a user-defined type
   *
   * Stores type definition for later lookup during type checking
   */
  registerType(name: string, info: TypeInfo): void {
    this.typeRegistry.set(name, info);
  }

  /**
   * Look up a user-defined type
   *
   * Searches this environment and all parent environments
   */
  lookupType(name: string): TypeInfo | undefined {
    const local = this.typeRegistry.get(name);
    if (local) {
      return local;
    }

    // Search parent scope
    return this.parent?.lookupType(name);
  }

  /**
   * Register types from an imported module
   */
  registerImportedTypes(moduleId: string, types: Map<string, TypeInfo>): void {
    this.importedTypeRegistries.set(moduleId, types);
  }

  /**
   * Look up a qualified type from an imported module
   * Example: lookupQualifiedType(['src', 'types'], 'ArticleMeta')
   */
  lookupQualifiedType(modulePath: string[], typeName: string): TypeInfo | undefined {
    const moduleId = modulePath.join('⋅');
    const registry = this.importedTypeRegistries.get(moduleId);
    if (registry) {
      return registry.get(typeName);
    }

    // Check parent scope
    return this.parent?.lookupQualifiedType(modulePath, typeName);
  }

  /**
   * Get all exported type names from a module (for error messages)
   */
  getImportedModuleTypeNames(moduleId: string): string[] | undefined {
    const registry = this.importedTypeRegistries.get(moduleId);
    if (registry) {
      return Array.from(registry.keys()).sort();
    }
    return this.parent?.getImportedModuleTypeNames(moduleId);
  }

  /**
   * Create a child environment with additional bindings
   *
   * Example: when entering a lambda or match arm with pattern bindings
   */
  extend(newBindings?: Map<string, InferenceType>): TypeEnvironment {
    const child = new TypeEnvironment(this);
    if (newBindings) {
      for (const [name, type] of newBindings) {
        child.bind(name, type);
      }
    }
    return child;
  }

  /**
   * Get all bindings in this scope (for debugging/testing)
   */
  getBindings(): Map<string, InferenceType> {
    return new Map(this.bindings);
  }

  /**
   * Create the initial environment with built-in operators
   */
  static createInitialEnvironment(): TypeEnvironment {
    const env = new TypeEnvironment();

    // Built-in operators are handled directly in synthesizeBinary/synthesizeUnary
    // This environment is primarily for user-defined functions and constants

    return env;
  }
}
