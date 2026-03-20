---
title: Why Sigil Has a Core Prelude
date: 2026-03-04
author: Sigil Language Team
slug: why-sigil-has-a-corePrelude
---

# Why Sigil Has a Core Prelude

The interesting question behind `core` versus `stdlib` is not namespace purity.
It is ownership. Sigil wants each foundational concept to have one canonical
home.

## The Problem

If a concept appears both as implicit vocabulary and as a normal library helper,
or if several modules feel equally responsible for it, then code generation and
ordinary programming both inherit an unnecessary naming decision.

That ambiguity is more important than whether a function call happens to be
qualified with a module prefix.

## The Decision

Sigil uses a small `core::prelude` for concepts that are foundational enough to
shape the whole language surface. Other operational domains remain in `stdlib`
with explicit ownership.

This keeps the distinction narrow:

- foundational language vocabulary goes in core
- operational helpers stay in the standard library

Current examples of implicit core vocabulary include:

- `ConcurrentOutcome[T,E]`
- `Option[T]`
- `Result[T,E]`
- `Aborted`, `Failure`, `Success`
- `Some`, `None`, `Ok`, `Err`

## Why This Matters

The goal is not to maximize or minimize prefixes. The goal is to keep ownership
canonical. A concept should have one obvious place where users, tools, and docs
expect to find it.

That is why the core prelude is small. It is not a convenience dump. It is a
carefully restricted ownership decision.
