# Sigil

A programming language for Claude Code and other AI coding agents.

## Getting Started

Here's how to write "Hello World" in Sigil:

<img src="assets/hello-world.png" alt="Hello World in Sigil" style="width: 100%; max-width: 1200px; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);">

## Why Sigil?

**Radical Canonicalization** - ONE way to write everything. No style debates, no formatting discussions. The compiler enforces canonical forms for filenames, declarations, parameters, even whitespace.

**Zero Ambiguity** - Explicit types, no shadowing, deterministic execution. Code means exactly what it says. Perfect for AI code generation.

**Concurrent by Default** - Built for async I/O without await syntax. Functions compose naturally with effects tracked in the type system.

## Quick Example

```sigil
i stdlib⋅list

λsum(numbers:[ℤ])→ℤ match numbers{
  []→0|
  [first,.rest]→first+sum(rest)
}

test sum_test()→Unit|AssertionFailure =
  assert sum([1,2,3,4,5])=15
```

---

This website is built from markdown that lives in the repo. Docs stay in `language/docs`, specs in `language/spec`. <a href="/about-site/">Learn more about the site →</a>
