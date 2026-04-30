---
title: Why decreases Is Total-Only in Sigil
date: 2026-04-29
author: Sigil Language Team
slug: why-decreases-is-total-only-in-sigil
---

# Why `decreases` Is Total-Only in Sigil

Recursion in Sigil was trying to do two different jobs at once.

Sometimes a recursive function is just executable code. It is part of ordinary
business logic, data transformation, rendering, search, parsing, or control
flow.

Sometimes a recursive function is stronger than that. Its termination is part of
what the program is claiming statically, and the compiler is expected to check
that claim.

Those are not the same thing. They should not share one default.

That is why Sigil now treats functions as ordinary by default and reserves
`decreases` for total code.

## The Problem Was Not Recursion Itself

The problem was proof burden.

If every self-recursive function must carry a termination argument, then the
language is effectively saying that every recursive helper is a proof site.

That is the wrong default for ordinary application code.

It pushes users toward one of two bad outcomes:

- fake fuel parameters that exist only to satisfy the checker
- unnatural helper reshaping even when the business logic was already clear

That tradeoff can make sense in a theorem prover or a language where totality is
the ambient default. It is a worse fit for Sigil's goal, which is to keep
ordinary code direct while still having a precise proof surface when it matters.

## The Split

Sigil now names the distinction explicitly.

Functions are ordinary by default.

If a file is mostly proof-relevant code, it may opt into a total default:

```sigil module
mode total
```

Individual declarations may still override that default with `total` or
`ordinary`.

That gives the language two clear modes:

- `ordinary` means executable code with no static totality claim
- `total` means the function participates in a checked total fragment

The point is not to decorate functions with a style label. The point is to say
whether termination is part of the declaration's meaning.

## Why `decreases` Belongs Only To Total Code

`decreases` is not just a hint about how recursion happens to proceed.

It is a proof-relevant clause. It exists to support a totality claim.

Once that is true, the rest follows.

An ordinary function should not declare `decreases`, because ordinary functions
are not making a totality claim in the first place. Allowing the clause there
would blur together two different ideas:

- "this code happens to recurse structurally"
- "the compiler should treat termination as part of the function contract"

Sigil keeps those separate.

A total function, on the other hand, must stay inside the total fragment. If the
compiler allowed a total function to call an ordinary declaration, the meaning
of total would become surprisingly weak. The declaration would carry a
termination proof on its own self-edge while still being able to disappear into
arbitrary ordinary code.

That is not a real totality boundary.

So Sigil makes the boundary explicit:

- ordinary self-recursive functions may recurse without `decreases`
- total self-recursive functions must provide `decreases`
- total functions may not call declarations marked `ordinary`

That gives `decreases` one precise meaning instead of several partial ones.

## What This Looks Like In Code

Ordinary recursion stays ordinary:

```sigil module
λcountdown(n:Int)=>String match n≤0{
  true=>"0"|
  false=>§string.intToString(n)
    ++","
    ++countdown(n-1)
}
```

Total recursion says more, so it must prove more:

```sigil module
total λfactorial(n:Int)=>Int
requires n≥0
decreases n
match n{
  0=>1|
  value=>value*factorial(value-1)
}
```

That is the intended reading difference.

The first function is ordinary executable recursion.

The second function is part of a checked total fragment, so termination belongs
to the declaration and `decreases` is the surface that makes that checkable.

## Why This Fits Sigil Better

Sigil prefers one canonical surface per concept.

The old all-recursion-needs-a-measure rule overloaded one mechanism across two
very different use cases:

- ordinary execution
- proof-relevant totality

The new split is stricter in the right place and looser in the right place.

It is looser for ordinary code because ordinary code does not need to pretend it
is participating in a proof discipline.

It is stricter for total code because totality only means something if the call
boundary is enforced and `decreases` remains a dedicated proof surface.

That is a better tradeoff than making every recursive function pay theorem-like
ceremony just to express routine program logic.

## PL/CS Design Notes

- Totality is a semantic mode, not a formatting preference. If the compiler is
  going to treat termination as part of meaning, that needs an explicit mode
  boundary.
- A termination measure is closer to a proof term than to a lint hint. It only
  makes sense when the surrounding call graph is constrained enough for the
  proof to mean what users think it means.
- This is a small, machine-first version of a familiar PL separation: ordinary
  executable code is not automatically proof-relevant code.
- Making ordinary the default avoids forcing every recursive program into a
  proof assistant posture, while still leaving a clear total fragment for code
  that genuinely needs that stronger static claim.