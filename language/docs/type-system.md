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

λfactorial(n:Int)=>Int match n{
  0=>1|
  1=>1|
  value=>value*factorial(value-1)
}
```

```sigil expr
λ(x:Int)=>Int=x*2
```

Missing parameter or return type annotations are parse errors.

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

Compound forms:

- lists: `[T]`
- maps: `{K↦V}`
- functions: `λ(T1,T2,...)=>R`
- named ADTs and aliases

## Records and Maps

Records and maps are different concepts:

- records are exact structural products using `:`
- maps are dynamic keyed collections using `↦`

Examples:

```sigil module
t User={id:Int,name:String}

t Scores={String↦Int}
```

Current Sigil has:

- no row polymorphism
- no open records
- no width subtyping

If a field may be absent, use `Option[T]` in an exact record.

## Type Equality

Sigil normalizes aliases and named product types before equality-sensitive
checks.

That means:

- aliases compare structurally
- named product types compare structurally after normalization
- sum types remain nominal

## Effects

Effect annotations are part of the current surface. Sigil ships with primitive
effects:

- `Clock`
- `Fs`
- `Http`
- `Log`
- `Process`
- `Tcp`
- `Timer`

Projects may define reusable multi-effect aliases only in `src/effects.lib.sigil`.
Aliases must expand to at least two primitive effects.

Example:

```sigil module projects/docsDriftAudit/src/effects.lib.sigil
effect CliIo=!Fs!Log!Process
```

Examples:

```sigil program
e axios:{get:λ(String)=>!Http String}

e console:{log:λ(String)=>!Log Unit}

λfetch()=>!Http String=axios.get("https://example.com")

λmain()=>!Log Unit=console.log("hello")
```

Tests can also declare effects:

```sigil program tests/writesLog.sigil
i stdlib::io

λmain()=>Unit=()

test "writes log" =>!Log  {
  stdlib::io.println("x")=()
}
```

The checker enforces effect propagation. If a body or callee requires `!Fs`,
`!Http`, or any other declared effect, the enclosing signature must declare a
covering effect set or compilation fails.

## Canonical Typed Rules

Some canonical rules depend on type information.

Current important example:

- a pure local binding used exactly once is rejected and must be inlined

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
t Message={createdAt:stdlib::time.Instant,text:String}

t Email=Email(String)
```

## Source of Truth

When prose and implementation disagree, current truth comes from:

- `language/compiler/crates/sigil-typechecker/`
- runnable examples and tests
- canonical validation behavior
