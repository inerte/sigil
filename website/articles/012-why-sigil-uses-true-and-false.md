---
title: "Why Sigil Uses true and false"
date: 2026-03-02
author: Sigil Language Team
slug: why-sigil-uses-true-and-false
---

# Why Sigil Uses true and false

**TL;DR:** Sigil replaced `⊤` and `⊥` with `true` and `false` because the ASCII literals are cheaper for LLMs, more aligned with Claude Code and Codex priors, and still preserve Sigil's one-way canonical syntax.

## The Old Choice Was Elegant, But Wrong For Sigil

`⊤` and `⊥` are mathematically neat.

They also lose on the dimensions Sigil actually cares about:

- they cost more tokens
- they are less likely to be produced naturally by coding agents
- they create avoidable parse failures and correction loops

Sigil is not optimizing for human language aesthetics first.
It is optimizing for machine production and machine consumption first.

That means a boolean literal should be judged by:

1. token cost in real programs
2. how naturally models reach for it
3. whether it still preserves one canonical form

`true` and `false` win all three.

## We Measured It, We Did Not Guess

We added an offline Unicode-replacement benchmark that:

- inventories Sigil syntax
- rewrites whole `.sigil` files in memory
- retokenizes the rewritten corpus
- includes separator costs and neighboring token effects

The baseline tokenizer is `cl100k_base`.
We also cross-check with two local heuristic proxies so we do not overfit to one tokenizer family.

For booleans, the result was decisive:

### `⊤ -> true`

- `cl100k_base`: `-219`
- local SentencePiece/Llama heuristic proxy: `-134`
- local Anthropic heuristic proxy: `-134`

### `⊥ -> false`

- `cl100k_base`: `-234`
- local SentencePiece/Llama heuristic proxy: `-130`
- local Anthropic heuristic proxy: `-130`

Those are whole-corpus savings, not isolated symbol trivia.

## Why This Matters For Claude Code And Codex

Most programming languages use `true` and `false`.

That matters because Sigil's primary user is not a human carefully memorizing a bespoke syntax surface. It is Claude Code or Codex generating code from familiar programming priors.

If the model naturally wants to write:

```sigil
done=false
```

but the language requires:

```sigil
done=⊥
```

you have introduced friction for no semantic gain.

That friction shows up as:

- incorrect first drafts
- extra repair passes
- more parser failures
- wasted tokens

## This Does Not Weaken Canonical Syntax

Sigil still has one boolean syntax.

Before:

- `⊤`
- `⊥`

Now:

- `true`
- `false`

This is not adding flexibility.
It is replacing one canonical form with a better canonical form.

That distinction matters.
Sigil is not becoming "accept anything humans like."
It is becoming stricter about using the forms that best serve AI-generated code.

## What Is Not Changing

- the boolean type is still `𝔹`
- boolean semantics do not change
- operators like `¬`, `and`, and `or` are not part of this decision

Only the literal spelling changes.

## The Broader Rule

Sigil should keep Unicode where Unicode pays for itself.

Sigil should replace Unicode where a common programming term wins clearly on:

- token cost
- model prior alignment
- canonical simplicity

`true` and `false` are the clearest case so far.

They are cheaper.
They are more universal.
They are what coding models already want to write.

For an AI-first language, that is the right answer.
