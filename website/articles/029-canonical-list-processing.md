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
- filter followed by length of the form `#(xs filter pred)`
- hand-rolled `map` clones
- hand-rolled `filter` clones
- hand-rolled `find` clones
- hand-rolled `flatMap` clones
- hand-rolled `reverse` clones
- hand-rolled `fold` clones

The required replacements are:

- `§list.all` for universal checks
- `§list.any` for existential checks
- `§list.countIf` for predicate counting
- `map` for projection
- `filter` for filtering
- `§list.find` for first-match search
- `§list.flatMap` for flattening projection
- `reduce ... from ...` or `§list.fold` for reduction
- `§list.reverse` for reversal

This is not a general optimizer and not a semantic equivalence engine. The
rules are narrow AST-shape checks.

A later compiler change extended the same idea to exact top-level wrappers
around canonical `§...` helper surfaces and direct `map` / `filter` /
`reduce ... from ...` wrappers. This article stays focused on recursive list
processing and exact traversal-shape bans.

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

```sigil invalid-module
λallPositive(xs:[Int])=>Bool match xs{
  []=>true|
  [x,.rest]=>isPositive(x) and allPositive(rest)
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-ALL-CLONE",
    "message": "SIGIL-CANON-RECURSION-ALL-CLONE: Recursive function 'allPositive' is a hand-rolled all."
  }
}
```

Required:

```sigil module
λallPositive(xs:[Int])=>Bool=§list.all(
  isPositive,
  xs
)

λisPositive(x:Int)=>Bool=x>0
```

### Any

Rejected:

```sigil invalid-module
λanyEven(xs:[Int])=>Bool match xs{
  []=>false|
  [x,.rest]=>isEven(x) or anyEven(rest)
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-ANY-CLONE",
    "message": "SIGIL-CANON-RECURSION-ANY-CLONE: Recursive function 'anyEven' is a hand-rolled any."
  }
}
```

Required:

```sigil module projects/repoAudit/src/anyEven.lib.sigil
λanyEven(xs:[Int])=>Bool=§list.any(
  isEven,
  xs
)

λisEven(x:Int)=>Bool=x%2=0
```

### Map

Rejected:

```sigil invalid-module
λdouble(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>[x*2]⧺double(rest)
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-MAP-CLONE",
    "message": "SIGIL-CANON-RECURSION-MAP-CLONE: Recursive function 'double' is a hand-rolled map."
  }
}
```

Required:

```sigil module
λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)
```

### Count

Rejected:

```sigil invalid-module
λcountEven(xs:[Int])=>Int=#(xs filter isEven)
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-TRAVERSAL-FILTER-COUNT",
    "message": "SIGIL-CANON-TRAVERSAL-FILTER-COUNT: Expression uses filter then length for counting."
  }
}
```

Required:

```sigil module
λcountEven(xs:[Int])=>Int=§list.countIf(
  isEven,
  xs
)

λisEven(x:Int)=>Bool=x%2=0
```

### Filter

Rejected:

```sigil invalid-module
λevens(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>match isEven(x){
    true=>[x]⧺evens(rest)|
    false=>evens(rest)
  }
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-FILTER-CLONE",
    "message": "SIGIL-CANON-RECURSION-FILTER-CLONE: Recursive function 'evens' is a hand-rolled filter."
  }
}
```

Required:

```sigil module
λevens(xs:[Int])=>[Int]=xs filter isEven

λisEven(x:Int)=>Bool=x%2=0
```

### Find

Rejected:

```sigil invalid-module
λfindEven(xs:[Int])=>Option[Int] match xs{
  []=>None()|
  [x,.rest]=>match isEven(x){
    true=>Some(x)|
    false=>findEven(rest)
  }
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-FIND-CLONE",
    "message": "SIGIL-CANON-RECURSION-FIND-CLONE: Recursive function 'findEven' is a hand-rolled find."
  }
}
```

Required:

```sigil module
λfindEven(xs:[Int])=>Option[Int]=§list.find(
  isEven,
  xs
)

λisEven(x:Int)=>Bool=x%2=0
```

### FlatMap

Rejected:

```sigil invalid-module
λexplode(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>digits(x)⧺explode(rest)
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-FLATMAP-CLONE",
    "message": "SIGIL-CANON-RECURSION-FLATMAP-CLONE: Recursive function 'explode' is a hand-rolled flatMap."
  }
}
```

Required:

```sigil module
λdigits(x:Int)=>[Int]=[x]

λexplode(xs:[Int])=>[Int]=§list.flatMap(
  digits,
  xs
)
```

### Reverse

Rejected:

```sigil invalid-module
λreverse(xs:[Int])=>[Int] match xs{
  []=>[]|
  [x,.rest]=>reverse(rest)⧺[x]
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-REVERSE-CLONE",
    "message": "SIGIL-CANON-RECURSION-REVERSE-CLONE: Recursive function 'reverse' is a hand-rolled reverse."
  }
}
```

Required:

```sigil module
λisPalindrome(xs:[Int])=>Bool=xs=§list.reverse(xs)
```

### Fold

Rejected:

```sigil invalid-module
λsum(xs:[Int])=>Int match xs{
  []=>0|
  [x,.rest]=>x+sum(rest)
}
```

```json
{
  "formatVersion": 1,
  "ok": false,
  "phase": "canonical",
  "error": {
    "code": "SIGIL-CANON-RECURSION-FOLD-CLONE",
    "message": "SIGIL-CANON-RECURSION-FOLD-CLONE: Recursive function 'sum' is a hand-rolled fold."
  }
}
```

Required:

```sigil module
λsum(xs:[Int])=>Int=xs reduce (λ(acc:Int,x:Int)=>Int=acc+x) from 0
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
