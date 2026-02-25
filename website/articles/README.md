# Sigil Language Design Articles

Articles documenting the evolution and design decisions of the Sigil programming language.

## Published Articles

### 2026

**February 24, 2026** - [The `#` Operator: Why Sigil Has ONE Way to Get Length](./001-canonical-length-operator.md)

How the canonical forms philosophy shaped our decision to use a single `#` operator instead of type-specific functions, and why this matters for AI code generation and training data quality.

**Key topics:** Canonical forms, training data quality, bidirectional type checking, compiler intrinsics

**February 24, 2026** - [Pattern Guards: How Dog-Fooding Evolved Sigil](./003-pattern-guards-dog-fooding.md)

How building Sigil's website in Sigil exposed the need for pattern guards, and how a small, focused language feature unlocked cleaner state-machine code.

**Key topics:** Dog-fooding, pattern matching, guards, parser ergonomics, language evolution

**February 25, 2026** - [Stdlib Tests + Claude Hooks: Making Sigil Dog-Food Itself Continuously](./004-stdlib-tests-and-claude-hooks.md)

Why we moved stdlib “test” demos into a dedicated Sigil test project and automated stdlib behavior tests with Claude Code hooks after relevant edits.

**Key topics:** First-class tests, repo canonicality, Claude Code hooks, stdlib regression testing, AI-assisted workflows

---

## About These Articles

These articles document real design decisions made during Sigil's development. They serve as:

- **Design rationale** - Why we made specific choices
- **Educational material** - Teaching machine-first language design
- **Historical record** - Evolution of the language
- **Philosophy guide** - Core principles applied in practice

Each article is written at the time of the design decision, capturing the reasoning and trade-offs in real-time.
