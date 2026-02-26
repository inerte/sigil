# Sigil Programming Language
## "Minimal Interpreted" - A Machine-First Language for the AI Era

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Status: Proof of Concept](https://img.shields.io/badge/Status-Proof%20of%20Concept-orange.svg)]()

> **"Code optimized for machines to write, AI to explain, and humans to guide."**

## What is Sigil?

**Sigil** is a revolutionary programming language that inverts traditional programming language design priorities:

- **Traditional Languages**: Optimize for humans writing → machines execute
- **Sigil**: Optimize for machines (LLMs) writing → humans understand via AI interpretation

### The Core Innovation

Sigil introduces **semantic source maps** (.sigil.map) - like TypeScript/JavaScript source maps, but for human understanding:

```
Sigil Code (.sigil)     ← What runs (optimized for LLMs/execution)
      ↕ (mapped by)
Semantic Map (.map)   ← What humans read (optimized for understanding)
```

**humans rarely write Sigil directly.** Instead, they use AI to generate and modify code while reviewing semantic explanations.

## Quick Example

### What's Stored (Dense Format - fibonacci.sigil)
```sigil
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
```

### What Humans See (IDE with Semantic Map)
```
💬 "This function calculates the nth Fibonacci number recursively.
    Base cases: F(0)=0, F(1)=1
    Recursive case: F(n) = F(n-1) + F(n-2)

    Complexity: O(2^n) time, O(n) space
    Warning: Inefficient for large n - consider memoization"
```

**~10-15% fewer tokens than TypeScript (11.2% avg in current benchmark suite)** - More code fits in LLM context windows!

## First-Class Testing (Agent-First)

Sigil includes first-class `test` declarations and a built-in test runner:

```bash
# JSON output by default (machine-readable)
node language/compiler/dist/cli.js test

# Human-readable output
node language/compiler/dist/cli.js test --human

# Run a subset by description substring
node language/compiler/dist/cli.js test --match "toggle"
```

- Tests must live under `./tests`
- Test bodies return `𝔹`
- Effectful tests declare effects explicitly
- Scoped mocking is built-in via `mockable` + `with_mock(...) { ... }`
- Test files run in parallel by default (JSON results are output in stable order)

See `docs/TESTING.md`.

## Module System (Typed Imports)

Sigil-to-Sigil imports are typechecked across modules (not trust-mode `any`).

Canonical Sigil imports:

```sigil
i src⋅todo-domain
i stdlib⋅list
```

Canonical exports are explicit:

```sigil
export λaddTodo(...)→...
export t Todo={...}
export c version:𝕊="1"
```

- Only `src/...` and `stdlib/...` are valid Sigil import roots
- Import cycles are compile errors
- FFI (`e module⋅path`) remains trust-mode and link-time validated

## Why Machine-First Design?

### The Paradigm Shift

If 93% of code is AI-generated (2026 stats), why optimize for the 7%?

### Key Advantages

1. **Token Density**: `λ` instead of `function` - machines don't need verbosity
2. **Zero Ambiguity**: Exactly ONE way to write anything - LLMs hallucinate less
3. **Perfect Formatting**: Code won't compile if not canonically formatted
4. **Strong Types**: Bidirectional type checking with mandatory annotations
5. **Context Efficiency**: ~10% more code fits in context windows (current benchmark average)

### How Humans Interact

Developers interact via the **AI Interpretation Layer**:

- **LSP** that shows semantic explanations on hover
- **AI assistants** that write/edit the dense code
- **Visual debugging** tools with natural language explanations
- **Semantic maps** (.sigil.map) that persist AI-generated documentation

## Design Principles

### 1. Radical Canonicalization
**"There is only one way to write it"**

- No alternative syntaxes for the same construct
- No optional keywords, brackets, or delimiters
- No syntactic sugar creating multiple representations
- Single import style, single function definition, single loop construct

### 2. Strong, Checked Types
**"Types are mandatory and checked bidirectionally"**

- Bidirectional type checking (synthesis ⇒ and checking ⇐ modes)
- Type annotations required on all function signatures (canonical form)
- No dynamic typing, no `any` type, controlled coercion
- Algebraic data types (sum + product types)
- Effect system for tracking side effects (planned)
- Compile-time guarantees prevent runtime type errors
- Better error messages than Hindley-Milner: "expected ℤ, got 𝕊"

### 3. Enforced Canonical Formatting
**"Unformatted code is a syntax error"**

- Formatter is part of the parser, not a separate tool
- Code that violates formatting rules doesn't parse
- LLMs learn ONE valid token sequence per semantic meaning

### 4. Minimal Token Syntax with Unicode
**"Every character carries maximum information density"**

Unicode symbols for ultimate density:
- `λ` for function (1 char vs 2-8)
- `→` for returns/maps-to (1 char vs 2)
- `≡` for pattern match (1 char vs 5+)
- `ℤ` for integers, `ℝ` for reals, `𝔹` for bool, `𝕊` for string
- `↦` for map (1 char vs 4)
- `⊳` for filter (1 char vs 7)
- `⊕` for fold/reduce (1 char vs 7)
- `∈` for iteration "in"
- `∅` for None/empty
- `⊤` for true, `⊥` for false

### 5. Functional-First Paradigm
**"It's all about the data"**

- Everything is an expression
- Immutable by default
- Pattern matching (only control flow)
- Algebraic data types
- No null - Option type only
- No exceptions - Result type only
- First-class functions

### 6. Built-in Correctness
**"Prevent errors at compile time"**

- Result/Option types for error handling
- Exhaustive pattern matching enforced
- No null/undefined
- Borrow checker for memory safety

## Syntax Examples

### Function Definition
```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
```

### Pattern Matching
```sigil
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
```

### HTTP Handler Example
```sigil
λhandle_request(req:Request)→Response!Error≡req.path{"/users"→get_users(req)|"/health"→Ok(Response{status:200,body:"OK"})|_→Err(Error{code:404,msg:"Not found"})}
```

### Data Types
```sigil
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
t User={id:ℤ,name:𝕊,email:𝕊,active:𝔹}
```

### Built-in List Operations
```sigil
⟦ Map: ↦ - Apply function to each element ⟧
[1,2,3,4,5]↦λx→x*2  ⟦ Result: [2,4,6,8,10] ⟧

⟦ Filter: ⊳ - Keep elements matching predicate ⟧
[1,2,3,4,5]⊳λx→x%2=0  ⟦ Result: [2,4] ⟧

⟦ Fold: ⊕ - Reduce with function and initial value ⟧
[1,2,3,4,5]⊕λ(acc,x)→acc+x⊕0  ⟦ Result: 15 ⟧

⟦ Chained operations ⟧
[1,2,3,4,5]↦λx→x*2⊳λx→x>5⊕λ(acc,x)→acc+x⊕0  ⟦ Result: 30 ⟧
```

### Pipeline Operations
```sigil
λprocess_users(users:[User])→[𝕊]=users|>filter(λu→u.active)|>map(λu→u.name)
```

## Token Efficiency Comparison

**Measured with `tiktoken` (`cl100k_base`) vs TypeScript across 8 benchmark algorithms**  
See `benchmarks/RESULTS.md` for methodology and per-algorithm code.

| Algorithm | Sigil Tokens | TypeScript Tokens | Improvement |
|----------|-------------:|------------------:|------------:|
| factorial | 45 | 52 | +15.6% |
| fibonacci | 45 | 60 | +33.3% |
| gcd | 43 | 48 | +11.6% |
| power | 47 | 52 | +10.6% |
| map-double | 53 | 59 | +11.3% |
| filter-even | 61 | 67 | +9.8% |
| is-palindrome | 45 | 49 | +8.9% |
| sum-list | 55 | 50 | -9.1% |
| **Average** | **49.3** | **54.6** | **+11.2%** |

**Practical takeaway:** current evidence supports a **~10-15% token reduction**, not 40-60%.

## Developer Workflow

### Traditional Workflow
```
Developer writes code → Compiler checks → If error, developer fixes
```

### Sigil Workflow
```
Developer: "Create a function that validates email addresses"
AI: [Generates dense code + semantic map]
AI: "I've created validate_email(email:𝕊)→𝔹!Error. It checks:
     - Contains exactly one @
     - Has characters before and after @
     - Domain has at least one dot"
Developer: Reviews semantic map, approves
Git: Commits both .sigil and .sigil.map
```

## Semantic Source Maps

Every `.sigil` file has a corresponding `.sigil.map` file:

**fibonacci.sigil** (what executes):
```sigil
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
```

**fibonacci.sigil.map** (human interpretation):
```json
{
  "version": 1,
  "file": "fibonacci.sigil",
  "mappings": {
    "function": {
      "range": [0, 67],
      "summary": "Computes the nth Fibonacci number recursively",
      "explanation": "Classic recursive Fibonacci. Base cases: F(0)=0, F(1)=1. For other values, sums the previous two Fibonacci numbers.",
      "complexity": "O(2^n) time, O(n) space",
      "warnings": ["Inefficient for large n", "Consider memoization"]
    }
  }
}
```

## IDE Features

The **AI Interpretation Layer** provides:

- **Hover tooltips**: Instant semantic explanations (from .sigil.map)
- **Unicode input helpers**: Type `lambda` → auto-insert `λ`
- **Semantic view panel**: Detailed explanations of selected code
- **Natural language queries**: "What does line 47 do?"
- **AI-mediated editing**: "Add error logging" → AI modifies code
- **Beautiful rendering**: Proper Unicode fonts and ligatures

## Project Status

**Current Phase**: Proof of Concept (Week 2-3)

### Completed
- ✅ Language design and philosophy
- ✅ Core syntax specification
- ✅ Project structure
- ✅ Lexer/Parser implementation
- ✅ TypeScript code generator
- ✅ Built-in list operations (↦ ⊳ ⊕)
- ✅ Canonical form enforcement (refined - blocks accumulator patterns, allows legitimate multi-param)
- ✅ Parameter classification via static analysis (structural, query, accumulator)
- ✅ Comprehensive test suite (18 tests)
- ✅ Pattern matching validation
- ✅ Multi-parameter recursion (GCD, binary search, nth, power, Hanoi - no accumulators)
- ✅ Type checker (Bidirectional with mandatory annotations) - ✓ COMPLETED (2026-02-22)
  - Bidirectional synthesis (⇒) and checking (⇐) modes
  - Mandatory type annotations on all function signatures
  - Pattern matching with exhaustiveness checking
  - List operations (↦, ⊳, ⊕) as language constructs
  - Better error messages with precise source locations
- ✅ Mutability checker (Immutable by default) - ✓ COMPLETED (2026-02-23)
  - Explicit `mut` keyword for mutable parameters
  - Compile-time prevention of illegal mutations
  - Aliasing prevention for mutable values
  - Clear error messages with source locations
- ✅ Multi-line comments with ⟦ ... ⟧ brackets - ✓ COMPLETED (2026-02-23)
  - Can span multiple lines
  - Can be inserted anywhere (mid-expression)
  - Stripped during lexing
  - Canonical form (only ONE comment syntax)
- ✅ Semantic map generator (Basic) - ✓ COMPLETED (2026-02-23)
  - Auto-generated during compilation
  - Extracts ranges, types, basic summaries from AST
  - JSON format matching spec/sourcemap-format.md
  - Outputs .sigil.map beside source files
  - Ready for enhancement via Claude Code CLI

- ✅ LSP Server (Phase 1: Basic Infrastructure) - ✓ COMPLETED (2026-02-23)
  - Server with document tracking and diagnostics
  - Real-time error reporting (syntax, type, canonical violations)
  - Hover provider showing semantic map content
  - Unicode symbol completion (lambda → λ, arrow → →, etc.)
  - Document symbols for outline view
  - Built on vscode-languageserver protocol

- ✅ VS Code Extension (Phase 2: Editor Integration) - ✓ COMPLETED (2026-02-23)
  - Full VS Code extension with LSP client
  - Syntax highlighting (TextMate grammar)
  - Language configuration (brackets, auto-closing, comments)
  - Extension activation for .sigil files
  - Ready to package and install

### In Progress
- 🔄 Testing and refinement

### Upcoming
- ⏳ VS Code extension packaging and distribution
- ⏳ Token efficiency benchmarks
- ⏳ LLM generation accuracy tests

## Installation (Future)

```bash
# Install Sigil compiler
brew install sigil-lang

# Create new project
sigil new my-project

# Compile to TypeScript
sigilc compile src/main.sigil --output dist/main.ts

# Generate semantic maps
sigilc map generate src/**/*.sigil

# Run REPL
sigil
```

## Documentation

- [Philosophy](docs/philosophy.md) - Why machine-first?
- [Syntax Reference](docs/syntax-reference.md) - Canonical syntax reference
- [Type System](docs/type-system.md) - Types and inference
- [Semantic Maps](docs/semantic-maps.md) - How .sigil.map works
- [Specification](spec/) - Formal language specification

## Contributing

This is a research project exploring machine-first language design. Contributions welcome!

**Areas of interest:**
- Unicode tokenization benchmarks (critical!)
- LLM code generation accuracy studies
- Alternative syntax explorations
- Tooling improvements (LSP, IDE extensions)
- Standard library design

## Research Questions

1. **Unicode Tokenization**: Do modern LLM tokenizers handle `λ` as 1 token or multiple?
2. **Generation Accuracy**: Can LLMs achieve >99% syntax correctness with canonical format?
3. **Developer Experience**: Do developers prefer AI-mediated coding over direct writing?
4. **Token Efficiency**: How much token reduction do we achieve in practice beyond the current ~11.2% benchmark average?
5. **Context Utilization**: Does denser code enable better LLM reasoning?

## License

MIT License - See [LICENSE](LICENSE) file

## Acknowledgments

Inspired by:
- [MoonBit's AI-Native Language Design](https://www.moonbitlang.com/blog/ai-coding)
- Haskell's type inference and functional purity
- OCaml's algebraic data types
- Rust's borrow checker and ownership model
- TypeScript/JavaScript source maps (the inspiration for semantic maps)

## Philosophy

**"This is a machine language, not a human language"**

Like XML vs JSON vs YAML - optimized for machine reading/writing, not human aesthetics. The difference is that we add an AI layer to make it understandable.

**The future of programming:**
- Nobody writes transpiled JavaScript directly → toolchains do it
- Nobody writes Sigil directly → AI does it
- Humans guide through natural language, AI generates optimal code
- Semantic maps make it more understandable than hand-written code

---

**Sigil** - Fresh code for AI 🌿
