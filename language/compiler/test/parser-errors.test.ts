/**
 * Parser Error Tests
 *
 * Tests that the parser correctly rejects invalid syntax patterns.
 * These tests verify that complex or unsupported syntax is rejected during parsing.
 */

import { describe, test } from 'node:test';
import assert from 'node:assert';
import { compileFromString } from '../src/api.js';

describe('Parser Error Handling', () => {
  describe('Tuple matching rejection', () => {
    test('rejects tuple pattern matching in match expressions', () => {
      // Binary search using tuple matching (not supported)
      // This pattern may parse, so just verify it fails
      const code = `λbinary_search(xs:[ℤ],target:ℤ,low:ℤ,high:ℤ)→ℤ=
  ≡(high<low,xs[0]=target,xs[0]<target){
    (⊤,_,_)→-1|
    (⊥,⊤,_)→0|
    (⊥,⊥,⊤)→binary_search(xs,target,1,high)|
    (⊥,⊥,⊥)→binary_search(xs,target,low,0)
  }

λmain()→ℤ=binary_search([1,3,5],3,0,2)
`;

      const result = compileFromString(code);

      // Just verify it rejects
      assert.strictEqual(result.ok, false);
    });
  });

  describe('Complex lambda nesting rejection', () => {
    test('rejects deeply nested inline lambdas', () => {
      // Insanely nested factorial using inline lambdas
      const code = `λmain()→ℤ=(λ(x:ℤ)→≡x{0→1|x→x*(λ(y:ℤ)→≡y{0→1|y→y*(λ(z:ℤ)→≡z{0→1|z→z*(λ(a:ℤ)→≡a{0→1|a→a*1})(z-1)})(y-1)})(x-1)})(4)
`;

      const result = compileFromString(code);

      // This pattern should fail somewhere in the pipeline
      assert.strictEqual(result.ok, false);
    });
  });

  describe('Y-combinator pattern rejection', () => {
    test('rejects Y-combinator factorial', () => {
      // Y-combinator implementation
      const code = `λy(f:λ(λ(ℤ)→ℤ)→λ(ℤ)→ℤ)→λ(ℤ)→ℤ=λ(x:ℤ)→f(y(f))(x)
λfactGen(rec:λ(ℤ)→ℤ)→λ(ℤ)→ℤ=λ(n:ℤ)→≡n{0→1|1→1|n→n*rec(n-1)}
λmain()→ℤ=y(factGen)(5)
`;

      const result = compileFromString(code);

      // Y-combinator should fail somewhere
      assert.strictEqual(result.ok, false);
    });
  });

  describe('Invalid syntax rejection', () => {
    test('rejects incomplete function declaration', () => {
      const code = `λfoo(
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
    });

    test('rejects invalid type syntax', () => {
      const code = `λfoo(x:InvalidType)→ℤ=42
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
    });

    test('rejects unclosed match expression', () => {
      const code = `λfoo(n:ℤ)→ℤ≡n{0→1
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
    });
  });

  describe('Valid complex patterns', () => {
    test('accepts well-formed recursive function', () => {
      const code = `λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
λmain()→ℤ=factorial(5)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('accepts nested match expressions', () => {
      const code = `λfoo(x:ℤ,y:ℤ)→ℤ≡x{0→≡y{0→0|y→y}|x→x+y}
λmain()→ℤ=foo(5,3)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });
  });
});
