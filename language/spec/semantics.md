# Sigil Operational Semantics

Version: 1.0.0
Last Updated: 2026-02-21

## Overview

This document defines the **operational semantics** of Sigil - how programs execute and evaluate. Sigil uses **immutable data** and a **concurrent-by-default, demand-driven** execution model.

## Evaluation Strategy

### Demand-Driven Execution

Sigil starts independent work early and joins results only when a strict construct needs a concrete value:

```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y

add(2+3,4+5)
// The arithmetic subexpressions may be started independently
// The call result is joined only when a strict consumer needs it
```

**Rationale**: one runtime model, better overlap for async work, and no sync/async surface split.

### Effect Initiation Order

Effectful sibling expressions are initiated **left-to-right**:

```sigil
f(g(x),h(y))
// If g and h are pure, the implementation may overlap them.
// If g and h are effectful, g starts first, then h starts.
// The call to f joins the values only when it needs them.
```

## Values

### What is a Value?

A **value** is an expression that cannot be reduced further:

```
v ::= n                  (* integer literal *)
    | f                  (* float literal *)
    | true | false              (* boolean literals *)
    | "s"                (* string literal *)
    | 'c'                (* char literal *)
    | ()                 (* unit *)
    | [v₁,v₂,...,vₙ]     (* list of values *)
    | {f₁:v₁,f₂:v₂,...}  (* record of values *)
    | C(v₁,v₂,...,vₙ)    (* constructor application *)
    | λx→e               (* lambda abstraction *)
```

**Non-values** (expressions that can be reduced):

```sigil
2+3              (* Can reduce to 5 *)
fibonacci(10)    (* Can reduce to 55 *)
[1,2+3,4]        (* Can reduce to [1,5,4] *)
```

List literals preserve element boundaries exactly.
If `v` is itself a list value, then `[v]` is a one-element outer list whose single element is `v`.
Concatenation is expressed separately with `⧺`.

## Small-Step Operational Semantics

### Notation

```
e → e'           (* Expression e reduces to e' in one step *)
e →* e'          (* Expression e reduces to e' in zero or more steps *)
e ⇓ v            (* Expression e evaluates to value v *)
```

### Reduction Rules

#### Arithmetic Operations

```
e₁ → e₁'
─────────────────
e₁ + e₂ → e₁' + e₂

v₁ + e₂ → v₁ + e₂'   (where e₂ → e₂')

n₁ + n₂ → n₃         (where n₃ is the sum of n₁ and n₂)
```

**Example**:
```sigil
(2+3)+(4+5)
→ 5+(4+5)           (* Reduce left operand *)
→ 5+9               (* Reduce right operand *)
→ 14                (* Compute sum *)
```

#### Function Application

```
e₁ → e₁'
─────────────────────
e₁(e₂) → e₁'(e₂)

(λx→e)(v) → e[x:=v]  (* Substitute v for x in e *)
```

**Example**:
```sigil
(λx→x+1)(2+3)
→ (λx→x+1)(5)       (* Evaluate argument *)
→ 5+1               (* Substitute 5 for x *)
→ 6                 (* Compute *)
```

#### Let Binding

```
e₁ → e₁'
────────────────────
l x=e₁;e₂ → l x=e₁';e₂

l x=v;e₂ → e₂[x:=v]
```

**Example**:
```sigil
l x=2+3;x*2
→ l x=5;x*2         (* Evaluate binding *)
→ 5*2               (* Substitute *)
→ 10                (* Compute *)
```

#### Pattern Matching

```
e → e'
────────────────────────────────
match e{p₁→e₁|...|pₙ→eₙ} → match e'{p₁→e₁|...|pₙ→eₙ}

match v{p₁→e₁|...|pₙ→eₙ} → eᵢ[bindings(pᵢ,v)]
  (where pᵢ is the first pattern that matches v)
```

**Example**:
```sigil
match 2+3{0→"zero"|5→"five"|_→"other"}
→ match 5{0→"zero"|5→"five"|_→"other"}    (* Evaluate scrutinee *)
→ "five"                              (* Match second pattern *)
```

#### List Operations

```
// List concatenation
[] ++ ys → ys
[x,.xs] ++ ys → [x,.xs++ys]

// List pattern matching
match []{[]→e₁|[x,.xs]→e₂} → e₁
match [v,.vs]{[]→e₁|[x,.xs]→e₂} → e₂[x:=v,xs:=vs]
```

In particular:

```
[[1,2]] ≠ [1,2]
[xs] does not reduce via concatenation
```

**Example**:
```sigil
[1,2]++[3,4]
→ [1,2++[3,4]]
→ [1,[2]++[3,4]]
→ [1,[2,3,4]]
→ [1,2,3,4]
```

#### Record Access

```
e → e'
─────────────
e.f → e'.f

{f₁:v₁,...,fᵢ:vᵢ,...,fₙ:vₙ}.fᵢ → vᵢ
```

**Example**:
```sigil
{id:1,name:"Alice"}.name → "Alice"
```

#### Pipeline Operator

```
e₁ → e₁'
────────────────
e₁|>e₂ → e₁'|>e₂

v|>f → f(v)
```

**Example**:
```sigil
5|>λx→x*2|>λx→x+1
→ (λx→x*2)(5)|>λx→x+1
→ 10|>λx→x+1
→ (λx→x+1)(10)
→ 11
```

## Big-Step Operational Semantics

### Notation

```
Γ ⊢ e ⇓ v           (* In environment Γ, expression e evaluates to value v *)
```

### Rules

#### Variables

```
───────────────
Γ ⊢ x ⇓ Γ(x)
```

Lookup variable `x` in environment `Γ`.

#### Literals

```
─────────────
Γ ⊢ n ⇓ n

─────────────
Γ ⊢ true ⇓ true

─────────────
Γ ⊢ "s" ⇓ "s"
```

Literals evaluate to themselves.

#### Lambda Abstraction

```
─────────────────────
Γ ⊢ λx→e ⇓ λx→e
```

Lambdas are values (closures would capture Γ in implementation).

#### Function Application

```
Γ ⊢ e₁ ⇓ λx→e    Γ ⊢ e₂ ⇓ v₂    Γ[x:=v₂] ⊢ e ⇓ v
───────────────────────────────────────────────────
Γ ⊢ e₁(e₂) ⇓ v
```

#### Let Binding

```
Γ ⊢ e₁ ⇓ v₁    Γ[x:=v₁] ⊢ e₂ ⇓ v₂
──────────────────────────────────
Γ ⊢ l x=e₁;e₂ ⇓ v₂
```

#### Pattern Matching

```
Γ ⊢ e ⇓ v    match(pᵢ,v) = θ    Γ ∪ θ ⊢ eᵢ ⇓ v'
─────────────────────────────────────────────────
Γ ⊢ match e{p₁→e₁|...|pₙ→eₙ} ⇓ v'
```

Where `match(p,v)` returns bindings if pattern `p` matches value `v`.

#### Binary Operations

```
Γ ⊢ e₁ ⇓ n₁    Γ ⊢ e₂ ⇓ n₂    n₃ = n₁ ⊕ n₂
────────────────────────────────────────────
Γ ⊢ e₁ ⊕ e₂ ⇓ n₃
```

Where `⊕` is any binary operator (+, -, *, /, etc.).

## Pattern Matching Semantics

### Pattern Matching Algorithm

```
match(p, v) → θ or fail
```

Returns bindings `θ` if pattern `p` matches value `v`, otherwise fails.

### Rules

```
match(x, v) = [x ↦ v]                    (* Variable pattern *)
match(_, v) = ∅                          (* Wildcard *)
match(n, n) = ∅                          (* Literal match *)
match(n, m) = fail  (if n ≠ m)          (* Literal mismatch *)
match(C(p₁,...,pₙ), C(v₁,...,vₙ)) =     (* Constructor *)
  match(p₁,v₁) ∪ ... ∪ match(pₙ,vₙ)
match(C₁(...), C₂(...)) = fail          (* Constructor mismatch *)
  (if C₁ ≠ C₂)
match([p₁,...,pₙ], [v₁,...,vₙ]) =      (* List *)
  match(p₁,v₁) ∪ ... ∪ match(pₙ,vₙ)
match([p,.ps], [v,.vs]) =               (* List cons *)
  match(p,v) ∪ match(ps,vs)
match({f₁:p₁,...}, {f₁:v₁,...}) =       (* Record *)
  match(p₁,v₁) ∪ ...
```

### Example

```sigil
match(Some(x), Some(5))
= match(x, 5)
= [x ↦ 5]

match([x,.xs], [1,2,3])
= match(x, 1) ∪ match(xs, [2,3])
= [x ↦ 1, xs ↦ [2,3]]
```

## Effect Semantics

### Pure Functions

Pure functions have **no observable effects**:

```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y

// Always returns same output for same input
// No side effects
// Can be memoized safely
// Order of evaluation doesn't matter (for independent calls)
```

### Effectful Functions

Functions with effects (`!IO`, `!Network`, etc.) have observable behavior:

```sigil
λread_file(path:𝕊)→Result[𝕊,IoError]!IO

// Different results possible for same input (file may change)
// Side effects observable (reads from disk)
// Cannot be memoized (safely)
// Order of evaluation matters
```

### Effect Ordering

Effects are initiated in **evaluation order** (left-to-right):

```sigil
l content1=read_file("a.txt");   (* Executes first *)
l content2=read_file("b.txt");   (* Executes second *)
print(content1++content2)        (* Executes third *)
```

Sigil preserves source-order effect initiation, but effect resolution may overlap when the backend supports it.

### Effect Isolation

Effects cannot escape pure contexts:

```sigil
λpure_fn(x:ℤ)→ℤ=
  read_file("data.txt")  (* ERROR: !IO effect in pure function *)

λio_fn(x:ℤ)→ℤ!IO=
  read_file("data.txt")  (* OK: function is marked !IO *)
```

## Memory Semantics

### Immutability by Default

All values are **immutable** unless explicitly marked `mut`:

```sigil
l x=5
l y=x      (* y is a copy *)
l x=10     (* ERROR: cannot reassign immutable binding *)
```

### Mutable Bindings

Use `mut` for mutable bindings:

```sigil
l mut x=5
l x=10     (* OK: x is mutable *)
l x=x+1    (* OK: x is now 11 *)
```

### Ownership and Borrowing

#### Move Semantics

By default, values are **moved** (ownership transferred):

```sigil
l x=[1,2,3]
l y=x         (* x moved to y *)
print(x)      (* ERROR: x was moved *)
```

#### Borrowing

Use `&` to **borrow** without taking ownership:

```sigil
λlength[T](list:&[T])→ℤ=...

l x=[1,2,3]
l len=length(&x)  (* Borrow x *)
print(x)          (* OK: x still owned here *)
```

#### Mutable Borrowing

Use `&mut` for mutable borrows:

```sigil
λappend[T](list:&mut [T],item:T)→𝕌=...

l mut xs=[1,2,3]
append(&mut xs,4)  (* Mutable borrow *)
print(xs)          (* [1,2,3,4] *)
```

#### Borrow Rules (Enforced by Borrow Checker)

1. **Multiple immutable borrows allowed**:
   ```sigil
   l x=[1,2,3]
   l len1=length(&x)
   l len2=length(&x)  (* OK: multiple & allowed *)
   ```

2. **Only one mutable borrow allowed**:
   ```sigil
   l mut x=[1,2,3]
   append(&mut x,4)
   append(&mut x,5)   (* ERROR: cannot have two &mut simultaneously *)
   ```

3. **Cannot mix immutable and mutable borrows**:
   ```sigil
   l mut x=[1,2,3]
   l y=&x
   append(&mut x,4)   (* ERROR: cannot &mut while & exists *)
   ```

4. **Borrows must not outlive owner**:
   ```sigil
   l y=
     l x=[1,2,3]
     &x            (* ERROR: x dropped, borrow would dangle *)
   ```

## Type Erasure at Runtime

Types are **erased** after compilation. At runtime, only values exist:

```sigil
// Compile time:
λidentity[T](x:T)→T=x

// Runtime (JavaScript):
function identity(x) { return x; }
```

**Rationale**: Smaller runtime, faster execution, types are for compile-time safety only.

## Evaluation Examples

### Example 1: Fibonacci

```sigil
λfibonacci(n:ℤ)→ℤ match n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}

// Evaluate fibonacci(3):
fibonacci(3)
→ match 3{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
→ fibonacci(3-1)+fibonacci(3-2)
→ fibonacci(2)+fibonacci(1)
→ (match 2{...} → fibonacci(1)+fibonacci(0)) + fibonacci(1)
→ ((match 1{...} → 1) + (match 0{...} → 0)) + (match 1{...} → 1)
→ (1 + 0) + 1
→ 1 + 1
→ 2
```

### Example 2: List Map

```sigil
λmap[T,U](fn:λ(T)→U,list:[T])→[U] match list{
  []→[]|
  [x,.xs]→[fn(x),.map(fn,xs)]
}

// Evaluate map(λn→n*2,[1,2,3]):
map(λn→n*2,[1,2,3])
→ match [1,2,3]{[]→[]|[x,.xs]→[fn(x),.map(fn,xs)]}
→ [(λn→n*2)(1),.map(λn→n*2,[2,3])]
→ [2,.map(λn→n*2,[2,3])]
→ [2,[(λn→n*2)(2),.map(λn→n*2,[3])]]
→ [2,[4,.map(λn→n*2,[3])]]
→ [2,[4,[(λn→n*2)(3),.map(λn→n*2,[])]]]
→ [2,[4,[6,.map(λn→n*2,[])]]]
→ [2,[4,[6,[]]]]
→ [2,4,6]
```

### Example 3: Pipeline

```sigil
[1,2,3]|>map(λx→x*2)|>filter(λx→x>2)|>reduce(0,λa,b→a+b)

// Evaluate:
[1,2,3]|>map(λx→x*2)
→ map(λx→x*2,[1,2,3])
→ [2,4,6]

[2,4,6]|>filter(λx→x>2)
→ filter(λx→x>2,[2,4,6])
→ [4,6]

[4,6]|>reduce(0,λa,b→a+b)
→ reduce(0,λa,b→a+b,[4,6])
→ (λa,b→a+b)(0,4) ... eventually
→ 10
```

## Denotational Semantics (Informal)

### Semantic Domains

```
⟦ℤ⟧ = ℤ                                 (* Mathematical integers *)
⟦ℝ⟧ = ℝ                                 (* Mathematical reals *)
⟦𝔹⟧ = {true, false}                    (* Booleans *)
⟦𝕊⟧ = String                           (* Strings *)
⟦[T]⟧ = ⟦T⟧*                            (* Lists are sequences *)
⟦T₁→T₂⟧ = ⟦T₁⟧ → ⟦T₂⟧                  (* Functions *)
⟦{f₁:T₁,...,fₙ:Tₙ}⟧ = ⟦T₁⟧ × ... × ⟦Tₙ⟧  (* Records are tuples *)
```

### Expression Semantics

```
⟦n⟧ρ = n
⟦x⟧ρ = ρ(x)
⟦λx→e⟧ρ = λv.⟦e⟧ρ[x:=v]
⟦e₁(e₂)⟧ρ = (⟦e₁⟧ρ)(⟦e₂⟧ρ)
⟦e₁+e₂⟧ρ = ⟦e₁⟧ρ + ⟦e₂⟧ρ
⟦l x=e₁;e₂⟧ρ = ⟦e₂⟧ρ[x:=⟦e₁⟧ρ]
```

Where `ρ` is the environment mapping variables to values.

## Compilation to JavaScript

### Type Erasure

```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
```

Compiles to:

```javascript
function add(x, y) {
  return x + y;
}
```

### Pattern Matching

```sigil
match option{Some(v)→v|None→0}
```

Compiles to:

```javascript
(function(option) {
  if (option.tag === 'Some') {
    return option.value;
  } else if (option.tag === 'None') {
    return 0;
  } else {
    throw new Error('Non-exhaustive pattern match');
  }
})(option)
```

### Pipeline Operator

```sigil
x|>f|>g
```

Compiles to:

```javascript
g(f(x))
```

## Formal Properties

### Determinism

**Theorem**: Sigil programs are **deterministic** - same input always produces same output (for pure functions).

**Proof sketch**:
- Evaluation order is fixed (left-to-right, innermost-first)
- No non-deterministic constructs
- Effects are explicitly tracked

### Type Safety

**Theorem**: Well-typed programs **do not go wrong**.

**Progress**: If `⊢ e : τ`, then either `e` is a value or `e → e'` for some `e'`.

**Preservation**: If `⊢ e : τ` and `e → e'`, then `⊢ e' : τ`.

### Termination

**Non-theorem**: Sigil programs are **not guaranteed to terminate**.

Counter-example:
```sigil
λloop()→𝕌=loop()
```

This is intentional - Turing-complete languages cannot guarantee termination.

## References

1. Pierce, B. C. (2002). "Types and Programming Languages" - Operational semantics
2. Plotkin, G. D. (1981). "A Structural Approach to Operational Semantics"
3. Wright, A. K., & Felleisen, M. (1994). "A Syntactic Approach to Type Soundness"
4. Harper, R. (2016). "Practical Foundations for Programming Languages"

---

**Next**: See `compiler/` for implementation of these semantics.
