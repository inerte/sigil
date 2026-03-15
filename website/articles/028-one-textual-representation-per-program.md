---
title: One Textual Representation Per Program
date: 2026-03-15
author: Sigil Language Team
slug: one-textual-representation-per-program
---

# One Textual Representation Per Program

Sigil has always cared about canonicality.

But there are two very different levels of canonicality:

- "there is one preferred style"
- "there is one accepted textual representation"

Sigil now chooses the second one.

## The Shift

Canonicality is now printer-first.

The compiler:

1. tokenizes the source
2. parses it into an AST
3. prints the canonical source for that AST internally
4. rejects the file unless the original bytes match that printed form exactly

That means canonicality is no longer mostly a checklist of spacing and layout
rules in the validator.

It is now a source normal form.

## What This Means In Practice

If two files parse to the same Sigil AST, only one of them is accepted.

Not "one is preferred."
Not "one is what the formatter would produce."

One is the language.

So:

- `sigil compile` rejects non-canonical source
- `sigil run` rejects non-canonical source
- `sigil test` rejects non-canonical source

There is no public `sigil format`.

If the text is wrong, the program does not compile.

## Why No Formatter

Because a public formatter would weaken the core claim.

If the language says "many textual forms are valid, but one tool can rewrite
them later," then Sigil still has multiple accepted source forms.

That is not what we want.

We want:

- one AST
- one text form
- one way for models to emit correct code

The compiler error is the enforcement point.

## Why This Matters For AI

LLMs do better when the language removes stylistic branching.

The important property is not just that Sigil is explicit.
It is that Sigil is predictable.

An agent should be able to learn:

- if I know the program structure, I know the exact text to emit
- there is no alternate wrapping style
- there is no alternate spacing style
- there is no "also valid" surface form

That is stronger than "use the formatter."

It turns canonicality into part of generation itself.

## What About Tokens

Dense inline syntax often saves more tokens.

We measured that while designing this change.

But token count is not the first goal.

The first goal is AI friendliness through one obvious textual shape.
The second goal is token efficiency.

So Sigil intentionally chooses structured multiline forms earlier when the
program starts branching or staging intermediate work.

That trades a small amount of token density for a much stronger generation
invariant:

> one program, one text form

## The Real Principle

Sigil is not trying to be a language with a style guide.

Sigil is trying to be a language where source itself has a canonical normal
form.

That is the next level of canonicality.

Not "formatting matters."

But:

**there is only one accepted way to write the program at all.**
