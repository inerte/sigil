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

**Radical Canonicalization** - ONE way to write everything. Not one preferred style: one accepted textual representation per program. The compiler owns the canonical printer internally and rejects any parseable source that does not match it.

**Canonical Programming Paths** - Sigil canonicalizes common programming patterns, not just syntax. Projection, filtering, reduction, search, and reversal flow through one blessed surface so humans and AI agents converge on the same code shapes.

**Zero Ambiguity** - Explicit bidirectional types, no shadowing, deterministic execution. Code means exactly what it says. Perfect for AI code generation.

**Concurrent by Default** - Built for async I/O without await syntax. Functions compose naturally with effects tracked in the type system.

**Testing Built In** - Sigil treats tests as first-class code, with language-level test declarations, JSON-readable results, and runtime-aware validation for things like topology-backed environments.

**Topology as Runtime Truth** - External dependencies (such as HTTP and database calls) are declared once, bound per environment, and used through typed handles instead of raw endpoints. Runtime wiring becomes compiler-validated project structure.

## Quick Example

```sigil
i stdlib::list

λsum(numbers:[Int])=>Int match numbers{
  []=>0|
  [first,.rest]=>first+sum(rest)
}

test sum_test()=>Unit|AssertionFailure =
  assert sum([1,2,3,4,5])=15
```

---

This website is built from markdown using a Static Site Generator built in Sigil. Docs stay in `language/docs`, specs in `language/spec`. <a href="about-site/">Learn more about the site =></a>
