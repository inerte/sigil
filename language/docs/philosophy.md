# Sigil Language Philosophy

## The Machine-First Revolution

### Why Sigil Exists

In 2026, **93% of code is AI-generated**. Yet our programming languages are still optimized for human authoring from the 1960s-2000s era. This is backwards.

**Sigil** inverts this priority: it's a language designed for machines (LLMs) to write, with AI-powered tools for humans to understand.

### The Core Insight

Traditional language design:
```
Human writes code → Machine executes
↓
Optimize for human writing (verbose keywords, flexible syntax, readability)
```

Sigil's design:
```
AI writes code → Machine executes
       ↓             ↑
Human understands via AI explanations
↓
Optimize for machine generation (dense syntax, zero ambiguity, minimal tokens)
```

## Key Principles

### 1. Radical Canonicalization

**"There is exactly ONE way to write it"**

- No alternative syntaxes
- No optional keywords, brackets, delimiters
- No syntactic sugar
- One import style, one function definition, one loop construct

**Why?** LLMs hallucinate less when there's only one correct answer. Choice paralysis causes errors.

**Example - Other Languages:**
```javascript
// JavaScript - 5 ways to define a function
function add(a, b) { return a + b; }
const add = function(a, b) { return a + b; };
const add = (a, b) => { return a + b; };
const add = (a, b) => a + b;
const add = new Function('a', 'b', 'return a + b');
```

**Sigil - ONE way:**
```sigil
λadd(a:ℤ,b:ℤ)→ℤ=a+b
```

#### Parameter Classification for Canonical Forms

Multi-parameter recursion is allowed if parameters are **algorithmically structural**, not accumulating state.

**The Distinction:**
- **Traditional FP**: Tail-call optimization via accumulators (imperative encoding)
- **Sigil**: Primitive recursion with multiple algorithmic inputs (pure structural)

**Examples:**

Algorithmic (ALLOWED):
- `gcd(a, b)` - both parameters swap and transform
- `binary_search(list, target, low, high)` - query + structural bounds
- `nth(list, index)` - parallel decomposition
- `power(base, exp)` - query (base constant) + structural (exp decreases)

State accumulation (FORBIDDEN):
- `factorial(n, acc)` - acc accumulates product
- `sum(n, total)` - total accumulates sum
- `reverse(list, result)` - result accumulates reversed list

**Why this preserves canonical forms:**

The real problem isn't multiple parameters - it's **accumulator-passing style**, which encodes imperative iteration in functional recursion. That creates ambiguity (recursive vs iterative implementations).

Legitimate multi-parameter algorithms like GCD, binary search, and nth element have NO ambiguity - there's still only ONE way to write them in Sigil. They're not accumulator patterns; they're genuinely multi-input algorithms.

This makes Sigil **more principled** (precise distinction) while **more practical** (enables O(log n) algorithms) - a rare win-win.

#### Async-by-Default: The Ultimate Canonical Form

**"ALL functions are async. No exceptions."**

In 2026, modern JavaScript is async-first:
- Node.js fs/promises (not fs)
- fetch() API (browsers and Node.js)
- Database clients return Promises
- Most npm packages are async

Yet developers still choose between sync and async implementations, creating:
- Two ways to write every function
- Mental overhead switching between modes
- API surface duplication (readFileSync vs readFile)
- Integration friction (mixing sync/async code)

**Sigil's solution:** Make async the ONLY option.

```sigil
⟦ Pure function - async ⟧
λadd(a:ℤ,b:ℤ)→ℤ=a+b

⟦ I/O function - async ⟧
e fs⋅promises
λread(path:𝕊)→!IO 𝕊=fs⋅promises.readFile(path,"utf8")
```

Both compile to `async function`. ALL calls use `await`. No choice, no ambiguity.

**Benefits:**
- **Canonical forms preserved** - ONE way to write functions
- **FFI just works** - Promise-returning APIs automatically awaited
- **Future-proof** - Ecosystem moving toward async-first anyway
- **No mental overhead** - Never decide "should this be async?"

**Trade-offs:**
- Slight performance overhead on pure functions (~microseconds)
- Requires ES2022+ (top-level await)
- Can't call from sync contexts (acceptable - Sigil is the entry point)

**Design philosophy:** Correctness and simplicity over micro-optimization. Having two function types would violate canonical forms for marginal performance gains.

See [docs/ASYNC.md](./ASYNC.md) for complete rationale.

### 2. Token Efficiency

**"Every character carries maximum information density"**

More code fits in LLM context windows = better understanding = better code generation.

**Token Savings:**
- `λ` instead of `function` (1 char vs 8)
- `→` instead of `:` or `=>` (1 char vs 1-2, but semantically richer)
- `≡` instead of `match` or `switch` (1 char vs 5-6)
- Unicode type symbols: `ℤℝ𝔹𝕊` instead of `Int,Float,Bool,String`

**Result:** Current benchmarks show ~10-15% fewer tokens than TypeScript on average (see `language/benchmarks/RESULTS.md`)

**Why Unicode?** Modern LLMs tokenize Unicode efficiently, and it provides unambiguous semantic meaning. `ℤ` universally means "integers" in mathematics.

### 3. Zero Ambiguity

**"The type checker catches everything"**

- Bidirectional type checking with mandatory explicit annotations
- No `any`, no type coercion, no `null`, no `undefined`
- Algebraic data types (sum + product types)
- Effect system tracks IO, network, async operations
- Borrow checker for memory safety

**Why?** Static analysis prevents ~80% of runtime errors. LLMs can generate code confidently knowing the type checker will catch mistakes.

### 4. Enforced Canonical Formatting

**"Unformatted code doesn't compile"**

The formatter is part of the parser. Code that violates formatting rules produces a parse error, not a warning.

**Rules:**
- No spaces around operators: `x+y` not `x + y`
- Single space after commas: `f(x, y)` not `f(x,y)`
- No trailing whitespace
- No line length limits (machines don't care)

**Why?** LLMs learn ONE valid token sequence per semantic meaning. No uncertainty.

### 5. Functional-First Paradigm

**"It's all about the data"**

- Everything is an expression
- Immutable by default
- Pattern matching (only control flow mechanism)
- First-class functions
- No loops - only recursion and higher-order functions
- No null - Option type
- No exceptions - Result type

**Why?**
1. Pure functions are easier for LLMs to reason about (no hidden state)
2. Composition is natural (build complex from simple)
3. Better type inference
4. Simpler AI explanations ("transforms X to Y" vs "mutates Z, depends on W")

### 6. AI-First Development

**"Humans ask Claude Code to explain code, not read it directly"**

This is the **killer feature** - Claude Code as the primary interface.

**Traditional approach:**
```
Code is human-readable → Humans read/edit → Compiler checks
```

**Sigil approach:**
```
Code is machine-optimal → Claude Code explains → Humans understand via AI
```

**Development Flow:**
```
fibonacci.sigil       # Dense canonical code: λfibonacci(n:ℤ)→ℤ≡n{...}
  ↓ (Claude Code reads via compiler CLI)
Natural language explanation on demand
```

**Claude Code Interface:**
- Developer asks "What does this do?" → Claude Code explains
- Developer asks "Add memoization" → Claude Code edits canonical syntax
- Compiler CLI provides diagnostics → Claude Code interprets
- No IDE tooling needed → Claude Code is the interface

**Workflow:**
```
Developer: "Create email validation function"
Claude Code: [Generates dense canonical code]
Claude Code: "I've created validate_email(email:𝕊)→𝔹!Error that checks..."
Developer: Asks questions via Claude Code (never touches dense syntax)
Git: Commits .sigil file
```

## The Analogy

**Nobody writes these directly:**
- Minified JavaScript (we write source, minifier optimizes)
- Machine code (we write C/Rust, compiler optimizes)
- SQL execution plans (we write queries, optimizer decides)

**Similarly - nobody writes Sigil directly:**
- Claude Code writes Sigil (machine-optimal)
- Humans review via Claude Code (AI explanations)
- Everyone benefits: compact code, better understanding than documentation

## Design Decisions

### Why Not Just Use Existing Languages?

**Python:** Designed for human readability (verbose keywords, flexible syntax)
**JavaScript:** Too many ways to do everything (var/let/const, ===/==, function definitions)
**Rust:** Close! But designed for human experts (steep learning curve, syntax complexity)
**Haskell:** Also close! But academic (type classes, monads, complex syntax)

**Sigil learns from all of these but optimizes differently.**

### Why Unicode Symbols?

**Objection:** "Unicode is hard to type!"
**Response:** You don't type it - AI does. IDE provides helpers: type `lambda` → inserts `λ`

**Objection:** "Unicode is hard to read!"
**Response:** You don't read it - you ask Claude Code to explain it. Dense code is for execution.

**Objection:** "What about tokenization efficiency?"
**Response:** We benchmark this! If `λ` tokenizes to multiple tokens vs `fn` to one, we'll reconsider. But early evidence suggests modern LLM tokenizers handle common Unicode efficiently.

**Benefits:**
- Universal mathematical meaning (ℤ = integers, ∀ = forall)
- More information per character
- Beautiful rendering in modern editors
- Unambiguous semantics

### Why Functional?

**Objection:** "Functional programming has a steep learning curve!"
**Response:** For humans, yes. For LLMs trained on millions of lines of Haskell/OCaml/F#? No. They excel at functional code.

**Objection:** "Performance?"
**Response:** We compile to TypeScript (then standard toolchains transpile to JavaScript). Modern JS engines optimize functional code well. For true performance-critical sections, escape hatches exist.

**Benefits:**
- Easier to reason about (no hidden state)
- Better type inference
- Natural composition
- Simpler AI explanations

## The Vision

### Short Term (2026)

Proof-of-concept:
- Compiler to TypeScript
- Claude Code integration
- Token efficiency benchmarks
- LLM generation accuracy studies

**Success metric:** 40%+ token reduction, >99% LLM syntax correctness

### Medium Term (2027)

Production tooling:
- LSP server with semantic overlay
- Web playground
- Standard library (stdlib)
- Package manager (sigilpm)
- MCP server for documentation

**Success metric:** Developers prefer AI-mediated Sigil coding for new projects

### Long Term (2028+)

**The future of programming:**

1. **Natural language specs** → Claude Code generates Sigil code
2. **AI pair programming** → Modify code via conversation with Claude Code
3. **Perfect understanding** → Claude Code explanations better than comments
4. **Massive context** → More code fits in LLM windows
5. **Zero ambiguity** → Type checker catches everything
6. **AI evolution** → Better models → better explanations (code unchanged)

## Controversial Takes

### "Code Readability" is Overrated

For 50 years we optimized for humans to read code directly. Result: verbose languages, inconsistent styles, endless formatter debates.

**New paradigm:** Code optimized for execution. AI explains it to humans.

This does **not** eliminate the need for human-facing references. Sigil still needs a canonical syntax reference for:
- debugging and review when AI output looks wrong
- compiler/LSP/tooling contributors
- grounding prompts and examples against the current language surface

The key difference is that syntax docs are a **reference for verification and tooling**, not a primary hand-authoring workflow.

Like assembly vs C vs Python - each level optimizes for different audience. Sigil optimizes for AI.

### Types Should Be Mandatory and Explicit

Dynamic typing: fast prototyping, runtime errors
Explicit static typing: safe but verbose

**Sigil:** Bidirectional type checking with mandatory annotations - explicit, canonical, and machine-checkable.

### One Right Way > Flexibility

Python's "multiple ways to do it" causes:
- Style guide battles (PEP 8)
- Formatter wars (black vs autopep8)
- Code review bikeshedding
- LLM uncertainty

**Sigil:** ONE way. Enforced by parser. No debates.

### AI Will Write Most Code Anyway

Current (2026): 93% AI-assisted
Future (2028): 99%+ AI-generated

Why optimize for the 1%? Design for the majority use case.

## Inspiration

**Languages:**
- Haskell: Type inference, functional purity, algebraic data types
- OCaml: Pragmatic functional programming
- Rust: Borrow checker, memory safety, sum types
- Clojure: Token efficiency, data-first
- MoonBit: AI-native language design

**Concepts:**
- TypeScript/JavaScript source maps: Mapping optimized code to source
- Minification: Machine-optimal vs human-optimal
- Language servers (LSP): AI-powered IDE integration
- Model Context Protocol (MCP): LLM-queryable documentation

## Frequently Asked Questions

**Q: Why would anyone use this?**
A: When 40% more code fits in context, LLMs generate better results. When types catch 80% of bugs, you ship faster. When AI explains everything, you understand faster.

**Q: Can humans write Sigil?**
A: Yes, like humans CAN write minified JavaScript. But why? Use AI.

**Q: What if AI writes bad code?**
A: Types catch most errors. Claude Code explains what code does. Humans review and approve. Net result: fewer bugs than hand-written code.

**Q: Is this just code golf?**
A: No. Code golf sacrifices readability for brevity. Sigil sacrifices DIRECT readability for brevity, but provides BETTER understanding via Claude Code.

**Q: What about debugging?**
A: Ask Claude Code! "Why is this failing?" gets you better explanations than reading stack traces.

**Q: Won't this make developers obsolete?**
A: No. It shifts work from writing syntax to describing intent, reviewing semantics, architecting systems. Higher-level thinking.

---

**Sigil** - Fresh code for AI 🌿

*The first language designed for 99% AI generation, 100% human understanding*
