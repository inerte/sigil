# Mint Standard Library Specification

Version: 1.0.0
Last Updated: 2026-02-21

## Overview

The Mint standard library provides essential types and functions that are automatically available in every Mint program. The design philosophy emphasizes:

1. **Minimal but complete** - Only include truly universal functionality
2. **Functional-first** - Pure functions, immutability by default
3. **Type-safe** - Leverage strong type system
4. **Composable** - Functions that work well together
5. **Zero-cost abstractions** - Compile to efficient JavaScript

## Automatic Imports

The prelude is automatically imported into every Mint module. No explicit import needed.

## Core Types

### Option[T]

Represents an optional value - Mint's null-safe alternative.

```mint
t Option[T]=Some(T)|None
```

**Constructors:**
- `Some[T](value:T)→Option[T]` - Wraps a value
- `None[T]()→Option[T]` - Represents absence

**Functions:**

```mint
λmap_option[T,U](fn:λ(T)→U,opt:Option[T])→Option[U]≡opt{Some(v)→Some(fn(v))|None→None}
λbind_option[T,U](opt:Option[T],fn:λ(T)→Option[U])→Option[U]≡opt{Some(v)→fn(v)|None→None}
λunwrap_or[T](opt:Option[T],default:T)→T≡opt{Some(v)→v|None→default}
λis_some[T](opt:Option[T])→𝔹≡opt{Some(_)→⊤|None→⊥}
λis_none[T](opt:Option[T])→𝔹≡opt{Some(_)→⊥|None→⊤}
```

### Result[T,E]

Represents a computation that may fail - Mint's exception-free error handling.

```mint
t Result[T,E]=Ok(T)|Err(E)
```

**Constructors:**
- `Ok[T,E](value:T)→Result[T,E]` - Success case
- `Err[T,E](error:E)→Result[T,E]` - Error case

**Functions:**

```mint
λmap_result[T,U,E](fn:λ(T)→U,res:Result[T,E])→Result[U,E]≡res{Ok(v)→Ok(fn(v))|Err(e)→Err(e)}
λbind_result[T,U,E](res:Result[T,E],fn:λ(T)→Result[U,E])→Result[U,E]≡res{Ok(v)→fn(v)|Err(e)→Err(e)}
λunwrap_or_result[T,E](res:Result[T,E],default:T)→T≡res{Ok(v)→v|Err(_)→default}
λis_ok[T,E](res:Result[T,E])→𝔹≡res{Ok(_)→⊤|Err(_)→⊥}
λis_err[T,E](res:Result[T,E])→𝔹≡res{Ok(_)→⊥|Err(_)→⊤}
```

## List Operations

### Core List Functions

```mint
λmap[T,U](fn:λ(T)→U,list:[T])→[U]
```
Apply function to each element, return new list.
- Complexity: O(n)
- Pure: Yes

```mint
λfilter[T](pred:λ(T)→𝔹,list:[T])→[T]
```
Keep only elements where predicate is true.
- Complexity: O(n)
- Pure: Yes

```mint
λreduce[T,U](fn:λ(U,T)→U,init:U,list:[T])→U
```
Reduce list to single value by repeatedly applying function.
- Also known as: fold, accumulate
- Complexity: O(n)
- Pure: Yes

```mint
λlength[T](list:[T])→ℤ
```
Return number of elements in list.
- Complexity: O(n)
- Pure: Yes

```mint
λreverse[T](list:[T])→[T]
```
Reverse the list.
- Complexity: O(n)
- Pure: Yes

```mint
λappend[T](list1:[T],list2:[T])→[T]
```
Concatenate two lists.
- Complexity: O(n) where n = length(list1)
- Pure: Yes
- Operator: `++`

```mint
λhead[T](list:[T])→Option[T]
```
Get first element, None if empty.
- Complexity: O(1)
- Pure: Yes

```mint
λtail[T](list:[T])→Option[[T]]
```
Get all elements except first, None if empty.
- Complexity: O(1)
- Pure: Yes

```mint
λtake[T](n:ℤ,list:[T])→[T]
```
Take first n elements.
- Complexity: O(n)
- Pure: Yes

```mint
λdrop[T](n:ℤ,list:[T])→[T]
```
Drop first n elements.
- Complexity: O(n)
- Pure: Yes

```mint
λzip[T,U](list1:[T],list2:[U])→[(T,U)]
```
Zip two lists into list of pairs. Stops at shorter list.
- Complexity: O(min(n,m))
- Pure: Yes

```mint
λfind[T](pred:λ(T)→𝔹,list:[T])→Option[T]
```
Find first element satisfying predicate.
- Complexity: O(n)
- Pure: Yes

```mint
λany[T](pred:λ(T)→𝔹,list:[T])→𝔹
```
Check if any element satisfies predicate.
- Complexity: O(n)
- Pure: Yes

```mint
λall[T](pred:λ(T)→𝔹,list:[T])→𝔹
```
Check if all elements satisfy predicate.
- Complexity: O(n)
- Pure: Yes

```mint
λsort[T](cmp:λ(T,T)→𝔹,list:[T])→[T]
```
Sort list using comparison function.
- Algorithm: Introsort (quicksort + heapsort + insertion sort)
- Complexity: O(n log n) average and worst case
- Pure: Yes

## String Operations

```mint
λstr_length(s:𝕊)→ℤ
```
Get string length (Unicode code points).
- Complexity: O(n)
- Pure: Yes

```mint
λstr_concat(s1:𝕊,s2:𝕊)→𝕊
```
Concatenate strings.
- Complexity: O(n+m)
- Pure: Yes
- Operator: `+`

```mint
λstr_split(s:𝕊,sep:𝕊)→[𝕊]
```
Split string by separator.
- Complexity: O(n)
- Pure: Yes

```mint
λstr_join(sep:𝕊,parts:[𝕊])→𝕊
```
Join strings with separator.
- Complexity: O(n)
- Pure: Yes

```mint
λstr_trim(s:𝕊)→𝕊
```
Remove leading/trailing whitespace.
- Complexity: O(n)
- Pure: Yes

```mint
λstr_to_upper(s:𝕊)→𝕊
```
Convert to uppercase.
- Complexity: O(n)
- Pure: Yes

```mint
λstr_to_lower(s:𝕊)→𝕊
```
Convert to lowercase.
- Complexity: O(n)
- Pure: Yes

```mint
λstr_contains(s:𝕊,substr:𝕊)→𝔹
```
Check if string contains substring.
- Complexity: O(n*m)
- Pure: Yes

```mint
λstr_starts_with(s:𝕊,prefix:𝕊)→𝔹
```
Check if string starts with prefix.
- Complexity: O(n)
- Pure: Yes

```mint
λstr_ends_with(s:𝕊,suffix:𝕊)→𝔹
```
Check if string ends with suffix.
- Complexity: O(n)
- Pure: Yes

## Map Operations

```mint
λmap_empty[K,V]()→{K:V}
```
Create empty map.
- Complexity: O(1)
- Pure: Yes

```mint
λmap_insert[K,V](key:K,value:V,map:{K:V})→{K:V}
```
Insert key-value pair. Returns new map.
- Complexity: O(log n)
- Pure: Yes

```mint
λmap_get[K,V](key:K,map:{K:V})→Option[V]
```
Get value for key.
- Complexity: O(log n)
- Pure: Yes

```mint
λmap_remove[K,V](key:K,map:{K:V})→{K:V}
```
Remove key. Returns new map.
- Complexity: O(log n)
- Pure: Yes

```mint
λmap_has[K,V](key:K,map:{K:V})→𝔹
```
Check if key exists.
- Complexity: O(log n)
- Pure: Yes

```mint
λmap_keys[K,V](map:{K:V})→[K]
```
Get all keys.
- Complexity: O(n)
- Pure: Yes

```mint
λmap_values[K,V](map:{K:V})→[V]
```
Get all values.
- Complexity: O(n)
- Pure: Yes

```mint
λmap_entries[K,V](map:{K:V})→[(K,V)]
```
Get all key-value pairs.
- Complexity: O(n)
- Pure: Yes

## Math Operations

```mint
λabs(n:ℤ)→ℤ
```
Absolute value.
- Complexity: O(1)
- Pure: Yes

```mint
λmin(a:ℤ,b:ℤ)→ℤ
```
Minimum of two integers.
- Complexity: O(1)
- Pure: Yes

```mint
λmax(a:ℤ,b:ℤ)→ℤ
```
Maximum of two integers.
- Complexity: O(1)
- Pure: Yes

```mint
λpow(base:ℤ,exp:ℤ)→ℤ
```
Exponentiation (integer power).
- Complexity: O(log exp)
- Pure: Yes

```mint
λsqrt(n:ℝ)→ℝ
```
Square root.
- Complexity: O(1)
- Pure: Yes

```mint
λfloor(n:ℝ)→ℤ
```
Round down to integer.
- Complexity: O(1)
- Pure: Yes

```mint
λceil(n:ℝ)→ℤ
```
Round up to integer.
- Complexity: O(1)
- Pure: Yes

```mint
λround(n:ℝ)→ℤ
```
Round to nearest integer.
- Complexity: O(1)
- Pure: Yes

## I/O Operations

All I/O operations have the `!IO` effect.

```mint
λprint(s:𝕊)→𝕌!IO
```
Print string to stdout.
- Effect: IO
- Complexity: O(n)

```mint
λprintln(s:𝕊)→𝕌!IO
```
Print string with newline.
- Effect: IO
- Complexity: O(n)

```mint
λread_line()→𝕊!IO
```
Read line from stdin.
- Effect: IO
- Complexity: O(n)

```mint
λread_file(path:𝕊)→Result[𝕊,IoError]!IO
```
Read entire file as string.
- Effect: IO
- Complexity: O(file size)

```mint
λwrite_file(path:𝕊,content:𝕊)→Result[𝕌,IoError]!IO
```
Write string to file.
- Effect: IO
- Complexity: O(n)

## Error Handling

```mint
t IoError={kind:𝕊,msg:𝕊}
t ParseError={line:ℤ,column:ℤ,msg:𝕊}
```

```mint
λpanic[T](msg:𝕊)→T
```
Immediately terminate program with error message.
- Effect: Diverges (returns ∅)
- Use sparingly - prefer Result for recoverable errors

```mint
λassert(condition:𝔹,msg:𝕊)→𝕌
```
Assert condition is true, panic if false.
- Effect: May diverge
- Use for invariants that should never be violated

## Type Conversion

```mint
λint_to_string(n:ℤ)→𝕊
```
Convert integer to string.
- Complexity: O(log n)
- Pure: Yes

```mint
λstring_to_int(s:𝕊)→Result[ℤ,ParseError]
```
Parse integer from string.
- Complexity: O(n)
- Pure: Yes

```mint
λfloat_to_string(n:ℝ)→𝕊
```
Convert float to string.
- Complexity: O(1)
- Pure: Yes

```mint
λstring_to_float(s:𝕊)→Result[ℝ,ParseError]
```
Parse float from string.
- Complexity: O(n)
- Pure: Yes

```mint
λbool_to_string(b:𝔹)→𝕊
```
Convert bool to string ("true" or "false").
- Complexity: O(1)
- Pure: Yes

## Composition Operators

```mint
λcompose[T,U,V](f:λ(U)→V,g:λ(T)→U)→λ(T)→V
```
Function composition: (f ∘ g)(x) = f(g(x))
- Operator: `>>`
- Pure: Yes

```mint
λpipe[T,U](value:T,fn:λ(T)→U)→U
```
Pipe value through function.
- Operator: `|>`
- Pure: Yes

## Module System

### Import Syntax

```mint
i stdlib/io
i stdlib/list_utils
i stdlib/result
```

### Export (Explicit)

Only explicitly exported top-level declarations are visible across modules.

Canonical export forms:

```mint
export λ...
export t...
export c...
```

No selective imports, no aliasing, no export lists.

## Standard Library Modules

### std/prelude

Auto-imported. Contains all core types and functions listed above.

### std/io

I/O operations (read_file, write_file, etc.)

### std/collections

Advanced collections: Set[T], Queue[T], Stack[T]

### std/math

Mathematical functions: sin, cos, tan, log, exp, etc.

### std/json

JSON parsing and serialization

```mint
t JsonValue=JsonNull|JsonBool(𝔹)|JsonInt(ℤ)|JsonFloat(ℝ)|JsonString(𝕊)|JsonArray([JsonValue])|JsonObject({𝕊:JsonValue})

λparse_json(s:𝕊)→Result[JsonValue,ParseError]
λstringify_json(value:JsonValue)→𝕊
```

### std/http

HTTP client and server

```mint
t HttpMethod=GET|POST|PUT|DELETE|PATCH
t HttpRequest={method:HttpMethod,url:𝕊,headers:{𝕊:𝕊},body:𝕊}
t HttpResponse={status:ℤ,headers:{𝕊:𝕊},body:𝕊}

λhttp_get(url:𝕊)→Result[HttpResponse,HttpError]!Network
λhttp_post(url:𝕊,body:𝕊)→Result[HttpResponse,HttpError]!Network
```

### std/async

Async/await primitives (Future type)

```mint
t Future[T]

λasync[T](fn:λ()→T)→Future[T]!Async
λawait[T](future:Future[T])→T!Async
λfuture_map[T,U](fn:λ(T)→U,future:Future[T])→Future[U]
```

### std/test

Testing utilities

```mint
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
- Implementation: stdlib/prelude.mint

---

**Next**: Implement standard library in stdlib/ directory.
