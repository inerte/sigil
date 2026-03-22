# Sigil Mutability Design Note

This document describes an older or future mutability design direction, not the
current implemented Sigil surface in this repository.

The examples below are explanatory design sketches unless explicitly updated
elsewhere to match the current compiler.

## Overview

Sigil uses **immutable by default** with explicit `mut` annotations for mutability.

**Purpose:** The `mut` keyword is primarily for **FFI type safety** - marking JavaScript functions that mutate their arguments. This prevents accidental aliasing bugs when calling JavaScript code.

**Note:** Sigil itself has NO mutating operations. All list operations (`map`, `filter`, `reduce ... from ...`) are immutable. This preserves canonical forms - there's exactly ONE way to write each algorithm.

## Rules

### Rule 1: Immutable by Default

All values are immutable unless marked `mut`:

```text
λsum(list:[Int])=>Int=list reduce (λ(a:Int,x:Int)=>Int=a+x) from 0
⟦ list cannot be modified ⟧
```

### Rule 2: Explicit Mutability

Use `mut` keyword for mutable parameters:

```text
λsort(list:mut [Int])=>Unit=quicksort_impl(list)
⟦ list will be modified in place ⟧
```

### Rule 3: No Aliasing of Mutables

Cannot create multiple references to mutable values:

```text
⟦ ERROR: Cannot alias mutable ⟧
λbad(x:mut [Int])=>Unit match {
  let y=x    ⟦ ERROR: Can't create alias ⟧
}

⟦ OK: Direct use ⟧
λgood(x:mut [Int])=>Unit=modify(x)
```

### Rule 4: FFI Mutation Tracking

The `mut` keyword is used when calling JavaScript functions that mutate:

```text
e Array
λsortJS(arr:mut [Int])=>Unit=Array.sort(arr)  ⟦ JS Array.sort mutates ⟧

⟦ Pure Sigil code uses immutable operations ⟧
λsorted(list:[Int])=>[Int]=list map λ(x)=>x  ⟦ Returns new sorted list ⟧
```

## Examples

### Valid Code

```text
⟦ Immutable list operations (canonical form) ⟧
λdouble(list:[Int])=>[Int]=list map λ(x:Int)=>Int=x*2

⟦ FFI with mutation ⟧
e Array
λsortArray(arr:mut [Int])=>Unit=Array.sort(arr)

⟦ Multiple immutable uses (OK) ⟧
λprocess(data:[Int])=>Int match {
  let sum=data reduce λ(a,x)=>a+x from 0
  let len=data reduce λ(a,_)=>a+1 from 0
  sum/len
}
```

### Errors Prevented

```text
⟦ Error: Aliasing mutable ⟧
λbad1(x:mut [Int])=>Unit match {
  let y=x    ⟦ Error: Cannot create alias of mutable value 'x' ⟧
}

⟦ Error: Passing immutable to mutable parameter (FFI) ⟧
e Array
λbad2()=>Unit match {
  let data=[1,2,3]
  Array.sort(data)    ⟦ Error: Cannot pass immutable 'data' to mut parameter ⟧
}
```

## Why Mutability Checking?

### Problems It Prevents

**1. Accidental Mutation (FFI):**
```text
e Array

⟦ Without mutability checking: ⟧
λprocess(data:[Int])=>[Int] match {
  Array.sort(data);    ⟦ Oops! Modified input ⟧
  data
}

⟦ With mutability checking: ⟧
⟦ Compile error: Cannot pass immutable 'data' to mut parameter ⟧
```

**2. Aliasing Bugs:**
```text
⟦ Without mutability checking: ⟧
λbug(x:mut [Int])=>Unit match {
  let y=x
  modify!(x)    ⟦ Modifies through x ⟧
  process(y)    ⟦ y changed too! ⟧
}

⟦ With mutability checking: ⟧
⟦ Compile error: Cannot create alias of mutable value 'x' ⟧
```

**3. Unclear Intent:**
```text
⟦ Pure Sigil code - always immutable ⟧
λsorted(data:[Int])=>[Int]=...        ⟦ Returns new list (canonical) ⟧

⟦ FFI - mut signals mutation ⟧
e Array
λsortArray(arr:mut [Int])=>Unit=Array.sort(arr)  ⟦ Mutates via FFI ⟧
```

## Comparison to Other Languages

| Language | Approach | Complexity | Memory Safety |
|----------|----------|------------|---------------|
| **Rust** | Borrow checker with `&`, `&mut`, lifetimes | High | Yes (prevents use-after-free) |
| **TypeScript** | No mutability tracking | None | No |
| **Sigil** | `mut` keyword with aliasing prevention | Low | No (relies on JS GC) |

### Why Not Full Borrow Checking?

**Rust needs borrow checking because:**
- Manual memory management
- Prevents use-after-free, double-free, data races
- Systems programming requirements

**Sigil doesn't need it because:**
- Compiles to TypeScript (transpiled to JavaScript, garbage collected)
- No manual memory management
- Goal is logic correctness, not memory safety

**Key Insight:**
Rust's borrow checker solves **memory safety**.
Sigil's mutability checker solves **logic correctness**.

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

**Sigil's simpler approach:**
```text
λprocess(data:[Int])=>Int=...           ⟦ Immutable by default ⟧
λmodify(data:mut [Int])=>Unit=...        ⟦ Explicit mut ⟧
```

**Just ONE new keyword:** `mut`

### Canonical Forms

Sigil enforces canonical forms—one way to do each thing.

**No tail-call optimization:**
```text
⟦ This style is BLOCKED: ⟧
λfactorial(n:Int,acc:Int)=>Int match n{
  0=>acc|
  n=>factorial(n-1,n*acc)
}

⟦ Only primitive recursion allowed: ⟧
λfactorial(n:Int)=>Int match n{
  0=>1|
  1=>1|
  n=>n*factorial(n-1)
}
```

Mutability fits this philosophy: either mutable or immutable, no third option.

## Error Messages

Sigil provides clear, actionable error messages:

```
Mutability Error: Cannot create alias of mutable value 'x'

  12 | λbad(x:mut [Int])=>Unit match {
  13 |   let y=x
       ^^^^^^^
```

```
Mutability Error: Cannot mutate immutable parameter 'list'

  5 | λprocess(list:[Int])=>Unit=list map! λ(x)=>x*2
                         ^^^^^^^^^^^^^^^^
```

## Effect Tracking

Sigil tracks effects explicitly and checks them at compile time.

Current primitive effects are:

- `Clock`
- `Fs`
- `Http`
- `Log`
- `Process`
- `Tcp`
- `Timer`

Examples:

```text
λread()=>!Fs String=...
λfetch(url:String)=>!Http Response=...
λsleep()=>!Timer Unit=...
```

Projects may also define reusable multi-effect aliases in `src/effects.lib.sigil`.

### NOT Planned: Mutating Operations

Sigil will **not** have mutating list operations like `map!` or `filter!`.

**Reason:** Violates canonical forms. Having both mutable and immutable versions creates ambiguity:
- `list map fn` vs `list map! fn` - which should LLMs choose?

Sigil enforces **ONE way** to write each algorithm. All list operations are immutable.

## Best Practices

### When to Use Mutable Parameters

**Use `mut` when:**
- Calling JavaScript functions that mutate (FFI)
- Wrapping mutating JavaScript APIs
- Interfacing with imperative JavaScript libraries

**Don't use `mut` for:**
- Pure Sigil code (use immutable operations)
- Performance optimization (not how Sigil works)
- Internal algorithms (canonical forms require immutable)

### Example: FFI with Mutation

```text
e Array
e console

⟦ JavaScript's Array.sort mutates in place ⟧
λsortAndLog(arr:mut [Int])=>Unit match {
  Array.sort(arr);
  console.log(arr)
}

⟦ Pure Sigil sorting returns new list ⟧
λsorted(list:[Int])=>[Int]=list map λ(x)=>x
```

## Summary

Sigil's mutability system:
- ✅ Prevents mutation bugs at compile time
- ✅ Prevents aliasing bugs
- ✅ Makes intent clear (`mut` = will be modified)
- ✅ Minimal syntax (just one keyword)
- ✅ Practical for TypeScript target
- ✅ Fits canonical form philosophy

It's the sweet spot between TypeScript (no checking) and Rust (complex borrow checking).
