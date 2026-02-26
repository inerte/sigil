# AGENTS.md (language/)

This guide is for AI coding agents working inside `language/` (the Sigil language implementation).

We are **designing** this new programming language together. Feel free to propose changes that still adhere to the overall goals.

Wear your PhD Computer Scientist and Programming Language Designer Expert hats when working in this repo.

Use the repo root guide for cross-repo coordination:
- `../AGENTS.md`

This file is the local authority for:
- compiler/frontend changes
- canonical syntax changes
- stdlib changes under `language/stdlib/`
- language docs/spec sync inside `language/docs/` and `language/spec/`

## Scope

`language/` contains:
- `compiler/` — lexer, parser, validator, typechecker, codegen, CLI
- `stdlib/` — canonical Sigil modules
- `examples/` — runnable/demo Sigil snippets
- `test-fixtures/` — compile/run regression fixtures
- `docs/` and `spec/` — syntax/specification/reference docs
- `tools/` — LSP / VS Code extension (language tooling)

## Sigil Priorities (for language changes)

1. Canonical syntax over flexibility
2. Deterministic parsing/validation/codegen over convenience
3. Executable examples/tests over prose claims
4. Explicit errors with corrective guidance
5. Minimize syntax ambiguity, especially for AI generation

When in doubt: prefer fewer surface forms and better diagnostics.

## Working Rules for Language Development

### 1) Change the whole pipeline when syntax changes

If you change syntax, audit all impacted layers:
- lexer tokens/scanning
- parser grammar + AST construction
- canonical validation (if applicable)
- typechecker assumptions/messages
- codegen expectations/comments
- CLI/help/error messages
- docs/spec examples
- runnable examples/tests/fixtures
- editor grammar (`tools/vscode-extension`)

Do not land syntax changes that only update the parser.

### 2) Preserve canonicality

Sigil is not “many ways to do it.” If adding a feature:
- define the one canonical surface form
- reject obvious alternatives with helpful errors
- update docs to present only the canonical form

If a parser ambiguity appears, favor the interpretation that preserves globally expected meaning (e.g., arithmetic operators should behave like arithmetic).

### 3) Keep user-facing errors actionable

Error messages should:
- state what was found
- state the canonical form
- give a minimal example fix when possible

Prefer:
- `Use "⋅" (e.g., i stdlib⋅list)`

Over:
- vague parse failures with no remediation

### 4) Stdlib modules are typed interfaces, not just examples

`stdlib/` modules are consumed through typed imports.

When adding or relying on stdlib functions:
- ensure required functions are exported (`export λ...`)
- keep module boundaries intentional (avoid duplicate public APIs across modules unless deliberate)
- update docs/spec references if canonical module names or public functions change

### 5) Comments/docs can be stale; compiler/tests are source of truth

Before assuming syntax is valid, verify with:
- `node language/compiler/dist/cli.js compile <file>`
- parser/validator/typechecker tests

If docs disagree with implementation, either:
- fix docs if implementation is intended
- or fix implementation + tests if docs/spec is intended

## Language Change Protocol (Recommended)

For non-trivial language changes (syntax, semantics, codegen contracts):

1. Confirm current behavior with a minimal failing/working example
2. Implement frontend/compiler changes
3. Update fixtures/examples that exercise the changed syntax
4. Update docs/specs in the same change
5. Run targeted tests/compiles
6. Summarize unrelated failures explicitly

## Common Commands (from repo root)

Build compiler:

```bash
pnpm --filter @sigil-lang/compiler build
```

Compile one Sigil file:

```bash
node language/compiler/dist/cli.js compile language/examples/fibonacci.sigil
```

Run one Sigil file:

```bash
node language/compiler/dist/cli.js run language/examples/fibonacci.sigil
```

Run project tests:

```bash
node language/compiler/dist/cli.js test projects/algorithms/tests
node language/compiler/dist/cli.js test projects/todo-app/tests
```

Run compiler unit tests:

```bash
pnpm --filter @sigil-lang/compiler test
```

## Directory-Specific Notes

### `compiler/src/lexer` and `compiler/src/parser`
- Syntax changes usually start here.
- Be explicit about token meaning and precedence.
- Avoid introducing context-sensitive parsing when a dedicated token/form can remove ambiguity.

### `compiler/src/validator`
- Canonical form rules live here.
- If parser accepts multiple forms but Sigil only allows one, validator must reject non-canonical forms clearly.

### `compiler/src/typechecker`
- If syntax/module naming changes affect namespaces/imports, update user-facing error text to match canonical Sigil syntax.
- Keep internal representations stable when possible (e.g., filesystem/module resolution formats).

### `compiler/src/codegen`
- Generated output should remain deterministic.
- Comments/examples in codegen should reflect current Sigil syntax even when emitted JS uses different separators/conventions.

### `stdlib/`
- Prefer small, canonical modules.
- Avoid duplicate overlapping functions across modules unless there is a clear module-boundary reason.
- Exported surface should match how tests/examples import the module.

### `examples/` and `.sigil.map` files
- `.sigil.map` files are source maps generated by the compiler - they map generated JavaScript back to Sigil source for debugging
- These files ARE committed to the repository (they're part of the language distribution)
- When you run/compile examples, the .sigil.map files will be regenerated
- Commit .sigil.map changes when they result from compiler improvements or example updates
- These enable better debugging and IDE/LSP integration

### `docs/` and `spec/`
- `docs/` = current practical/canonical usage
- `spec/` = formal / broader design contracts
- If implementation intentionally diverges from spec, note it explicitly instead of silently drifting examples

### `tools/vscode-extension`
- Update syntax highlighting patterns when syntax tokens/operators change.
- Highlighting is secondary to compiler correctness, but should ship in the same change for syntax updates.

## What to Include in Change Summaries

For language work, summarize:
- language invariant changed (what is now canonical)
- compiler layers touched
- docs/spec/examples updated
- verification commands run
- known unrelated failures (if any)

## Commit Guidance (language/)

Good commit messages explain why the language/compiler change matters:
- ambiguity removed
- canonical form enforced
- typed import/export bug fixed
- diagnostics improved

Examples of useful verbs:
- `Fix` parser ambiguity for namespace/division parsing
- `Update` canonical import syntax to use ⋅ separators
- `Export` stdlib list utilities for typed imports
- `Sync` docs/spec examples with parser behavior
