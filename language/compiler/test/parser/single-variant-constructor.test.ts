/**
 * Tests for single-variant type constructor registration.
 * Ensures that single-variant sum types are correctly identified and their constructors are registered.
 */

import { describe, test } from 'node:test';
import assert from 'node:assert';
import { compileFromString } from '../../src/api.js';

describe('Single-Variant Constructor Registration', () => {
  test('single-variant constructor with one arg should compile', () => {
    const code = 't Result=Ok(𝕊)\nλmain()→Result=Ok("hello")\n';
    const result = compileFromString(code);
    if (!result.ok) {
      console.error('Compile error:', result.error);
    }
    assert.strictEqual(result.ok, true, 'Should compile successfully');
  });

  test('single-variant constructor with multiple args should compile', () => {
    const code = 't Point=Coord(ℤ,ℤ)\nλmain()→Point=Coord(1,2)\n';
    const result = compileFromString(code);
    assert.strictEqual(result.ok, true, 'Should compile successfully');
  });

  test('single-variant constructor with no args should compile', () => {
    const code = 't Unit=Empty()\nλmain()→Unit=Empty()\n';
    const result = compileFromString(code);
    assert.strictEqual(result.ok, true, 'Should compile successfully');
  });

  test('multi-variant constructor should still work', () => {
    const code = 't Result=Ok(𝕊)|Err(𝕊)\nλmain()→Result=Ok("hello")\n';
    const result = compileFromString(code);
    assert.strictEqual(result.ok, true, 'Should compile successfully');
  });

  test('single-variant constructor in pattern match', () => {
    const code = 't Maybe=Just(ℤ)\nλmain()→ℤ=unwrap(Just(42))\nλunwrap(m:Maybe)→ℤ≡m{Just(x)→x}\n';
    const result = compileFromString(code);
    assert.strictEqual(result.ok, true, 'Should compile successfully');
  });

  test('unbound variable error for non-existent constructor', () => {
    const code = 't Result=Ok(𝕊)\nλmain()→Result=Err("fail")\n';
    const result = compileFromString(code);
    assert.strictEqual(result.ok, false);
    if (!result.ok) {
      // Should get unbound variable error for Err, not Ok
      assert.match(result.error.message, /Err/);
    }
  });
});
