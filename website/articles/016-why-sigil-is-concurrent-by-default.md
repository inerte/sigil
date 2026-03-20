---
title: Why Sigil Is Concurrent by Default
date: 2026-03-03
author: Sigil Language Team
slug: why-sigil-is-concurrentByDefault
---

# Why Sigil Is Concurrent by Default

This article described an earlier direction for Sigil's runtime model.

It is now superseded.

Sigil no longer uses broad implicit fanout as the concurrency story. The current
model keeps one promise-shaped runtime but introduces widening only through
named concurrent regions with explicit policy.

See:

- `/articles/named-concurrent-regions`

The current language docs and specs are the source of truth:

- `language/docs/ASYNC.md`
- `language/docs/syntax-reference.md`
- `language/spec/semantics.md`
