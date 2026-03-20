# Sigil Type System Specification

Version: 1.0.0
Last Updated: 2026-03-14

## Overview

Sigil currently uses bidirectional type checking with:

- algebraic data types
- exact record types
- map types
- explicit top-level parametric polymorphism
- effect annotations

Current Sigil does not implement:

- borrow checking
- ownership/lifetimes
- Hindley-Milner let-polymorphism

This spec describes the implemented checker in the current repository.

## Bidirectional Checking

Sigil alternates between:

1. synthesis: infer a type from expression structure
2. checking: verify an expression against an expected type

This matches Sigil’s explicit surface:

- function parameter types are required
- function return types are required
- lambda parameter types are required
- lambda return types are required

## Local Bindings

Local `l` bindings are monomorphic.

Sigil does not perform Hindley-Milner let-generalization for locals.
Polymorphism comes from explicitly generic top-level declarations.

## Explicit Top-Level Polymorphism

Generic declarations are allowed at top level:

```sigil decl generic
λidentity[T](x:T)=>T=x
λmapOption[T,U](fn:λ(T)=>U,opt:Option[T])=>Option[U]
```

Generic instantiation is driven by ordinary bidirectional typing.

Current Sigil does not include:

- generic lambdas
- explicit call-site type arguments like `f[Int](x)`

## Core Type Forms

Primitive types:

- `Int`
- `Float`
- `Bool`
- `String`
- `Char`
- `Unit`
- `Never`

Constructed types:

- `[T]`
- `{K↦V}`
- `λ(T1,T2,...)=>R`
- named ADTs and aliases

## Algebraic Data Types

Examples:

```sigil module
t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)

t Option[T]=Some(T)|None()

t Result[T,E]=Ok(T)|Err(E)

t Color=Red()|Green()|Blue()
```

Imported constructors use fully qualified module syntax in expressions and
patterns.

## Records and Maps

Records and maps are distinct:

- records are exact fixed-shape products using `:`
- maps are dynamic keyed collections using `↦`

Sigil currently has:

- no row polymorphism
- no width subtyping for records
- no open records

If a field may be absent, the canonical representation is `Option[T]` inside an
exact record.

## Structural Equality

Type equality normalizes aliases and named product types before comparison.

That means:

- aliases compare structurally
- named product types compare structurally after normalization
- sum types remain nominal

This is a checker invariant, not inference.

## Effects

Sigil supports explicit effect annotations in function and test signatures.

Examples:

```sigil program
e axios:{get:λ(String)=>!Network String}

e console

λfetch()=>!Network String=axios.get("https://example.com")

λmain()=>!IO Unit=console.log("hello")
```

Effects are explicit surface syntax. The checker tracks them as part of the
typed program.

## Canonical Typed Rules

Some canonical rules depend on typing information.

Current important example:

- a pure local binding used exactly once is rejected and must be inlined

This rule is applied after type checking by the canonical validator.

## What This Spec Does Not Claim

This document intentionally does not specify:

- ownership semantics
- borrow rules
- inferred lifetimes
- Algorithm W

Those are not part of the implemented Sigil type system in this repository.
