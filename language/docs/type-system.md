# Sigil Type System: Bidirectional Type Checking

## Overview

Sigil uses **bidirectional type checking** instead of traditional Hindley-Milner type inference.

### Why Bidirectional?

Sigil's philosophy is **"ONE way to write it"**. Type annotations must be:
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

```sigil
⟦ Function declarations ⟧
λfactorial(n:ℤ)→ℤ=...

⟦ Lambda expressions ⟧
[1,2,3]↦λ(x:ℤ)→ℤ=x*2

⟦ Constants (when supported) ⟧
c pi:ℝ=3.14
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
  Location: factorial.sigil:2:16

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

### Status: ✅ Completed (2026-02-22)

The bidirectional type checker is fully implemented and integrated into the compiler pipeline.

**Location:** `compiler/src/typechecker/`
- `index.ts` - Main entry point
- `types.ts` - Type representations
- `errors.ts` - Error formatting
- `bidirectional.ts` - Core type checking algorithm

### Current Phase: Monomorphic Types

**Phase 1** (Implemented): All basic types without polymorphism
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
| **Fit for Sigil** | Designed for different use case | Perfect fit |

## Pattern Matching Type Checking

Pattern matching is type-checked using bidirectional rules:

```sigil
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

```sigil
[1,2,3]↦λ(x:ℤ)→ℤ=x*2        ⟦ [ℤ] ↦ (ℤ→ℤ) ⇒ [ℤ] ⟧
[1,2,3]⊳λ(x:ℤ)→𝔹=x>1        ⟦ [ℤ] ⊳ (ℤ→𝔹) ⇒ [ℤ] ⟧
[1,2,3]⊕λ(acc:ℤ,x:ℤ)→ℤ=acc+x⊕0  ⟦ [ℤ] ⊕ (ℤ→ℤ→ℤ) ⊕ ℤ ⇒ ℤ ⟧
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

## Sum Types (Algebraic Data Types)

Sigil supports sum types (also called tagged unions or algebraic data types) for type-safe value representation.

### Syntax

```sigil
⟦ Simple enum (no type parameters) ⟧
t Color=Red|Green|Blue

⟦ Generic Option type ⟧
t Option[T]=Some(T)|None

⟦ Generic Result type ⟧
t Result[T,E]=Ok(T)|Err(E)

⟦ Multiple fields ⟧
t Tree[T]=Leaf(T)|Branch(Tree[T],Tree[T])
```

### Type Declaration

Sum types are declared with `t TypeName=Variant1|Variant2|...`:
- Type name must be uppercase
- Variant names must be uppercase
- Variants can have zero or more fields
- Type parameters use `[T,U,...]` syntax

### Constructor Calls

Constructors are functions that create sum type values:

```sigil
⟦ Nullary constructors (no fields) - require () ⟧
λgetRed()→Color=Red()
λgetGreen()→Color=Green()

⟦ Constructors with fields ⟧
λsomeValue()→Option=Some(42)
λnoValue()→Option=None()

⟦ Multiple fields ⟧
λokResult()→Result=Ok(100)
λerrResult()→Result=Err("file not found")
```

**Important:** Even nullary constructors (like `Red`, `None`) require `()` to be called.

### Pattern Matching

Sum types are deconstructed using pattern matching:

```sigil
⟦ Match on simple enum ⟧
λcolorToInt(color:Color)→ℤ≡color{
  Red→1|
  Green→2|
  Blue→3
}

⟦ Extract values from constructors ⟧
λprocessOption(opt:Option)→ℤ≡opt{
  Some(x)→x|
  None→0
}

⟦ Nested patterns ⟧
λprocessResult(res:Result)→𝕊≡res{
  Ok(value)→"Success: "+value|
  Err(msg)→"Error: "+msg
}
```

### Type Checking Rules

Constructor pattern matching is type-checked with environment lookup:

```
Γ ⊢ scrutinee ⇒ Constructor(TypeName, [])
Constructor ∈ Γ
Γ ⊢ Constructor ⇒ (T₁,...,Tₙ) → Constructor(TypeName, [])
Γ, x₁:T₁,...,xₙ:Tₙ ⊢ body ⇒ R
────────────────────────────────────────────────
Γ ⊢ Constructor(x₁,...,xₙ)→body : R  (Constructor-Pattern)
```

The type checker:
1. Looks up constructor in environment
2. Verifies constructor returns the scrutinee type
3. Binds pattern variables to constructor parameter types
4. Type checks the arm body with extended environment

### Code Generation

Sum types compile to TypeScript/JavaScript objects with `__tag` and `__fields`:

```javascript
// t Color=Red|Green|Blue compiles to:
export function Red() {
  return { __tag: "Red", __fields: [] };
}
export function Green() {
  return { __tag: "Green", __fields: [] };
}
export function Blue() {
  return { __tag: "Blue", __fields: [] };
}

// Pattern matching compiles to:
// ≡color{Red→1|...} becomes:
switch(color.__tag) {
  case "Red": return 1;
  // ...
}
```

### Standard Library Types

The standard library provides two essential sum types:

**Option[T]** - Represents optional values:
```sigil
t Option[T]=Some(T)|None

⟦ Usage ⟧
λdivide(a:ℤ,b:ℤ)→Option≡b{
  0→None()|
  b→Some(a/b)
}
```

**Result[T,E]** - Represents success or failure:
```sigil
t Result[T,E]=Ok(T)|Err(E)

⟦ Usage ⟧
λparseInt(s:𝕊)→Result≡validInput(s){
  ⊤→Ok(parseInt(s))|
  ⊥→Err("invalid input")
}
```

### Current Limitations

**Phase 1** (Implemented):
- Sum type declarations with `t Name=V1|V2`
- Constructor function generation
- Pattern matching with type checking
- Generic type declarations (`Option[T]`, `Result[T,E]`)

**Limitations:**
- Generic type inference incomplete (type parameters use `any`)
- No generic utility functions yet (e.g., `map[T,U](opt,fn)` not supported)
- Nullary constructors require explicit `()` calls

**Future improvements:**
- Full generic type inference
- Type parameter constraints
- Generic utility functions in stdlib
- Exhaustiveness checking for pattern matches

### Examples

See `examples/sum-types-demo.sigil` for comprehensive examples including:
- Simple enums (Color)
- Generic Option and Result types
- Pattern matching techniques
- Practical use cases

## Concatenation Operators

Sigil uses distinct operators for distinct concatenation semantics:

- `++` for string concatenation (`𝕊 × 𝕊 → 𝕊`)
- `⧺` for list concatenation (`[T] × [T] → [T]`)

```sigil
λgreet(name:𝕊)→𝕊="Hello, "++name
λmerge(xs:[ℤ],ys:[ℤ])→[ℤ]=xs⧺ys
```

This preserves canonical surface forms by avoiding one overloaded concat operator for different data kinds.

## Empty List Contextual Typing

The empty list literal `[]` requires type context to determine its element type.

**Works in these contexts:**
- **Function return type**: `λf()→[ℤ]=[]` provides `[ℤ]` context
- **Pattern matching arms**: First arm establishes type for subsequent arms
- **Explicit checking contexts**: Where expected type flows downward

**Example - Pattern Matching:**
```sigil
⟦ Basic: empty list infers from function return type ⟧
λemptyInts()→[ℤ]=[]

⟦ Pattern matching: first arm pattern infers from scrutinee, body from return type ⟧
λreverse(xs:[ℤ])→[ℤ]≡xs{
  []→[]|                 ⟦ OK: expected type is [ℤ] from function signature ⟧
  [x,.rest]→reverse(rest)⧺[x]
}

⟦ Pattern matching: subsequent arms checked against first arm's type ⟧
λfirstNonEmpty(a:[ℤ],b:[ℤ])→[ℤ]≡a{
  [x,.xs] → a|      ⟦ First arm synthesizes to [ℤ] ⟧
  [] → b            ⟦ Second arm checked against [ℤ] from first arm ⟧
}

⟦ Multiple empty arms work when return type provides context ⟧
t Foo=A|B|C

λtest(x:Foo)→[ℤ]≡x{
  A → [1,2,3]|      ⟦ First arm synthesizes to [ℤ] ⟧
  B → []|           ⟦ Checked against [ℤ] ⟧
  C → []            ⟦ Checked against [ℤ] ⟧
}
```

**Does NOT work when:**
- Standalone expression with no context: `c x=[]` (no type known)
- All pattern arms are empty and no function return type
- Nested expressions in synthesis mode without surrounding context

## Examples

### Valid Programs

```sigil
⟦ Factorial with pattern matching ⟧
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

⟦ GCD (multi-parameter recursion allowed) ⟧
λgcd(a:ℤ,b:ℤ)→ℤ≡b{
  0→a|
  b→gcd(b,a%b)
}

⟦ List operations ⟧
λdoubleEvens(list:[ℤ])→[ℤ]=
  list↦λ(x:ℤ)→ℤ=x*2⊳λ(x:ℤ)→𝔹=x%2=0
```

### Type Errors

```sigil
⟦ Error: Type mismatch ⟧
λbad()→ℤ="hello"
⟦ Error: Literal type mismatch: expected ℤ, got 𝕊 ⟧

⟦ Error: Argument type mismatch ⟧
λid(x:ℤ)→ℤ=x
λmain()→𝕊=id("hello")
⟦ Error: Argument 0 type mismatch: expected ℤ, got 𝕊 ⟧

⟦ Error: Pattern match type mismatch ⟧
λneg(b:𝔹)→𝔹≡b{5→⊥|_→⊤}
⟦ Error: Pattern type mismatch: expected 𝔹, got ℤ ⟧
```

## Summary

Bidirectional type checking is the right choice for Sigil because:

1. **Mandatory annotations** are a core principle → use a system designed for them
2. **Simpler implementation** → less code, fewer bugs, easier to maintain
3. **Better errors** → help developers understand and fix issues quickly
4. **More extensible** → natural framework for future features
5. **Perfect fit** → aligns with Sigil's canonical form philosophy

Like the canonical form refinement (blocking accumulators while allowing structural parameters), this is a case of **using the right tool for the job**.
