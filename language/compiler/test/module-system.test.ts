import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { tokenize } from '../src/lexer/lexer.js';
import { parse } from '../src/parser/parser.js';
import { typeCheck } from '../src/typechecker/index.js';
import type { InferenceType } from '../src/typechecker/types.js';

function parseProgram(code: string) {
  return parse(tokenize(code));
}

describe('Module system (exports + typed imports)', () => {
  test('parses explicit export on function/type/const', () => {
    const ast = parseProgram(
      'export λf(x:ℤ)→ℤ=x\n' +
      'export tTodo={id:ℤ}\n' +
      'export cversion:𝕊=\"1\"\n'
    );

    assert.equal(ast.declarations[0].type, 'FunctionDecl');
    assert.equal((ast.declarations[0] as any).isExported, true);
    assert.equal(ast.declarations[1].type, 'TypeDecl');
    assert.equal((ast.declarations[1] as any).isExported, true);
    assert.equal(ast.declarations[2].type, 'ConstDecl');
    assert.equal((ast.declarations[2] as any).isExported, true);
  });

  test('rejects export test declarations', () => {
    assert.throws(() => parseProgram('export test \"x\" { ⊤ }\n'), /Cannot export test declarations/i);
  });

  test('typed Mint import namespace allows exported member access', () => {
    const ast = parseProgram(
      'i src/foo\n' +
      'test \"uses imported fn\" {\n' +
      '  src/foo.verse(2)=\"ok\"\n' +
      '}\n'
    );

    const fields = new Map<string, InferenceType>();
    fields.set('verse', {
      kind: 'function',
      params: [{ kind: 'primitive', name: 'Int' }],
      returnType: { kind: 'primitive', name: 'String' },
      effects: new Set(),
    });

    assert.doesNotThrow(() =>
      typeCheck(ast, 'i src/foo\ntest \"uses imported fn\" {\n  src/foo.verse(2)=\"ok\"\n}\n', {
        importedNamespaces: new Map([
          ['src/foo', { kind: 'record', fields }],
        ]),
      })
    );
  });

  test('typed Mint import namespace rejects non-exported member access', () => {
    const ast = parseProgram(
      'i src/foo\n' +
      'test \"uses hidden fn\" {\n' +
      '  src/foo.hidden(2)=\"ok\"\n' +
      '}\n'
    );

    const fields = new Map<string, InferenceType>();
    fields.set('verse', {
      kind: 'function',
      params: [{ kind: 'primitive', name: 'Int' }],
      returnType: { kind: 'primitive', name: 'String' },
      effects: new Set(),
    });

    assert.throws(() =>
      typeCheck(ast, 'i src/foo\ntest \"uses hidden fn\" {\n  src/foo.hidden(2)=\"ok\"\n}\n', {
        importedNamespaces: new Map([
          ['src/foo', { kind: 'record', fields }],
        ]),
      })
    , /does not export member 'hidden'/i);
  });
});
