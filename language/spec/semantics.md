# Sigil Operational Semantics

Version: 1.0.0
Last Updated: 2026-03-14

## Overview

This document records the current operational model described by the language
docs and enforced surface, without specifying unimplemented ownership or borrow
semantics.

Sigil is:

- immutable by default
- explicit about effects
- concurrent by default at the runtime model level

## Evaluation Strategy

### Demand-Driven Execution

Sigil may start independent work early and join results when a strict consumer
needs the value.

### Effect Initiation Order

Effectful sibling expressions are initiated in source order.

That means:

- pure sibling work may overlap
- effectful sibling work starts left-to-right

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
- list operations

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

Examples:

```sigil module
λclassify(n:Int)=>String match n{
  0=>"zero"|
  5=>"five"|
  _=>"other"
}
```

## Lists

List literals preserve nesting exactly as written.

Examples:

```text
[[1,2]] ≠ [1,2]
```

Concatenation is expressed with `⧺`, not by implicit flattening.

## Records

Record access selects a field from an already-evaluated record value.

```sigil expr
{id:1,name:"Alice"}.name
```

## Effects

Pure code has no observable effects.

Effectful functions and tests declare effects explicitly in the surface syntax.

Examples:

```sigil program language/test-fixtures/tests/semanticsEffects.sigil
e console

λmain()=>Unit=()

test "writes log" =>!IO  {
  console.log("x")=()
}
```

## What This Spec Does Not Claim

This document intentionally does not specify:

- mutable bindings
- ownership transfer rules
- borrowing with `&` or `&mut`
- borrow checker constraints
- lifetimes

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
