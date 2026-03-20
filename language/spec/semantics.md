# Sigil Operational Semantics

Version: 1.0.0
Last Updated: 2026-03-19

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
- `if` conditions
- `match` scrutinees and guards
- field access and indexing

### Explicit Concurrent Regions

Sigil widens work only through named concurrent regions:

```text
concurrent regionName(config){
  spawn expr
  spawnEach list fn
}
```

Current region invariants:

- the region is named
- the config is an exact record literal
- record fields are canonical alphabetical order
- the body is spawn-only

Current config surface:

- `concurrency:Int`
- `jitterMs:Option[{max:Int,min:Int}]`
- `stopOn: λ(E)=>Bool`
- `windowMs:Option[Int]`

Current child surface:

- `spawn expr` where `expr : !IO Result[T,E]`
- `spawnEach list fn` where `fn : A=>!IO Result[T,E]`

Region result:

- `!IO [ConcurrentOutcome[T,E]]`

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

- no more than `concurrency` child starts in any `windowMs` window

`jitterMs` means:

- each child start may be delayed by a randomized value inside the configured
  range

These controls apply only inside the named region that declares them.

Nested regions are allowed and use their own policies independently.

## Values

Examples of values include:

- literals
- list literals of values
- record literals of values
- constructor applications over values
- lambdas

## Core Expression Forms

Current operationally relevant expression forms include:

- function application
- local `l` binding
- `match`
- lambdas
- record access
- canonical list operators
- named concurrent regions

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
- source-level canonical validation may reject some pure single-use locals
  earlier and require the already-inlined form

## Pattern Matching

`match` evaluates the scrutinee, then selects the first matching arm.

## Lists

List literals preserve nesting exactly as written.

Concatenation is expressed with `⧺`, not by implicit flattening.

Pure list operators remain canonical value transforms:

- `↦`
- `⊳`
- `⊕`

They are not the concurrency surface.

## Records

Record access selects a field from an already-evaluated record value.

```sigil expr
{id:1,name:"Alice"}.name
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

- recursive result-building of the form `self(rest)⧺rhs`
- filter then length of the form `#(xs⊳pred)`

The required replacements are:

- `stdlib::list.all`
- `stdlib::list.any`
- `stdlib::list.countIf`
- `↦`
- `⊳`
- `stdlib::list.find`
- `stdlib::list.flatMap`
- `⊕` / `stdlib::list.fold`
- `stdlib::list.reverse`

These are exact-shape canonicality rules, not general semantic equivalence or
complexity proofs.
