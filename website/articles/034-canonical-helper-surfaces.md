---
title: Canonical Helper Surfaces in Sigil
date: 2026-03-28
author: Sigil Language Team
slug: canonical-helper-surfaces
---

# Canonical Helper Surfaces in Sigil

Sigil now rejects exact top-level wrappers around canonical helper surfaces,
not just hand-rolled recursive list plumbing.

## The Change

Outside `language/stdlib/`, the validator now rejects functions whose body is
already one canonical helper surface over that function's own parameters.

Current direct-wrapper bans cover:

- direct `§...` helper calls such as `§list.sum(xs)` or `§string.trim(s)`
- direct `map` wrappers such as `xs map fn`
- direct `filter` wrappers such as `xs filter pred`
- direct `reduce ... from ...` wrappers such as `xs reduce fn from init`

This sits alongside the earlier exact recursive list-processing bans. Sigil is
not treating wrapper aliases as harmless style variation anymore when the body
is already the canonical helper.

## Why The Compiler Is Doing This

The repo already had a broader audit looking for local helper duplication, but
the high-confidence subset belongs in the compiler.

The practical problem is straightforward:

- people write `sum1`, `total`, `trimmed`, or `project`
- LLMs do the same thing with even more variation
- those wrappers teach multiple names for the same canonical operation

At the same time, compiler blocking has a stricter bar than a repo audit. A
false positive is a real build failure.

That is why the new rule is intentionally narrow. It is name-agnostic, but it
only fires on exact AST shapes that the compiler can identify with high
confidence.

## What Is Rejected

Rejected:

```sigil invalid-module
λsum1(xs:[Int])=>Int=§list.sum(xs)
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-HELPER-DIRECT-WRAPPER",
    "message": "Function 'sum1' duplicates canonical helper '§list.sum'"
  }
}
```

Rejected:

```sigil invalid-module
λproject[T,U](fn:λ(T)=>U,xs:[T])=>[U]=xs map fn
```

Required:

```sigil module
λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)

λreportedSum(xs:[Int])=>String=§string.intToString(§list.sum(xs))
```

The wrapper name does not matter. `sum1`, `addTwo`, `total`, or `project` are
all blocked if the body is still just the canonical helper surface.

## What Is Still Allowed

This is not general semantic equivalence checking.

Still allowed:

```sigil module
λsumPlusOne(xs:[Int])=>Int=§list.sum(xs)+1
```

```sigil module
λtrimmedLines(text:String)=>[String]=§string.lines(§string.trim(text))
```

```sigil module
total λgo(acc:Int,xs:[Int])=>Int
decreases #xs
match xs{
  []=>acc|
  [
  x,
  .rest
]=>go(
    acc+x,
    rest
  )
}

λsumWithHelper(xs:[Int])=>Int=go(
  0,
  xs
)
```

If the helper carries a termination proof, mark that helper `total`. The public
wrapper can stay ordinary unless it also needs total reasoning.

Those may still be undesirable for other reasons, but they are no longer exact
wrappers. The compiler does not try to prove that arbitrary helper code is
equivalent to `§list.sum`, `§string.trim`, or another canonical surface.

`language/stdlib/` is also exempt. The stdlib is where canonical helper
definitions themselves live.

## Why This Stays Separate From Canonical List Processing

The earlier recursive list-processing change and this direct-wrapper change are
related, but they are not the same rule.

The recursive rules collapse hand-written traversal shapes into one canonical
surface. The new wrapper rule collapses thin aliases around helpers that are
already canonical. Both are exact-shape rules, but they operate at different
levels of the language.
