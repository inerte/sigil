# Sigil Type System Specification

Version: 1.0.0
Last Updated: 2026-03-02

## Overview

Sigil uses a **bidirectional type checking system** with:
- Algebraic data types (sum types + product types)
- Effect tracking
- Borrow checking (ownership and lifetimes)
- mandatory explicit type annotations on function signatures

**Design Philosophy**: Types are mandatory and explicit in canonical positions. Bidirectional checking provides precise errors while preserving deterministic syntax.

## Type Checking

### Bidirectional Checking

Sigil alternates between:

1. **Synthesis (⇒)**: infer a type from expression structure
2. **Checking (⇐)**: verify an expression against an expected type

Sigil does not rely on Hindley-Milner/Algorithm W as its primary typing strategy.

### Inference Rules

#### Variables

```
Γ ⊢ x : σ    if (x : σ) ∈ Γ
```

If variable `x` has type scheme `σ` in environment `Γ`, then `x` has type `σ`.

#### Literals

```
Γ ⊢ n : ℤ           (integer literals)
Γ ⊢ f : ℝ           (float literals)
Γ ⊢ true : 𝔹           (true)
Γ ⊢ false : 𝔹           (false)
Γ ⊢ "s" : 𝕊         (string literals)
Γ ⊢ 'c' : ℂ         (char literals)
Γ ⊢ () : 𝕌          (unit)
```

#### Function Application

```
Γ ⊢ e₁ : τ₁ → τ₂    Γ ⊢ e₂ : τ₁
─────────────────────────────────
Γ ⊢ e₁(e₂) : τ₂
```

#### Lambda Abstraction

```
Γ, x : τ₁ ⊢ e : τ₂
──────────────────────────
Γ ⊢ λx→e : τ₁ → τ₂
```

#### Let Binding (Monomorphic)

```
Γ ⊢ e₁ : τ₁    Γ, x : τ₁ ⊢ e₂ : τ₂
────────────────────────────────
Γ ⊢ l x=e₁;e₂ : τ₂
```

Local `l` bindings do **not** undergo Hindley-Milner let-generalization.
Only explicitly generic top-level declarations introduce quantified type schemes.

Where `σ` is the generalization of the type of `e₁`.

#### Pattern Matching

```
Γ ⊢ e : τ    Γ ⊢ p₁ : τ    Γ, bindings(p₁) ⊢ e₁ : τ'
      Γ ⊢ p₂ : τ    Γ, bindings(p₂) ⊢ e₂ : τ'
──────────────────────────────────────────────────────
Γ ⊢ match e{p₁→e₁|p₂→e₂} : τ'
```

All match arms must have the same result type `τ'`.

**Bidirectional Typing for Match Expressions:**

In synthesis mode (⇒), the first arm establishes the type that subsequent arms are checked against:

```
Γ ⊢ e ⇒ τ_scrutinee
Γ, Δ₁ ⊢ p₁ ⇐ τ_scrutinee ⇝ Δ₁
Γ, Δ₁ ⊢ e₁ ⇒ τ
Γ, Δᵢ ⊢ pᵢ ⇐ τ_scrutinee ⇝ Δᵢ  (for i > 1)
Γ, Δᵢ ⊢ eᵢ ⇐ τ                 (for i > 1)
─────────────────────────────────────────
Γ ⊢ (match e { p₁ → e₁ | ... | pₙ → eₙ }) ⇒ τ
```

Note: The first arm body is synthesized (⇒) to establish expected type τ. Remaining arm bodies are checked (⇐) against that type. This allows empty list `[]` in later arms when the first arm provides context.

**Bidirectional Typing for Record Expressions:**

In checking mode (⇐), record fields are checked against the expected field types:

```
Γ ⊢ τ ⇒ {f₁:τ₁, ..., fₙ:τₙ}
Γ ⊢ e₁ ⇐ τ₁
...
Γ ⊢ eₙ ⇐ τₙ
─────────────────────────────────────────
Γ ⊢ {f₁=e₁, ..., fₙ=eₙ} ⇐ {f₁:τ₁, ..., fₙ:τₙ}
```

Note: Each field value eᵢ is checked (⇐) against the expected field type τᵢ from the record type. This allows empty list `[]` in record fields when the record type specifies list types for those fields.

### Canonical Binding Rule

Sigil canonical validation forbids local shadowing.

If a binding name is already present in the active lexical environment, a nested:
- function parameter
- lambda parameter
- `l` binding
- pattern binding

must use a fresh name instead of rebinding the existing one.

### Type Schemes

Type schemes represent polymorphic types:

```
σ ::= ∀α₁...αₙ.τ
```

Example:
```sigil
λidentity(x)=x
```
Infers type: `∀T.λ(T)→T`

### Generalization

When binding variables with `l`, generalize free type variables:

```sigil
l id=λx→x;
l result=id(5);
l result2=id("hello");
```

Type of `id` is generalized to `∀T.λ(T)→T`, allowing it to be used with different types.

### Instantiation

When using polymorphic values, instantiate with fresh type variables:

```sigil
l id=λx→x;
id(5)        (* T instantiated to ℤ *)
id("hi")     (* T instantiated to 𝕊 *)
```

## Primitive Types

### Built-in Types

| Symbol | Name | Description | Values |
|--------|------|-------------|--------|
| `ℤ` | Integer | Whole numbers | `-2147483648` to `2147483647` |
| `ℝ` | Float | Floating point | IEEE 754 double precision |
| `𝔹` | Boolean | Truth values | `true` (true), `false` (false) |
| `𝕊` | String | UTF-8 strings | `"hello"` |
| `ℂ` | Character | Unicode character | `'a'` |
| `𝕌` | Unit | Empty type | `()` |
| `∅` | Never | Uninhabited type | No values (for diverging functions) |

### Type Constructors

```sigil
[T]          (* List of T *)
{K↦V}        (* Map from K to V *)
(T₁,T₂,...) (* Tuple *)
```

## Algebraic Data Types

### Sum Types (Tagged Unions)

```sigil
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
t Color=Red|Green|Blue
```

**Type Rules**:
- Each variant is a constructor
- Variants can carry data or be nullary
- Pattern matching must be exhaustive

**Constructors**:
```
Some : ∀T.T → Option[T]
None : ∀T.Option[T]
Ok : ∀T,E.T → Result[T,E]
Err : ∀T,E.E → Result[T,E]
```

Imported constructors are referenced with fully qualified module syntax:

```sigil
i src⋅graphTypes

src⋅graphTypes.Ordering([1,2,3])
```

Imported constructor patterns use the same qualification:

```sigil
match result{
  src⋅graphTypes.Ordering(order)→order|
  src⋅graphTypes.CycleDetected()→[]
}
```

### Product Types (Records)

```sigil
t User={email:𝕊,id:ℤ,name:𝕊}
t Point={x:ℝ,y:ℝ}
```

**Type Rules**:
- Fields have names and types
- Field access: `user.name` has type `𝕊` if `user : User`
- Record literals must include all fields
- Canonical form requires record fields to be alphabetically ordered in declarations, literals, and patterns
- Named product types participate in structural equality after normalization

### Type Aliases

```sigil
t UserId=ℤ
t Email=𝕊
```

Type aliases create synonyms, not new types (structural typing).

### Canonical Semantic Equality

Sigil compares aliases and named product types by their normalized canonical form
whenever compatibility/equality is checked.

Informally:

```
normalize(UserId) = ℤ            if t UserId=ℤ
normalize(Todo) = {done:𝔹,id:ℤ,text:𝕊}
```

Compatibility is then checked on normalized forms:

```
compatible(τ1, τ2) iff types_equal(normalize(τ1), normalize(τ2))
```

This applies to:
- constant annotations
- function arguments and returns
- list append and higher-order list operators
- branch compatibility
- structural equality-sensitive checks generally

This is not Hindley-Milner-style inference. Types are still explicit in canonical
positions; normalization only resolves the canonical meaning of named structural types.

Sum types remain nominal and are not normalized into structural payload shapes.

## Function Types

### Function Signatures

```sigil
λ(T₁,T₂,...,Tₙ) → R
```

Example:
```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
```

Type: `λ(ℤ,ℤ)→ℤ`

### Higher-Order Functions

Functions can take functions as arguments:

```sigil
λmap[T,U](fn:λ(T)→U,list:[T])→[U]=...
```

Type: `∀T,U.λ(λ(T)→U,[T])→[U]`

### Currying

Functions are **not curried** by default (unlike Haskell). Multi-argument functions take tuples implicitly:

```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
```

Type is `λ(ℤ,ℤ)→ℤ`, **not** `λ(ℤ)→λ(ℤ)→ℤ`.

To create curried functions explicitly:

```sigil
λadd(x:ℤ)→λ(ℤ)→ℤ=λy→x+y
```

## Pattern Matching Type Rules

### Exhaustiveness Checking

Pattern matches must cover all possible values:

```sigil
(* OK - exhaustive *)
λsign(n:ℤ)→𝕊 match n{
  0→"zero"|
  n→match n>0{true→"positive"|false→"negative"}
}

(* ERROR - not exhaustive, missing None case *)
λunwrap[T](opt:Option[T])→T match opt{
  Some(v)→v
}
```

### Pattern Type Inference

Patterns introduce bindings with inferred types:

```sigil
match option{
  Some(x)→x+1|    (* x : T where option : Option[T] *)
  None→0           (* return type must match: ℤ *)
}
```

### Type Constraints from Patterns

```sigil
λlength[T](list:[T])→ℤ match list{
  []→0|
  [x,.xs]→1+length(xs)
}
```

- `[]` pattern constrains `list` to be `[T]` type
- `x` has type `T`
- `xs` has type `[T]`

## Effect System

### Effect Annotations

Functions can declare effects:

```sigil
λread_file(path:𝕊)→Result[𝕊,IoError]!IO
λfetch_url(url:𝕊)→Result[𝕊,HttpError]!Network
```

**Syntax**: `!EffectName`

### Pure Functions

Functions without effect annotations are pure:

```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y    (* Pure, no effects *)
```

### Effect Inference

Effects are inferred from function calls:

```sigil
λprocess_file(path:𝕊)→Result[ℤ,Error]!IO=
  l content=read_file(path)?;    (* read_file has !IO *)
  l lines=split(content,"\n");   (* split is pure *)
  Ok(count_lines(lines))
```

Result: `process_file` has effect `!IO` (propagated from `read_file`).

### Effect Polymorphism

```sigil
λmap[T,U,E](fn:λ(T)→U!E,list:[T])→[U]!E=...
```

`map` propagates whatever effects `fn` has.

## Ownership and Borrowing

### Ownership Rules

1. **Each value has one owner**
2. **Owner can transfer ownership** (move)
3. **When owner goes out of scope, value is dropped**

### Move Semantics

By default, passing values moves ownership:

```sigil
l x=[1,2,3];
l y=x;        (* x moved to y *)
print(x);     (* ERROR: x was moved *)
```

### Borrowing

Use `&` for immutable borrows, `&mut` for mutable borrows:

```sigil
λlength[T](list:&[T])→ℤ=...    (* Borrows list, doesn't take ownership *)

l x=[1,2,3];
l len=length(&x);               (* Borrow x *)
print(x);                       (* OK, x still owned here *)
```

### Borrow Checker Rules

1. **Multiple immutable borrows allowed**
2. **Only one mutable borrow allowed**
3. **Cannot have immutable and mutable borrows simultaneously**
4. **Borrows must not outlive the owner**

Example:

```sigil
l mut x=5;
l y=&x;        (* Immutable borrow *)
l z=&x;        (* OK - multiple immutable borrows *)
l w=&mut x;    (* ERROR - cannot have &mut while & exists *)
```

### Lifetimes

Lifetimes are inferred automatically (no explicit annotation in Sigil v1.0):

```sigil
λfirst[T](list:&[T])→Option[&T]=match list{
  [x,..]→Some(&x)|
  []→None
}
```

The lifetime of the returned reference is tied to the input `list`.

## Type Checking Algorithm

### Algorithm W (Simplified)

```
W(Γ, e) = (S, τ)
where S is a substitution and τ is a type

W(Γ, x) = (∅, inst(Γ(x)))
W(Γ, n) = (∅, ℤ)
W(Γ, λx→e) =
  let α = fresh type variable
      (S, τ) = W(Γ[x:α], e)
  in (S, S(α) → τ)
W(Γ, e₁(e₂)) =
  let (S₁, τ₁) = W(Γ, e₁)
      (S₂, τ₂) = W(S₁(Γ), e₂)
      α = fresh type variable
      S₃ = unify(S₂(τ₁), τ₂ → α)
  in (S₃ ∘ S₂ ∘ S₁, S₃(α))
```

### Unification

Unification finds a substitution `S` such that `S(τ₁) = S(τ₂)`:

```
unify(ℤ, ℤ) = ∅
unify(α, τ) = [α ↦ τ]  if α ∉ FV(τ)  (occurs check)
unify(τ, α) = [α ↦ τ]  if α ∉ FV(τ)
unify(τ₁→τ₂, τ₃→τ₄) =
  let S₁ = unify(τ₁, τ₃)
      S₂ = unify(S₁(τ₂), S₁(τ₄))
  in S₂ ∘ S₁
```

### Type Errors

Common type errors:

1. **Type mismatch**:
   ```sigil
   λadd(x:ℤ,y:ℤ)→ℤ=x+y
   add(5,"hello")    (* ERROR: expected ℤ, got 𝕊 *)
   ```

2. **Non-exhaustive pattern match**:
   ```sigil
   λunwrap[T](opt:Option[T])→T match opt{
     Some(v)→v       (* ERROR: missing None case *)
   }
   ```

3. **Occurs check failure** (infinite types):
   ```sigil
   λloop(x)=loop(x)  (* ERROR: x : α = α → β, infinite type *)
   ```

4. **Effect mismatch**:
   ```sigil
   λpure_fn(x:ℤ)→ℤ=read_file("data.txt")  (* ERROR: !IO effect in pure function *)
   ```

## Type Examples

### Example 1: List Map

```sigil
λmap[T,U](fn:λ(T)→U,list:[T])→[U] match list{
  []→[]|
  [x,.xs]→[fn(x),.map(fn,xs)]
}
```

Type inference:
1. `fn` has type `λ(T)→U` (given)
2. `list` has type `[T]` (given)
3. Result type is `[U]` (given)
4. Pattern `[]` implies `list : [T]` ✓
5. Pattern `[x,.xs]` implies `x : T` and `xs : [T]` ✓
6. `fn(x)` has type `U` ✓
7. `map(fn,xs)` has type `[U]` (recursive call) ✓
8. `[fn(x),.map(fn,xs)]` has type `[U]` ✓

### Example 2: Option Binding

```sigil
λbind[T,U](opt:Option[T],fn:λ(T)→Option[U])→Option[U] match opt{
  Some(v)→fn(v)|
  None→None
}
```

Type inference:
1. `opt : Option[T]`, `fn : λ(T)→Option[U]`
2. `Some(v)` implies `v : T`
3. `fn(v)` has type `Option[U]` ✓
4. `None` has type `Option[U]` (polymorphic instantiation) ✓

### Example 3: Fibonacci with Memoization

```sigil
t Memo={cache:{ℤ:ℤ}}

λfib_memo(n:ℤ,memo:&mut Memo)→ℤ=
  match memo.cache.get(n){
    Some(result)→result|
    None→
      l result=match n{
        0→0|
        1→1|
        n→fib_memo(n-1,memo)+fib_memo(n-2,memo)
      };
      memo.cache.insert(n,result);
      result
  }
```

Types:
- `memo : &mut Memo` (mutable borrow)
- `memo.cache : {ℤ:ℤ}`
- `memo.cache.get(n) : Option[ℤ]`
- `result : ℤ`

## Type System Extensions (Future)

### Higher-Kinded Types

```sigil
t Functor[F[_]]={
  map:∀T,U.λ(λ(T)→U,F[T])→F[U]
}
```

### Dependent Types

```sigil
t Vec[T,n:ℤ]=[T]  (* Vector of length n *)

λhead[T,n:ℤ](v:Vec[T,n])→T where n>0=...
```

### Row Polymorphism

```sigil
t User={id:ℤ,name:𝕊,..r}  (* User with at least id and name; row tail follows fixed fields *)
```

## References

1. Damas, L., & Milner, R. (1982). "Principal type-schemes for functional programs"
2. Pierce, B. C. (2002). "Types and Programming Languages"
3. Harper, R. (2016). "Practical Foundations for Programming Languages"
4. Jung, R., et al. (2017). "RustBelt: Securing the Foundations of the Rust Programming Language"

---

**Next**: See `semantics.md` for operational semantics and evaluation rules.
