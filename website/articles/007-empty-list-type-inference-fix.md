---
title: Empty List Type Inference in Pattern Matching
date: 2026-02-25
author: Sigil Language Team
slug: 007-empty-list-type-inference-fix
tags: [compiler, type-system, pattern-matching]
---

# Empty List Type Inference in Pattern Matching

Empty lists are an ordinary case in recursive code, but they expose a familiar
type inference problem: `[]` by itself does not carry enough information to tell
the checker what element type it should have. In Sigil, that became visible
inside match arms.

## The Problem

Consider a function like this:

```sigil exprs
λtail(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.xs]=>xs
}
```

The return type is clear to a human reader, but the empty list arm has no local
annotation of its own. If the checker tries to synthesize the type of `[]`
without surrounding context, it has too little information.

## The Fix

The solution did not require new syntax. It used Sigil's existing bidirectional
typing structure more carefully.

When checking a multi-arm match:

1. the checker synthesizes an expected result type from an arm that contains
   enough information
2. later arms are checked against that type rather than forced to synthesize
   independently

That means the empty-list arm can be validated against the surrounding return
type instead of pretending it has enough local information on its own.

## Why This Approach Matters

This fix is a good example of Sigil's type-system preference. The language wants
explicit, predictable typing behavior, but it also wants ordinary code to read
cleanly. Adding new syntax for empty-list annotations inside every match would
have solved the immediate problem, but it would have pushed complexity into the
surface language.

Using the existing bidirectional checker preserved the simpler source form while
keeping the inference rule local and understandable.

## Result

Pattern matches that return empty lists now type-check correctly when the
surrounding structure determines the element type. The change is small, but it
removes a rough edge in one of the most common recursive shapes in the language.
