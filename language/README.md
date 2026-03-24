# Sigil Programming Language
## "Minimal Interpreted" - A Machine-First Language for the AI Era

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Status: Proof of Concept](https://img.shields.io/badge/Status-Proof%20of%20Concept-orange.svg)]()

> **"Code optimized for machines to write, AI to explain, and humans to guide."**

## What is Sigil?

**Sigil** is a revolutionary programming language that inverts traditional programming language design priorities:

- **Traditional Languages**: Optimize for humans writing => machines execute
- **Sigil**: Optimize for machines (LLMs) writing => humans understand via AI interpretation

### The Core Innovation

**Humans don't read code anymore** - they ask Claude Code to explain it.

Sigil is optimized for:
- **AI generation**: Dense, canonical syntax reduces hallucinations
- **AI explanation**: Claude Code reads source and explains via CLI
- **Deterministic compilation**: One way to write anything ensures consistency

## Quick Example

### What's Written (Dense, Canonical Format)
```sigil module
λfibonacci(n:Int)=>Int=fibonacciHelper(0,1,n)

λfibonacciHelper(a:Int,b:Int,n:Int)=>Int match n{
  0=>a|
  count=>fibonacciHelper(b,a+b,count-1)
}
```

### How Humans Understand It
```
You: "Claude, what does fibonacci.sigil do?"
Claude Code: "This function calculates the nth Fibonacci number with a helper
              that threads the current and next values through one recursive
              step at a time.

              Base case: when n is 0, return the accumulator
              Recursive step: shift (a,b) to (b,a+b)

              Complexity: O(n) time, O(n) stack unless the backend turns the
              helper into a loop"
```

**28.1% fewer tokens than TypeScript in the current published benchmark corpus** - More code fits in LLM context windows.

## First-Class Testing (Agent-First)

Sigil includes first-class `test` declarations and a built-in test runner:

```bash
# JSON output by default (machine-readable)
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test

# Run a subset by description substring
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test --match "toggle"
```

- Tests must live under `./tests`
- Test bodies return `Bool`
- Effectful tests declare effects explicitly
- `config/<env>.lib.sigil` exports the baseline `world`
- Tests may derive that world locally with `world { ... }`
- `test::observe` and `test::check` inspect the active test world
- `sigil test` enforces project `src/*.lib.sigil` surface coverage
- Test files run in parallel by default (JSON results are output in stable order)

See `docs/TESTING.md`.

## Module System (Typed Imports)

Sigil-to-Sigil imports are typechecked across modules (not trust-mode `any`).

Canonical Sigil imports:

```sigil module projects/todo-app/src/countTodos.lib.sigil
i src::todoDomain

i stdlib::list

λtodoCount(todos:[src::todoDomain.Todo])=>Int=#todos
```

Exports are determined by file extension:

- **`.lib.sigil` files**: ALL declarations are automatically exported (libraries)
- **`.sigil` files**: NOTHING is exported (executables with `main()` function)

No `export` keyword exists - the file extension declares the intent.

- Only `src/...`, `config/...`, `core/...`, `stdlib/...`, `world/...`, and `test/...` are valid Sigil import roots
- Import cycles are compile errors
- FFI (`e module::path`) remains trust-mode and link-time validated

Sigil also has a very small implicit core prelude:
- `Option[T]`, `Result[T,E]`
- `Some`, `None`, `Ok`, `Err`

These are available without import because they define everyday control and data vocabulary. Most operational helpers still live in namespaced modules like `core::map`, `stdlib::string`, `stdlib::file`, `stdlib::path`, `stdlib::process`, and `stdlib::time`.

## Why Machine-First Design?

### The Paradigm Shift

If 93% of code is AI-generated (2026 stats), why optimize for the 7%?

### Key Advantages

1. **Token Density**: `λ` instead of `function` - machines don't need verbosity
2. **Zero Ambiguity**: Exactly ONE way to write anything - LLMs hallucinate less
3. **One Textual Representation**: Code won't compile if it doesn't match the compiler's canonical printed form
   Parseable-but-non-canonical source is rejected by `compile`, `run`, and `test`.
4. **Strong Types**: Bidirectional type checking with mandatory annotations
5. **Context Efficiency**: ~10% more code fits in context windows (current benchmark average)

### How Humans Interact

Developers interact through **Claude Code**:

- **Ask Claude Code** to explain any code section
- **Claude Code writes/edits** the dense canonical code
- **Compiler CLI** provides diagnostic errors and type information
- **No IDE tooling needed** - Claude Code is the interface

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
- Better error messages than Hindley-Milner: "expected Int, got String"

### 3. Printer-First Canonical Source
**"There is one accepted textual form per program"**

- The compiler owns an internal canonical source printer
- Parseable-but-non-canonical source is rejected before codegen
- There is no public formatter command
- LLMs learn ONE valid text shape per AST

### 4. Minimal Token Syntax for Models
**"Every character carries maximum information density"**

Compact canonical syntax for model-facing efficiency:
- `λ` for function (1 char vs 2-8)
- `=>` for returns/maps-to
- `::` for namespace paths without colliding with field access
- `match` for pattern match (common keyword with strong model priors)
- `Int` for integers, `Float` for reals, `Bool` for bool, `String` for string
- `map` for list projection
- `filter` for list filtering
- `reduce` for list reduction
- `∈` for iteration "in"
- `Never` for None/empty
- `true` for true, `false` for false

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
```sigil module
λadd(x:Int,y:Int)=>Int=x+y
```

### Pattern Matching
```sigil module
λfactorial(n:Int)=>Int match n{
  0=>1|
  1=>1|
  value=>value*factorial(value-1)
}
```

### HTTP Handler Example
```sigil module
λhandleRequest(path:String)=>String match path{
  "/health"=>"OK"|
  "/users"=>"users"|
  _=>"not found"
}
```

### Data Types
```sigil module
t Option[T]=Some(T)|None()

t Result[T,E]=Ok(T)|Err(E)

t User={active:Bool,email:String,id:Int,name:String}
```

Sigil supports explicit parametric polymorphism on top-level declarations.
It does not use Hindley-Milner let-polymorphism for local bindings.

### Built-in List Operations
```sigil module
i stdlib::numeric

λdoubled()=>[Int]=[1,2,3,4,5] map (λ(x:Int)=>Int=x*2)

λevens()=>[Int]=[1,2,3,4,5] filter stdlib::numeric.isEven

λtotal()=>Int=[1,2,3,4,5] reduce (λ(acc:Int,x:Int)=>Int=acc+x) from 0
```

### Composed List Operations
```sigil module
t User={active:Bool,name:String}

λactiveNames(users:[User])=>[String]=users filter (λ(user:User)=>Bool=user.active) map (λ(user:User)=>String=user.name)
```

## Token Efficiency Comparison

**Measured with `tiktoken` (`cl100k_base`) vs TypeScript across 8 benchmark algorithms**  
See `benchmarks/tokens/RESULTS.md` for methodology and per-algorithm code.

| Algorithm | Sigil Tokens | TypeScript Tokens | Sigil Fewer Tokens vs TS |
|----------|-------------:|------------------:|-------------------------:|
| factorial | 44 | 52 | 15.4% |
| fibonacci | 57 | 60 | 5.0% |
| gcd | 21 | 48 | 56.3% |
| power | 44 | 52 | 15.4% |
| map-double | 44 | 59 | 25.4% |
| filter-even | 47 | 67 | 29.9% |
| is-palindrome | 31 | 49 | 36.7% |
| sum-list | 26 | 50 | 48.0% |
| **Average** | **39.3** | **54.6** | **28.1%** |

**Practical takeaway:** current published evidence supports roughly **28% fewer tokens than TypeScript** across the active 8-algorithm corpus.

## Developer Workflow

### Traditional Workflow
```
Developer writes code => Compiler checks => If error, developer fixes
```

### Sigil Workflow
```
Developer: "Create a function that validates email addresses"
Claude Code: [Generates dense code]
Claude Code: "I've created validate_email(email:String)=>Bool!Error. It checks:
              - Contains exactly one @
              - Has characters before and after @
              - Domain has at least one dot"
Developer: Reviews via compiler, approves
Git: Commits .sigil file
```

## AI-First Development

**Claude Code is the primary interface:**

```bash
# Compile code (machine-readable JSON output)
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile src/main.sigil

# Ask Claude Code to explain any code
"Claude, what does this function do?"
"Claude, why did compilation fail?"
"Claude, add error logging to main.sigil"

# Run tests
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test
```

**No IDE tooling needed** - Claude Code uses the compiler CLI directly:
- Reads source files
- Invokes compiler for diagnostics
- Explains code in natural language
- Writes/edits canonical Sigil code

## Project Status

**Current Phase**: Proof of Concept (Week 2-3)

### Completed
- ✅ Language design and philosophy
- ✅ Core syntax specification
- ✅ Project structure
- ✅ Lexer/Parser implementation
- ✅ TypeScript code generator
- ✅ Built-in list operations (`map` `filter` `reduce`)
- ✅ Canonical form enforcement (refined - blocks accumulator patterns, allows legitimate multi-param)
- ✅ Parameter classification via static analysis (structural, query, accumulator)
- ✅ Comprehensive test suite (18 tests)
- ✅ Pattern matching validation
- ✅ Multi-parameter recursion (GCD, binary search, nth, power, Hanoi - no accumulators)
- ✅ Type checker (Bidirectional with mandatory annotations) - ✓ COMPLETED (2026-02-22)
  - Bidirectional synthesis (⇒) and checking (⇐) modes
  - Mandatory type annotations on all function signatures
  - Pattern matching with exhaustiveness checking
  - List operations (`map`, `filter`, `reduce`) as language constructs
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

### In Progress
- 🔄 Testing and refinement

### Upcoming
- ⏳ Token efficiency benchmarks
- ⏳ LLM generation accuracy tests
- ⏳ Claude Code integration enhancements

## Installation

```bash
# Download the native CLI archive for your platform from GitHub Releases
# Extract the archive and move `sigil` onto your PATH
sigil --version

# Compile to TypeScript
sigil compile src/main.sigil -o dist/main.ts

# Run tests
sigil test
```

GitHub Releases are the canonical installation path.

- Official release versions use UTC timestamps in the format `YYYY-MM-DDTHH-mm-ssZ`
- Source builds are for contributors and compiler development
- Homebrew packaging is generated from the release artifacts in this repo and can publish to a separate tap repo when configured

## Documentation

- [Philosophy](docs/philosophy.md) - Why machine-first?
- [Syntax Reference](docs/syntax-reference.md) - Canonical syntax reference
- [Type System](docs/type-system.md) - Types and inference
- [Specification](spec/) - Formal language specification

## Contributing

This is a research project exploring machine-first language design. Contributions welcome!

**Areas of interest:**
- Unicode tokenization benchmarks (critical!)
- LLM code generation accuracy studies
- Alternative syntax explorations
- Claude Code integration improvements
- Standard library design

## Research Questions

1. **Unicode Tokenization**: Do modern LLM tokenizers handle `λ` as 1 token or multiple?
2. **Generation Accuracy**: Can LLMs achieve >99% syntax correctness with canonical format?
3. **Developer Experience**: Do developers prefer AI-mediated coding over direct writing?
4. **Token Efficiency**: How much token reduction do we achieve in practice beyond the current 28.1% published benchmark result?
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
- Nobody writes transpiled JavaScript directly => toolchains do it
- Nobody writes Sigil directly => Claude Code does it
- Humans guide through natural language, Claude Code generates optimal code
- Claude Code explains code better than human-written documentation

---

**Sigil** - Fresh code for AI 🌿
