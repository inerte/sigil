# Sigil Syntax Reference

This document describes the current Sigil surface accepted by the compiler in
this repository.

Sigil is canonical by design. This is not a style guide with alternatives. It
documents the one surface form the parser, internal canonical printer,
validator, and typechecker accept.

If source parses but does not exactly match the compiler's canonical printed
form for that AST, `compile`, `run`, and `test` reject it. There is no public
formatter.

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

```text
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
t => e => i => c => λ => test
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

```sigil module
λadd(x:Int,y:Int)=>Int=x+y
```

Match body:

```sigil module
λfactorial(n:Int)=>Int match n{
  0=>1|
  1=>1|
  value=>value*factorial(value-1)
}
```

For function declarations:

- `=` is required before a non-`match` body
- `=` is forbidden before a `match` body
- the canonical printer keeps the full signature on one physical line
- a direct `match` body begins on that same line

Effects, when present, appear between `=>` and the return type:

```sigil program
e axios

e console

i stdlib::string

λfetchUser(id:Int)=>!Network String=axios.get("https://example.com/"+stdlib::string.intToString(id))

λmain()=>!IO Unit=console.log("hello")
```

## Lambda Expressions

Lambda expressions are fully typed and use the same body rule as top-level
functions:

```sigil expr
λ(x:Int)=>Int=x*2
```

```sigil expr
λ(value:Int)=>Int match value{
  0=>1|
  n=>n+1
}
```

Lambda expressions require:

- parentheses around parameters
- typed parameters
- a return type

Generic lambdas are not part of Sigil's surface.

## Type Declarations

### Product Types

```sigil module
t User={active:Bool,id:Int,name:String}
```

Record fields are canonical alphabetical order everywhere records appear.

### Sum Types

```sigil module
t Color=Red()|Green()|Blue()

t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)

t Option[T]=Some(T)|None()

t Result[T,E]=Ok(T)|Err(E)
```

Imported constructors use qualified module syntax in expressions and patterns:

```sigil module projects/algorithms/src/orderingExample.lib.sigil
i src::graphTypes

λorderingResult()=>src::graphTypes.TopologicalSortResult=src::graphTypes.Ordering([1,2,3])

λorderingValues()=>[Int] match orderingResult(){
  src::graphTypes.Ordering(order)=>order|
  src::graphTypes.CycleDetected()=>[]
}
```

## Constants

Constants require a value ascription:

```sigil module
c answer=(42:Int)

c greeting=("hello":String)
```

Current parser behavior requires the typed form above. Untyped constants and the
older `c name:Type=value` surface are not current Sigil.

## Imports

Sigil imports are namespace imports only:

```sigil module projects/todo-app/src/importsExample.lib.sigil
i core::map

i src::todoDomain

i stdlib::list

i stdlib::json
```

Use imported members through the namespace:

```sigil expr
src::todoDomain.completedCount(todos)
```

```sigil expr
stdlib::list.last(items)
```

Canonical import roots include:

- `core::...`
- `src::...`
- `stdlib::...`

There are no selective imports and no import aliases.

## Externs

Extern declarations use `e`:

```sigil module
e console

e axios:{get:λ(String)=>!Network String}
```

## Local Bindings

Local bindings use `l` inside expressions:

```sigil module
λdoubleAndAdd(x:Int,y:Int)=>Int={
  l doubled=(x*2:Int);
  doubled+doubled+y
}
```

Local names must not shadow names from the same or any enclosing lexical scope.

Pure local bindings used exactly once are non-canonical and must be inlined.

## Pattern Matching

Sigil uses `match` for value-based branching:

```sigil module
λclassify(value:Int)=>String match value{
  0=>"zero"|
  1=>"one"|
  _=>"many"
}
```

Canonical `match` shape comes from the internal printer:

- multi-arm `match` prints multiline
- each arm begins as `pattern=>`
- nested branching may continue on following indented lines
- there is no alternate printed layout for the same `match` AST

Patterns include:

- literals
- identifiers
- `_`
- constructors
- list patterns
- record patterns

Examples:

```sigil module
λfromOption(option:Option[Int])=>Int match option{
  Some(value)=>value|
  None()=>0
}

λheadOrZero(list:[Int])=>Int match list{
  []=>0|
  [head,.rest]=>head
}
```

## Lists, Maps, and Records

List type:

```sigil module
t IntList=[Int]
```

List literal:

```sigil expr
[1,2,3]
```

Map type:

```sigil module
t StringIntMap={String↦Int}
```

Map literals use `↦`:

```sigil exprs
{"a"↦1,"b"↦2}
({↦}:{String↦Int})
```

Record types and literals use `:`:

```sigil module
t User={id:Int,name:String}

λsampleUser()=>User={id:1,name:"Ana"}
```

## Built-In List Operators

Sigil includes canonical list operators:

- `↦` map
- `⊳` filter
- `⊕` ordered reduction
- `⧺` concatenation

Examples:

```sigil module
λconcatenated()=>[Int]=[1,2]⧺[3,4]

λdoubled()=>[Int]=[1,2,3]↦(λ(x:Int)=>Int=x*2)

λfiltered()=>[Int]=[1,2,3]⊳(λ(x:Int)=>Bool=x>1)

λsummed()=>Int=[1,2,3]⊕(λ(acc:Int,x:Int)=>Int=acc+x)⊕0
```

`↦` and `⊳` require pure callbacks.

## Concurrent Regions

Sigil uses one explicit concurrency surface:

```sigil module
λmain()=>!IO [ConcurrentOutcome[Int,String]]=concurrent urlAudit({concurrency:5,jitterMs:Some({max:25,min:1}),stopOn:shouldStop,windowMs:Some(1000)}){
  spawn one()
  spawnEach [1,2,3] process
}

λone()=>!IO Result[Int,String]=Ok(1)

λprocess(value:Int)=>!IO Result[Int,String]=Ok(value)

λshouldStop(err:String)=>Bool=false
```

Rules:

- regions are named: `concurrent name(config){...}`
- config must be a record literal
- config fields are canonical alphabetical order:
  - `concurrency`
  - `jitterMs`
  - `stopOn`
  - `windowMs`
- region bodies are spawn-only:
  - `spawn expr`
  - `spawnEach list fn`
- `spawn` requires an effectful computation returning `Result[T,E]`
- `spawnEach` requires a list and an effectful function returning `Result[T,E]`
- regions return `[ConcurrentOutcome[T,E]]`

`windowMs` and `jitterMs` belong to the region policy, not to `↦` or `⊳`.

Sigil also treats these operators as the canonical surface for common list
plumbing:

- do not hand-write recursive `all` clones; use `stdlib::list.all`
- do not hand-write recursive `any` clones; use `stdlib::list.any`
- do not count with `#(xs⊳pred)`; use `stdlib::list.countIf`
- do not hand-write recursive `map` clones when `↦` fits
- do not hand-write recursive `filter` clones when `⊳` fits
- do not hand-write recursive `find` clones; use `stdlib::list.find`
- do not hand-write recursive `flatMap` clones; use `stdlib::list.flatMap`
- do not hand-write recursive `fold` clones when `⊕` fits
- do not hand-write recursive `reverse` clones; use `stdlib::list.reverse`
- do not build recursive list results with `self(rest)⧺rhs`

## Tests

Tests are top-level declarations and must live under `tests/`:

```sigil program language/test-fixtures/tests/addsNumbers.sigil
λmain()=>Unit=()

test "adds numbers" {
  1+1=2
}
```

Effectful tests use explicit effect annotations:

```sigil program language/test-fixtures/tests/writesLog.sigil
e console

λmain()=>Unit=()

test "writes log" =>!IO  {
  console.log("x")=()
}
```

## withMock

Sigil includes a built-in `withMock(...) { ... }` expression for tests:

```sigil program language/test-fixtures/tests/withMockExample.sigil
λfetchUser(id:Int)=>!Network String="real"

λmain()=>Unit=()

test "fallback on API failure" =>!Network  {
  withMock(fetchUser,λ(id:Int)=>!Network String="ERR"){fetchUser(1)="ERR"}
}
```

Rules:

- `withMock(...)` is only valid directly inside `test` declaration bodies
- allowed targets are any Sigil function or an extern member

## Canonical References

For canonical formatting and validator-enforced rules, see:

- `language/docs/CANONICAL_FORMS.md`
- `language/docs/CANONICAL_ENFORCEMENT.md`
