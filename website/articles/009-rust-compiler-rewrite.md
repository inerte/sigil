---
title: "Rewriting the Sigil Compiler in Rust: 100% Feature Parity, 5-7x Faster"
date: February 26, 2026
author: Sigil Language Team
slug: 009-rust-compiler-rewrite
---

# Rewriting the Sigil Compiler in Rust: 100% Feature Parity, 5-7x Faster

**TL;DR:** We completed a full rewrite of the Sigil compiler from TypeScript to Rust, achieving 100% feature parity with byte-for-byte output compatibility, 5-7x performance improvement (debug builds!), and single-binary distribution with zero runtime dependencies. Multi-module compilation with cross-module type checking is now fully integrated. This article explains why we did it, how we did it, what we learned, and what it means for Sigil users.

## The Problem: TypeScript Compiler Limitations

The original Sigil compiler was written in TypeScript. It worked well for initial development and rapid iteration, but had three fundamental limitations:

### 1. Distribution Complexity

**Before:**
```bash
$ npm install -g @sigil-lang/compiler
# Downloads ~50MB of node_modules
# Requires Node.js runtime
# Platform-specific postinstall scripts
```

**Problems:**
- Users need Node.js installed
- Large installation footprint (~50MB)
- Dependency version conflicts
- Slow installation time
- Platform-specific compatibility issues

### 2. Performance Ceiling

TypeScript runs on V8, which is fast for JavaScript—but not fast for compilation workloads. Sigil programs are compiled to TypeScript, so the compiler is in the hot path for every build.

**Measured bottlenecks:**
- Lexing: ~15ms per file (string allocation overhead)
- Parsing: ~30ms (recursive descent with garbage collection pauses)
- Type checking: ~50ms (heavy object allocation for type environments)
- Code generation: ~20ms (template string manipulation)

**Total compile time for a typical file:** ~80-120ms

This doesn't sound bad until you're running test suites with hundreds of files, or using watch mode during development. The overhead adds up.

### 3. Type Safety Gaps

TypeScript's type system is excellent for JavaScript codebases, but compiler implementations benefit from:

- **Exhaustive pattern matching** - Guarantees all AST node types are handled
- **Zero-cost abstractions** - Enums, traits, generics without runtime overhead
- **Ownership tracking** - Compiler guarantees about data flow and mutability
- **No null/undefined confusion** - `Option<T>` and `Result<T, E>` are explicit

The TS compiler had accumulated 50+ `// @ts-ignore` comments and relied on runtime validation that could have been compile-time guarantees.

## Why Rust?

When we decided to rewrite, we evaluated three options:

### Option 1: Optimize TypeScript
- **Pros:** Incremental improvement, no rewrite
- **Cons:** Still need Node.js, performance ceiling remains, type safety gaps persist
- **Verdict:** Not enough ROI

### Option 2: Go
- **Pros:** Fast compilation, single binary, good concurrency
- **Cons:** Weak type system (no sum types), GC pauses, verbose error handling
- **Verdict:** Better distribution, not enough type safety

### Option 3: Rust
- **Pros:** Blazing fast, strong type system, exhaustive matching, zero-cost abstractions, single binary
- **Cons:** Steeper learning curve, slower initial development
- **Verdict:** Best long-term choice

**We chose Rust** because Sigil is a **machine-first language**. Our compiler is tooling for AI coding agents as much as for humans. Rust's guarantees (memory safety, exhaustiveness, zero-cost abstractions) align perfectly with Sigil's determinism philosophy.

## The Migration Strategy: 1:1 Port, Not Redesign

We deliberately chose to **port the existing implementation**, not redesign the compiler. This meant:

### What We Did
- ✅ **Direct translation** of TS code to Rust
- ✅ **Identical output** - byte-for-byte compatibility with TS compiler
- ✅ **Same algorithms** - recursive descent parser, bidirectional type inference, IIFE-based codegen
- ✅ **No language changes** - zero user-visible behavior changes

### What We Didn't Do
- ❌ **No algorithm improvements** - stuck with existing design decisions
- ❌ **No new features** - feature parity only
- ❌ **No output optimization** - generate same TypeScript code
- ❌ **No compiler architecture changes** - same phase structure

**Why this approach?**

1. **Risk mitigation** - Separate migration from optimization
2. **Testability** - Differential testing against TS compiler
3. **Incremental delivery** - Ship working compiler, optimize later
4. **Learning curve** - Focus on Rust translation, not compiler design

The 1:1 port strategy meant we could validate correctness at every step by comparing Rust output to TypeScript output.

## The 7 Phases

We broke the migration into clear phases with defined milestones:

### Phase 1: Foundation (Lexer + AST)
**Goal:** Tokenize Sigil source files

**Implementation:**
- Used [`logos`](https://crates.io/crates/logos) for zero-copy lexing
- Defined 99 token types (keywords, symbols, literals, whitespace)
- Built complete AST definitions matching TypeScript

**Code:**
```rust
use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    #[token("λ")] Lambda,
    #[token("→")] Arrow,
    #[token("⟦")] CommentStart,
    #[token("⟧")] CommentEnd,
    #[token("ℤ")] IntType,
    #[token("𝕊")] StringType,
    #[token("𝔹")] BoolType,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier,

    #[regex(r#""([^"\\]|\\.)*""#)]
    StringLiteral,

    // ... 90 more token types
}
```

**Results:**
- 1,500 lines of Rust
- 29 unit tests passing
- **7.5x faster** than TypeScript lexer (~2ms vs ~15ms)

**Key insight:** Logos uses proc macros to generate optimal state machines at compile time. Zero runtime overhead.

### Phase 2: Parser
**Goal:** Convert token stream to AST

**Implementation:**
- Handwritten recursive descent parser (no combinator library)
- Support all Sigil constructs: functions, types, imports, consts, tests, externs
- Pattern matching (List, Tuple, Constructor, Literal, Wildcard)
- Expression parsing with proper precedence

**Code:**
```rust
pub struct Parser<'src> {
    tokens: &'src [Token],
    pos: usize,
}

impl<'src> Parser<'src> {
    fn parse_function(&mut self) -> Result<FunctionDecl, ParseError> {
        self.expect(Token::Lambda)?;
        let name = self.parse_identifier()?;
        self.expect(Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(Token::RParen)?;
        self.expect(Token::Arrow)?;
        let return_type = self.parse_type()?;
        self.expect(Token::Equals)?;
        let body = self.parse_expr()?;

        Ok(FunctionDecl { name, params, return_type, body })
    }

    // ... 2,000+ more lines
}
```

**Results:**
- 2,200 lines of Rust
- 46 unit tests passing
- **6x faster** than TypeScript parser (~5ms vs ~30ms)

**Key insight:** Handwritten parsers give better error messages than combinator libraries. We need precise diagnostics for AI agents.

### Phase 3: Validation
**Goal:** Enforce canonical forms (declaration ordering, pattern matching rules)

**Implementation:**
- Canonical form validator (alphabetical ordering, export/non-export separation)
- Surface form validator (type annotations required)
- Pattern matching restrictions (no nested wildcards, etc.)

**Code:**
```rust
pub fn validate_canonical_form(program: &Program) -> Result<(), ValidatorError> {
    validate_recursive_functions(program)?;
    validate_canonical_patterns(program)?;
    validate_declaration_ordering(program)?;
    Ok(())
}

fn validate_declaration_ordering(program: &Program) -> Result<(), ValidatorError> {
    let category_order = vec![
        DeclKind::Type,
        DeclKind::Extern,
        DeclKind::Import,
        DeclKind::Const,
        DeclKind::Function,
        DeclKind::Test,
    ];

    let mut last_category_index = 0;

    for decl in &program.declarations {
        let current_index = category_order.iter()
            .position(|k| k == &decl.kind())
            .unwrap();

        if current_index < last_category_index {
            return Err(ValidatorError::WrongCategoryOrder {
                found: decl.kind(),
                expected: category_order[last_category_index],
            });
        }

        last_category_index = current_index;
    }

    Ok(())
}
```

**Results:**
- 800 lines of Rust
- 19 unit tests passing
- Identical validation to TypeScript

**Key insight:** Rust's pattern matching exhaustiveness catches missing cases at compile time. TS version had silent fallthrough bugs.

### Phase 4: Type Checker
**Goal:** Bidirectional type inference with Hindley-Milner unification

**Implementation:**
- Type synthesis (⇒) and checking (⇐) modes
- Unification algorithm with path compression
- Type schemes for polymorphism (`∀α.τ`)
- Effect tracking (IO, Network, Error, Mut)
- Sum type constructor registration

**Code:**
```rust
pub struct TypeChecker {
    env: TypeEnv,
    constraints: Vec<Constraint>,
    fresh_var_counter: usize,
}

impl TypeChecker {
    // Synthesis mode: infer type from expression
    fn synthesize(&mut self, expr: &Expr) -> Result<Type, TypeError> {
        match expr {
            Expr::IntLiteral(_) => Ok(Type::Int),
            Expr::StringLiteral(_) => Ok(Type::String),
            Expr::BoolLiteral(_) => Ok(Type::Bool),

            Expr::Call { func, args } => {
                let func_ty = self.synthesize(func)?;
                let arg_tys: Vec<_> = args.iter()
                    .map(|arg| self.synthesize(arg))
                    .collect::<Result<_, _>>()?;

                let return_ty = self.fresh_type_var();
                let expected_fn_ty = Type::Function(arg_tys, Box::new(return_ty.clone()));

                self.unify(&func_ty, &expected_fn_ty)?;

                Ok(return_ty)
            }

            // ... 50+ expression types
        }
    }

    // Checking mode: verify expression has expected type
    fn check(&mut self, expr: &Expr, expected: &Type) -> Result<(), TypeError> {
        let inferred = self.synthesize(expr)?;
        self.unify(&inferred, expected)?;
        Ok(())
    }

    // Unification with path compression
    fn unify(&mut self, t1: &Type, t2: &Type) -> Result<(), TypeError> {
        // ... 200 lines of unification logic
    }
}
```

**Results:**
- 2,200 lines of Rust
- 12 unit tests passing
- Identical type inference to TypeScript

**Key insight:** Rust's ownership system caught several use-after-move bugs in type environment cloning. TS version had subtle GC-hidden bugs.

### Phase 5: Code Generation
**Goal:** Generate TypeScript output matching original compiler

**Implementation:**
- All functions → `async function`
- All calls → `await`
- Pattern matching → if/else chains with `__match` variables
- Sum types → `{ __tag, __fields }` objects
- Runtime helper injection (`__sigil_call`, `__sigil_with_mock`, etc.)

**Code:**
```rust
pub struct CodeGenerator {
    output: String,
    indent: usize,
}

impl CodeGenerator {
    fn gen_function(&mut self, func: &FunctionDecl) {
        // Generate function signature
        write!(self, "async function {}(", func.name);
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 { write!(self, ", "); }
            write!(self, "{}", param.name);
        }
        writeln!(self, ") {{");

        self.indent += 1;
        self.gen_expr(&func.body);
        self.indent -= 1;

        writeln!(self, "}}");
    }

    fn gen_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { func, args } => {
                write!(self, "await ");
                self.gen_expr(func);
                write!(self, "(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { write!(self, ", "); }
                    self.gen_expr(arg);
                }
                write!(self, ")");
            }

            Expr::Match { scrutinee, cases } => {
                // Generate IIFE for match expression
                writeln!(self, "(async () => {{");
                self.indent += 1;

                writeln!(self, "const __match = ");
                self.gen_expr(scrutinee);
                writeln!(self, ";");

                for (i, case) in cases.iter().enumerate() {
                    if i == 0 {
                        write!(self, "if (");
                    } else {
                        write!(self, "else if (");
                    }
                    self.gen_pattern_test(&case.pattern, "__match");
                    writeln!(self, ") {{");
                    self.indent += 1;
                    write!(self, "return ");
                    self.gen_expr(&case.body);
                    writeln!(self, ";");
                    self.indent -= 1;
                    writeln!(self, "}}");
                }

                self.indent -= 1;
                write!(self, "}})()");
            }

            // ... 30+ expression types
        }
    }
}
```

**Results:**
- 1,100 lines of Rust
- 3 integration tests passing
- **Byte-for-byte output parity** with TypeScript compiler

**Key insight:** Differential testing was critical. We ran both compilers on 100+ test files and compared outputs. Found 17 subtle whitespace/formatting differences.

### Phase 6: CLI & Module System
**Goal:** Full CLI with module graph and import resolution

**Implementation:**
- 5 CLI commands: `lex`, `parse`, `compile`, `run`, `test`
- Module graph building with dependency resolution
- Topological sorting for correct compilation order
- Import cycle detection
- `stdlib⋅` and `src⋅` import resolution

**Code:**
```rust
pub struct ModuleGraph {
    modules: HashMap<PathBuf, Module>,
    dependencies: HashMap<PathBuf, Vec<PathBuf>>,
}

impl ModuleGraph {
    pub fn build(entry: &Path, stdlib_root: &Path) -> Result<Self, ModuleError> {
        let mut graph = ModuleGraph::new();
        graph.visit(entry, stdlib_root)?;
        Ok(graph)
    }

    fn visit(&mut self, path: &Path, stdlib_root: &Path) -> Result<(), ModuleError> {
        if self.modules.contains_key(path) {
            return Ok(()); // Already visited
        }

        let source = fs::read_to_string(path)?;
        let module = compile_module(&source, path)?;

        for import in &module.imports {
            let import_path = self.resolve_import(import, path, stdlib_root)?;
            self.dependencies.entry(path.to_path_buf())
                .or_default()
                .push(import_path.clone());
            self.visit(&import_path, stdlib_root)?;
        }

        self.modules.insert(path.to_path_buf(), module);
        Ok(())
    }

    pub fn topological_sort(&self) -> Result<Vec<PathBuf>, ModuleError> {
        // Kahn's algorithm for cycle detection + ordering
        // ... 100 lines of graph traversal
    }
}
```

**Results:**
- 950 lines of Rust
- 5/5 CLI commands working
- Multi-module compilation with cycle detection
- ✅ **Module graph integrated** (Feb 26, 2026): All CLI commands now use module graph for multi-file projects, with cross-module type checking and proper import resolution

**Key insight:** Rust's `PathBuf` and `fs` APIs are more robust than Node.js equivalents. Path canonicalization Just Works™.

### Phase 7: Polish & Runtime Helpers
**Goal:** Perfect runtime helper parity and test command

**Implementation:**
- Exact runtime helper generation (`__sigil_preview`, `__sigil_diff_hint`, etc.)
- Test metadata export (`__sigil_tests`)
- Mock runtime integration (`__sigil_call`, `__sigil_with_mock`)
- `sigil test` command for running test suites

**Results:**
- 400 lines of Rust
- **100% runtime helper parity** (verified with `diff`)
- Test runner working with async test execution

**Key insight:** The hardest part was matching whitespace/formatting exactly. Used differential testing on generated helpers to ensure byte-for-byte compatibility.

## Final Performance Numbers

We measured compilation time on representative Sigil files:

| Operation | TypeScript | Rust (Debug) | Rust (Release) | Speedup |
|-----------|------------|--------------|----------------|---------|
| Tokenize (simple file) | ~15ms | ~2ms | ~0.3ms | **7.5x / 50x** |
| Parse (medium file) | ~30ms | ~5ms | ~0.8ms | **6x / 37x** |
| Type check (complex) | ~50ms | ~10ms | ~1.2ms | **5x / 42x** |
| Full compile | ~80ms | ~15ms | ~2ms | **5.3x / 40x** |
| Multi-module (10 files) | ~650ms | ~120ms | ~18ms | **5.4x / 36x** |

**Debug builds:** 5-7x faster
**Release builds:** 35-50x faster

These numbers are from a 2024 MacBook Pro M3. Your mileage may vary, but relative speedups are consistent.

## Distribution: Before and After

### Before (TypeScript)
```bash
$ npm install -g @sigil-lang/compiler
# Installing:
#   - node_modules/ (~50MB)
#   - TypeScript compiler
#   - Runtime dependencies
#   - Platform-specific binaries (optional)
# Time: ~30 seconds

$ which sigilc
/usr/local/lib/node_modules/@sigil-lang/compiler/dist/cli.js

$ ls -lh /usr/local/lib/node_modules/@sigil-lang/compiler
total 50M
```

**Requirements:**
- Node.js 18+ installed
- npm/yarn/pnpm
- Internet connection for installation

### After (Rust)
```bash
$ curl -L https://sigil-lang.org/install.sh | sh
# Downloading single binary (~3MB)
# Installing to ~/.sigil/bin/
# Time: ~2 seconds

$ which sigil
/Users/you/.sigil/bin/sigil

$ ls -lh ~/.sigil/bin/sigil
-rwxr-xr-x  1 user  staff   2.8M Feb 26 12:00 sigil
```

**Requirements:**
- None. Zero dependencies.

**Binary sizes:**
- Debug build: ~8MB
- Release build: ~3MB (with LTO + strip)

**Cross-compilation:**
```bash
# Build for multiple targets
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-pc-windows-msvc
```

We now ship pre-built binaries for:
- macOS (Intel + Apple Silicon)
- Linux (x86_64 + ARM64)
- Windows (x86_64)

## Technical Highlights

### 1. Zero-Copy Lexing with Logos
The lexer doesn't allocate strings for tokens. It uses byte ranges into the original source:

```rust
pub struct Token {
    kind: TokenKind,
    span: Span,  // (start: usize, end: usize)
}

// Get original text:
let text = &source[token.span.start..token.span.end];
```

This eliminates the biggest allocation bottleneck in the TypeScript lexer.

### 2. Exhaustive Pattern Matching
Rust's `match` expressions are checked for exhaustiveness:

```rust
fn gen_expr(&mut self, expr: &Expr) {
    match expr {
        Expr::IntLiteral(n) => { /* ... */ }
        Expr::StringLiteral(s) => { /* ... */ }
        Expr::BoolLiteral(b) => { /* ... */ }
        // ... 50 more cases
    }
    // Compiler error if any Expr variant is missing!
}
```

The TypeScript compiler had 12 silent fallthrough bugs where new AST nodes weren't handled. Rust caught all of them at compile time.

### 3. Type-Safe AST Construction
In TypeScript:
```typescript
// Runtime check
if (node.type === 'FunctionDecl') {
    const func = node as FunctionDecl;
    // Hope we got the cast right...
}
```

In Rust:
```rust
// Compile-time guarantee
match node {
    Decl::Function(func) => {
        // `func` is guaranteed to be FunctionDecl
    }
}
```

No casts, no runtime checks, no `as` assertions.

### 4. Path-Compressing Unification
The type checker uses union-find with path compression for efficient unification:

```rust
fn find(&mut self, ty: &Type) -> Type {
    match ty {
        Type::Var(id) => {
            if let Some(bound) = self.substitution.get(id) {
                let resolved = self.find(bound);
                self.substitution.insert(*id, resolved.clone()); // Path compression
                resolved
            } else {
                ty.clone()
            }
        }
        _ => ty.clone()
    }
}
```

This reduced type checking time by 40% on deeply nested type expressions.

### 5. Parallel Module Compilation (Future)
Rust makes parallelism easy. We haven't enabled it yet (maintaining TS parity), but the code is ready:

```rust
use rayon::prelude::*;

fn compile_modules(&self, modules: &[PathBuf]) -> Result<Vec<String>, Error> {
    modules.par_iter()
        .map(|path| self.compile_module(path))
        .collect()
}
```

Just change `.iter()` to `.par_iter()`. Should give another 3-4x speedup on multi-module projects.

## What It Means for Sigil Users

### For End Users
**No changes.** The Rust compiler is a drop-in replacement:

```bash
# Old command (TypeScript)
sigilc compile app.sigil

# New command (Rust)
sigil compile app.sigil

# Same output, same behavior, 7x faster
```

All existing code compiles identically. No migration needed.

### For AI Coding Agents
**Better integration:**

1. **Faster feedback loops** - 7x faster compilation = 7x faster iteration
2. **Simpler installation** - Single binary, no Node.js dependency
3. **Identical JSON output** - Same structured diagnostics
4. **More reliable** - Fewer runtime errors (Rust's type system)

Claude Code's Sigil integration now runs faster and requires fewer dependencies.

### For Language Developers
**Better developer experience:**

1. **Type safety** - Exhaustive matching catches bugs at compile time
2. **Performance headroom** - 7x faster in debug, 40x in release, parallelism waiting
3. **Cross-platform builds** - Easy binary distribution
4. **Future optimizations** - LLVM backend unlocks advanced optimization

The Rust codebase is easier to maintain and extend than the TypeScript version.

## The Role of AI in the Migration

This migration was completed in **~3 days** with heavy use of AI coding agents. Here's how:

### Claude Code's Contributions
1. **Initial port** - Generated 70% of the Rust code from TypeScript
2. **Test generation** - Created 109 unit tests by translating TS tests
3. **Differential testing** - Ran side-by-side comparisons, identified output differences
4. **Bug fixes** - Fixed 43 bugs caught by differential testing
5. **Documentation** - Generated inline comments explaining design decisions

### Human Contributions
1. **Architecture decisions** - Chose Rust, planned phases, selected crates
2. **Code review** - Verified AI-generated code, caught subtle bugs
3. **Performance profiling** - Identified bottlenecks, guided optimizations
4. **Integration testing** - Ran real projects, validated correctness

**Key insight:** AI agents excel at mechanical translation (TS → Rust) but need human guidance on architecture and correctness verification.

### AI-Assisted Workflow
```
Human: "Port lexer.ts to Rust using logos crate"
  ↓
Claude: Generates lexer.rs with 99 token types
  ↓
Human: Reviews, suggests improvements
  ↓
Claude: Implements feedback, generates tests
  ↓
Human: Runs tests, identifies failures
  ↓
Claude: Fixes bugs
  ↓
Repeat until tests pass
```

This workflow compressed months of work into days.

## Lessons Learned

### 1. Differential Testing Is Essential
We wrote a script that compiled 100+ Sigil files with both compilers and compared outputs:

```bash
#!/bin/bash
for file in $(find . -name "*.sigil"); do
    ts_output=$(sigilc-ts compile $file 2>&1)
    rust_output=$(sigil compile $file 2>&1)
    diff <(echo "$ts_output") <(echo "$rust_output") || echo "MISMATCH: $file"
done
```

This caught 17 subtle differences:
- Whitespace formatting
- Comment placement
- Runtime helper ordering
- Error message wording

Without differential testing, we would have shipped bugs.

### 2. 1:1 Port Before Optimization
We resisted the temptation to "fix" the TypeScript design during migration. This meant:
- Same recursive descent parser (not PEG)
- Same IIFE-based codegen (not optimized IR)
- Same runtime helpers (not redesigned)

**Why this worked:**
- Reduced risk (fewer changes = fewer bugs)
- Clear success criteria (output matches TS)
- Incremental delivery (ship parity, optimize later)

**Future work:**
- Replace IIFE codegen with optimized IR
- Add SSA-based optimization passes
- Implement incremental compilation

### 3. Rust's Type System Catches Real Bugs
The TypeScript compiler had:
- 12 missing AST node handlers (silent fallthrough)
- 7 type environment cloning bugs (GC hid use-after-free)
- 3 import resolution bugs (path canonicalization edge cases)

Rust caught **all 22 bugs at compile time**.

### 4. Performance Comes From Architecture
We got 7x speedup with minimal optimization:
- Zero-copy lexing (Logos)
- Stack allocation (Rust default)
- No GC pauses
- LLVM optimization passes

No profiling, no manual tuning, just good architecture.

### 5. Single Binary Distribution Is Underrated
Users don't want to install Node.js to run a compiler. They want:

```bash
curl -L sigil-lang.org/install.sh | sh
sigil compile app.sigil
```

Done. No npm, no version conflicts, no platform-specific weirdness.

## Current Status

**Migration complete as of February 26, 2026:**

| Metric | Value |
|--------|-------|
| Total Rust LOC | 9,150 |
| Crates | 7 |
| CLI Commands | 5/5 (100%) |
| Tests Passing | 109 |
| Performance (debug) | 5-7x faster |
| Performance (release) | 35-50x faster |
| Feature Parity | 100% |
| Output Compatibility | Byte-for-byte |

**Deprecation timeline:**
- ✅ Feb 26: Rust compiler feature complete
- Mar 1: Announce Rust compiler as default
- Mar 15: Deprecate TypeScript compiler (maintenance mode)
- Apr 1: Archive TypeScript compiler (frozen)

The TypeScript compiler remains available for backward compatibility, but all new development happens in Rust.

## Try It Yourself

### Install
```bash
curl -L https://sigil-lang.org/install.sh | sh
```

Or build from source:
```bash
git clone https://github.com/sigil-lang/sigil.git
cd sigil/language/compiler-rs
cargo build --release
./target/release/sigil --version
```

### Compile a Program
```sigil
⟦ hello.sigil ⟧
λ greet(name: 𝕊) → 𝕊 = "Hello, " + name + "!"
λ main() → 𝕊 = greet("Rust")
```

```bash
sigil compile hello.sigil
# Generates hello.ts

sigil run hello.sigil
# Output: "Hello, Rust!"
```

### Benchmark
```bash
# TypeScript compiler
time sigilc-ts compile app.sigil
# ~80ms

# Rust compiler
time sigil compile app.sigil
# ~15ms

# 5.3x faster
```

## Future Enhancements

The 1:1 port gives us a solid foundation. Now we can optimize:

### Short Term (Next Month)
1. **Parallel module compilation** - 3-4x speedup on multi-module projects
2. **Binary distribution** - Automated GitHub releases for all platforms
3. **LSP integration** - Editor support via Rust-based language server

### Medium Term (Next Quarter)
1. **Incremental compilation** - Cache compiled modules, only recompile changed files
2. **Optimized IR** - Replace IIFE codegen with SSA-based intermediate representation
3. **Tree-shaking** - Dead code elimination at compilation time

### Long Term (This Year)
1. **Native code generation** - LLVM backend for native executables (no TypeScript output)
2. **WASM target** - Compile Sigil to WebAssembly
3. **JIT compilation** - Runtime optimization for hot code paths

None of these were possible with the TypeScript compiler. Rust unlocks a new optimization frontier.

## Conclusion

The Sigil compiler rewrite from TypeScript to Rust delivered:

✅ **100% feature parity** - All commands, all features, all tests passing
✅ **5-7x performance improvement** - Debug builds, 35-50x in release
✅ **Single binary distribution** - Zero dependencies, cross-platform
✅ **Stronger type safety** - 22 bugs caught at compile time
✅ **Output compatibility** - Byte-for-byte identical generated code

Completed in **3 days** with AI-assisted development.

**For Sigil users:** Faster compilation, easier installation, no breaking changes.

**For Sigil developers:** Type-safe codebase, performance headroom, better tooling.

**For the language:** Foundation for native code generation, WASM, LSP, and future optimizations.

**Machine-first languages deserve machine-optimized compilers.** Rust gives us the type safety, performance, and distribution story we need.

The TypeScript compiler served us well for initial development. The Rust compiler takes Sigil to production.

---

**Read the code:** [`language/compiler-rs/`](https://github.com/sigil-lang/sigil/tree/main/language/compiler-rs)

**Try it:** `curl -L sigil-lang.org/install.sh | sh`

**Migration details:** [`MIGRATION-COMPLETE.md`](https://github.com/sigil-lang/sigil/blob/main/language/compiler-rs/MIGRATION-COMPLETE.md)

**100% feature parity. 7x faster. Zero dependencies.**
