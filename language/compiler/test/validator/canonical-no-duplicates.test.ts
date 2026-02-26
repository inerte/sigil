/**
 * Test suite for canonical form - no duplicate declarations
 *
 * Sigil enforces ONE canonical declaration per name.
 * No duplicate types, externs, imports, consts, or functions allowed.
 */

import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { tokenize } from '../../src/lexer/lexer.js';
import { parse } from '../../src/parser/parser.js';
import { validateCanonicalForm } from '../../src/validator/canonical.js';

describe('Canonical Form - No Duplicate Declarations', () => {

  describe('REJECT: Duplicate Types', () => {
    test('duplicate type declarations should fail', () => {
      const code = `
        t Foo={x:ℤ}
        t Foo={x:ℤ}
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate type declaration: "Foo"/);
    });

    test('different type names should succeed', () => {
      const code = `
        t Bar={y:ℤ}
        t Foo={x:ℤ}
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.doesNotThrow(() => validateCanonicalForm(ast));
    });
  });

  describe('REJECT: Duplicate Externs', () => {
    test('duplicate extern declarations should fail', () => {
      const code = `
        e console
        e console
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate extern declaration: "console"/);
    });

    test('different extern modules should succeed', () => {
      const code = `
        e console
        e fs
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.doesNotThrow(() => validateCanonicalForm(ast));
    });

    test('duplicate dotted extern declarations should fail', () => {
      const code = `
        e fs⋅promises
        e fs⋅promises
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate extern declaration: "fs⋅promises"/);
    });
  });

  describe('REJECT: Duplicate Imports', () => {
    test('duplicate import declarations should fail', () => {
      const code = `
        i stdlib⋅string
        i stdlib⋅string
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate import declaration: "stdlib⋅string"/);
    });

    test('different import modules should succeed', () => {
      const code = `
        i stdlib⋅list
        i stdlib⋅string
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.doesNotThrow(() => validateCanonicalForm(ast));
    });
  });

  describe('REJECT: Duplicate Consts', () => {
    test('duplicate const declarations should fail', () => {
      const code = `
        c my_const:ℤ=42
        c my_const:ℤ=43
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate const declaration: "my_const"/);
    });

    test('different const names should succeed', () => {
      const code = `
        c const_a:ℤ=42
        c const_b:ℤ=43
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.doesNotThrow(() => validateCanonicalForm(ast));
    });
  });

  describe('REJECT: Duplicate Functions', () => {
    test('duplicate function declarations should fail', () => {
      const code = `
        λfoo()→𝕌=()
        λfoo()→𝕌=()
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate function declaration: "foo"/);
    });

    test('different function names should succeed', () => {
      const code = `
        λbar()→𝕌=()
        λfoo()→𝕌=()
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.doesNotThrow(() => validateCanonicalForm(ast));
    });

    test('duplicate function with different signatures should still fail', () => {
      const code = `
        λfoo(x:ℤ)→ℤ=x
        λfoo(x:ℤ,y:ℤ)→ℤ=x+y
        λmain()→!IO 𝕌=()
      `;
      const tokens = tokenize(code);
      const ast = parse(tokens);

      assert.throws(() => validateCanonicalForm(ast), /Duplicate function declaration: "foo"/);
    });
  });

  // Note: Test declarations can only appear in project tests/ directories
  // so we cannot test duplicate test detection in unit tests
});
