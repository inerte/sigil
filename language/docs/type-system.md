# Sigil Type System

Sigil uses bidirectional type checking with explicit function, lambda, and
effect annotations.

This page describes the current implemented system, not older design ideas.

## Current Model

Implemented today:

- bidirectional type checking
- explicit top-level parametric polymorphism
- algebraic data types
- exact records
- map types
- solver-backed type refinements
- nominal type labels
- function contracts
- explicit effect annotations

Not implemented today:

- Hindley-Milner let-polymorphism
- generic lambdas
- call-site type arguments like `f[Int](x)`
- borrow checking
- ownership/lifetimes

## Explicit Types

Function and lambda signatures must be fully typed. Sigil does not implement
general-purpose let-polymorphism or broad signature inference, but ordinary
expression checking is still bidirectional rather than requiring every
subexpression to carry an explicit `:T`.

Examples:

```sigil module
c pi=(3.14:Float)

total λfactorial(n:Int)=>Int
requires n≥0
decreases n
match n{
  0=>1|
  1=>1|
  value=>value*factorial(value-1)
}
```

```sigil expr
λ(x:Int)=>Int=x*2
```

Missing parameter or return type annotations are parse errors.

## Function Modes

Top-level functions are ordinary by default.

A file may place `mode total` once at the top to make total the default for
later function declarations. Any individual declaration may override that
default with `total λname...` or `ordinary λname...`.

Rules:

- only total self-recursive functions may declare `decreases`
- ordinary self-recursive functions may recurse without a termination proof
- total functions may not call declarations marked `ordinary`

## Type Ascription

Type ascription uses one explicit expression form:

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

Sigil intentionally keeps this as one uniform rule. It does not use separate
binding-level surfaces like `c name:Type=value`, and it does not use bare
postfix expression forms like `expr:Type`.

The goal is canonical simplicity for humans and LLMs:

- if you want to ascribe a type to an expression, write `(expr:Type)`
- the same rule applies everywhere
- there is no second declaration-specific rule to learn

The tradeoff is extra parentheses. Sigil accepts that cost to keep one
teachable surface instead of multiple equivalent annotation forms.

## Top-Level Generics

Sigil supports explicit generic declarations at top level:

```sigil decl generic
λidentity[T](x:T)=>T=x
λmapOption[T,U](fn:λ(T)=>U,opt:Option[T])=>Option[U]
```

Polymorphism comes from those declarations.
Local `l` bindings remain monomorphic.

## Type Forms

Primitive types:

- `Int`
- `Float`
- `Bool`
- `String`
- `Char`
- `Unit`
- `Never`

`Never` is Sigil's terminating expression type:

- an expression of type `Never` may be used anywhere another result type is expected
- this is one-way compatibility for expression checking, not a general subtyping feature
- `Never` marks code paths that do not continue, such as `§process.exit(...)`

Compound forms:

- lists: `[T]`
- maps: `{K↦V}`
- functions: `λ(T1,T2,...)=>R`
- owned resources: `Owned[T]`
- named ADTs and aliases

`Owned[T]` is the compiler-known resource wrapper type.

Current ownership rules:

- `Owned[T]` values are introduced by stdlib resource creators and typed extern subscriptions
- they are intended to be consumed with `using`
- borrowed resource values inside `using` scopes must not escape the scope
- `Owned[T]` is affine rather than freely duplicable; Sigil rejects ordinary `l` bindings of owned values and rejects storing them inside ordinary list/record/map literals

## Feature Flag Types

First-class `featureFlag` declarations currently support:

- `Bool`
- named sum types

Examples:

```sigil module
t CheckoutColor=Citrus()|Control()|Ocean()

featureFlag NewCheckout:Bool
  createdAt "2026-04-12T14-00-00Z"
  default false

featureFlag CheckoutColorChoice:CheckoutColor
  createdAt "2026-04-12T14-00-00Z"
  default Control()
```

Current rules:

- the declared `default` must typecheck against the flag type
- the default expression must be pure
- flag descriptors are ordinary typed values and may be passed to `§featureFlags.get`

## Project Types

In projects with `sigil.json`, project-defined named types live in
`src/types.lib.sigil` and are referenced elsewhere as `µTypeName`.

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

Sigil separates type membership from type classification.

`where` describes what values belong to a type.
`label` describes what kind of data a value represents for boundary handling.

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

Rules:

- labels are nominal classifications, not predicates over `value`
- `label X combines Y` adds implied labels during checking
- labelled values behave like ordinary values inside local computation
- labels matter when a labelled value crosses a named topology boundary
- direct consumers must handle directly exposed labelled data at their own boundaries
- unlabeled data is unaffected by boundary-rule checking

Projects pair labelled types with boundary rules in `src/policies.lib.sigil`.
That file owns `rule` and `transform` declarations.

Topology-aware labelled-boundary tests run under `sigil test --env <name>` and
assert the resulting boundary behavior directly with named-boundary helpers such
as:

- `※check::file.existsAt(path,•topology.exportsDir)`
- `※check::log.containsAt(message,•topology.auditLog)`
- `※observe::process.commandsAt(•topology.govBrCli)`

## Records and Maps

Records and maps are different concepts:

- records are exact structural products using `:`
- maps are dynamic keyed collections using `↦`

Examples:

```sigil module
t User={
  id:Int,
  name:String
}

t Scores={String↦Int}
```

Current Sigil has:

- no row polymorphism
- no open records
- no width subtyping

If a field may be absent, use `Option[T]` in an exact record.

## Constrained Types

Named user-defined types may carry a pure `where` clause:

```sigil module
t BirthYear=Int where value>1800 and value<10000

t DateRange={
  end:Int,
  start:Int
} where value.end≥value.start
```

Current rules:

- only `value` is in scope inside the constraint
- the constraint must typecheck to `Bool`
- constraints are pure and world-independent
- constrained aliases and constrained named product types act as compile-time refinements over their underlying type
- values may flow into a constrained type only when the checker can prove the predicate in Sigil's canonical solver-backed refinement fragment
- constrained values widen to their underlying type automatically
- the current proof fragment covers Bool/Int literals, `value`, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, and `not`
- control flow is part of that proof story: `match`, exact record patterns, and internal branching propagate supported branch facts into refinement checking
- direct boolean local aliases of supported facts also narrow
- there is no generated runtime validation in v1

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

Functions may also carry pure compile-time contracts:

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

Current rules:

- `requires` is checked at call sites
- `ensures` is checked against the function body and then flows back to callers as a proven fact
- each function may declare at most one `requires` clause and at most one `ensures` clause
- `requires` may reference only parameters
- `ensures` may reference parameters plus `result`
- contracts must typecheck to `Bool`
- contracts are pure and world-independent
- effectful functions may still carry contracts, but the contracts describe only parameter obligations and returned-value guarantees
- contracts use the same solver-backed proof fragment as constrained types and branch narrowing
- contracts do not inject runtime checks

That makes `where`, `requires`, and `ensures` complementary:

- `where` defines membership in a type
- `requires` states what a caller must prove before a call
- `ensures` states what the callee guarantees after the call returns

The same surfaces now appear in runnable project code too:

- `projects/todo-app/src/todoDomain.lib.sigil`
- `projects/flashcards/src/flashcardsDomain.lib.sigil`
- `projects/algorithms/src/fibonacciSearch.lib.sigil`
- `projects/game-2048/src/game2048.lib.sigil`
- `projects/minesweep/src/minesweepDomain.lib.sigil`
- `projects/roguelike/src/roguelike.lib.sigil`

## Type Equality

Sigil normalizes unconstrained aliases and unconstrained named product types
before equality-sensitive checks.

That means:

- unconstrained aliases compare structurally
- unconstrained named product types compare structurally after normalization
- constrained aliases and named product types use refinement checking over their underlying type instead of plain structural equality
- sum types remain nominal

## Effects

Effect annotations are part of the current surface. Sigil ships with primitive
effects:

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

Tests can also declare effects:

```sigil program language/examples/testWorld.sigil
λmain()=>Unit=()

test "captured log contains line" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("captured"):Unit);
  ※check::log.contains("captured")
}
```

The checker enforces effect propagation. If a body or callee requires `!Fs`,
`!Http`, or any other declared effect, the enclosing signature must declare a
covering effect set or compilation fails.

## Canonical Typed Rules

Some canonical rules depend on type information.

Current important example:

- a pure local binding used exactly once is rejected and must be inlined
- a wildcard sequencing binding must not discard a pure expression

This happens after type checking as part of typed canonical validation.

## Trusted Internal Data

Sigil wants business logic to operate on validated internal values rather than
raw boundary data.

Canonical shape:

```text
raw input
=> parse
=> decode / validate
=> exact internal record or named wrapper
```

Examples:

```sigil module
t Email=Email(String)

t Message={
  createdAt:§time.Instant,
  text:String
}
```

## Source of Truth

When prose and implementation disagree, current truth comes from:

- `language/compiler/crates/sigil-typechecker/`
- runnable examples and tests
- canonical validation behavior
