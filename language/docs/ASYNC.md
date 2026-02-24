# Async-by-Default in Sigil

## Philosophy

**ALL Sigil functions are async.** No exceptions, no opt-out, no flags.

This is a fundamental design decision that aligns with Sigil's canonical forms philosophy: there is exactly **ONE way** to write functions, and that way is async.

## Why Async-by-Default?

### 1. Modern JavaScript is Async-First

The Node.js and browser ecosystems have moved decisively toward async APIs:
- `fs/promises` for file operations
- `fetch()` for HTTP requests
- Database clients return Promises
- Most I/O libraries are async-first

Making Sigil sync-by-default would mean constantly fighting the ecosystem.

### 2. All I/O is Async

Real-world applications need I/O:
- Reading files
- Making HTTP requests
- Querying databases
- Interacting with external services

Async is not optional for building useful programs.

### 3. ONE Canonical Way

Sigil enforces canonical forms: every algorithm has exactly one valid representation.

Having both sync and async versions of functions would create:
- ❌ Two ways to write every function
- ❌ Mental overhead switching between modes
- ❌ API surface duplication
- ❌ Training data pollution (LLMs see inconsistent patterns)

With async-by-default:
- ✅ ONE way to write functions
- ✅ No mental model switching
- ✅ Consistent code generation
- ✅ Clean training data for LLMs

### 4. Future-Proof

The JavaScript ecosystem will only become more async over time. By being async-first, Sigil is ready for:
- Top-level await (ES2022+)
- Async generators and iterators
- Streaming APIs
- Future async primitives

## How It Works

### All Functions Are Async

Every function in Sigil compiles to an `async function` in JavaScript/TypeScript:

```sigil
⟦ Pure function - still async ⟧
λadd(a:ℤ,b:ℤ)→ℤ=a+b

⟦ Compiles to: ⟧
async function add(a, b) {
  return (a + b);
}
```

### All Function Calls Use Await

Every function call is awaited:

```sigil
λmain()→ℤ=add(1,2)

⟦ Compiles to: ⟧
export async function main() {
  return await add(1, 2);
}
```

### Lambdas Are Async

All lambda expressions are async:

```sigil
λdouble(x:ℤ)→ℤ=x*2
[1,2,3]↦double

⟦ Compiles to: ⟧
await Promise.all((await [1,2,3]).map(async (x) => await double(x)))
```

### List Operations Use Promise.all

Map and filter operations run in parallel using `Promise.all`:

```sigil
[1,2,3,4,5]↦double⊳is_even

⟦ Compiles to: ⟧
await Promise.all((await [1,2,3,4,5])
  .map(async (x) => await double(x)))
  .filter(...)
```

### Top-Level Await

The generated code uses ES2022 top-level await, so Sigil programs can be run directly:

```javascript
// Generated runner code
import { main } from './program';
const result = await main();
if (result !== undefined) {
  console.log(result);
}
```

## Examples

### Pure Functions Work Fine

Even pure functions are async, with minimal overhead:

```sigil
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}

λmain()→ℤ=factorial(5)
⟦ Output: 120 ⟧
```

**Generated code:**
```javascript
async function factorial(n) {
  return (async () => {
    const __match = await n;
    if (__match === 0) {
      return 1;
    } else if (__match === 1) {
      return 1;
    } else {
      return (n * (await factorial((n - 1))));
    }
  })()
}
```

### FFI Calls Are Properly Awaited

No more Promise-wrapping mistakes:

```sigil
e fs⋅promises

λread_file(path:𝕊)→!IO 𝕊=fs⋅promises.readFile(path,"utf8")

λmain()→!IO 𝕊=read_file("data.txt")
```

**Generated code:**
```javascript
import * as fs_promises from 'fs/promises';

async function read_file(path) {
  return await __sigil_call("extern:fs/promises.readFile",
    fs_promises.readFile, [path, "utf8"]);
}

export async function main() {
  return await read_file("data.txt");
}
```

The FFI call returns a Promise, and Sigil automatically awaits it. It "just works."

### List Operations Are Parallel

Map operations run in parallel for better performance:

```sigil
λdouble(x:ℤ)→ℤ=x*2
λis_even(x:ℤ)→𝔹=x%2=0
λsum(acc:ℤ,x:ℤ)→ℤ=acc+x

λprocess(xs:[ℤ])→ℤ=xs↦double⊳is_even⊕sum⊕0

λmain()→ℤ=process([1,2,3,4,5])
⟦ Output: 30 (doubles [1,2,3,4,5] to [2,4,6,8,10], filters to [2,4,6,8,10], sums to 30) ⟧
```

All `double` calls run in parallel thanks to `Promise.all`.

### Effects Still Track I/O

Effect annotations still work the same way:

```sigil
e console

⟦ Pure function ⟧
λadd(a:ℤ,b:ℤ)→ℤ=a+b

⟦ Effectful function ⟧
λlog(msg:𝕊)→!IO 𝕌=console.log(msg)

⟦ ERROR: Can't call effectful from pure ⟧
λbad()→ℤ≡{l _=log("oops");42}
⟦ Effect mismatch: pure function calls !IO function ⟧
```

The `!IO` effect indicates side effects, but the async behavior is orthogonal to effects.

## Trade-offs

### Slight Performance Overhead

Pure functions pay a small cost for being async:

```javascript
// Sync (not Sigil)
function add(a, b) { return a + b; }

// Async (Sigil)
async function add(a, b) { return a + b; }
```

**Cost:** Microseconds per call (Promise allocation + microtask scheduling)

**Reality:**
- V8 heavily optimizes async/await
- Overhead is negligible unless you have tight loops with millions of iterations
- Most Sigil code is I/O-bound anyway (file reading, HTTP, databases)

**Design decision:** Correctness over micro-optimization. Getting FFI calls right is more important than saving microseconds.

### Can't Call Sigil from Sync Contexts

You can't call Sigil functions from synchronous JavaScript:

```javascript
// ❌ Doesn't work - returns a Promise
import { factorial } from './factorial';
const result = factorial(5); // Promise { <pending> }

// ✅ Works - await the result
import { factorial } from './factorial';
const result = await factorial(5); // 120
```

**Mitigation:** Sigil is the entry point for applications. If you need to integrate with sync code, use a tiny async wrapper:

```javascript
// bridge.js
import { sigilFunction } from './generated';

export function syncWrapper(...args) {
  let result;
  (async () => { result = await sigilFunction(...args); })();
  return result; // ⚠️ Will be Promise
}
```

Or better: make your entry point async (which Node.js, browsers, and modern frameworks all support).

### Requires ES2022+

Top-level await requires ES2022 or later.

**Minimum Node.js version:** 16+ (released 2021)

**Minimum Browser support:**
- Chrome 89+ (2021)
- Firefox 89+ (2021)
- Safari 15+ (2021)
- Edge 89+ (2021)

If you're targeting older runtimes, Sigil is not for you. But modern JavaScript is async-first, and Sigil embraces that.

## Comparison to Other Languages

### Rust

Rust requires explicit `async` annotations and has two separate function types:

```rust
fn sync_add(a: i32, b: i32) -> i32 { a + b }
async fn async_add(a: i32, b: i32) -> i32 { a + b }
```

You can't call async functions from sync contexts without blocking. This creates "color" problems: functions are either sync or async, and you have to maintain both.

**Sigil approach:** Everything is async. No color problem.

### JavaScript/TypeScript

JavaScript allows both sync and async functions:

```javascript
function syncAdd(a, b) { return a + b; }
async function asyncAdd(a, b) { return a + b; }
```

This creates ambiguity: should you use sync or async? Different projects make different choices, leading to inconsistent codebases.

**Sigil approach:** No choice = no ambiguity. Always async.

### Python

Python 3.5+ added `async/await`, but sync is still the default:

```python
def sync_add(a, b): return a + b
async def async_add(a, b): return a + b
```

Most Python code is sync, which makes async code feel like a second-class citizen.

**Sigil approach:** Async is first-class. It's the ONLY class.

### Go

Go uses goroutines and channels instead of async/await:

```go
func add(a, b int) int { return a + b }
```

All I/O is blocking, but goroutines make it efficient. However, this doesn't map cleanly to JavaScript's Promise-based concurrency.

**Sigil approach:** Embrace JavaScript's async model, don't fight it.

## Performance Tips

### Avoid Deep Recursion

Async functions still use the call stack:

```sigil
λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}

⟦ Stack depth: O(n) ⟧
⟦ Will overflow for large n (typically n > 10000) ⟧
```

**Mitigation:** Use list operations (map/filter/fold) instead of recursion where possible.

### Leverage Parallelism

List operations run in parallel:

```sigil
⟦ All fetch calls run in parallel ⟧
urls↦fetch_url
```

This is a **benefit** of async-by-default: automatic parallelization.

### Don't Worry About Pure Functions

The overhead for pure functions is negligible:

```sigil
λadd(a:ℤ,b:ℤ)→ℤ=a+b
```

Async overhead: ~0.01 milliseconds per call on modern hardware.

If you're calling `add` a million times, you might notice. But:
1. Most code doesn't do that
2. If you do, you're probably using Sigil wrong (use native JS for tight loops)

## Future Enhancements

### Parallel vs Sequential Control

Currently, map operations run in parallel, but there's no way to force sequential execution.

**Future syntax idea:**
```sigil
xs↦⧄fn  ⟦ Sequential map (↦⧄) ⟧
xs↦∥fn  ⟦ Parallel map (↦∥) - default ⟧
```

### Async Generators

Support for async iteration:

```sigil
λstream_lines(path:𝕊)→!IO [𝕊]≡{
  ⟦ Read file line-by-line without loading entire file ⟧
}
```

### Streaming APIs

Native support for streams and observables:

```sigil
λprocess_stream(stream:Stream[ℤ])→!IO Stream[ℤ]≡{
  stream↦double⊳is_even
}
```

## Summary

**Async-by-default is a feature, not a bug.**

It aligns with:
- Modern JavaScript ecosystem (Node.js, browsers, frameworks)
- Sigil's canonical forms philosophy (ONE way to write code)
- Real-world needs (all useful programs need I/O)

Trade-offs:
- Slight performance overhead on pure functions (negligible in practice)
- Requires modern runtimes (ES2022+, Node 16+)
- Can't call from sync contexts (acceptable - Sigil is the entry point)

**Bottom line:** If you're building real applications on modern JavaScript runtimes, async-by-default makes your life easier.
