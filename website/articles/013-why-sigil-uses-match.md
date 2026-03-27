---
title: Why Sigil Uses match
date: 2026-03-02
author: Sigil Language Team
slug: why-sigil-uses-match
---

# Why Sigil Uses match

> Update (2026-03-26): `match` is now Sigil's one branching surface,
> including boolean and tuple branching. The compiler also rejects
> non-exhaustive matches and dead arms. See
> [033-exhaustive-match-and-proof-rich-compile-errors](./033-exhaustive-match-and-proof-rich-compile-errors.md).

Sigil previously used a symbolic form for pattern matching. It now uses the
keyword `match`.

## The Decision

The change was driven by the same criteria behind several other syntax updates:

- token cost in real programs
- compatibility with the priors of coding agents
- preservation of one canonical surface form

The older symbolic form was compact, but `match` performed better when measured
in surrounding code and was easier to generate correctly in ordinary editing and
tooling contexts.

## Why This Fits Sigil

Pattern matching is too central a feature to optimize mainly for visual novelty.
It needs to be easy to recognize, easy to tokenize, and easy to generate
consistently.

Using `match` makes the construct more legible to both humans and tools without
reintroducing multiple spellings for the same concept.
