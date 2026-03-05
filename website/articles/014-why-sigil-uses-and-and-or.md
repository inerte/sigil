---
title: "Why Sigil Uses and and or"
date: 2026-03-02
author: Sigil Language Team
slug: why-sigil-uses-and-and-or
---

# Why Sigil Uses and and or

**TL;DR:** Sigil replaced `∧` and `∨` with `and` and `or` because the word operators are cheaper across real programs, align better with Claude Code and Codex priors, and still preserve Sigil's one-way canonical syntax.

## `∧` And `∨` Were Compact, But That Was Not Enough

The symbolic operators had obvious appeal.

They were short.
They looked mathematically clean.
They fit the early Sigil aesthetic.

That still was not the right standard.

Sigil is an AI-first language. Its syntax should optimize for:

1. token cost in real programs
2. how naturally coding agents produce the syntax
3. whether there is still exactly one canonical surface form

`and` and `or` won that comparison.

## We Measured The Replacement

The Unicode benchmark does not compare isolated glyphs and stop there.

It rewrites whole `.sigil` files in memory and retokenizes the rewritten corpus, so separator and boundary costs are included in the result.

For `∧ -> and`, the whole-corpus result was:

- `cl100k_base`: `-14`
- local SentencePiece/Llama heuristic proxy: `-23`
- local Anthropic heuristic proxy: `-23`

For `∨ -> or`, the result was:

- `cl100k_base`: `-9`
- local SentencePiece/Llama heuristic proxy: `-7`
- local Anthropic heuristic proxy: `-7`

These are not giant numbers, but they are clean wins in the same direction across all measured tokenizers.

## Why This Matters For Claude Code And Codex

`and` and `or` are much stronger programming-language priors than `∧` and `∨`.

Agents see them constantly:

- Python
- Ruby
- shell-like logic in prompts
- pseudocode
- tutorial material
- documentation examples

That lowers the chance that a model has to stop, repair, or relearn the surface syntax just to express boolean logic.

For Sigil, that matters more than preserving a mathematically elegant symbol.

## Canonical Syntax Is Still Preserved

This is not a move toward optional style.

Before:

- `∧`
- `∨`

Now:

- `and`
- `or`

Sigil still has exactly one way to write conjunction and disjunction. The canonicality principle stays intact. Only the canonical forms changed.

## What Is Not Changing

- boolean semantics are unchanged
- precedence and associativity are unchanged
- the typechecker still requires boolean operands
- the AST and codegen meaning for logical conjunction/disjunction are unchanged

Only the source spelling changed.

## The Broader Rule

Sigil should keep Unicode when Unicode clearly earns its place.

Sigil should replace Unicode when a common programming term wins on both:

- measured token efficiency
- model-prior alignment

`and` and `or` are good examples of that rule.

They are common.
They are semantically exact.
They are cheaper in real Sigil programs.

For an AI-first language, that makes them the right canonical form.
