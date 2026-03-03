# Concurrent-by-Default in Sigil

## Philosophy

Sigil uses a **uniform async runtime model**, but it is no longer "await everywhere".

The compiler starts independent work early, keeps results promise-shaped while they flow through the program, and only joins them at **strict demand points**.

That gives Sigil one function model without forcing users to write or manage async syntax.

## Core Rules

### 1. Functions return promise-shaped values

Sigil functions compile to JavaScript functions that return values through Promise composition.
They do not eagerly `await` every call.

### 2. Independent work starts early

If two subexpressions do not depend on each other, the compiler may start both before either result is consumed.

### 3. Strict constructs join what they need

The compiler forces values only when a construct needs a concrete result now.

Strict demand points include:
- `if` conditions
- `match` scrutinees and guards
- arithmetic and comparison operators
- field access and indexing
- final observable runner/test results

### 4. Effects start in source order

Effectful operations are still initiated left-to-right.
Sigil may overlap their resolution, but it does not silently reorder effect start.

## Example

```sigil
λleft()→ℤ=21
λright()→ℤ=21

λmain()→ℤ=left()+right()
```

The important property is:
- `left()` and `right()` do not need to be eagerly joined at each call site
- the compiler can start both computations
- the `+` operator is the point that joins them

Representative generated shape:

```ts
function main() {
  return __sigil_all([left(), right()])
    .then(([__left, __right]) => (__left + __right));
}
```

## FFI Behavior

Promise-returning FFI calls are started automatically.
They are joined only when a strict consumer needs their values.

That means Sigil keeps one interop model:
- no separate async syntax
- no sync/async API split
- no manual Promise plumbing in user code

## List Operators

Sigil treats list operators as canonical execution forms:

- `↦` is a pure data-parallel map
- `⊳` is a pure data-parallel filter
- `⊕` is an ordered reduction

So:
- callbacks passed to `↦` must be pure
- predicates passed to `⊳` must be pure
- reducers passed to `⊕` remain ordered because each step depends on the previous accumulator

## What This Does Not Mean

On the JavaScript backend, "concurrent by default" means:
- async I/O can overlap
- independent Promise-based work can overlap

It does **not** mean:
- CPU-bound work runs on multiple threads automatically

## Removed Surface

Sigil no longer uses `!Async` as an effect annotation.
Concurrency is part of the execution model, not a user-visible effect.
