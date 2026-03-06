# Sigil Standard Library Specification

Version: 1.0.0
Last Updated: 2026-02-21

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
λis_some[T](opt:Option[T])→𝔹 match opt{Some(_)→true|None()→false}
λis_none[T](opt:Option[T])→𝔹 match opt{Some(_)→false|None()→true}
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
λis_ok[T,E](res:Result[T,E])→𝔹 match res{Ok(_)→true|Err(_)→false}
λis_err[T,E](res:Result[T,E])→𝔹 match res{Ok(_)→false|Err(_)→true}
```

## List Operations

### Implemented `stdlib⋅list` Functions

```sigil
λcontains[T](item:T,xs:[T])→𝔹
λcount[T](item:T,xs:[T])→ℤ
λdrop[T](n:ℤ,xs:[T])→[T]
λfind[T](pred:λ(T)→𝔹,xs:[T])→Option[T]
λfold[T,U](acc:U,fn:λ(U,T)→U,xs:[T])→U
λin_bounds[T](idx:ℤ,xs:[T])→𝔹
λlast[T](xs:[T])→Option[T]
λmax(xs:[ℤ])→Option[ℤ]
λmin(xs:[ℤ])→Option[ℤ]
λnth[T](idx:ℤ,xs:[T])→Option[T]
λproduct(xs:[ℤ])→ℤ
λremove_first[T](item:T,xs:[T])→[T]
λreverse[T](xs:[T])→[T]
λsorted_asc(xs:[ℤ])→𝔹
λsorted_desc(xs:[ℤ])→𝔹
λsum(xs:[ℤ])→ℤ
λtake[T](n:ℤ,xs:[T])→[T]
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
t DivMod={quotient:ℤ,remainder:ℤ}
λabs(x:ℤ)→ℤ
λclamp(hi:ℤ,lo:ℤ,x:ℤ)→ℤ
λdivisible(d:ℤ,n:ℤ)→𝔹
λdivmod(a:ℤ,b:ℤ)→DivMod
λgcd(a:ℤ,b:ℤ)→ℤ
λin_range(max:ℤ,min:ℤ,x:ℤ)→𝔹
λis_even(x:ℤ)→𝔹
λis_negative(x:ℤ)→𝔹
λis_non_negative(x:ℤ)→𝔹
λis_odd(x:ℤ)→𝔹
λis_positive(x:ℤ)→𝔹
λis_prime(n:ℤ)→𝔹
λlcm(a:ℤ,b:ℤ)→ℤ
λmax(a:ℤ,b:ℤ)→ℤ
λmin(a:ℤ,b:ℤ)→ℤ
λmod(a:ℤ,b:ℤ)→ℤ
λpow(base:ℤ,exp:ℤ)→ℤ
λrange(start:ℤ,stop:ℤ)→[ℤ]
λsign(x:ℤ)→ℤ
```

## String Operations

```sigil
λchar_at(idx:ℤ,s:𝕊)→𝕊
```
Get character at index.
- Complexity: O(1)
- Pure: Yes

```sigil
λdrop(n:ℤ,s:𝕊)→𝕊
```
Drop first `n` characters.
- Complexity: O(n)
- Pure: Yes

```sigil
λends_with(s:𝕊,suffix:𝕊)→𝔹
```
Check if string ends with suffix.
- Complexity: O(n)
- Pure: Yes

```sigil
λindex_of(s:𝕊,search:𝕊)→ℤ
```
Find index of first occurrence, or `-1` if missing.
- Complexity: O(n)
- Pure: Yes

```sigil
λint_to_string(n:ℤ)→𝕊
```
Convert an integer to a string.
- Complexity: O(n)
- Pure: Yes

```sigil
λis_digit(s:𝕊)→𝔹
```
Check whether a string is exactly one decimal digit.
- Complexity: O(1)
- Pure: Yes

```sigil
λjoin(separator:𝕊,strings:[𝕊])→𝕊
```
Join strings with a separator.
- Complexity: O(n)
- Pure: Yes

```sigil
λlines(s:𝕊)→[𝕊]
```
Split a string on newline characters.
- Complexity: O(n)
- Pure: Yes

```sigil
λreplace_all(pattern:𝕊,replacement:𝕊,s:𝕊)→𝕊
```
Replace all occurrences of a pattern with a replacement string.
- Complexity: O(n)
- Pure: Yes

```sigil
λrepeat(count:ℤ,s:𝕊)→𝕊
```
Repeat a string `count` times.
- Complexity: O(n)
- Pure: Yes

```sigil
λsplit(delimiter:𝕊,s:𝕊)→[𝕊]
```
Split a string by delimiter.
- Complexity: O(n)
- Pure: Yes

```sigil
λstarts_with(prefix:𝕊,s:𝕊)→𝔹
```
Check if string starts with prefix.
- Complexity: O(n)
- Pure: Yes

```sigil
λsubstring(end:ℤ,s:𝕊,start:ℤ)→𝕊
```
Get substring from `start` to `end`.
- Complexity: O(n)
- Pure: Yes

```sigil
λtake(n:ℤ,s:𝕊)→𝕊
```
Take first `n` characters.
- Complexity: O(n)
- Pure: Yes

```sigil
λto_lower(s:𝕊)→𝕊
```
Convert to lowercase.
- Complexity: O(n)
- Pure: Yes

```sigil
λto_upper(s:𝕊)→𝕊
```
Convert to uppercase.
- Complexity: O(n)
- Pure: Yes

```sigil
λtrim(s:𝕊)→𝕊
```
Remove leading/trailing whitespace.
- Complexity: O(n)
- Pure: Yes

```sigil
λunlines(lines:[𝕊])→𝕊
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
λhas[K,V](key:K,map:{K↦V})→𝔹
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
t JsonError={message:𝕊}
t JsonValue=JsonArray([JsonValue])|JsonBool(𝔹)|JsonNull|JsonNumber(ℝ)|JsonObject({𝕊↦JsonValue})|JsonString(𝕊)

λparse(input:𝕊)→Result[JsonValue,JsonError]
λstringify(value:JsonValue)→𝕊
λget_field(key:𝕊,obj:{𝕊↦JsonValue})→Option[JsonValue]
λget_index(arr:[JsonValue],idx:ℤ)→Option[JsonValue]
λas_array(value:JsonValue)→Option[[JsonValue]]
λas_bool(value:JsonValue)→Option[𝔹]
λas_number(value:JsonValue)→Option[ℝ]
λas_object(value:JsonValue)→Option[{𝕊↦JsonValue}]
λas_string(value:JsonValue)→Option[𝕊]
λis_null(value:JsonValue)→𝔹
```

Notes:
- `parse` is exception-safe and returns `Err({message})` for invalid JSON.
- `stringify` is canonical JSON output for the provided `JsonValue`.

## Time Operations

```sigil
t Instant={epoch_millis:ℤ}
t TimeError={message:𝕊}

λparse_iso(input:𝕊)→Result[Instant,TimeError]
λformat_iso(instant:Instant)→𝕊
λnow()→!IO Instant
λfrom_epoch_millis(millis:ℤ)→Instant
λto_epoch_millis(instant:Instant)→ℤ
λcompare(left:Instant,right:Instant)→ℤ
λis_before(left:Instant,right:Instant)→𝔹
λis_after(left:Instant,right:Instant)→𝔹
```

Notes:
- `parse_iso` is strict ISO-8601 only.
- Non-ISO text must be normalized before calling `parse_iso`.

## Math Operations

```sigil
λabs(n:ℤ)→ℤ
```
Absolute value.
- Complexity: O(1)
- Pure: Yes

```sigil
λmin(a:ℤ,b:ℤ)→ℤ
```
Minimum of two integers.
- Complexity: O(1)
- Pure: Yes

```sigil
λmax(a:ℤ,b:ℤ)→ℤ
```
Maximum of two integers.
- Complexity: O(1)
- Pure: Yes

```sigil
λpow(base:ℤ,exp:ℤ)→ℤ
```
Exponentiation (integer power).
- Complexity: O(log exp)
- Pure: Yes

```sigil
λsqrt(n:ℝ)→ℝ
```
Square root.
- Complexity: O(1)
- Pure: Yes

```sigil
λfloor(n:ℝ)→ℤ
```
Round down to integer.
- Complexity: O(1)
- Pure: Yes

```sigil
λceil(n:ℝ)→ℤ
```
Round up to integer.
- Complexity: O(1)
- Pure: Yes

```sigil
λround(n:ℝ)→ℤ
```
Round to nearest integer.
- Complexity: O(1)
- Pure: Yes

## I/O Operations

All I/O operations have the `!IO` effect.

```sigil
λprint(s:𝕊)→𝕌!IO
```
Print string to stdout.
- Effect: IO
- Complexity: O(n)

```sigil
λprintln(s:𝕊)→𝕌!IO
```
Print string with newline.
- Effect: IO
- Complexity: O(n)

```sigil
λread_line()→𝕊!IO
```
Read line from stdin.
- Effect: IO
- Complexity: O(n)

```sigil
λread_file(path:𝕊)→Result[𝕊,IoError]!IO
```
Read entire file as string.
- Effect: IO
- Complexity: O(file size)

```sigil
λwrite_file(path:𝕊,content:𝕊)→Result[𝕌,IoError]!IO
```
Write string to file.
- Effect: IO
- Complexity: O(n)

## Error Handling

```sigil
t IoError={kind:𝕊,msg:𝕊}
t ParseError={column:ℤ,line:ℤ,msg:𝕊}
```

```sigil
λpanic[T](msg:𝕊)→T
```
Immediately terminate program with error message.
- Effect: Diverges (returns ∅)
- Use sparingly - prefer Result for recoverable errors

```sigil
λassert(condition:𝔹,msg:𝕊)→𝕌
```
Assert condition is true, panic if false.
- Effect: May diverge
- Use for invariants that should never be violated

## Type Conversion

```sigil
λint_to_string(n:ℤ)→𝕊
```
Convert integer to string.
- Complexity: O(log n)
- Pure: Yes

```sigil
λstring_to_int(s:𝕊)→Result[ℤ,ParseError]
```
Parse integer from string.
- Complexity: O(n)
- Pure: Yes

```sigil
λfloat_to_string(n:ℝ)→𝕊
```
Convert float to string.
- Complexity: O(1)
- Pure: Yes

```sigil
λstring_to_float(s:𝕊)→Result[ℝ,ParseError]
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
- `append_text`
- `exists`
- `list_dir`
- `make_dir`
- `make_dirs`
- `read_text`
- `remove`
- `remove_tree`
- `write_text`

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
λparse(input:𝕊)→Result[JsonValue,JsonError]
λstringify(value:JsonValue)→𝕊
```

### std/time

Time and instant handling (`Instant`, strict ISO parsing, clock access)

```sigil
λparse_iso(input:𝕊)→Result[Instant,TimeError]
λformat_iso(instant:Instant)→𝕊
λnow()→!IO Instant
```

### std/http

HTTP client and server

```sigil
t HttpMethod=GET|POST|PUT|DELETE|PATCH
t HttpRequest={body:𝕊,headers:{𝕊:𝕊},method:HttpMethod,url:𝕊}
t HttpResponse={body:𝕊,headers:{𝕊:𝕊},status:ℤ}

λhttp_get(url:𝕊)→Result[HttpResponse,HttpError]!Network
λhttp_post(url:𝕊,body:𝕊)→Result[HttpResponse,HttpError]!Network
```

### std/test

Testing utilities

```sigil
λtest(name:𝕊,fn:λ()→𝕌)→𝕌!Test
λassert_eq[T](expected:T,actual:T)→𝕌
λassert_ne[T](a:T,b:T)→𝕌
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
- `!Network` - Network requests
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
