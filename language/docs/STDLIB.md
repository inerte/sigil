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
- ✅ Sum types (Option, Result) - `stdlib/option`, `stdlib/result`
- ✅ Length operator (`#`) - works on strings and lists

**Not yet implemented:**
- ⏳ I/O operations
- ⏳ JSON parsing/serialization

## Import Syntax

```sigil
⟦ Import modules (works like FFI - no selective imports) ⟧
i stdlib⋅list
i stdlib⋅numeric
i stdlib⋅list

⟦ Use with fully qualified names ⟧
λmain()→𝕌=console.log(
  stdlib⋅list.sorted_asc([1,2,3]) ++ " " ++
  stdlib⋅string.int_to_string(#[1,2,3])
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

Sigil modules use explicit exports. Standard library modules export the functions/types they expose via:

```sigil
export λ...
export t...
export c...
```

Imported modules only expose exported members. Accessing a non-exported member is a compile error.

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

### is_empty

Check if a list is empty.

```sigil
λis_empty(xs:[ℤ])→𝔹
```

**Examples:**
```sigil
is_empty([])           ⟦ → true ⟧
is_empty([1])          ⟦ → false ⟧
```

### is_non_empty

Check if a list is non-empty.

```sigil
λis_non_empty(xs:[ℤ])→𝔹
```

**Examples:**
```sigil
is_non_empty([1,2])    ⟦ → true ⟧
is_non_empty([])       ⟦ → false ⟧
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

### head

Get the first element of a list.

```sigil
λhead(xs:[ℤ])→ℤ
```

**Examples:**
```sigil
head([1,2,3])              ⟦ → 1 ⟧
head([42])                 ⟦ → 42 ⟧
```

**Warning:** Unsafe - crashes on empty list. Check with `is_non_empty` first.

### IntOption

Concrete optional integer used by safe integer-list access helpers.

```sigil
t IntOption=IntNone|IntSome(ℤ)
```

### last

Get the last element safely.

```sigil
λlast(xs:[ℤ])→stdlib⋅list.IntOption
```

Examples:
```sigil
stdlib⋅list.last([])         ⟦ → stdlib⋅list.IntNone() ⟧
stdlib⋅list.last([1,2,3])    ⟦ → stdlib⋅list.IntSome(3) ⟧
```

### nth

Get the item at a zero-based index safely.

```sigil
λnth(idx:ℤ,xs:[ℤ])→stdlib⋅list.IntOption
```

Examples:
```sigil
stdlib⋅list.nth(0,[7,8])     ⟦ → stdlib⋅list.IntSome(7) ⟧
stdlib⋅list.nth(2,[7,8])     ⟦ → stdlib⋅list.IntNone() ⟧
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

### tail

Get all elements except the first.

```sigil
λtail(xs:[ℤ])→[ℤ]
```

**Examples:**
```sigil
tail([1,2,3])              ⟦ → [2,3] ⟧
tail([42])                 ⟦ → [] ⟧
```

**Warning:** Unsafe - crashes on empty list. Check with `is_non_empty` first.

### take

Take the first `n` elements.

```sigil
λtake(n:ℤ,xs:[ℤ])→[ℤ]
```

## Numeric Helpers

**Module:** `stdlib/numeric`

### range_inclusive

Build an inclusive ascending integer range.

```sigil
λrange_inclusive(start:ℤ,stop:ℤ)→[ℤ]
```

Examples:
```sigil
stdlib⋅numeric.range_inclusive(2,5)   ⟦ → [2,3,4,5] ⟧
stdlib⋅numeric.range_inclusive(3,3)   ⟦ → [3] ⟧
stdlib⋅numeric.range_inclusive(5,2)   ⟦ → [] ⟧
```

## String Operations

**Module:** `stdlib/string`

Comprehensive string manipulation functions. These are **compiler intrinsics** - the compiler emits optimized JavaScript directly instead of calling Sigil functions.

### char_at

Get character at index.

```sigil
λchar_at(s:𝕊,idx:ℤ)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.char_at("hello",0)    ⟦ → "h" ⟧
stdlib⋅string.char_at("hello",4)    ⟦ → "o" ⟧
```

**Codegen:** `s.charAt(idx)`

### substring

Get substring from start to end index.

```sigil
λsubstring(s:𝕊,start:ℤ,end:ℤ)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.substring("hello world",6,11)    ⟦ → "world" ⟧
stdlib⋅string.substring("hello",0,3)           ⟦ → "hel" ⟧
```

**Codegen:** `s.substring(start, end)`

### take

Take first n characters.

```sigil
λtake(s:𝕊,n:ℤ)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.take("hello",3)    ⟦ → "hel" ⟧
stdlib⋅string.take("hi",5)       ⟦ → "hi" (takes available chars) ⟧
```

**Implementation:** `substring(s, 0, n)` (in Sigil)

### drop

Drop first n characters.

```sigil
λdrop(s:𝕊,n:ℤ)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.drop("hello",2)    ⟦ → "llo" ⟧
stdlib⋅string.drop("hi",5)       ⟦ → "" (drops all available) ⟧
```

**Implementation:** `substring(s, n, #s)` (in Sigil, uses `#` operator)

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
λsplit(s:𝕊,delimiter:𝕊)→[𝕊]
```

**Examples:**
```sigil
stdlib⋅string.split("a,b,c",",")           ⟦ → ["a","b","c"] ⟧
stdlib⋅string.split("line1\nline2","\n")   ⟦ → ["line1","line2"] ⟧
```

**Codegen:** `s.split(delimiter)`

### replace_all

Replace all occurrences of pattern with replacement.

```sigil
λreplace_all(s:𝕊,pattern:𝕊,replacement:𝕊)→𝕊
```

**Examples:**
```sigil
stdlib⋅string.replace_all("hello hello","hello","hi")    ⟦ → "hi hi" ⟧
```

**Codegen:** `s.replaceAll(pattern, replacement)`

## String Predicates

**Module:** `stdlib/string`

Boolean validation predicates for string properties. These are **compiler intrinsics**.

### starts_with

Check if string starts with prefix.

```sigil
λstarts_with(s:𝕊,prefix:𝕊)→𝔹
```

**Examples:**
```sigil
stdlib⋅string.starts_with("# Title","# ")    ⟦ → true ⟧
stdlib⋅string.starts_with("Title","# ")      ⟦ → false ⟧
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

**Design Note:** No redundant predicates like `is_empty`, `is_whitespace`, or `contains`. Users compose these:
- `is_empty(s)` → `#s = 0`
- `is_whitespace(s)` → `stdlib⋅string.trim(s) = ""`
- `contains(s, search)` → `stdlib⋅string.index_of(s, search) ≠ -1`

This follows Sigil's "ONE way to do things" philosophy.

## Numeric Predicates

**Module:** `stdlib/numeric`

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

### is_zero

Check if a number is zero.

```sigil
λis_zero(x:ℤ)→𝔹
```

**Examples:**
```sigil
is_zero(0)                    ⟦ → true ⟧
is_zero(5)                    ⟦ → false ⟧
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

## Sum Types

**Modules:** `stdlib/option`, `stdlib/result`

### Option[T]

Represents an optional value - either `Some(T)` or `None`.

```sigil
i stdlib⋅option

t Option[T]=Some(T)|None
```

**Type declaration:**
```sigil
t Option[T]=Some(T)|None
```

**Usage:**
```sigil
⟦ Pattern matching on Option ⟧
λgetOrDefault(opt:Option,default:ℤ)→ℤ match opt{
  Some(x)→x|
  None→default
}

⟦ Safe division returning Option ⟧
λdivide(a:ℤ,b:ℤ)→Option match b{
  0→None()|
  b→Some(a/b)
}
```

**Note:** Generic utility functions like `map[T,U](opt,fn)` not yet available due to incomplete generic type inference.

### Result[T,E]

Represents success (`Ok(T)`) or failure (`Err(E)`).

```sigil
i stdlib⋅result

t Result[T,E]=Ok(T)|Err(E)
```

**Type declaration:**
```sigil
t Result[T,E]=Ok(T)|Err(E)
```

**Usage:**
```sigil
⟦ Pattern matching on Result ⟧
λprocessResult(res:Result)→𝕊 match res{
  Ok(value)→"Success: "+value|
  Err(msg)→"Error: "+msg
}

⟦ Safe parsing returning Result ⟧
λparsePositive(s:𝕊)→Result match validInput(s){
  true→Ok(parseInt(s))|
  false→Err("invalid input")
}
```

**See also:** `examples/sum-types-demo.sigil` for comprehensive examples.

## Future Additions

### Option/Result Utility Functions

When generic type inference is complete:
```sigil
λmap[T,U](opt:Option[T],fn:λ(T)→U)→Option[U]
λunwrap_or[T](opt:Option[T],default:T)→T
λmap[T,U,E](res:Result[T,E],fn:λ(T)→U)→Result[U,E]
λunwrap[T,E](res:Result[T,E])→T
```

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
