# Sigil Standard Library

## Overview

The Sigil standard library provides core utility functions and predicates for common programming tasks. All functions follow canonical form principles - exactly ONE way to solve each problem.

## Current Status

**Implemented:**
- ✅ Decode / validation pipeline for trusted internal data - `stdlib/decode`
- ✅ List predicates (validation, checking) - `stdlib/list`
- ✅ Numeric predicates and ranges - `stdlib/numeric`
- ✅ List utilities (head, tail, take/drop/reverse, safe lookup) - `stdlib/list`
- ✅ String operations (manipulation, searching) - `stdlib/string`
- ✅ String predicates (prefix/suffix checking) - `stdlib/string`
- ✅ File system operations - `stdlib/file`
- ✅ Process execution for harnesses and tooling - `stdlib/process`
- ✅ Regular-expression compile/test/search - `stdlib/regex`
- ✅ HTTP and TCP clients and servers - `stdlib/httpClient`, `stdlib/httpServer`, `stdlib/tcpClient`, `stdlib/tcpServer`
- ✅ Runtime dependency topology - `stdlib/topology`
- ✅ Runtime dependency config bindings - `stdlib/config`
- ✅ JSON parsing/serialization - `stdlib/json`
- ✅ Path manipulation - `stdlib/path`
- ✅ Time parsing/comparison/clock - `stdlib/time`
- ✅ URL parsing/query helpers - `stdlib/url`
- ✅ Core prelude vocabulary (Option, Result) - `core/prelude` (implicit)
- ✅ Length operator (`#`) - works on strings and lists

**Not yet implemented:**
- ⏳ Crypto utilities

## Import Syntax

```sigil program
e console

i stdlib::file

i stdlib::httpClient

i stdlib::httpServer

i stdlib::json

i stdlib::list

i stdlib::numeric

i stdlib::path

i stdlib::process

i stdlib::regex

i stdlib::string

i stdlib::time

i stdlib::url

λmain()=>Unit=console.log(stdlib::string.intToString(#[1,2,3])++" "++stdlib::time.formatIso(stdlib::time.fromEpochMillis(0)))
```

**Design:** Imports work exactly like FFI (`e module::path`). No selective imports, always use fully qualified names. This prevents name collisions and makes code explicit.

## Length Operator (`#`)

The `#` operator is a **built-in language operator** that returns the length of strings and lists.

**Syntax:**
```text
#expression => Int
```

**Type Checking:**
- Works on strings (`String`) and lists (`[T]`)
- Compile error for other types
- Always returns integer (`Int`)

**Examples:**
```sigil program
λmain()=>Bool=#"hello"=5 and #""=0 and #[1,2,3]=3
```

**Note on Empty Lists:**
Empty lists `[]` infer their type from context:
- In pattern matching: First arm establishes the type
- In function return: Return type annotation provides context
- In standalone expressions: Type cannot be inferred (use function with explicit return type)

**Why `#` instead of functions?**

1. **ONE canonical form** - Not `stdlib::string` helper calls vs `stdlib::list` helper calls, just `#`
2. **Leverages bidirectional type checking** - Type is known at compile time
3. **Concise** - Machine-first language optimizes for brevity (`#s` vs `len(s)`)
4. **Zero syntactic variation** - Single way to express "get length"

**Codegen:**
```typescript
#s          => (await s).length
#[1,2,3]    => (await [1,2,3]).length
```

**Note:** The deprecated `stdlib::list.len` function has been removed. Use `#` instead.

## Module Exports

Sigil uses file-based visibility:
- `.lib.sigil` exports all top-level declarations automatically
- `.sigil` files are executable-oriented

There is no `export` keyword.

## File, Path, Process, JSON, Time, and URL

`stdlib::file` exposes canonical UTF-8 filesystem helpers:

```sigil program
i stdlib::file

i stdlib::path

λmain()=>!IO Unit={
  l out=(stdlib::path.join("/tmp","sigil.txt"):String);
  l written=(stdlib::file.writeText("hello",out):Unit);
  l text=(stdlib::file.readText(out):String);
  ()
}
```

It also exposes `makeTempDir(prefix)` for canonical temp workspace creation in
tooling and harness code.

`stdlib::path` exposes canonical filesystem path operations:

```sigil program
i stdlib::path

λmain()=>Unit={
  l article=(stdlib::path.basename("website/articles/hello.md"):String);
  l directory=(stdlib::path.join("website","articles"):String);
  ()
}
```

`stdlib::process` exposes canonical argv-based child-process execution:

```sigil program
i stdlib::process

λmain()=>!IO Unit={
  l result=(stdlib::process.run(stdlib::process.command(["git","status"])):stdlib::process.ProcessResult);
  match result.code=0{
    true=>()|
    false=>()
  }
}
```

The canonical process surface is:
- `command`
- `exit`
- `withCwd`
- `withEnv`
- `run`
- `start`
- `wait`
- `kill`

Commands are argv-based only. Non-zero exit status is returned in
`ProcessResult.code`; it is not a separate failure channel.

`stdlib::regex` exposes a small JavaScript-backed regular-expression surface:

```sigil program
i stdlib::regex

λmain()=>Unit match stdlib::regex.compile("i","^(sigil)-(.*)$"){
  Ok(regex)=>match stdlib::regex.find("Sigil-lang",regex){
    Some(found)=>{
      l matched=(found.full:String);
      ()
    }|
    None()=>()
  }|
  Err(_)=>()
}
```

The canonical regex surface is:
- `compile`
- `find`
- `isMatch`

Regex semantics in v1 follow JavaScript `RegExp`, including pattern syntax and
flags. `compile` validates the pattern/flags first and returns `Err` on invalid
input. `find` returns only the first match.

`stdlib::json` exposes a typed JSON AST with safe parsing:

```sigil program
i stdlib::json

λmain()=>Unit match stdlib::json.parse("{\"ok\":true}"){
  Ok(value)=>match stdlib::json.asObject(value){
    Some(_)=>()|
    None()=>()
  }|
  Err(_)=>()
}
```

`stdlib::decode` is the canonical layer for turning raw `JsonValue` into trusted
internal Sigil values:

```sigil module
t Message={createdAt:stdlib::time.Instant,text:String}

i stdlib::decode

i stdlib::json

i stdlib::time

λinstant(value:stdlib::json.JsonValue)=>Result[stdlib::time.Instant,stdlib::decode.DecodeError] match stdlib::decode.string(value){
  Ok(text)=>match stdlib::time.parseIso(text){
    Ok(instant)=>Ok(instant)|
    Err(error)=>Err({message:error.message,path:[]})
  }|
  Err(error)=>Err(error)
}

λmessage(value:stdlib::json.JsonValue)=>Result[Message,stdlib::decode.DecodeError] match stdlib::decode.field(instant,"createdAt")(value){
  Ok(createdAt)=>match stdlib::decode.field(stdlib::decode.string,"text")(value){
    Ok(text)=>Ok({createdAt:createdAt,text:text})|
    Err(error)=>Err(error)
  }|
  Err(error)=>Err(error)
}
```

The intended split is:
- `stdlib::json` for raw parse / inspect / stringify
- `stdlib::decode` for decode / validate / trust

If a field may be absent, keep the record exact and use `Option[T]` in that
field. Sigil does not use open or partial records for this.

`stdlib::time` exposes strict ISO parsing, instant comparison, and harness sleep:

```sigil program
i stdlib::time

λmain()=>Unit match stdlib::time.parseIso("2026-03-03"){
  Ok(instant)=>{
    l millis=(stdlib::time.toEpochMillis(instant):Int);
    ()
  }|
  Err(_)=>()
}
```

Effectful code may also use `stdlib::time.sleepMs(ms)` for retry loops and
process orchestration.

`stdlib::url` exposes strict parse results and typed URL fields for both absolute and relative targets:

```sigil program
i stdlib::url

λmain()=>Unit match stdlib::url.parse("../language/spec/cli-json.md?view=raw#schema"){
  Ok(url)=>{
    l path=(url.path:String);
    l suffix=(stdlib::url.suffix(url):String);
    ()
  }|
  Err(_)=>()
}
```

## HTTP Client and Server

`stdlib::httpClient` is the canonical text-based HTTP client layer.

For topology-aware projects, the canonical surface is handle-based rather than
raw-URL based:

```sigil program projects/topology-http/src/getClient.sigil
i stdlib::httpClient

i src::topology

λmain()=>!IO Unit match stdlib::httpClient.get(src::topology.mailerApi,stdlib::httpClient.emptyHeaders(),"/health"){
  Ok(response)=>{
    l body=(response.body:String);
    ()
  }|
  Err(error)=>{
    l message=(error.message:String);
    ()
  }
}
```

The split is:
- transport/URL failures return `Err(HttpError)`
- any received HTTP response, including `404` and `500`, returns `Ok(HttpResponse)`
- JSON helpers compose over `stdlib::json`
- topology-aware application code must not pass raw base URLs directly

`stdlib::topology` owns the dependency handles.
`stdlib::config` owns per-environment bindings in `config/*.lib.sigil`.

`stdlib::httpServer` is the canonical request/response server layer:

```sigil program
i stdlib::httpServer

λhandle(request:stdlib::httpServer.Request)=>!IO stdlib::httpServer.Response match request.path{
  "/health"=>stdlib::httpServer.ok("healthy")|
  _=>stdlib::httpServer.notFound()
}

λmain()=>!IO Unit=stdlib::httpServer.serve(handle,8080)
```

`serve` is a long-lived runtime entrypoint: once the server is listening, the
process stays open until it is terminated externally.

## TCP Client and Server

`stdlib::tcpClient` is the canonical one-request, one-response TCP client layer.

For topology-aware projects, the canonical surface is handle-based:

```sigil program projects/topology-tcp/src/pingClient.sigil
i src::topology

i stdlib::tcpClient

λmain()=>!IO Unit match stdlib::tcpClient.send(src::topology.eventStream,"ping"){
  Ok(response)=>{
    l message=(response.message:String);
    ()
  }|
  Err(error)=>{
    l errorMessage=(error.message:String);
    ()
  }
}
```

The canonical framing model is:
- UTF-8 text only
- one newline-delimited request per connection
- one newline-delimited response per connection

`stdlib::topology` owns the dependency handles.
`stdlib::config` owns per-environment bindings in `config/*.lib.sigil`.

`stdlib::tcpServer` is the matching minimal TCP server layer:

```sigil program
i stdlib::tcpServer

λhandle(request:stdlib::tcpServer.Request)=>!IO stdlib::tcpServer.Response=stdlib::tcpServer.response(request.message)

λmain()=>!IO Unit=stdlib::tcpServer.serve(handle,45120)
```

`serve` is long-lived: once the TCP server is listening, the process stays open
until it is terminated externally.

## Topology

`stdlib::topology` is the canonical declaration layer for external HTTP and TCP
runtime dependencies. `stdlib::config` is the canonical binding layer.

Topology-aware projects define `src/topology.lib.sigil`, the selected
`config/<env>.lib.sigil`, and use typed handles instead
of raw endpoints in application code:

```sigil program projects/topology-http/src/getClient.sigil
i src::topology

i stdlib::httpClient

λmain()=>!IO Unit match stdlib::httpClient.get(src::topology.mailerApi,stdlib::httpClient.emptyHeaders(),"/health"){
  Ok(_)=>()|
  Err(_)=>()
}
```

See [topology.md](./topology.md) for the full model.

## List Predicates

**Module:** `stdlib/list`

### sortedAsc

Check if a list is sorted in ascending order.

```sigil decl stdlib::list
λsortedAsc(xs:[Int])=>Bool
```

**Examples:**
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.sortedAsc([1,2,3]) and ¬stdlib::list.sortedAsc([3,2,1]) and stdlib::list.sortedAsc([]) and stdlib::list.sortedAsc([5])
```

**Use case:** Validate precondition for binary search or other sorted-list algorithms.

### sortedDesc

Check if a list is sorted in descending order.

```sigil decl stdlib::list
λsortedDesc(xs:[Int])=>Bool
```

**Examples:**
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.sortedDesc([3,2,1]) and ¬stdlib::list.sortedDesc([1,2,3])
```

### all

Check if all elements in a list satisfy a predicate.

```sigil decl stdlib::list
λall[T](pred:λ(T)=>Bool,xs:[T])=>Bool
```

**Examples:**
```sigil program
i stdlib::list

i stdlib::numeric

λmain()=>Bool=stdlib::list.all(stdlib::numeric.isPositive,[1,2,3]) and ¬stdlib::list.all(stdlib::numeric.isPositive,[1,-2,3]) and stdlib::list.all(stdlib::numeric.isEven,[2,4,6])
```

**Use case:** Validate that all elements meet a requirement.

### any

Check if any element in a list satisfies a predicate.

```sigil decl stdlib::list
λany[T](pred:λ(T)=>Bool,xs:[T])=>Bool
```

**Examples:**
```sigil program
i stdlib::list

i stdlib::numeric

λmain()=>Bool=¬stdlib::list.any(stdlib::numeric.isEven,[1,3,5]) and stdlib::list.any(stdlib::numeric.isEven,[1,2,3]) and stdlib::list.any(stdlib::numeric.isPrime,[4,6,8,7])
```

**Use case:** Check if at least one element meets a requirement.

### contains

Check if an element exists in a list.

```sigil decl stdlib::list
λcontains[T](item:T,xs:[T])=>Bool
```

**Examples:**
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.contains(3,[1,2,3,4]) and ¬stdlib::list.contains(5,[1,2,3,4]) and ¬stdlib::list.contains(1,[])
```

**Use case:** Membership testing.

### count

Count occurrences of an element in a list.

```sigil decl stdlib::list
λcount[T](item:T,xs:[T])=>Int
```

### countIf

Count elements that satisfy a predicate.

```sigil decl stdlib::list
λcountIf[T](pred:λ(T)=>Bool,xs:[T])=>Int
```

### drop

Drop the first `n` elements.

```sigil decl stdlib::list
λdrop[T](n:Int,xs:[T])=>[T]
```

### find

Find the first element that satisfies a predicate.

```sigil decl stdlib::list
λfind[T](pred:λ(T)=>Bool,xs:[T])=>Option[T]
```

Examples:
```sigil program
i stdlib::list

i stdlib::numeric

λmain()=>Bool=(match stdlib::list.find(stdlib::numeric.isEven,[1,3,4,6]){
  Some(value)=>value=4|
  None()=>false
}) and (match stdlib::list.find(stdlib::numeric.isEven,[1,3,5]){
  Some(_)=>false|
  None()=>true
})
```

### flatMap

Map each element to a list and flatten the results in order.

```sigil decl stdlib::list
λflatMap[T,U](fn:λ(T)=>[U],xs:[T])=>[U]
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.flatMap(λ(x:Int)=>[Int]=[x,x],[1,2,3])=[1,1,2,2,3,3]
```

### fold

Reduce a list to a single value by threading an accumulator from left to right.

```sigil decl stdlib::list
λfold[T,U](acc:U,fn:λ(U,T)=>U,xs:[T])=>U
```

Examples:
```sigil program
i stdlib::list

λappendDigit(acc:Int,x:Int)=>Int=acc*10+x

λmain()=>Bool=stdlib::list.fold(0,λ(acc:Int,x:Int)=>Int=acc+x,[1,2,3])=6 and stdlib::list.fold(0,appendDigit,[1,2,3])=123
```

### inBounds

Check if an index is valid for a list (in range [0, len-1]).

```sigil decl stdlib::list
λinBounds[T](idx:Int,xs:[T])=>Bool
```

**Examples:**
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.inBounds(0,[1,2,3]) and stdlib::list.inBounds(2,[1,2,3]) and ¬stdlib::list.inBounds(3,[1,2,3]) and ¬stdlib::list.inBounds(-1,[1,2,3]) and ¬stdlib::list.inBounds(0,[])
```

**Use case:** Validate array/list access before indexing. Prevents out-of-bounds errors.

**Implementation:** Uses `#xs` to check bounds.

## List Utilities

**Module:** `stdlib/list`

**Note:** Use the `#` operator for list length instead of a function (e.g., `#[1,2,3]` => `3`).

### last

Get the last element safely.

```sigil decl stdlib::list
λlast[T](xs:[T])=>Option[T]
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=(match stdlib::list.last([]){
  Some(_)=>false|
  None()=>true
}) and (match stdlib::list.last([1,2,3]){
  Some(value)=>value=3|
  None()=>false
})
```

### max

Get the maximum element safely.

```sigil decl stdlib::list
λmax(xs:[Int])=>Option[Int]
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=(match stdlib::list.max([]){
  Some(_)=>false|
  None()=>true
}) and (match stdlib::list.max([3,9,4]){
  Some(value)=>value=9|
  None()=>false
})
```

### min

Get the minimum element safely.

```sigil decl stdlib::list
λmin(xs:[Int])=>Option[Int]
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=(match stdlib::list.min([]){
  Some(_)=>false|
  None()=>true
}) and (match stdlib::list.min([3,9,4]){
  Some(value)=>value=3|
  None()=>false
})
```

### nth

Get the item at a zero-based index safely.

```sigil decl stdlib::list
λnth[T](idx:Int,xs:[T])=>Option[T]
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=(match stdlib::list.nth(0,[7,8]){
  Some(value)=>value=7|
  None()=>false
}) and (match stdlib::list.nth(2,[7,8]){
  Some(_)=>false|
  None()=>true
})
```

### product

Multiply all integers in a list.

```sigil decl stdlib::list
λproduct(xs:[Int])=>Int
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.product([])=1 and stdlib::list.product([2,3,4])=24
```

### removeFirst

Remove the first occurrence of an element.

```sigil decl stdlib::list
λremoveFirst[T](item:T,xs:[T])=>[T]
```

### reverse

Reverse a list.

```sigil decl stdlib::list
λreverse[T](xs:[T])=>[T]
```

### sum

Sum all integers in a list.

```sigil decl stdlib::list
λsum(xs:[Int])=>Int
```

Examples:
```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.sum([])=0 and stdlib::list.sum([1,2,3,4])=10
```

### take

Take the first `n` elements.

```sigil decl stdlib::list
λtake[T](n:Int,xs:[T])=>[T]
```

## Numeric Helpers

**Module:** `stdlib/numeric`

### range

Build an ascending integer range, inclusive at both ends.

```sigil decl stdlib::numeric
λrange(start:Int,stop:Int)=>[Int]
```

Examples:
```sigil program
i stdlib::numeric

λmain()=>Bool=stdlib::numeric.range(2,5)=[2,3,4,5] and stdlib::numeric.range(3,3)=[3] and stdlib::numeric.range(5,2)=[]
```

## Canonical List-Processing Surface

For ordinary list work, Sigil expects the canonical operators and stdlib path,
not hand-rolled recursive plumbing:

- use `stdlib::list.all` for universal checks
- use `stdlib::list.any` for existential checks
- use `stdlib::list.countIf` for predicate counting
- use `↦` for projection
- use `⊳` for filtering
- use `stdlib::list.find` for first-match search
- use `stdlib::list.flatMap` for flattening projection
- use `⊕` or `stdlib::list.fold` for reduction
- use `stdlib::list.reverse` for reversal

Sigil now rejects exact recursive clones of `all`, `any`, `map`, `filter`,
`find`, `flatMap`, `fold`, and `reverse`, rejects `#(xs⊳pred)` in favor of
`stdlib::list.countIf`, and rejects recursive result-building of the form
`self(rest)⧺rhs`.

## String Operations

**Module:** `stdlib/string`

Comprehensive string manipulation functions. These are **compiler intrinsics** - the compiler emits optimized JavaScript directly instead of calling Sigil functions.

### charAt

Get character at index.

```sigil decl stdlib::string
λcharAt(idx:Int,s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.charAt(0,"hello")="h" and stdlib::string.charAt(4,"hello")="o"
```

**Codegen:** `s.charAt(idx)`

### substring

Get substring from start to end index.

```sigil decl stdlib::string
λsubstring(end:Int,s:String,start:Int)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.substring(11,"hello world",6)="world" and stdlib::string.substring(3,"hello",0)="hel"
```

**Codegen:** `s.substring(start, end)`

### take

Take first n characters.

```sigil decl stdlib::string
λtake(n:Int,s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.take(3,"hello")="hel" and stdlib::string.take(5,"hi")="hi"
```

**Implementation:** `substring(n, s, 0)` (in Sigil)

### drop

Drop first n characters.

```sigil decl stdlib::string
λdrop(n:Int,s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.drop(2,"hello")="llo" and stdlib::string.drop(5,"hi")=""
```

**Implementation:** `substring(#s, s, n)` (in Sigil, uses `#` operator)

### lines

Split a string on newline characters.

```sigil decl stdlib::string
λlines(s:String)=>[String]
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.lines("a\nb\nc")=["a","b","c"] and stdlib::string.lines("hello")=["hello"]
```

**Implementation:** `split("\n", s)` (in Sigil)

### toUpper

Convert to uppercase.

```sigil decl stdlib::string
λtoUpper(s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.toUpper("hello")="HELLO"
```

**Codegen:** `s.toUpperCase()`

### toLower

Convert to lowercase.

```sigil decl stdlib::string
λtoLower(s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.toLower("WORLD")="world"
```

**Codegen:** `s.toLowerCase()`

### trim

Remove leading and trailing whitespace.

```sigil decl stdlib::string
λtrim(s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.trim("  hello  ")="hello" and stdlib::string.trim("\n\ttest\n")="test"
```

**Codegen:** `s.trim()`

### indexOf

Find index of first occurrence (returns -1 if not found).

```sigil decl stdlib::string
λindexOf(s:String,search:String)=>Int
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.indexOf("hello world","world")=6 and stdlib::string.indexOf("hello","xyz")=-1
```

**Codegen:** `s.indexOf(search)`

### split

Split string by delimiter.

```sigil decl stdlib::string
λsplit(delimiter:String,s:String)=>[String]
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.split(",","a,b,c")=["a","b","c"] and stdlib::string.split("\n","line1\nline2")=["line1","line2"]
```

**Codegen:** `s.split(delimiter)`

### replaceAll

Replace all occurrences of pattern with replacement.

```sigil decl stdlib::string
λreplaceAll(pattern:String,replacement:String,s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.replaceAll("hello","hi","hello hello")="hi hi"
```

**Codegen:** `s.replaceAll(pattern, replacement)`

### repeat

Repeat a string `count` times.

```sigil decl stdlib::string
λrepeat(count:Int,s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.repeat(3,"ab")="ababab" and stdlib::string.repeat(0,"ab")=""
```

**Implementation:** recursive concatenation in Sigil

### reverse

Reverse a string.

```sigil decl stdlib::string
λreverse(s:String)=>String
```

**Examples:**
```sigil program
i stdlib::string

λmain()=>Bool=stdlib::string.reverse("stressed")="desserts" and stdlib::string.reverse("abc")="cba"
```

**Codegen:** `s.split("").reverse().join("")`

## Current String Surface

`stdlib::string` currently exposes:

- `charAt`
- `drop`
- `endsWith`
- `indexOf`
- `intToString`
- `isDigit`
- `join`
- `lines`
- `replaceAll`
- `repeat`
- `reverse`
- `split`
- `startsWith`
- `substring`
- `take`
- `toLower`
- `toUpper`
- `trim`
- `unlines`

Design notes:

- use `#s=0` instead of a dedicated `isEmpty`
- use `stdlib::string.trim(s)=""` instead of a dedicated whitespace predicate
- use `stdlib::string.indexOf(s,search)≠-1` for containment checks

## Current Numeric Surface

`stdlib::numeric` currently exposes:

- `abs`
- `clamp`
- `divisible`
- `divmod`
- `gcd`
- `inRange`
- `isEven`
- `isNegative`
- `isNonNegative`
- `isOdd`
- `isPositive`
- `isPrime`
- `lcm`
- `max`
- `min`
- `mod`
- `pow`
- `range`
- `sign`

Examples:

```sigil program
i stdlib::numeric

λmain()=>Bool=stdlib::numeric.abs(-5)=5 and stdlib::numeric.isEven(4) and stdlib::numeric.isPrime(17) and stdlib::numeric.range(2,5)=[2,3,4,5]
```

## Core Prelude

`ConcurrentOutcome[T,E]`, `Option[T]`, `Result[T,E]`, `Aborted`, `Failure`,
`Success`, `Some`, `None`, `Ok`, and `Err` are part of the implicit
`core::prelude`. They do not require imports.

Current canonical type forms:

```sigil module
t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)

t Option[T]=Some(T)|None()

t Result[T,E]=Ok(T)|Err(E)
```

Typical usage:

```sigil module
λgetOrDefault(default:Int,opt:Option[Int])=>Int match opt{
  Some(value)=>value|
  None()=>default
}

λprocessResult(res:Result[String,String])=>String match res{
  Ok(value)=>"Success: "++value|
  Err(msg)=>"Error: "++msg
}
```

## Core Map

`core::map` is the canonical helper surface for `{K↦V}` values.

Canonical type and literal forms:

```sigil module
t Headers={String↦String}

c empty=(({↦}:{String↦String}):{String↦String})

c filled=({"content-type"↦"text/plain"}:{String↦String})
```

Canonical helper import:

```sigil module
i core::map
```

## Stability Note

This document describes the current shipped stdlib surface. Placeholder future APIs and older snake_case names are intentionally omitted here. When the surface changes, update the checked declarations and examples in this file instead of keeping speculative or legacy aliases around.
