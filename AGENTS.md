# Claude Code Instructions for Mint Programming Language

## Project Structure

```
ai-pl/
├── src/              # User Mint programs (committed to git)
├── examples/         # Example Mint programs (committed to git)
├── .local/           # ALL compiled output (gitignored)
│   ├── src/          # Compiled from src/
│   └── *.js          # Compiled from root
└── compiler/         # The Mint compiler (TypeScript)
```

## When Writing Mint Programs

### 1. Choose the Right Location

**For new programs the user asks you to create:**
- Put in `src/` directory: `src/program-name.mint`
- Compiler outputs to `.local/src/program-name.js`

**For quick tests or experiments:**
- Put in root directory: `program-name.mint`
- Compiler outputs to `.local/program-name.js`

**For examples/documentation:**
- Put in `examples/` directory: `examples/program-name.mint`
- Compiler outputs beside source: `examples/program-name.js`

### 2. All Runnable Programs MUST Have main()

```mint
λmain()→𝕊="Hello, World!"
```

Or for programs that just do side effects:
```mint
λmain()→𝕌=process_data()
```

**Why:** `mintc run` requires a `main()` function as the entry point.

### 3. Compilation Commands

**Smart defaults (PREFERRED):**
```bash
node compiler/dist/cli.js compile src/myprogram.mint
# Automatically outputs to: build/myprogram.js

node compiler/dist/cli.js compile myprogram.mint
# Automatically outputs to: .local/myprogram.js
```

**Run directly:**
```bash
node compiler/dist/cli.js run src/myprogram.mint
# Compiles to .local/ and executes main()
```

**Custom output (rarely needed):**
```bash
node compiler/dist/cli.js compile src/myprogram.mint -o custom/path.js
```

## Mint Language Quick Reference

### Function Definition
```mint
λfunctionName(param:Type)→ReturnType=expression
```

### Pattern Matching
```mint
≡value{
  pattern1→result1|
  pattern2→result2|
  _→defaultResult
}
```

### Tuple Patterns (for multiple conditions)
```mint
≡(condition1,condition2){
  (⊤,⊤)→"both true"|
  (⊤,⊥)→"first true"|
  (⊥,⊤)→"second true"|
  (⊥,⊥)→"both false"
}
```

### Types
- `ℤ` - Integer
- `𝕊` - String
- `𝔹` - Boolean
- `𝕌` - Unit (void)
- `[T]` - List of T
- `⊤` - true
- `⊥` - false

### Lists
```mint
[1,2,3]              # List literal
[x,.rest]            # Pattern: x is first, rest is tail
[value,.recursive()]  # Construction with spread
```

## Common Patterns

### FizzBuzz
```mint
λfizzbuzz(n:ℤ)→𝕊≡(n%3=0,n%5=0){
  (⊤,⊤)→"FizzBuzz"|
  (⊤,⊥)→"Fizz"|
  (⊥,⊤)→"Buzz"|
  (⊥,⊥)→n
}
λmain()→𝕊=fizzbuzz(15)
```

### List Processing
```mint
λmap[T,U](fn:λ(T)→U,list:[T])→[U]≡list{
  []→[]|
  [x,.xs]→[fn(x),.map(fn,xs)]
}
```

### Recursion with Base Case
```mint
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}
```

## CRITICAL: ONE Way to Do Things - NO ALTERNATIVES

Mint is designed for **ZERO ambiguity**. There is EXACTLY ONE way to implement any algorithm.

### The Rule

**If the user asks for "X and Y" implementations, provide ONLY ONE.**

Examples:
- "Factorial (recursive and iterative)" → ONLY recursive
- "Loop and map versions" → ONLY map
- "If/else and match" → ONLY match
- "Imperative and functional" → ONLY functional

### Why?

**Human preference does NOT matter.** Mint is for LLMs, not humans. Multiple implementations create:
- ❌ Ambiguity for LLMs
- ❌ Wasted tokens
- ❌ Conflicting patterns in training data

### The Canonical Way

When you write Mint code:

1. ✅ **Use tuple patterns** for multiple conditions - NEVER nested matches
2. ✅ **Use pattern matching** - NEVER if/else chains
3. ✅ **Use simple recursion** - NEVER tail recursion helpers or accumulators unless absolutely necessary
4. ✅ **Put programs in src/** - NEVER scattered in root
5. ✅ **Have main()** in runnable programs - ALWAYS

### Examples

**❌ WRONG - Multiple implementations:**
```mint
λfactorial_recursive(n:ℤ)→ℤ=...
λfactorial_iterative(n:ℤ)→ℤ=...
```

**✅ CORRECT - One canonical way:**
```mint
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
```

**If the user wants "both recursive and iterative", tell them:**
> "In Mint, there is only one canonical way to implement factorial. Here's the recursive version (which is the only version)."

## Testing Your Code

After writing a Mint program:

```bash
# Compile and run
node compiler/dist/cli.js run src/myprogram.mint

# Or compile and inspect
node compiler/dist/cli.js compile src/myprogram.mint
cat build/myprogram.js
```

## Don't

- ❌ Don't create .js files manually - let the compiler generate them
- ❌ Don't put compiled .js files in git - they're in .gitignore
- ❌ Don't create files in root without reason - use src/
- ❌ Don't write programs without main() if they need to run
- ❌ Don't use multiple ways to solve the same problem

## Do

- ✅ Write dense, canonical Mint syntax
- ✅ Use tuple patterns for clarity
- ✅ Let the compiler choose output locations
- ✅ Always include main() in runnable programs
- ✅ Keep programs in src/ directory
