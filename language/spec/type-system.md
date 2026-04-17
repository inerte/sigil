# Sigil Type System Specification

Version: 1.0.0
Last Updated: 2026-04-05

## Overview

Sigil currently uses bidirectional type checking with:

- algebraic data types
- exact record types
- map types
- explicit top-level parametric polymorphism
- solver-backed type refinements
- nominal type labels
- function contracts
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

## Type Ascription

Sigil defines one expression-level type-ascription form:

```sigil expr
(expr:Type)
```

Examples:

```sigil module
c airAccel=(1:Int)
```

```sigil program
λmain()=>Int={
  l speed=(1:Int);
  speed+speed
}
```

Current Sigil does not define separate declaration-level annotation surfaces
such as `c name:Type=value`, and it does not define a bare postfix expression
surface such as `expr:Type`.

This is intentional. The language keeps one canonical ascription rule:

- if a type is being ascribed to an expression, the source form is `(expr:Type)`
- the same form is used in constant values, local bindings, and ordinary
  subexpressions

The purpose is canonical simplicity rather than minimizing parentheses.

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
- `Owned[T]`
- named ADTs and aliases

`Owned[T]` is the compiler-known resource wrapper type.

Current ownership rules:

- stdlib resource creators and typed extern subscriptions may return `Owned[T]`
- `Owned[T]` values are intended to be consumed by `using`
- borrowed resource values introduced by `using` must not escape the scope
- `Owned[T]` is affine rather than freely duplicable; ordinary `l` bindings of owned values are rejected, and owned values are rejected inside ordinary list/record/map literals

## Feature Flag Types

First-class `featureFlag` declarations currently allow:

- `Bool`
- named sum types

Example:

```sigil module
t CheckoutColor=Citrus()|Control()|Ocean()

featureFlag NewCheckout:Bool
  createdAt "2026-04-12T14-00-00Z"
  default false

featureFlag CheckoutColorChoice:CheckoutColor
  createdAt "2026-04-12T14-00-00Z"
  default Control()
```

Rules:

- the declared `default` must check against the flag type
- the default expression must be pure
- feature-flag declarations synthesize typed descriptor values usable through
  `§featureFlags`

## Project-Defined Named Types

In projects with `sigil.json`, project-defined named types live in
`src/types.lib.sigil` and are referenced elsewhere with `µ...`.

Example:

```sigil module projects/todo-app/src/types.lib.sigil
t BirthYear=Int where value>1800 and value<10000

t User={
  birthYear:BirthYear,
  name:String
}
```

```sigil module projects/todo-app/src/todoDomain.lib.sigil
λtodoId(todo:µTodo)=>Int=todo.id
```

`src/types.lib.sigil` owns `t`, `label`, and `label ... combines ...`
declarations. Type definitions and constraints may reference only `§...` and
`¶...`.

## Labelled Types

Sigil has a separate type-classification layer in addition to `where`
refinements.

Example:

```sigil module projects/labelled-boundaries/src/types.lib.sigil
label Brazil

label Credential

label GovAuth

label Pii

label Usa

t Cpf=String label [Brazil,Pii]

t GovBrToken=String label [Brazil,Credential,GovAuth]

t Ssn=String label [Pii,Usa]
```

Label rules:

- labels are nominal classifications rather than value predicates
- `label X combines Y` contributes transitive implied labels during checking
- labelled values keep those labels through aggregate construction
- field projection returns the projected field's labels
- labels participate in named-boundary checking, not ordinary local computation
- unlabeled values are unaffected by boundary-rule enforcement

Projects pair labelled types with `src/policies.lib.sigil`, which owns
`rule` and `transform` declarations for named topology boundaries.

Topology-aware labelled-boundary tests run under `sigil test --env <name>` and
assert the resulting boundary behavior with named-boundary helpers such as
`※check::file.existsAt`, `※check::log.containsAt`, and
`※observe::process.commandsAt`.

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

## Constrained Types

Named user-defined types may carry a pure `where` clause:

```sigil module
t BirthYear=Int where value>1800 and value<10000

t DateRange={
  end:Int,
  start:Int
} where value.end≥value.start
```

Constraint rules:

- only `value` is in scope
- the constraint must typecheck to `Bool`
- constraints are pure and world-independent
- constrained aliases and constrained named product types act as compile-time refinements over their underlying type
- values flow into a constrained type only when the checker can prove the predicate in Sigil's canonical solver-backed refinement fragment
- constrained values widen to their underlying type automatically
- the current proof fragment covers Bool/Int literals, `value`, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, and `not`
- `match`, exact record patterns, and internal branching propagate supported branch facts into refinement checking
- direct boolean local aliases of supported facts also narrow
- constraints do not imply automatic runtime validation

Example:

```sigil module
t BirthYear=Int where value>1800

λpromote(year:Int)=>BirthYear match year>1800{
  true=>year|
  false=>1900
}
```

Runnable examples:

- `language/examples/functionContracts.sigil`
- `language/examples/proofMeasures.sigil`

## Function Contracts

Functions may carry pure compile-time contracts:

```sigil module
t BirthYear=Int where value>1800

λnormalizeYear(raw:Int)=>Int
ensures result>1800
match raw>1800{
  true=>raw|
  false=>1900
}

λpositiveGap(baseline:Int,current:BirthYear)=>Int
requires current≥baseline
ensures result≥0
=current-baseline
```

Contract rules:

- `requires` is checked at call sites
- `ensures` is checked against the function body and then contributes facts back to callers
- a function may declare at most one `requires` clause and at most one `ensures` clause
- `requires` may reference only parameters
- `ensures` may reference parameters plus `result`
- both clauses must typecheck to `Bool`
- both clauses are pure and world-independent
- effectful functions may carry contracts, but those contracts describe only parameter obligations and returned-value guarantees
- contracts use the same solver-backed proof fragment as constrained types and narrowing
- contracts do not imply automatic runtime checks

Dogfooded project usage now lives in:

- `projects/todo-app/src/todoDomain.lib.sigil`
- `projects/flashcards/src/flashcardsDomain.lib.sigil`
- `projects/algorithms/src/fibonacciSearch.lib.sigil`
- `projects/game-2048/src/game2048.lib.sigil`
- `projects/minesweep/src/minesweepDomain.lib.sigil`
- `projects/roguelike/src/roguelike.lib.sigil`

## Structural Equality

Type equality normalizes unconstrained aliases and unconstrained named product
types before comparison.

That means:

- unconstrained aliases compare structurally
- unconstrained named product types compare structurally after normalization
- constrained aliases and named product types use refinement checking over their underlying type instead of plain structural equality
- sum types remain nominal

This is a checker invariant, not inference.

## Effects

Sigil supports explicit effect annotations in function and test signatures.

Sigil ships with primitive effects:

- `Clock`
- `Fs`
- `FsWatch`
- `Http`
- `Log`
- `Process`
- `Pty`
- `Random`
- `Stream`
- `Tcp`
- `Terminal`
- `Timer`
- `WebSocket`

Projects may define reusable multi-effect aliases only in `src/effects.lib.sigil`.
Aliases must expand to at least two primitive effects.

Example:

```sigil module projects/repoAudit/src/effects.lib.sigil
effect CliIo=!Fs!Log!Process
```

Examples:

```sigil program
e axios:{get:λ(String)=>!Http String}

e console:{log:λ(String)=>!Log Unit}

λfetch()=>!Http String=axios.get("https://example.com")

λmain()=>!Http!Log Unit={
  l _=(fetch():String);
  console.log("hello")
}
```

Effects are explicit surface syntax. The checker tracks them as part of the
typed program and rejects callees or bodies whose required effects are not
covered by the enclosing signature.

## Canonical Typed Rules

Some canonical rules depend on typing information.

Current important example:

- a pure local binding used exactly once is rejected and must be inlined
- a wildcard sequencing binding must not discard a pure expression

This rule is applied after type checking by the canonical validator.

## What This Spec Does Not Claim

This document intentionally does not specify:

- ownership semantics
- borrow rules
- inferred lifetimes
- Algorithm W

Those are not part of the implemented Sigil type system in this repository.
