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

## Implicit Prelude and Rooted Modules

The prelude is available in every Sigil module without qualification. Other
modules are reached through rooted references such as `§list`, `•topology`,
`†runtime`, `※check::log`, and `☴router`.

## Core Types

### ConcurrentOutcome[T,E]

Implicit core prelude sum type:

```sigil decl ¶prelude
t ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)
```

- `Aborted[T,E]()=>ConcurrentOutcome[T,E]`
- `Failure[T,E](error:E)=>ConcurrentOutcome[T,E]`
- `Success[T,E](value:T)=>ConcurrentOutcome[T,E]`

### Option[T]

Represents an optional value - Sigil's null-safe alternative.

```sigil module
t Option[T]=Some(T)|None()
```

**Constructors:**
- `Some[T](value:T)=>Option[T]` - Wraps a value
- `None[T]()=>Option[T]` - Represents absence

**Functions:**

```text
mapOption(fn,opt)
bindOption(fn,opt)
unwrapOr(fallback,opt)
isSome(opt)
isNone(opt)
```

### Result[T,E]

Represents a computation that may fail - Sigil's exception-free error handling.

```sigil module
t Result[T,E]=Ok(T)|Err(E)
```

**Constructors:**
- `Ok[T,E](value:T)=>Result[T,E]` - Success case
- `Err[T,E](error:E)=>Result[T,E]` - Error case

**Functions:**

```text
mapResult(fn,res)
bindResult(fn,res)
unwrapOrResult(fallback,res)
isOk(res)
isErr(res)
```

## List Operations

### Implemented `§list` Functions

```sigil decl §list
λall[T](pred:λ(T)=>Bool,xs:[T])=>Bool
λany[T](pred:λ(T)=>Bool,xs:[T])=>Bool
λcontains[T](item:T,xs:[T])=>Bool
λcount[T](item:T,xs:[T])=>Int
λcountIf[T](pred:λ(T)=>Bool,xs:[T])=>Int
λdrop[T](n:Int,xs:[T])=>[T]
λfind[T](pred:λ(T)=>Bool,xs:[T])=>Option[T]
λflatMap[T,U](fn:λ(T)=>[U],xs:[T])=>[U]
λfold[T,U](acc:U,fn:λ(U,T)=>U,xs:[T])=>U
λinBounds[T](idx:Int,xs:[T])=>Bool
λlast[T](xs:[T])=>Option[T]
λmax(xs:[Int])=>Option[Int]
λmin(xs:[Int])=>Option[Int]
λnth[T](idx:Int,xs:[T])=>Option[T]
λproduct(xs:[Int])=>Int
λremoveFirst[T](item:T,xs:[T])=>[T]
λreverse[T](xs:[T])=>[T]
λsortedAsc(xs:[Int])=>Bool
λsortedDesc(xs:[Int])=>Bool
λsum(xs:[Int])=>Int
λtake[T](n:Int,xs:[T])=>[T]
```

Safe element access uses `Option[T]`:
- `last([])=>None()`
- `find(pred,[])=>None()`
- `max([])=>None()`
- `min([])=>None()`
- `nth(-1,xs)=>None()`
- `nth(idx,xs)=>None()` when out of bounds

### Canonical list-processing restrictions

Sigil treats the list-processing surface as canonical:

- use `§list.all` for universal checks
- use `§list.any` for existential checks
- use `§list.countIf` for predicate counting
- use `map` for projection
- use `filter` for filtering
- use `§list.find` for first-match search
- use `§list.flatMap` for flattening projection
- use `reduce ... from ...` or `§list.fold` for reduction
- use `§list.reverse` for reversal

The validator rejects exact recursive clones of `all`, `any`, `map`, `filter`,
`find`, `flatMap`, `fold`, and `reverse`, rejects `#(xs filter pred)` in favor of
`§list.countIf`, and rejects recursive result-building of the form
`self(rest)⧺rhs`. These are narrow AST-shape rules, not a general complexity
prover.

Outside `language/stdlib/`, the validator also rejects exact top-level wrappers
whose body is already a canonical helper surface, such as `§list.sum(xs)`,
`§numeric.max(a,b)`, `§string.trim(s)`, `xs map fn`, `xs filter pred`, or
`xs reduce fn from init`. Sigil keeps one canonical helper surface instead of
supporting thin local aliases for the same operation.

### Implemented `§numeric` Helpers

```sigil decl §numeric
t DivMod={quotient:Int,remainder:Int}
λabs(x:Int)=>Int
λclamp(hi:Int,lo:Int,x:Int)=>Int
λdivisible(d:Int,n:Int)=>Bool
λdivmod(a:Int,b:Int)=>DivMod
λgcd(a:Int,b:Int)=>Int
λinRange(max:Int,min:Int,x:Int)=>Bool
λisEven(x:Int)=>Bool
λisNegative(x:Int)=>Bool
λisNonNegative(x:Int)=>Bool
λisOdd(x:Int)=>Bool
λisPositive(x:Int)=>Bool
λisPrime(n:Int)=>Bool
λlcm(a:Int,b:Int)=>Int
λmax(a:Int,b:Int)=>Int
λmin(a:Int,b:Int)=>Int
λmod(a:Int,b:Int)=>Int
λpow(base:Int,exp:Int)=>Int
λrange(start:Int,stop:Int)=>[Int]
λsign(x:Int)=>Int
```

### Implemented `§random` Functions

```sigil decl §random
λintBetween(max:Int,min:Int)=>!Random Int
λpick[T](items:[T])=>!Random Option[T]
λshuffle[T](items:[T])=>!Random [T]
```

Semantics:
- `intBetween` is inclusive and order-insensitive over its two bounds
- `pick([])` returns `None()`
- `shuffle` returns a full permutation of the input list
- runtime behavior comes from the active world's `random` entry

## String Operations

```sigil decl §string
λcharAt(idx:Int,s:String)=>String
λcontains(s:String,search:String)=>Bool
λdrop(n:Int,s:String)=>String
λendsWith(s:String,suffix:String)=>Bool
λindexOf(s:String,search:String)=>Int
λintToString(n:Int)=>String
λisDigit(s:String)=>Bool
λjoin(separator:String,strings:[String])=>String
λlines(s:String)=>[String]
λreplaceAll(pattern:String,replacement:String,s:String)=>String
λrepeat(count:Int,s:String)=>String
λreverse(s:String)=>String
λsplit(delimiter:String,s:String)=>[String]
λstartsWith(prefix:String,s:String)=>Bool
λsubstring(end:Int,s:String,start:Int)=>String
λtake(n:Int,s:String)=>String
λtoLower(s:String)=>String
λtoUpper(s:String)=>String
λtrimEndChars(chars:String,s:String)=>String
λtrimStartChars(chars:String,s:String)=>String
λtrim(s:String)=>String
λunlines(lines:[String])=>String
```

## File and Process Operations

### Implemented `§file` Functions

```sigil decl §file
λappendText(content:String,path:String)=>!Fs Unit
λexists(path:String)=>!Fs Bool
λlistDir(path:String)=>!Fs [String]
λmakeDir(path:String)=>!Fs Unit
λmakeDirs(path:String)=>!Fs Unit
λmakeTempDir(prefix:String)=>!Fs String
λreadText(path:String)=>!Fs String
λremove(path:String)=>!Fs Unit
λremoveTree(path:String)=>!Fs Unit
λwriteText(content:String,path:String)=>!Fs Unit
```

`makeTempDir(prefix)` creates a fresh temp directory and returns its absolute
path. Cleanup remains explicit through `removeTree`.

### Implemented `§process` Types and Functions

```sigil decl §process
t Command={argv:[String],cwd:Option[String],env:{String↦String}}
t RunningProcess={pid:Int}
t ProcessResult={code:Int,stderr:String,stdout:String}

λcommand(argv:[String])=>Command
λexit(code:Int)=>!Process Unit
λwithCwd(command:Command,cwd:String)=>Command
λwithEnv(command:Command,env:{String↦String})=>Command
λrun(command:Command)=>!Process ProcessResult
λstart(command:Command)=>!Process RunningProcess
λwait(process:RunningProcess)=>!Process ProcessResult
λkill(process:RunningProcess)=>!Process Unit
```

Process rules:
- command execution is argv-based only
- `withEnv` overlays explicit variables on top of the inherited environment
- non-zero exit codes are reported in `ProcessResult.code`
- `run` captures stdout and stderr in memory
- `kill` is a normal termination request, not a timeout/escalation protocol

### Implemented `§terminal` Types and Functions

```sigil decl §terminal
t Key=Escape()|Text(String)

λclearScreen()=>!Terminal Unit
λdisableRawMode()=>!Terminal Unit
λenableRawMode()=>!Terminal Unit
λhideCursor()=>!Terminal Unit
λreadKey()=>!Terminal Key
λshowCursor()=>!Terminal Unit
λwrite(text:String)=>!Terminal Unit
```

Terminal rules:
- terminal interaction is raw-key oriented rather than line-oriented
- `readKey` returns canonical `Key` values
- `Escape()` represents the escape key and escape sequences
- `Text(String)` carries normalized plain-text key input
- interactive programs should restore cursor visibility and raw-mode state before exit

### Implemented `§regex` Types and Functions

```sigil decl §regex
t Regex={flags:String,pattern:String}
t RegexError={message:String}
t RegexMatch={captures:[String],end:Int,full:String,start:Int}

λcompile(flags:String,pattern:String)=>Result[Regex,RegexError]
λfind(input:String,regex:Regex)=>Option[RegexMatch]
λisMatch(input:String,regex:Regex)=>Bool
```

Regex rules:
- v1 semantics follow JavaScript `RegExp`
- `compile` validates both flags and pattern before returning `Ok`
- `find` returns the first match only
- unmatched capture groups are returned as empty strings in `captures`

### Implemented `§time` Additions

```sigil decl §time
λsleepMs(ms:Int)=>!Timer Unit
```

`sleepMs` is the canonical delay primitive for retry loops and harness
orchestration.

## Map Operations

Maps are a core collection concept, and helper functions live in `¶map`.

```sigil decl ¶map
t Entry[K,V]={key:K,value:V}

λempty[K,V]()=>{K↦V}
λentries[K,V](map:{K↦V})=>[Entry[K,V]]
λfilter[K,V](map:{K↦V},pred:λ(K,V)=>Bool)=>{K↦V}
λfold[K,V,U](fn:λ(U,K,V)=>U,init:U,map:{K↦V})=>U
λfromList[K,V](entries:[Entry[K,V]])=>{K↦V}
λget[K,V](key:K,map:{K↦V})=>Option[V]
λhas[K,V](key:K,map:{K↦V})=>Bool
λinsert[K,V](key:K,map:{K↦V},value:V)=>{K↦V}
λkeys[K,V](map:{K↦V})=>[K]
λmapValues[K,V,U](fn:λ(V)=>U,map:{K↦V})=>{K↦U}
λmerge[K,V](left:{K↦V},right:{K↦V})=>{K↦V}
λremove[K,V](key:K,map:{K↦V})=>{K↦V}
λsingleton[K,V](key:K,value:V)=>{K↦V}
λsize[K,V](map:{K↦V})=>Int
λvalues[K,V](map:{K↦V})=>[V]
```

## JSON Operations

```sigil decl §json
t JsonError={message:String}
t JsonValue=JsonArray([JsonValue])|JsonBool(Bool)|JsonNull|JsonNumber(Float)|JsonObject({String↦JsonValue})|JsonString(String)

λparse(input:String)=>Result[JsonValue,JsonError]
λstringify(value:JsonValue)=>String
λgetField(key:String,obj:{String↦JsonValue})=>Option[JsonValue]
λgetIndex(arr:[JsonValue],idx:Int)=>Option[JsonValue]
λasArray(value:JsonValue)=>Option[[JsonValue]]
λasBool(value:JsonValue)=>Option[Bool]
λasNumber(value:JsonValue)=>Option[Float]
λasObject(value:JsonValue)=>Option[{String↦JsonValue}]
λasString(value:JsonValue)=>Option[String]
λisNull(value:JsonValue)=>Bool
```

Notes:
- `parse` is exception-safe and returns `Err({message})` for invalid JSON.
- `stringify` is canonical JSON output for the provided `JsonValue`.

## Decode Operations

`§decode` is the canonical boundary layer from raw `JsonValue` to trusted
internal Sigil values.

```sigil decl §decode
t DecodeError={message:String,path:[String]}
t Decoder[T]=λ(JsonValue)=>Result[T,DecodeError]

λrun[T](decoder:Decoder[T],value:JsonValue)=>Result[T,DecodeError]
λparse[T](decoder:Decoder[T],input:String)=>Result[T,DecodeError]
λsucceed[T](value:T)=>Decoder[T]
λfail[T](message:String)=>Decoder[T]
λmap[T,U](decoder:Decoder[T],fn:λ(T)=>U)=>Decoder[U]
λbind[T,U](decoder:Decoder[T],fn:λ(T)=>Decoder[U])=>Decoder[U]

λbool(value:JsonValue)=>Result[Bool,DecodeError]
λfloat(value:JsonValue)=>Result[Float,DecodeError]
λint(value:JsonValue)=>Result[Int,DecodeError]
λstring(value:JsonValue)=>Result[String,DecodeError]

λlist[T](decoder:Decoder[T])=>Decoder[[T]]
λdict[T](decoder:Decoder[T])=>Decoder[{String↦T}]
λfield[T](decoder:Decoder[T],key:String)=>Decoder[T]
λoptionalField[T](decoder:Decoder[T],key:String)=>Decoder[Option[T]]
```

Notes:
- `§json` owns raw parsing and inspection.
- `§decode` owns conversion into trusted internal types.
- `DecodeError.path` records the nested field/index path of the failure.
- If a field may be absent, keep the record exact and use `Option[T]` for that field.
- Sigil does not use open records or partial records for this boundary story.

## Time Operations

```sigil decl §time
t Instant={epochMillis:Int}
t TimeError={message:String}

λparseIso(input:String)=>Result[Instant,TimeError]
λformatIso(instant:Instant)=>String
λnow()=>!Clock Instant
λfromEpochMillis(millis:Int)=>Instant
λtoEpochMillis(instant:Instant)=>Int
λcompare(left:Instant,right:Instant)=>Int
λisBefore(left:Instant,right:Instant)=>Bool
λisAfter(left:Instant,right:Instant)=>Bool
```

Notes:
- `parseIso` is strict ISO-8601 only.
- Non-ISO text must be normalized before calling `parseIso`.

## Math Operations

The numeric helper surface is owned by `§numeric`; there is no separate
math module today.

## Logging Operations

```sigil decl §io
λdebug(msg:String)=>!Log Unit
λeprintln(msg:String)=>!Log Unit
λprint(msg:String)=>!Log Unit
λprintln(msg:String)=>!Log Unit
λwarn(msg:String)=>!Log Unit
```

## Module System

### Import Syntax

```sigil module
```

### Export Visibility

File extension determines visibility:

**`.lib.sigil` files** (libraries):
- All top-level declarations are automatically visible to other modules
- No `export` keyword needed or allowed

**`.sigil` files** (executables):
- Export nothing directly
- Have `main()` function

No import declarations, no aliasing, no export lists.

## Standard Library Modules

### core/prelude

Implicitly available. Contains the foundational vocabulary types:
- `Option[T]`
- `Result[T,E]`
- `Some`
- `None`
- `Ok`
- `Err`

### §file

UTF-8 filesystem helpers:
- `appendText`
- `exists`
- `listDir`
- `makeDir`
- `makeDirs`
- `makeTempDir`
- `readText`
- `remove`
- `removeTree`
- `writeText`

### §path

Filesystem path helpers:
- `basename`
- `dirname`
- `extname`
- `join`
- `normalize`
- `relative`

### §io

Console and process I/O only (`print`, `println`, `eprintln`, `warn`, `debug`)

### ¶map

Dynamic keyed collection helpers over `{K↦V}` values.

### §numeric

Integer helpers (`abs`, `divmod`, `gcd`, `lcm`, `max`, `min`, `mod`,
predicates like `isEven`, and ranges).

### §json

Typed JSON parsing and serialization (`JsonValue`, `parse`, `stringify`)

```sigil decl §json
λparse(input:String)=>Result[JsonValue,JsonError]
λstringify(value:JsonValue)=>String
```

### §decode

Canonical JSON-to-domain decoding (`Decoder[T]`, `DecodeError`, `run`, `parse`)

```sigil decl §decode
λrun[T](decoder:Decoder[T],value:JsonValue)=>Result[T,DecodeError]
λparse[T](decoder:Decoder[T],input:String)=>Result[T,DecodeError]
```

### §time

Time and instant handling (`Instant`, strict ISO parsing, clock access)

```sigil decl §time
λparseIso(input:String)=>Result[Instant,TimeError]
λformatIso(instant:Instant)=>String
λnow()=>!Clock Instant
```

### §topology

Canonical declaration layer for external HTTP and TCP runtime dependencies.

```sigil decl §topology
t Environment=Environment(String)
t HttpServiceDependency=HttpServiceDependency(String)
t TcpServiceDependency=TcpServiceDependency(String)

λenvironment(name:String)=>Environment
λhttpService(name:String)=>HttpServiceDependency
λtcpService(name:String)=>TcpServiceDependency
```

### §config

Low-level helper layer for topology-backed environment config data.

Canonical project environment files now export `world` values built through
`†runtime`, `†http`, and `†tcp`. `§config` remains
available inside config modules for binding-shaped helper values, but it is no
longer the exported environment ABI.

```sigil decl §config
t BindingValue=EnvVar(String)|Literal(String)
t Bindings={httpBindings:[HttpBinding],tcpBindings:[TcpBinding]}
t HttpBinding={baseUrl:BindingValue,dependencyName:String}
t PortBindingValue=EnvVarPort(String)|LiteralPort(Int)
t TcpBinding={dependencyName:String,host:BindingValue,port:PortBindingValue}

λbindHttp(baseUrl:String,dependency:§topology.HttpServiceDependency)=>HttpBinding
λbindHttpEnv(dependency:§topology.HttpServiceDependency,envVar:String)=>HttpBinding
λbindTcp(dependency:§topology.TcpServiceDependency,host:String,port:Int)=>TcpBinding
λbindTcpEnv(dependency:§topology.TcpServiceDependency,hostEnvVar:String,portEnvVar:String)=>TcpBinding
λbindings(httpBindings:[HttpBinding],tcpBindings:[TcpBinding])=>Bindings
```

### §httpClient

Canonical text-based HTTP client.

```sigil decl §httpClient
t Headers={String↦String}
t HttpError={kind:HttpErrorKind,message:String}
t HttpErrorKind=InvalidJson()|InvalidUrl()|Network()|Timeout()|Topology()
t HttpMethod=Delete()|Get()|Patch()|Post()|Put()
t HttpRequest={body:Option[String],dependency:§topology.HttpServiceDependency,headers:Headers,method:HttpMethod,path:String}
t HttpResponse={body:String,headers:Headers,status:Int,url:String}

λrequest(request:HttpRequest)=>!Http Result[HttpResponse,HttpError]
λget(dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[HttpResponse,HttpError]
λdelete(dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[HttpResponse,HttpError]
λpost(body:String,dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[HttpResponse,HttpError]
λput(body:String,dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[HttpResponse,HttpError]
λpatch(body:String,dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[HttpResponse,HttpError]

λgetJson(dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[JsonValue,HttpError]
λdeleteJson(dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[JsonValue,HttpError]
λpostJson(body:JsonValue,dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[JsonValue,HttpError]
λputJson(body:JsonValue,dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[JsonValue,HttpError]
λpatchJson(body:JsonValue,dependency:§topology.HttpServiceDependency,headers:Headers,path:String)=>!Http Result[JsonValue,HttpError]
λresponseJson(response:HttpResponse)=>Result[JsonValue,HttpError]

λemptyHeaders()=>Headers
λjsonHeaders()=>Headers
λheader(key:String,value:String)=>Headers
λmergeHeaders(left:Headers,right:Headers)=>Headers
```

Semantics:
- any successfully received HTTP response returns `Ok(HttpResponse)`, including `404` and `500`
- invalid URL, transport failure, topology resolution failure, and JSON parse failure return `Err(HttpError)`
- request and response bodies are UTF-8 text in v1

### §httpServer

Canonical request/response HTTP server.

```sigil decl §httpServer
t Headers={String↦String}
t Request={body:String,headers:Headers,method:String,path:String}
t Response={body:String,headers:Headers,status:Int}
t Server={port:Int}

λresponse(body:String,contentType:String,status:Int)=>Response
λok(body:String)=>Response
λjson(body:String,status:Int)=>Response
λlisten(handler:λ(Request)=>Response,port:Int)=>!Http Server
λnotFound()=>Response
λnotFoundMsg(message:String)=>Response
λport(server:Server)=>Int
λserverError(message:String)=>Response
λlogRequest(request:Request)=>!Log Unit
λserve(handler:λ(Request)=>Response,port:Int)=>!Http Unit
λwait(server:Server)=>!Http Unit
```

Semantics:
- `serve(handler,port)` is equivalent to blocking on a started server
- `listen` returns a server handle that can be observed with `port` and awaited with `wait`
- passing `0` as the port asks the OS to choose any free ephemeral port
- `port(server)` returns the actual bound port, including after a `0` bind
- `serve` and `wait` are long-lived once listening succeeds

### §tcpClient

Canonical one-request, one-response TCP client.

```sigil decl §tcpClient
t TcpError={kind:TcpErrorKind,message:String}
t TcpErrorKind=Connection()|InvalidAddress()|Protocol()|Timeout()|Topology()
t TcpRequest={dependency:§topology.TcpServiceDependency,message:String}
t TcpResponse={message:String}

λrequest(request:TcpRequest)=>!Tcp Result[TcpResponse,TcpError]
λsend(dependency:§topology.TcpServiceDependency,message:String)=>!Tcp Result[TcpResponse,TcpError]
```

Semantics:
- requests are UTF-8 text
- the client writes one newline-delimited message and expects one newline-delimited response
- address validation, socket failure, timeout, topology resolution failure, and framing failure return `Err(TcpError)`

### §tcpServer

Canonical one-request, one-response TCP server.

```sigil decl §tcpServer
t Request={host:String,message:String,port:Int}
t Response={message:String}
t Server={port:Int}

λlisten(handler:λ(Request)=>Response,port:Int)=>!Tcp Server
λport(server:Server)=>Int
λresponse(message:String)=>Response
λserve(handler:λ(Request)=>Response,port:Int)=>!Tcp Unit
λwait(server:Server)=>!Tcp Unit
```

Semantics:
- the server reads one UTF-8 line per connection
- the handler returns one UTF-8 line response
- the server closes each connection after the response is written
- `serve(handler,port)` is equivalent to blocking on a started server
- `listen` returns a server handle that can be observed with `port` and awaited with `wait`
- passing `0` as the port asks the OS to choose any free ephemeral port
- `port(server)` returns the actual bound port, including after a `0` bind
- `serve` and `wait` are long-lived once listening succeeds

### Testing

Testing is built into the language with `test` declarations and the `sigil
test` runner. There is no current `§test` module surface.

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
- String concatenation in loops is O(n²) - prefer §string.join when building from parts

### Effect System

Effects are tracked at type level:
- `!Clock`
- `!Fs`
- `!Http`
- `!Log`
- `!Process`
- `!Random`
- `!Tcp`
- `!Timer`
- Pure functions have no effect annotation

Projects may define reusable multi-effect aliases in `src/effects.lib.sigil`.

## Future Extensions

Planned for future stdlib versions:

- **§crypto** - Cryptographic functions
- **§stream** - Streaming I/O
- **§concurrency** - Threads and channels

## See Also

- [Type System](type-system.md) - Type inference and checking
- [Grammar](grammar.ebnf) - Language syntax
- Implementation: core/prelude.lib.sigil

---

**Next**: Implement standard library in stdlib/ directory.
