---
title: "Why Sigil Uses match"
date: March 2, 2026
author: Sigil Language Team
slug: why-sigil-uses-match
---

# Why Sigil Uses match

**TL;DR:** Sigil replaced `≡` with `match` because `match` is cheaper across real programs, aligns better with Claude Code and Codex priors, and still preserves Sigil's one-way canonical syntax.

## `≡` Was Elegant, But Not The Best Fit

`≡` had a clean mathematical feel.

That was not enough.

Sigil is optimizing for machine production and machine consumption first, not for symbolic elegance in isolation. A pattern-matching keyword should be judged by:

1. token cost in real programs
2. how naturally coding agents produce it
3. whether it preserves one canonical form

`match` won that comparison.

## We Measured The Replacement

The Unicode benchmark rewrites whole `.sigil` files in memory and retokenizes the rewritten corpus. It does not rely on isolated symbol counts.

For `≡ -> match`, the result was:

- `cl100k_base`: `-137`
- local SentencePiece/Llama heuristic proxy: `-53`
- local Anthropic heuristic proxy: `-55`

That is a real whole-corpus win, not just a local glyph trick.

## Why This Matters For Claude Code And Codex

`match` is a strong programming-language prior.

Models see it in Rust, OCaml discussions, compilers, interpreters, and pattern-matching examples everywhere. `≡` is much rarer as executable language syntax.

That means `match` reduces:

- first-draft syntax mistakes
- repair loops
- parser failures
- wasted tokens spent correcting uncommon surface forms

For an AI-first language, that matters more than preserving a mathematically neat symbol.

## Canonical Syntax Is Still Intact

This is not a move toward flexibility.

Before:

- `≡`

Now:

- `match`

Sigil still has exactly one way to write a pattern match. The canonicality principle is preserved. Only the canonical form changed.

## What Is Not Changing

- match semantics are unchanged
- pattern guards are unchanged
- exhaustiveness rules are unchanged
- the AST and typechecker model for match expressions are unchanged

Only the source spelling changes.

## The Broader Rule

Sigil should keep Unicode where Unicode clearly pays for itself.

Sigil should replace Unicode where a common programming term wins on both:

- measured token efficiency
- model-prior alignment

`match` is one of the clearest examples so far.

It is common.
It is semantically exact.
It is cheaper in real programs.

For Sigil, that makes it the right canonical form.
