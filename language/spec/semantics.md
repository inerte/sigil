# Sigil Operational Semantics

Version: 1.0.0
Last Updated: 2026-04-05

## Overview

This document records the current operational model enforced by the implemented
Sigil surface.

Sigil is:

- immutable by default
- explicit about effects
- promise-shaped at runtime
- explicit about concurrency widening

## Evaluation Strategy

### Ordinary Evaluation

Ordinary Sigil expressions do not introduce broad implicit sibling fanout.

That means:

- expression evaluation stays promise-shaped
- sibling subexpressions are not treated as a hidden unbounded concurrency
  boundary
- strict consumers still demand concrete values when needed

Examples of strict consumers include:

- arithmetic and comparison operators
- `match` scrutinees and guards
- field access and indexing

### Explicit Concurrent Regions

Sigil widens work only through named concurrent regions:

```text
concurrent regionName@width:{policy}{
  spawn expr
  spawnEach list fn
}
```

Current region invariants:

- the region is named
- width is required after `@`
- optional policy is an exact record literal
- policy fields are canonical alphabetical order when present
- the body is spawn-only

Current region surface:

- width after `@`: `Int`
- `jitterMs:Option[{max:Int,min:Int}]`
- `stopOn: λ(E)=>Bool`
- `windowMs:Option[Int]`

Current child surface:

- `spawn expr` where `expr : !Fx Result[T,E]`
- `spawnEach list fn` where `fn : A=>!Fx Result[T,E]`

Region result:

- `[ConcurrentOutcome[T,E]]` under the combined effect set of width, policy,
  and child computations

with:

```text
ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)
```

### Ordering

Region outcomes are stable:

- `spawn` preserves lexical spawn order
- `spawnEach` preserves input order

This is independent of completion order.

### Stop Behavior

Each child returns `Result[T,E]`.

The region maps child completion into outcomes:

- `Ok(value)` => `Success(value)`
- `Err(error)` => `Failure(error)`
- unfinished or stopped work => `Aborted()`

When `stopOn(error)` returns `true`:

- new work is no longer scheduled
- unfinished work becomes `Aborted()` on a best-effort basis

The current implementation does not claim universal force-cancellation for
already-started backend operations.

### Window and Jitter

`windowMs` means:

- no more than `width` child starts in any `windowMs` window

`jitterMs` means:

- each child start may be delayed by a randomized value inside the configured
  range

Defaults when policy is omitted:

- `jitterMs = None()`
- `stopOn` is a pure function that always returns `false`
- `windowMs = None()`

These controls apply only inside the named region that declares them.

Nested regions are allowed and use their own policies independently.

## Values

Examples of values include:

- literals
- list literals of values
- record literals of values
- constructor applications over values
- lambdas

## Strings

Sigil string literals use one surface form: `"..."`

Current implemented behavior:

- string values are the exact contents between the quotes
- raw `\n` inside the literal becomes a newline in the value
- indentation spaces inside a multiline string literal remain part of the value
- escape sequences such as `\\`, `\"`, `\n`, `\r`, and `\t` still decode as usual
- there is no dedent or heredoc normalization step

## Core Expression Forms

Current operationally relevant expression forms include:

- function application
- local `l` binding
- `match`
- lambdas
- record access
- canonical list operators
- named concurrent regions

## Function Contracts

Functions may declare pure compile-time contracts with `requires` and
`ensures`.

Current implemented behavior:

- `requires` is checked at call sites against the current proof context
- `ensures` is checked against the function body with `result` bound to the returned value
- successful calls add proven `ensures` facts back into the caller's proof context
- contracts are pure and world-independent even on effectful functions
- effectful contracts describe only parameter obligations and returned-value guarantees, not world transitions or effect history
- contracts do not produce runtime checks or runtime metadata by themselves

## Local Bindings

Local bindings evaluate their right-hand side, then continue with the bound
value in scope:

```sigil module
λdoubledSum()=>Int={
  l x=(2+3:Int);
  x+x
}
```

Canonical note:

- runtime semantics include local bindings as expressions
- source-level canonical validation may reject zero-use named locals
- source-level canonical validation may reject some pure single-use locals
  earlier and require the already-inlined form
- use `l _=(...)` when the binding exists only to sequence effects

## Pattern Matching

`match` evaluates the scrutinee, then selects the first matching arm.

Current implemented invariants:

- `match` is the language's branching surface; there is no separate public `if`
- matches over `Bool`, `Unit`, tuples, list shapes, exact record patterns, and nominal sum constructors are checked for exhaustiveness
- redundant and unreachable arms are rejected before code generation
- coverage, contracts, and refinement narrowing share the same canonical proof fragment
- supported proof facts include Bool/Int literals, rooted or pattern-bound values, `value`, `result`, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, `not`, direct boolean local aliases of those supported facts, and shape facts introduced by tuple/list/record/constructor patterns
- unsupported guard facts remain valid source, but they are opaque to coverage and refinement narrowing

## Lists

List literals preserve nesting exactly as written.

Concatenation is expressed with `⧺`, not by implicit flattening.

Pure list operators remain canonical value transforms:

- `map`
- `filter`
- `reduce ... from ...`

They are not the concurrency surface.

## Records

Record access selects a field from an already-evaluated record value.

```sigil expr
{
  id:1,
  name:"Alice"
}.name
```

## Effects

Pure code has no observable effects.

Effectful functions and tests declare effects explicitly in the surface syntax.

## What This Spec Does Not Claim

This document intentionally does not specify:

- mutable bindings
- ownership transfer rules
- borrowing with `&` or `&mut`
- borrow checker constraints
- lifetimes
- automatic CPU parallelism
- general cancellation of arbitrary started backend effects

Those semantics are not part of the current implemented Sigil surface.

## Canonical Recursive List Processing

The current implementation also treats a small set of recursive list-plumbing
shapes as non-canonical when Sigil already has one required surface.

The validator rejects exact recursive clones of:

- `all`
- `any`
- `flatMap`
- `map`
- `filter`
- `find`
- `fold`
- `reverse`

It also rejects:

- exact top-level wrappers around canonical `§...` helper calls when the body
  is already that helper over the function's own parameters
- exact top-level wrappers around `map`, `filter`, and `reduce ... from ...`
- recursive result-building of the form `self(rest)⧺rhs`
- filter then length of the form `#(xs filter pred)`

The required replacements are:

- `§list.all`
- `§list.any`
- `§list.countIf`
- `map`
- `filter`
- `§list.find`
- `§list.flatMap`
- `reduce ... from ...` / `§list.fold`
- `§list.reverse`

These are exact-shape canonicality rules, not general semantic equivalence or
complexity proofs. `language/stdlib/` remains the place where canonical helper
definitions themselves live, so the direct-wrapper ban applies to ordinary
project/example/test code rather than to stdlib implementation files.
