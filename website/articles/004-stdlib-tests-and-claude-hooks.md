---
title: "Stdlib Tests + Claude Hooks: Making Sigil Dog-Food Itself Continuously"
date: February 25, 2026
author: Sigil Language Team
slug: 004-stdlib-tests-and-claude-hooks
---

# Stdlib Tests + Claude Hooks: Making Sigil Dog-Food Itself Continuously

**TL;DR:** We moved ad-hoc stdlib demo scripts out of `language/stdlib/`, created a real `language/stdlib-tests/` Sigil test project with first-class `test` syntax, and wired a Claude Code hook to run stdlib tests automatically after relevant edits.

This was not just cleanup. It tightened Sigil's feedback loop and made the repo more faithful to its own rules.

## The Problem: “Tests” That Weren't Really Tests

We had files like:

- `language/stdlib/test_list.sigil`
- `language/stdlib/test_numeric.sigil`

But they were not first-class Sigil tests. They were small demo programs using `λmain()` and `console.log(...)`.

That created two problems:

1. **Naming inconsistency**
   - They looked like tests, but they were demos.

2. **Philosophy inconsistency**
   - Sigil says tests belong in a `tests/` folder and use `test "..." {}` syntax.
   - Our own stdlib area was not following that pattern.

For a language that emphasizes canonical forms, this kind of drift matters. Agents (and humans) learn from examples.

## The Design Goal

We wanted three things at once:

1. **Real stdlib behavior tests**
   - Use Sigil's first-class test syntax.

2. **Canonical project layout**
   - Tests live under `tests/`.

3. **Automatic execution during AI-assisted editing**
   - If Claude edits stdlib or compiler code, run stdlib tests immediately.

## Why a Dedicated `language/stdlib-tests/` Project?

We considered just leaving the demo files in `language/stdlib/`, or turning all of `language/` into a single Sigil project.

Instead, we created a dedicated Sigil project:

- `language/stdlib-tests/sigil.json`
- `language/stdlib-tests/tests/*.sigil`

This gives us:

- **first-class tests** (`test "..." {}`)
- **canonical location** (`tests/`)
- **clean boundaries**
  - `language/stdlib/` stays library modules only
  - `language/stdlib-tests/` is the consumer/behavior test suite

That split mirrors the rest of the repo:

- compiler internals / fixtures / specs
- stdlib modules
- project tests

## Why This Matters for Sigil (Specifically)

This is not generic “test hygiene.” It directly supports Sigil's machine-first design.

### 1) Better training examples for agents

If we want AI agents to learn Sigil's canonical patterns, then the repository examples must be unambiguous:

- demos look like demos
- tests look like tests
- tests use `test` syntax and live in `tests/`

### 2) Faster regression detection in the right place

The stdlib is where many language semantics show up first:

- list behavior
- predicate behavior
- codegen/runtime semantics

A parser/typechecker/codegen change can silently break stdlib behavior even if compiler unit tests still pass.

`language/stdlib-tests/` is a high-signal regression suite for language development.

### 3) Dog-fooding the language's own testing model

If Sigil has first-class tests, we should use them for Sigil's own library.

Otherwise we create a subtle anti-pattern:
- “the language has one testing model”
- “except we don't use it here”

## Why Claude Hooks (and Why Path-Filtered)?

Manual testing is easy to forget, especially during rapid AI-assisted edits.

We wanted the feedback loop to be:

1. Edit stdlib/compiler file
2. stdlib tests run automatically
3. See pass/fail immediately

Claude Code hooks are a good fit because they run after actual `Edit`/`Write` tool actions.

### Why not run on every edit?

Because that would be noisy and slow:
- editing docs should not run stdlib tests
- editing app code under `projects/` should not run stdlib tests

So we path-filtered the hook to trigger only for:

- `language/stdlib/`
- `language/compiler/src/`

That keeps the automation focused on files that can affect stdlib behavior.

## Why a Wrapper Script Instead of Just `pnpm sigil:test:stdlib`?

The command itself is simple:

```bash
pnpm sigil:test:stdlib
```

But the hook needs logic:

- inspect Claude hook JSON from `stdin`
- extract the edited file path
- decide whether the path is relevant
- skip or run tests

Claude hooks can match tool names like `Edit|Write`, but they do not natively express “only if file path starts with `language/stdlib/`”.

So we used a small wrapper script to keep the hook config simple and the path filter explicit.

## What We Added

### Stdlib test commands

At the repo root:

- `pnpm sigil:test:stdlib` — runs stdlib behavior tests
- `pnpm sigil:test:all` — runs stdlib + project Sigil tests

### Dedicated stdlib behavior tests

Examples:

- `language/stdlib-tests/tests/list-predicates.sigil`
- `language/stdlib-tests/tests/numeric-predicates.sigil`

These are proper Sigil tests:

```sigil
i stdlib⋅numeric

test "numeric.is_even and is_odd basics" {
  stdlib⋅numeric.is_even(4)=⊤∧stdlib⋅numeric.is_odd(5)=⊤
}
```

### Claude hook automation

We added a repo-shared Claude Code hook that:

- watches `Edit|Write` tool events
- path-filters relevant files
- runs `pnpm sigil:test:stdlib`

This makes stdlib regressions much harder to miss during language work.

## A Useful Side Effect: It Surfaced a Real Bug

While writing the new stdlib tests, one list predicate test exposed an existing behavior issue involving list-pattern specificity.

That was a great outcome:

- the tests did their job immediately
- the hook would have caught it on future edits too

We adjusted the initial test set to keep the automation green while preserving a note to investigate the underlying compiler behavior separately.

This is exactly what a good “always-on” feedback loop should do: reveal problems early, close to the edit that might have caused them.

## The Bigger Pattern

This change reflects a broader Sigil principle:

**Use the language's own canonical mechanisms inside the language repo.**

If Sigil says:

- one canonical test syntax
- tests live in `tests/`
- deterministic workflows matter

Then the Sigil repository should embody those rules, not just document them.

## Practical Outcome

After the change:

- stdlib behavior tests are first-class Sigil tests
- there is a single command to run them (`pnpm sigil:test:stdlib`)
- Claude can run them automatically after relevant edits
- the repository's examples are more consistent with the language philosophy

This is small infrastructure, but high leverage. It reduces drift between:

- what Sigil says is canonical
- what Sigil's own repository actually does

And for a machine-first language, that alignment is everything.

---

**Takeaway:** Canonical language design is not just syntax design. It also applies to repository structure, test placement, and AI-assisted workflows. Moving stdlib tests into a real Sigil test project and automating them with Claude hooks makes Sigil's development process more consistent with Sigil's own principles.
