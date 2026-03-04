---
title: "Why Sigil Has a Core Prelude"
date: March 4, 2026
author: Sigil Language Team
slug: why-sigil-has-a-core-prelude
---

People often turn "core vs stdlib" into a namespace argument.

That is not the interesting part.

For Sigil, the real question is: **who canonically owns a concept?**

LLMs do not care very much whether a function is written as `join(...)` or `stdlib⋅string.join(...)`.
They care much more about:

1. whether there is one canonical spelling
2. whether the same concept appears under multiple competing names
3. whether a concept feels foundational enough to show up in most programs

That is why Sigil now has a small `core⋅prelude`.

## What Moved Into Core

These names are now implicit vocabulary:

```sigil
Option[T]
Result[T,E]
Some
None
Ok
Err
```

They are not special syntax. They are just foundational enough that forcing an import adds noise without adding real clarity.

So this:

```sigil
Some(42)
Ok("done")
```

is better for Sigil than:

```sigil
stdlib⋅option.Some(42)
stdlib⋅result.Ok("done")
```

The second form is not "more principled." It is just more repetitive.

## Why Most Helpers Still Stay Namespaced

Sigil does **not** want a giant implicit universe.

These still live behind module names:

```sigil
core⋅map.get("content-type",headers)
core⋅option.unwrap_or("guest",maybe_name)
stdlib⋅string.join(",",items)
```

That keeps the implicit surface small while still giving each concept one canonical owner.

The rule is:

1. foundational control/data vocabulary may belong in core
2. operational APIs usually stay namespaced
3. backend implementation details do not define the language surface

## Why `Map` Had To Become Real

`Map` was stuck in an awkward half-state:

1. the type existed in the language
2. the value-level story was unclear
3. records and dynamic dictionaries were too easy to blur together

That is worse than either extreme.

So Sigil now makes the distinction explicit:

### Records

Fixed-shape products use `:`

```sigil
t Response={body:𝕊,status:ℤ}
Response{body:"OK",status:200}
```

### Maps

Dynamic keyed collections use `↦`

```sigil
{"content-type"↦"text/plain","x-id"↦"42"}
({↦}:{𝕊↦𝕊})
```

This is the important distinction, not whether map operations happen to be implemented with JavaScript under the hood.

## The Real Design Lesson

Sigil is not trying to win a purity argument about prefixes.

It is trying to keep one canonical surface for each concept.

A large stdlib is fine.
A small core is fine.
What is not fine is:

1. duplicate ownership
2. muddy boundaries
3. half-core / half-library concepts

That is the standard future Sigil changes should follow.
