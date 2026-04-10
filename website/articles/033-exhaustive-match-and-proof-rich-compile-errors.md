---
title: Exhaustive match and Proof-Rich Compile Errors
date: 2026-03-26
author: Sigil Language Team
slug: exhaustive-match-and-proof-rich-compile-errors
---

# Exhaustive match and Proof-Rich Compile Errors

Sigil now treats `match` as the language's one branching surface and checks it
accordingly. Non-exhaustive matches are rejected. Dead arms are rejected.
Boolean and tuple matches are not second-class special cases. They are part of
the same checked branching model as sums and list shapes.

## One Branching Surface

Earlier compiler residue still carried the idea that boolean branching should
eventually move to a separate `if` form. That is no longer the language
direction.

The implemented language is simpler than that:

- `match` handles structural branching
- `match` also handles boolean branching
- the compiler checks the same construct for exhaustiveness and redundancy

That keeps Sigil aligned with its general preference for one canonical surface
instead of parallel constructs that differ mostly by tradition.

## What the Checker Covers

The current exhaustiveness pass handles:

- `Bool`
- `Unit`
- tuples
- list shapes
- exact record patterns
- nominal sum constructors

So these are real checked programs:

```sigil module
λchoose(flag:Bool)=>String match flag{
  true=>"enabled"|
  false=>"disabled"
}

λheadOrZero(xs:[Int])=>Int match xs{
  []=>0|
  [
  head,
  .tail
]=>head
}

λpairLabel(left:Bool,right:Bool)=>String match (
  left,
  right
){
  (
  true,
  true
)=>"tt"|
  (
  true,
  false
)=>"tf"|
  (
  false,
  true
)=>"ft"|
  (
  false,
  false
)=>"ff"
}
```

If one arm is missing, `compile` now fails instead of silently accepting an
incomplete branch tree.

## Guards and the Refinement Fragment

Pattern guards are part of the same fact system that constrained types and
function contracts now use. The compiler does not have one proof model for
`where`, another for `requires` / `ensures`, and a third for `match`. It uses
one canonical proof fragment for coverage and flow-sensitive narrowing.

The supported fragment covers:

- `true` and `false`
- Bool/Int literals
- rooted or pattern-bound values
- `value` and `result` when those names are in scope
- field access
- `#` over strings, lists, and maps
- `+` and `-`
- comparisons
- boolean `and`, `or`, and `not`
- direct boolean local aliases of those supported facts
- shape facts from tuple, list, exact-record, and nominal-constructor patterns

That means the checker can understand cases like:

```sigil module
λband(n:Int)=>String match n{
  value when value<0=>"negative"|
  0=>"zero"|
  value when value<10=>"single-digit positive"|
  _=>"double-digit or larger"
}
```

So the checker can understand not only direct guards, but also small staged
facts like:

```sigil module
λband(n:Int)=>String={
  l ok=(n<10:Bool);
  match n{
    value when ok and value≥0=>"single-digit positive"|
    value when ok=>"single-digit"|
    _=>"other"
  }
}
```

But it still does not try to prove arbitrary facts about:

- function calls
- world-dependent expressions
- general arithmetic relations between multiple variables
- user-defined proof helpers

Those guards remain valid source. They just remain opaque to coverage and
refinement narrowing.

## Why the Compile Errors Changed

Sigil's compile step already emits machine-readable JSON, so the match checker
now puts its proof context directly there instead of requiring a second command.

On relevant failures, `compile` now includes structured details such as:

- uncovered cases
- suggested missing arms
- known facts from earlier arms
- unsupported guard facts
- the current proof fragment
- proof assumptions, goals, and solver outcomes when a proof obligation fails
- counterexample models when the solver finds one

That matters for both humans and agents. The compiler is not only saying
"non-exhaustive." It is also saying what it still believes is uncovered and
which facts stayed outside the supported fragment.

## The Goal

This is not an attempt to turn Sigil into a general theorem prover. The current
design is narrower than that on purpose.

The point is to take the parts of branching that are already finite and
structural, check them aggressively, and report the results in a form that is
both readable and repairable. That closes a real safety hole without adding a
second branching construct or hiding the proof model behind vague diagnostics.
