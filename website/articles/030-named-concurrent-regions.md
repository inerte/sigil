---
title: Named Concurrent Regions in Sigil
date: 2026-03-19
author: Sigil Language Team
slug: named-concurrent-regions
---

# Named Concurrent Regions in Sigil

Sigil now uses explicit named concurrent regions for batch concurrency.

This replaces the older broad default-fanout direction. The language still
keeps one promise-shaped runtime model, but widening work is now explicit,
bounded, and policy-driven.

## The Problem

Broad implicit overlap sounds attractive until real programs need actual
engineering controls:

- concurrency width
- rate windows
- jitter
- selective stop on systemic failures
- stable ordered results

Those are not properties of `↦`. They are properties of an execution region.

## The Surface

Sigil now has one canonical concurrency surface:

```sigil module
λisSystemic(err:String)=>Bool=err="NETWORK"

λprocessUrl(url:String)=>!IO Result[Int,String]=Ok(#url)

λrun(urls:[String])=>!IO [ConcurrentOutcome[Int,String]]=concurrent urlAudit({concurrency:5,jitterMs:Some({max:25,min:1}),stopOn:isSystemic,windowMs:Some(1000)}){
  spawnEach urls processUrl
}
```

Important points:

- the region is named
- the config is an exact record literal
- config fields are alphabetical
- the body is spawn-only

Current config fields are:

- `concurrency`
- `jitterMs`
- `stopOn`
- `windowMs`

## Why It Is a Region

The policy belongs to the region because the work inside it is not always one
operator application.

A real batch often mixes:

- list traversal
- HTTP calls
- file writes
- nested bounded work

Attaching rate limits and jitter to `map` itself would be the wrong abstraction.
Those concerns belong to the execution boundary that owns the batch.

## Result Shape

Regions return one ordered list of outcomes:

```sigil module
t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)
```

That gives the caller one deterministic post-batch value:

- `Success(value)` when a child returns `Ok(value)`
- `Failure(error)` when a child returns `Err(error)`
- `Aborted()` when work was stopped before completion

Order is stable:

- `spawn` order for explicit child spawns
- input order for `spawnEach`

## Error Policy

Sigil does not hard-code one default stop policy for all programs.

Instead, the region takes an explicit predicate:

```sigil module
λshouldStop(err:String)=>Bool=err="NETWORK"
```

That means a batch can keep going through local failures while still stopping on
systemic ones.

For example:

- an HTTP `404` may be modeled as an ordinary successful response
- a transport failure may be modeled as `Err(HttpError)` and trigger `stopOn`

This is the right split. The child computation classifies the domain result, and
the region decides whether that failure should stop new starts.

## List Operators Stay Canonical

This change does not introduce a second family of `map` helpers.

Sigil still keeps:

- `↦` for pure projection
- `⊳` for pure filtering
- `⊕` for ordered reduction

Those operators are canonical value transforms, not the concurrency surface.

## What Changed in the Backend

Two things changed together:

1. ordinary expression composition no longer lowers through broad implicit
   overlap
2. named concurrent regions now own explicit bounded scheduling

That gives the runtime one clearer story:

- normal code is promise-shaped, but not silently widened
- concurrent batching happens only inside `concurrent name(config){...}`

## Why This Is Better

It gives Sigil one explicit place for:

- bounded concurrency
- per-window start limits
- jitter
- stop behavior
- ordered outcomes

And it removes a lot of hidden behavior from ordinary expressions.

That is a better fit for Sigil's goals:

- one canonical surface
- visible cost shape
- deterministic result ordering
- less room for accidental fanout in generated code
