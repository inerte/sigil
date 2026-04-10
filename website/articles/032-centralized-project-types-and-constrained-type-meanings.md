---
title: Centralized Project Types and Constrained Type Meanings
date: 2026-03-26
author: Sigil Language Team
slug: centralized-project-types-and-constrained-type-meanings
---

# Centralized Project Types and Constrained Type Meanings

Sigil now treats project-defined types as part of the project's foundational
vocabulary instead of ordinary per-module implementation detail.

## The Problem

Once a project grows, domain types start to spread.

One module defines `User`, another defines `PersistedState`, a third adds
`Email`, and a fourth quietly invents a second near-duplicate wrapper for the
same concept. Even when the names are good, the vocabulary is fragmented.

That fragmentation creates two problems:

- the project has no single canonical place for its domain language
- many of the facts people care about end up back in comments because plain
  `Int` and `String` are too weak to carry the intended meaning

Sigil already prefers explicit, compiler-owned structure over conventions. Type
vocabulary should follow the same rule.

## The Decision

Projects now centralize named project types in one file:

```text
src/types.lib.sigil
```

That file is compiler-known and owns project type declarations. It now also
hosts project `label` declarations, while the `µ...` reference surface for
project-defined types stays the same outside the file.

Example:

```sigil module projects/algorithms/src/types.lib.sigil
t BirthYear=Int where value>1800 and value<10000

t TopologicalSortResult=CycleDetected()|Ordering([Int])

t User={
  birthYear:BirthYear,
  name:String
}
```

```sigil module projects/algorithms/src/topologicalSortView.lib.sigil
λorderingValues(result:µTopologicalSortResult)=>[Int] match result{
  µOrdering(order)=>order|
  µCycleDetected()=>[]
}
```

This does two things at once:

- it gives the project one canonical home for named domain vocabulary
- it makes project-defined types visibly different from stdlib, config, world,
  and ordinary source-module references

## Constrained Types

Named user-defined types may also carry a pure `where` clause:

```sigil module projects/algorithms/src/types.lib.sigil
t BirthYear=Int where value>1800 and value<10000

t DateRange={
  end:Int,
  start:Int
} where value.end≥value.start
```

The point is not to turn every type declaration into a runtime admission gate.
The point is to let types carry checked semantic meaning directly in the source.

That means:

- `BirthYear` says more than bare `Int`
- `DateRange` says more than a record with two integers
- fewer invariants need to be repeated in comments

Current Sigil gives this a precise compile-time role:

- `where` is pure and world-independent
- only `value` is in scope
- the compiler typechecks the constraint expression
- constrained aliases and constrained named product types act as refinements over their underlying type
- values flow into the constrained type only when the compiler can prove the predicate
- constrained values widen back to the underlying type automatically
- the current proof fragment covers Bool/Int literals, `value`, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, and `not`
- `match`, exact record patterns, and internal branching propagate supported branch facts into that proof
- direct boolean local aliases of supported facts also narrow
- the proof backend is solver-backed, but the surface stays the same small canonical Sigil syntax
- there is no generated runtime validation

So this feature is about **stronger type meaning with compile-time proof**,
not about silently inserting runtime checks.

That proof is flow-sensitive, not only literal-based. For example:

```sigil module
t BirthYear=Int where value>1800

λpromote(year:Int)=>BirthYear match year>1800{
  true=>year|
  false=>1900
}
```

The `true` arm can return `year` directly because the branch fact becomes part
of the refinement proof.

For function boundaries, Sigil now uses a separate contract surface rather than
overloading `where`:

- `where` defines membership in a type
- `requires` states what a caller must prove before a call
- `ensures` states what a callee guarantees after it returns

That keeps type membership and call-boundary obligations distinct. See
[/articles/requires-and-ensures-function-contracts/](/articles/requires-and-ensures-function-contracts/).

## Type Equality

This also clarifies the structural-equality story.

Sigil still normalizes unconstrained aliases and unconstrained named product
types structurally. That part has not changed.

What changed is that constrained aliases and constrained project-defined named
products no longer participate in plain structural equality. They use
refinement checks over their underlying type. If a type carries an extra
predicate, the checker should preserve that fact instead of erasing it during
compatibility checks.

## Why This Matters

This is useful for both humans and tools.

Humans get one place to look for the project's domain vocabulary.

LLMs get a clearer, compiler-enforced project shape:

- `•...` for source modules
- `§...` for stdlib
- `†...` for world
- `※...` for test
- `µ...` for project-defined types

And the type declarations themselves can now carry some of the meaning that
would otherwise drift into comments, while the compiler enforces that meaning
at the places where values are introduced.

That is the real goal of this change: not to make Sigil more ceremonial, but to
move more domain intent into checked, canonical source.
