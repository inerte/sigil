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
- nominal sum constructors

So these are real checked programs:

```sigil
λchoose(flag:Bool)=>String match flag{
  true=>"enabled"|
  false=>"disabled"
}

λheadOrZero(xs:[Int])=>Int match xs{
  []=>0|
  [head,.tail]=>head
}

λpairLabel(left:Bool,right:Bool)=>String match (left,right){
  (true,true)=>"tt"|
  (true,false)=>"tf"|
  (false,true)=>"ft"|
  (false,false)=>"ff"
}
```

If one arm is missing, `compile` now fails instead of silently accepting an
incomplete branch tree.

## Guards and the Proof Fragment

Pattern guards are still part of the language, but the compiler now makes an
explicit distinction between guards it can reason about and guards it cannot.

The supported proof fragment is intentionally small:

- `true` and `false`
- equality and order comparisons between a bound pattern variable and a literal
- boolean `and`, `or`, and `not` over those facts

That means the checker can understand cases like:

```sigil
λband(n:Int)=>String match n{
  value when value<0=>"negative"|
  0=>"zero"|
  value when value<10=>"single-digit positive"|
  _=>"double-digit or larger"
}
```

But it does not try to prove arbitrary facts about:

- function calls
- world-dependent expressions
- general arithmetic relations between multiple variables
- field-access-heavy predicates

Those guards remain valid source. They just remain opaque to the exhaustiveness
proof.

## Why the Compile Errors Changed

Sigil's compile step already emits machine-readable JSON, so the match checker
now puts its proof context directly there instead of requiring a second command.

On relevant failures, `compile` now includes structured details such as:

- uncovered cases
- suggested missing arms
- known facts from earlier arms
- unsupported guard facts
- the current proof fragment

That matters for both humans and agents. The compiler is not only saying
"non-exhaustive." It is also saying what it still believes is uncovered and
which guard facts it ignored.

## The Goal

This is not an attempt to turn Sigil into a general theorem prover. The current
design is narrower than that on purpose.

The point is to take the parts of branching that are already finite and
structural, check them aggressively, and report the results in a form that is
both readable and repairable. That closes a real safety hole without adding a
second branching construct or hiding the proof model behind vague diagnostics.
