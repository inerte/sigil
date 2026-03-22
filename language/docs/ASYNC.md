# Async Execution in Sigil

## Overview

Sigil uses one async-capable runtime model and one explicit concurrency surface.

The rule is:

- ordinary Sigil expressions compose through one async-capable runtime model
- explicit concurrency is introduced only through named concurrent regions

That gives Sigil one function model without forcing users to write `await`,
while keeping batching, rate limits, and stop behavior visible in the source.

## Ordinary Evaluation

Sigil functions compile to JavaScript functions that return promise-shaped
values. Ordinary expression structure is not a concurrency surface.

- sibling expressions are not an implicit fanout boundary
- list operators do not lower through broad `Promise.all`
- pure `map` and `filter` remain canonical list transforms
- `reduce ... from ...` remains ordered reduction

So Sigil hides promise plumbing, but it does not treat ordinary expression
structure as permission to widen work.

<h2 id="named-concurrent-regions">Named Concurrent Regions</h2>

Sigil uses one concurrency surface:

```sigil program
i stdlib::time

λisTransportFailure(err:String)=>Bool=err="NETWORK"

λmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@5:{jitterMs:Some({max:25,min:1}),stopOn:isTransportFailure,windowMs:Some(1000)}{
  spawnEach ["alpha","beta"] processUrl
}

λprocessUrl(url:String)=>!Timer Result[Int,String]={
  l _=(stdlib::time.sleepMs(0):Unit);
  Ok(#url)
}
```

Region rules:

- regions are named: `concurrent regionName@width{ ... }`
- width is required after `@`
- width may be a literal, identifier, postfix chain, or a parenthesized expression
- optional policy attaches as `:{...}`
- policy fields are alphabetical when present:
  - `jitterMs`
  - `stopOn`
  - `windowMs`
- region bodies are spawn-only:
  - `spawn expr`
  - `spawnEach list fn`

## Region Policy

The region surface is:

- width after `@`: `Int`
- `jitterMs:Option[{max:Int,min:Int}]`
- `stopOn: λ(E)=>Bool`
- `windowMs:Option[Int]`

Semantics:

- width is the maximum number of live child tasks
- `windowMs` means no more than `width` child starts in any `windowMs`
  window
- `jitterMs` adds a randomized start delay per child
- `stopOn` is evaluated on child failures and decides whether the region should
  stop scheduling new work

Defaults:

- omitted `jitterMs` behaves like `None()`
- omitted `stopOn` never stops early
- omitted `windowMs` behaves like `None()`

Canonical code omits default-valued policy entirely, so the smallest region is:

```sigil program
i stdlib::time

λmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@5{
  spawnEach ["alpha","beta"] processUrl
}

λprocessUrl(url:String)=>!Timer Result[Int,String]={
  l _=(stdlib::time.sleepMs(0):Unit);
  Ok(#url)
}
```

## Child Result Shape

Regions return one ordered list of outcomes:

```sigil module
t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)
```

Ordering is stable:

- `spawn` preserves spawn order
- `spawnEach` preserves input order

That means a concurrent batch has one deterministic result shape even when work
resolves at different times.

## Stop Behavior

Children inside a region return `Result[T,E]`.

The region maps them into `ConcurrentOutcome[T,E]`:

- `Ok(value)` becomes `Success(value)`
- `Err(error)` becomes `Failure(error)`
- stopped or not-yet-finished work becomes `Aborted()`

When `stopOn(error)` returns `true`:

- Sigil stops scheduling new work
- unfinished work becomes `Aborted()` on a best-effort basis

This is intentionally not a claim of universal force-cancellation. The runtime
stops new starts immediately, but already-started work may still settle if the
underlying backend surface cannot be cooperatively cancelled.

## Why Sigil Does It This Way

Real batch workflows need more than width:

- rate windows
- jitter
- best-effort collection
- selective stop on systemic failures

Those are properties of an execution region, not properties of `map` itself.

So Sigil has:

- one canonical list-processing surface for value transforms
- one canonical concurrent-region surface for explicit widening

## What This Does Not Mean

Sigil does not promise:

- automatic CPU parallelism
- general cancellation of arbitrary started tasks
- implicit concurrency everywhere a collection is traversed

The language keeps one async-capable runtime model, and explicit widening
belongs to named concurrent regions.
