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
- ⏳ Regex utilities
- ⏳ Crypto utilities

## Import Syntax

```sigil
⟦ Import modules (works like FFI - no selective imports) ⟧
i stdlib::list
i stdlib::json
i stdlib::file
i stdlib::numeric
i stdlib::path
i stdlib::process
i stdlib::string
i stdlib::time
i stdlib::url
i stdlib::httpClient
i stdlib::httpServer

⟦ Use with fully qualified names ⟧
λmain()=>Unit=console.log(
  stdlib::string.intToString(#[1,2,3]) ++ " " ++
  stdlib::time.formatIso(stdlib::time.fromEpochMillis(0))
)
```

**Design:** Imports work exactly like FFI (`e module::path`). No selective imports, always use fully qualified names. This prevents name collisions and makes code explicit.

## Length Operator (`#`)

The `#` operator is a **built-in language operator** that returns the length of strings and lists.

**Syntax:**
```sigil
#expression => Int
```

**Type Checking:**
- Works on strings (`String`) and lists (`[T]`)
- Compile error for other types
- Always returns integer (`Int`)

**Examples:**
```sigil
#"hello"        ⟦ => 5 ⟧
#""             ⟦ => 0 ⟧
#[1,2,3]        ⟦ => 3 ⟧
#[]             ⟦ => 0 (empty list type inferred from context) ⟧
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

```sigil
i stdlib::file
i stdlib::path

λmain()=>!IO Unit=
  l out=(stdlib::path.join("/tmp","sigil.txt"):String);
  l _=(stdlib::file.writeText("hello",out):Unit);
  l _2=(stdlib::file.readText(out):String);
  ()
```

It also exposes `makeTempDir(prefix)` for canonical temp workspace creation in
tooling and harness code.

`stdlib::path` exposes canonical filesystem path operations:

```sigil
i stdlib::path

λmain()=>Unit=
  l _=(stdlib::path.basename("website/articles/hello.md"):String);
  l _2=(stdlib::path.join("website","articles"):String);
  ()
```

`stdlib::process` exposes canonical argv-based child-process execution:

```sigil
i stdlib::process

λmain()=>!IO Unit=
  l command=(stdlib::process.command(["git","status"]):stdlib::process.Command);
  l result=(stdlib::process.run(command):stdlib::process.ProcessResult);
  match result.code=0{
    true=>()|
    false=>()
  }
```

The canonical process surface is:
- `command`
- `withCwd`
- `withEnv`
- `run`
- `spawn`
- `wait`
- `kill`

Commands are argv-based only. Non-zero exit status is returned in
`ProcessResult.code`; it is not a separate failure channel.

`stdlib::json` exposes a typed JSON AST with safe parsing:

```sigil
i stdlib::json

λmain()=>Unit=
  match stdlib::json.parse("{\"ok\":true}"){
    Ok(value)=>match stdlib::json.asObject(value){
      Some(_)=>()|
      None()=>()
    }|
    Err(_)=>()
  }
```

`stdlib::decode` is the canonical layer for turning raw `JsonValue` into trusted
internal Sigil values:

```sigil
i stdlib::decode
i stdlib::json
i stdlib::time

t Message={createdAt:stdlib::time.Instant,text:String}

λinstant(value:stdlib::json.JsonValue)=>Result[stdlib::time.Instant,stdlib::decode.DecodeError] match stdlib::decode.string(value){
  Ok(text)=>
    match stdlib::time.parseIso(text){
      Ok(instant)=>Ok(instant)|
      Err(error)=>Err({message:error.message,path:[]})
    }|
  Err(error)=>Err(error)
}

λmessage(value:stdlib::json.JsonValue)=>Result[Message,stdlib::decode.DecodeError] match stdlib::decode.field(instant,"createdAt")(value){
  Ok(createdAt)=>
    match stdlib::decode.field(stdlib::decode.string,"text")(value){
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

```sigil
i stdlib::time

λmain()=>Unit=
  match stdlib::time.parseIso("2026-03-03"){
    Ok(instant)=>
      l _=(stdlib::time.toEpochMillis(instant):Int);
      ()|
    Err(_)=>()
  }
```

Effectful code may also use `stdlib::time.sleepMs(ms)` for retry loops and
process orchestration.

`stdlib::url` exposes strict parse results and typed URL fields for both absolute and relative targets:

```sigil
i stdlib::url

λmain()=>Unit=
  match stdlib::url.parse("../language/spec/cli-json.md?view=raw#schema"){
    Ok(url)=>
      l _=(url.path:String);
      l _2=(stdlib::url.suffix(url):String);
      ()|
    Err(_)=>()
  }
```

## HTTP Client and Server

`stdlib::httpClient` is the canonical text-based HTTP client layer.

For topology-aware projects, the canonical surface is handle-based rather than
raw-URL based:

```sigil
i stdlib::httpClient
i src::topology

λmain()=>!IO Unit=
  match stdlib::httpClient.get(
    src::topology.mailerApi,
    stdlib::httpClient.emptyHeaders(),
    "/health"
  ){
    Ok(response)=>
      l _=(response.body:String);
      ()|
    Err(error)=>
      l _=(error.message:String);
      ()
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

```sigil
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

```sigil
i src::topology
i stdlib::tcpClient

λmain()=>!IO Unit=
  match stdlib::tcpClient.send(src::topology.eventStream,"ping"){
    Ok(response)=>
      l _=(response.message:String);
      ()|
    Err(error)=>
      l _=(error.message:String);
      ()
  }
```

The canonical framing model is:
- UTF-8 text only
- one newline-delimited request per connection
- one newline-delimited response per connection

`stdlib::topology` owns the dependency handles.
`stdlib::config` owns per-environment bindings in `config/*.lib.sigil`.

`stdlib::tcpServer` is the matching minimal TCP server layer:

```sigil
i stdlib::tcpServer

λhandle(request:stdlib::tcpServer.Request)=>!IO stdlib::tcpServer.Response=
  stdlib::tcpServer.response(request.message)

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

```sigil
i src::topology
i stdlib::httpClient

λmain()=>!IO Unit=
  match stdlib::httpClient.get(src::topology.mailerApi,stdlib::httpClient.emptyHeaders(),"/health"){
    Ok(_)=>()|
    Err(_)=>()
  }
```

See [topology.md](/Users/jnobreganetto/Documents/GitHub/ai-pl/language/docs/topology.md) for the full model.

## List Predicates

**Module:** `stdlib/list`

### sorted_asc

Check if a list is sorted in ascending order.

```sigil
λsorted_asc(xs:[Int])=>Bool
```

**Examples:**
```sigil
sorted_asc([1,2,3])    ⟦ => true ⟧
sorted_asc([3,2,1])    ⟦ => false ⟧
sorted_asc([])         ⟦ => true (empty is sorted) ⟧
sorted_asc([5])        ⟦ => true (single element is sorted) ⟧
```

**Use case:** Validate precondition for binary search or other sorted-list algorithms.

### sorted_desc

Check if a list is sorted in descending order.

```sigil
λsorted_desc(xs:[Int])=>Bool
```

**Examples:**
```sigil
sorted_desc([3,2,1])   ⟦ => true ⟧
sorted_desc([1,2,3])   ⟦ => false ⟧
```

### all

Check if all elements in a list satisfy a predicate.

```sigil
λall[T](pred:λ(T)=>Bool,xs:[T])=>Bool
```

**Examples:**
```sigil
all(is_positive,[1,2,3])      ⟦ => true ⟧
all(is_positive,[1,-2,3])     ⟦ => false ⟧
all(is_even,[2,4,6])          ⟦ => true ⟧
```

**Use case:** Validate that all elements meet a requirement.

### any

Check if any element in a list satisfies a predicate.

```sigil
λany[T](pred:λ(T)=>Bool,xs:[T])=>Bool
```

**Examples:**
```sigil
any(is_even,[1,3,5])          ⟦ => false ⟧
any(is_even,[1,2,3])          ⟦ => true ⟧
any(is_prime,[4,6,8,7])       ⟦ => true (7 is prime) ⟧
```

**Use case:** Check if at least one element meets a requirement.

### contains

Check if an element exists in a list.

```sigil
λcontains(item:Int,xs:[Int])=>Bool
```

**Examples:**
```sigil
contains(3,[1,2,3,4])         ⟦ => true ⟧
contains(5,[1,2,3,4])         ⟦ => false ⟧
contains(1,[])                ⟦ => false ⟧
```

**Use case:** Membership testing.

### count

Count occurrences of an element in a list.

```sigil
λcount(item:Int,xs:[Int])=>Int
```

### drop

Drop the first `n` elements.

```sigil
λdrop(n:Int,xs:[Int])=>[Int]
```

### find

Find the first element that satisfies a predicate.

```sigil
λfind[T](pred:λ(T)=>Bool,xs:[T])=>Option[T]
```

Examples:
```sigil
stdlib::list.find(stdlib::numeric.is_even,[1,3,4,6])   ⟦ => Some(4) ⟧
stdlib::list.find(stdlib::numeric.is_even,[1,3,5])     ⟦ => None() ⟧
```

### fold

Reduce a list to a single value by threading an accumulator from left to right.

```sigil
λfold[T,U](acc:U,fn:λ(U,T)=>U,xs:[T])=>U
```

Examples:
```sigil
stdlib::list.fold(0,λ(acc:Int,x:Int)=>Int=acc+x,[1,2,3])   ⟦ => 6 ⟧
stdlib::list.fold(0,λ(acc:Int,x:Int)=>Int=acc*10+x,[1,2,3]) ⟦ => 123 ⟧
```

### in_bounds

Check if an index is valid for a list (in range [0, len-1]).

```sigil
λin_bounds(idx:Int,xs:[Int])=>Bool
```

**Examples:**
```sigil
in_bounds(0,[1,2,3])          ⟦ => true ⟧
in_bounds(2,[1,2,3])          ⟦ => true ⟧
in_bounds(3,[1,2,3])          ⟦ => false (out of bounds) ⟧
in_bounds(-1,[1,2,3])         ⟦ => false (negative index) ⟧
in_bounds(0,[])               ⟦ => false (empty list) ⟧
```

**Use case:** Validate array/list access before indexing. Prevents out-of-bounds errors.

**Implementation:** Uses `#xs` to check bounds.

## List Utilities

**Module:** `stdlib/list`

**Note:** Use the `#` operator for list length instead of a function (e.g., `#[1,2,3]` => `3`).

### last

Get the last element safely.

```sigil
λlast[T](xs:[T])=>Option[T]
```

Examples:
```sigil
stdlib::list.last([])         ⟦ => None() ⟧
stdlib::list.last([1,2,3])    ⟦ => Some(3) ⟧
```

### max

Get the maximum element safely.

```sigil
λmax(xs:[Int])=>Option[Int]
```

Examples:
```sigil
stdlib::list.max([])          ⟦ => None() ⟧
stdlib::list.max([3,9,4])     ⟦ => Some(9) ⟧
```

### min

Get the minimum element safely.

```sigil
λmin(xs:[Int])=>Option[Int]
```

Examples:
```sigil
stdlib::list.min([])          ⟦ => None() ⟧
stdlib::list.min([3,9,4])     ⟦ => Some(3) ⟧
```

### nth

Get the item at a zero-based index safely.

```sigil
λnth[T](idx:Int,xs:[T])=>Option[T]
```

Examples:
```sigil
stdlib::list.nth(0,[7,8])     ⟦ => Some(7) ⟧
stdlib::list.nth(2,[7,8])     ⟦ => None() ⟧
```

### product

Multiply all integers in a list.

```sigil
λproduct(xs:[Int])=>Int
```

Examples:
```sigil
stdlib::list.product([])         ⟦ => 1 ⟧
stdlib::list.product([2,3,4])    ⟦ => 24 ⟧
```

### remove_first

Remove the first occurrence of an element.

```sigil
λremove_first(item:Int,xs:[Int])=>[Int]
```

### reverse

Reverse a list.

```sigil
λreverse(xs:[Int])=>[Int]
```

### sum

Sum all integers in a list.

```sigil
λsum(xs:[Int])=>Int
```

Examples:
```sigil
stdlib::list.sum([])          ⟦ => 0 ⟧
stdlib::list.sum([1,2,3,4])   ⟦ => 10 ⟧
```

### take

Take the first `n` elements.

```sigil
λtake(n:Int,xs:[Int])=>[Int]
```

## Numeric Helpers

**Module:** `stdlib/numeric`

### range

Build an ascending integer range, inclusive at both ends.

```sigil
λrange(start:Int,stop:Int)=>[Int]
```

Examples:
```sigil
stdlib::numeric.range(2,5)   ⟦ => [2,3,4,5] ⟧
stdlib::numeric.range(3,3)   ⟦ => [3] ⟧
stdlib::numeric.range(5,2)   ⟦ => [] ⟧
```

## Canonical List-Processing Surface

For ordinary list work, Sigil expects the canonical operators and stdlib path,
not hand-rolled recursive plumbing:

- use `stdlib::list.all` for universal checks
- use `stdlib::list.any` for existential checks
- use `↦` for projection
- use `⊳` for filtering
- use `stdlib::list.find` for first-match search
- use `⊕` or `stdlib::list.fold` for reduction
- use `stdlib::list.reverse` for reversal

Sigil now rejects exact recursive clones of `all`, `any`, `map`, `filter`,
`find`, `fold`, and `reverse`, as well as recursive result-building of the form
`self(rest)⧺rhs`.

## String Operations

**Module:** `stdlib/string`

Comprehensive string manipulation functions. These are **compiler intrinsics** - the compiler emits optimized JavaScript directly instead of calling Sigil functions.

### char_at

Get character at index.

```sigil
λchar_at(idx:Int,s:String)=>String
```

**Examples:**
```sigil
stdlib::string.char_at(0,"hello")    ⟦ => "h" ⟧
stdlib::string.char_at(4,"hello")    ⟦ => "o" ⟧
```

**Codegen:** `s.charAt(idx)`

### substring

Get substring from start to end index.

```sigil
λsubstring(end:Int,s:String,start:Int)=>String
```

**Examples:**
```sigil
stdlib::string.substring(11,"hello world",6)    ⟦ => "world" ⟧
stdlib::string.substring(3,"hello",0)           ⟦ => "hel" ⟧
```

**Codegen:** `s.substring(start, end)`

### take

Take first n characters.

```sigil
λtake(n:Int,s:String)=>String
```

**Examples:**
```sigil
stdlib::string.take(3,"hello")    ⟦ => "hel" ⟧
stdlib::string.take(5,"hi")       ⟦ => "hi" (takes available chars) ⟧
```

**Implementation:** `substring(n, s, 0)` (in Sigil)

### drop

Drop first n characters.

```sigil
λdrop(n:Int,s:String)=>String
```

**Examples:**
```sigil
stdlib::string.drop(2,"hello")    ⟦ => "llo" ⟧
stdlib::string.drop(5,"hi")       ⟦ => "" (drops all available) ⟧
```

**Implementation:** `substring(#s, s, n)` (in Sigil, uses `#` operator)

### lines

Split a string on newline characters.

```sigil
λlines(s:String)=>[String]
```

**Examples:**
```sigil
stdlib::string.lines("a\nb\nc")    ⟦ => ["a","b","c"] ⟧
stdlib::string.lines("hello")      ⟦ => ["hello"] ⟧
```

**Implementation:** `split("\n", s)` (in Sigil)

### to_upper

Convert to uppercase.

```sigil
λto_upper(s:String)=>String
```

**Examples:**
```sigil
stdlib::string.to_upper("hello")    ⟦ => "HELLO" ⟧
```

**Codegen:** `s.toUpperCase()`

### to_lower

Convert to lowercase.

```sigil
λto_lower(s:String)=>String
```

**Examples:**
```sigil
stdlib::string.to_lower("WORLD")    ⟦ => "world" ⟧
```

**Codegen:** `s.toLowerCase()`

### trim

Remove leading and trailing whitespace.

```sigil
λtrim(s:String)=>String
```

**Examples:**
```sigil
stdlib::string.trim("  hello  ")    ⟦ => "hello" ⟧
stdlib::string.trim("\n\ttest\n")   ⟦ => "test" ⟧
```

**Codegen:** `s.trim()`

### index_of

Find index of first occurrence (returns -1 if not found).

```sigil
λindex_of(s:String,search:String)=>Int
```

**Examples:**
```sigil
stdlib::string.index_of("hello world","world")    ⟦ => 6 ⟧
stdlib::string.index_of("hello","xyz")            ⟦ => -1 ⟧
```

**Codegen:** `s.indexOf(search)`

### split

Split string by delimiter.

```sigil
λsplit(delimiter:String,s:String)=>[String]
```

**Examples:**
```sigil
stdlib::string.split(",","a,b,c")           ⟦ => ["a","b","c"] ⟧
stdlib::string.split("\n","line1\nline2")   ⟦ => ["line1","line2"] ⟧
```

**Codegen:** `s.split(delimiter)`

### replace_all

Replace all occurrences of pattern with replacement.

```sigil
λreplace_all(pattern:String,replacement:String,s:String)=>String
```

**Examples:**
```sigil
stdlib::string.replace_all("hello","hi","hello hello")    ⟦ => "hi hi" ⟧
```

**Codegen:** `s.replaceAll(pattern, replacement)`

### repeat

Repeat a string `count` times.

```sigil
λrepeat(count:Int,s:String)=>String
```

**Examples:**
```sigil
stdlib::string.repeat(3,"ab")    ⟦ => "ababab" ⟧
stdlib::string.repeat(0,"ab")    ⟦ => "" ⟧
```

**Implementation:** recursive concatenation in Sigil

## String Predicates

**Module:** `stdlib/string`

Boolean validation predicates for string properties. These are **compiler intrinsics**.

### starts_with

Check if string starts with prefix.

```sigil
λstarts_with(prefix:String,s:String)=>Bool
```

**Examples:**
```sigil
stdlib::string.starts_with("# ","# Title")    ⟦ => true ⟧
stdlib::string.starts_with("# ","Title")      ⟦ => false ⟧
```

**Codegen:** `s.startsWith(prefix)`

**Use case:** Parse markdown headers, check file extensions.

### ends_with

Check if string ends with suffix.

```sigil
λends_with(s:String,suffix:String)=>Bool
```

**Examples:**
```sigil
stdlib::string.ends_with("test.sigil",".sigil")    ⟦ => true ⟧
stdlib::string.ends_with("test.txt",".sigil")      ⟦ => false ⟧
```

**Codegen:** `s.endsWith(suffix)`

**Use case:** File extension checking, URL validation.

### is_digit

Check whether a string is exactly one decimal digit.

```sigil
λis_digit(s:String)=>Bool
```

**Examples:**
```sigil
stdlib::string.is_digit("5")     ⟦ => true ⟧
stdlib::string.is_digit("42")    ⟦ => false ⟧
```

**Codegen:** `/^[0-9]$/.test(s)`

### unlines

Join lines with newline separators.

```sigil
λunlines(lines:[String])=>String
```

**Examples:**
```sigil
stdlib::string.unlines(["a","b","c"])    ⟦ => "a\nb\nc" ⟧
stdlib::string.unlines([])               ⟦ => "" ⟧
```

**Implementation:** `join("\n", lines)` (in Sigil)

**Design Note:** No redundant predicates like `is_empty`, `is_whitespace`, or `contains`. Users compose these:
- `is_empty(s)` => `#s = 0`
- `is_whitespace(s)` => `stdlib::string.trim(s) = ""`
- `contains(s, search)` => `stdlib::string.index_of(s, search) ≠ -1`

This follows Sigil's "ONE way to do things" philosophy.

## Numeric Predicates

**Module:** `stdlib/numeric`

### abs

Absolute value of an integer.

```sigil
λabs(x:Int)=>Int
```

Examples:
```sigil
stdlib::numeric.abs(-5)   ⟦ => 5 ⟧
stdlib::numeric.abs(7)    ⟦ => 7 ⟧
```

### DivMod

Quotient and remainder pair returned by `divmod`.

```sigil
t DivMod={quotient:Int,remainder:Int}
```

### divmod

Return integer quotient and Euclidean remainder together.

```sigil
λdivmod(a:Int,b:Int)=>stdlib::numeric.DivMod
```

Examples:
```sigil
stdlib::numeric.divmod(17,5)    ⟦ => DivMod{quotient:3,remainder:2} ⟧
stdlib::numeric.divmod(-17,5)   ⟦ => DivMod{quotient:-4,remainder:3} ⟧
```

### is_positive

Check if a number is positive (> 0).

```sigil
λis_positive(x:Int)=>Bool
```

**Examples:**
```sigil
is_positive(5)                ⟦ => true ⟧
is_positive(-3)               ⟦ => false ⟧
is_positive(0)                ⟦ => false ⟧
```

### is_negative

Check if a number is negative (< 0).

```sigil
λis_negative(x:Int)=>Bool
```

**Examples:**
```sigil
is_negative(-5)               ⟦ => true ⟧
is_negative(3)                ⟦ => false ⟧
is_negative(0)                ⟦ => false ⟧
```

### is_non_negative

Check if a number is non-negative (>= 0).

```sigil
λis_non_negative(x:Int)=>Bool
```

**Examples:**
```sigil
is_non_negative(0)            ⟦ => true ⟧
is_non_negative(5)            ⟦ => true ⟧
is_non_negative(-1)           ⟦ => false ⟧
```

### is_even

Check if a number is even.

```sigil
λis_even(x:Int)=>Bool
```

**Examples:**
```sigil
is_even(4)                    ⟦ => true ⟧
is_even(5)                    ⟦ => false ⟧
is_even(0)                    ⟦ => true ⟧
```

### is_odd

Check if a number is odd.

```sigil
λis_odd(x:Int)=>Bool
```

**Examples:**
```sigil
is_odd(3)                     ⟦ => true ⟧
is_odd(4)                     ⟦ => false ⟧
```

**Implementation:** Uses negation of `is_even` for correctness.

### is_prime

Check if a number is prime.

```sigil
λis_prime(n:Int)=>Bool
```

**Examples:**
```sigil
is_prime(2)                   ⟦ => true ⟧
is_prime(7)                   ⟦ => true ⟧
is_prime(8)                   ⟦ => false ⟧
is_prime(17)                  ⟦ => true ⟧
is_prime(1)                   ⟦ => false (1 is not prime) ⟧
is_prime(0)                   ⟦ => false ⟧
```

**Algorithm:** Trial division up to sqrt(n). Uses helper function `is_prime_helper`.

**Performance:** O(sqrt(n)) time complexity.

### lcm

Least common multiple.

```sigil
λlcm(a:Int,b:Int)=>Int
```

Examples:
```sigil
stdlib::numeric.lcm(6,8)     ⟦ => 24 ⟧
stdlib::numeric.lcm(-6,8)    ⟦ => 24 ⟧
stdlib::numeric.lcm(0,7)     ⟦ => 0 ⟧
```

### mod

Euclidean modulo with a non-negative remainder.

```sigil
λmod(a:Int,b:Int)=>Int
```

Examples:
```sigil
stdlib::numeric.mod(17,5)     ⟦ => 2 ⟧
stdlib::numeric.mod(-17,5)    ⟦ => 3 ⟧
stdlib::numeric.mod(17,-5)    ⟦ => 2 ⟧
```

### in_range

Check if a number is in the inclusive range [min, max].

```sigil
λin_range(x:Int,min:Int,max:Int)=>Bool
```

**Examples:**
```sigil
in_range(5,1,10)              ⟦ => true ⟧
in_range(0,1,10)              ⟦ => false ⟧
in_range(1,1,10)              ⟦ => true (inclusive bounds) ⟧
in_range(10,1,10)             ⟦ => true (inclusive bounds) ⟧
```

**Use case:** Bounds validation, input checking.

### sign

Return `-1`, `0`, or `1` based on the sign of the input.

```sigil
λsign(x:Int)=>Int
```

Examples:
```sigil
stdlib::numeric.sign(-8)    ⟦ => -1 ⟧
stdlib::numeric.sign(0)     ⟦ => 0 ⟧
stdlib::numeric.sign(12)    ⟦ => 1 ⟧
```

## Common Patterns

### Validation with Predicates

```sigil
⟦ Validate input before processing ⟧
λprocess_positive(x:Int)=>String match is_positive(x){
  false=>"Error: Must be positive"|
  true=>"Processing..."
}
```

### Filtering Lists

```sigil
⟦ Filter primes from a list ⟧
λget_primes(xs:[Int])=>[Int]=xs⊳is_prime
```

### Higher-Order Validation

```sigil
⟦ Check all values are in range ⟧
λall_in_range(xs:[Int])=>Bool=all(λx=>in_range(x,0,100),xs)
```

### Precondition Checks

```sigil
⟦ Algorithm that requires sorted input ⟧
λbinary_search(xs:[Int],target:Int)=>String match sorted_asc(xs){
  false=>"Error: List must be sorted"|
  true=>"Searching..."
}
```

## Design Principles

### Canonical Forms Only

Each predicate has exactly ONE implementation:
- ❌ NO iterative versions
- ❌ NO accumulator-passing variants
- ✅ ONLY primitive recursion

### Helper Functions Allowed

Predicates can use helper functions for complex logic:
```sigil
λis_prime(n:Int)=>Bool=...
λis_prime_helper(n:Int,divisor:Int)=>Bool=...  ⟦ Allowed ⟧
```

### Pure Functions

All predicates are pure (no side effects):
- Same input always produces same output
- No mutation
- No I/O
- No state

### Type Safety

All predicates have explicit type signatures:
- Parameter types declared
- Return types declared
- No type inference needed

## Limitations & Known Issues

### ~~Module Imports Not Working~~ ✅ FIXED

**Issue:** ~~Imports don't currently register in the typechecker.~~

**Resolution:** Module imports now fully working. Use like FFI: `i stdlib::module` then `stdlib::module.function()`.

### ~~Missing Unicode Operators~~ ✅ FIXED

**Issue:** ~~Typechecker doesn't support ≤, ≥, ≠, and, or.~~

**Resolution:** Unicode operators now fully supported in typechecker. Predicates updated to use cleaner Unicode syntax.

## Core Prelude

`Option[T]`, `Result[T,E]`, `Some`, `None`, `Ok`, and `Err` are part of the implicit `core::prelude`. They do not require imports.

### Option[T]

Represents an optional value - either `Some(T)` or `None`.

**Type declaration:**
```sigil
t Option[T]=Some(T)|None
```

**Usage:**
```sigil
⟦ Pattern matching on Option ⟧
λgetOrDefault(default:Int,opt:Option[Int])=>Int match opt{
  Some(x)=>x|
  None()=>default
}

⟦ Safe division returning Option ⟧
λdivide(a:Int,b:Int)=>Option[Int] match b{
  0=>None()|
  b=>Some(a/b)
}
```

**Implemented helpers:**
```sigil
λbind_option[T,U](fn:λ(T)=>Option[U],opt:Option[T])=>Option[U]
λis_none[T](opt:Option[T])=>Bool
λis_some[T](opt:Option[T])=>Bool
λmap_option[T,U](fn:λ(T)=>U,opt:Option[T])=>Option[U]
λunwrap_or[T](fallback:T,opt:Option[T])=>T
```

### Result[T,E]

Represents success (`Ok(T)`) or failure (`Err(E)`).

**Type declaration:**
```sigil
t Result[T,E]=Ok(T)|Err(E)
```

**Usage:**
```sigil
⟦ Pattern matching on Result ⟧
λprocessResult(res:Result[String,String])=>String match res{
  Ok(value)=>"Success: "+value|
  Err(msg)=>"Error: "+msg
}

⟦ Safe parsing returning Result ⟧
λparsePositive(s:String)=>Result[Int,String] match validInput(s){
  true=>Ok(parseInt(s))|
  false=>Err("invalid input")
}
```

**Implemented helpers:**
```sigil
λbind_result[T,U,E](fn:λ(T)=>Result[U,E],res:Result[T,E])=>Result[U,E]
λis_err[T,E](res:Result[T,E])=>Bool
λis_ok[T,E](res:Result[T,E])=>Bool
λmap_result[T,U,E](fn:λ(T)=>U,res:Result[T,E])=>Result[U,E]
λunwrap_or_result[T,E](fallback:T,res:Result[T,E])=>T
```

**See also:** `examples/sumTypesDemo.sigil` for comprehensive examples.

### Core Helper Modules

Use these when you need operational helpers:

```sigil
i core::map
i core::option
i core::result
```

### Core Map

`Map` is a core collection concept, not a stdlib-only add-on.

Canonical type and literal forms:

```sigil
{String↦String}
{"content-type"↦"text/plain"}
({↦}:{String↦String})
```

Canonical helper module:

```sigil
i core::map
```

## Future Additions

### String Predicates

```sigil
λstr_contains(s:String,substr:String)=>Bool
λstr_starts_with(s:String,prefix:String)=>Bool
λstr_ends_with(s:String,suffix:String)=>Bool
λstr_is_empty(s:String)=>Bool
```

### List Utility Functions

```sigil
λlen[T](xs:[T])=>Int
λhead[T](xs:[T])=>Option[T]
λtail[T](xs:[T])=>[T]
λreverse[T](xs:[T])=>[T]
```

## Contracts (Future)

Predicates will integrate with the future contract system:

```sigil
⟦ Today - manual validation ⟧
λbinary_search(xs:[Int],target:Int)=>Int match sorted_asc(xs){
  false=>-1|
  true=>...
}

⟦ Future - contracts with predicates ⟧
λbinary_search(xs:[Int],target:Int)=>Int
  [requires sorted_asc(xs)]
  [ensures in_range(result,0,len(xs))]
=...
```

This ensures predicates are useful TODAY while setting foundation for formal verification later.

---

**See also:**
- `spec/stdlib-spec.md` - Full standard library specification
- `examples/` - Example programs using predicates
- `AGENTS.md` - Sigil language guide
