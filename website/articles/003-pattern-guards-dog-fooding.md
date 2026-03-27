---
title: Pattern Guards and Sigil's Website Parser
date: 2026-02-24
author: Sigil Language Team
slug: 003-patternGuards-dog-fooding
---

# Pattern Guards and Sigil's Website Parser

> Update (2026-03-26): guards now participate in exhaustiveness and dead-arm
> checking only through a small explicit proof fragment. Unsupported guard
> facts remain valid source, but they are opaque to coverage proofs. See
> [033-exhaustive-match-and-proof-rich-compile-errors](./033-exhaustive-match-and-proof-rich-compile-errors.md).

Pattern guards came out of a concrete implementation problem rather than an
abstract feature wishlist. While building Sigil's website tooling in Sigil, we
ran into parser code that was structurally a good fit for pattern matching but
still needed boolean conditions on the bound values. The language had no clean
way to express that combination.

## The Problem

The markdown parser behaved like a small state machine. Some branches depended
on structure alone, but many depended on both structure and an additional
predicate. Without guards, that pushed the code toward nested `match`
expressions and repeated boolean branching.

The result was valid, but harder to read than it needed to be. The structure of
the parser was visible, yet the actual decision logic ended up buried inside
secondary matches.

## The Decision

Sigil added `when` guards on match arms:

```text
match value{
  pattern when condition=>result
}
```

This keeps the structural part of the decision and the extra boolean condition
in the same place. It also fits naturally with the rest of Sigil's pattern
matching model: bindings come from the pattern first, then the guard is checked,
and if the guard is false the next arm is tried.

## Why Guards Were the Right Feature

There were other ways to address the readability problem:

- allow deeper nested matches and accept the noise
- use explicit boolean conditionals inside each arm body
- duplicate patterns across cases with different predicates

None of those were good language-level answers. The parser code was not doing
anything unusual. It was expressing a common combination of structural matching
and predicate refinement. Once that need showed up in a real stdlib-scale parser
implementation, the missing feature was hard to justify.

## Implementation Shape

Adding guards touched the expected parts of the compiler:

- the lexer gained the `when` keyword
- match-arm AST nodes gained an optional guard expression
- the parser learned to read the guard between pattern and `=>`
- the type checker enforced that guards have type `Bool`
- code generation emitted conditional fallthrough inside the match arm logic

The important part was not just parsing the syntax. Guards had to be checked in
the environment extended by the pattern bindings so that the guard could refer
to the names introduced by the pattern itself.

## What This Changed

The feature made parser and state-machine code noticeably clearer, but it also
served as a useful dog-fooding result. Building real tools in Sigil keeps
surfacing where the language is missing an honest feature and where the problem
is only stylistic discomfort. Pattern guards fell into the first category.

That is the kind of language change Sigil should keep making: features added in
response to concrete structural pressure, not just borrowed because they are
common elsewhere.
