---
title: Exact Records and Trusted Boundaries
date: 2026-03-07
author: Sigil Language Team
slug: exact-records-and-trusted-boundaries
---

# Exact Records and Trusted Boundaries

> Update (2026-03-26): project-defined internal records and wrappers now live
> in `src/types.lib.sigil` and are referenced elsewhere as `µMessage`,
> `µEmail`, `µUserId`, and so on. The boundary story below still applies; the
> domain vocabulary is just centralized now. See
> [/articles/centralized-project-types-and-constrained-type-meanings/](/articles/centralized-project-types-and-constrained-type-meanings/).

Sigil treats internal records as exact, closed products. That choice is closely
related to the language's approach to boundaries: uncertainty should be handled
at the edge of the system, not carried indefinitely through internal business
logic.

## The Problem

In many codebases, raw decoded objects and trusted domain values are represented
by shapes that are too similar. That encourages defensive checks inside ordinary
application logic against states that the type system was supposed to rule out.

Once that happens, the boundary between "validated data" and "raw external data"
starts to disappear.

## The Decision

If a Sigil value has type:

```sigil module
t Message={
  createdAt:§time.Instant,
  text:String
}
```

then the language treats that as an exact internal shape:

- the fields listed are the fields present
- required fields are present
- extra fields are not silently tolerated

If absence is real, Sigil expects it in the type:

```sigil module
t MaybeMessage={
  createdAt:Option[§time.Instant],
  text:String
}
```

## Why the Boundary Matters

External formats such as JSON are still uncertain. Sigil's intended flow is:

```text
raw input
=> parse
=> decode / validate
=> trusted internal record
```

That keeps fuzzy data at the boundary and gives the rest of the program a shape
it can trust.

The same logic applies to wrapper-backed values such as `Email` or `UserId`.
Once a value has crossed a validation boundary and been wrapped, internal code
should stop treating it like an unchecked primitive.

## Result

Exact records and trusted wrappers are part of the same policy: make validated
internal values look meaningfully different from raw external ones, and let the
compiler enforce that distinction wherever it can.
