# Async Execution in Sigil

## Overview

Sigil keeps one promise-shaped runtime model, but it no longer widens ordinary
expression evaluation into broad implicit overlap.

The current rule is:

- ordinary Sigil expressions compose through one async-capable runtime model
- explicit concurrency is introduced only through named concurrent regions

That gives Sigil one function model without forcing users to write `await`,
while also making batching, rate limits, and stop behavior explicit.

## Ordinary Evaluation

Sigil functions still compile to JavaScript functions that return promise-shaped
values.

What changed is the widening model:

- sibling expressions are no longer treated as an implicit fanout boundary
- list operators no longer lower through broad `Promise.all`
- pure `↦` and `⊳` remain canonical list transforms
- `⊕` remains ordered reduction

So Sigil still hides promise plumbing, but it does not silently turn ordinary
expression structure into unbounded concurrent work.

## Named Concurrent Regions

Explicit concurrency uses one surface:

```sigil module
λisTransportFailure(err:String)=>Bool=false

λmain()=>!IO [ConcurrentOutcome[Int,String]]=concurrent urlAudit({concurrency:5,jitterMs:Some({max:25,min:1}),stopOn:isTransportFailure,windowMs:Some(1000)}){
  spawnEach urls processUrl
}

λprocessUrl(url:String)=>!IO Result[Int,String]=Ok(#url)
```

Region rules:

- regions are named: `concurrent regionName(config){ ... }`
- config is an exact record literal
- config fields are alphabetical:
  - `concurrency`
  - `jitterMs`
  - `stopOn`
  - `windowMs`
- region bodies are spawn-only:
  - `spawn expr`
  - `spawnEach list fn`

## Region Policy

Current config surface:

- `concurrency:Int`
- `jitterMs:Option[{max:Int,min:Int}]`
- `stopOn: λ(E)=>Bool`
- `windowMs:Option[Int]`

Semantics:

- `concurrency` is the maximum number of live child tasks
- `windowMs` means no more than `concurrency` child starts in any `windowMs`
  window
- `jitterMs` adds a randomized start delay per child
- `stopOn` is evaluated on child failures and decides whether the region should
  stop scheduling new work

`stopOn` is explicit because Sigil does not assume one default error policy for
all programs.

## Child Result Shape

Regions return one ordered list of outcomes:

```sigil module
t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)
```

Ordering is stable:

- `spawn` preserves spawn order
- `spawnEach` preserves input order

That means a concurrent batch still has one deterministic result shape, even
when work resolves at different times.

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

Those are properties of an execution region, not properties of `↦` itself.

So Sigil keeps:

- one canonical list-processing surface for value transforms
- one canonical concurrent-region surface for explicit widening

## What This Does Not Mean

Sigil still does not promise:

- automatic CPU parallelism
- general cancellation of arbitrary started tasks
- implicit concurrency everywhere a collection is traversed

The language keeps one async-capable runtime model, but explicit widening now
belongs to named concurrent regions.
