# Canonical Forms in Sigil

Sigil enforces canonical forms so one valid program has one accepted surface.

This document records the current canonical rules enforced by the lexer,
parser, validator, and typechecker in this repository.

## Why Canonical Forms Exist

Canonical forms are not style guidance. They are part of the language contract.

Goals:

- remove alternative spellings for the same construct
- improve deterministic code generation
- make diagnostics corrective instead of advisory
- keep examples, tests, and generated code aligned

## File Purpose

Sigil uses file extensions to distinguish file purpose:

- `.lib.sigil` for libraries
- `.sigil` for executables and tests

Current canonical rules include:

- `.lib.sigil` files must not define `main`
- non-test `.sigil` files must define `main`
- `test` declarations are only allowed under `tests/`

## Filename Rules

Basenames must be `lowerCamelCase`.

Valid:

- `hello.sigil`
- `userService.lib.sigil`
- `example01Introduction.sigil`

Invalid:

- `UserService.sigil`
- `user_service.lib.sigil`
- `user-service.sigil`
- `1intro.sigil`

Current filename diagnostics:

- `SIGIL-CANON-FILENAME-CASE`
- `SIGIL-CANON-FILENAME-INVALID-CHAR`
- `SIGIL-CANON-FILENAME-FORMAT`

## Declaration Ordering

Top-level declarations must appear in this category order:

```text
t => e => c => λ => test
```

Module scope is declaration-only.
Top-level `l` is invalid.

## No `export` Keyword

Current Sigil does not have an `export` token.

Visibility is file-based:

- declarations in `.lib.sigil` files are referenceable from other modules
- `.sigil` files are executable-oriented

## Function and Lambda Surface

Canonical function/lambda rules:

- parameter types are required
- return types are required
- effects, when present, appear between `=>` and the return type
- `=` is required before non-`match` bodies
- `=` is forbidden before `match` bodies
- multi-item aggregate subforms inside signatures follow the global printer rule, so type arguments and other delimited `2+` forms may span lines

Examples:

```sigil module
λadd(x:Int,y:Int)=>Int=x+y

λfactorial(n:Int)=>Int match n{
  0=>1|
  1=>1|
  value=>value*factorial(value-1)
}
```

## Constants

Current constant syntax is typed value ascription:

```sigil module projects/repoAudit/src/example.lib.sigil
c answer=(42:Int)
```

The older `c answer:Int=42` form is not current Sigil.

## Records and Maps

Records and maps are distinct.

- records use `:`
- maps use `↦`

Examples:

```sigil module
t User={
  id:Int,
  name:String
}

t Scores={String↦Int}
```

Record fields are canonical alphabetical order in:

- product type declarations
- record literals
- typed record constructors

## Local Binding Rules

Local names must not shadow names from the same or any enclosing lexical scope.

This applies to:

- function parameters
- lambda parameters
- `l` bindings
- pattern bindings

## Single-Use Pure Bindings

Sigil currently rejects pure local bindings used exactly once.

Example:

```sigil module
λgreeting(name:String)=>String={
  l prefix=("Hello, ":String);
  prefix
    ++name
    ++prefix
}
```

Required canonical form:

```sigil module
λgreeting(name:String)=>String="Hello, "
  ++name
  ++"Hello, "
```

Current mechanical rule:

- if a local binding is pure
- and the bound name is used exactly once
- the binding is rejected and must be inlined

The current validator does not perform a separate “substitution legality”
analysis. This document describes the implementation as it exists today.

## No Dead Surface

Sigil also rejects dead names where the compiler can determine they serve no
purpose.

Current enforced rules:

- extern declarations must be used
- named local bindings used zero times are rejected
- executable `.sigil` files reject top-level functions, consts, and types that
  are not reachable from `main` or tests

Library note:

- `.lib.sigil` files may still expose top-level declarations that are unused in
  the defining file, because the file surface is the module API

## Canonical List Processing

Sigil now rejects a small set of exact recursive list-plumbing clones when the
language already has one canonical surface.

Current exact-shape bans:

- recursive append-to-result of the form `self(rest)⧺rhs`
- hand-rolled recursive `all` clones
- hand-rolled recursive `any` clones
- filter followed by length of the form `#(xs filter pred)`
- hand-rolled recursive `map` clones
- hand-rolled recursive `filter` clones
- hand-rolled recursive `find` clones
- hand-rolled recursive `flatMap` clones
- hand-rolled recursive `reverse` clones
- hand-rolled recursive `fold` clones

Canonical replacements:

- universal checks: `§list.all`
- existential checks: `§list.any`
- predicate counting: `§list.countIf`
- projection: `map`
- filtering: `filter`
- first-match search: `§list.find`
- flattening projection: `§list.flatMap`
- reduction: `reduce ... from ...` or `§list.fold`
- reversal: `§list.reverse`
- custom list building: wrapper + accumulator helper, reversing once at the end if needed

These are exact-shape validator rules, not general algorithm analysis.
Recursive algorithms that do not match these narrow patterns remain valid.

## Canonical Helper Wrappers

Outside `language/stdlib/`, Sigil also rejects exact top-level helper wrappers
when the body is already one canonical helper surface over that function's own
parameters.

Current exact-wrapper bans:

- direct `§...` helper calls whose arguments are exactly the function parameters
- direct `map` wrappers like `xs map fn`
- direct `filter` wrappers like `xs filter pred`
- direct `reduce ... from ...` wrappers like `xs reduce fn from init`

Examples of rejected shapes:

```sigil invalid-module
λsum1(xs:[Int])=>Int=§list.sum(xs)

λproject[T,U](fn:λ(T)=>U,xs:[T])=>[U]=xs map fn
```

Required canonical forms:

```sigil module
λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)

λreportedSum(xs:[Int])=>String=§string.intToString(§list.sum(xs))
```

This is still a narrow exact-shape rule.
Sigil does not try to prove that arbitrary helper code is semantically
equivalent to a canonical stdlib/helper surface.

## Topology / Config Boundaries

For topology-aware projects:

- topology declarations live in `src/topology.lib.sigil`
- type labels live in `src/types.lib.sigil`
- boundary rules and transforms live in `src/policies.lib.sigil`
- selected environment bindings live in `config/<env>.lib.sigil`
- `process.env` is only allowed in `config/*.lib.sigil`
- application code must use topology dependency handles, not raw endpoints

Validation is currently per selected `--env`, not a whole-project scan across
all declared environments.

## Printer-First Source

Sigil no longer describes canonicality mainly as a checklist of spacing rules.
The authoritative rule is:

- parse source
- print the canonical source for that AST internally
- reject the file unless the bytes match exactly

That gives Sigil a source normal form:

- one textual representation per valid AST
- no public formatter command
- no "preferred style" separate from the language

Some surface constraints are still easiest to think about mechanically:

- delimited aggregate forms stay flat with `0` or `1` item and print multiline with `2+` items
- repeated `++`, `⧺`, `and`, and `or` chains print vertically one continued operand per line
- direct `match` bodies begin on that same line
- multi-arm `match` prints multiline
- string values containing newline characters print as multiline `"` literals with exact preserved line breaks

Canonical examples:

```sigil invalid-program
λfib(n:Int)=>Int match n{
  0=>0|
  1=>1|
  value=>fib(value-1)+fib(value-2)
}
```

## Canonical Branching Recursion

Sigil rejects one narrow recursive shape as non-canonical: sibling self-calls that all directly reduce the same parameter while leaving the other arguments unchanged.

### Blocked Pattern

```sigil invalid-module
λfib(n:Int)=>Int match n{
  0=>0|
  1=>1|
  value=>fib(value-1)+fib(value-2)  // ❌ SIGIL-CANON-BRANCHING-SELF-RECURSION
}
```

Sigil rejects this shape because it duplicates work instead of following one canonical recursion path.

### Canonical Replacement

```sigil module
λfib(n:Int)=>Int=fibHelper(
  0,
  1,
  n
)

λfibHelper(a:Int,b:Int,n:Int)=>Int match n{
  0=>a|
  count=>fibHelper(
    b,
    a+b,
    count-1
  )
}
```

The preferred replacement is a wrapper plus helper function that threads the working state through one recursive step at a time.

### What Gets Rejected

Sigil rejects only exact branching self-recursion when all of these are true:

1. there are multiple sibling self-calls in the same expression
2. each self-call directly reduces the same parameter, such as `n-1` and `n-2`
3. the other arguments are unchanged across those sibling calls

Sigil also rejects obvious nested amplification of that same shape, such as:

```sigil invalid-module
λbad(n:Int)=>Int=bad(bad(n-1)+bad(n-2))
```

### Allowed Patterns

**Single recursive call:**
```sigil module
λlength(xs:[Int])=>Int match xs{
  []=>0|
  [
  h,
  .tail
]=>1+length(tail)
}
```

**Different non-reduced arguments:**
```sigil module
λmerge(left:[Int],right:[Int])=>[Int] match left{
  []=>right|
  [
  lh,
  .lt
]=>match right{
    []=>left|
    [
  rh,
  .rt
]=>match lh≤rh{
      true=>[lh]⧺merge(
        lt,
        right
      )|
      _=>[rh]⧺merge(
        left,
        rt
      )
    }
  }
}
```

Sigil does not attempt general complexity proofs or general exponential-recursion detection. This rule exists to ban one specific non-canonical recursion shape with a clear canonical replacement.

### Error Code

`SIGIL-CANON-BRANCHING-SELF-RECURSION` - Non-canonical branching self-recursion detected. Use a wrapper plus helper state-threading shape instead of sibling self-calls over the same reduced parameter.

## Validation Pipeline

Canonical validation happens in two stages:

1. after parsing, for syntax- and structure-level canonical rules
2. after typechecking, for typed canonical rules such as dead-binding rejection
   and single-use pure bindings

The overall pipeline is:

```text
read source
=> tokenize
=> parse
=> canonical validation
=> typecheck
=> typed canonical validation
=> codegen / run / test
```

## Source of Truth

When prose disagrees with implementation, current truth comes from:

- parser
- validator
- typechecker
- runnable examples and tests
