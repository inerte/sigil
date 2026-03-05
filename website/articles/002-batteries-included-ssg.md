---
title: Batteries Included: Building Sigil's SSG with Fat Stdlib
date: 2026-02-24
author: Sigil Language Team
slug: 002-batteries-included-ssg
---

# Batteries Included: Building Sigil's SSG with Fat Stdlib

**The best way to prove your language works? Use it to build its own website.**

This article documents how we built Sigil's Static Site Generator (SSG) entirely with stdlib components - no npm packages, no `node_modules`, no dependency hell. Just Sigil, a fat stdlib, and ~1,200 lines of code.

## The Node.js Problem

Before we talk about Sigil's approach, let's acknowledge the elephant in the room: **npm's decision paralysis**.

Say you want to build a static site generator in Node.js:

```bash
# How to parse markdown?
npm install marked        # 847k/week
npm install markdown-it   # 2.1M/week
npm install remark        # 1.2M/week
npm install showdown      # 124k/week

# How to serve HTTP?
npm install express       # 39M/week
npm install fastify       # 1.8M/week
npm install koa           # 856k/week
npm install hapi          # 166k/week

# Now you have 847MB in node_modules
# And 15 versions of lodash
# And 3 critical security vulnerabilities
# And your build broke because a left-pad maintainer unpublished
```

For **AI code generation**, this is catastrophic:

1. **Decision fatigue** - Which library should Claude choose?
2. **Version conflicts** - Transitive dependencies break constantly
3. **Training data noise** - LLMs see 47 ways to parse markdown
4. **Prompt token waste** - Need to specify versions, imports, config

## Sigil's Philosophy: Fat Stdlib

Sigil takes inspiration from **Go, Python, and Deno** - languages that ship with comprehensive standard libraries:

```go
// Go - batteries included
import "net/http"  // HTTP server in stdlib
import "html/template"  // templating in stdlib

// Python - batteries included
import http.server  # HTTP server in stdlib
import markdown  # Markdown in stdlib (external, but canonical)

// Deno - batteries included
import { serve } from "https://deno.land/std/http/server.ts";
```

**Sigil's approach:**
- Ship markdown parser with the compiler
- Ship HTTP server wrapper with the compiler
- Ship file I/O, string ops, list ops with the compiler
- **ONE canonical way to do each thing**
- Zero npm dependencies

Result: deterministic imports for AI agents, no decision paralysis, and **everything works out of the box**.

## The SSG Architecture

Our SSG demonstrates how stdlib components compose:

### Component 1: `stdlib⋅markdown` (~600 lines)

**A pure Sigil markdown parser** - not FFI, not npm, just Sigil code.

```sigil
i stdlib⋅markdown

λmain()→!IO 𝕌={
  l md="# Hello\n\nThis is **bold** text.";
  l html=stdlib⋅markdown.parse(md);
  console.log(html)
}

⟦ Output: ⟧
⟦ <h1>Hello</h1>
   <p>This is <strong>bold</strong> text.</p> ⟧
```

**Why pure Sigil instead of FFI?**

We could have wrapped `marked` or `markdown-it` with one line of FFI. But building it in pure Sigil:
- **Dog-foods the language** - proves Sigil can handle real parsing
- **Showcases features** - pattern matching, recursion, string ops
- **Creates canonical implementation** - no "which markdown library?" question
- **Produces better training data** - LLMs see how to build parsers in Sigil

The implementation uses:
- `stdlib⋅string` for substring, split, trim, char_at
- `stdlib⋅string` for starts_with, ends_with, is_digit
- Recursive descent parsing with state machines
- Pattern matching on block types (Header, Paragraph, CodeBlock, etc.)

### Component 2: `stdlib⋅http_server` (~200 lines)

**Thin FFI wrapper around Node.js HTTP** - canonical interface.

```sigil
i stdlib⋅http_server

λhandle(req:Request)→!IO Response={
  stdlib⋅http_server.log_request(req);

  req.path="/" ?
    stdlib⋅http_server.ok("<h1>Welcome</h1>") :
    stdlib⋅http_server.not_found()
}

λmain()→!IO 𝕌={
  stdlib⋅http_server.serve(3000,handle)
}
```

**Why a wrapper instead of raw FFI?**

We wrap Node's `http` module to provide:
- **Type safety** - Sigil Request/Response types instead of Node objects
- **Canonical interface** - ONE way to create servers
- **Simplified API** - Helper functions (ok, not_found, log_request)
- **Future portability** - Could target Deno/Bun without changing user code

### Component 3: SSG Build Pipeline

**Orchestrates stdlib components** to build the site.

```sigil
i stdlib⋅io          ⟦ File I/O ⟧
i stdlib⋅markdown    ⟦ Markdown parsing ⟧
i stdlib⋅string  ⟦ String operations ⟧

λbuild(input_dir:𝕊,output_dir:𝕊)→!IO 𝕌={
  ⟦ 1. Read all .md files ⟧
  l files=list_markdown_files(input_dir);

  ⟦ 2. Parse frontmatter (title, date, author) ⟧
  l articles=files↦parse_article;

  ⟦ 3. Convert markdown to HTML ⟧
  l html_articles=articles↦(λ(a)→Article={
    a with {html=stdlib⋅markdown.parse(a.markdown)}
  });

  ⟦ 4. Apply HTML templates ⟧
  l pages=html_articles↦generate_page;

  ⟦ 5. Write HTML files ⟧
  pages↦write_page;

  ⟦ 6. Generate index ⟧
  write_index(html_articles)
}
```

**The power of composition:**
- Each stdlib module does ONE thing well
- Modules compose without impedance mismatch
- No glue code, no adapters, no wrappers
- Type-safe throughout (bidirectional type checking)

## The Dog-Fooding Moment

Here's the beautiful part: **you're reading this article on a site built with this SSG.**

The page you're viewing was:
1. Written in `website/articles/002-batteries-included-ssg.md`
2. Parsed by `stdlib⋅markdown.parse()`
3. Wrapped in HTML templates (string concatenation)
4. Written to `dist/` by file I/O
5. Served by `stdlib⋅http_server` during development

**Zero npm packages. Zero external dependencies. Just Sigil.**

## Comparison to Other Ecosystems

### JavaScript/Node.js ❌
```bash
npm install marked express
# 847MB node_modules
# 15 versions of lodash
# "which markdown library?" decision for AI
```

### Python ✅
```python
import http.server  # stdlib
import markdown     # External but canonical
# Reasonable, but markdown not in stdlib
```

### Go ✅
```go
import "net/http"          // stdlib
import "github.com/russross/blackfriday"  // External but canonical
// Good stdlib, but markdown not included
```

### Deno ✅
```typescript
import { serve } from "https://deno.land/std/http/server.ts";
import { marked } from "https://esm.sh/marked";
// Good stdlib, canonical imports, but URL imports add friction
```

### Sigil ✅✅
```sigil
i stdlib⋅http_server  ⟦ Ships with compiler ⟧
i stdlib⋅markdown     ⟦ Ships with compiler ⟧
⟦ Zero external dependencies, ONE way to do each thing ⟧
```

## Benefits for AI Code Generation

When Claude Code builds a static site generator in Sigil:

**No decisions required:**
- ONE way to parse markdown: `stdlib⋅markdown.parse()`
- ONE way to serve HTTP: `stdlib⋅http_server.serve()`
- ONE way to read files: `fs⋅promises.readFile()` (FFI)
- ONE way to write files: `fs⋅promises.writeFile()` (FFI)

**No version management:**
- No `package.json`
- No `npm install`
- No dependency resolution
- No security audits

**Clean training data:**
- Every Sigil SSG uses the same stdlib modules
- Every markdown parser import is identical
- No syntactic variation, no library churn
- LLMs learn ONE way, generate deterministically

**Faster generation:**
- No "let me think about which library" step
- No "let me check which version" step
- No "let me configure the library" step
- Just: import stdlib, call function, done

## What We Learned

Building Sigil's website in Sigil taught us:

### 1. Pure Sigil parsers are viable
Writing the markdown parser in pure Sigil (~600 lines) proved:
- Pattern matching is expressive for parsing
- Recursive functions handle nested structures well
- String intrinsics (`stdlib⋅string`) are fast enough
- No need for parser combinator libraries

### 2. Thin FFI wrappers beat raw FFI
The `stdlib⋅http_server` wrapper (~200 lines) provides:
- Type safety (Sigil types instead of Node objects)
- Simpler API (helper functions)
- Future portability (could retarget)
- Canonical interface (no "which wrapper?" question)

### 3. Composition beats frameworks
Rather than a monolithic "SSG framework", we have:
- Small, focused stdlib modules
- Composable functions
- Clear data flow
- Easy to understand, easy to modify

### 4. Dog-fooding validates design
Building real software exposed:
- Missing string operations (we added `char_at`, `index_of`)
- Awkward FFI patterns (we improved type conversions)
- Documentation gaps (we wrote this article!)

## The "Batteries Included" Principle

**What should go in stdlib?**

We use this rubric:

✅ **Include if:**
- Commonly needed (most projects use it)
- Has ONE obvious right way to implement
- Small enough to ship with compiler (<1000 lines)
- Benefits from canonical implementation
- Improves AI code generation (eliminates decisions)

❌ **Don't include if:**
- Niche use case
- Many valid implementations (subjective)
- Large/complex (bloats compiler)
- Requires external resources (databases, etc.)
- Better as ecosystem libraries

**Examples:**

| Feature | In Stdlib? | Rationale |
|---------|------------|-----------|
| Markdown parser | ✅ | Common, canonical, small, demo value |
| HTTP server | ✅ | Common, canonical wrapper, essential |
| String operations | ✅ | Fundamental, tiny, compiler intrinsics |
| JSON parser | ✅ | Universal, canonical, small |
| SQL client | ❌ | Large, many DBs, external resources |
| React framework | ❌ | Large, subjective, ecosystem library |
| OAuth library | ❌ | Many providers, large, niche patterns |

## Implementation Stats

**Stdlib modules (ship with compiler):**
- `stdlib⋅markdown`: 600 lines (pure Sigil)
- `stdlib⋅http_server`: 200 lines (FFI wrapper)
- `stdlib⋅string`: compiler intrinsics
- `stdlib⋅string`: compiler intrinsics

**SSG project (example usage):**
- `src/build.sigil`: 150 lines
- `src/server.sigil`: 100 lines
- `src/templates.sigil`: 100 lines
- `src/frontmatter.sigil`: 100 lines
- `src/types.sigil`: 50 lines

**Total: ~1,300 lines** for complete SSG with dev server.

Compare to typical Node.js SSG:
- `node_modules/`: 847MB
- Dependencies: 247 packages
- Code: ~500 lines (plus 247 libraries)
- Configuration: ~200 lines (webpack, babel, etc.)

## Try It Yourself

Clone the repo and run the SSG:

```bash
git clone https://github.com/sigil-lang/sigil.git
cd sigil/projects/ssg

# Build the site
node ../../language/compiler/dist/cli.js run src/build.sigil

# Start dev server
node ../../language/compiler/dist/cli.js run src/server.sigil

# Visit http://localhost:3000
```

You'll see:
- This article
- The `#` operator article
- All rendered with `stdlib⋅markdown`
- All served with `stdlib⋅http_server`
- Zero npm packages

## Future: More Batteries

We're considering adding to stdlib:

**Near-term candidates:**
- `stdlib⋅json` - JSON parse/stringify
- `stdlib⋅http_client` - Fetch wrapper (for API calls)
- `stdlib⋅testing` - Test framework (for stdlib itself)

**Longer-term candidates:**
- `stdlib⋅crypto` - Hashing, signing (thin wrapper)
- `stdlib⋅datetime` - Date/time operations
- `stdlib⋅path` - Path manipulation (thin wrapper)

Each addition is carefully considered through the "batteries included" rubric.

## Conclusion

Building Sigil's website with Sigil proved the "fat stdlib" approach works:

✅ **No npm dependencies** - Everything ships with compiler
✅ **No decision paralysis** - ONE way to do each thing
✅ **Clean training data** - Deterministic imports for AI
✅ **Dog-fooding success** - Sigil builds its own site
✅ **Composition works** - Small modules, clear interfaces

When you optimize for **machine-first code generation**, you need:
- Canonical forms (no syntactic variation)
- Fat stdlib (no library decisions)
- Deterministic behavior (no surprises)

The SSG is a small example, but it demonstrates a big principle: **eliminate choices that waste LLM context and training data**.

In 2026, when 93% of code is AI-generated, we should design languages for the 93%, not the 7%.

---

**Next article:** We'll explore how Sigil's bidirectional type checker eliminates runtime type errors while keeping syntax clean for AI generation.

**Feedback?** Open an issue at [github.com/sigil-lang/sigil](https://github.com/sigil-lang/sigil)
