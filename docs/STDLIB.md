# Mint Standard Library

## Overview

The Mint standard library provides core utility functions and predicates for common programming tasks. All functions follow canonical form principles - exactly ONE way to solve each problem.

## Current Status

**Implemented:**
- ✅ List predicates (validation, checking) - `stdlib/list_predicates`
- ✅ Numeric predicates (range checking, properties) - `stdlib/numeric_predicates`
- ✅ List utilities (len, head, tail) - `stdlib/list_utils`

**Not yet implemented:**
- ⏳ Option/Result predicates (requires sum types)
- ⏳ String operations
- ⏳ I/O operations
- ⏳ JSON parsing/serialization

## Import Syntax

```mint
⟦ Import modules (works like FFI - no selective imports) ⟧
i stdlib/list_predicates
i stdlib/numeric_predicates
i stdlib/list_utils

⟦ Use with fully qualified names ⟧
λmain()→𝕌=console.log(
  stdlib/list_predicates.sorted_asc([1,2,3]) ++ " " ++
  stdlib/list_utils.len([1,2,3])
)
```

**Design:** Imports work exactly like FFI (`e module/path`). No selective imports, always use fully qualified names. This prevents name collisions and makes code explicit.

## List Predicates

**Module:** `stdlib/list_predicates`

### sorted_asc

Check if a list is sorted in ascending order.

```mint
λsorted_asc(xs:[ℤ])→𝔹
```

**Examples:**
```mint
sorted_asc([1,2,3])    ⟦ → ⊤ ⟧
sorted_asc([3,2,1])    ⟦ → ⊥ ⟧
sorted_asc([])         ⟦ → ⊤ (empty is sorted) ⟧
sorted_asc([5])        ⟦ → ⊤ (single element is sorted) ⟧
```

**Use case:** Validate precondition for binary search or other sorted-list algorithms.

### sorted_desc

Check if a list is sorted in descending order.

```mint
λsorted_desc(xs:[ℤ])→𝔹
```

**Examples:**
```mint
sorted_desc([3,2,1])   ⟦ → ⊤ ⟧
sorted_desc([1,2,3])   ⟦ → ⊥ ⟧
```

### is_empty

Check if a list is empty.

```mint
λis_empty(xs:[ℤ])→𝔹
```

**Examples:**
```mint
is_empty([])           ⟦ → ⊤ ⟧
is_empty([1])          ⟦ → ⊥ ⟧
```

### is_non_empty

Check if a list is non-empty.

```mint
λis_non_empty(xs:[ℤ])→𝔹
```

**Examples:**
```mint
is_non_empty([1,2])    ⟦ → ⊤ ⟧
is_non_empty([])       ⟦ → ⊥ ⟧
```

### all

Check if all elements in a list satisfy a predicate.

```mint
λall(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹
```

**Examples:**
```mint
all(is_positive,[1,2,3])      ⟦ → ⊤ ⟧
all(is_positive,[1,-2,3])     ⟦ → ⊥ ⟧
all(is_even,[2,4,6])          ⟦ → ⊤ ⟧
```

**Use case:** Validate that all elements meet a requirement.

### any

Check if any element in a list satisfies a predicate.

```mint
λany(pred:λ(ℤ)→𝔹,xs:[ℤ])→𝔹
```

**Examples:**
```mint
any(is_even,[1,3,5])          ⟦ → ⊥ ⟧
any(is_even,[1,2,3])          ⟦ → ⊤ ⟧
any(is_prime,[4,6,8,7])       ⟦ → ⊤ (7 is prime) ⟧
```

**Use case:** Check if at least one element meets a requirement.

### contains

Check if an element exists in a list.

```mint
λcontains(item:ℤ,xs:[ℤ])→𝔹
```

**Examples:**
```mint
contains(3,[1,2,3,4])         ⟦ → ⊤ ⟧
contains(5,[1,2,3,4])         ⟦ → ⊥ ⟧
contains(1,[])                ⟦ → ⊥ ⟧
```

**Use case:** Membership testing.

### in_bounds

Check if an index is valid for a list (in range [0, len-1]).

```mint
λin_bounds(idx:ℤ,xs:[ℤ])→𝔹
```

**Examples:**
```mint
in_bounds(0,[1,2,3])          ⟦ → ⊤ ⟧
in_bounds(2,[1,2,3])          ⟦ → ⊤ ⟧
in_bounds(3,[1,2,3])          ⟦ → ⊥ (out of bounds) ⟧
in_bounds(-1,[1,2,3])         ⟦ → ⊥ (negative index) ⟧
in_bounds(0,[])               ⟦ → ⊥ (empty list) ⟧
```

**Use case:** Validate array/list access before indexing. Prevents out-of-bounds errors.

**Implementation:** Uses `len()` function to check bounds.

## List Utilities

**Module:** `stdlib/list_utils`

### len

Get the length of a list.

```mint
λlen(xs:[ℤ])→ℤ
```

**Examples:**
```mint
len([1,2,3])               ⟦ → 3 ⟧
len([])                    ⟦ → 0 ⟧
len([42])                  ⟦ → 1 ⟧
```

**Algorithm:** Recursive counting with primitive recursion.

**Complexity:** O(n) time, O(n) space (call stack).

### head

Get the first element of a list.

```mint
λhead(xs:[ℤ])→ℤ
```

**Examples:**
```mint
head([1,2,3])              ⟦ → 1 ⟧
head([42])                 ⟦ → 42 ⟧
```

**Warning:** Unsafe - crashes on empty list. Check with `is_non_empty` first.

### tail

Get all elements except the first.

```mint
λtail(xs:[ℤ])→[ℤ]
```

**Examples:**
```mint
tail([1,2,3])              ⟦ → [2,3] ⟧
tail([42])                 ⟦ → [] ⟧
```

**Warning:** Unsafe - crashes on empty list. Check with `is_non_empty` first.

## Numeric Predicates

**Module:** `stdlib/numeric_predicates`

### is_positive

Check if a number is positive (> 0).

```mint
λis_positive(x:ℤ)→𝔹
```

**Examples:**
```mint
is_positive(5)                ⟦ → ⊤ ⟧
is_positive(-3)               ⟦ → ⊥ ⟧
is_positive(0)                ⟦ → ⊥ ⟧
```

### is_negative

Check if a number is negative (< 0).

```mint
λis_negative(x:ℤ)→𝔹
```

**Examples:**
```mint
is_negative(-5)               ⟦ → ⊤ ⟧
is_negative(3)                ⟦ → ⊥ ⟧
is_negative(0)                ⟦ → ⊥ ⟧
```

### is_zero

Check if a number is zero.

```mint
λis_zero(x:ℤ)→𝔹
```

**Examples:**
```mint
is_zero(0)                    ⟦ → ⊤ ⟧
is_zero(5)                    ⟦ → ⊥ ⟧
```

### is_non_negative

Check if a number is non-negative (>= 0).

```mint
λis_non_negative(x:ℤ)→𝔹
```

**Examples:**
```mint
is_non_negative(0)            ⟦ → ⊤ ⟧
is_non_negative(5)            ⟦ → ⊤ ⟧
is_non_negative(-1)           ⟦ → ⊥ ⟧
```

### is_even

Check if a number is even.

```mint
λis_even(x:ℤ)→𝔹
```

**Examples:**
```mint
is_even(4)                    ⟦ → ⊤ ⟧
is_even(5)                    ⟦ → ⊥ ⟧
is_even(0)                    ⟦ → ⊤ ⟧
```

### is_odd

Check if a number is odd.

```mint
λis_odd(x:ℤ)→𝔹
```

**Examples:**
```mint
is_odd(3)                     ⟦ → ⊤ ⟧
is_odd(4)                     ⟦ → ⊥ ⟧
```

**Implementation:** Uses negation of `is_even` for correctness.

### is_prime

Check if a number is prime.

```mint
λis_prime(n:ℤ)→𝔹
```

**Examples:**
```mint
is_prime(2)                   ⟦ → ⊤ ⟧
is_prime(7)                   ⟦ → ⊤ ⟧
is_prime(8)                   ⟦ → ⊥ ⟧
is_prime(17)                  ⟦ → ⊤ ⟧
is_prime(1)                   ⟦ → ⊥ (1 is not prime) ⟧
is_prime(0)                   ⟦ → ⊥ ⟧
```

**Algorithm:** Trial division up to sqrt(n). Uses helper function `is_prime_helper`.

**Performance:** O(sqrt(n)) time complexity.

### in_range

Check if a number is in the inclusive range [min, max].

```mint
λin_range(x:ℤ,min:ℤ,max:ℤ)→𝔹
```

**Examples:**
```mint
in_range(5,1,10)              ⟦ → ⊤ ⟧
in_range(0,1,10)              ⟦ → ⊥ ⟧
in_range(1,1,10)              ⟦ → ⊤ (inclusive bounds) ⟧
in_range(10,1,10)             ⟦ → ⊤ (inclusive bounds) ⟧
```

**Use case:** Bounds validation, input checking.

## Common Patterns

### Validation with Predicates

```mint
⟦ Validate input before processing ⟧
λprocess_positive(x:ℤ)→𝕊≡is_positive(x){
  ⊥→"Error: Must be positive"|
  ⊤→"Processing..."
}
```

### Filtering Lists

```mint
⟦ Filter primes from a list ⟧
λget_primes(xs:[ℤ])→[ℤ]=xs⊳is_prime
```

### Higher-Order Validation

```mint
⟦ Check all values are in range ⟧
λall_in_range(xs:[ℤ])→𝔹=all(λx→in_range(x,0,100),xs)
```

### Precondition Checks

```mint
⟦ Algorithm that requires sorted input ⟧
λbinary_search(xs:[ℤ],target:ℤ)→𝕊≡sorted_asc(xs){
  ⊥→"Error: List must be sorted"|
  ⊤→"Searching..."
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
```mint
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

**Resolution:** Module imports now fully working. Use like FFI: `i stdlib/module` then `stdlib/module.function()`.

### ~~Missing Unicode Operators~~ ✅ FIXED

**Issue:** ~~Typechecker doesn't support ≤, ≥, ≠, ∧, ∨.~~

**Resolution:** Unicode operators now fully supported in typechecker. Predicates updated to use cleaner Unicode syntax.

## Future Additions

### Option Type Predicates

When `Option[T]` sum type is added:
```mint
λis_some[T](opt:Option[T])→𝔹
λis_none[T](opt:Option[T])→𝔹
```

### Result Type Predicates

When `Result[T,E]` sum type is added:
```mint
λis_ok[T,E](res:Result[T,E])→𝔹
λis_err[T,E](res:Result[T,E])→𝔹
```

### String Predicates

```mint
λstr_contains(s:𝕊,substr:𝕊)→𝔹
λstr_starts_with(s:𝕊,prefix:𝕊)→𝔹
λstr_ends_with(s:𝕊,suffix:𝕊)→𝔹
λstr_is_empty(s:𝕊)→𝔹
```

### List Utility Functions

```mint
λlen[T](xs:[T])→ℤ
λhead[T](xs:[T])→Option[T]
λtail[T](xs:[T])→[T]
λreverse[T](xs:[T])→[T]
```

## Contracts (Future)

Predicates will integrate with the future contract system:

```mint
⟦ Today - manual validation ⟧
λbinary_search(xs:[ℤ],target:ℤ)→ℤ≡sorted_asc(xs){
  ⊥→-1|
  ⊤→...
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
- `AGENTS.md` - Mint language guide
