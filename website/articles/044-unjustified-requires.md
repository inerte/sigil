---
title: "SIGIL-CANON-UNJUSTIFIED-REQUIRES: Enforcing Load-Bearing Contracts"
date: 2026-05-08
author: Sigil Language Team
slug: unjustified-requires
---

# `SIGIL-CANON-UNJUSTIFIED-REQUIRES`: Enforcing Load-Bearing Contracts

In Sigil, `requires` is not documentation. Every `requires` clause on a function
becomes a proof obligation that every call site must discharge — the compiler
runs Z3 to verify that callers can demonstrate the precondition holds before
the call. A `requires` clause that is never mechanically useful still imposes
that burden.

This creates a class of non-canonical code: a `total` function with a `requires`
clause that contributes nothing to any proof the compiler actually runs.

```sigil module
mode total

λisPalindrome(s:String)=>Bool
requires #s>0
=s=§string.reverse(s)
```

`§string.reverse` accepts any string including the empty string. The body
typechecks without the precondition. The `ensures` is absent. There is no
termination proof. Every call site must prove `#s>0`, but removing the clause
changes nothing about what the compiler can verify. The precondition is dead
weight.

Sigil now detects and rejects this pattern as `SIGIL-CANON-UNJUSTIFIED-REQUIRES`.

## What Makes a `requires` Clause Load-Bearing

A `requires` clause is load-bearing if removing it causes at least one
mechanically-checked obligation to fail:

- **Body type-checking** — the body uses a callee whose `requires` needs the
  precondition to be discharged at the call site
- **Constrained return type** — the body cannot be proved to satisfy a refined
  return type without the precondition
- **`ensures` proof** — a postcondition cannot be proved from the body without
  the precondition
- **`decreases` proof** — the termination measure cannot be bounded or shown
  to decrease without the precondition

The check replays all of these obligations with the `requires` formula stripped
from the proof context. If every obligation still passes, the clause is
unjustified.

```sigil module
total λfibHelper(a:Int,b:Int,n:Int)=>Int
requires n≥0
decreases n
match n{
  0=>a|
  count=>fibHelper(
    b,
    a+b,
    count-1
  )
}
```

Here `requires n≥0` is load-bearing: the `decreases n` bound check must prove
`n≥0` at function entry, and the recursive call `fibHelper(b,a+b,count-1)`
must prove `count-1≥0` (which uses `n≥0` together with the match arm
`count≠0`). Removing the precondition causes both proofs to fail.

## Current Scope

The check currently applies to `total` functions and transforms that have no
`decreases` clause, where the `requires` expression is a **length predicate**
— a comparison where at least one side uses the `#` (length) operator, or a
conjunction or disjunction of such comparisons.

Length predicates are fully within Sigil's symbolic proof fragment. The replay
result is reliable for them: either the obligations pass without the clause
(it is unjustified) or they do not (it is load-bearing).

Value-domain restrictions such as `requires n≥0` or `requires b≥0` are
excluded from the check for now. These often carry semantic meaning that the
current proof system cannot mechanically verify — a callee that has no
matching contract, or a function that handles negative inputs but produces
mathematically wrong results for them. Collapsing semantic intent and
mechanical justification is a problem the type system needs to solve more
completely before the check can extend there cleanly.

Functions with a `decreases` clause are also excluded. The termination
proof infrastructure adds implicit non-negativity assumptions for measure
components, which makes the bound-check replay unreliable without changes to
that infrastructure.

## Protocol-State Exceptions

`requires handle.state=StateName` assertions are always exempt, including when
mixed with other predicates. Protocol-state facts are axiomatic — they declare
runtime state obligations the proof system does not attempt to prove from
function bodies, and they have no equivalent encoding as constrained parameter
types.

## The Canonical Fix

When the check fires, the fix is to remove the unjustified clause:

```sigil invalid-module
mode total

λisPalindrome(s:String)=>Bool
requires #s>0
=s=§string.reverse(s)
```

```sigil module
mode total

λisPalindrome(s:String)=>Bool=s=§string.reverse(s)
```

If the intent is to restrict the parameter domain for correctness reasons, the
domain belongs in the type:

```sigil module
t NonEmptyString=String where #value>0

λisPalindrome(s:NonEmptyString)=>Bool=s=§string.reverse(s)
```

The constrained type enforces the restriction at every call site through the
type system rather than through a free-floating proof obligation. Callers that
have a value of type `NonEmptyString` pass it directly; callers with a plain
`String` must promote it, which the compiler checks against the `where`
constraint.

## Why This Is a Canonical Form Violation

Sigil treats `requires` as a mechanical contract, not a comment. A `requires`
clause that does no mechanical work violates the principle that every surface
form has one meaning. Keeping a dead precondition says one thing to the reader
(this input restriction matters) and something different to the compiler (it
makes no difference). That gap is non-canonical.

The check is conservative by design. It fires only when the proof replay is
definitive: when the proof fragment covers the predicate completely and the
obligations demonstrably pass without it. Cases where the answer is ambiguous
are left alone until the infrastructure for a reliable verdict exists.
