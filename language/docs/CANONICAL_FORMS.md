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

### Filename Format Rule

**Rule**: Filenames must be lowercase with hyphens for word separation.

**Rationale**: Enforce one canonical filename format for consistency and filesystem safety.

**Format**:
- Basename (before extension) must match: `^[a-z0-9]+(-[a-z0-9]+)*$`
- Lowercase letters only (a-z)
- Numbers allowed (0-9)
- Hyphens for word separation (-)
- No underscores, spaces, or special characters

**Valid:**
```
hello.sigil
user-service.lib.sigil
01-introduction.sigil
ffi-node-console.lib.sigil
```

**Invalid:**
```
UserService.sigil           # uppercase → SIGIL-CANON-FILENAME-CASE
user_service.lib.sigil      # underscore → SIGIL-CANON-FILENAME-INVALID-CHAR
user service.sigil          # space → SIGIL-CANON-FILENAME-INVALID-CHAR
user@service.sigil          # special char → SIGIL-CANON-FILENAME-INVALID-CHAR
-hello.sigil                # starts with hyphen → SIGIL-CANON-FILENAME-FORMAT
hello-.sigil                # ends with hyphen → SIGIL-CANON-FILENAME-FORMAT
hello--world.sigil          # consecutive hyphens → SIGIL-CANON-FILENAME-FORMAT
```

**Error Codes**:
- `SIGIL-CANON-FILENAME-CASE` - Contains uppercase letters
- `SIGIL-CANON-FILENAME-INVALID-CHAR` - Contains underscores, spaces, or special characters
- `SIGIL-CANON-FILENAME-FORMAT` - Format violations (hyphens at edges, consecutive hyphens, empty basename)

**Rationale:**
- **Case-insensitive filesystem safety**: Prevents `User.sigil` vs `user.sigil` confusion on macOS/Windows
- **Consistent import paths**: Module names map predictably to filenames
- **One canonical way**: No choice between `user_service`, `UserService`, or `user-service`
- **Readability**: Kebab-case is clear and web-friendly

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

### 2. Formatting Rules

Enforced by: **Canonical validator** (`validator/canonical.ts`)

Sigil enforces canonical forms at all levels, including formatting.

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

#### Rule 6: Parameter Alphabetical Ordering

Function parameters must be in alphabetical order by name.

**Error code:** `SIGIL-CANON-PARAM-ORDER`

```sigil
✅ VALID - alphabetical order:
λfoo(a:ℤ,b:ℤ,c:ℤ)→ℤ=a+b+c

❌ REJECTED - non-alphabetical:
λfoo(c:ℤ,a:ℤ,b:ℤ)→ℤ=a+b+c
⟦ Error: Parameter out of alphabetical order ⟧
```

**Applies to:**
- Function declarations: `λfoo(x:ℤ,y:ℤ)→ℤ=x+y`
- Lambda expressions: `(λ(a:ℤ,b:ℤ)→ℤ=a+b)(1,2)`
- All parameter lists regardless of length

**Rationale:**
- Alphabetical ordering is deterministic and language-agnostic
- Eliminates debate about "natural" parameter ordering
- Consistent with declaration alphabetical ordering
- One canonical way to write every function signature
- Improves training data quality for AI code generation

**Error message:**
```
Parameter out of alphabetical order in function "foo"

Found: c at position 3
After: b at position 2

Parameters must be alphabetically ordered.
Expected 'c' to come before 'b'.

Alphabetical order uses Unicode code point comparison (case-sensitive).
Reorder parameters: a, b, c

Sigil enforces ONE WAY: canonical parameter ordering.
```

#### Rule 7: Effect Alphabetical Ordering

Effect annotations must be in alphabetical order.

**Error code:** `SIGIL-CANON-EFFECT-ORDER`

```sigil
✅ VALID - alphabetical order:
λfetch()→!Async !IO !Network 𝕊="data"

❌ REJECTED - non-alphabetical:
λfetch()→!Network !IO !Async 𝕊="data"
⟦ Error: Effect out of alphabetical order ⟧
```

**Standard effect order (alphabetical):**
- `!Async` before `!Error`
- `!Error` before `!IO`
- `!IO` before `!Mut`
- `!Mut` before `!Network`

**Rationale:**
- Deterministic effect declaration
- No arbitrary ordering choices
- Consistent with all other alphabetical ordering rules
- One canonical way to declare effects

**Error message:**
```
Effect out of alphabetical order in function "fetch"

Found: !IO at position 2
After: !Network at position 1

Effects must be alphabetically ordered.
Expected 'IO' to come before 'Network'.

Correct order: !Async !IO !Network

Sigil enforces ONE WAY: canonical effect ordering.
```

**Declaration ordering error message:**
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

Canonical validation runs after parsing:

```
1. Read source file
2. Tokenize            ← enforces tabs, \r
3. Parse
4. Validate canonical form ← enforces formatting, algorithms, structure
5. Type check
6. Compile to TypeScript
```

This ensures all canonical rules are checked early with clear error messages.

## Error Messages

All canonical form errors include:
- Error code (SIGIL-CANON-*)
- Filename
- Line number (where applicable)
- Column number (where applicable)
- Clear description of the violation
- Hint about the canonical form

Examples:

```
Error: SIGIL-CANON-EOF-NEWLINE
File must end with a newline
File: myfile.sigil

Error: SIGIL-CANON-TRAILING-WHITESPACE
Trailing whitespace
File: myfile.sigil
Line: 5

Error: SIGIL-CANON-BLANK-LINES
Multiple consecutive blank lines
File: myfile.sigil
Line: 10

Error: Parse error at line 3, column 15: Expected "=" before function body (canonical form: λf()→T=...)
Got: IDENTIFIER (x)

Error: Parse error at line 7, column 20: Unexpected "=" before match expression (canonical form: λf()→T≡...)
Got: MATCH (≡)
```

## Testing Your Code

All files must pass canonical validation:

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
8. **Order declarations alphabetically within categories** (types, externs, imports, consts, functions, tests)
9. **Order function parameters alphabetically by name** (`λfoo(a,b,z)` not `λfoo(z,b,a)`)
10. **Order effect annotations alphabetically** (`!Async !IO !Network` not `!Network !IO !Async`)

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
