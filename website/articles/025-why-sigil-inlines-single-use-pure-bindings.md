---
title: Why Sigil Inlines Single-Use Pure Bindings
date: 2026-03-12
author: Sigil Language Team
slug: why-sigil-inlines-single-use-pure-bindings
---

# Why Sigil Inlines Single-Use Pure Bindings

This is one of those language rules that sounds small until you look at how often
it creates pointless variation.

Take code like this:

```sigil
λformulaText(checksums:Checksums,version:String)→String={
  l repo=(releaseRepo():String);
  src⋅formula.formula({checksums:checksums,repo:repo,version:version})
}
```

There is nothing wrong with it semantically.
But there is also another perfectly valid version:

```sigil
λformulaText(checksums:Checksums,version:String)→String=
  src⋅formula.formula({checksums:checksums,repo:releaseRepo(),version:version})
```

That means the same program has two acceptable surfaces:
- one with a one-shot local name
- one with the pure expression left inline

Sigil does not want both.

## Practical Rule First

If a local binding is:
- pure
- used exactly once
- and can be substituted directly

then Sigil requires the inline form.

So this is rejected:

```sigil
λparagraph(text:String)→String={
  l processed=(processInline(text):String);
  "<p>"++processed++"</p>"
}
```

And this is the required form:

```sigil
λparagraph(text:String)→String=
  "<p>"++processInline(text)++"</p>"
```

The point is not terseness for its own sake.
The point is that Sigil does not allow two equivalent surface programs when the
difference is only a rhetorical local alias.

## What Locals Are Still For

This rule does **not** mean “never use locals.”

Locals are still the right tool when they mark something real:
- the value is reused
- an effect must be sequenced
- a pattern is being destructured
- recursion or syntax requires an explicit staging point

For example, this stays valid:

```sigil
λload(path:String)→!IO String={
  l text=(stdlib⋅file.readText(path):String);
  stdlib⋅string.trim(text)
}
```

`text` is not just rhetorical. The binding is the effect boundary.

And this stays valid too:

```sigil
λrenderTwice(page:Page)→String={
  l rendered=(render(page):String);
  rendered++rendered
}
```

That local exists because the value is reused.

So the practical rule is:
- reuse or sequencing: bind
- one-shot pure intermediate: inline

## Why This Helps Working Code

Without this rule, teams and models drift immediately.

One person writes:

```sigil
l repo=(releaseRepo():String);
...
repo
```

Another writes:

```sigil
releaseRepo()
```

Now your codebase has two styles for one idea, and every future generated file
can pick either one.

Sigil's answer is simple:
- do not leave that choice open

This matters more in an AI-heavy workflow than in a human-only one. Models learn
from examples, imitate local patterns, and happily alternate between equivalent
forms if the language allows both.

If the compiler picks one, the repo stays tighter.

## The PL Version

In more formal terms, this rule eliminates one class of **administrative let**
variation.

The language already knows:
- whether a binding is pure
- how many times the bound name is used
- whether direct substitution preserves the source form legally

So Sigil uses that information to normalize source programs toward one canonical
surface.

This is not an optimizer and it is not a style lint. It is canonical validation.

The relevant distinction is:
- **semantic necessity**
- versus **surface-level administrative variation**

If a local binding only introduces a one-use pure alias, then it is just another
way of spelling the same program. Sigil rejects that second spelling.

## Why Sigil Does This

Sigil already enforces:
- declaration ordering
- exact records
- naming conventions
- no shadowing
- topology-backed runtime dependencies

Rejecting one-use pure aliases is the same idea applied one level deeper.

The rule is intentionally mechanical.
The compiler does **not** try to decide whether the name “helps readability.”
That would just reintroduce taste and ambiguity.

Instead, Sigil asks:
- is it pure?
- is it used once?
- can it be inlined safely?

If yes, then there is only one canonical way to write it.

That is the whole point.
