# Sigil Type System Specification

Version: 1.0.0
Last Updated: 2026-03-07

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
Γ ⊢ n : Int           (integer literals)
Γ ⊢ f : Float           (float literals)
Γ ⊢ true : Bool           (true)
Γ ⊢ false : Bool           (false)
Γ ⊢ "s" : String         (string literals)
Γ ⊢ 'c' : Char         (char literals)
Γ ⊢ () : Unit          (unit)
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

Sigil canonical validation also forbids single-use pure local aliases.

If:
- a local `l` binding is pure
- its bound name is used exactly once
- direct substitution is syntactically valid

then the binding is rejected and the expression must be inlined.

This rule is mechanical and does not depend on naming intent or readability judgments. Local bindings are reserved for reuse, effect sequencing, destructuring, recursion, or syntax-required staging.

### Type Schemes

Type schemes represent explicit top-level polymorphic types:

```
σ ::= ∀α₁...αₙ.τ
```

Example:
```sigil
λidentity[T](x:T)→T=x
```
Infers type: `∀T.λ(T)→T`

### Instantiation

When using an explicitly polymorphic top-level value, instantiate it with fresh type variables:

```sigil
λidentity[T](x:T)→T=x
identity(5)        (* T instantiated to Int *)
identity("hi")     (* T instantiated to String *)
```

## Primitive Types

### Built-in Types

| Symbol | Name | Description | Values |
|--------|------|-------------|--------|
| `Int` | Integer | Whole numbers | `-2147483648` to `2147483647` |
| `Float` | Float | Floating point | IEEE 754 double precision |
| `Bool` | Boolean | Truth values | `true` (true), `false` (false) |
| `String` | String | UTF-8 strings | `"hello"` |
| `Char` | Character | Unicode character | `'a'` |
| `Unit` | Unit | Empty type | `()` |
| `Never` | Never | Uninhabited type | No values (for diverging functions) |

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
t User={email:String,id:Int,name:String}
t Point={x:Float,y:Float}
```

**Type Rules**:
- Fields have names and types
- Field access: `user.name` has type `String` if `user : User`
- Record literals must include all fields
- Record types are **closed exact products**
- Missing fields are rejected
- Extra fields are rejected
- Record types do **not** use width subtyping
- Sigil has no row polymorphism, row tails, or open/partial-record semantics
- If a field may be absent, the canonical encoding is an exact record with `Option[T]`
- Canonical form requires record fields to be alphabetically ordered in declarations, literals, and patterns
- Named product types participate in structural equality after normalization

Exactness is enforced by the frontend and typechecker, not left as a style recommendation.
Attempts to write open/partial-record forms are rejected with `SIGIL-CANON-RECORD-EXACTNESS`.

### Type Aliases

```sigil
t UserId=Int
t Email=String
```

Type aliases create synonyms, not new types (structural typing).

If a value must remain distinct from its raw representation, use a named wrapper
type instead of an alias:

```sigil
t Email=Email(String)
t UserId=UserId(Int)
```

This is the canonical way to represent validated boundary values that should not
be interchangeable with raw strings or integers inside business logic.

## Trusted Internal Data and Boundaries

Sigil distinguishes two phases of data:

1. **External / uncertain data**
   - JSON text
   - parsed `JsonValue`
   - URL text
   - timestamp text
2. **Trusted internal data**
   - exact records
   - explicit `Option[T]` / `Result[T,E]`
   - validated wrapper types

The canonical pipeline is:

```sigil
raw input → parse → decode / validate → trusted internal type
```

For JSON-backed boundaries, `stdlib⋅json` owns raw parsing and `stdlib⋅decode`
owns conversion into trusted internal types.

Example:

```sigil
t Message={createdAt:Instant,text:String}

⟦ raw JSON must be decoded before business logic sees Message ⟧
```

From that point on, `message.createdAt` is simply present. Sigil does not expect
internal business logic to keep treating exact validated values as if they were
still raw external blobs.

### Canonical Semantic Equality

Sigil compares aliases and named product types by their normalized canonical form
whenever compatibility/equality is checked.

Informally:

```
normalize(UserId) = Int            if t UserId=Int
normalize(Todo) = {done:Bool,id:Int,text:String}
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
λadd(x:Int,y:Int)→Int=x+y
```

Type: `λ(Int,Int)→Int`

### Higher-Order Functions

Functions can take functions as arguments:

```sigil
λmap[T,U](fn:λ(T)→U,list:[T])→[U]=...
```

Type: `∀T,U.λ(λ(T)→U,[T])→[U]`

### Currying

Functions are **not curried** by default (unlike Haskell). Multi-argument functions take tuples implicitly:

```sigil
λadd(x:Int,y:Int)→Int=x+y
```

Type is `λ(Int,Int)→Int`, **not** `λ(Int)→λ(Int)→Int`.

To create curried functions explicitly:

```sigil
λadd(x:Int)→λ(Int)→Int=λy→x+y
```

## Pattern Matching Type Rules

### Exhaustiveness Checking

Pattern matches must cover all possible values:

```sigil
(* OK - exhaustive *)
λsign(n:Int)→String match n{
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
  None→0           (* return type must match: Int *)
}
```

### Type Constraints from Patterns

```sigil
λlength[T](list:[T])→Int match list{
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
λread_file(path:String)→Result[String,IoError]!IO
λfetch_url(url:String)→Result[String,HttpError]!Network
```

**Syntax**: `!EffectName`

### Pure Functions

Functions without effect annotations are pure:

```sigil
λadd(x:Int,y:Int)→Int=x+y    (* Pure, no effects *)
```

### Effect Inference

Effects are inferred from function calls:

```sigil
λprocess_file(path:String)→Result[Int,Error]!IO=
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
λlength[T](list:&[T])→Int=...    (* Borrows list, doesn't take ownership *)

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

W(Γ, x) = (Never, inst(Γ(x)))
W(Γ, n) = (Never, Int)
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
unify(Int, Int) = Never
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
   λadd(x:Int,y:Int)→Int=x+y
   add(5,"hello")    (* ERROR: expected Int, got String *)
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
   λpure_fn(x:Int)→Int=read_file("data.txt")  (* ERROR: !IO effect in pure function *)
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
t Memo={cache:{Int:Int}}

λfib_memo(n:Int,memo:&mut Memo)→Int=
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
- `memo.cache : {Int:Int}`
- `memo.cache.get(n) : Option[Int]`
- `result : Int`

## Type System Extensions (Future)

### Higher-Kinded Types

```sigil
t Functor[F[_]]={
  map:∀T,U.λ(λ(T)→U,F[T])→F[U]
}
```

### Dependent Types

```sigil
t Vec[T,n:Int]=[T]  (* Vector of length n *)

λhead[T,n:Int](v:Vec[T,n])→T where n>0=...
```

### Row Polymorphism

```sigil
t User={id:Int,name:String,..r}  (* User with at least id and name; row tail follows fixed fields *)
```

## References

1. Damas, L., & Milner, R. (1982). "Principal type-schemes for functional programs"
2. Pierce, B. C. (2002). "Types and Programming Languages"
3. Harper, R. (2016). "Practical Foundations for Programming Languages"
4. Jung, R., et al. (2017). "RustBelt: Securing the Foundations of the Rust Programming Language"

---

**Next**: See `semantics.md` for operational semantics and evaluation rules.
