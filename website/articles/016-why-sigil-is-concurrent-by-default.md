---
title: "Why Sigil Is Concurrent by Default"
date: March 3, 2026
author: Sigil Language Team
slug: why-sigil-is-concurrent-by-default
---

# Why Sigil Is Concurrent by Default

Sigil used to describe itself as "async by default", but the generated JavaScript still eagerly emitted `await` at ordinary call sites. That kept the surface language uniform while leaving most generated programs more sequential than they looked.

The new rule is stricter and more honest: Sigil is **concurrent by default**.

## The model

Sigil keeps one function form. There is no extra `async` syntax and no `Future` syntax in user code.

Instead, the compiler:

- starts independent work early
- keeps results promise-shaped while they flow through expressions
- joins values only at strict demand points

Strict demand points include:

- `if` conditions
- `match` scrutinees and guards
- arithmetic and comparison operators
- indexing and field access
- the final observable result of a program or test

## Why not add async syntax?

Because that would reintroduce the exact split Sigil is trying to avoid.

If the language had separate sync and async spellings, users and models would have to decide which form to use for every function, every API, and every example. Sigil wants one dominant way to write code and one runtime model underneath it.

## Ordered effects, concurrent resolution

Concurrent-by-default does **not** mean "effects happen in random order".

Effectful operations are initiated in source order. What changes is that Sigil no longer has to wait for each one to resolve before starting the next independent step.

That means:

- source order still matters for effects
- promise-based I/O can overlap
- the compiler does not force an `await` after every call

## Example

Source:

```sigil
λleft()→ℤ=21
λright()→ℤ=21

λmain()→ℤ=left()+right()
```

Representative generated shape:

```ts
function main() {
  return __sigil_all([left(), right()])
    .then(([__left, __right]) => (__left + __right));
}
```

The important change is not laziness for its own sake. It is that the compiler can start both calls before the `+` operator forces their values.

## Why remove `!Async`?

Because concurrency is no longer a user-visible effect.

`!IO`, `!Network`, `!Error`, and `!Mut` still describe observable behavior. `Async` does not. It is an execution strategy the compiler applies uniformly.

## Why `↦` and `⊳` stay pure

Sigil treats `↦` and `⊳` as canonical pure data-parallel operators.

That gives them simple semantics:

- `xs↦fn` means map a pure function across a list
- `xs⊳pred` means filter with a pure predicate

Ordered accumulation still belongs to `⊕`, because reductions depend on the previous accumulator value.

## Backend honesty

On the JavaScript backend, concurrent-by-default means:

- overlapping async I/O
- overlapping Promise-based work

It does **not** mean automatic CPU parallelism.

That distinction matters. Sigil is making the generated code truly asynchronous, not pretending JavaScript can run CPU-bound pure code on multiple threads by itself.
