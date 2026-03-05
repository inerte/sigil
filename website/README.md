# Sigil

A programming language for Claude Code and other AI coding agents.

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

*This website is built from markdown that lives in the repo. Docs stay in `language/docs`, specs in `language/spec`. [Learn more](/about-site/).*
