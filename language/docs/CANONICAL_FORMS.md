# Canonical Forms in Mint

## Philosophy: Zero Ambiguity

Mint enforces **canonical forms** at every level - from algorithms to formatting. Every valid Mint program has exactly ONE syntactic representation.

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

```mint
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

```mint
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

```mint
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
```mint
✅ VALID:
λdouble(x:ℤ)→ℤ=x*2
λsum(xs:[ℤ])→ℤ=xs⊕(λ(a,x)→a+x)⊕0

❌ REJECTED:
λdouble(x:ℤ)→ℤ x*2
⟦ Error: Expected "=" before function body (canonical form: λf()→T=...) ⟧
```

**Match expressions forbid `=`:**
```mint
✅ VALID:
λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}
λsign(n:ℤ)→𝕊≡(n>0,n<0){(⊤,⊥)→"positive"|...}

❌ REJECTED:
λfactorial(n:ℤ)→ℤ=≡n{...}
⟦ Error: Unexpected "=" before match expression (canonical form: λf()→T≡...) ⟧
```

**Rationale:** The `≡` operator already signals "this is the body", making `=` redundant and non-canonical.

## Already Enforced (Lexer Level)

The lexer rejects:

### Tab Characters
```mint
❌ REJECTED:
λmain()→ℤ=1[TAB]2
⟦ Error: Tab characters not allowed - use spaces ⟧
```

### Standalone `\r`
```mint
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
node compiler/dist/cli.js compile myfile.mint

# Common failures:
# - Missing final newline → add newline at end
# - Trailing spaces → remove spaces from line ends
# - Multiple blank lines → remove extra blank lines
# - Wrong = placement → check if using ≡ (match) or regular expression
```

## For AI Agents and LLMs

**When generating Mint code:**

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

**Mint:**
- Compiler enforces ONE way
- No style debates
- No linter needed
- Perfect training data for AI

This is fundamental to Mint's mission: be the first language designed for AI code generation from the ground up.
