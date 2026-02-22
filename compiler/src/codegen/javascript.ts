/**
 * Mint to JavaScript Code Generator
 *
 * Compiles Mint AST to runnable JavaScript (ES2022).
 */

import * as AST from '../parser/ast.js';

export class JavaScriptGenerator {
  private indent = 0;
  private output: string[] = [];

  generate(program: AST.Program): string {
    this.output = [];
    this.indent = 0;

    // Generate code for all declarations
    for (const decl of program.declarations) {
      this.generateDeclaration(decl);
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
        // Type declarations are erased in JavaScript
        this.emit(`// type ${decl.name} (erased)`);
        break;
      case 'ConstDecl':
        this.generateConst(decl);
        break;
      case 'ImportDecl':
        this.generateImport(decl);
        break;
      case 'TestDecl':
        this.generateTest(decl);
        break;
    }
  }

  private generateFunction(func: AST.FunctionDecl): void {
    // Function signature
    const params = func.params.map(p => p.name).join(', ');
    this.emit(`export function ${func.name}(${params}) {`);
    this.indent++;

    // Function body
    const bodyCode = this.generateExpression(func.body);
    this.emit(`return ${bodyCode};`);

    this.indent--;
    this.emit('}');
  }

  private generateConst(constDecl: AST.ConstDecl): void {
    const value = this.generateExpression(constDecl.value);
    this.emit(`export const ${constDecl.name} = ${value};`);
  }

  private generateImport(importDecl: AST.ImportDecl): void {
    const path = importDecl.modulePath.join('/');
    if (importDecl.imports === null) {
      // import all
      this.emit(`import * as ${importDecl.modulePath[importDecl.modulePath.length - 1]} from './${path}.js';`);
    } else {
      const imports = importDecl.imports.join(', ');
      this.emit(`import { ${imports} } from './${path}.js';`);
    }
  }

  private generateTest(test: AST.TestDecl): void {
    this.emit(`// Test: ${test.description}`);
    this.emit(`export function test_${this.sanitizeName(test.description)}() {`);
    this.indent++;
    const bodyCode = this.generateExpression(test.body);
    this.emit(`return ${bodyCode};`);
    this.indent--;
    this.emit('}');
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
      case 'IndexExpr':
        return this.generateIndex(expr);
      case 'PipelineExpr':
        return this.generatePipeline(expr);
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
    return `((${params}) => ${body})`;
  }

  private generateApplication(app: AST.ApplicationExpr): string {
    const func = this.generateExpression(app.func);
    const args = app.args.map(arg => this.generateExpression(arg)).join(', ');
    return `${func}(${args})`;
  }

  private generateBinary(binary: AST.BinaryExpr): string {
    const left = this.generateExpression(binary.left);
    const right = this.generateExpression(binary.right);

    // Map Mint operators to JavaScript
    const opMap: Record<string, string> = {
      '∧': '&&',
      '∨': '||',
      '≠': '!==',
      '=': '===',
      '≤': '<=',
      '≥': '>=',
      '++': '+', // String/array concatenation
      '^': '**', // Exponentiation
    };

    const op = opMap[binary.operator] || binary.operator;

    // Handle special operators
    if (binary.operator === '++') {
      // Array concatenation or string concatenation
      return `${left}.concat(${right})`;
    }

    return `(${left} ${op} ${right})`;
  }

  private generateUnary(unary: AST.UnaryExpr): string {
    const operand = this.generateExpression(unary.operand);
    const opMap: Record<string, string> = {
      '¬': '!',
      '-': '-',
    };
    const op = opMap[unary.operator] || unary.operator;
    return `${op}${operand}`;
  }

  private generateMatch(match: AST.MatchExpr): string {
    // Generate an IIFE that implements pattern matching
    const scrutinee = this.generateExpression(match.scrutinee);

    // For now, implement simple pattern matching
    // This could be optimized later
    const lines: string[] = [];
    lines.push(`(() => {`);
    lines.push(`  const __match = ${scrutinee};`);

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
      lines.push(`    return ${body};`);
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
    // Generate IIFE for let binding
    const value = this.generateExpression(letExpr.value);
    const body = this.generateExpression(letExpr.body);
    const bindings = this.generatePatternBindings(letExpr.pattern, '__value');

    return `(() => {
  const __value = ${value};
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
    return `${object}.${access.field}`;
  }

  private generateIndex(index: AST.IndexExpr): string {
    const object = this.generateExpression(index.object);
    const idx = this.generateExpression(index.index);
    return `${object}[${idx}]`;
  }

  private generatePipeline(pipeline: AST.PipelineExpr): string {
    const left = this.generateExpression(pipeline.left);
    const right = this.generateExpression(pipeline.right);

    // |> is function application: x |> f  becomes  f(x)
    if (pipeline.operator === '|>') {
      return `${right}(${left})`;
    }

    // >> is function composition: f >> g  becomes  (x) => g(f(x))
    if (pipeline.operator === '>>') {
      return `((x) => ${right}(${left}(x)))`;
    }

    // << is reverse composition: f << g  becomes  (x) => f(g(x))
    if (pipeline.operator === '<<') {
      return `((x) => ${left}(${right}(x)))`;
    }

    return `${left} ${pipeline.operator} ${right}`;
  }

  private emit(code: string): void {
    const indentation = '  '.repeat(this.indent);
    this.output.push(indentation + code + '\n');
  }

  private sanitizeName(name: string): string {
    return name.replace(/[^a-zA-Z0-9_]/g, '_');
  }
}

/**
 * Compile a Mint program to JavaScript
 */
export function compile(program: AST.Program): string {
  const generator = new JavaScriptGenerator();
  return generator.generate(program);
}
