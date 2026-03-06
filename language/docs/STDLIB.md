# Sigil Standard Library

## Overview

The Sigil standard library provides core utility functions and predicates for common programming tasks. All functions follow canonical form principles - exactly ONE way to solve each problem.

## Current Status

**Implemented:**
- ✅ List predicates (validation, checking) - `stdlib/list`
- ✅ Numeric predicates and ranges - `stdlib/numeric`
- ✅ List utilities (head, tail, take/drop/reverse, safe lookup) - `stdlib/list`
- ✅ String operations (manipulation, searching) - `stdlib/string`
- ✅ String predicates (prefix/suffix checking) - `stdlib/string`
- ✅ File system operations - `stdlib/file`
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
i stdlib⋅list
i stdlib⋅json
i stdlib⋅file
i stdlib⋅numeric
i stdlib⋅path
i stdlib⋅string
i stdlib⋅time
i stdlib⋅url

⟦ Use with fully qualified names ⟧
λmain()→𝕌=console.log(
  stdlib⋅string.int_to_string(#[1,2,3]) ++ " " ++
  stdlib⋅time.format_iso(stdlib⋅time.from_epoch_millis(0))
)
```

**Design:** Imports work exactly like FFI (`e module⋅path`). No selective imports, always use fully qualified names. This prevents name collisions and makes code explicit.

## Length Operator (`#`)

The `#` operator is a **built-in language operator** that returns the length of strings and lists.

**Syntax:**
```sigil
#expression → ℤ
```

**Type Checking:**
- Works on strings (`𝕊`) and lists (`[T]`)
- Compile error for other types
- Always returns integer (`ℤ`)

**Examples:**
```sigil
#"hello"        ⟦ → 5 ⟧
#""             ⟦ → 0 ⟧
#[1,2,3]        ⟦ → 3 ⟧
#[]             ⟦ → 0 (empty list type inferred from context) ⟧
```

**Note on Empty Lists:**
Empty lists `[]` infer their type from context:
- In pattern matching: First arm establishes the type
- In function return: Return type annotation provides context
- In standalone expressions: Type cannot be inferred (use function with explicit return type)

**Why `#` instead of functions?**

1. **ONE canonical form** - Not `stdlib⋅string` helper calls vs `stdlib⋅list` helper calls, just `#`
2. **Leverages bidirectional type checking** - Type is known at compile time
3. **Concise** - Machine-first language optimizes for brevity (`#s` vs `len(s)`)
4. **Zero syntactic variation** - Single way to express "get length"

**Codegen:**
```typescript
#s          → (await s).length
#[1,2,3]    → (await [1,2,3]).length
```

**Note:** The deprecated `stdlib⋅list.len` function has been removed. Use `#` instead.

## Module Exports

Sigil uses file-based visibility:
- `.lib.sigil` exports all top-level declarations automatically
- `.sigil` files are executables and are not importable (outside tests)

There is no `export` keyword.

## File, Path, JSON, Time, and URL

`stdlib⋅file` exposes canonical UTF-8 filesystem helpers:

```sigil
i stdlib⋅file
i stdlib⋅path

λmain()→!IO 𝕌=
  l out=(stdlib⋅path.join("/tmp","sigil.txt"):𝕊);
  l _=(stdlib⋅file.write_text("hello",out):𝕌);
  l _2=(stdlib⋅file.read_text(out):𝕊);
  ()
```

`stdlib⋅path` exposes canonical filesystem path operations:

```sigil
i stdlib⋅path

λmain()→𝕌=
  l _=(stdlib⋅path.basename("website/articles/hello.md"):𝕊);
  l _2=(stdlib⋅path.join("website","articles"):𝕊);
  ()
```

`stdlib⋅json` exposes a typed JSON AST with safe parsing:

```sigil
i stdlib⋅json

λmain()→𝕌=
  match stdlib⋅json.parse("{\"ok\":true}"){
    Ok(value)→match stdlib⋅json.as_object(value){
      Some(_)→()|
      None→()
    }|
    Err(_)→()
  }
```

`stdlib⋅time` exposes strict ISO parsing and instant comparison:

```sigil
i stdlib⋅time

λmain()→𝕌=
  match stdlib⋅time.parse_iso("2026-03-03"){
    Ok(instant)→
      l _=(stdlib⋅time.to_epoch_millis(instant):ℤ);
      ()|
    Err(_)→()
  }
```

`stdlib⋅url` exposes strict parse results and typed URL fields for both absolute and relative targets:

```sigil
i stdlib⋅url

λmain()→𝕌=
  match stdlib⋅url.parse("../language/spec/cli-json.md?view=raw#schema"){
    Ok(url)→
      l _=(url.path:𝕊);
      l _2=(stdlib⋅url.suffix(url):𝕊);
      ()|
    Err(_)→()
  }
```

## List Predicates

**Module:** `stdlib/list`

### sorted_asc

Check if a list is sorted in ascending order.

```sigil
λsorted_asc(xs:[ℤ])→𝔹
```

**Examples:**
```sigil
sorted_asc([1,2,3])    ⟦ → true ⟧
sorted_asc([3,2,1])    ⟦ → false ⟧
sorted_asc([])         ⟦ → true (empty is sorted) ⟧
sorted_asc([5])        ⟦ → true (single element is sorted) ⟧
```

**Use case:** Validate precondition for binary search or other sorted-list algorithms.

### sorted_desc

Check if a list is sorted in descending order.

```sigil
λsorted_desc(xs:[ℤ])→𝔹
```

**Examples:**
```sigil
sorted_desc([3,2,1])   ⟦ → true ⟧
sorted_desc([1,2,3])   ⟦ → false ⟧
```

### all

Check if all elements in a list satisfy a predicate.

```sigil
λall(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹
```

**Examples:**
```sigil
all(is_positive,[1,2,3])      ⟦ → true ⟧
all(is_positive,[1,-2,3])     ⟦ → false ⟧
all(is_even,[2,4,6])          ⟦ → true ⟧
```

**Use case:** Validate that all elements meet a requirement.

### any

Check if any element in a list satisfies a predicate.

```sigil
λany(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹
```

**Examples:**
```sigil
any(is_even,[1,3,5])          ⟦ → false ⟧
any(is_even,[1,2,3])          ⟦ → true ⟧
any(is_prime,[4,6,8,7])       ⟦ → true (7 is prime) ⟧
```

**Use case:** Check if at least one element meets a requirement.

### contains

Check if an element exists in a list.

```sigil
λcontains(item:ℤ,xs:[ℤ])→𝔹
```

**Examples:**
```sigil
contains(3,[1,2,3,4])         ⟦ → true ⟧
contains(5,[1,2,3,4])         ⟦ → false ⟧
contains(1,[])                ⟦ → false ⟧
```

**Use case:** Membership testing.

### count

Count occurrences of an element in a list.

```sigil
λcount(item:ℤ,xs:[ℤ])→ℤ
```

### drop

Drop the first `n` elements.

```sigil
λdrop(n:ℤ,xs:[ℤ])→[ℤ]
```

### find

Find the first element that satisfies a predicate.

```sigil
λfind[T](pred:λ(T)→𝔹,xs:[T])→Option[T]
```

Examples:
```sigil
stdlib⋅list.find(stdlib⋅numeric.is_even,[1,3,4,6])   ⟦ → Some(4) ⟧
stdlib⋅list.find(stdlib⋅numeric.is_even,[1,3,5])     ⟦ → None() ⟧
```

### fold

Reduce a list to a single value by threading an accumulator from left to right.

```sigil
λfold[T,U](acc:U,fn:λ(U,T)→U,xs:[T])→U
```

Examples:
```sigil
stdlib⋅list.fold(0,λ(acc:ℤ,x:ℤ)→ℤ=acc+x,[1,2,3])   ⟦ → 6 ⟧
stdlib⋅list.fold(0,λ(acc:ℤ,x:ℤ)→ℤ=acc*10+x,[1,2,3]) ⟦ → 123 ⟧
```

### in_bounds

Check if an index is valid for a list (in range [0, len-1]).

```sigil
λin_bounds(idx:ℤ,xs:[ℤ])→𝔹
```

**Examples:**
```sigil
in_bounds(0,[1,2,3])          ⟦ → true ⟧
in_bounds(2,[1,2,3])          ⟦ → true ⟧
in_bounds(3,[1,2,3])          ⟦ → false (out of bounds) ⟧
in_bounds(-1,[1,2,3])         ⟦ → false (negative index) ⟧
in_bounds(0,[])               ⟦ → false (empty list) ⟧
```

**Use case:** Validate array/list access before indexing. Prevents out-of-bounds errors.

**Implementation:** Uses `#xs` to check bounds.

## List Utilities

**Module:** `stdlib/list`

**Note:** Use the `#` operator for list length instead of a function (e.g., `#[1,2,3]` → `3`).

### last

Get the last element safely.

```sigil
λlast[T](xs:[T])→Option[T]
```

Examples:
```sigil
stdlib⋅list.last([])         ⟦ → None() ⟧
stdlib⋅list.last([1,2,3])    ⟦ → Some(3) ⟧
```

### max

Get the maximum element safely.

```sigil
λmax(xs:[ℤ])→Option[ℤ]
```

Examples:
```sigil
stdlib⋅list.max([])          ⟦ → None() ⟧
stdlib⋅list.max([3,9,4])     ⟦ → Some(9) ⟧
```

### min

Get the minimum element safely.

```sigil
λmin(xs:[ℤ])→Option[ℤ]
```

Examples:
```sigil
stdlib⋅list.min([])          ⟦ → None() ⟧
stdlib⋅list.min([3,9,4])     ⟦ → Some(3) ⟧
```

### nth

Get the item at a zero-based index safely.

```sigil
λnth[T](idx:ℤ,xs:[T])→Option[T]
```

Examples:
```sigil
stdlib⋅list.nth(0,[7,8])     ⟦ → Some(7) ⟧
stdlib⋅list.nth(2,[7,8])     ⟦ → None() ⟧
```

### product

Multiply all integers in a list.

```sigil
λproduct(xs:[ℤ])→ℤ
```

Examples:
```sigil
stdlib⋅list.product([])         ⟦ → 1 ⟧
stdlib⋅list.product([2,3,4])    ⟦ → 24 ⟧
```

### remove_first

Remove the first occurrence of an element.

```sigil
λremove_first(item:ℤ,xs:[ℤ])→[ℤ]
```

### reverse

Reverse a list.

```sigil
λreverse(xs:[ℤ])→[ℤ]
```

### sum

Sum all integers in a list.

```sigil
λsum(xs:[ℤ])→ℤ
```

Examples:
```sigil
stdlib⋅list.sum([])          ⟦ → 0 ⟧
stdlib⋅list.sum([1,2,3,4])   ⟦ → 10 ⟧
```

### take

Take the first `n` elements.

```sigil
λtake(n:ℤ,xs:[ℤ])→[ℤ]
```

## Numeric Helpers

**Module:** `stdlib/numeric`

### range

Build an ascending integer range, inclusive at both ends.

```sigil
λrange(start:ℤ,stop:ℤ)→[ℤ]
```

Examples:
```sigil
stdlib⋅numeric.range(2,5)   ⟦ → [2,3,4,5] ⟧
stdlib⋅numeric.range(3,3)   ⟦ → [3] ⟧
stdlib⋅numeric.range(5,2)   ⟦ → [] ⟧
```

## String Operations

**Module:** `stdlib/string`

Comprehensive string manipulation functions. These are **compiler intrinsics** - the compiler emits optimized JavaScript directly instead of calling Sigil functions.

### char_at

Get character at index.

```sigil
λchar_at(idx:ℤ,s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.char_at(0,"hello")    ⟦ → "h" ⟧
stdlib⋅string.char_at(4,"hello")    ⟦ → "o" ⟧
```

**Codegen:** `s.charAt(idx)`

### substring

Get substring from start to end index.

```sigil
λsubstring(end:ℤ,s:𝕊,start:ℤ)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.substring(11,"hello world",6)    ⟦ → "world" ⟧
stdlib⋅string.substring(3,"hello",0)           ⟦ → "hel" ⟧
```

**Codegen:** `s.substring(start, end)`

### take

Take first n characters.

```sigil
λtake(n:ℤ,s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.take(3,"hello")    ⟦ → "hel" ⟧
stdlib⋅string.take(5,"hi")       ⟦ → "hi" (takes available chars) ⟧
```

**Implementation:** `substring(n, s, 0)` (in Sigil)

### drop

Drop first n characters.

```sigil
λdrop(n:ℤ,s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.drop(2,"hello")    ⟦ → "llo" ⟧
stdlib⋅string.drop(5,"hi")       ⟦ → "" (drops all available) ⟧
```

**Implementation:** `substring(#s, s, n)` (in Sigil, uses `#` operator)

### lines

Split a string on newline characters.

```sigil
λlines(s:𝕊)→[𝕊]
```

**Examples:**
```sigil
stdlib⋅string.lines("a\nb\nc")    ⟦ → ["a","b","c"] ⟧
stdlib⋅string.lines("hello")      ⟦ → ["hello"] ⟧
```

**Implementation:** `split("\n", s)` (in Sigil)

### to_upper

Convert to uppercase.

```sigil
λto_upper(s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.to_upper("hello")    ⟦ → "HELLO" ⟧
```

**Codegen:** `s.toUpperCase()`

### to_lower

Convert to lowercase.

```sigil
λto_lower(s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.to_lower("WORLD")    ⟦ → "world" ⟧
```

**Codegen:** `s.toLowerCase()`

### trim

Remove leading and trailing whitespace.

```sigil
λtrim(s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.trim("  hello  ")    ⟦ → "hello" ⟧
stdlib⋅string.trim("\n\ttest\n")   ⟦ → "test" ⟧
```

**Codegen:** `s.trim()`

### index_of

Find index of first occurrence (returns -1 if not found).

```sigil
λindex_of(s:𝕊,search:𝕊)→ℤ
```

**Examples:**
```sigil
stdlib⋅string.index_of("hello world","world")    ⟦ → 6 ⟧
stdlib⋅string.index_of("hello","xyz")            ⟦ → -1 ⟧
```

**Codegen:** `s.indexOf(search)`

### split

Split string by delimiter.

```sigil
λsplit(delimiter:𝕊,s:𝕊)→[𝕊]
```

**Examples:**
```sigil
stdlib⋅string.split(",","a,b,c")           ⟦ → ["a","b","c"] ⟧
stdlib⋅string.split("\n","line1\nline2")   ⟦ → ["line1","line2"] ⟧
```

**Codegen:** `s.split(delimiter)`

### replace_all

Replace all occurrences of pattern with replacement.

```sigil
λreplace_all(pattern:𝕊,replacement:𝕊,s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.replace_all("hello","hi","hello hello")    ⟦ → "hi hi" ⟧
```

**Codegen:** `s.replaceAll(pattern, replacement)`

### repeat

Repeat a string `count` times.

```sigil
λrepeat(count:ℤ,s:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.repeat(3,"ab")    ⟦ → "ababab" ⟧
stdlib⋅string.repeat(0,"ab")    ⟦ → "" ⟧
```

**Implementation:** recursive concatenation in Sigil

## String Predicates

**Module:** `stdlib/string`

Boolean validation predicates for string properties. These are **compiler intrinsics**.

### starts_with

Check if string starts with prefix.

```sigil
λstarts_with(prefix:𝕊,s:𝕊)→𝔹
```

**Examples:**
```sigil
stdlib⋅string.starts_with("# ","# Title")    ⟦ → true ⟧
stdlib⋅string.starts_with("# ","Title")      ⟦ → false ⟧
```

**Codegen:** `s.startsWith(prefix)`

**Use case:** Parse markdown headers, check file extensions.

### ends_with

Check if string ends with suffix.

```sigil
λends_with(s:𝕊,suffix:𝕊)→𝔹
```

**Examples:**
```sigil
stdlib⋅string.ends_with("test.sigil",".sigil")    ⟦ → true ⟧
stdlib⋅string.ends_with("test.txt",".sigil")      ⟦ → false ⟧
```

**Codegen:** `s.endsWith(suffix)`

**Use case:** File extension checking, URL validation.

### is_digit

Check whether a string is exactly one decimal digit.

```sigil
λis_digit(s:𝕊)→𝔹
```

**Examples:**
```sigil
stdlib⋅string.is_digit("5")     ⟦ → true ⟧
stdlib⋅string.is_digit("42")    ⟦ → false ⟧
```

**Codegen:** `/^[0-9]$/.test(s)`

### unlines

Join lines with newline separators.

```sigil
λunlines(lines:[𝕊])→𝕊
```

**Examples:**
```sigil
stdlib⋅string.unlines(["a","b","c"])    ⟦ → "a\nb\nc" ⟧
stdlib⋅string.unlines([])               ⟦ → "" ⟧
```

**Implementation:** `join("\n", lines)` (in Sigil)

**Design Note:** No redundant predicates like `is_empty`, `is_whitespace`, or `contains`. Users compose these:
- `is_empty(s)` → `#s = 0`
- `is_whitespace(s)` → `stdlib⋅string.trim(s) = ""`
- `contains(s, search)` → `stdlib⋅string.index_of(s, search) ≠ -1`

This follows Sigil's "ONE way to do things" philosophy.

## Numeric Predicates

**Module:** `stdlib/numeric`

### abs

Absolute value of an integer.

```sigil
λabs(x:ℤ)→ℤ
```

Examples:
```sigil
stdlib⋅numeric.abs(-5)   ⟦ → 5 ⟧
stdlib⋅numeric.abs(7)    ⟦ → 7 ⟧
```

### DivMod

Quotient and remainder pair returned by `divmod`.

```sigil
t DivMod={quotient:ℤ,remainder:ℤ}
```

### divmod

Return integer quotient and Euclidean remainder together.

```sigil
λdivmod(a:ℤ,b:ℤ)→stdlib⋅numeric.DivMod
```

Examples:
```sigil
stdlib⋅numeric.divmod(17,5)    ⟦ → DivMod{quotient:3,remainder:2} ⟧
stdlib⋅numeric.divmod(-17,5)   ⟦ → DivMod{quotient:-4,remainder:3} ⟧
```

### is_positive

Check if a number is positive (> 0).

```sigil
λis_positive(x:ℤ)→𝔹
```

**Examples:**
```sigil
is_positive(5)                ⟦ → true ⟧
is_positive(-3)               ⟦ → false ⟧
is_positive(0)                ⟦ → false ⟧
```

### is_negative

Check if a number is negative (< 0).

```sigil
λis_negative(x:ℤ)→𝔹
```

**Examples:**
```sigil
is_negative(-5)               ⟦ → true ⟧
is_negative(3)                ⟦ → false ⟧
is_negative(0)                ⟦ → false ⟧
```

### is_non_negative

Check if a number is non-negative (>= 0).

```sigil
λis_non_negative(x:ℤ)→𝔹
```

**Examples:**
```sigil
is_non_negative(0)            ⟦ → true ⟧
is_non_negative(5)            ⟦ → true ⟧
is_non_negative(-1)           ⟦ → false ⟧
```

### is_even

Check if a number is even.

```sigil
λis_even(x:ℤ)→𝔹
```

**Examples:**
```sigil
is_even(4)                    ⟦ → true ⟧
is_even(5)                    ⟦ → false ⟧
is_even(0)                    ⟦ → true ⟧
```

### is_odd

Check if a number is odd.

```sigil
λis_odd(x:ℤ)→𝔹
```

**Examples:**
```sigil
is_odd(3)                     ⟦ → true ⟧
is_odd(4)                     ⟦ → false ⟧
```

**Implementation:** Uses negation of `is_even` for correctness.

### is_prime

Check if a number is prime.

```sigil
λis_prime(n:ℤ)→𝔹
```

**Examples:**
```sigil
is_prime(2)                   ⟦ → true ⟧
is_prime(7)                   ⟦ → true ⟧
is_prime(8)                   ⟦ → false ⟧
is_prime(17)                  ⟦ → true ⟧
is_prime(1)                   ⟦ → false (1 is not prime) ⟧
is_prime(0)                   ⟦ → false ⟧
```

**Algorithm:** Trial division up to sqrt(n). Uses helper function `is_prime_helper`.

**Performance:** O(sqrt(n)) time complexity.

### lcm

Least common multiple.

```sigil
λlcm(a:ℤ,b:ℤ)→ℤ
```

Examples:
```sigil
stdlib⋅numeric.lcm(6,8)     ⟦ → 24 ⟧
stdlib⋅numeric.lcm(-6,8)    ⟦ → 24 ⟧
stdlib⋅numeric.lcm(0,7)     ⟦ → 0 ⟧
```

### mod

Euclidean modulo with a non-negative remainder.

```sigil
λmod(a:ℤ,b:ℤ)→ℤ
```

Examples:
```sigil
stdlib⋅numeric.mod(17,5)     ⟦ → 2 ⟧
stdlib⋅numeric.mod(-17,5)    ⟦ → 3 ⟧
stdlib⋅numeric.mod(17,-5)    ⟦ → 2 ⟧
```

### in_range

Check if a number is in the inclusive range [min, max].

```sigil
λin_range(x:ℤ,min:ℤ,max:ℤ)→𝔹
```

**Examples:**
```sigil
in_range(5,1,10)              ⟦ → true ⟧
in_range(0,1,10)              ⟦ → false ⟧
in_range(1,1,10)              ⟦ → true (inclusive bounds) ⟧
in_range(10,1,10)             ⟦ → true (inclusive bounds) ⟧
```

**Use case:** Bounds validation, input checking.

### sign

Return `-1`, `0`, or `1` based on the sign of the input.

```sigil
λsign(x:ℤ)→ℤ
```

Examples:
```sigil
stdlib⋅numeric.sign(-8)    ⟦ → -1 ⟧
stdlib⋅numeric.sign(0)     ⟦ → 0 ⟧
stdlib⋅numeric.sign(12)    ⟦ → 1 ⟧
```

## Common Patterns

### Validation with Predicates

```sigil
⟦ Validate input before processing ⟧
λprocess_positive(x:ℤ)→𝕊 match is_positive(x){
  false→"Error: Must be positive"|
  true→"Processing..."
}
```

### Filtering Lists

```sigil
⟦ Filter primes from a list ⟧
λget_primes(xs:[ℤ])→[ℤ]=xs⊳is_prime
```

### Higher-Order Validation

```sigil
⟦ Check all values are in range ⟧
λall_in_range(xs:[ℤ])→𝔹=all(λx→in_range(x,0,100),xs)
```

### Precondition Checks

```sigil
⟦ Algorithm that requires sorted input ⟧
λbinary_search(xs:[ℤ],target:ℤ)→𝕊 match sorted_asc(xs){
  false→"Error: List must be sorted"|
  true→"Searching..."
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
λis_prime(n:ℤ)→𝔹=...
λis_prime_helper(n:ℤ,divisor:ℤ)→𝔹=...  ⟦ Allowed ⟧
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

**Resolution:** Module imports now fully working. Use like FFI: `i stdlib⋅module` then `stdlib⋅module.function()`.

### ~~Missing Unicode Operators~~ ✅ FIXED

**Issue:** ~~Typechecker doesn't support ≤, ≥, ≠, and, or.~~

**Resolution:** Unicode operators now fully supported in typechecker. Predicates updated to use cleaner Unicode syntax.

## Core Prelude

`Option[T]`, `Result[T,E]`, `Some`, `None`, `Ok`, and `Err` are part of the implicit `core⋅prelude`. They do not require imports.

### Option[T]

Represents an optional value - either `Some(T)` or `None`.

**Type declaration:**
```sigil
t Option[T]=Some(T)|None
```

**Usage:**
```sigil
⟦ Pattern matching on Option ⟧
λgetOrDefault(default:ℤ,opt:Option[ℤ])→ℤ match opt{
  Some(x)→x|
  None()→default
}

⟦ Safe division returning Option ⟧
λdivide(a:ℤ,b:ℤ)→Option[ℤ] match b{
  0→None()|
  b→Some(a/b)
}
```

**Implemented helpers:**
```sigil
λbind_option[T,U](fn:λ(T)→Option[U],opt:Option[T])→Option[U]
λis_none[T](opt:Option[T])→𝔹
λis_some[T](opt:Option[T])→𝔹
λmap_option[T,U](fn:λ(T)→U,opt:Option[T])→Option[U]
λunwrap_or[T](fallback:T,opt:Option[T])→T
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
λprocessResult(res:Result[𝕊,𝕊])→𝕊 match res{
  Ok(value)→"Success: "+value|
  Err(msg)→"Error: "+msg
}

⟦ Safe parsing returning Result ⟧
λparsePositive(s:𝕊)→Result[ℤ,𝕊] match validInput(s){
  true→Ok(parseInt(s))|
  false→Err("invalid input")
}
```

**Implemented helpers:**
```sigil
λbind_result[T,U,E](fn:λ(T)→Result[U,E],res:Result[T,E])→Result[U,E]
λis_err[T,E](res:Result[T,E])→𝔹
λis_ok[T,E](res:Result[T,E])→𝔹
λmap_result[T,U,E](fn:λ(T)→U,res:Result[T,E])→Result[U,E]
λunwrap_or_result[T,E](fallback:T,res:Result[T,E])→T
```

**See also:** `examples/sum-types-demo.sigil` for comprehensive examples.

### Core Helper Modules

Use these when you need operational helpers:

```sigil
i core⋅map
i core⋅option
i core⋅result
```

### Core Map

`Map` is a core collection concept, not a stdlib-only add-on.

Canonical type and literal forms:

```sigil
{𝕊↦𝕊}
{"content-type"↦"text/plain"}
({↦}:{𝕊↦𝕊})
```

Canonical helper module:

```sigil
i core⋅map
```

## Future Additions

### String Predicates

```sigil
λstr_contains(s:𝕊,substr:𝕊)→𝔹
λstr_starts_with(s:𝕊,prefix:𝕊)→𝔹
λstr_ends_with(s:𝕊,suffix:𝕊)→𝔹
λstr_is_empty(s:𝕊)→𝔹
```

### List Utility Functions

```sigil
λlen[T](xs:[T])→ℤ
λhead[T](xs:[T])→Option[T]
λtail[T](xs:[T])→[T]
λreverse[T](xs:[T])→[T]
```

## Contracts (Future)

Predicates will integrate with the future contract system:

```sigil
⟦ Today - manual validation ⟧
λbinary_search(xs:[ℤ],target:ℤ)→ℤ match sorted_asc(xs){
  false→-1|
  true→...
}

⟦ Future - contracts with predicates ⟧
λbinary_search(xs:[ℤ],target:ℤ)→ℤ
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
