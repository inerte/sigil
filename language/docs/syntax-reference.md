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

Comments are non-semantic trivia. They are allowed in canonical source, but
they do not participate in canonical source comparison or code coverage
extraction for checked docs.

## Top-Level Declarations

Module scope is declaration-only.

Valid top-level forms:

- `label`
- `rule`
- `transform`
- `t`
- `e`
- `c`
- `λ`
- `test`

Invalid at top level:

- `l`

Canonical declaration ordering is:

```text
label => t => e => c => transform => λ => rule => test
```

There is no `export` keyword in current Sigil. Visibility is file-based:

- top-level declarations in `.lib.sigil` files are referenceable from other modules
- `.sigil` files are executable-oriented
- top-level functions, consts, and types in `.sigil` files must be reachable
  from `main` or tests
- `.lib.sigil` files may still expose declarations that are unused locally

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
e axios:{get:λ(String)=>!Http String}

e console:{log:λ(String)=>!Log Unit}

λfetchUser(id:Int)=>!Http String=axios.get("https://example.com/"+§string.intToString(id))

λmain()=>!Http!Log Unit={
  l _=(fetchUser(1):String);
  console.log("hello")
}
```

The built-in primitive effects are:

- `Clock`
- `Fs`
- `Http`
- `Log`
- `Process`
- `Random`
- `Tcp`
- `Timer`

Projects may define reusable multi-effect aliases only in `src/effects.lib.sigil`:

```sigil module projects/repoAudit/src/effects.lib.sigil
effect CliIo=!Fs!Log!Process
```

Those aliases are project-global and may be used directly in signatures.

Function contracts, when present, appear after the signature and before the
body:

```sigil module
λnormalizeYear(raw:Int)=>Int
requires raw>0
ensures result>1800
match raw>1800{
  true=>raw|
  false=>1900
}
```

Contract rules:

- each function may declare at most one `requires` clause and at most one `ensures` clause
- if both are present, `requires` must come before `ensures`
- `requires` may reference only parameters
- `ensures` may reference parameters plus `result`
- contracts must typecheck to `Bool`
- contracts must be pure and world-independent
- direct `match` bodies still begin with `match`, not `=match`, even after contract lines
- effectful functions may carry contracts, but the contract surface talks only about parameters and the returned value

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

### Project-Defined Named Types

Inside a project with `sigil.json`, all project-defined named types live in:

```text
src/types.lib.sigil
```

Rules:

- `src/types.lib.sigil` may contain only `t` and `label` declarations
- outside that file, project-defined types are referenced as `µTypeName`
- project sum constructors and patterns from `src/types.lib.sigil` also use `µ...`
- `src/types.lib.sigil` may reference only `§...` and `¶...` inside type definitions and constraints

### Labels

Projects and standalone files may declare labels:

```sigil module
label Brazil

label Paraguay

label Pii

label Mercosur combines [Brazil,Paraguay]
```

Types may attach one or more labels:

```sigil module
label Pii

label Usa

t Ssn=String label [Pii,Usa]
```

Rules:

- `where` and `label` are separate surfaces
- `label` classifies the type; it does not replace value-level refinement
- `label X combines Y` adds implied labels during boundary checking
- singleton label attachment prints as `label Pii`
- multiple labels print as `label [A,B]`

### Boundary Rules and Transforms

Projects use:

```text
src/policies.lib.sigil
```

That file is the canonical home for:

- `rule`
- `transform`

Standalone `.sigil` and `.lib.sigil` files may also declare these forms locally
for small examples and scripts.

Example:

```sigil module projects/labelled-boundaries/src/policies.lib.sigil
transform λredactSsn(ssn:µSsn)=>String="***"

rule [µ.Pii,µ.Usa] for •topology.auditLog=Through(•policies.redactSsn)
```

`rule` targets exact named boundaries only in the current surface.

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

Project-defined constructors from `src/types.lib.sigil` use `µ...` in expressions
and patterns:

```sigil module projects/algorithms/src/types.lib.sigil
t TopologicalSortResult=CycleDetected()|Ordering([Int])
```

```sigil module projects/algorithms/src/orderingExample.lib.sigil
λorderingResult()=>µTopologicalSortResult=µOrdering([1,2,3])

λorderingValues(result:µTopologicalSortResult)=>[Int] match result{
  µOrdering(order)=>order|
  µCycleDetected()=>[]
}
```

### Constrained Types

Named types may carry a pure `where` clause:

```sigil module
t BirthYear=Int where value>1800 and value<10000

t DateRange={end:Int,start:Int} where value.end≥value.start
```

Constraint rules:

- only `value` is in scope
- the expression must typecheck to `Bool`
- constraints are pure and world-independent
- constrained aliases and constrained named product types act as compile-time refinements over their underlying type
- values flow into a constrained type only when the checker can prove the predicate in Sigil's canonical solver-backed refinement fragment
- constrained values widen to their underlying type automatically
- the current proof fragment covers Bool/Int literals, `value`, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, and `not`
- `match`, exact record patterns, and internal branching propagate supported branch facts into that refinement proof
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

## Constants

Constants require a value ascription:

```sigil module
c answer=(42:Int)

c greeting=("hello":String)
```

Current parser behavior requires the typed form above. Untyped constants and the
older `c name:Type=value` surface are not current Sigil.

## String Literals

Sigil uses one string literal surface:

```sigil expr
"hello"
```

The same `"` form also allows multiline strings:

```sigil expr
"hello
world"
```

String literal rules:

- the string value is exactly the raw contents between the quotes
- literal newlines inside the quotes are preserved as newline characters
- indentation spaces inside the quotes are preserved exactly
- `\\`, `\"`, `\n`, `\r`, and `\t` remain valid escapes
- there is no heredoc, triple-quote, dedent, or trim-first-line variant

Canonical note:

- if a string value contains newline characters, canonical source prints it as a multiline `"` string with literal line breaks rather than `\n` escapes

## Rooted References

Sigil uses rooted module references directly at the use site. There are no
top-level import declarations:

```sigil module projects/todo-app/src/countTodos.lib.sigil
λtodoCount(todos:[µTodo])=>Int=•todoDomain.completedCount(todos)
```

Use rooted members through the namespace:

```sigil expr
•todoDomain.completedCount(todos)
```

```sigil expr
§list.last(items)
```

Canonical module roots include:

- `¶...`
- `¤...`
- `•...`
- `§...`
- `※...`
- `†...`

Project-defined named types and project sum constructors use:

- `µ...`

There are no selective imports, import aliases, or separate import
declarations.

## Externs

Extern declarations use `e`:

```sigil module
e console:{log:λ(String)=>!Log Unit}

e axios:{get:λ(String)=>!Http String}
```

Unused extern declarations are non-canonical.

## Local Bindings

Local bindings use `l` inside expressions:

```sigil module
λdoubleAndAdd(x:Int,y:Int)=>Int={
  l doubled=(x*2:Int);
  doubled+doubled+y
}
```

Local names must not shadow names from the same or any enclosing lexical scope.

Named local bindings used zero times are non-canonical.

Pure local bindings used exactly once are non-canonical and must be inlined.

When a binding exists only to sequence effects, use the wildcard pattern:

```sigil expr
{
  l _=(§io.println("x"):Unit);
  ()
}
```

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
- tuple patterns

Current match rules:

- `match` is Sigil's only branching surface
- matches over finite structural spaces must be exhaustive
- redundant and unreachable arms are rejected
- `Bool`, `Unit`, tuples, list shapes, exact record patterns, and nominal sum constructors participate in exhaustiveness checking
- coverage, contracts, and refinement narrowing share the same canonical proof fragment
- supported proof facts include Bool/Int literals, rooted or pattern-bound values, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, `not`, direct boolean local aliases of those supported facts, and shape facts from tuple/list/record/constructor patterns
- unsupported guards remain valid source, but they stay opaque to coverage and refinement narrowing
- exact record patterns must mention every field of the matched exact record type

Examples:

```sigil module
t Point={x:Int,y:Int}

λfromOption(option:Option[Int])=>Int match option{
  Some(value)=>value|
  None()=>0
}

λheadOrZero(list:[Int])=>Int match list{
  []=>0|
  [head,.rest]=>head
}

λpairLabel(left:Bool,right:Bool)=>String match (left,right){
  (true,true)=>"tt"|
  (true,false)=>"tf"|
  (false,true)=>"ft"|
  (false,false)=>"ff"
}

λpointLabel(point:Point)=>String match point{
  {x:0,y:0}=>"origin"|
  {x:0,y}=>"y-axis"|
  {x,y:0}=>"x-axis"|
  {x,y}=>"plane"
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

- `map` projection
- `filter` filtering
- `reduce ... from ...` ordered reduction
- `⧺` concatenation

Examples:

```sigil module
λconcatenated()=>[Int]=[1,2]⧺[3,4]

λdoubled()=>[Int]=[1,2,3] map (λ(x:Int)=>Int=x*2)

λfiltered()=>[Int]=[1,2,3] filter (λ(x:Int)=>Bool=x>1)

λsummed()=>Int=[1,2,3] reduce (λ(acc:Int,x:Int)=>Int=acc+x) from 0
```

`map` and `filter` require pure callbacks.

## Concurrent Regions

Sigil uses one explicit concurrency surface:

```sigil program
λmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@5:{jitterMs:Some({max:25,min:1}),stopOn:shouldStop,windowMs:Some(1000)}{
  spawn one()
  spawnEach [1,2,3] process
}

λone()=>!Timer Result[Int,String]={
  l _=(§time.sleepMs(0):Unit);
  Ok(1)
}

λprocess(value:Int)=>!Timer Result[Int,String]={
  l _=(§time.sleepMs(0):Unit);
  Ok(value)
}

λshouldStop(err:String)=>Bool=false
```

Rules:

- regions are named: `concurrent name@width{...}`
- width is required after `@`
- optional policy attaches as `:{...}`
- policy fields are canonical alphabetical order:
  - `jitterMs`
  - `stopOn`
  - `windowMs`
- region bodies are spawn-only:
  - `spawn expr`
  - `spawnEach list fn`
- `spawn` requires an effectful computation returning `Result[T,E]`
- `spawnEach` requires a list and an effectful function returning `Result[T,E]`
- regions return `[ConcurrentOutcome[T,E]]`

Omitted policy defaults to no jitter, no early stop, and no windowing.

`windowMs` and `jitterMs` belong to the region policy, not to `map` or `filter`.

Sigil also treats these operators as the canonical surface for common list
plumbing:

- do not hand-write recursive `all` clones; use `§list.all`
- do not hand-write recursive `any` clones; use `§list.any`
- do not count with `#(xs filter pred)`; use `§list.countIf`
- do not hand-write recursive `map` clones when `map` fits
- do not hand-write recursive `filter` clones when `filter` fits
- do not hand-write recursive `find` clones; use `§list.find`
- do not hand-write recursive `flatMap` clones; use `§list.flatMap`
- do not hand-write recursive `fold` clones when `reduce ... from ...` fits
- do not hand-write recursive `reverse` clones; use `§list.reverse`
- do not build recursive list results with `self(rest)⧺rhs`
- do not wrap canonical helpers just to rename them; exact wrappers like `λsum1(xs)=>Int=§list.sum(xs)` are rejected
- do not wrap `map`, `filter`, or `reduce ... from ...` in trivial top-level aliases; use the operator surface directly

## Tests

Tests are top-level declarations and must live under `tests/`:

```sigil program language/tests/addsNumbers.sigil
λmain()=>Unit=()

test "adds numbers" {
  1+1=2
}
```

Effectful tests use explicit effect annotations:

```sigil program tests/writesLog.sigil
λmain()=>Unit=()

test "writes log" =>!Log {
  l _=(§io.println("x"):Unit);
  true
}
```

Tests may also derive the active world locally:

```sigil program language/tests/testWorld.sigil
λmain()=>Unit=()

test "captured log contains line" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("captured"):Unit);
  ※check::log.contains("captured")
}
```

Rules:

- `world { ... }` appears between the optional test effects and the body
- world clauses are declaration-only and use `c` bindings
- world bindings must be pure entry values from `†...`
- `※observe` and `※check` are test-only roots for reading the active test world

## Canonical References

For canonical formatting and validator-enforced rules, see:

- `language/docs/CANONICAL_FORMS.md`
- `language/docs/CANONICAL_ENFORCEMENT.md`
