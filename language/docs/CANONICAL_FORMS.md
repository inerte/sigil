# Canonical Forms in Sigil

## Philosophy: Zero Ambiguity

Sigil enforces **canonical forms** at every level - from algorithms to formatting. Every valid Sigil program has exactly ONE syntactic representation.

This ensures:
- **Training data quality**: No syntactic variations polluting LLM datasets
- **Deterministic generation**: AI models generate exactly one correct form
- **Byte-for-byte reproducibility**: Same semantics = same bytes
- **Zero ambiguity**: No judgment calls, no style debates

## Two Levels of Enforcement

### 1. Semantic Canonical Forms (Algorithm Level)

Enforced by: **Canonical form validator** (`validator/canonical.ts`)

**What's blocked:**
- Tail-call optimization (TCO)
- Accumulator-passing style
- Continuation-passing style (CPS)
- Helper functions that encode iterative patterns
- Closure-based state accumulation
- Boolean pattern matching when value matching works
- **Files with ambiguous purpose** (neither executable nor library)
- **Files with dual purpose** (both executable and library)

**File Purpose Rule (Legacy - see File Extension Convention below):**

NOTE: This section describes the old validation approach. Modern Sigil uses file extensions (`.lib.sigil` vs `.sigil`) to distinguish file purpose. See "File Extension Convention" section below for current canonical approach.

### File Extension Convention

Sigil uses file extensions to distinguish libraries from executables at the filesystem level.

**Extension rules:**
- `.lib.sigil` → Libraries (all functions visible, no main)
- `.sigil` → Executables (have main, not imported except by tests)
- `tests/*.sigil` → Tests (have main and test blocks, can import from anywhere)

**Examples:**

✅ VALID - Library file:
```sigil
// math.lib.sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
λmultiply(x:ℤ,y:ℤ)→ℤ=x*y
// All functions automatically visible to importers
```

✅ VALID - Executable file:
```sigil
// calculator.sigil
i src⋅math

λmain()→ℤ=src⋅math.add(2,3)
```

✅ VALID - Test file:
```sigil
// tests/math.sigil
i src⋅math

λmain()→𝕌=()

test "addition works" {
  src⋅math.add(2,3)=5
}
```

❌ REJECTED - .lib.sigil with main():
```sigil
// math.lib.sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
λmain()→ℤ=42  // ERROR: SIGIL-CANON-LIB-NO-MAIN
```

❌ REJECTED - .sigil without main (and not in tests/):
```sigil
// math.sigil
λhelper(x:ℤ)→ℤ=x*2  // ERROR: SIGIL-CANON-EXEC-NEEDS-MAIN
// Solution: Add λmain() or rename to math.lib.sigil
```

**Import statements:**

Import statements use logical module names, not file extensions:

```sigil
i stdlib⋅list      // Resolves to stdlib/list.lib.sigil
i stdlib⋅numeric   // Resolves to stdlib/numeric.lib.sigil
i src⋅math         // Resolves to src/math.lib.sigil
```

**Test file special visibility:**

Test files in `tests/` directories can import from ANY file (including `.sigil` executables) and access ALL functions, even those not in `.lib.sigil` files. This enables testing internal implementation details.

**Rationale:**
- Tools can determine file purpose from filename alone (no need to read contents)
- Clear at a glance in file trees and directory listings
- Import resolution is deterministic
- No `export` keyword needed - everything is visible
- Reinforces "ONE WAY" canonical philosophy

#### Test Location Rule

Test blocks can ONLY appear in files under `tests/` directories.

**Canonical enforcement:**

```sigil
✅ VALID - Test file in tests/ directory:
// tests/list-predicates.sigil
i stdlib⋅list

λmain()→𝕌=()

test "list.in_bounds checks valid indexes" {
  stdlib⋅list.in_bounds(0,[10,20,30])=⊤
}

❌ REJECTED - Test blocks outside tests/ directory:
// examples/fibonacci.sigil
λfibonacci(n:ℤ)→ℤ=...

test "fibonacci works" {  // ERROR: SIGIL-CANON-TEST-LOCATION
  fibonacci(5)=5
}

❌ REJECTED - Test file without main():
// tests/my-test.sigil
test "example" { ⊤ }
// ERROR: SIGIL-CANON-FILE-PURPOSE-NONE
// Hint: Test files are executables and must have a main() function.

❌ REJECTED - Test file with exports (not applicable with .lib.sigil convention):
// tests/my-test.sigil
// Test files are .sigil executables, not .lib.sigil libraries
test "example" { ⊤ }
λmain()→𝕌=()
```

**Rationale:**
- Tests are executables with test blocks, not a separate category
- Location-based enforcement prevents scattered test code
- `main()→𝕌` is a marker - actual execution via test runner
- Tests use `.sigil` extension (executables), not `.lib.sigil` (libraries)

**What's allowed:**
- Primitive recursion (direct recursive calls)
- Direct style (no continuations)
- Value-based pattern matching
- Utility/predicate functions

See `docs/ACCUMULATOR_DETECTION.md` for details.

### 2. Surface Form Canonical Forms (Formatting Level)

Enforced by: **Surface form validator** (`validator/surface-form.ts`)

**What's enforced:**

#### Rule 1: Final Newline Required

Every file must end with `\n`.

```sigil
✅ VALID:
λmain()→ℤ=1
[newline]

❌ REJECTED - no final newline:
λmain()→ℤ=1[EOF]
```

**Error message:**
```
Error: File must end with a newline
```

#### Rule 2: No Trailing Whitespace

Lines cannot end with spaces or tabs.

```sigil
❌ REJECTED:
λmain()→ℤ=1
⟦ Error: Line 1 has trailing whitespace ⟧
```

**Error message:**
```
Error: Line N has trailing whitespace
```

#### Rule 3: Maximum One Consecutive Blank Line

Only one blank line allowed between declarations.

```sigil
✅ VALID:
λa()→ℤ=1

λb()→ℤ=2

❌ REJECTED:
λa()→ℤ=1


λb()→ℤ=2
```

**Error message:**
```
Error: Multiple blank lines at line N (only one consecutive blank line allowed)
```

#### Rule 4: Equals Sign Placement (Context-Dependent)

The presence/absence of `=` depends on the function body type.

**Regular expressions require `=`:**
```sigil
✅ VALID:
λdouble(x:ℤ)→ℤ=x*2
λsum(xs:[ℤ])→ℤ=xs⊕(λ(a,x)→a+x)⊕0

❌ REJECTED:
λdouble(x:ℤ)→ℤ x*2
⟦ Error: Expected "=" before function body (canonical form: λf()→T=...) ⟧
```

**Match expressions forbid `=`:**
```sigil
✅ VALID:
λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}
λsign(n:ℤ)→𝕊≡(n>0,n<0){(⊤,⊥)→"positive"|...}

❌ REJECTED:
λfactorial(n:ℤ)→ℤ=≡n{...}
⟦ Error: Unexpected "=" before match expression (canonical form: λf()→T≡...) ⟧
```

**Rationale:** The `≡` operator already signals "this is the body", making `=` redundant and non-canonical.

#### Rule 5: Declaration Category Ordering

Module-level declarations must appear in strict categorical order:

**`t → e → i → c → λ → test`**

```sigil
✅ VALID:
t User = { name: 𝕊, age: ℤ }
e console
i stdlib⋅list
c MAX_SIZE : ℤ = 100
λmain()→ℤ=0
test "example" { ... }

❌ REJECTED - extern before type:
e console
t User = { name: 𝕊, age: ℤ }
⟦ Error: Type declarations must come before extern declarations ⟧
```

**Category meanings:**
- `t` = types (must come first so externs can reference them)
- `e` = externs (FFI imports)
- `i` = imports (Sigil modules)
- `c` = consts
- `λ` = functions
- `test` = tests

**Within-category ordering:**
- Alphabetically by name within each category

**Error message:**
```
Canonical Ordering Error: Wrong category position

Found: e (extern) at line 5
Expected: extern declarations must come before import declarations

Category order: t → e → i → c → λ → test
  t    = types
  e    = externs (FFI imports)
  i    = imports (Sigil modules)
  c    = consts
  λ    = functions
  test = tests

Move all extern declarations to appear before import declarations.

Sigil enforces ONE way: canonical declaration ordering.
```

**Rationale:** Types-first ordering enables typed FFI declarations to reference named types. This is a language design choice that prioritizes correctness over convenience.

## Already Enforced (Lexer Level)

The lexer rejects:

### Tab Characters
```sigil
❌ REJECTED:
λmain()→ℤ=1[TAB]2
⟦ Error: Tab characters not allowed - use spaces ⟧
```

### Standalone `\r`
```sigil
❌ REJECTED:
λmain()→ℤ=1\r\n
⟦ Error: Standalone \r not allowed - use \n for line breaks ⟧
```

Only `\n` is accepted for line breaks (or `\r\n` as a unit on Windows).

## Compilation Pipeline

Surface form validation runs BEFORE tokenization:

```
1. Read source file
2. Validate surface form ← enforces formatting
3. Tokenize            ← enforces tabs, \r
4. Parse
5. Validate canonical form ← enforces algorithms
6. Type check
7. Compile to TypeScript
```

This ensures all canonical rules are checked early with clear error messages.

## Error Messages

All surface form errors include:
- Filename
- Line number (where applicable)
- Column number (where applicable)
- Clear description of the violation
- Hint about the canonical form

Examples:

```
Error: File must end with a newline
SurfaceFormError: File must end with a newline
    at validateSurfaceForm (...)

Error: Line 5 has trailing whitespace
SurfaceFormError: Line 5 has trailing whitespace
    at validateSurfaceForm (...)

Error: Multiple blank lines at line 10 (only one consecutive blank line allowed)
SurfaceFormError: Multiple blank lines at line 10 (only one consecutive blank line allowed)
    at validateSurfaceForm (...)

Error: Parse error at line 3, column 15: Expected "=" before function body (canonical form: λf()→T=...)
Got: IDENTIFIER (x)

Error: Parse error at line 7, column 20: Unexpected "=" before match expression (canonical form: λf()→T≡...)
Got: MATCH (≡)
```

## Testing Your Code

All files must pass surface form validation:

```bash
# This will fail if formatting is wrong
node compiler/dist/cli.js compile myfile.sigil

# Common failures:
# - Missing final newline → add newline at end
# - Trailing spaces → remove spaces from line ends
# - Multiple blank lines → remove extra blank lines
# - Wrong = placement → check if using ≡ (match) or regular expression
```

## For AI Agents and LLMs

**When generating Sigil code:**

1. Always end files with `\n`
2. Never add trailing spaces
3. Use exactly one blank line between top-level declarations
4. Use `=` for regular expressions: `λf()→T=expr`
5. Omit `=` for match expressions: `λf()→T≡value{...}`
6. Use spaces (never tabs)
7. Use `\n` for line breaks (never `\r`)

**Remember:** There is exactly ONE valid way to write each program. If you generate non-canonical code, compilation will fail.

## Future Enhancements

Potential future rules (not yet implemented):

- Operator spacing (dense `a+b` or spaced `a + b`)
- Comma spacing (`(a,b)` vs `(a, b)`)
- Colon spacing (`a:ℤ` vs `a: ℤ`)
- Indentation consistency (2 spaces per level)
- Parenthesis placement in nested expressions

These will be added only if they improve deterministic generation for AI models.

## Why This Matters

**Traditional languages:**
- Multiple ways to format (tabs vs spaces, brace styles, etc.)
- Style guides attempt to standardize
- Linters enforce conventions
- Still allows variations

**Sigil:**
- Compiler enforces ONE way
- No style debates
- No linter needed
- Perfect training data for AI

This is fundamental to Sigil's mission: be the first language designed for AI code generation from the ground up.
