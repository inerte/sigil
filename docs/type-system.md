# Mint Type System: Bidirectional Type Checking

## Overview

Mint uses **bidirectional type checking** instead of traditional Hindley-Milner type inference.

### Why Bidirectional?

Mint's philosophy is **"ONE way to write it"**. Type annotations must be:
- **Mandatory** on all function signatures
- **Explicit** (no optional syntax)
- **Canonical** (exactly one valid representation)

This makes Hindley-Milner's primary feature (inferring types with minimal annotations) unnecessary. Bidirectional type checking is:
- **Simpler** to implement (~40% less code)
- **Better** error messages ("expected ℤ, got 𝕊" instead of "failed to unify")
- **More extensible** (natural framework for polymorphism, effects, refinements)
- **Faster** to compile (no complex constraint solving in common cases)

## Two Modes

### Synthesis (⇒): Infer type from structure

Used for expressions where type can be determined from the expression itself:
- **Literals**: `5` ⇒ `ℤ`, `"hello"` ⇒ `𝕊`, `⊤` ⇒ `𝔹`
- **Variables**: `x` ⇒ look up in environment
- **Applications**: `f(x)` ⇒ synthesize `f`, check args, return result type
- **Pattern matching**: `≡n{...}` ⇒ synthesize scrutinee, check arms have same type
- **Binary operations**: `x + y` ⇒ check operands, return result type

### Checking (⇐): Verify against expected type

Used for expressions where expected type is known from context:
- **Lambda bodies**: check against declared return type
- **Pattern match arms**: check against expected result type
- **Function arguments**: check against parameter types
- **Literals**: verify literal matches expected type

## Type Annotations

### Required Everywhere

All function signatures must have complete type annotations:

```mint
# Function declarations
λfactorial(n:ℤ)→ℤ=...

# Lambda expressions
[1,2,3]↦λ(x:ℤ)→ℤ=x*2

# Constants (when supported)
c PI:ℝ=3.14
```

### Parse Errors for Missing Annotations

The parser rejects code without type annotations:

```
Error: Expected ":" after parameter "n"
λfactorial(n)→ℤ=...
           ^
Type annotations are required (canonical form).

Error: Expected "→" after parameters for function "factorial"
λfactorial(n:ℤ)=...
               ^
Return type annotations are required (canonical form).
```

## Error Messages

Bidirectional type checking provides **excellent error messages**:

```
Error: Type mismatch in function 'main'
  Expected: ℤ
  Got: 𝕊
  Location: factorial.mint:2:16

  2 | λmain()→ℤ="hello"
    |                ^

Literal type mismatch: expected ℤ, got 𝕊
```

Compare to traditional Hindley-Milner errors:
```
Failed to unify types Int and String
(no clear location or context)
```

## Type Inference Rules

The type checker uses two main functions:

```typescript
synthesize(expr: Expr, env: Env): Type
check(expr: Expr, expectedType: Type, env: Env): void
```

### Synthesis Rules

```
Γ ⊢ 5 ⇒ ℤ                           (Literal-Int)

Γ ⊢ "hello" ⇒ 𝕊                     (Literal-String)

x : T ∈ Γ
─────────────                        (Var)
Γ ⊢ x ⇒ T

Γ ⊢ f ⇒ (T₁,...,Tₙ) → R
Γ ⊢ e₁ ⇐ T₁  ...  Γ ⊢ eₙ ⇐ Tₙ
────────────────────────────         (App)
Γ ⊢ f(e₁,...,eₙ) ⇒ R

Γ ⊢ e ⇒ T
Γ, x₁:T₁,...,xₙ:Tₙ = match(p, T)
Γ, x₁:T₁,...,xₙ:Tₙ ⊢ body ⇒ R
──────────────────────────────       (Match-Arm)
Γ ⊢ ≡e{p→body|...} ⇒ R
```

### Checking Rules

```
Γ ⊢ e ⇒ T    T = T'
────────────────────                 (Check-Synth)
Γ ⊢ e ⇐ T'

λ(x₁:T₁,...,xₙ:Tₙ)→R annotation
Γ, x₁:T₁,...,xₙ:Tₙ ⊢ body ⇐ R
────────────────────────────         (Lambda)
Γ ⊢ λ(x₁:T₁,...,xₙ:Tₙ)→R=body ⇐ (T₁,...,Tₙ)→R
```

## Implementation

### Current Phase: Monomorphic Types

**Phase 1** (Current): All basic types without polymorphism
- Primitive types: `ℤ` (Int), `𝕊` (String), `𝔹` (Bool), `𝕌` (Unit)
- Function types: `λ(T₁,...,Tₙ)→R`
- List types: `[T]`
- Tuple types: `(T₁,T₂,...,Tₙ)`
- Record types: `{field₁:T₁, field₂:T₂, ...}`
- No generics (each function is monomorphic)

**Type equality** is structural:
```typescript
function typesEqual(t1: Type, t2: Type): boolean {
  // ℤ = ℤ, 𝕊 = 𝕊, etc.
  // (A→B) = (C→D) if A=C and B=D
  // [T] = [U] if T = U
  // etc.
}
```

### Future Phase: Polymorphism

**Phase 2** (Future): Add parametric polymorphism if needed
- Reintroduce unification for generics
- Support `∀T.` quantifiers
- Example: `λmap[T,U](fn:λ(T)→U, list:[T])→[U]`
- Still simpler than full HM because checking mode reduces inference burden

### Future Phase: Advanced Features

**Phase 3+** (Future): Extend as needed
- **Higher-rank polymorphism**: Functions taking polymorphic functions
- **Refinement types**: Types with constraints (e.g., `{n:ℤ | n > 0}`)
- **Effect tracking**: `λread()→!IO 𝕊`
- **Dependent types**: If needed for verification

All these are **easier** to add with bidirectional typing than with Hindley-Milner.

## Comparison: Bidirectional vs Hindley-Milner

| Feature | Hindley-Milner | Bidirectional |
|---------|----------------|---------------|
| **Type annotations** | Optional | Mandatory |
| **Best for** | Type inference | Type checking |
| **Error messages** | "Failed to unify X and Y" | "Expected X, got Y at line:col" |
| **Implementation** | Complex (unification, generalization) | Simpler (structural equality) |
| **Code size** | ~1,468 lines (inference + unification + patterns) | ~829 lines |
| **Extensibility** | Hard to extend | Natural framework |
| **Performance** | Good for inference | Excellent for checking |
| **Fit for Mint** | Designed for different use case | Perfect fit |

## Pattern Matching Type Checking

Pattern matching is type-checked using bidirectional rules:

```mint
λlength(list:[ℤ])→ℤ≡list{
  []→0|
  [_,.rest]→1+length(rest)
}
```

Type checking:
1. **Synthesize** scrutinee type: `list : [ℤ]`
2. **Check** each pattern against scrutinee type:
   - `[]` : `[ℤ]` ✓ (empty list pattern)
   - `[_,.rest]` : `[ℤ]` ✓ (binds `rest : [ℤ]`)
3. **Synthesize** each arm body:
   - `0` ⇒ `ℤ` ✓
   - `1+length(rest)` ⇒ `ℤ` ✓
4. **Verify** all arms have same type: `ℤ = ℤ` ✓
5. **Return** result type: `ℤ`

## List Operations

Built-in list operations are type-checked specially:

```mint
[1,2,3]↦λ(x:ℤ)→ℤ=x*2        # [ℤ] ↦ (ℤ→ℤ) ⇒ [ℤ]
[1,2,3]⊳λ(x:ℤ)→𝔹=x>1        # [ℤ] ⊳ (ℤ→𝔹) ⇒ [ℤ]
[1,2,3]⊕λ(acc:ℤ,x:ℤ)→ℤ=acc+x⊕0  # [ℤ] ⊕ (ℤ→ℤ→ℤ) ⊕ ℤ ⇒ ℤ
```

Type rules:
```
Γ ⊢ list ⇒ [T]
Γ ⊢ fn ⇐ λ(T)→U
─────────────────
Γ ⊢ list↦fn ⇒ [U]

Γ ⊢ list ⇒ [T]
Γ ⊢ pred ⇐ λ(T)→𝔹
────────────────────
Γ ⊢ list⊳pred ⇒ [T]

Γ ⊢ list ⇒ [T]
Γ ⊢ fn ⇐ λ(R,T)→R
Γ ⊢ init ⇐ R
──────────────────────
Γ ⊢ list⊕fn⊕init ⇒ R
```

## String Coercion

The `+` operator has special handling for string concatenation:

```mint
λmain()→𝕊="factorial(5) = " + factorial(5)
```

If either operand is a string, `+` becomes string concatenation with automatic coercion:
- `𝕊 + ℤ` ⇒ `𝕊` (coerce ℤ to 𝕊)
- `ℤ + 𝕊` ⇒ `𝕊` (coerce ℤ to 𝕊)
- `ℤ + ℤ` ⇒ `ℤ` (integer addition)

This is the only implicit coercion in Mint.

## Examples

### Valid Programs

```mint
# Factorial with pattern matching
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

# GCD (multi-parameter recursion allowed)
λgcd(a:ℤ,b:ℤ)→ℤ≡b{
  0→a|
  b→gcd(b,a%b)
}

# List operations
λdoubleEvens(list:[ℤ])→[ℤ]=
  list↦λ(x:ℤ)→ℤ=x*2⊳λ(x:ℤ)→𝔹=x%2=0
```

### Type Errors

```mint
# Error: Type mismatch
λbad()→ℤ="hello"
# Error: Literal type mismatch: expected ℤ, got 𝕊

# Error: Argument type mismatch
λid(x:ℤ)→ℤ=x
λmain()→𝕊=id("hello")
# Error: Argument 0 type mismatch: expected ℤ, got 𝕊

# Error: Pattern match type mismatch
λneg(b:𝔹)→𝔹≡b{5→⊥|_→⊤}
# Error: Pattern type mismatch: expected 𝔹, got ℤ
```

## Summary

Bidirectional type checking is the right choice for Mint because:

1. **Mandatory annotations** are a core principle → use a system designed for them
2. **Simpler implementation** → less code, fewer bugs, easier to maintain
3. **Better errors** → help developers understand and fix issues quickly
4. **More extensible** → natural framework for future features
5. **Perfect fit** → aligns with Mint's canonical form philosophy

Like the canonical form refinement (blocking accumulators while allowing structural parameters), this is a case of **using the right tool for the job**.
