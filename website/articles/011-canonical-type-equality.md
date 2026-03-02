---
title: "Canonical Type Equality: Why Sigil Normalizes Structural Types Everywhere"
date: March 2, 2026
author: Sigil Language Team
slug: canonical-type-equality
---

# Canonical Type Equality: Why Sigil Normalizes Structural Types Everywhere

**TL;DR:** Sigil already enforced canonical syntax. This change makes type compatibility canonical too: aliases and named product types now compare by their normalized structural form everywhere in the checker. That gives Claude Code and Codex one semantic rule instead of branch-specific surprises.

## The Problem: Same Type, Different Answers

Sigil had several checker paths that compared raw synthesized types directly.

That produced behavior like this:

```sigil
t MkdirOptions={recursive:𝔹}
c opts=({recursive:true}:MkdirOptions)
```

and:

```sigil
t Todo={done:𝔹,id:ℤ,text:𝕊}
λaddTodo(id:ℤ,text:𝕊,todos:[Todo])→[Todo]=[Todo{done:false,id:id,text:text}]⧺todos
```

Both examples are obviously the same explicit type relation:

- `MkdirOptions` vs `{recursive:𝔹}`
- `Todo` vs `{done:𝔹,id:ℤ,text:𝕊}`

But some checker paths accepted them while others rejected them.

That is unacceptable for an AI-first language.

## Why This Is Not Type Inference

Sigil is not adding hidden type guessing here.

The programmer still writes explicit types.
The checker just resolves the canonical meaning of named structural types before asking:

> are these two explicit types the same?

That is semantic normalization, not inference.

## The Rule

Sigil now follows one invariant everywhere equality-sensitive checks happen:

- type aliases normalize to their underlying type
- named product types normalize to their structural record form
- sum types remain nominal

Examples:

```sigil
t UserId=ℤ
t Todo={done:𝔹,id:ℤ,text:𝕊}
```

Canonical semantic forms:

- `UserId` → `ℤ`
- `Todo` → `{done:𝔹,id:ℤ,text:𝕊}`

So the checker compares the normalized forms, not the unresolved names.

## Why Sigil Needs This

Sigil’s primary user is not a human hand-authoring syntax.
It is Claude Code and Codex generating canonical code and relying on deterministic semantics.

That means the language needs:

1. One canonical syntax
2. One canonical semantic meaning
3. One compatibility rule everywhere in the checker

If `Todo` equals its record form in one checker branch but not another, the model learns the wrong lesson:

- “sometimes names matter”
- “sometimes structure matters”
- “it depends where the type appears”

That is exactly the kind of ambiguity Sigil is supposed to remove.

## Why Not Normalize Everything?

Because product types and sum types serve different purposes.

Aliases and named product types are structural descriptions. Normalizing them preserves their declared meaning.

Sum types are algebraic data types. Their identity matters. `Result` is not interchangeable with a record just because `Ok` carries one.

So the correct rule is:

- aliases + products normalize structurally
- sums stay nominal

## The Outcome

This change turns several bug fixes into one language invariant:

- typed FFI named option records work consistently
- list append respects named product types
- higher-order list operators compare named structural types consistently
- branch compatibility is deterministic

That is not a convenience feature.
It is canonical semantic equality for an AI-first language.
