# Claude Code Instructions for Sigil Programming Language

⟦ Repo split note: this file lives under `language/` in the monorepo. Canonical user Sigil projects live under `projects/` and should use `sigil.json`, `src/`, and `tests/`. ⟧

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

## Canonical Surface Forms: Byte-for-Byte Reproducibility

Sigil enforces **canonical formatting** at compile-time. Every program has exactly ONE valid textual representation.

**Enforced formatting rules:**

### 1. Final Newline (Required)
```sigil
✅ VALID:
λmain()→ℤ=1
[newline here]

❌ REJECTED:
λmain()→ℤ=1[EOF without newline]
⟦ Error: File must end with a newline ⟧
```

### 2. No Trailing Whitespace
```sigil
❌ REJECTED:
λmain()→ℤ=1   [spaces here]
⟦ Error: Line 1 has trailing whitespace ⟧
```

### 3. Maximum One Blank Line
```sigil
✅ VALID:
λa()→ℤ=1

λb()→ℤ=2

❌ REJECTED:
λa()→ℤ=1


λb()→ℤ=2
⟦ Error: Multiple blank lines at line 2 (only one consecutive blank line allowed) ⟧
```

### 4. Equals Sign Placement (Context-Dependent)
```sigil
✅ VALID - Regular expression (= required):
λdouble(x:ℤ)→ℤ=x*2

✅ VALID - Match expression (NO = allowed):
λfactorial(n:ℤ)→ℤ≡n{0→1|n→n*factorial(n-1)}

❌ REJECTED - Missing =:
λdouble(x:ℤ)→ℤ x*2
⟦ Error: Expected "=" before function body (canonical form: λf()→T=...) ⟧

❌ REJECTED - Unwanted = before match:
λfactorial(n:ℤ)→ℤ=≡n{...}
⟦ Error: Unexpected "=" before match expression (canonical form: λf()→T≡...) ⟧
```

**Why enforce surface forms?**

1. **Training data quality** - No syntactic variations polluting datasets
2. **Deterministic generation** - LLMs generate exactly one form
3. **Zero ambiguity** - Byte-for-byte reproducibility
4. **Canonical philosophy** - One way extends from algorithms to formatting

**Already enforced by lexer:**
- ✅ Tab characters forbidden (use spaces)
- ✅ Standalone `\r` forbidden (use `\n`)

## Type System: Bidirectional Type Checking

**Paradigm:** Bidirectional type checking (not Hindley-Milner)

**Why bidirectional?**
- Sigil requires **mandatory type annotations everywhere** (canonical forms)
- Hindley-Milner's strength is type inference with minimal annotations
- Bidirectional is simpler and better suited for mandatory annotations
- Better error messages: "expected X, got Y" with precise source locations
- More extensible: natural framework for polymorphism, refinement types, effects

**Type Annotations Required:**
```sigil
✅ CORRECT (only valid form):
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}

❌ SYNTAX ERROR (missing annotations):
λfactorial(n)=...        ⟦ Missing parameter type ⟧
λfactorial(n:ℤ)=...      ⟦ Missing return type ⟧
λfactorial(n)→ℤ=...      ⟦ Missing parameter type ⟧
```

**How it works:**
- **Synthesis mode (⇒)**: Infer type from expression structure
- **Checking mode (⇐)**: Verify expression matches expected type
- System alternates between modes based on available information

**Benefits:**
- Zero syntactic ambiguity (ONE way to write types)
- Clear error messages with precise locations
- Canonical forms enforced by parser and type checker
- Simpler implementation than Hindley-Milner for our use case

## Effect Tracking: Compile-Time Side Effect Safety

**Paradigm:** Explicit effect annotations (not inference)

Mint tracks side effects at compile time to prevent bugs and document behavior clearly.

**Syntax:** `→!Effect1 !Effect2 Type`

**Valid effects:**
- `!IO` - Console I/O, file system access, system calls
- `!Network` - HTTP requests, network communication
- `!Async` - Asynchronous operations, promises
- `!Error` - Error-prone operations
- `!Mut` - Mutation of data structures (future use)

**Examples:**
```sigil
⟦ Pure function (no effects) ⟧
λadd(a:ℤ,b:ℤ)→ℤ=a+b

⟦ Single effect ⟧
e console
λlog(msg:𝕊)→!IO 𝕌=console.log(msg)

⟦ Multiple effects ⟧
λprocessData()→!IO !Network 𝕊≡{
  log("Starting");
  fetchData()
}

⟦ Effect propagation - main must declare all effects ⟧
λmain()→!IO !Network 𝕌=processData()
```

**Rules:**
1. **Pure functions cannot call effectful functions** (compile error)
2. **Effectful functions must declare all effects** (compile error if missing)
3. **Effect subtyping:** Effectful can call pure (but not vice versa)

**Why effect tracking?**
- Prevents accidental side effects (catch bugs early)
- Documents behavior explicitly (function signature shows what it does)
- Helps LLM reasoning (AI sees effects in type signatures)
- Preserves canonical forms (one signature per function)

**Example errors:**
```sigil
e console
λlog(msg:𝕊)→!IO 𝕌=console.log(msg)

⟦ ERROR: Pure calling effectful ⟧
λbad()→𝕌=log("oops")
⟦ Effect mismatch in function "bad": ⟧
⟦   Declared effects: (pure) ⟧
⟦   Undeclared effects used: !IO ⟧

⟦ FIX: Declare the effect ⟧
λgood()→!IO 𝕌=log("works!")
```

See `examples/effect-demo.sigil` for complete examples.

## External Module Interop (FFI)

**Syntax:** `e module/path` (ONLY way)

Sigil can call external modules (including TypeScript/JavaScript packages) and npm packages.

**Examples:**
```sigil
e console
λmain()→𝕌=console.log("Hello from Sigil!")

e fs/promises
λwriteFile(path:𝕊,content:𝕊)→𝕌=fs/promises.writeFile(path,content)

e axios
λfetch(url:𝕊)→𝕌=axios.get(url)
```

**Usage:**
- Declare: `e module/path`
- Use: `module/path.member(args)`
- Full path is namespace (no conflicts)
- Validated at link-time (catches typos before running)

**Key Points:**
- NO type annotations needed (validated structurally)
- NO member lists (`e module{a,b}` ❌)
- NO aliasing (`e module as m` ❌)
- ONE canonical way

See `docs/FFI.md` for full documentation.

**React/Browser apps (recommended pattern):**
- Put deterministic domain policy/logic in Sigil (`.sigil`)
- Compile Mint to generated TypeScript (`.ts`)
- Use a separate `bridge.ts` / `bridge.tsx` for React hooks, JSX, browser events, and localStorage
- Keep the bridge lintable/prettifiable; keep Mint canonical

## Comments: Multi-line Only

**Syntax:** `⟦ ... ⟧` (Mathematical white square brackets)

**Rules:**
- Comments can span multiple lines
- Comments can be inserted anywhere (mid-expression, between tokens)
- Comments are stripped during lexing (don't affect AST)
- Only ONE comment syntax (canonical form)

**Examples:**
```sigil
⟦ This function computes factorial recursively ⟧
λfactorial(n:ℤ)→ℤ≡n{
  0→1|  ⟦ base case ⟧
  1→1|
  n→n*⟦ recursive call ⟧factorial(n-1)
}

⟦ Multi-line comment explaining
   a complex algorithm step-by-step ⟧
λprocess(data:[ℤ])→ℤ=data⊕(λ(a:ℤ,x:ℤ)→ℤ=a+x)⊕0
```

**Why multi-line only?**
- Avoids having multiple comment syntaxes (`//` vs `⟦⟧`)
- Fits canonical form philosophy (ONE way)
- Can be used inline or multi-line (flexible)
- Visually distinctive (Unicode brackets)

## Mutability System: Immutable by Default

**Paradigm:** Explicit mutability with compile-time checking

**Why mutability tracking?**
- Prevents logic errors (mutation of unintended values)
- Prevents aliasing bugs (multiple mutable references)
- Keeps syntax simple (just `mut` keyword)
- Fits the TypeScript compilation target (no memory safety needed)

**Mutability Rules:**
```sigil
✅ CORRECT:
λprocess(data:[ℤ])→ℤ=...              ⟦ Immutable (default) ⟧
λsort(data:mut [ℤ])→𝕌=...             ⟦ Explicit mutation ⟧

❌ ERRORS:
e Array
λbad1(data:[ℤ])→𝕌=Array.sort(data)  ⟦ Can't pass immutable to mut param ⟧
λbad2(x:mut [ℤ])→𝕌≡{let y=x; ...}    ⟦ Can't alias mutable ⟧
```

**Benefits:**
- Catch mutation bugs at compile time
- Clear intent (mut = will be modified)
- Minimal syntax (one keyword vs Rust's &, &mut, lifetimes)
- Works with garbage collection
- Practical for TypeScript target

## Semantic Maps: Machine Code, Human Explanations

**The killer feature of Sigil**: Dense, machine-optimized code with AI-generated explanations.

### How Semantic Maps Work

Every `.sigil` file gets a `.sigil.map` file (auto-generated by compiler):

```
fibonacci.sigil     ← Dense code: λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→...}
fibonacci.sigil.map ← AI docs: "Computes nth Fibonacci recursively. O(2^n)..."
```

### Your Role: Enhance Semantic Maps

When `sigilc compile` runs, it creates **basic** semantic maps with structural info (ranges, types, summaries).

**You enhance them with rich AI-generated content.**

### When Invoked

The compiler calls you automatically via:
```bash
claude -p "Enhance semantic map..." --allowedTools Write Read
```

### What You Do

1. **Read the basic semantic map** (e.g., `src/factorial.sigil.map`)
2. **For each mapping**, enhance with:
   - **explanation**: Detailed markdown explanation (what it does, how it works)
   - **complexity**: Time/space complexity (e.g., "O(n) time, O(1) space")
   - **warnings**: Edge cases, performance issues, limitations
   - **examples**: Usage examples (input → output)
   - **related**: Related function/type names
3. **Write enhanced map back** to same file

### Example Enhancement

**Before (basic):**
```json
{
  "factorial": {
    "range": [0, 47],
    "summary": "Function: factorial",
    "explanation": "Function with 1 parameter(s), returns ℤ",
    "type": "λ(ℤ)→ℤ"
  }
}
```

**After (enhanced):**
```json
{
  "factorial": {
    "range": [0, 47],
    "summary": "Function: factorial",
    "explanation": "Computes the factorial of n recursively using pattern matching. Base cases: 0! = 1 and 1! = 1. Recursive case: n! = n × (n-1)!. Uses primitive recursion (Mint's canonical form).",
    "type": "λ(ℤ)→ℤ",
    "complexity": "O(n) time, O(n) space (call stack due to primitive recursion)",
    "warnings": [
      "Stack overflow for large n (typically n > 10000)",
      "O(n) stack depth is inherent to Mint's canonical primitive recursion",
      "Not suitable for extremely large inputs"
    ],
    "examples": [
      "factorial(5) → 120",
      "factorial(0) → 1",
      "factorial(10) → 3628800"
    ],
    "related": ["main"]
  }
}
```

### Quality Bar

Match the examples in `examples/*.sigil.map`:
- fibonacci.sigil.map
- list-operations.sigil.map
- http-handler.sigil.map

**Key insights to include:**
- Algorithm explanation (not just "does factorial")
- Performance characteristics
- Real-world considerations
- Concrete examples

### CRITICAL: Mint-Appropriate Warnings

**DON'T suggest impossible alternatives:**
- ❌ "Consider iterative version" (Mint blocks iteration)
- ❌ "Use tail-call optimization" (Mint blocks TCO)
- ❌ "Add accumulator parameter" (Mint blocks accumulator-passing style)

**DO provide Mint-appropriate guidance:**
- ✅ "O(n) stack depth is inherent to Mint's canonical primitive recursion"
- ✅ "Not suitable for extremely large inputs due to stack depth"
- ✅ "Performance characteristic is fundamental to primitive recursion"

**Remember:** Sigil enforces canonical forms. ONE way to write each algorithm. Your warnings should acknowledge this, not fight it.

## Project Structure

```
ai-pl/
├── src/              # User Sigil programs (committed to git)
├── examples/         # Example Sigil programs (committed to git)
├── .local/           # ALL compiled output (gitignored)
│   ├── src/          # Compiled from src/
│   └── *.ts          # Compiled from root
└── compiler/         # The Sigil compiler (TypeScript)
```

## When Writing Mint Programs

### 1. Choose the Right Location

**For new programs the user asks you to create:**
- Put in `src/` directory: `src/program-name.sigil`
- Compiler outputs to `.local/src/program-name.ts`

**For quick tests or experiments:**
- Put in root directory: `program-name.sigil`
- Compiler outputs to `.local/program-name.ts`

**For examples/documentation:**
- Put in `examples/` directory: `examples/program-name.sigil`
- Compiler outputs beside source: `examples/program-name.ts`

### 2. All Runnable Programs MUST Have main()

```sigil
λmain()→𝕊="Hello, World!"
```

Or for programs that just do side effects:
```sigil
λmain()→𝕌=process_data()
```

**Why:** `sigilc run` requires a `main()` function as the entry point.

### 3. Compilation Commands

**Smart defaults (PREFERRED):**
```bash
node language/compiler/dist/cli.js compile src/myprogram.sigil
# Automatically outputs to: build/myprogram.ts

node language/compiler/dist/cli.js compile myprogram.sigil
# Automatically outputs to: .local/myprogram.ts
```

**Run directly:**
```bash
node language/compiler/dist/cli.js run src/myprogram.sigil
# Compiles to .local/ and executes main()
```

**Custom output (rarely needed):**
```bash
node language/compiler/dist/cli.js compile src/myprogram.sigil -o custom/path.ts
```

## Docs Sync (Required When Syntax Changes)

When changing Sigil syntax (declarations, operators, imports/exports, comments, tests, effects), update docs/examples in the same change.

Minimum files to review:
- `language/docs/syntax-reference.md` (canonical syntax surface)
- `language/README.md` (top-level examples)
- `language/AGENTS.md` (quick reference snippets)
- relevant focused docs (`language/docs/type-system.md`, `language/docs/TESTING.md`, `language/docs/FFI.md`, etc.)

Rule:
- All ` ```sigil ` code fences must contain valid Sigil syntax, including Sigil comments `⟦ ... ⟧` (never `#` or `//` in Sigil examples).

## Sigil Language Quick Reference

### Standard Library

Sigil includes a standard library with common utility functions and predicates.

**Import modules (like FFI):**
```sigil
i stdlib/list_predicates
i stdlib/numeric_predicates
i stdlib/list_utils
```

**List predicates:**
```sigil
stdlib/list_predicates.sorted_asc([1,2,3])           ⟦ Check if sorted ascending ⟧
stdlib/list_predicates.all(is_positive,[1,2,3])      ⟦ Check if all elements satisfy predicate ⟧
stdlib/list_predicates.any(is_even,[1,3,5])          ⟦ Check if any element satisfies predicate ⟧
stdlib/list_predicates.contains(3,[1,2,3,4])         ⟦ Check if element in list ⟧
```

**Numeric predicates:**
```sigil
stdlib/numeric_predicates.is_positive(5)             ⟦ Check if > 0 ⟧
stdlib/numeric_predicates.is_even(4)                 ⟦ Check if divisible by 2 ⟧
stdlib/numeric_predicates.is_prime(7)                ⟦ Check if prime number ⟧
stdlib/numeric_predicates.in_range(5,1,10)           ⟦ Check if in range [min,max] ⟧
```

**List utilities:**
```sigil
stdlib/list_utils.len([1,2,3])                       ⟦ Get list length ⟧
stdlib/list_utils.head([1,2,3])                      ⟦ Get first element ⟧
stdlib/list_utils.tail([1,2,3])                      ⟦ Get all but first ⟧
```

**Common patterns:**
```sigil
i stdlib/numeric_predicates

⟦ Validation ⟧
λprocess(x:ℤ)→𝕊≡stdlib/numeric_predicates.is_positive(x){
  ⊥→"Error: Must be positive"|
  ⊤→"Processing..."
}

⟦ Filtering ⟧
λget_primes(xs:[ℤ])→[ℤ]=xs⊳stdlib/numeric_predicates.is_prime

⟦ Preconditions ⟧
λbinary_search(xs:[ℤ],target:ℤ)→ℤ≡stdlib/list_predicates.sorted_asc(xs){
  ⊥→-1|
  ⊤→search_impl(...)
}
```

See `docs/STDLIB.md` for complete reference.

### External Module Interop (FFI)
```sigil
e module/path              ⟦ Import external module ⟧
module/path.member(args)   ⟦ Call external module function ⟧

⟦ Examples: ⟧
e console
console.log("Hello!")

e fs/promises
fs/promises.writeFile("file.txt", "content")

e axios
axios.get("https://api.example.com")
```

### Function Definition
```sigil
⟦ Pure function ⟧
λfunctionName(param:Type)→ReturnType=expression

⟦ Function with effects ⟧
λfunctionName(param:Type)→!Effect1 !Effect2 ReturnType=expression
```

### Pattern Matching
```sigil
≡value{
  pattern1→result1|
  pattern2→result2|
  _→defaultResult
}
```

### Tuple Patterns (for multiple conditions)
```sigil
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

### Sum Types (Algebraic Data Types)
```sigil
⟦ Type declarations ⟧
t Color=Red|Green|Blue              ⟦ Simple enum ⟧
t Option[T]=Some(T)|None            ⟦ Generic optional value ⟧
t Result[T,E]=Ok(T)|Err(E)          ⟦ Generic success/failure ⟧

⟦ Constructor calls (always use parentheses) ⟧
Red()                               ⟦ Nullary constructor ⟧
Some(42)                            ⟦ Constructor with value ⟧
Ok(100)                             ⟦ Success value ⟧
Err("not found")                    ⟦ Error value ⟧

⟦ Pattern matching ⟧
λprocessColor(c:Color)→ℤ≡c{
  Red→1|
  Green→2|
  Blue→3
}

λprocessOption(opt:Option)→ℤ≡opt{
  Some(x)→x|                        ⟦ Extract value from Some ⟧
  None→0                            ⟦ Default for None ⟧
}

λprocessResult(res:Result)→𝕊≡res{
  Ok(value)→"Success: "++value|
  Err(msg)→"Error: "++msg
}
```

**Standard library sum types:**
- `Option[T]` - in `stdlib/option.sigil`
- `Result[T,E]` - in `stdlib/result.sigil`

See `examples/sum-types-demo.sigil` for comprehensive examples.

### Lists
```sigil
[1,2,3]              ⟦ List literal ⟧
[x,.rest]            ⟦ Pattern: x is first, rest is tail ⟧
[value,.recursive()]  ⟦ Construction with spread ⟧
```

**Empty list typing (`[]`)**
- `[]` requires a known expected list type (contextual typing)
- Works in checked positions (e.g., function returns, match arms) when the return type is already `[T]`
- Rejected when no element type can be determined

### Concatenation
```sigil
"Hello, "++"Sigil"     ⟦ String concatenation (only for strings) ⟧
[1,2]⧺[3,4]            ⟦ List concatenation (only for lists) ⟧
```

### Built-in List Operations (Language Constructs)
```sigil
list↦fn              ⟦ Map: ↦ (apply fn to each element) ⟧
list⊳predicate       ⟦ Filter: ⊳ (keep elements matching predicate) ⟧
list⊕fn⊕init         ⟦ Fold: ⊕ (reduce with fn starting from init) ⟧

⟦ Example: sum of doubled even numbers ⟧
[1,2,3,4,5]↦λx→x*2⊳λx→x%2=0⊕λ(acc,x)→acc+x⊕0  ⟦ Result: 30 ⟧
```

**Note:** Map, filter, and fold are **language constructs**, not library functions. They compile directly to TypeScript/JavaScript array methods (`.map()`, `.filter()`, `.reduce()`).

## Common Patterns

### FizzBuzz
```sigil
λfizzbuzz(n:ℤ)→𝕊≡(n%3=0,n%5=0){
  (⊤,⊤)→"FizzBuzz"|
  (⊤,⊥)→"Fizz"|
  (⊥,⊤)→"Buzz"|
  (⊥,⊥)→n
}
λmain()→𝕊=fizzbuzz(15)
```

### List Processing (Using Built-in Operations)
```sigil
λdouble(x:ℤ)→ℤ=x*2
λisEven(x:ℤ)→𝔹=x%2=0
λsum(acc:ℤ,x:ℤ)→ℤ=acc+x

⟦ Chain operations: map → filter → fold ⟧
λmain()→ℤ=[1,2,3,4,5]↦double⊳isEven⊕sum⊕0  ⟦ Result: 30 ⟧
```

### Manual Recursion (When needed)
```sigil
⟦ Custom recursive list processing ⟧
λmap[T,U](fn:λ(T)→U,list:[T])→[U]≡list{
  []→[]|
  [x,.xs]→[fn(x),.map(fn,xs)]
}
```

### Recursion with Base Case
```sigil
⟦ Single parameter primitive recursion ⟧
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

⟦ Multi-parameter algorithms (ALLOWED when all params are structural or query) ⟧
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

Sigil enforces **canonical forms** for all code. Every algorithm has exactly ONE syntactically valid representation.

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

The Sigil compiler uses **static analysis** to reject non-canonical code:

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

```sigil
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

#### Rule 2: Canonical pattern matching only

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
- ✅ DO: Utility functions (is_valid, sorted, len)
- ✅ DO: Predicate functions for contracts
- ✅ DO: Code decomposition via helper functions
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
- ✅ DO: Functions in logical groups
- ✅ DO: Programs in `src/`
- ✅ DO: `main()` as entry point
- ❌ BLOCKED: Files scattered in root

### Examples

**❌ WRONG - Multiple implementations:**
```sigil
λfactorial_recursive(n:ℤ)→ℤ=...
λfactorial_iterative(n:ℤ)→ℤ=...
```

**✅ CORRECT - One canonical way:**
```sigil
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
```

**If the user wants "both recursive and iterative", tell them:**
> "Sigil does NOT support tail-call optimization or accumulator-passing style. There is only primitive recursion (the canonical form)."

**If the user wants "boolean matching", tell them:**
> "Sigil requires direct value matching when possible. Boolean pattern matching is only allowed for complex conditions."

## Testing Your Code

After writing a Mint program:

```bash
# Compile and run
node language/compiler/dist/cli.js run src/myprogram.sigil

# Or compile and inspect
node language/compiler/dist/cli.js compile src/myprogram.sigil
cat build/myprogram.ts
```

First-class Sigil tests (agent-first, JSON default):

```bash
# Run all tests from ./tests (JSON to stdout by default)
node language/compiler/dist/cli.js test

# Human-readable output
node language/compiler/dist/cli.js test --human

# Filter by test description substring (great for agent TDD loops)
node language/compiler/dist/cli.js test --match "toggle"
```

Testing rules:
- Test declarations are only allowed under `./tests` (canonical project layout)
- Test files may include regular Sigil declarations plus `test` declarations
- Test bodies must evaluate to `𝔹`
- Effectful tests must declare effects explicitly (`test "..." →!IO { ... }`)
- Use `mockable` + `with_mock(...) { ... }` for explicit scoped mocks
- `sigilc test` runs test files in parallel by default (JSON output remains deterministically ordered)

Example:

```sigil
mockable λping()→!IO 𝕊="real"

test "ping can be mocked" →!IO {
  with_mock(ping, λ()→!IO 𝕊="fake") {
    ping()="fake"
  }
}
```

## Don't

- ❌ Don't create .ts output files manually - let the compiler generate them
- ❌ Don't put compiled output files in git unless the example/docs specifically commit generated `.ts`
- ❌ Don't create files in root without reason - use src/
- ❌ Don't write programs without main() if they need to run
- ❌ Don't use multiple ways to solve the same problem

## Do

- ✅ Write dense, canonical Sigil syntax
- ✅ Use tuple patterns for clarity
- ✅ Let the compiler choose output locations
- ✅ Always include main() in runnable programs
- ✅ Keep programs in src/ directory
