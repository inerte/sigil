---
title: Sigil Ships Embedded Docs to Solve the LLM Cold-Start Problem
date: 2026-04-15
author: Sigil Language Team
slug: sigil-ships-embedded-docs-for-llm-cold-starts
---

# Sigil Ships Embedded Docs to Solve the LLM Cold-Start Problem

Sigil is new enough that an installed AI assistant should not be assumed to
know the language.

That is a different problem from code generation quality inside an established
language. JavaScript, Python, and TypeScript already exist in model weights,
search indexes, IDE plugins, blog posts, and public examples. Sigil does not.

So a user who downloads `sigil` on April 15, 2026 needs a different bootstrap
story:

- the binary they installed should be able to teach the language
- the assistant should not need to guess from sparse web results
- the docs it reads should match the version of `sigil` on disk

That is why Sigil now ships an embedded local docs corpus inside the CLI.

## The Problem Is Language Cold-Start, Not Just Command Discovery

`sigil help` is enough to discover commands.

It is not enough to teach a model:

- Sigil syntax
- stdlib ownership
- package rules
- topology/config conventions
- first-party design rationale

An assistant can read a source file once it exists. That does not mean it knows
the language well enough to create or edit the file correctly.

This is the real cold-start or bootstrap problem for a new language: the model
does not yet have trustworthy priors.

## Why Web Search Is The Wrong Default

The obvious fallback is “just search the web.”

That is weaker than it sounds for a brand-new language:

- the website may not rank yet
- mirrors and summaries may drift from the current release
- older articles may describe earlier surface forms
- repeated web lookups are slower and more expensive than local retrieval

The important point is not that web docs are bad. It is that the installed
binary is the most authoritative thing the user has at that moment.

If the binary can expose its own language knowledge directly, that should be
the first retrieval surface.

## What Sigil Ships Instead

The `sigil` binary now embeds a local corpus built from:

- guides
- language docs
- formal specs
- the grammar sketch
- design articles

That corpus is exposed through a new top-level command family:

```bash
sigil docs list
sigil docs search "feature flags"
sigil docs show docs/syntax-reference --start-line 350 --end-line 390
sigil docs context --list
sigil docs context packages
```

The discovery loop stays simple:

```bash
sigil help
sigil docs context --list
sigil docs search "syntax reference"
```

The help text is still plain CLI help. The retrieval commands return JSON by
default so Claude Code, Codex, and similar tools can treat them as a normal
machine interface.

## Why Embed The Corpus In The Binary

The main benefit is version matching.

If a user installs one specific Sigil release, the local docs surface should
describe that exact release. There should be no guesswork about whether the
assistant is reading:

- an older article
- a cached website page
- a partial mirror
- a different branch of the repo

Embedding the corpus also avoids a second installation step. The user does not
need to install docs separately or configure a local server. The binary is the
tool and the local reference at the same time.

For a language that is explicitly designed around AI-assisted workflows, that is
the right default.

## `context`, `search`, And `show` Solve Different Retrieval Problems

The commands are deliberately small and composable.

`context` is the broad-start surface. It answers “which documents should I read
to understand packages, topology, testing, or the type system?”

`search` is the exact lookup surface. It answers “which document and line talks
about feature flags, records, or rooted modules?”

`show` is the precise reading surface. It returns one document, optionally
restricted to a line range, so an assistant can search first and then open only
the chunk it needs.

That is a better fit for tool-driven reasoning than putting a chat model inside
the CLI itself.

## This Still Matters After Models Learn Sigil

Embedded docs are not just a temporary crutch until future models absorb the
language.

Even in that future, the local surface is still useful:

- it is cheaper than repeated web search
- it is faster than repeated web search
- it is guaranteed to match the installed version
- it gives one first-party retrieval surface for humans and tools

The web remains useful for distribution and reading. It just should not be the
first dependency for language bootstrap.
