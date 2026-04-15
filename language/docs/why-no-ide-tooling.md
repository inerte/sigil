# Why Sigil Removed Human IDE Tooling

**February 25, 2026**

Yesterday we deleted 8,300 lines of code from Sigil. We removed the LSP server, the VS Code extension, semantic map generation, and all human-focused IDE tooling infrastructure. We removed 104 files and 292 npm packages.

This wasn't technical debt cleanup. This was a fundamental rethinking of how programming languages should be built in the AI era.

## The Old World: Human-First Tooling

For decades, the workflow was clear:

1. **Human writes code** in an IDE with syntax highlighting
2. **Human reads code** with hover tooltips, go-to-definition, semantic overlays
3. **IDE helps human** understand code through pre-generated documentation

We built Sigil's first generation of tooling around this assumption:

- **Semantic maps** (`.sigil.map` files): AI-generated explanations alongside every function, stored as JSON
- **LSP server**: Provided hover information, completions, diagnostics to VS Code
- **VS Code extension**: Syntax highlighting, semantic overlays showing the `.sigil.map` content
- **Integration layer**: Complex machinery to keep semantic maps in sync with source

We had 69 `.sigil.map` files across the codebase. The LSP server was 844 lines. The VS Code extension was 803 lines. The semantic map generator was 374 lines.

Here's what the workflow looked like:

```
Developer opens fibonacci.sigil in VS Code
  ↓
VS Code syntax highlights: λfibonacci(n:Int)=>Int match n{0=>0|1=>1|...}
  ↓
Developer hovers over function
  ↓
LSP reads fibonacci.sigil.map
  ↓
Tooltip shows: "Computes the nth Fibonacci number using pattern matching..."
```

This seemed reasonable. Standard practice, even. Every modern language has an LSP server.

## The Realization: Nobody Uses It Anymore

Then we started actually developing with Sigil. And we noticed something:

**We never used the IDE tooling. We asked Claude Code instead.**

Real workflow:

```
Developer: "Claude, what does this fibonacci function do?"
Claude Code: *reads fibonacci.sigil directly*
Claude Code: "This is a recursive Fibonacci implementation that uses
pattern matching. For n=0 it returns 0, for n=1 it returns 1,
otherwise it recursively computes fib(n-1)+fib(n-2)..."
```

The semantic maps were obsolete before we finished generating them. The LSP hover tooltips? Never clicked. The VS Code extension? Just syntax highlighting (which we barely needed since Claude Code handles the code).

## The Paradigm Shift

The traditional mental model:

```
Code is written for humans to read
  ↓
Invest heavily in readability tooling
  ↓
LSP, semantic overlays, documentation generators
```

The actual 2026 workflow:

```
Code is written by AI (Claude Code)
  ↓
Humans ask AI to explain code when needed
  ↓
Compiler provides diagnostics, AI interprets
```

When you think about it, this makes perfect sense:

- **Claude Code already knows how to read source files** - it doesn't need `.sigil.map` files
- **Claude Code invokes the compiler directly** - `cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile`
- **Claude Code provides better explanations** than static documentation - contextualized, interactive, always current

The semantic maps were documentation frozen at compile time. Claude Code's explanations are generated on-demand from live source code.

That still leaves one real requirement for a brand-new language: the assistant
may not know the language surface yet. Sigil now solves that separately with
`sigil docs ...`, which ships an embedded local corpus of guides, specs,
articles, and grammar inside the binary itself. Source reading explains one
program; embedded docs bootstrap the language.

Which would you rather have?

## What We Kept: The Essentials

We didn't remove tooling indiscriminately. We kept everything Claude Code actually uses:

### 1. Strong Compiler with Excellent Diagnostics

The compiler CLI provides the machine surfaces Sigil actually relies on:

- Detailed parse errors with location information
- Type checking with bidirectional inference
- Canonical form validation
- Clear, actionable error messages

Example error:

```
Error: Expected a Sigil root for module reference
Found: stdlib/list.length(xs)
Expected: §list.length(xs)
```

Claude Code reads these diagnostics and explains them to humans in natural language.

### 2. Canonical Syntax Enforcement

Sigil enforces one way to write anything:

```sigil invalid-module
λfibonacci(n:Int)=>Int match n{0=>0|1=>1|n=>fibonacci(n-1)+fibonacci(n-2)}
```

Not:
- `function fibonacci(n: Int): Int = ...` (wrong keywords)
- `λ fibonacci(n:Int) => Int = ...` (wrong spacing)
- `def fib(n) { ... }` (missing types)

There's only one valid token sequence. No ambiguity. No style debates. Claude Code generates it correctly every time.

### 3. First-Class Testing Framework

The compiler has built-in test discovery and execution:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests
```

Claude Code uses this to verify correctness after generating code.

### 4. Bidirectional Type Checking

Strong type inference with mandatory annotations:

```text
⟦ Type error: Cannot pass String where Int expected ⟧
λadd(a:Int,b:Int)=>Int=a+b
test "add strings"=add("1","2") match 3  ⟦ Error here ⟧
```

The type checker catches mistakes before runtime. Claude Code uses type errors to self-correct.

## The Benefits: Simpler is Better

Removing 8,300 lines of code wasn't just about reducing maintenance burden (though that's nice). It was about **architectural simplicity**.

### Before: Complex Machinery

```
Source (.sigil)
  ↓
Compiler generates semantic maps (.sigil.map)
  ↓
LSP server loads source + maps
  ↓
VS Code extension formats for display
  ↓
Human reads hover tooltip
```

Four layers of infrastructure. Files must stay in sync. Build system must regenerate maps. LSP must watch for changes.

### After: Direct Integration

```
Source (.sigil)
  ↓
Claude Code invokes compiler CLI
  ↓
Claude Code explains to human
```

Two layers. No generated artifacts. No synchronization. No stale documentation.

### Concrete Wins

**Removed infrastructure:**
- 17 LSP server files (844 lines)
- 17 VS Code extension files (803 lines)
- 6 semantic map generator files (374 lines)
- 69 generated `.sigil.map` files (6,279 lines of JSON)
- 292 npm packages from workspace dependencies

**Kept infrastructure:**
- Compiler CLI (the only tool Claude Code needs)
- Test framework (for verification)
- Type checker (for correctness)

**Developer experience improvement:**
- Faster: No LSP startup time, no map generation
- Simpler: Fewer concepts to understand
- More accurate: Claude Code's explanations are always current
- Better: Interactive Q&A beats static tooltips

## The AI-First Development Workflow

Here's what actually happens when you develop with Sigil in 2026:

### Writing Code

```
Developer: "Create a function to validate email addresses"

Claude Code: *generates dense canonical Sigil*
λvalidate_email(email:String)=>Bool=
  match email{
    ""=>false
    |s=>contains(s,"@") and contains(s,".")
  }

Claude Code: "I've created validate_email that returns true if
the email contains both @ and . characters, false otherwise."
```

The developer never writes `λvalidate_email(email:String)=>Bool=...` by hand. They describe intent, Claude Code generates canonical syntax.

### Understanding Code

```
Developer: "What does fibonacci.sigil do?"

Claude Code: *reads fibonacci.sigil*
λfibonacci(n:Int)=>Int match n{0=>0|1=>1|n=>fibonacci(n-1)+fibonacci(n-2)}

Claude Code: "This is a recursive Fibonacci implementation:
- Base case: fib(0)=0, fib(1)=1
- Recursive case: fib(n)=fib(n-1)+fib(n-2)
- Uses pattern matching for the three cases
- Returns integer (Int) given integer input"
```

No hovering over symbols. No clicking through documentation. Just ask.

### Debugging Errors

```
Developer: "Why is the compiler complaining?"

Claude Code: *reads compiler output*
Error: Type mismatch in function call
Expected: Int
Found: String
At: fibonacci("5")

Claude Code: "You're passing a string '5' to fibonacci, which
expects an integer. Change to: fibonacci(5)"
```

Claude Code interprets compiler diagnostics better than any LSP hover could.

### Modifying Code

```
Developer: "Add memoization to fibonacci"

Claude Code: *edits fibonacci.sigil with canonical syntax*
c memo={...}
λfibonacci(n:Int)=>Int=
  match memo[n]{
    Some(cached)=>cached
    |None=>...
  }

Claude Code: "I've added a memo cache that stores computed values..."
```

The developer reviews the explanation, not the dense syntax.

## Lessons for Other Languages

If you're designing a programming language in 2026 and your primary audience is "developers using AI assistants," consider:

### 1. Maybe You Don't Need an LSP Server

Traditional LSP features:
- **Hover information**: AI explains better, contextually
- **Go to definition**: AI shows you the code and explains it
- **Auto-completion**: AI generates complete functions, not fragments
- **Diagnostics**: Compiler CLI provides these, AI interprets

What's left? Syntax highlighting, which is nice but not essential when AI mediates the interaction.

### 2. Maybe You Don't Need IDE Extensions

VS Code extensions provide:
- Syntax highlighting
- Semantic overlays
- Inline documentation

But if developers rarely read raw code directly (they ask AI), why invest in visual tooling?

### 3. Focus on Compiler Quality, Not IDE Integration

Where to invest engineering effort:

**High value:**
- Clear, actionable error messages
- Fast compilation
- Excellent type checking
- Deterministic behavior
- Comprehensive diagnostics

**Lower value:**
- LSP protocol compliance
- IDE extension APIs
- Syntax highlighting themes
- Tooltip formatting

Let the AI tools handle the "human interface" layer.

### 4. Documentation Should Be On-Demand

Static documentation (`.sigil.map` files, doc comments, hover tooltips):
- **Stale**: Out of sync with code changes
- **Fixed**: Can't adapt to user's specific question
- **Limited**: Can't provide context from surrounding code

AI-generated explanations:
- **Fresh**: Generated from current source
- **Contextual**: Answers the specific question asked
- **Comprehensive**: Can explain relationships between modules

## The Counterarguments

### "But humans need to read code sometimes!"

Yes, and they can. Sigil source is valid UTF-8 text. You can read it in any editor.

The point is: when you want to **understand** code, asking Claude Code is more effective than staring at dense syntax. Just like you don't read minified JavaScript directly - you look at the source map or ask a tool to explain it.

### "What if Claude Code is unavailable?"

Then you read the source code directly, just like you'd read any programming language. Sigil syntax is documented in `language/docs/syntax-reference.md`.

But realistically, if Claude Code is unavailable, you have bigger problems than reading Sigil code.

### "This only works because Sigil is small!"

Actually, this works **better** as codebases grow:

- **Large codebases**: More code fits in LLM context (the current published corpus shows Sigil using 21.1% fewer tokens than TypeScript)
- **Complex code**: AI explanations scale better than human documentation
- **Maintenance**: No stale documentation to keep in sync

### "Not everyone has access to Claude Code"

True, but:
1. This is the direction the industry is moving (GitHub Copilot, Cursor, etc.)
2. Any AI assistant can use the compiler CLI - it's not Claude-specific
3. Any AI assistant can also query `sigil docs ...` locally instead of hoping the web has already indexed the right Sigil docs
4. Traditional workflows still work (you can read/write Sigil by hand)

The point is: **optimize for the common case** (AI-assisted development), not the edge case (hand-authoring without AI).

## The Controversial Conclusion

Here's the uncomfortable truth: **most IDE tooling is now obsolete**.

Not because IDEs are bad. Because AI assistants have fundamentally changed how developers interact with code.

When humans:
- Ask AI to generate code (not write it manually)
- Ask AI to explain code (not read it directly)
- Ask AI to debug code (not interpret errors manually)

Then IDE tooling becomes middleware that adds complexity without adding value.

The new stack is simpler:

```
Source code
  ↓
Compiler (excellent diagnostics)
  ↓
AI assistant (interprets, explains, generates)
  ↓
Human (reviews, approves, directs)
```

Sigil embraces this. We removed the IDE layer entirely. We focus on:
1. **Compiler quality**: Clear errors, fast builds, strong types
2. **Canonical syntax**: Zero ambiguity for AI generation
3. **AI integration**: Compiler CLI designed for tool usage

And we let Claude Code handle the human interface.

## The Results

Since removing IDE tooling:

- **Faster development**: No LSP startup delays, no map regeneration
- **Simpler architecture**: 104 fewer files to maintain
- **Better explanations**: Claude Code beats static tooltips
- **Lighter dependencies**: 292 fewer npm packages
- **Clearer focus**: Invest in compiler, not IDE integration

We haven't lost functionality. We've gained simplicity.

The workflow is better:
- Developer describes intent => Claude Code generates canonical Sigil
- Developer asks questions => Claude Code explains from source
- Compiler provides diagnostics => Claude Code interprets
- Tests verify correctness => Claude Code shows results

No LSP required. No VS Code extension required. No semantic maps required.

Just source code, a strong compiler, and an AI assistant.

## Implications

If this approach works (and early evidence says it does), what does it mean for:

### Programming Language Design

Maybe the priorities should be:
1. **Canonical syntax** (one way to write anything - AI generates correctly)
2. **Excellent diagnostics** (AI interprets compiler output)
3. **Deterministic behavior** (AI can predict results)
4. **Token efficiency** (more code fits in context)

And not:
1. ~~Human readability~~ (AI explains)
2. ~~Flexible syntax~~ (creates ambiguity)
3. ~~IDE integration~~ (AI is the interface)

### Developer Tools

Maybe we need:
1. **Better compilers** (faster, clearer errors)
2. **Better AI integration** (structured output, tool use)
3. **Better testing frameworks** (AI-verifiable correctness)

And less:
1. ~~LSP servers~~ (AI doesn't need hover tooltips)
2. ~~IDE extensions~~ (AI doesn't need syntax highlighting)
3. ~~Documentation generators~~ (AI generates fresh explanations)

### Development Practices

Maybe the workflow should be:
1. **Describe intent** in natural language
2. **AI generates** canonical code
3. **Compiler validates** correctness
4. **AI explains** what was generated
5. **Human approves** or requests changes

Not:
1. ~~Human writes code~~ (slower, more errors)
2. ~~IDE assists~~ (autocomplete fragments)
3. ~~Human reads code~~ (dense syntax)
4. ~~Documentation explains~~ (often stale)

## The Experiment Continues

We're not claiming this is the final answer. We're claiming it's worth exploring.

Sigil is a laboratory for AI-first language design. Removing IDE tooling is an experiment:

- **Hypothesis**: AI assistants provide better code understanding than static IDE tooling
- **Method**: Remove LSP/maps/extensions, rely on Claude Code + compiler CLI
- **Metrics**: Developer productivity, code quality, onboarding speed
- **Timeline**: 6 months of real-world usage

If it fails, we'll rebuild the IDE tooling. If it succeeds, we'll have proven that the traditional language tooling stack needs rethinking.

Early results are promising. Development is faster, explanations are better, architecture is simpler.

But the real test is: can a team of developers build production systems this way?

We're building Sigil with Sigil to find out.

## Try It Yourself

The Sigil compiler is open source. No LSP server. No VS Code extension. Just:

```bash
# Compile Sigil code
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile fibonacci.sigil

# Run Sigil code
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run fibonacci.sigil

# Run tests
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests
```

Use Claude Code (or your AI assistant of choice) to:
- Generate Sigil code from natural language
- Explain existing Sigil code
- Debug compiler errors
- Refactor implementations

See if you miss the IDE tooling. We don't.

---

**Sigil** - Fresh code for AI
*Where Claude Code is the IDE*

---

## Appendix: What We Removed

Complete list of deletions across 5 commits:

### Commit 1: Remove semantic map generation from compiler
- Removed mapgen integration from CLI (14 lines)
- Removed 'mapgen' from SigilPhase type
- Files: `cli.ts`, `diagnostics/types.ts`

### Commit 2: Delete semantic map generator source
- Deleted: compiler map generation code (6 files, 374 lines)
  - `enhance.ts` (78 lines)
  - `extractor.ts` (54 lines)
  - `generator.ts` (135 lines)
  - `index.ts` (50 lines)
  - `types.ts` (41 lines)
  - `writer.ts` (16 lines)

### Commit 3: Delete all generated semantic map files
- Deleted: 69 `.sigil.map` files (6,279 lines)
  - 15 example maps
  - 8 stdlib maps
  - 13 test fixture maps
  - 33 project maps

### Commit 4: Remove LSP server and VS Code extension
- Deleted: `tools/lsp/` (9 files, 844 lines)
  - LSP server implementation
  - Completion, diagnostics, hover, symbols
- Deleted: `tools/vscode-extension/` (8 files, 803 lines)
  - VS Code extension
  - TextMate grammar
  - Language configuration
- Also removed empty directories: `tools/ai-editor/`, `tools/cursor-extension/`, `tools/mapgen/`, `tools/mcp-server/`, `tools/repl/`

### Commit 5: Clean up workspace after removing tooling
- Removed package.json entries for deleted tools
- Removed 292 npm dependencies no longer needed
- Updated workspace configuration

**Total: 104 files, 8,300+ lines removed**
