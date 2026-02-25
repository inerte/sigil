/**
 * Sigil to TypeScript Code Generator
 *
 * Compiles Sigil AST to runnable TypeScript (ES2022-compatible output).
 */

import * as AST from '../parser/ast.js';
import { dirname, relative, resolve } from 'path';

export interface CodegenOptions {
  sourceFile?: string;
  outputFile?: string;
  projectRoot?: string;
}

export class JavaScriptGenerator {
  private indent = 0;
  private output: string[] = [];
  private sourceFile?: string;
  private outputFile?: string;
  private projectRoot?: string;
  private testMetaEntries: string[] = [];
  private mockableFunctions = new Set<string>();

  constructor(options?: CodegenOptions) {
    this.sourceFile = options?.sourceFile;
    this.outputFile = options?.outputFile;
    this.projectRoot = options?.projectRoot;
  }

  generate(program: AST.Program): string {
    this.output = [];
    this.indent = 0;
    this.testMetaEntries = [];
    this.mockableFunctions = new Set(
      program.declarations
        .filter((d): d is AST.FunctionDecl => d.type === 'FunctionDecl' && d.isMockable)
        .map(d => d.name)
    );

    this.emitMockRuntimeHelpers();

    // Generate code for all declarations
    for (const decl of program.declarations) {
      this.generateDeclaration(decl);
      this.output.push('\n');
    }

    if (this.testMetaEntries.length > 0) {
      this.emit(`export const __sigil_tests = [`);
      this.indent++;
      for (const entry of this.testMetaEntries) {
        this.emit(`${entry},`);
      }
      this.indent--;
      this.emit(`];`);
      this.output.push('\n');
    }

    return this.output.join('');
  }

  private generateDeclaration(decl: AST.Declaration): void {
    switch (decl.type) {
      case 'FunctionDecl':
        this.generateFunction(decl);
        break;
      case 'TypeDecl':
        this.generateTypeDecl(decl);
        break;
      case 'ConstDecl':
        this.generateConst(decl);
        break;
      case 'ImportDecl':
        this.generateImport(decl);
        break;
      case 'ExternDecl':
        this.generateExtern(decl);
        break;
      case 'TestDecl':
        this.generateTest(decl);
        break;
    }
  }

  private generateFunction(func: AST.FunctionDecl): void {
    // Function signature
    const params = func.params.map(p => p.name).join(', ');
    const implName = func.isMockable ? `__sigil_impl_${func.name}` : func.name;

    const shouldExport = func.isExported || func.name === 'main';
    const fnKeyword = shouldExport ? 'export async function' : 'async function';
    this.emit(`${fnKeyword} ${implName}(${params}) {`);
    this.indent++;

    // Function body
    const bodyCode = this.generateExpression(func.body);
    this.emit(`return ${bodyCode};`);

    this.indent--;
    this.emit('}');

    if (func.isMockable) {
      const key = this.mockKeyForFunction(func.name);
      const wrapperKeyword = shouldExport ? 'export async function' : 'async function';
      this.emit(`${wrapperKeyword} ${func.name}(${params}) {`);
      this.indent++;
      this.emit(`return await __sigil_call("${key}", ${implName}, [${params}]);`);
      this.indent--;
      this.emit('}');
    } else {
      // Non-mockable functions are already emitted with the final name.
    }
  }

  private generateTypeDecl(decl: AST.TypeDecl): void {
    // Generate constructor functions for sum types
    if (decl.definition.type === 'SumType') {
      this.emit(`// type ${decl.name}${decl.typeParams.length > 0 ? `[${decl.typeParams.join(',')}]` : ''}`);

      for (const variant of decl.definition.variants) {
        // Generate constructor function
        // Example: Some(x) → { __tag: "Some", __fields: [x] }
        const paramNames = variant.types.map((_, i) => `_${i}`);
        const params = paramNames.join(', ');

        const ctorKeyword = decl.isExported ? 'export async function' : 'async function';
        this.emit(`${ctorKeyword} ${variant.name}(${params}) {`);
        this.indent++;
        this.emit(`return { __tag: "${variant.name}", __fields: [${params}] };`);
        this.indent--;
        this.emit('}');
      }
    } else {
      // Product types and type aliases are erased for now
      this.emit(`// type ${decl.name} (erased)`);
    }
  }

  private generateConst(constDecl: AST.ConstDecl): void {
    const value = this.generateExpression(constDecl.value);
    const kw = constDecl.isExported ? 'export const' : 'const';
    this.emit(`${kw} ${constDecl.name} = ${value};`);
  }

  private generateImport(importDecl: AST.ImportDecl): void {
    const modulePath = importDecl.modulePath.join('/');

    // Convert slash to underscore for generated TypeScript identifier
    // stdlib⋅list_utils (Sigil) → stdlib_list_utils (generated JS identifier)
    const jsName = this.namespaceIdentifier(importDecl.modulePath);

    // Always use full module path
    // Import resolution is simplest with absolute paths from project root
    // i stdlib⋅list_utils → import * as stdlib_list_utils from './stdlib/list_utils'

    // Always use namespace import (import * as name)
    // Works exactly like FFI: i stdlib⋅list_utils → import * as stdlib_list_utils
    // Use as: stdlib⋅list_utils.len(xs) → stdlib_list_utils.len(xs)
    const importSpecifier = this.resolveSigilImportSpecifier(modulePath);
    this.emit(`import * as ${jsName} from '${importSpecifier}';`);
  }

  private generateExtern(externDecl: AST.ExternDecl): void {
    const modulePath = externDecl.modulePath.join('/');

    // Convert slash to underscore for generated TypeScript identifier
    // fs⋅promises (Sigil) / fs/promises (JS specifier) → fs_promises
    const jsName = this.namespaceIdentifier(externDecl.modulePath);

    // Always use namespace import (import * as name)
    // This matches our namespace.member usage in Sigil
    this.emit(`import * as ${jsName} from '${modulePath}';`);
  }

  private generateTest(test: AST.TestDecl): void {
    const fnName = `test_${this.sanitizeName(test.description)}`;
    this.emit(`// Test: ${test.description}`);
    this.emit(`export async function ${fnName}() {`);
    this.indent++;
    const bodyCode = this.generateTestBodyResult(test.body);
    this.emit(`return ${bodyCode};`);
    this.indent--;
    this.emit('}');
    const testId = `${this.sourceFile ?? '<unknown>'}::${test.description}`;
    const assertionMeta = this.generateAssertionMetadata(test.body);
    this.testMetaEntries.push(`{ id: ${JSON.stringify(testId)}, name: ${JSON.stringify(test.description)}, fn: ${fnName}, location: ${JSON.stringify(test.location)}, declaredEffects: ${JSON.stringify(test.effects)}, assertion: ${assertionMeta} }`);
  }

  private generateExpression(expr: AST.Expr): string {
    switch (expr.type) {
      case 'LiteralExpr':
        return this.generateLiteral(expr);
      case 'IdentifierExpr':
        return expr.name;
      case 'LambdaExpr':
        return this.generateLambda(expr);
      case 'ApplicationExpr':
        return this.generateApplication(expr);
      case 'BinaryExpr':
        return this.generateBinary(expr);
      case 'UnaryExpr':
        return this.generateUnary(expr);
      case 'MatchExpr':
        return this.generateMatch(expr);
      case 'LetExpr':
        return this.generateLet(expr);
      case 'IfExpr':
        return this.generateIf(expr);
      case 'ListExpr':
        return this.generateList(expr);
      case 'RecordExpr':
        return this.generateRecord(expr);
      case 'TupleExpr':
        return this.generateTuple(expr);
      case 'FieldAccessExpr':
        return this.generateFieldAccess(expr);
      case 'MemberAccessExpr':
        return this.generateMemberAccess(expr);
      case 'WithMockExpr':
        return this.generateWithMock(expr);
      case 'IndexExpr':
        return this.generateIndex(expr);
      case 'PipelineExpr':
        return this.generatePipeline(expr);
      case 'MapExpr':
        return this.generateMap(expr);
      case 'FilterExpr':
        return this.generateFilter(expr);
      case 'FoldExpr':
        return this.generateFold(expr);
      default:
        throw new Error(`Unsupported expression type: ${(expr as any).type}`);
    }
  }

  private generateLiteral(lit: AST.LiteralExpr): string {
    if (lit.literalType === 'Unit') {
      return 'undefined';
    }
    if (lit.literalType === 'String' || lit.literalType === 'Char') {
      return JSON.stringify(lit.value);
    }
    return String(lit.value);
  }

  private generateLambda(lambda: AST.LambdaExpr): string {
    const params = lambda.params.map(p => p.name).join(', ');
    const body = this.generateExpression(lambda.body);
    return `(async (${params}) => ${body})`;
  }

  private generateApplication(app: AST.ApplicationExpr): string {
    // Check for stdlib intrinsics first
    if (app.func.type === 'MemberAccessExpr') {
      const intrinsic = this.tryGenerateIntrinsic(app.func, app.args);
      if (intrinsic) {
        return intrinsic;
      }
    }

    const args = app.args.map(arg => this.generateExpression(arg)).join(', ');
    if (app.func.type === 'MemberAccessExpr') {
      const func = this.generateMemberAccess(app.func);
      const key = this.mockKeyForExtern(app.func);
      return `await __sigil_call(${JSON.stringify(key)}, ${func}, [${args}])`;
    }
    if (app.func.type === 'IdentifierExpr' && this.mockableFunctions.has(app.func.name)) {
      const func = this.generateExpression(app.func);
      const key = this.mockKeyForFunction(app.func.name);
      return `await __sigil_call(${JSON.stringify(key)}, ${func}, [${args}])`;
    }
    const func = this.generateExpression(app.func);
    return `await ${func}(${args})`;
  }

  private generateBinary(binary: AST.BinaryExpr): string {
    const left = this.generateExpression(binary.left);
    const right = this.generateExpression(binary.right);

    // Map Sigil operators to TypeScript/JavaScript
    const opMap: Record<string, string> = {
      '∧': '&&',
      '∨': '||',
      '≠': '!==',
      '=': '===',
      '≤': '<=',
      '≥': '>=',
      '++': '+', // String concatenation
      '^': '**', // Exponentiation
    };

    const op = opMap[binary.operator] || binary.operator;

    // Handle special operators
    if (binary.operator === '⧺') {
      // List concatenation
      return `${left}.concat(${right})`;
    }

    return `(${left} ${op} ${right})`;
  }

  private generateUnary(unary: AST.UnaryExpr): string {
    const operand = this.generateExpression(unary.operand);

    // Special case for # (length) operator
    if (unary.operator === '#') {
      return `(await ${operand}).length`;
    }

    const opMap: Record<string, string> = {
      '¬': '!',
      '-': '-',
    };
    const op = opMap[unary.operator] || unary.operator;
    return `${op}${operand}`;
  }

  private generateMatch(match: AST.MatchExpr): string {
    // Generate an async IIFE that implements pattern matching
    const scrutinee = this.generateExpression(match.scrutinee);

    // For now, implement simple pattern matching
    // This could be optimized later
    const lines: string[] = [];
    lines.push(`(async () => {`);
    lines.push(`  const __match = await ${scrutinee};`);

    for (let i = 0; i < match.arms.length; i++) {
      const arm = match.arms[i];
      const condition = this.generatePatternCondition(arm.pattern, '__match');
      const body = this.generateExpression(arm.body);
      const bindings = this.generatePatternBindings(arm.pattern, '__match');

      if (i === 0) {
        lines.push(`  if (${condition}) {`);
      } else if (arm.pattern.type === 'WildcardPattern') {
        lines.push(`  else {`);
      } else {
        lines.push(`  else if (${condition}) {`);
      }

      if (bindings) {
        lines.push(`    ${bindings}`);
      }

      // Add guard check if present
      if (arm.guard) {
        const guardExpr = this.generateExpression(arm.guard);
        lines.push(`    if (await ${guardExpr}) {`);
        lines.push(`      return ${body};`);
        lines.push(`    }`);
      } else {
        lines.push(`    return ${body};`);
      }

      lines.push(`  }`);
    }

    lines.push(`  throw new Error('Match failed: no pattern matched');`);
    lines.push(`})()`);

    return lines.join('\n');
  }

  private generatePatternCondition(pattern: AST.Pattern, scrutinee: string): string {
    switch (pattern.type) {
      case 'LiteralPattern':
        if (pattern.literalType === 'String' || pattern.literalType === 'Char') {
          return `${scrutinee} === ${JSON.stringify(pattern.value)}`;
        }
        return `${scrutinee} === ${pattern.value}`;

      case 'IdentifierPattern':
        // Identifier patterns always match
        return 'true';

      case 'WildcardPattern':
        return 'true';

      case 'ConstructorPattern':
        // Check constructor name and recursively check fields
        // For now, simplified
        return `${scrutinee}?.__tag === ${JSON.stringify(pattern.name)}`;

      case 'ListPattern':
        if (pattern.patterns.length === 0) {
          return `${scrutinee}.length === 0`;
        }
        return `${scrutinee}.length >= ${pattern.patterns.length}`;

      case 'TuplePattern':
        // Check array length and recursively check each element
        const lengthCheck = `Array.isArray(${scrutinee}) && ${scrutinee}.length === ${pattern.patterns.length}`;
        const elementChecks = pattern.patterns
          .map((p, i) => this.generatePatternCondition(p, `${scrutinee}[${i}]`))
          .filter(c => c !== 'true') // Skip always-true patterns
          .join(' && ');

        if (elementChecks) {
          return `${lengthCheck} && ${elementChecks}`;
        }
        return lengthCheck;

      case 'RecordPattern':
        return 'true'; // Simplified for now

      default:
        return 'true';
    }
  }

  private generatePatternBindings(pattern: AST.Pattern, scrutinee: string): string | null {
    switch (pattern.type) {
      case 'IdentifierPattern':
        return `const ${pattern.name} = ${scrutinee};`;

      case 'ConstructorPattern':
        // Extract fields from constructor
        const bindings = pattern.patterns
          .map((p, i) => this.generatePatternBindings(p, `${scrutinee}.__fields[${i}]`))
          .filter(b => b !== null)
          .join(' ');
        return bindings || null;

      case 'ListPattern':
        // Bind list elements
        const listBindings = pattern.patterns
          .map((p, i) => this.generatePatternBindings(p, `${scrutinee}[${i}]`))
          .filter(b => b !== null)
          .join(' ');
        if (pattern.rest) {
          const restBinding = `const ${pattern.rest} = ${scrutinee}.slice(${pattern.patterns.length});`;
          return listBindings ? `${listBindings} ${restBinding}` : restBinding;
        }
        return listBindings || null;

      case 'TuplePattern':
        const tupleBindings = pattern.patterns
          .map((p, i) => this.generatePatternBindings(p, `${scrutinee}[${i}]`))
          .filter(b => b !== null)
          .join(' ');
        return tupleBindings || null;

      default:
        return null;
    }
  }

  private generateLet(letExpr: AST.LetExpr): string {
    // Generate async IIFE for let binding
    const value = this.generateExpression(letExpr.value);
    const body = this.generateExpression(letExpr.body);
    const bindings = this.generatePatternBindings(letExpr.pattern, '__value');

    return `(async () => {
  const __value = await ${value};
  ${bindings || ''}
  return ${body};
})()`;
  }

  private generateIf(ifExpr: AST.IfExpr): string {
    const condition = this.generateExpression(ifExpr.condition);
    const thenBranch = this.generateExpression(ifExpr.thenBranch);
    const elseBranch = ifExpr.elseBranch
      ? this.generateExpression(ifExpr.elseBranch)
      : 'undefined';

    return `(${condition} ? ${thenBranch} : ${elseBranch})`;
  }

  private generateList(list: AST.ListExpr): string {
    if (list.elements.length === 0) {
      return '[]';
    }

    // Simple approach: just use concat
    // [a, b, c] becomes [].concat([a], [b], [c])
    // This works whether elements are single values or arrays
    if (list.elements.length === 1) {
      const elem = this.generateExpression(list.elements[0]);
      // Single element - check if it needs to be wrapped
      if (list.elements[0].type === 'ApplicationExpr' ||
          list.elements[0].type === 'IdentifierExpr') {
        // Could be an array, could be a value - wrap in Array() to ensure it's array-like
        return `[].concat(${elem})`;
      }
      return `[${elem}]`;
    }

    // Multiple elements - use concat for all
    const parts = list.elements.map(e => {
      const code = this.generateExpression(e);
      // Wrap each element so concat works properly
      // If it's already an array, concat handles it. If it's a value, it adds the value.
      if (e.type === 'ApplicationExpr' || e.type === 'IdentifierExpr') {
        return code; // Could be array or value, concat handles both
      }
      return `[${code}]`; // Definitely a single value, wrap it
    });

    return `[].concat(${parts.join(', ')})`;
  }

  private generateRecord(record: AST.RecordExpr): string {
    const fields = record.fields
      .map(f => `${JSON.stringify(f.name)}: ${this.generateExpression(f.value)}`)
      .join(', ');
    return `{ ${fields} }`;
  }

  private generateTuple(tuple: AST.TupleExpr): string {
    const elements = tuple.elements.map(e => this.generateExpression(e)).join(', ');
    return `[${elements}]`;
  }

  private generateFieldAccess(access: AST.FieldAccessExpr): string {
    const object = this.generateExpression(access.object);
    return `(${object}).${access.field}`;
  }

  private generateMemberAccess(access: AST.MemberAccessExpr): string {
    // Convert namespace path to generated TypeScript identifier
    // fs⋅promises (Sigil) / fs/promises (JS specifier) → fs_promises
    const jsNamespace = this.namespaceIdentifier(access.namespace);

    // Generate: namespace.member
    return `${jsNamespace}.${access.member}`;
  }

  private generateWithMock(expr: AST.WithMockExpr): string {
    const replacement = this.generateExpression(expr.replacement);
    const body = this.generateExpression(expr.body);
    const key = this.mockKeyForTarget(expr.target);
    if (expr.target.type === 'MemberAccessExpr') {
      const actualFn = this.generateMemberAccess(expr.target);
      return `await __sigil_with_mock_extern(${JSON.stringify(key)}, ${actualFn}, ${replacement}, async () => ${body})`;
    }
    return `await __sigil_with_mock(${JSON.stringify(key)}, ${replacement}, async () => ${body})`;
  }

  private generateIndex(index: AST.IndexExpr): string {
    const object = this.generateExpression(index.object);
    const idx = this.generateExpression(index.index);
    return `(${object})[${idx}]`;
  }

  private generatePipeline(pipeline: AST.PipelineExpr): string {
    const left = this.generateExpression(pipeline.left);
    const right = this.generateExpression(pipeline.right);

    // |> is function application: x |> f  becomes  f(x)
    if (pipeline.operator === '|>') {
      return `await ${right}(await ${left})`;
    }

    // >> is function composition: f >> g  becomes  (x) => g(f(x))
    if (pipeline.operator === '>>') {
      return `(async (x) => await ${right}(await ${left}(x)))`;
    }

    // << is reverse composition: f << g  becomes  (x) => f(g(x))
    if (pipeline.operator === '<<') {
      return `(async (x) => await ${left}(await ${right}(x)))`;
    }

    return `${left} ${pipeline.operator} ${right}`;
  }

  private generateMap(map: AST.MapExpr): string {
    const list = this.generateExpression(map.list);
    const fn = this.generateExpression(map.fn);
    return `await Promise.all((await ${list}).map(${fn}))`;
  }

  private generateFilter(filter: AST.FilterExpr): string {
    const list = this.generateExpression(filter.list);
    const predicate = this.generateExpression(filter.predicate);
    return `(await Promise.all((await ${list}).map(async (x) => ({ x, keep: await ${predicate}(x) })))).filter(({ keep }) => keep).map(({ x }) => x)`;
  }

  private generateFold(fold: AST.FoldExpr): string {
    const list = this.generateExpression(fold.list);
    const fn = this.generateExpression(fold.fn);
    const init = this.generateExpression(fold.init);
    return `(await ${list}).reduce(async (accPromise, x) => await ${fn}(await accPromise, x), await ${init})`;
  }

  private emit(code: string): void {
    const indentation = '  '.repeat(this.indent);
    this.output.push(indentation + code + '\n');
  }

  private sanitizeName(name: string): string {
    return name.replace(/[^a-zA-Z0-9_]/g, '_');
  }

  /**
   * Try to generate optimized code for stdlib intrinsics.
   * Returns null if not an intrinsic.
   */
  private tryGenerateIntrinsic(func: AST.MemberAccessExpr, args: AST.Expr[]): string | null {
    const module = func.namespace.join('/');
    const member = func.member;

    // String operations intrinsics
    if (module === 'stdlib/string_ops') {
      const generatedArgs = args.map(arg => this.generateExpression(arg));

      switch (member) {
        case 'char_at':
          return `(await ${generatedArgs[0]}).charAt(await ${generatedArgs[1]})`;
        case 'substring':
          return `(await ${generatedArgs[0]}).substring(await ${generatedArgs[1]}, await ${generatedArgs[2]})`;
        case 'to_upper':
          return `(await ${generatedArgs[0]}).toUpperCase()`;
        case 'to_lower':
          return `(await ${generatedArgs[0]}).toLowerCase()`;
        case 'trim':
          return `(await ${generatedArgs[0]}).trim()`;
        case 'index_of':
          return `(await ${generatedArgs[0]}).indexOf(await ${generatedArgs[1]})`;
        case 'split':
          return `(await ${generatedArgs[0]}).split(await ${generatedArgs[1]})`;
        case 'replace_all':
          return `(await ${generatedArgs[0]}).replaceAll(await ${generatedArgs[1]}, await ${generatedArgs[2]})`;
        // take and drop are implemented in Sigil, not intrinsics
      }
    }

    // String predicates intrinsics
    if (module === 'stdlib/string_predicates') {
      const generatedArgs = args.map(arg => this.generateExpression(arg));

      switch (member) {
        case 'starts_with':
          return `(await ${generatedArgs[0]}).startsWith(await ${generatedArgs[1]})`;
        case 'ends_with':
          return `(await ${generatedArgs[0]}).endsWith(await ${generatedArgs[1]})`;
      }
    }

    return null;
  }

  private mockKeyForExtern(expr: AST.MemberAccessExpr): string {
    return `extern:${expr.namespace.join('/')}.${expr.member}`;
  }

  private mockKeyForFunction(name: string): string {
    return `fn:${this.sourceFile ?? '<unknown>'}:${name}`;
  }

  private mockKeyForTarget(target: AST.Expr): string {
    if (target.type === 'MemberAccessExpr') {
      return this.mockKeyForExtern(target);
    }
    if (target.type === 'IdentifierExpr') {
      return this.mockKeyForFunction(target.name);
    }
    throw new Error(`Unsupported with_mock target expression: ${target.type}`);
  }

  private emitMockRuntimeHelpers(): void {
    this.emit(`const __sigil_mocks = new Map();`);
    this.emit(`function __sigil_preview(value) {`);
    this.indent++;
    this.emit(`try { return JSON.stringify(value); } catch { return String(value); }`);
    this.indent--;
    this.emit(`}`);
    this.emit(`function __sigil_diff_hint(actual, expected) {`);
    this.indent++;
    this.emit(`if (Array.isArray(actual) && Array.isArray(expected)) {`);
    this.indent++;
    this.emit(`if (actual.length !== expected.length) { return { kind: 'array_length', actualLength: actual.length, expectedLength: expected.length }; }`);
    this.emit(`for (let i = 0; i < actual.length; i++) { if (actual[i] !== expected[i]) { return { kind: 'array_first_diff', index: i, actual: __sigil_preview(actual[i]), expected: __sigil_preview(expected[i]) }; } }`);
    this.emit(`return null;`);
    this.indent--;
    this.emit(`}`);
    this.emit(`if (actual && expected && typeof actual === 'object' && typeof expected === 'object') {`);
    this.indent++;
    this.emit(`const actualKeys = Object.keys(actual).sort();`);
    this.emit(`const expectedKeys = Object.keys(expected).sort();`);
    this.emit(`if (actualKeys.join('|') !== expectedKeys.join('|')) { return { kind: 'object_keys', actualKeys, expectedKeys }; }`);
    this.emit(`for (const k of actualKeys) { if (actual[k] !== expected[k]) { return { kind: 'object_field', field: k, actual: __sigil_preview(actual[k]), expected: __sigil_preview(expected[k]) }; } }`);
    this.emit(`return null;`);
    this.indent--;
    this.emit(`}`);
    this.emit(`return null;`);
    this.indent--;
    this.emit(`}`);
    this.emit(`async function __sigil_test_bool_result(ok) {`);
    this.indent++;
    this.emit(`const result = await ok;`);
    this.emit(`return result === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to ⊥' } };`);
    this.indent--;
    this.emit(`}`);
    this.emit(`async function __sigil_test_compare_result(op, leftFn, rightFn) {`);
    this.indent++;
    this.emit(`const actual = await leftFn();`);
    this.emit(`const expected = await rightFn();`);
    this.emit(`let ok = false;`);
    this.emit(`switch (op) {`);
    this.indent++;
    this.emit(`case '=': ok = actual === expected; break;`);
    this.emit(`case '≠': ok = actual !== expected; break;`);
    this.emit(`case '<': ok = actual < expected; break;`);
    this.emit(`case '>': ok = actual > expected; break;`);
    this.emit(`case '≤': ok = actual <= expected; break;`);
    this.emit(`case '≥': ok = actual >= expected; break;`);
    this.emit(`default: throw new Error('Unsupported test comparison operator: ' + String(op));`);
    this.indent--;
    this.emit(`}`);
    this.emit(`if (ok) { return { ok: true }; }`);
    this.emit(`return { ok: false, failure: { kind: 'comparison_mismatch', message: 'Comparison test failed', operator: op, actual: __sigil_preview(actual), expected: __sigil_preview(expected), diffHint: __sigil_diff_hint(actual, expected) } };`);
    this.indent--;
    this.emit(`}`);
    this.emit(`async function __sigil_call(key, actualFn, args) {`);
    this.indent++;
    this.emit(`const mockFn = __sigil_mocks.get(key);`);
    this.emit(`const fn = mockFn ?? actualFn;`);
    this.emit(`return await fn(...args);`);
    this.indent--;
    this.emit(`}`);
    this.emit(`async function __sigil_with_mock(key, mockFn, body) {`);
    this.indent++;
    this.emit(`const had = __sigil_mocks.has(key);`);
    this.emit(`const prev = __sigil_mocks.get(key);`);
    this.emit(`__sigil_mocks.set(key, mockFn);`);
    this.emit(`try {`);
    this.indent++;
    this.emit(`return await body();`);
    this.indent--;
    this.emit(`} finally {`);
    this.indent++;
    this.emit(`if (had) { __sigil_mocks.set(key, prev); } else { __sigil_mocks.delete(key); }`);
    this.indent--;
    this.emit(`}`);
    this.indent--;
    this.emit(`}`);
    this.emit(`async function __sigil_with_mock_extern(key, actualFn, mockFn, body) {`);
    this.indent++;
    this.emit(`if (typeof actualFn !== 'function') { throw new Error('with_mock extern target is not callable'); }`);
    this.emit(`if (typeof mockFn !== 'function') { throw new Error('with_mock replacement must be callable'); }`);
    this.emit(`if (actualFn.length !== mockFn.length) { throw new Error(\`with_mock extern arity mismatch for \${key}: expected \${actualFn.length}, got \${mockFn.length}\`); }`);
    this.emit(`return await __sigil_with_mock(key, mockFn, body);`);
    this.indent--;
    this.emit(`}`);
  }

  private generateTestBodyResult(expr: AST.Expr): string {
    if (expr.type === 'BinaryExpr' && ['=', '≠', '<', '>', '≤', '≥'].includes(expr.operator)) {
      const left = this.generateExpression(expr.left);
      const right = this.generateExpression(expr.right);
      return `await __sigil_test_compare_result(${JSON.stringify(expr.operator)}, async () => ${left}, async () => ${right})`;
    }
    const body = this.generateExpression(expr);
    return `await __sigil_test_bool_result(${body})`;
  }

  private generateAssertionMetadata(expr: AST.Expr): string {
    if (expr.type === 'BinaryExpr' && ['=', '≠', '<', '>', '≤', '≥'].includes(expr.operator)) {
      return JSON.stringify({
        kind: 'comparison',
        operator: expr.operator,
        left: { location: expr.left.location },
        right: { location: expr.right.location }
      });
    }
    return 'null';
  }

  private namespaceIdentifier(pathSegments: string[]): string {
    return pathSegments
      .map(seg => seg.replace(/[^A-Za-z0-9_]/g, '_'))
      .join('_');
  }

  private resolveSigilImportSpecifier(modulePath: string): string {
    // Project-root imports like i src⋅foo should point to generated .local/src/foo from the current generated file.
    if (modulePath.startsWith('src/') && this.outputFile && this.projectRoot) {
      const generatedTarget = resolve(this.projectRoot, '.local', `${modulePath}.ts`);
      const fromDir = dirname(resolve(this.outputFile));
      let rel = relative(fromDir, generatedTarget).replace(/\\/g, '/');
      rel = rel.replace(/\.ts$/, '');
      if (!rel.startsWith('.')) {
        rel = `./${rel}`;
      }
      return rel;
    }

    if (modulePath.startsWith('stdlib/') && this.outputFile && this.projectRoot) {
      const generatedTarget = resolve(this.projectRoot, '.local', `${modulePath}.ts`);
      const fromDir = dirname(resolve(this.outputFile));
      let rel = relative(fromDir, generatedTarget).replace(/\\/g, '/');
      rel = rel.replace(/\.ts$/, '');
      if (!rel.startsWith('.')) {
        rel = `./${rel}`;
      }
      return rel;
    }

    // Legacy behavior for stdlib and non-project imports.
    return `./${modulePath}`;
  }
}

/**
 * Compile a Sigil program to TypeScript
 */
export function compile(program: AST.Program, options?: string | CodegenOptions): string {
  const normalized: CodegenOptions = typeof options === 'string'
    ? { sourceFile: options }
    : (options ?? {});
  const generator = new JavaScriptGenerator(normalized);
  return generator.generate(program);
}
