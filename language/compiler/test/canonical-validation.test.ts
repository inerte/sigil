/**
 * Canonical Form Validation Tests
 *
 * Tests that the compiler correctly rejects non-canonical code patterns.
 * These tests use the compileFromString API to test invalid patterns without
 * needing .sigil files that fail compilation.
 */

import { describe, test } from 'node:test';
import assert from 'node:assert';
import { compileFromString } from '../src/api.js';

describe('Canonical Form Validation', () => {
  describe('Accumulator-passing style rejection', () => {
    test('rejects accumulator parameter in recursive function', () => {
      // Tail-recursive factorial with accumulator parameter
      const code = `λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→factorial(n-1,n*acc)}
λmain()→ℤ=factorial(5,1)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-RECURSION-ACCUMULATOR');
      }
    });

    test('rejects helper function with accumulator pattern', () => {
      // Helper function that uses accumulator-passing
      const code = `λhelper(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→helper(n-1,n*acc)}
λfactorial(n:ℤ)→ℤ=helper(n,1)
λmain()→ℤ=factorial(5)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-RECURSION-ACCUMULATOR');
      }
    });
  });

  describe('CPS style rejection', () => {
    test('rejects continuation-passing style recursion', () => {
      // CPS factorial that returns a function taking a continuation
      // Note: This fails at parse time, not canonical validation
      const code = `λfactorial(n:ℤ)→λ(ℤ)→ℤ≡n{0→λ(k:ℤ)→k|n→λ(k:ℤ)→factorial(n-1)(n*k)}
λmain()→ℤ=factorial(5)(1)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      // CPS style fails during parsing (function types in return position)
      if (!result.ok) {
        assert.match(result.error.code, /SIGIL-(PARSE|CANON)/);
      }
    });
  });

  describe('Alphabetical ordering enforcement', () => {
    test('rejects non-alphabetically ordered function declarations', () => {
      const code = `λzebra()→ℤ=1
λapple()→ℤ=2
λmain()→ℤ=apple()+zebra()
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-DECL-ALPHABETICAL');
      }
    });

    test('allows alphabetically ordered function declarations', () => {
      const code = `λapple()→ℤ=1
λmain()→ℤ=apple()+zebra()
λzebra()→ℤ=2
`;

      const result = compileFromString(code);

      // Should succeed (alphabetical order: apple, main, zebra)
      assert.strictEqual(result.ok, true);
    });
  });

  describe('File purpose enforcement', () => {
    test('rejects main() function in .lib.sigil file', () => {
      // Library files should not have main()
      const code = `λhelper()→ℤ=42
λmain()→ℤ=helper()
`;

      const result = compileFromString(code, 'test.lib.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-LIB-NO-MAIN');
      }
    });

    test('allows main() function in .sigil file', () => {
      const code = `λmain()→ℤ=42
`;

      const result = compileFromString(code, 'test.sigil');

      assert.strictEqual(result.ok, true);
    });

    test('allows functions without main() in .lib.sigil file', () => {
      const code = `λhelper()→ℤ=42
λutil()→ℤ=helper()+1
`;

      const result = compileFromString(code, 'test.lib.sigil');

      assert.strictEqual(result.ok, true);
    });
  });

  describe('Surface form validation', () => {
    test('rejects multiple consecutive blank lines', () => {
      const code = `λfoo()→ℤ=1


λmain()→ℤ=foo()
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-SURFACE-BLANK-LINES');
      }
    });

    test('allows single blank lines between declarations', () => {
      const code = `λfoo()→ℤ=1

λmain()→ℤ=foo()
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('rejects trailing whitespace', () => {
      // Line with trailing space (added via concatenation)
      const code = 'λmain()→ℤ=42 \n';

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-SURFACE-TRAILING-WHITESPACE');
      }
    });
  });

  describe('Valid canonical patterns', () => {
    test('accepts simple recursive factorial', () => {
      const code = `λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}
λmain()→ℤ=factorial(5)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('accepts fold-based factorial', () => {
      const code = `i stdlib⋅list

λfactorial(n:ℤ)→ℤ=stdlib⋅list.fold([1,2,3,4,5],1,λ(acc:ℤ,x:ℤ)→ℤ=acc*x)
λmain()→ℤ=factorial(5)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('accepts helper function that does not use accumulator pattern', () => {
      const code = `λhelper(n:ℤ)→ℤ≡n{0→1|n→n*helper(n-1)}
λmain()→ℤ=helper(5)
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });
  });
});
