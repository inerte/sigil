# Mint Mutability System

## Overview

Mint uses **immutable by default** with explicit `mut` annotations for mutability.

**Purpose:** The `mut` keyword is primarily for **FFI type safety** - marking JavaScript functions that mutate their arguments. This prevents accidental aliasing bugs when calling JavaScript code.

**Note:** Mint itself has NO mutating operations. All list operations (↦, ⊳, ⊕) are immutable. This preserves canonical forms - there's exactly ONE way to write each algorithm.

## Rules

### Rule 1: Immutable by Default

All values are immutable unless marked `mut`:

```mint
λsum(list:[ℤ])→ℤ=list⊕(λ(a:ℤ,x:ℤ)→ℤ=a+x)⊕0
⟦ list cannot be modified ⟧
```

### Rule 2: Explicit Mutability

Use `mut` keyword for mutable parameters:

```mint
λsort(list:mut [ℤ])→𝕌=quicksort_impl(list)
⟦ list will be modified in place ⟧
```

### Rule 3: No Aliasing of Mutables

Cannot create multiple references to mutable values:

```mint
⟦ ERROR: Cannot alias mutable ⟧
λbad(x:mut [ℤ])→𝕌≡{
  let y=x    ⟦ ERROR: Can't create alias ⟧
}

⟦ OK: Direct use ⟧
λgood(x:mut [ℤ])→𝕌=modify(x)
```

### Rule 4: FFI Mutation Tracking

The `mut` keyword is used when calling JavaScript functions that mutate:

```mint
e Array
λsortJS(arr:mut [ℤ])→𝕌=Array.sort(arr)  ⟦ JS Array.sort mutates ⟧

⟦ Pure Mint code uses immutable operations ⟧
λsorted(list:[ℤ])→[ℤ]=list↦λ(x)→x  ⟦ Returns new sorted list ⟧
```

## Examples

### Valid Code

```mint
# Immutable list operations (canonical form)
λdouble(list:[ℤ])→[ℤ]=list↦λ(x:ℤ)→ℤ=x*2

# FFI with mutation
e Array
λsortArray(arr:mut [ℤ])→𝕌=Array.sort(arr)

# Multiple immutable uses (OK)
λprocess(data:[ℤ])→ℤ≡{
  let sum=data⊕λ(a,x)→a+x⊕0
  let len=data⊕λ(a,_)→a+1⊕0
  sum/len
}
```

### Errors Prevented

```mint
# Error: Aliasing mutable
λbad1(x:mut [ℤ])→𝕌≡{
  let y=x    # Error: Cannot create alias of mutable value 'x'
}

# Error: Passing immutable to mutable parameter (FFI)
e Array
λbad2()→𝕌≡{
  let data=[1,2,3]
  Array.sort(data)    # Error: Cannot pass immutable 'data' to mut parameter
}
```

## Why Mutability Checking?

### Problems It Prevents

**1. Accidental Mutation (FFI):**
```mint
e Array

# Without mutability checking:
λprocess(data:[ℤ])→[ℤ]≡{
  Array.sort(data);    # Oops! Modified input
  data
}

# With mutability checking:
# Compile error: Cannot pass immutable 'data' to mut parameter
```

**2. Aliasing Bugs:**
```mint
# Without mutability checking:
λbug(x:mut [ℤ])→𝕌≡{
  let y=x
  modify!(x)    # Modifies through x
  process(y)    # y changed too!
}

# With mutability checking:
# Compile error: Cannot create alias of mutable value 'x'
```

**3. Unclear Intent:**
```mint
# Pure Mint code - always immutable
λsorted(data:[ℤ])→[ℤ]=...        # Returns new list (canonical)

# FFI - mut signals mutation
e Array
λsortArray(arr:mut [ℤ])→𝕌=Array.sort(arr)  # Mutates via FFI
```

## Comparison to Other Languages

| Language | Approach | Complexity | Memory Safety |
|----------|----------|------------|---------------|
| **Rust** | Borrow checker with `&`, `&mut`, lifetimes | High | Yes (prevents use-after-free) |
| **TypeScript** | No mutability tracking | None | No |
| **Mint** | `mut` keyword with aliasing prevention | Low | No (relies on JS GC) |

### Why Not Full Borrow Checking?

**Rust needs borrow checking because:**
- Manual memory management
- Prevents use-after-free, double-free, data races
- Systems programming requirements

**Mint doesn't need it because:**
- Compiles to TypeScript (transpiled to JavaScript, garbage collected)
- No manual memory management
- Goal is logic correctness, not memory safety

**Key Insight:**
Rust's borrow checker solves **memory safety**.
Mint's mutability checker solves **logic correctness**.

Different problems require different solutions.

## Design Philosophy

### Simplicity Over Complexity

**Instead of Rust's approach:**
```rust
fn process(data: &Vec<i32>) -> usize { ... }      // Immutable borrow
fn modify(data: &mut Vec<i32>) { ... }            // Mutable borrow
let x = &data;                                     // Borrow
let y = &mut data;                                 // Mutable borrow
```

**Mint's simpler approach:**
```mint
λprocess(data:[ℤ])→ℤ=...           # Immutable by default
λmodify(data:mut [ℤ])→𝕌=...        # Explicit mut
```

**Just ONE new keyword:** `mut`

### Canonical Forms

Mint enforces canonical forms—one way to do each thing.

**No tail-call optimization:**
```mint
# This style is BLOCKED:
λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{
  0→acc|
  n→factorial(n-1,n*acc)
}

# Only primitive recursion allowed:
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}
```

Mutability fits this philosophy: either mutable or immutable, no third option.

## Error Messages

Mint provides clear, actionable error messages:

```
Mutability Error: Cannot create alias of mutable value 'x'

  12 | λbad(x:mut [ℤ])→𝕌≡{
  13 |   let y=x
       ^^^^^^^
```

```
Mutability Error: Cannot mutate immutable parameter 'list'

  5 | λprocess(list:[ℤ])→𝕌=list↦!λ(x)→x*2
                         ^^^^^^^^^^^^^^^^
```

## Future Enhancements

### Planned: Effect Tracking

Effect tracking will be added to track side effects:

```mint
λread()→!IO 𝕊=...                    # IO effect
λfetch(url:𝕊)→!Network Response=... # Network effect
```

This helps prevent accidental side effects and documents function behavior clearly.

### NOT Planned: Mutating Operations

Mint will **not** have mutating list operations like `↦!` or `⊳!`.

**Reason:** Violates canonical forms. Having both mutable and immutable versions creates ambiguity:
- `list↦fn` vs `list↦!fn` - which should LLMs choose?

Mint enforces **ONE way** to write each algorithm. All list operations are immutable.

## Best Practices

### When to Use Mutable Parameters

**Use `mut` when:**
- Calling JavaScript functions that mutate (FFI)
- Wrapping mutating JavaScript APIs
- Interfacing with imperative JavaScript libraries

**Don't use `mut` for:**
- Pure Mint code (use immutable operations)
- Performance optimization (not how Mint works)
- Internal algorithms (canonical forms require immutable)

### Example: FFI with Mutation

```mint
e Array
e console

⟦ JavaScript's Array.sort mutates in place ⟧
λsortAndLog(arr:mut [ℤ])→𝕌≡{
  Array.sort(arr);
  console.log(arr)
}

⟦ Pure Mint sorting returns new list ⟧
λsorted(list:[ℤ])→[ℤ]=list↦λ(x)→x
```

## Summary

Mint's mutability system:
- ✅ Prevents mutation bugs at compile time
- ✅ Prevents aliasing bugs
- ✅ Makes intent clear (`mut` = will be modified)
- ✅ Minimal syntax (just one keyword)
- ✅ Practical for TypeScript target
- ✅ Fits canonical form philosophy

It's the sweet spot between TypeScript (no checking) and Rust (complex borrow checking).
