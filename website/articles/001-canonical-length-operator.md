---
title: Why Sigil Uses the # Length Operator
date: 2026-02-24
author: Sigil Language Team
slug: 001-canonical-length-operator
---

# Why Sigil Uses the `#` Length Operator

One of Sigil's early syntax decisions was how to express length for strings and
lists. Many languages expose several competing forms: a built-in function, a
property, or type-specific helpers. That flexibility is familiar, but it also
creates representational noise. Sigil is trying to remove that kind of choice.

## The Problem

The semantic operation is simple: obtain the length of a sequence-like value.
The difficulty is that mainstream languages usually expose the same operation in
multiple syntactic forms.

```python
len("hello")
"hello".length
StringUtils.len("hello")
```

For Sigil, that would create several problems at once:

- the same concept would have multiple surface forms
- code generation would need one more style decision
- training examples for tools and agents would drift immediately

The language did not need another namespace decision for a primitive operation.

## The Decision

Sigil uses a dedicated prefix operator:

```sigil exprs
#"hello"
#[1,2,3]
```

The choice is intentionally narrow. There is no alternate `len(...)` spelling,
no `.length` property, and no type-specific helper namespace for this concept.

That gives Sigil one canonical representation for "get the length of a string or
list."

## Why an Operator Instead of a Function

We considered both a generic `len()` function and type-specific helpers.

Type-specific helpers were the weakest option because they would split one
concept across multiple namespaces. A generic function was cleaner, but still
introduced a second naming layer for something the type checker already knows
how to validate directly.

The operator keeps the rule small:

- one surface form
- one compile-time check
- one result type (`Int`)

It also keeps length aligned with Sigil's preference for compact, dedicated
syntax for primitive operations.

## Type Checking and Code Generation

`#` is not a polymorphic library function. It is a primitive operator checked by
the compiler against known types. The operand must be either `String` or `[T]`,
and the result is always `Int`.

Because the type is already known statically, code generation is straightforward
and does not require runtime dispatch. On the current JavaScript target, both
strings and arrays map cleanly to `.length` after the type checker has already
validated the operation.

## Why This Fits Sigil

This is a small feature, but it shows the broader rule Sigil is trying to
follow: if one idea does not need multiple spellings, the language should not
leave them available. The `#` operator is not only shorter than the obvious
alternatives. More importantly, it makes length a single canonical concept in
the surface language.
