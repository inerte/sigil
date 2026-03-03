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

**February 26, 2026** - [Rewriting the Sigil Compiler in Rust: 100% Feature Parity, 5-7x Faster](./009-rust-compiler-rewrite.md)

How we migrated the entire Sigil compiler from TypeScript to Rust in 3 days, achieved byte-for-byte output compatibility, 5-7x performance improvement, and single-binary distribution with zero dependencies.

**Key topics:** Compiler rewrite, Rust migration, performance optimization, differential testing, AI-assisted development, type safety, distribution

**March 2, 2026** - [Canonical Type Equality: Why Sigil Normalizes Structural Types Everywhere](./011-canonical-type-equality.md)

Why Sigil compares aliases and named product types by their canonical normalized form everywhere in the checker, and why this matters more for Claude Code and Codex than for human readability.

**Key topics:** Canonical semantics, structural typing, named product types, AI-first language design, typechecker determinism

**March 2, 2026** - [Why Sigil Uses true and false](./012-why-sigil-uses-true-and-false.md)

Why Sigil replaced `⊤` and `⊥` with `true` and `false`, and why token efficiency plus model-prior alignment matters more than mathematical elegance in an AI-first language.

**Key topics:** Boolean literals, token efficiency, Claude Code, Codex, canonical syntax, AI-first language design

**March 2, 2026** - [Why Sigil Uses match](./013-why-sigil-uses-match.md)

Why Sigil replaced `≡` with `match`, and why common pattern-matching keywords are a better fit than symbolic elegance for AI-generated code.

**Key topics:** Pattern matching, token efficiency, Claude Code, Codex, canonical syntax, AI-first language design

**March 2, 2026** - [Why Sigil Uses and and or](./014-why-sigil-uses-and-and-or.md)

Why Sigil replaced `∧` and `∨` with `and` and `or`, and why common boolean operators are a better fit than symbolic logic glyphs for AI-generated code.

**Key topics:** Boolean operators, token efficiency, Claude Code, Codex, canonical syntax, AI-first language design

**March 3, 2026** - [Why Sigil Bans Shadowing](./015-why-sigil-bans-shadowing.md)

Why Sigil rejects local shadowing so one name keeps one meaning, and why that matters both for refactoring safety and for AI-generated code.

**Key topics:** Canonical bindings, shadowing, refactoring safety, Claude Code, Codex, AI-first language design

---

## About These Articles

These articles document real design decisions made during Sigil's development. They serve as:

- **Design rationale** - Why we made specific choices
- **Educational material** - Teaching machine-first language design
- **Historical record** - Evolution of the language
- **Philosophy guide** - Core principles applied in practice

Each article is written at the time of the design decision, capturing the reasoning and trade-offs in real-time.
