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
λmap_option[T,U](fn:λ(T)→U,opt:Option[T])→Option[U] match opt{Some(v)→Some(fn(v))|None→None}
λbind_option[T,U](opt:Option[T],fn:λ(T)→Option[U])→Option[U] match opt{Some(v)→fn(v)|None→None}
λunwrap_or[T](opt:Option[T],default:T)→T match opt{Some(v)→v|None→default}
λis_some[T](opt:Option[T])→𝔹 match opt{Some(_)→true|None→false}
λis_none[T](opt:Option[T])→𝔹 match opt{Some(_)→false|None→true}
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
λbind_result[T,U,E](res:Result[T,E],fn:λ(T)→Result[U,E])→Result[U,E] match res{Ok(v)→fn(v)|Err(e)→Err(e)}
λunwrap_or_result[T,E](res:Result[T,E],default:T)→T match res{Ok(v)→v|Err(_)→default}
λis_ok[T,E](res:Result[T,E])→𝔹 match res{Ok(_)→true|Err(_)→false}
λis_err[T,E](res:Result[T,E])→𝔹 match res{Ok(_)→false|Err(_)→true}
```

## List Operations

### Implemented `stdlib⋅list` Functions

```sigil
λall(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹
λany(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹
λcontains(item:ℤ,xs:[ℤ])→𝔹
λcount(item:ℤ,xs:[ℤ])→ℤ
λdrop(n:ℤ,xs:[ℤ])→[ℤ]
λhead(xs:[ℤ])→ℤ
λin_bounds(idx:ℤ,xs:[ℤ])→𝔹
λis_empty(xs:[ℤ])→𝔹
λis_non_empty(xs:[ℤ])→𝔹
 t IntOption=IntNone|IntSome(ℤ)
λlast(xs:[ℤ])→IntOption
λnth(idx:ℤ,xs:[ℤ])→IntOption
λremove_first(item:ℤ,xs:[ℤ])→[ℤ]
λreverse(xs:[ℤ])→[ℤ]
λsorted_asc(xs:[ℤ])→𝔹
λsorted_desc(xs:[ℤ])→𝔹
λtail(xs:[ℤ])→[ℤ]
λtake(n:ℤ,xs:[ℤ])→[ℤ]
```

Safe element access uses `IntOption`:
- `last([])→IntNone()`
- `nth(-1,xs)→IntNone()`
- `nth(idx,xs)→IntNone()` when out of bounds

Unsafe `head` and `tail` remain concrete convenience functions.

### Implemented `stdlib⋅numeric` Helpers

```sigil
λclamp(hi:ℤ,lo:ℤ,x:ℤ)→ℤ
λdivisible(d:ℤ,n:ℤ)→𝔹
λfactorial(n:ℤ)→ℤ
λfib(n:ℤ)→ℤ
λgcd(a:ℤ,b:ℤ)→ℤ
λin_range(max:ℤ,min:ℤ,x:ℤ)→𝔹
λis_even(x:ℤ)→𝔹
λis_negative(x:ℤ)→𝔹
λis_non_negative(x:ℤ)→𝔹
λis_odd(x:ℤ)→𝔹
λis_positive(x:ℤ)→𝔹
λis_prime(n:ℤ)→𝔹
λis_zero(x:ℤ)→𝔹
λmax(a:ℤ,b:ℤ)→ℤ
λmin(a:ℤ,b:ℤ)→ℤ
λpow(base:ℤ,exp:ℤ)→ℤ
λrange(start:ℤ,stop:ℤ)→[ℤ]
λsum_range(a:ℤ,b:ℤ)→ℤ
λsum_to(n:ℤ)→ℤ
```

## String Operations

```sigil
λstr_length(s:𝕊)→ℤ
```
Get string length (Unicode code points).
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_concat(s1:𝕊,s2:𝕊)→𝕊
```
Concatenate strings.
- Complexity: O(n+m)
- Pure: Yes
- Operator: `+`

```sigil
λstr_split(s:𝕊,sep:𝕊)→[𝕊]
```
Split string by separator.
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_join(sep:𝕊,parts:[𝕊])→𝕊
```
Join strings with separator.
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_trim(s:𝕊)→𝕊
```
Remove leading/trailing whitespace.
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_to_upper(s:𝕊)→𝕊
```
Convert to uppercase.
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_to_lower(s:𝕊)→𝕊
```
Convert to lowercase.
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_contains(s:𝕊,substr:𝕊)→𝔹
```
Check if string contains substring.
- Complexity: O(n*m)
- Pure: Yes

```sigil
λstr_starts_with(s:𝕊,prefix:𝕊)→𝔹
```
Check if string starts with prefix.
- Complexity: O(n)
- Pure: Yes

```sigil
λstr_ends_with(s:𝕊,suffix:𝕊)→𝔹
```
Check if string ends with suffix.
- Complexity: O(n)
- Pure: Yes

## Map Operations

```sigil
λmap_empty[K,V]()→{K:V}
```
Create empty map.
- Complexity: O(1)
- Pure: Yes

```sigil
λmap_insert[K,V](key:K,value:V,map:{K:V})→{K:V}
```
Insert key-value pair. Returns new map.
- Complexity: O(log n)
- Pure: Yes

```sigil
λmap_get[K,V](key:K,map:{K:V})→Option[V]
```
Get value for key.
- Complexity: O(log n)
- Pure: Yes

```sigil
λmap_remove[K,V](key:K,map:{K:V})→{K:V}
```
Remove key. Returns new map.
- Complexity: O(log n)
- Pure: Yes

```sigil
λmap_has[K,V](key:K,map:{K:V})→𝔹
```
Check if key exists.
- Complexity: O(log n)
- Pure: Yes

```sigil
λmap_keys[K,V](map:{K:V})→[K]
```
Get all keys.
- Complexity: O(n)
- Pure: Yes

```sigil
λmap_values[K,V](map:{K:V})→[V]
```
Get all values.
- Complexity: O(n)
- Pure: Yes

```sigil
λmap_entries[K,V](map:{K:V})→[(K,V)]
```
Get all key-value pairs.
- Complexity: O(n)
- Pure: Yes

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

```sigil
λbool_to_string(b:𝔹)→𝕊
```
Convert bool to string ("true" or "false").
- Complexity: O(1)
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
i stdlib⋅io
i stdlib⋅list
i stdlib⋅result
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

### std/prelude

Auto-imported. Contains all core types and functions listed above.

### std/io

I/O operations (read_file, write_file, etc.)

### std/collections

Advanced collections: Set[T], Queue[T], Stack[T]

### std/numeric

Mathematical functions: sin, cos, tan, log, exp, etc.

### std/json

JSON parsing and serialization

```sigil
t JsonValue=JsonNull|JsonBool(𝔹)|JsonInt(ℤ)|JsonFloat(ℝ)|JsonString(𝕊)|JsonArray([JsonValue])|JsonObject({𝕊:JsonValue})

λparse_json(s:𝕊)→Result[JsonValue,ParseError]
λstringify_json(value:JsonValue)→𝕊
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

### std/async

Async/await primitives (Future type)

```sigil
t Future[T]

λasync[T](fn:λ()→T)→Future[T]!Async
λawait[T](future:Future[T])→T!Async
λfuture_map[T,U](fn:λ(T)→U,future:Future[T])→Future[U]
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
- String concatenation in loops is O(n²) - use str_join instead

### Effect System

Effects are tracked at type level:
- `!IO` - Input/output operations
- `!Network` - Network requests
- `!Async` - Asynchronous operations
- `!Test` - Test operations
- Pure functions have no effect annotation

## Future Extensions

Planned for future stdlib versions:

- **std/regex** - Regular expressions
- **std/crypto** - Cryptographic functions
- **std/time** - Date and time handling
- **std/random** - Random number generation
- **std/stream** - Streaming I/O
- **std/concurrency** - Threads and channels

## See Also

- [Type System](type-system.md) - Type inference and checking
- [Grammar](grammar.ebnf) - Language syntax
- Implementation: stdlib/prelude.lib.sigil

---

**Next**: Implement standard library in stdlib/ directory.
