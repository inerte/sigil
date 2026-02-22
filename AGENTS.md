# Claude Code Instructions for Mint Programming Language

## Language Philosophy: Canonical Forms Only

Mint is a **canonicalization-enforced** language. Every algorithm has exactly ONE valid representation.

**Blocked Techniques (Compile-Time Errors):**
- Tail-call optimization (TCO)
- Accumulator-passing style
- Continuation-passing style (CPS)
- Trampolines
- Y combinator / Fixed-point combinators
- Mutual recursion / Co-recursion
- Helper functions / Auxiliary functions
- Closure-based state encoding
- Boolean pattern matching (when value matching works)
- Multi-field records as recursive parameters
- Collection types (lists, tuples, maps) as recursive parameters

**Enforced Techniques (Only Valid Forms):**
- Primitive recursion (direct recursive calls)
- Direct style (no continuations)
- Value-based pattern matching
- Single primitive parameter for recursive functions
- Self-contained function definitions
- Syntactic uniqueness (one syntax per semantic meaning)

This ensures **zero ambiguity** for LLM code generation and training data quality.

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

### Built-in List Operations (Language Constructs)
```mint
list↦fn              # Map: ↦ (apply fn to each element)
list⊳predicate       # Filter: ⊳ (keep elements matching predicate)
list⊕fn⊕init         # Fold: ⊕ (reduce with fn starting from init)

# Example: sum of doubled even numbers
[1,2,3,4,5]↦λx→x*2⊳λx→x%2=0⊕λ(acc,x)→acc+x⊕0  # Result: 30
```

**Note:** Map, filter, and fold are **language constructs**, not library functions. They compile directly to JavaScript's `.map()`, `.filter()`, and `.reduce()`.

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

### List Processing (Using Built-in Operations)
```mint
λdouble(x:ℤ)→ℤ=x*2
λisEven(x:ℤ)→𝔹=x%2=0
λsum(acc:ℤ,x:ℤ)→ℤ=acc+x

# Chain operations: map → filter → fold
λmain()→ℤ=[1,2,3,4,5]↦double⊳isEven⊕sum⊕0  # Result: 30
```

### Manual Recursion (When needed)
```mint
# Custom recursive list processing
λmap[T,U](fn:λ(T)→U,list:[T])→[U]≡list{
  []→[]|
  [x,.xs]→[fn(x),.map(fn,xs)]
}
```

### Recursion with Base Case
```mint
# Single parameter primitive recursion
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

# Multi-parameter algorithms (ALLOWED when all params are structural or query)
λgcd(a:ℤ,b:ℤ)→ℤ≡b{
  0→a|
  b→gcd(b,a%b)
}

λpower(base:ℤ,exp:ℤ)→ℤ≡exp{
  0→1|
  exp→base*power(base,exp-1)
}
```

**Why these are allowed:**
- **GCD**: Both `a` and `b` transform algorithmically (swap and modulo) - **STRUCTURAL**
- **Power**: `base` is query (constant), `exp` decreases - **QUERY + STRUCTURAL**

**Contrast with forbidden patterns:**
- `λfactorial(n:ℤ,acc:ℤ)` - `acc` is **ACCUMULATOR** (only multiplies, builds up product)
- `λsum(n:ℤ,acc:ℤ)` - `acc` is **ACCUMULATOR** (only adds, builds up sum)

The key distinction: parameters must be **algorithmically structural** (decompose/transform) or **query** (constant), not **accumulating state** (tail-call optimization).

## CRITICAL: Canonical Form Enforcement - COMPILER ENFORCED

Mint enforces **canonical forms** for all code. Every algorithm has exactly ONE syntactically valid representation.

**Computer Science Terms:**
- **Canonical form**: Unique normal form for equivalent programs
- **Syntactic uniqueness**: One syntax per semantic meaning
- **Deterministic code synthesis**: Eliminates ambiguity in code generation
- **Normalization**: Reducing programs to standard form

**THIS IS ENFORCED BY STATIC ANALYSIS** at compile-time - not just a suggestion.

### The Rule

**If the user asks for "X and Y" implementations, provide ONLY ONE.**

Examples:
- "Factorial (recursive and iterative)" → ONLY recursive
- "Loop and map versions" → ONLY map
- "If/else and match" → ONLY match
- "Imperative and functional" → ONLY functional

### Compiler-Enforced Rules

The Mint compiler uses **static analysis** to reject non-canonical code:

#### Rule 1: Recursive functions cannot use accumulator parameters

**Accumulator parameters are FORBIDDEN** (parameters that only grow/accumulate during recursion).

**Why:** Accumulator-passing style is tail-call optimization, which Mint blocks to enforce canonical forms.

**Allowed:** Multi-parameter recursion where ALL parameters are:
- **STRUCTURAL**: Decrease/decompose during recursion (n-1, xs from [x,.xs], a%b)
- **QUERY**: Stay constant or swap algorithmically (target in binary search, pegs in Hanoi)

**Forbidden:** Parameters that only accumulate/build up state:
- Multiplication accumulator: `factorial(n-1, n*acc)` where acc only grows
- Addition accumulator: `sum(n-1, acc+n)` where acc only increases
- List accumulator: `reverse(xs, [x,.acc])` where acc builds up result

**CS Terms:**
- Blocks: Tail recursion, accumulator-passing style, iterative encodings
- Allows: Primitive recursion with multiple algorithmic inputs, structural recursion
- Enforces: One canonical form per algorithm

```mint
✅ COMPILES - single parameter:
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}

✅ COMPILES - multi-param algorithmic (both transform):
λgcd(a:ℤ,b:ℤ)→ℤ≡b{0→a|b→gcd(b,a%b)}

✅ COMPILES - query + structural:
λpower(base:ℤ,exp:ℤ)→ℤ≡exp{0→1|exp→base*power(base,exp-1)}

❌ COMPILE ERROR - accumulator parameter:
λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→factorial(n-1,n*acc)}

Error: Accumulator-passing style detected in function 'factorial'.
       Parameter roles:
         - n: structural (decreases)
         - acc: ACCUMULATOR (grows)
       The parameter(s) [acc] are accumulators (grow during recursion).
```

#### Rule 2: No auxiliary functions

**Why:** Auxiliary functions enable alternative implementations via function composition

**CS Terms:**
- Blocks: Helper functions, auxiliary functions, wrapper patterns
- Detects: Call graph analysis for single-caller detection
- Enforces: Self-contained function definitions

```mint
✅ COMPILES - single function:
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
λmain()→ℤ=factorial(5)

❌ COMPILE ERROR - helper pattern:
λhelper(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→helper(n-1,n*acc)}
λfactorial(n:ℤ)→ℤ=helper(n,1)

Error: Function 'helper' is only called by 'factorial'.
       Helper functions are not allowed.
```

#### Rule 3: Canonical pattern matching only

**Why:** Syntactic variations pollute training data

**CS Terms:**
- Blocks: Boolean pattern matching when value matching possible
- Blocks: Syntactic alternatives for identical semantics
- Enforces: Most direct pattern matching form
- Uses: AST analysis to detect pattern redundancy

### Why Canonical Forms?

**Human preference does NOT matter.** Mint optimizes for machine learning, not human ergonomics.

**Training Data Quality:**
- ❌ Syntactic ambiguity → inconsistent code generation
- ❌ Multiple representations → wasted model capacity
- ❌ Algorithmic alternatives → conflicting patterns in training
- ✅ Canonical forms → deterministic, unambiguous synthesis

**CS Foundation:**
Like λ-calculus normal forms or term rewriting canonical forms, Mint ensures each semantic concept has exactly one syntactic representation.

### What Mint Supports (and Blocks)

**Recursion:**
- ✅ DO: Primitive recursion (direct recursive calls)
- ❌ BLOCKED: Tail-call optimization
- ❌ BLOCKED: Accumulator-passing style
- ❌ BLOCKED: Continuation-passing style (CPS)
- ❌ BLOCKED: Trampolines
- ❌ BLOCKED: Y combinator / fixed-point combinators
- ❌ BLOCKED: Mutual recursion

**Functions:**
- ✅ DO: Direct style (one function per algorithm)
- ❌ BLOCKED: Helper functions / auxiliary functions
- ❌ BLOCKED: Function composition for control flow
- ❌ BLOCKED: Closure-based state encoding

**Pattern Matching:**
- ✅ DO: Direct value matching (`≡n{0→...|n→...}`)
- ✅ DO: Tuple patterns for complex conditions (`≡(x>0,y>0){...}`)
- ❌ BLOCKED: Boolean matching when value matching works
- ❌ BLOCKED: Syntactic alternatives (multiple ways to write same match)

**Data Structures:**
- ✅ DO: Primitive types (ℤ, 𝕊, 𝔹, 𝕌)
- ✅ DO: Single-field records (not encoding multiple values)
- ❌ BLOCKED: Multi-field records for recursive state
- ❌ BLOCKED: Lists/tuples as recursive parameters
- ❌ BLOCKED: Closure-based state

**Code Organization:**
- ✅ DO: Self-contained functions
- ✅ DO: Programs in `src/`
- ✅ DO: `main()` as entry point
- ❌ BLOCKED: Helper function patterns
- ❌ BLOCKED: Files scattered in root

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
> "Mint does NOT support tail-call optimization or accumulator-passing style. There is only primitive recursion (the canonical form)."

**If the user wants "helper functions", tell them:**
> "Mint does NOT support auxiliary functions. Each function must be self-contained."

**If the user wants "boolean matching", tell them:**
> "Mint requires direct value matching when possible. Boolean pattern matching is only allowed for complex conditions."

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
