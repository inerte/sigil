---
title: "Why Sigil Uses Explicit Generics Without Hindley-Milner"
date: March 3, 2026
author: Sigil Language Team
slug: why-sigil-uses-explicit-generics-without-hm
---

Sigil now has real generics, but it still does not use Hindley-Milner as its user model.

That distinction matters.

`Option[T]`, `Result[T,E]`, generic list helpers, and generic top-level functions are all valuable for a machine-first language because they compress vocabulary. `Option[T]` is one canonical abstraction. `IntOption`, `StringOption`, `UserOption`, and every other monomorphic wrapper explode the name surface that models need to remember and generate consistently.

What Sigil does **not** want back is HM-style let-polymorphism. We do not want a local binding like `l id=...` to become implicitly polymorphic through checker magic. That is good for terse human-written code, but it is a poor fit for Sigil's goals:

1. It hides behavior that is not obvious from the declaration.
2. It makes local reasoning weaker for code generators and repair tools.
3. It introduces more ways for the checker to "do something clever" behind the source.

So the rule is intentionally narrow:

1. Genericity is declared explicitly on top-level declarations.
2. Generic ADTs and constructors are real.
3. Generic top-level functions are real.
4. Local bindings remain monomorphic.
5. There is no call-site `f[T](x)` syntax.

That gives Sigil one regular pattern without reopening the whole HM inference story.

## Examples

Generic type declarations:

```sigil
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
```

Generic top-level functions:

```sigil
λidentity[T](x:T)→T=x
λunwrap_or[T](fallback:T,opt:Option[T])→T match opt{
  Some(value)→value|
  None()→fallback
}
```

Imported constructors use the same qualified form as everything else when they are not part of the implicit core prelude:

```sigil
i core⋅option

λmain()→Option[ℤ]=Some(42)
```

Pattern matching keeps the same shape:

```sigil
match opt{
  Some(value)→value|
  None()→0
}
```

## Why Not Add Call-Site Type Arguments?

Because Sigil already has a canonical place to supply type information:

1. declaration annotations
2. expected return types
3. type ascriptions
4. pattern-match scrutinee types

If a generic use is underconstrained, the language should ask for ordinary type information in those canonical places instead of adding another syntax surface.

## Why This Is Better For LLMs

This is the main reason for the design.

Explicit generics reduce hallucination pressure:

1. one canonical `Option[T]` instead of many invented wrapper names
2. one canonical `Result[T,E]` instead of ad hoc error containers
3. reusable generic list helpers instead of per-type copies

At the same time, rejecting HM-style local generalization keeps the language tighter:

1. genericity appears where it is declared
2. local code does not become polymorphic implicitly
3. the checker stays predictable about where generic behavior can originate

That is the balance Sigil wants: compositional abstractions without hidden polymorphic magic.
