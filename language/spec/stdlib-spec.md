# Sigil Standard Library Specification

Version: 1.0.0
Last Updated: 2026-03-07

## Overview

The Sigil standard library provides essential types and functions that are automatically available in every Sigil program. The design philosophy emphasizes:

1. **Minimal but complete** - Only include truly universal functionality
2. **Functional-first** - Pure functions, immutability by default
3. **Type-safe** - Leverage strong type system
4. **Composable** - Functions that work well together
5. **Zero-cost abstractions** - Compile to efficient JavaScript

## Automatic Imports

The prelude is automatically imported into every Sigil module. No explicit import needed.

## Core Types

### Option[T]

Represents an optional value - Sigil's null-safe alternative.

```sigil
t Option[T]=Some(T)|None
```

**Constructors:**
- `Some[T](value:T)→Option[T]` - Wraps a value
- `None[T]()→Option[T]` - Represents absence

**Functions:**

```sigil
λmap_option[T,U](fn:λ(T)→U,opt:Option[T])→Option[U] match opt{Some(v)→Some(fn(v))|None()→None()}
λbind_option[T,U](fn:λ(T)→Option[U],opt:Option[T])→Option[U] match opt{Some(v)→fn(v)|None()→None()}
λunwrap_or[T](fallback:T,opt:Option[T])→T match opt{Some(v)→v|None()→fallback}
λis_some[T](opt:Option[T])→Bool match opt{Some(_)→true|None()→false}
λis_none[T](opt:Option[T])→Bool match opt{Some(_)→false|None()→true}
```

### Result[T,E]

Represents a computation that may fail - Sigil's exception-free error handling.

```sigil
t Result[T,E]=Ok(T)|Err(E)
```

**Constructors:**
- `Ok[T,E](value:T)→Result[T,E]` - Success case
- `Err[T,E](error:E)→Result[T,E]` - Error case

**Functions:**

```sigil
λmap_result[T,U,E](fn:λ(T)→U,res:Result[T,E])→Result[U,E] match res{Ok(v)→Ok(fn(v))|Err(e)→Err(e)}
λbind_result[T,U,E](fn:λ(T)→Result[U,E],res:Result[T,E])→Result[U,E] match res{Ok(v)→fn(v)|Err(e)→Err(e)}
λunwrap_or_result[T,E](fallback:T,res:Result[T,E])→T match res{Ok(v)→v|Err(_)→fallback}
λis_ok[T,E](res:Result[T,E])→Bool match res{Ok(_)→true|Err(_)→false}
λis_err[T,E](res:Result[T,E])→Bool match res{Ok(_)→false|Err(_)→true}
```

## List Operations

### Implemented `stdlib⋅list` Functions

```sigil
λcontains[T](item:T,xs:[T])→Bool
λcount[T](item:T,xs:[T])→Int
λdrop[T](n:Int,xs:[T])→[T]
λfind[T](pred:λ(T)→Bool,xs:[T])→Option[T]
λfold[T,U](acc:U,fn:λ(U,T)→U,xs:[T])→U
λin_bounds[T](idx:Int,xs:[T])→Bool
λlast[T](xs:[T])→Option[T]
λmax(xs:[Int])→Option[Int]
λmin(xs:[Int])→Option[Int]
λnth[T](idx:Int,xs:[T])→Option[T]
λproduct(xs:[Int])→Int
λremove_first[T](item:T,xs:[T])→[T]
λreverse[T](xs:[T])→[T]
λsorted_asc(xs:[Int])→Bool
λsorted_desc(xs:[Int])→Bool
λsum(xs:[Int])→Int
λtake[T](n:Int,xs:[T])→[T]
```

Safe element access uses `Option[T]`:
- `last([])→None()`
- `find(pred,[])→None()`
- `max([])→None()`
- `min([])→None()`
- `nth(-1,xs)→None()`
- `nth(idx,xs)→None()` when out of bounds

### Implemented `stdlib⋅numeric` Helpers

```sigil
t DivMod={quotient:Int,remainder:Int}
λabs(x:Int)→Int
λclamp(hi:Int,lo:Int,x:Int)→Int
λdivisible(d:Int,n:Int)→Bool
λdivmod(a:Int,b:Int)→DivMod
λgcd(a:Int,b:Int)→Int
λin_range(max:Int,min:Int,x:Int)→Bool
λis_even(x:Int)→Bool
λis_negative(x:Int)→Bool
λis_non_negative(x:Int)→Bool
λis_odd(x:Int)→Bool
λis_positive(x:Int)→Bool
λis_prime(n:Int)→Bool
λlcm(a:Int,b:Int)→Int
λmax(a:Int,b:Int)→Int
λmin(a:Int,b:Int)→Int
λmod(a:Int,b:Int)→Int
λpow(base:Int,exp:Int)→Int
λrange(start:Int,stop:Int)→[Int]
λsign(x:Int)→Int
```

## String Operations

```sigil
λchar_at(idx:Int,s:String)→String
```
Get character at index.
- Complexity: O(1)
- Pure: Yes

```sigil
λdrop(n:Int,s:String)→String
```
Drop first `n` characters.
- Complexity: O(n)
- Pure: Yes

```sigil
λends_with(s:String,suffix:String)→Bool
```
Check if string ends with suffix.
- Complexity: O(n)
- Pure: Yes

```sigil
λindex_of(s:String,search:String)→Int
```
Find index of first occurrence, or `-1` if missing.
- Complexity: O(n)
- Pure: Yes

```sigil
λintToString(n:Int)→String
```
Convert an integer to a string.
- Complexity: O(n)
- Pure: Yes

```sigil
λis_digit(s:String)→Bool
```
Check whether a string is exactly one decimal digit.
- Complexity: O(1)
- Pure: Yes

```sigil
λjoin(separator:String,strings:[String])→String
```
Join strings with a separator.
- Complexity: O(n)
- Pure: Yes

```sigil
λlines(s:String)→[String]
```
Split a string on newline characters.
- Complexity: O(n)
- Pure: Yes

```sigil
λreplace_all(pattern:String,replacement:String,s:String)→String
```
Replace all occurrences of a pattern with a replacement string.
- Complexity: O(n)
- Pure: Yes

```sigil
λrepeat(count:Int,s:String)→String
```
Repeat a string `count` times.
- Complexity: O(n)
- Pure: Yes

```sigil
λsplit(delimiter:String,s:String)→[String]
```
Split a string by delimiter.
- Complexity: O(n)
- Pure: Yes

```sigil
λstarts_with(prefix:String,s:String)→Bool
```
Check if string starts with prefix.
- Complexity: O(n)
- Pure: Yes

```sigil
λsubstring(end:Int,s:String,start:Int)→String
```
Get substring from `start` to `end`.
- Complexity: O(n)
- Pure: Yes

```sigil
λtake(n:Int,s:String)→String
```
Take first `n` characters.
- Complexity: O(n)
- Pure: Yes

```sigil
λto_lower(s:String)→String
```
Convert to lowercase.
- Complexity: O(n)
- Pure: Yes

```sigil
λto_upper(s:String)→String
```
Convert to uppercase.
- Complexity: O(n)
- Pure: Yes

```sigil
λtrim(s:String)→String
```
Remove leading/trailing whitespace.
- Complexity: O(n)
- Pure: Yes

```sigil
λunlines(lines:[String])→String
```
Join lines with newline separators.
- Complexity: O(n)
- Pure: Yes

## Map Operations

```sigil
λempty[K,V]()→{K↦V}
```
Create empty map.
- Complexity: O(1)
- Pure: Yes

```sigil
λinsert[K,V](key:K,map:{K↦V},value:V)→{K↦V}
```
Insert key-value pair. Returns new map.
- Complexity: O(log n)
- Pure: Yes

```sigil
λget[K,V](key:K,map:{K↦V})→Option[V]
```
Get value for key.
- Complexity: O(log n)
- Pure: Yes

```sigil
λremove[K,V](key:K,map:{K↦V})→{K↦V}
```
Remove key. Returns new map.
- Complexity: O(log n)
- Pure: Yes

```sigil
λhas[K,V](key:K,map:{K↦V})→Bool
```
Check if key exists.
- Complexity: O(log n)
- Pure: Yes

```sigil
λkeys[K,V](map:{K↦V})→[K]
```
Get all keys.
- Complexity: O(n)
- Pure: Yes

```sigil
λvalues[K,V](map:{K↦V})→[V]
```
Get all values.
- Complexity: O(n)
- Pure: Yes

```sigil
λentries[K,V](map:{K↦V})→[(K,V)]
```
Get all key-value pairs.
- Complexity: O(n)
- Pure: Yes

## JSON Operations

```sigil
t JsonError={message:String}
t JsonValue=JsonArray([JsonValue])|JsonBool(Bool)|JsonNull|JsonNumber(Float)|JsonObject({String↦JsonValue})|JsonString(String)

λparse(input:String)→Result[JsonValue,JsonError]
λstringify(value:JsonValue)→String
λgetField(key:String,obj:{String↦JsonValue})→Option[JsonValue]
λgetIndex(arr:[JsonValue],idx:Int)→Option[JsonValue]
λasArray(value:JsonValue)→Option[[JsonValue]]
λasBool(value:JsonValue)→Option[Bool]
λasNumber(value:JsonValue)→Option[Float]
λasObject(value:JsonValue)→Option[{String↦JsonValue}]
λasString(value:JsonValue)→Option[String]
λisNull(value:JsonValue)→Bool
```

Notes:
- `parse` is exception-safe and returns `Err({message})` for invalid JSON.
- `stringify` is canonical JSON output for the provided `JsonValue`.

## Decode Operations

`stdlib⋅decode` is the canonical boundary layer from raw `JsonValue` to trusted
internal Sigil values.

```sigil
t DecodeError={message:String,path:[String]}
t Decoder[T]=λ(JsonValue)→Result[T,DecodeError]

λrun[T](decoder:Decoder[T],value:JsonValue)→Result[T,DecodeError]
λparse[T](decoder:Decoder[T],input:String)→Result[T,DecodeError]
λsucceed[T](value:T)→Decoder[T]
λfail[T](message:String)→Decoder[T]
λmap[T,U](decoder:Decoder[T],fn:λ(T)→U)→Decoder[U]
λbind[T,U](decoder:Decoder[T],fn:λ(T)→Decoder[U])→Decoder[U]

λbool(value:JsonValue)→Result[Bool,DecodeError]
λfloat(value:JsonValue)→Result[Float,DecodeError]
λint(value:JsonValue)→Result[Int,DecodeError]
λstring(value:JsonValue)→Result[String,DecodeError]

λlist[T](decoder:Decoder[T])→Decoder[[T]]
λdict[T](decoder:Decoder[T])→Decoder[{String↦T}]
λfield[T](decoder:Decoder[T],key:String)→Decoder[T]
λoptionalField[T](decoder:Decoder[T],key:String)→Decoder[Option[T]]
```

Notes:
- `stdlib⋅json` owns raw parsing and inspection.
- `stdlib⋅decode` owns conversion into trusted internal types.
- `DecodeError.path` records the nested field/index path of the failure.
- If a field may be absent, keep the record exact and use `Option[T]` for that field.
- Sigil does not use open records or partial records for this boundary story.

## Time Operations

```sigil
t Instant={epochMillis:Int}
t TimeError={message:String}

λparseIso(input:String)→Result[Instant,TimeError]
λformatIso(instant:Instant)→String
λnow()→!IO Instant
λfromEpochMillis(millis:Int)→Instant
λtoEpochMillis(instant:Instant)→Int
λcompare(left:Instant,right:Instant)→Int
λisBefore(left:Instant,right:Instant)→Bool
λisAfter(left:Instant,right:Instant)→Bool
```

Notes:
- `parseIso` is strict ISO-8601 only.
- Non-ISO text must be normalized before calling `parseIso`.

## Math Operations

```sigil
λabs(n:Int)→Int
```
Absolute value.
- Complexity: O(1)
- Pure: Yes

```sigil
λmin(a:Int,b:Int)→Int
```
Minimum of two integers.
- Complexity: O(1)
- Pure: Yes

```sigil
λmax(a:Int,b:Int)→Int
```
Maximum of two integers.
- Complexity: O(1)
- Pure: Yes

```sigil
λpow(base:Int,exp:Int)→Int
```
Exponentiation (integer power).
- Complexity: O(log exp)
- Pure: Yes

```sigil
λsqrt(n:Float)→Float
```
Square root.
- Complexity: O(1)
- Pure: Yes

```sigil
λfloor(n:Float)→Int
```
Round down to integer.
- Complexity: O(1)
- Pure: Yes

```sigil
λceil(n:Float)→Int
```
Round up to integer.
- Complexity: O(1)
- Pure: Yes

```sigil
λround(n:Float)→Int
```
Round to nearest integer.
- Complexity: O(1)
- Pure: Yes

## I/O Operations

All I/O operations have the `!IO` effect.

```sigil
λprint(s:String)→Unit!IO
```
Print string to stdout.
- Effect: IO
- Complexity: O(n)

```sigil
λprintln(s:String)→Unit!IO
```
Print string with newline.
- Effect: IO
- Complexity: O(n)

```sigil
λread_line()→String!IO
```
Read line from stdin.
- Effect: IO
- Complexity: O(n)

```sigil
λread_file(path:String)→Result[String,IoError]!IO
```
Read entire file as string.
- Effect: IO
- Complexity: O(file size)

```sigil
λwrite_file(path:String,content:String)→Result[Unit,IoError]!IO
```
Write string to file.
- Effect: IO
- Complexity: O(n)

## Error Handling

```sigil
t IoError={kind:String,msg:String}
t ParseError={column:Int,line:Int,msg:String}
```

```sigil
λpanic[T](msg:String)→T
```
Immediately terminate program with error message.
- Effect: Diverges (returns Never)
- Use sparingly - prefer Result for recoverable errors

```sigil
λassert(condition:Bool,msg:String)→Unit
```
Assert condition is true, panic if false.
- Effect: May diverge
- Use for invariants that should never be violated

## Type Conversion

```sigil
λintToString(n:Int)→String
```
Convert integer to string.
- Complexity: O(log n)
- Pure: Yes

```sigil
λstring_to_int(s:String)→Result[Int,ParseError]
```
Parse integer from string.
- Complexity: O(n)
- Pure: Yes

```sigil
λfloat_to_string(n:Float)→String
```
Convert float to string.
- Complexity: O(1)
- Pure: Yes

```sigil
λstring_to_float(s:String)→Result[Float,ParseError]
```
Parse float from string.
- Complexity: O(n)
- Pure: Yes

## Composition Operators

```sigil
λcompose[T,U,V](f:λ(U)→V,g:λ(T)→U)→λ(T)→V
```
Function composition: (f ∘ g)(x) = f(g(x))
- Operator: `>>`
- Pure: Yes

```sigil
λpipe[T,U](value:T,fn:λ(T)→U)→U
```
Pipe value through function.
- Operator: `|>`
- Pure: Yes

## Module System

### Import Syntax

```sigil
i stdlib⋅file
i stdlib⋅list
i stdlib⋅path
i core⋅result
```

### Export Visibility

File extension determines visibility:

**`.lib.sigil` files** (libraries):
- All top-level declarations are automatically visible to importers
- No `export` keyword needed or allowed

**`.sigil` files** (executables):
- Cannot be imported (except by test files in `tests/` directories)
- Have `main()` function

No selective imports, no aliasing, no export lists.

## Standard Library Modules

### core/prelude

Auto-imported. Contains the foundational vocabulary types:
- `Option[T]`
- `Result[T,E]`
- `Some`
- `None`
- `Ok`
- `Err`

### std/file

UTF-8 filesystem helpers:
- `appendText`
- `exists`
- `listDir`
- `makeDir`
- `makeDirs`
- `readText`
- `remove`
- `removeTree`
- `writeText`

### std/path

Filesystem path helpers:
- `basename`
- `dirname`
- `extname`
- `join`
- `normalize`
- `relative`

### std/io

Console and process I/O only (`print`, `println`, `eprintln`, `warn`, `debug`)

### std/collections

Advanced collections: Set[T], Queue[T], Stack[T]

### std/numeric

Mathematical functions: sin, cos, tan, log, exp, etc.

### std/json

Typed JSON parsing and serialization (`JsonValue`, `parse`, `stringify`)

```sigil
λparse(input:String)→Result[JsonValue,JsonError]
λstringify(value:JsonValue)→String
```

### std/decode

Canonical JSON-to-domain decoding (`Decoder[T]`, `DecodeError`, `run`, `parse`)

```sigil
λrun[T](decoder:Decoder[T],value:JsonValue)→Result[T,DecodeError]
λparse[T](decoder:Decoder[T],input:String)→Result[T,DecodeError]
```

### std/time

Time and instant handling (`Instant`, strict ISO parsing, clock access)

```sigil
λparseIso(input:String)→Result[Instant,TimeError]
λformatIso(instant:Instant)→String
λnow()→!IO Instant
```

### std/httpClient

Canonical text-based HTTP client.

```sigil
t Headers={String↦String}
t HttpError={kind:HttpErrorKind,message:String}
t HttpErrorKind=InvalidJson()|InvalidUrl()|Network()|Timeout()
t HttpMethod=Delete()|Get()|Patch()|Post()|Put()
t HttpRequest={body:Option[String],headers:Headers,method:HttpMethod,url:String}
t HttpResponse={body:String,headers:Headers,status:Int,url:String}

λrequest(request:HttpRequest)→!IO Result[HttpResponse,HttpError]
λget(headers:Headers,url:String)→!IO Result[HttpResponse,HttpError]
λdelete(headers:Headers,url:String)→!IO Result[HttpResponse,HttpError]
λpost(body:String,headers:Headers,url:String)→!IO Result[HttpResponse,HttpError]
λput(body:String,headers:Headers,url:String)→!IO Result[HttpResponse,HttpError]
λpatch(body:String,headers:Headers,url:String)→!IO Result[HttpResponse,HttpError]

λgetJson(headers:Headers,url:String)→!IO Result[JsonValue,HttpError]
λdeleteJson(headers:Headers,url:String)→!IO Result[JsonValue,HttpError]
λpostJson(body:JsonValue,headers:Headers,url:String)→!IO Result[JsonValue,HttpError]
λputJson(body:JsonValue,headers:Headers,url:String)→!IO Result[JsonValue,HttpError]
λpatchJson(body:JsonValue,headers:Headers,url:String)→!IO Result[JsonValue,HttpError]
λresponseJson(response:HttpResponse)→Result[JsonValue,HttpError]

λemptyHeaders()→Headers
λjsonHeaders()→Headers
λheader(key:String,value:String)→Headers
λmergeHeaders(left:Headers,right:Headers)→Headers
```

Semantics:
- any successfully received HTTP response returns `Ok(HttpResponse)`, including `404` and `500`
- invalid URL, transport failure, and JSON parse failure return `Err(HttpError)`
- request and response bodies are UTF-8 text in v1

### std/httpServer

Canonical request/response HTTP server.

```sigil
t Headers={String↦String}
t Request={body:String,headers:Headers,method:String,path:String}
t Response={body:String,headers:Headers,status:Int}

λresponse(body:String,contentType:String,status:Int)→Response
λok(body:String)→Response
λjson(body:String,status:Int)→Response
λnotFound()→Response
λnotFoundMsg(message:String)→Response
λserverError(message:String)→Response
λlogRequest(request:Request)→!IO Unit
λserve(handler:λ(Request)→!IO Response,port:Int)→!IO Unit
```

`serve` is long-lived: once the server is listening, the process remains active
until it is terminated externally.

### std/test

Testing utilities

```sigil
λtest(name:String,fn:λ()→Unit)→Unit!Test
λassert_eq[T](expected:T,actual:T)→Unit
λassert_ne[T](a:T,b:T)→Unit
```

## Implementation Notes

### JavaScript Compilation

- Lists compile to JavaScript arrays
- Maps compile to JavaScript Map objects
- Strings are JavaScript strings (UTF-16)
- Integers are JavaScript numbers (beware 32-bit limits!)
- Floats are JavaScript numbers (IEEE 754 double)

### Performance Considerations

- List operations are functional (immutable) - use sparingly for large lists
- For performance-critical code, consider using mutable collections explicitly
- String concatenation in loops is O(n²) - prefer stdlib⋅string.join when building from parts

### Effect System

Effects are tracked at type level:
- `!IO` - Input/output operations
- `!Test` - Test operations
- Pure functions have no effect annotation

## Future Extensions

Planned for future stdlib versions:

- **std/regex** - Regular expressions
- **std/crypto** - Cryptographic functions
- **std/random** - Random number generation
- **std/stream** - Streaming I/O
- **std/concurrency** - Threads and channels

## See Also

- [Type System](type-system.md) - Type inference and checking
- [Grammar](grammar.ebnf) - Language syntax
- Implementation: core/prelude.lib.sigil

---

**Next**: Implement standard library in stdlib/ directory.
