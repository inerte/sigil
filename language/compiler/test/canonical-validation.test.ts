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

  describe('Formatting validation', () => {
    test('rejects multiple consecutive blank lines', () => {
      const code = `λfoo()→ℤ=1


λmain()→ℤ=foo()
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-BLANK-LINES');
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
        assert.strictEqual(result.error.code, 'SIGIL-CANON-TRAILING-WHITESPACE');
      }
    });

    test('rejects missing EOF newline', () => {
      const code = 'λmain()→𝕌=()';  // No trailing newline

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-EOF-NEWLINE');
      }
    });

    test('accepts properly formatted code', () => {
      const code = `λfoo()→ℤ=1

λmain()→ℤ=foo()
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });
  });

  describe('Type checking', () => {
    test('rejects type mismatch in FFI call', () => {
      // console.log expects string but receives integer
      const code = `e console : { log : λ(𝕊) → 𝕌 }

λbad()→𝕌=console.log(42)
λmain()→𝕌=()
`;

      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-TYPE-ERROR');
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

  describe('Filename validation', () => {
    test('rejects uppercase in filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'UserService.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-CASE');
      }
    });

    test('rejects underscores in filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'user_service.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-INVALID-CHAR');
      }
    });

    test('rejects special characters in filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'user@service.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-INVALID-CHAR');
      }
    });

    test('rejects spaces in filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'user service.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-INVALID-CHAR');
      }
    });

    test('rejects filename starting with hyphen', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, '-hello.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-FORMAT');
      }
    });

    test('rejects filename ending with hyphen', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'hello-.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-FORMAT');
      }
    });

    test('rejects consecutive hyphens in filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'hello--world.sigil');

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-FILENAME-FORMAT');
      }
    });

    test('accepts valid kebab-case filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, 'user-service.sigil');

      assert.strictEqual(result.ok, true);
    });

    test('accepts numbers in filename', () => {
      const code = `λmain()→𝕌=()
`;
      const result = compileFromString(code, '01-introduction.sigil');

      assert.strictEqual(result.ok, true);
    });

    test('accepts .lib.sigil extension', () => {
      const code = `λfoo()→ℤ=1
`;
      const result = compileFromString(code, 'ffi-node-console.lib.sigil');

      assert.strictEqual(result.ok, true);
    });
  });

  describe('Parameter and effect ordering', () => {
    test('rejects non-alphabetical parameter order', () => {
      const code = `λfoo(z:ℤ,a:ℤ)→ℤ=z+a
λmain()→ℤ=foo(1,2)
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-PARAM-ORDER');
      }
    });

    test('accepts alphabetical parameter order', () => {
      const code = `λfoo(a:ℤ,z:ℤ)→ℤ=a+z
λmain()→ℤ=foo(1,2)
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('rejects non-alphabetical effect order', () => {
      const code = `λfoo()→!Network !IO 𝕌=()
λmain()→𝕌=()
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-EFFECT-ORDER');
      }
    });

    test('accepts alphabetical effect order', () => {
      const code = `λfoo()→!IO !Network 𝕌=()
λmain()→𝕌=()
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('handles single parameter (no ordering required)', () => {
      const code = `λfoo(x:ℤ)→ℤ=x
λmain()→ℤ=foo(5)
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('handles no parameters (no ordering required)', () => {
      const code = `λfoo()→ℤ=42
λmain()→ℤ=foo()
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('validates lambda parameter ordering', () => {
      const code = `λfoo()→ℤ=(λ(z:ℤ,a:ℤ)→ℤ=z+a)(1,2)
λmain()→ℤ=foo()
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-PARAM-ORDER');
      }
    });

    test('accepts alphabetical lambda parameter order', () => {
      const code = `λfoo()→ℤ=(λ(a:ℤ,z:ℤ)→ℤ=a+z)(1,2)
λmain()→ℤ=foo()
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, true);
    });

    test('validates parameter ordering with multiple parameters', () => {
      const code = `λfoo(y:ℤ,z:ℤ,x:ℤ)→ℤ=x+y+z
λmain()→ℤ=foo(1,2,3)
`;
      const result = compileFromString(code);

      assert.strictEqual(result.ok, false);
      if (!result.ok) {
        assert.strictEqual(result.error.code, 'SIGIL-CANON-PARAM-ORDER');
        // Should suggest correct order: x, y, z
        assert.match(result.error.message, /x, y, z/);
      }
    });
  });
});
