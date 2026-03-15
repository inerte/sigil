# Sigil Syntax Reference

This document describes the current Sigil surface accepted by the compiler in
this repository.

Sigil is canonical by design. This is not a style guide with alternatives. It
documents the one surface form the parser, validator, and typechecker expect.

## Source Files

Sigil distinguishes file purpose with file extensions:

- `.lib.sigil` for libraries
- `.sigil` for executables and tests

Canonical filename rules:

- basename must be `lowerCamelCase`
- no underscores
- no hyphens
- no spaces
- filename must end with `.sigil` or `.lib.sigil`

Valid examples:

- `userService.lib.sigil`
- `fibonacci.sigil`
- `ffiNodeConsole.lib.sigil`

Invalid examples:

- `UserService.lib.sigil`
- `user_service.lib.sigil`
- `user-service.lib.sigil`

## Comments

Sigil uses one comment syntax:

```sigil
⟦ This is a comment ⟧
```

`#`, `//`, and `/* ... */` are not Sigil comments.

## Top-Level Declarations

Module scope is declaration-only.

Valid top-level forms:

- `t`
- `e`
- `i`
- `c`
- `λ`
- `test`

Invalid at top level:

- `l`

Canonical declaration ordering is:

```text
t → e → i → c → λ → test
```

There is no `export` keyword in current Sigil. Visibility is file-based:

- top-level declarations in `.lib.sigil` files are importable
- `.sigil` files are executable-oriented

## Function Declarations

Function declarations require:

- a name
- typed parameters
- a return type

Regular expression body:

```sigil
λadd(x:Int,y:Int)→Int=x+y
```

Match body:

```sigil
λfactorial(n:Int)→Int match n{
  0→1|
  1→1|
  value→value*factorial(value-1)
}
```

For function declarations:

- `=` is required before a non-`match` body
- `=` is forbidden before a `match` body

Effects, when present, appear between `→` and the return type:

```sigil
λmain()→!IO Unit=console.log("hello")
λfetchUser(id:Int)→!Network String=axios.get("https://example.com/"+stdlib⋅string.intToString(id))
```

## Lambda Expressions

Lambda expressions are fully typed and use the same body rule as top-level
functions:

```sigil
λ(x:Int)→Int=x*2
λ(value:Int)→Int match value{
  0→1|
  n→n+1
}
```

Lambda expressions require:

- parentheses around parameters
- typed parameters
- a return type

Generic lambdas are not part of Sigil's surface.

## Type Declarations

### Product Types

```sigil
t User={active:Bool,id:Int,name:String}
```

Record fields are canonical alphabetical order everywhere records appear.

### Sum Types

```sigil
t Color=Red|Green|Blue
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
```

Imported constructors use qualified module syntax in expressions and patterns:

```sigil
i src⋅graphTypes

src⋅graphTypes.Ordering([1,2,3])

match result{
  src⋅graphTypes.Ordering(order)→order|
  src⋅graphTypes.CycleDetected()→[]
}
```

## Constants

Constants require a value ascription:

```sigil
c answer=(42:Int)
c greeting=("hello":String)
```

Current parser behavior requires the typed form above. Untyped constants and the
older `c name:Type=value` surface are not current Sigil.

## Imports

Sigil imports are namespace imports only:

```sigil
i core⋅map
i src⋅todoDomain
i stdlib⋅list
i stdlib⋅json
```

Use imported members through the namespace:

```sigil
src⋅todoDomain.completedCount(todos)
stdlib⋅list.last(items)
```

Canonical import roots include:

- `core⋅...`
- `src⋅...`
- `stdlib⋅...`

There are no selective imports and no import aliases.

## Externs

Extern declarations use `e`:

```sigil
e console
e axios:{get:λ(String)→!Network String}
```

## Local Bindings

Local bindings use `l` inside expressions:

```sigil
λdoubleAndAdd(x:Int,y:Int)→Int={
  l doubled=(x*2:Int);
  doubled+y
}
```

Local names must not shadow names from the same or any enclosing lexical scope.

Pure local bindings used exactly once are non-canonical and must be inlined.

## Pattern Matching

Sigil uses `match` for value-based branching:

```sigil
match value{
  0→"zero"|
  1→"one"|
  _→"many"
}
```

Patterns include:

- literals
- identifiers
- `_`
- constructors
- list patterns
- record patterns

Examples:

```sigil
match option{
  Some(value)→value|
  None()→0
}

match list{
  []→0|
  [head,..rest]→head
}
```

## Lists, Maps, and Records

List type:

```sigil
[Int]
```

List literal:

```sigil
[1,2,3]
```

Map type:

```sigil
{String↦Int}
```

Map literals use `↦`:

```sigil
{"a"↦1,"b"↦2}
({↦}:{String↦Int})
```

Record types and literals use `:`:

```sigil
t User={id:Int,name:String}
{id:1,name:"Ana"}
```

## Built-In List Operators

Sigil includes canonical list operators:

- `↦` map
- `⊳` filter
- `⊕` ordered reduction
- `⧺` concatenation

Examples:

```sigil
[1,2,3]↦λ(x:Int)→Int=x*2
[1,2,3]⊳λ(x:Int)→Bool=x>1
[1,2,3]⊕λ(acc:Int,x:Int)→Int=acc+x⊕0
[1,2]⧺[3,4]
```

`↦` and `⊳` require pure callbacks.

## Tests

Tests are top-level declarations and must live under `tests/`:

```sigil
test "adds numbers" {
  1+1=2
}
```

Effectful tests use explicit effect annotations:

```sigil
test "writes log" →!IO {
  console.log("x")=()
}
```

## withMock

Sigil includes a built-in `withMock(...) { ... }` expression for tests:

```sigil
λfetchUser(id:Int)→!Network String="real"

test "fallback on API failure" →!Network {
  withMock(fetchUser, λ(id:Int)→!Network String="ERR") {
    fetchUser(1)="ERR"
  }
}
```

Rules:

- `withMock(...)` is only valid directly inside `test` declaration bodies
- allowed targets are any Sigil function or an extern member

## Canonical References

For canonical formatting and validator-enforced rules, see:

- `language/docs/CANONICAL_FORMS.md`
- `language/docs/CANONICAL_ENFORCEMENT.md`
