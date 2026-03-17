---
title: Canonical List Processing in Sigil
date: 2026-03-17
author: Sigil Language Team
slug: canonical-list-processing
---

# Canonical List Processing in Sigil

Sigil now rejects a small set of exact recursive list-processing clones when the
language already has one canonical surface.

## The Change

The validator now rejects these exact recursive shapes:

- recursive append-to-result of the form `self(rest)⧺rhs`
- hand-rolled `all` clones
- hand-rolled `any` clones
- filter followed by length of the form `#(xs⊳pred)`
- hand-rolled `map` clones
- hand-rolled `filter` clones
- hand-rolled `find` clones
- hand-rolled `flatMap` clones
- hand-rolled `reverse` clones
- hand-rolled `fold` clones

The required replacements are:

- `stdlib::list.all` for universal checks
- `stdlib::list.any` for existential checks
- `stdlib::list.countIf` for predicate counting
- `↦` for projection
- `⊳` for filtering
- `stdlib::list.find` for first-match search
- `stdlib::list.flatMap` for flattening projection
- `⊕` or `stdlib::list.fold` for reduction
- `stdlib::list.reverse` for reversal

This is not a general optimizer and not a semantic equivalence engine. The
rules are narrow AST-shape checks.

## Why Sigil Is Doing This

These recursive shapes are common in human-written tutorial code and in
LLM-generated code. They also create exactly the kind of style branching Sigil
tries to remove:

- multiple encodings of the same operation
- examples and projects teaching different defaults
- less predictable generated code
- list-building patterns that are often less efficient than the canonical
  replacement

The goal is not to ban recursion. The goal is to collapse common list plumbing
into one obvious path.

## Examples

### All

Rejected:

```sigil
λallPositive(xs:[Int])=>Bool match xs{
  []=>true|
  [x,.rest]=>isPositive(x) and allPositive(rest)
}
```

Required:

```sigil
λallPositive(xs:[Int])=>Bool=stdlib::list.all(isPositive,xs)
```

### Any

Rejected:

```sigil
λanyEven(xs:[Int])=>Bool match xs{
  []=>false|
  [x,.rest]=>isEven(x) or anyEven(rest)
}
```

Required:

```sigil
λanyEven(xs:[Int])=>Bool=stdlib::list.any(isEven,xs)
```

### Map

Rejected:

```sigil
λdouble(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>[x*2]⧺double(rest)
}
```

Required:

```sigil
λdouble(xs:[Int])=>[Int]=xs↦(λ(x:Int)=>Int=x*2)
```

### Count

Rejected:

```sigil
λcountEven(xs:[Int])=>Int=#(xs⊳isEven)
```

Required:

```sigil
λcountEven(xs:[Int])=>Int=stdlib::list.countIf(isEven,xs)
```

### Filter

Rejected:

```sigil
λevens(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>match isEven(x){
    true=>[x]⧺evens(rest)|
    false=>evens(rest)
  }
}
```

Required:

```sigil
λevens(xs:[Int])=>[Int]=xs⊳isEven
```

### Find

Rejected:

```sigil
λfindEven(xs:[Int])=>Option[Int] match xs{
  []=>None()|
  [x,.rest]=>match isEven(x){
    true=>Some(x)|
    false=>findEven(rest)
  }
}
```

Required:

```sigil
λfindEven(xs:[Int])=>Option[Int]=stdlib::list.find(isEven,xs)
```

### FlatMap

Rejected:

```sigil
λexplode(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>digits(x)⧺explode(rest)
}
```

Required:

```sigil
λexplode(xs:[Int])=>[Int]=stdlib::list.flatMap(digits,xs)
```

### Reverse

Rejected:

```sigil
λreverse(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>reverse(rest)⧺[x]
}
```

Required:

```sigil
λreverse(xs:[Int])=>[Int]=stdlib::list.reverse(xs)
```

### Fold

Rejected:

```sigil
λsum(xs:[Int])=>Int match xs{
  []=>0|
  [x,.rest]=>x+sum(rest)
}
```

Required:

```sigil
λsum(xs:[Int])=>Int=xs⊕(λ(acc:Int,x:Int)=>Int=acc+x)⊕0
```

## Performance Angle

The performance argument is not the whole reason for these rules, but it is a
real reason.

The append-to-result shape `self(rest)⧺rhs` is a classic way to build lists by
repeatedly extending the recursive result at the expensive end. The canonical
replacement is usually:

- a built-in list operator with a direct meaning
- or a wrapper plus accumulator helper that builds in one pass and reverses once

So this change aligns two goals:

- fewer equivalent encodings
- better default traversal and result-building shapes

## Why Exact-Shape Rules

Sigil is not trying to prove algorithmic optimality. That would be brittle and
too broad for canonical validation.

Instead, the language now rejects a small set of high-confidence patterns where:

- the intent is obvious
- the canonical replacement is obvious
- the alternative shape is not something Sigil wants in its corpus

That is enough to materially shape examples, projects, and LLM output without
turning the validator into a theorem prover.
