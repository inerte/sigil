# Getting Started with Mint

Welcome to Mint - the machine-first programming language! This guide will help you explore the current proof-of-concept implementation.

## What Works Now

✅ **Lexer** - Fully implemented and tested. Tokenizes Unicode Sigil code.
✅ **Parser** - Complete recursive descent parser. Builds Abstract Syntax Trees (AST).
⏳ **Type Checker** - Coming next (Hindley-Milner type inference)
⏳ **Code Generator** - Coming soon (compile to JavaScript)
⏳ **Semantic Map Generator** - Coming soon (AI explanations)

## Prerequisites

- Node.js (v24 LTS recommended, v22+ also works) - managed via nvm
- pnpm (faster alternative to npm: `npm install -g pnpm`)

## Installation

1. **Clone and navigate:**
```bash
cd REPO_ROOT
```

2. **Install dependencies (using pnpm):**
```bash
pnpm install
```

3. **Build the compiler:**
```bash
pnpm --filter @sigil-lang/compiler build
# Or from compiler directory:
cd compiler && pnpm build
```

## Using the Lexer

### Tokenize an example file:

```bash
node compiler/dist/cli.js lex examples/fibonacci.sigil
```

Output:
```
Tokens for examples/fibonacci.sigil:

LAMBDA(λ) at 1:1
IDENTIFIER(fibonacci) at 1:2
LPAREN(() at 1:11
...
Total tokens: 37
```

### Try other examples:

```bash
# Factorial function
node compiler/dist/cli.js lex examples/factorial.sigil

# Type definitions
node compiler/dist/cli.js lex examples/types.sigil

# List operations (map, filter, reduce)
node compiler/dist/cli.js lex examples/list-operations.sigil

# HTTP handler
node compiler/dist/cli.js lex examples/http-handler.sigil
```

## Exploring Mint Code

### Example 1: Fibonacci (Dense Format)

**examples/fibonacci.sigil:**
```sigil
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
```

**What it means** (from fibonacci.sigil.map):
> Computes the nth Fibonacci number recursively.
> Base cases: F(0)=0, F(1)=1
> Recursive case: F(n) = F(n-1) + F(n-2)

### Example 2: Type Definitions

**examples/types.sigil:**
```sigil
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
t User={id:ℤ,name:𝕊,email:𝕊,active:𝔹}
```

**Breakdown:**
- `t` = type declaration keyword
- `Option[T]` = generic type with type parameter T
- `Some(T)|None` = sum type (tagged union)
- `{id:ℤ,...}` = product type (record/struct)

### Example 3: HTTP Handler

**examples/http-handler.sigil:**
```sigil
λhandle_request(req:Request)→Result[Response,Error]≡req.path{
  "/users"→get_users(req)|
  "/health"→Ok(Response{status:200,body:"OK",headers:{}})|
  _→Err(Error{code:404,msg:"Not found"})
}
```

**Pattern matching on request path:**
- `/users` → delegate to get_users
- `/health` → return 200 OK
- `_` (wildcard) → return 404 error

## Understanding Unicode Symbols

Mint uses Unicode for ultimate token density:

| Symbol | Meaning | ASCII Alternative | Tokens Saved |
|--------|---------|-------------------|--------------|
| `λ` | lambda (function) | `fn` or `function` | 1-7 chars |
| `→` | arrow (returns, maps to) | `->` or `=>` | 0-1 chars |
| `≡` | equivalence (pattern match) | `match` | 4 chars |
| `ℤ` | integers (from ℤ in math) | `Int` or `int` | 2 chars |
| `ℝ` | real numbers | `Float` | 4 chars |
| `𝔹` | booleans | `Bool` | 3 chars |
| `𝕊` | strings | `String` | 5 chars |
| `⊤` | true (top) | `true` | 3 chars |
| `⊥` | false (bottom) | `false` | 4 chars |
| `≠` | not equal | `!=` | 0-1 chars |
| `≤` | less than or equal | `<=` | 0-1 chars |
| `≥` | greater than or equal | `>=` | 0-1 chars |
| `∧` | logical and | `&&` or `and` | 1-2 chars |
| `∨` | logical or | `\|\|` or `or` | 1-2 chars |
| `¬` | logical not | `!` or `not` | 0-2 chars |

**Total savings:** ~40-60% fewer tokens for equivalent code!

## Writing Mint Code

### Option 1: Type Unicode Directly (if you have Unicode input)

On macOS, you can use the Character Viewer (Ctrl+Cmd+Space) to insert symbols.

### Option 2: Copy from Examples

All example files use the correct symbols - just copy and modify.

### Option 3: Wait for IDE Extension (Coming Soon)

The VS Code extension will let you type ASCII and auto-convert:
- Type `lambda` → auto-converts to `λ`
- Type `->` → auto-converts to `→`
- Type `Int` → auto-converts to `ℤ`

## Reading Semantic Maps

Each `.sigil` file has a corresponding `.sigil.map` file with AI-generated explanations.

**Example:** Open `examples/fibonacci.sigil.map` to see:
```json
{
  "version": 1,
  "file": "fibonacci.sigil",
  "mappings": {
    "fibonacci": {
      "summary": "Computes the nth Fibonacci number recursively",
      "explanation": "Classic recursive approach...",
      "complexity": "O(2^n) time, O(n) space",
      "warnings": ["Inefficient for large n..."],
      "examples": ["fibonacci(5) = 5", ...]
    }
  }
}
```

**In the future:** IDE will show this automatically on hover!

## Current Limitations

⚠️ **Type checker not yet implemented** - no type inference yet
⚠️ **Code generator not yet implemented** - can't run programs yet (coming soon!)
⚠️ **No IDE extension yet** - use text editors manually
⚠️ **Semantic map generator not built** - .sigil.map files are hand-written examples

## Project Structure

```
ai-pl/
├── README.md              # Project overview
├── STATUS.md              # Implementation progress
├── GETTING_STARTED.md     # This file!
├── spec/
│   ├── grammar.ebnf       # Formal grammar
│   ├── type-system.md     # Type system specification
│   ├── sourcemap-format.md# Semantic map format
│   └── stdlib-spec.md     # Standard library design
├── docs/
│   └── philosophy.md      # Why machine-first?
├── examples/
│   ├── fibonacci.sigil     # Example programs
│   ├── fibonacci.sigil.map # Semantic explanations
│   └── ...
├── compiler/
│   ├── src/
│   │   ├── lexer/         # Tokenizer (✅ complete)
│   │   ├── parser/        # AST parser (✅ complete)
│   │   ├── typechecker/   # Type inference (⏳ next)
│   │   └── codegen/       # JS compiler (⏳ next)
│   └── dist/              # Compiled output
└── tools/                 # LSP, extensions (⏳ later)
```

## Next Steps

1. **Explore examples** - Read the `.sigil` files and their `.sigil.map` explanations
2. **Study the grammar** - See `spec/grammar.ebnf` for complete syntax
3. **Read the philosophy** - Understand why Mint is designed this way (`docs/philosophy.md`)
4. **Watch this space** - Parser, type checker, and code generator coming soon!

## Contributing Ideas

While the POC is in active development, here are areas where research/input would be valuable:

1. **Unicode Tokenization Benchmarks**
   - How do GPT-4, Claude, DeepSeek tokenize `λ` vs `fn`?
   - Is there a measurable difference in token count?

2. **LLM Generation Testing**
   - Can current LLMs generate syntactically correct Sigil code?
   - What prompt engineering works best?

3. **Alternative Syntax Explorations**
   - Are there better Unicode symbols?
   - Should we have ASCII fallbacks?

4. **Standard Library Design**
   - What functions are truly essential?
   - How should effects be organized?

5. **Error Messages for LLMs**
   - What format helps LLMs self-correct?
   - Should errors be JSON for machine parsing?

## Questions?

This is a research project exploring machine-first language design. The core question:

**If we optimize languages for AI to write instead of humans to write, what would change?**

Mint is one answer. We're excited to see where this leads!

---

**Happy exploring!** 🌿

**Status:** Proof-of-concept in active development
**Lexer:** ✅ Complete and tested
**Parser:** ✅ Complete - builds full AST
**Next:** Type checker improvements and module-system/typechecking enhancements (bidirectional checker)

For more details, see [STATUS.md](STATUS.md)
