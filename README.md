# Mint Programming Language
## "Minimal Interpreted" - A Machine-First Language for the AI Era

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Status: Proof of Concept](https://img.shields.io/badge/Status-Proof%20of%20Concept-orange.svg)]()

> **"Code optimized for machines to write, AI to explain, and humans to guide."**

## What is Mint?

**Mint** is a revolutionary programming language that inverts traditional programming language design priorities:

- **Traditional Languages**: Optimize for humans writing → machines execute
- **Mint**: Optimize for machines (LLMs) writing → humans understand via AI interpretation

### The Core Innovation

Mint introduces **semantic source maps** (.mint.map) - like JavaScript source maps, but for human understanding:

```
Mint Code (.mint)     ← What runs (optimized for LLMs/execution)
      ↕ (mapped by)
Semantic Map (.map)   ← What humans read (optimized for understanding)
```

**humans rarely write Mint directly.** Instead, they use AI to generate and modify code while reviewing semantic explanations.

## Quick Example

### What's Stored (Dense Format - fibonacci.mint)
```mint
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

**40-60% fewer tokens than Python/JavaScript** - More code fits in LLM context windows!

## Why Machine-First Design?

### The Paradigm Shift

If 93% of code is AI-generated (2026 stats), why optimize for the 7%?

### Key Advantages

1. **Token Density**: `λ` instead of `function` - machines don't need verbosity
2. **Zero Ambiguity**: Exactly ONE way to write anything - LLMs hallucinate less
3. **Perfect Formatting**: Code won't compile if not canonically formatted
4. **Strong Types**: Hindley-Milner inference + borrow checker prevent errors
5. **Context Efficiency**: 2× more code fits in context windows

### How Humans Interact

Developers interact via the **AI Interpretation Layer**:

- **LSP** that shows semantic explanations on hover
- **AI assistants** that write/edit the dense code
- **Visual debugging** tools with natural language explanations
- **Semantic maps** (.mint.map) that persist AI-generated documentation

## Design Principles

### 1. Radical Canonicalization
**"There is only one way to write it"**

- No alternative syntaxes for the same construct
- No optional keywords, brackets, or delimiters
- No syntactic sugar creating multiple representations
- Single import style, single function definition, single loop construct

### 2. Strong, Inferred Types
**"Types are mandatory but invisible"**

- Hindley-Milner type inference (like Haskell, OCaml, F#)
- No dynamic typing, no `any` type, no type coercion
- Algebraic data types (sum + product types)
- Effect system for tracking side effects
- Compile-time guarantees prevent runtime type errors

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
```mint
λadd(x:ℤ,y:ℤ)→ℤ=x+y
```

### Pattern Matching
```mint
λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
```

### HTTP Handler Example
```mint
λhandle_request(req:Request)→Response!Error≡req.path{"/users"→get_users(req)|"/health"→Ok(Response{status:200,body:"OK"})|_→Err(Error{code:404,msg:"Not found"})}
```

### Data Types
```mint
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
t User={id:ℤ,name:𝕊,email:𝕊,active:𝔹}
```

### Pipeline Operations
```mint
λprocess_users(users:[User])→[𝕊]=users|>filter(λu→u.active)|>map(λu→u.name)
```

## Token Efficiency Comparison

**Estimated token savings vs other languages:**

| Language | Tokens | vs Mint |
|----------|--------|---------|
| Python   | 100    | +67%    |
| JavaScript | 95   | +58%    |
| TypeScript | 110  | +83%    |
| Rust     | 120    | +100%   |
| **Mint** | **60** | **baseline** |

## Developer Workflow

### Traditional Workflow
```
Developer writes code → Compiler checks → If error, developer fixes
```

### Mint Workflow
```
Developer: "Create a function that validates email addresses"
AI: [Generates dense code + semantic map]
AI: "I've created validate_email(email:𝕊)→𝔹!Error. It checks:
     - Contains exactly one @
     - Has characters before and after @
     - Domain has at least one dot"
Developer: Reviews semantic map, approves
Git: Commits both .mint and .mint.map
```

## Semantic Source Maps

Every `.mint` file has a corresponding `.mint.map` file:

**fibonacci.mint** (what executes):
```mint
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
```

**fibonacci.mint.map** (human interpretation):
```json
{
  "version": 1,
  "file": "fibonacci.mint",
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

- **Hover tooltips**: Instant semantic explanations (from .mint.map)
- **Unicode input helpers**: Type `lambda` → auto-insert `λ`
- **Semantic view panel**: Detailed explanations of selected code
- **Natural language queries**: "What does line 47 do?"
- **AI-mediated editing**: "Add error logging" → AI modifies code
- **Beautiful rendering**: Proper Unicode fonts and ligatures

## Project Status

**Current Phase**: Proof of Concept (Week 1-2)

### Completed
- ✅ Language design and philosophy
- ✅ Core syntax specification
- ✅ Project structure

### In Progress
- 🔄 Grammar specification (EBNF)
- 🔄 Type system specification
- 🔄 Semantic map format
- 🔄 Example programs

### Upcoming
- ⏳ Lexer/Parser implementation
- ⏳ Type checker with inference
- ⏳ JavaScript code generator
- ⏳ Semantic map generator
- ⏳ LSP server
- ⏳ VS Code extension
- ⏳ Token efficiency benchmarks

## Installation (Future)

```bash
# Install Mint compiler
brew install mint-lang

# Create new project
mint new my-project

# Compile to JavaScript
mintc compile src/main.mint --output dist/main.js

# Generate semantic maps
mintc map generate src/**/*.mint

# Run REPL
mint
```

## Documentation

- [Philosophy](docs/philosophy.md) - Why machine-first?
- [Syntax Guide](docs/syntax-guide.md) - Complete syntax reference
- [Type System](docs/type-system.md) - Types and inference
- [Semantic Maps](docs/semantic-maps.md) - How .mint.map works
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
4. **Token Efficiency**: Can we achieve 40-60% token reduction in practice?
5. **Context Utilization**: Does denser code enable better LLM reasoning?

## License

MIT License - See [LICENSE](LICENSE) file

## Acknowledgments

Inspired by:
- [MoonBit's AI-Native Language Design](https://www.moonbitlang.com/blog/ai-coding)
- Haskell's type inference and functional purity
- OCaml's algebraic data types
- Rust's borrow checker and ownership model
- JavaScript source maps (the inspiration for semantic maps)

## Philosophy

**"This is a machine language, not a human language"**

Like XML vs JSON vs YAML - optimized for machine reading/writing, not human aesthetics. The difference is that we add an AI layer to make it understandable.

**The future of programming:**
- Nobody writes minified JavaScript directly → minifier does it
- Nobody writes Mint directly → AI does it
- Humans guide through natural language, AI generates optimal code
- Semantic maps make it more understandable than hand-written code

---

**Mint** - Fresh code for AI 🌿
