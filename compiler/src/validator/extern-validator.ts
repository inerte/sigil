/**
 * Mint FFI - Link-Time External Module Validator
 *
 * Validates external modules and member access BEFORE writing JavaScript.
 * This catches errors like:
 * - Module not installed (npm install missing)
 * - Typos in member names (axios.gett instead of axios.get)
 * - Non-existent members
 *
 * Works by dynamically importing modules and checking if accessed members exist.
 */

import * as AST from '../parser/ast.js';

/**
 * Validate all external modules and member accesses
 *
 * This runs AFTER type checking but BEFORE JavaScript code generation.
 * It's "link-time" validation - like a linker checking if symbols exist.
 *
 * @param program - The AST program
 * @throws Error if module can't be loaded or member doesn't exist
 */
export async function validateExterns(program: AST.Program): Promise<void> {
  // Step 1: Collect all extern declarations
  const externs = program.declarations.filter(
    (d): d is AST.ExternDecl => d.type === 'ExternDecl'
  );

  // Step 2: Load each module dynamically and cache it
  const loadedModules = new Map<string, any>();

  for (const ext of externs) {
    const modulePath = ext.modulePath.join('/');
    try {
      // Dynamic import - works with npm packages and Node.js built-ins
      const module = await import(modulePath);
      loadedModules.set(modulePath, module);
    } catch (err: any) {
      throw new Error(
        `Cannot load external module '${modulePath}':\n` +
          `  ${err.message}\n` +
          `Make sure it's installed: npm install ${modulePath}`
      );
    }
  }

  // Step 3: Collect all member accesses (namespace.member)
  // This includes both MemberAccessExpr (e.g., fs/promises.readFile)
  // and FieldAccessExpr on extern namespaces (e.g., console.log)
  const memberAccesses = collectMemberAccesses(program, loadedModules);

  // Step 4: Validate each member exists
  for (const access of memberAccesses) {
    const module = loadedModules.get(access.namespacePath);

    if (!module) {
      // Not an extern module - skip validation
      continue;
    }

    // Check if member exists on the module
    if (typeof module[access.memberName] === 'undefined') {
      // Member doesn't exist - provide helpful error
      const available = Object.keys(module)
        .filter((key) => !key.startsWith('_')) // Hide internal keys
        .slice(0, 10) // Show first 10
        .join(', ');

      throw new Error(
        `Member '${access.memberName}' does not exist on module '${access.namespacePath}'\n` +
          `Available members: ${available}${Object.keys(module).length > 10 ? ', ...' : ''}\n` +
          `Check for typos or see module documentation.`
      );
    }

    // Member exists! Validation passed.
  }
}

/**
 * Unified member access for validation
 */
interface MemberAccess {
  namespacePath: string;
  memberName: string;
}

/**
 * Recursively collect all member accesses from the AST
 * This includes:
 * - MemberAccessExpr: fs/promises.readFile
 * - FieldAccessExpr on extern namespaces: console.log
 */
function collectMemberAccesses(
  program: AST.Program,
  loadedModules: Map<string, any>
): MemberAccess[] {
  const accesses: MemberAccess[] = [];

  function visitExpr(expr: AST.Expr) {
    // MemberAccessExpr: fs/promises.readFile
    if (expr.type === 'MemberAccessExpr') {
      accesses.push({
        namespacePath: expr.namespace.join('/'),
        memberName: expr.member,
      });
    }

    // FieldAccessExpr on extern namespace: console.log
    if (expr.type === 'FieldAccessExpr' && expr.object.type === 'IdentifierExpr') {
      const namespaceName = expr.object.name;
      // Check if this identifier is an extern namespace
      if (loadedModules.has(namespaceName)) {
        accesses.push({
          namespacePath: namespaceName,
          memberName: expr.field,
        });
      }
    }

    // Recursively visit child expressions
    switch (expr.type) {
      case 'ApplicationExpr':
        visitExpr(expr.func);
        expr.args.forEach(visitExpr);
        break;
      case 'BinaryExpr':
        visitExpr(expr.left);
        visitExpr(expr.right);
        break;
      case 'UnaryExpr':
        visitExpr(expr.operand);
        break;
      case 'MatchExpr':
        visitExpr(expr.scrutinee);
        expr.arms.forEach((arm) => visitExpr(arm.body));
        break;
      case 'LetExpr':
        visitExpr(expr.value);
        visitExpr(expr.body);
        break;
      case 'IfExpr':
        visitExpr(expr.condition);
        visitExpr(expr.thenBranch);
        if (expr.elseBranch) visitExpr(expr.elseBranch);
        break;
      case 'ListExpr':
        expr.elements.forEach(visitExpr);
        break;
      case 'RecordExpr':
        expr.fields.forEach((f) => visitExpr(f.value));
        break;
      case 'TupleExpr':
        expr.elements.forEach(visitExpr);
        break;
      case 'FieldAccessExpr':
        visitExpr(expr.object);
        break;
      case 'IndexExpr':
        visitExpr(expr.object);
        visitExpr(expr.index);
        break;
      case 'PipelineExpr':
        visitExpr(expr.left);
        visitExpr(expr.right);
        break;
      case 'MapExpr':
        visitExpr(expr.list);
        visitExpr(expr.fn);
        break;
      case 'FilterExpr':
        visitExpr(expr.list);
        visitExpr(expr.predicate);
        break;
      case 'FoldExpr':
        visitExpr(expr.list);
        visitExpr(expr.fn);
        visitExpr(expr.init);
        break;
      case 'LambdaExpr':
        visitExpr(expr.body);
        break;
      // Literals and identifiers have no child expressions
      case 'LiteralExpr':
      case 'IdentifierExpr':
      case 'MemberAccessExpr':
        break;
    }
  }

  function visitDecl(decl: AST.Declaration) {
    if (decl.type === 'FunctionDecl') {
      visitExpr(decl.body);
    } else if (decl.type === 'ConstDecl') {
      visitExpr(decl.value);
    } else if (decl.type === 'TestDecl') {
      visitExpr(decl.body);
    }
    // ExternDecl, TypeDecl, ImportDecl have no expressions
  }

  program.declarations.forEach(visitDecl);

  return accesses;
}
