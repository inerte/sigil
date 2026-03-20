# Sigil

A programming language for Claude Code and other AI coding agents.

## Getting Started

Here's how to write "Hello World" in Sigil (for now at least, until models learn more about Sigil):

```bash
claude "Write a Hello World in Sigil and run it"
```

<div style="text-align: center;">
<img src="assets/hello-world.png" alt="Hello World in Sigil" style="max-width: 100%; height: auto; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1); display: inline-block;">
</div>

## Why Sigil?

**<a href="/docs/philosophy/#radical-canonicalization">Radical Canonicalization</a>** - ONE way to write everything. Not one preferred style: one accepted textual representation per program. The compiler owns the canonical printer internally and rejects any parseable source that does not match it.

**<a href="/articles/canonical-list-processing/">Canonical Programming Paths</a>** - Sigil canonicalizes common programming patterns, not just syntax. Projection, filtering, reduction, search, and reversal flow through one blessed surface so humans and AI agents converge on the same code shapes.

**<a href="/docs/philosophy/#zero-ambiguity">Zero Ambiguity</a>** - Explicit bidirectional types, no shadowing, deterministic execution. Code means exactly what it says. Perfect for AI code generation.

**<a href="/docs/async/#named-concurrent-regions">Async Without Await</a>** - Built for async I/O without `await` syntax. Wider concurrency is explicit through named concurrent regions.

**<a href="/docs/testing/#first-class-tests">Testing Built In</a>** - Sigil treats tests as first-class code, with language-level test declarations, JSON-readable results, and runtime-aware validation for things like topology-backed environments.

**<a href="/docs/topology/#topology-is-runtime-truth">Topology as Runtime Truth</a>** - External dependencies (such as HTTP and database calls) are declared once, bound per environment, and used through typed handles instead of raw endpoints. Runtime wiring becomes compiler-validated project structure.

## Quick Example

```sigil program
i stdlib::list

λmain()=>Bool=stdlib::list.sum([1,2,3,4,5])=15
```

---

This website is built from markdown using a Static Site Generator built in Sigil. Docs stay in `language/docs`, specs in `language/spec`. <a href="about-site/">Learn more about the site =></a>
