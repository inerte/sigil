---
title: Why Sigil Inlines Single-Use Pure Bindings
date: 2026-03-12
author: Sigil Language Team
slug: why-sigil-inlines-single-use-pure-bindings
---

# Why Sigil Inlines Single-Use Pure Bindings

Sigil rejects single-use pure locals when they can be substituted directly. The
rule is small, but it removes a common form of stylistic variation.

## The Problem

Consider these two forms:

```sigil invalid-expr
λformulaText(repo:String,version:String)=>String={
  l text=(repo++":"++version:String);
  "["++text++"]"
}
```

```sigil exprs
λformulaText(repo:String,version:String)=>String="["
  ++repo
  ++":"
  ++version
  ++"]"
```

They mean the same thing. The only difference is whether the program introduces
an administrative local name for a pure expression that is used once.

Sigil does not want both surface forms.

## The Rule

If a local binding is:

- pure
- used exactly once
- safe to substitute directly

then Sigil requires the inline form.

Bindings are still valid when they represent something real, such as:

- reuse
- sequencing of effects
- destructuring
- a syntactic staging point that the language actually needs

## Why This Fits Sigil

This rule removes a stylistic choice that otherwise spreads quickly through a
codebase. Some authors introduce one-shot locals to name intermediate values;
others inline them. Once both forms are legal, both forms appear everywhere.

Sigil prefers to settle that choice in the language. If the local name is not
carrying real semantic weight, the source should not keep it.
