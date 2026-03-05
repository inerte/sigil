---
title: "Why Sigil Bans Shadowing"
date: 2026-03-03
author: Sigil Language Team
slug: why-sigil-bans-shadowing
---

# Why Sigil Bans Shadowing

**TL;DR:** Sigil bans shadowing because one local name should have one meaning. That makes generated code safer to refactor, easier for Claude Code and Codex to explain, and more consistent with Sigil's canonical-forms philosophy.

## Many Languages Allow Shadowing

Shadowing is common.

An inner `x` can replace an outer `x`.
A nested lambda parameter can reuse the same name as an enclosing local.
A match pattern can bind a name that already exists outside the match.

That is legal in many languages, but Sigil is not trying to preserve every familiar convenience.

Sigil is trying to preserve one canonical meaning per local name.

## The Problem Is Not Scope Rules

The problem is variation.

When code can reuse names across nested scopes, the same short identifier starts meaning different things depending on where you are in the function.

That creates two costs:

1. safety cost
2. explanation cost

The safety cost is obvious:

```sigil
⟦ BAD ⟧
λprocess_user(name:𝕊)→𝕊={
  l name=(stdlib⋅string.trim(name):𝕊);
  name
}
```

The explanation cost matters just as much for Sigil:

- Claude Code has to explain which `name` is which
- Codex has to decide whether reusing `name` is safe
- generated code becomes more context-sensitive than it needs to be

## Sigil's Rule

Sigil now treats shadowing as non-canonical.

That means:

- function parameters cannot be rebound
- lambda parameters cannot reuse enclosing local names
- `l` bindings cannot reuse names from enclosing scopes
- pattern bindings cannot silently override outer locals

This is valid:

```sigil
λprocess_user(name:𝕊)→𝕊={
  l normalized_name=(stdlib⋅string.trim(name):𝕊);
  normalized_name
}
```

This is not:

```sigil
λprocess_user(name:𝕊)→𝕊={
  l name=(stdlib⋅string.trim(name):𝕊);
  name
}
```

## Why This Is A Safety Rule

Rebinding short names is a common source of subtle mistakes:

- a parameter is hidden by a local binding
- a match pattern quietly overrides an outer value
- a nested lambda changes the meaning of a reused identifier

Sigil would rather force an explicit rename than accept that ambiguity.

`normalized_name`, `validated_name`, and `final_result` are better than pretending one reused identifier still means the same thing.

## Why This Is Also An AI Rule

Sigil is written primarily by Claude Code and Codex.

For those systems, shadowing is extra bookkeeping with no real upside.

If every local name has a single identity:

- generation is more deterministic
- explanations are cleaner
- edits are safer
- canonical code stays more uniform across the repo

This is exactly the kind of tradeoff Sigil should make.

## Canonical Forms, Applied To Names

Sigil already applies objective canonical rules to:

- declaration ordering
- parameter ordering
- effect ordering
- record field ordering

Banning shadowing extends that same idea to lexical bindings.

The language is saying:

> a local name is not a stylistic choice; it is part of the canonical program shape

## What Does Not Change

- lexical scopes still exist
- nested functions and matches still introduce nested scopes
- pattern matching semantics do not change
- type checking does not become more complex

Only one thing changes:

- reusing a local name from the same or an enclosing scope is now rejected

## The Broader Principle

Sigil should not preserve convenience when that convenience makes generated code more ambiguous.

Shadowing is one of those conveniences.

Other languages can keep it.
Sigil can do better for its actual users by enforcing:

- one local name
- one meaning
- one canonical program shape
