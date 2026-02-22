# Canonical Form Enforcement

Mint enforces "ONE way to do things" at the **compiler level**, not just through documentation.

## The Problem

Traditional languages allow multiple ways to write the same algorithm. For example, factorial can be written:

1. **Simple recursion**: `factorial(n) = n * factorial(n-1)`
2. **Tail recursion with accumulator**: `factorial(n, acc) = factorial(n-1, n*acc)`
3. **Iterative with loop**: `for i in range...`

This creates ambiguity for LLMs, leading to inconsistent code generation.

## Mint's Solution

**Make alternative patterns syntactically impossible.**

### Rule 1: Recursive Functions → ONE PRIMITIVE Parameter Only

**Enforced by:** Compiler rejects recursive functions with:
- 2+ parameters
- Collection-type parameters (lists, tuples, maps)

**Why:**
- Accumulator pattern requires 2+ parameters (e.g., `n` and `acc`)
- Collection types can encode multiple values within one parameter

**Examples:**

```mint
✅ COMPILES - canonical form (primitive parameter):
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}

❌ COMPILE ERROR - two parameters:
λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→factorial(n-1,n*acc)}

❌ COMPILE ERROR - list parameter (loophole attempt):
λfactorial(state:[ℤ])→ℤ≡state{[0,acc]→acc|[n,acc]→factorial([n-1,n*acc])}
```

**Error messages:**
```
# Multi-parameter error:
Error: Recursive function 'factorial' has 2 parameters.
Recursive functions must have exactly ONE primitive parameter.
This prevents accumulator-style tail recursion.

# Collection-type parameter error:
Error: Recursive function 'factorial' has a collection-type parameter.
Parameter type: [Int]

Recursive functions must have a PRIMITIVE parameter (ℤ, 𝕊, 𝔹, etc).
Collection types (lists, tuples, maps) can encode multiple values,
which enables accumulator-style tail recursion.

Example canonical form:
  λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}

Mint enforces ONE way to write recursive functions.
```

### Rule 2: No Helper Functions

**Enforced by:** Compiler rejects functions only called by one other function

**Why:** Helper functions enable wrapper patterns (e.g., `factorial(n) = helper(n, 1)`)

**Example:**

```mint
✅ COMPILES - single function:
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
λmain()→ℤ=factorial(5)

❌ COMPILE ERROR:
λhelper(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→helper(n-1,n*acc)}
λfactorial(n:ℤ)→ℤ=helper(n,1)
λmain()→ℤ=factorial(5)
```

**Error message:**
```
Error: Function 'helper' is only called by 'factorial'.
Helper functions are not allowed.

Options:
  1. Inline 'helper' into 'factorial'
  2. Export 'helper' and use it elsewhere

Mint enforces ONE way: each function stands alone.
```

## Implementation

**Location:** `compiler/src/validator/canonical.ts`

**Pipeline:**
```
Source → Tokenize → Parse → Validate Canonical Form → Type Check → Codegen
                                       ↑
                               Enforces ONE way
```

**Validation runs:**
- After parsing (AST available)
- Before type checking (fail fast)
- In both `compile` and `run` commands

## Why This Matters

### Traditional Approach
- Document: "Please write code this way"
- LLM: *generates code in alternative style*
- Human: *manually fixes*
- Result: Inconsistent codebase

### Mint Approach
- Compiler: **REJECTS** alternative patterns
- LLM: Gets compile error immediately
- LLM: Generates canonical form
- Result: **100% consistency**

## Benefits

1. **Zero Ambiguity**: LLMs cannot generate non-canonical code
2. **Immediate Feedback**: Compile errors guide to correct form
3. **Training Data**: All Mint code in the wild is canonical
4. **No Choice Paralysis**: One way = no decisions needed
5. **Future-Proof**: Even new LLMs learn the ONE way

## Testing

Test files in `src/`:
- `factorial-valid.mint` - ✅ Compiles successfully
- `factorial-invalid-accumulator.mint` - ❌ Rejects 2-parameter recursion
- `factorial-invalid-helper.mint` - ❌ Rejects helper function pattern

Try them:
```bash
# This works
node compiler/dist/cli.js run src/factorial-valid.mint

# These fail with helpful errors
node compiler/dist/cli.js compile src/factorial-invalid-accumulator.mint
node compiler/dist/cli.js compile src/factorial-invalid-helper.mint
```

## Philosophy

**"You can't write it wrong if the language won't let you."**

This is the key to machine-first programming languages. Instead of relying on:
- Style guides (humans forget)
- Linters (can be disabled)
- Code review (subjective)

We make the language **fundamentally incapable** of expressing alternatives.

Like how JavaScript can't express goto, or how Rust can't express null pointer dereference, Mint can't express multiple ways to solve the same problem.

## Future Extensions

Other patterns we could enforce:
- ❌ `if/else` → ✅ Only pattern matching
- ❌ Multiple loop constructs → ✅ Only `map/filter/reduce`
- ❌ Null checks → ✅ Only `Option` type
- ❌ Try/catch → ✅ Only `Result` type

Each restriction eliminates ambiguity and makes LLM code generation more reliable.
