# Sigil PR Audit Invariants

Audit against these invariants before trusting a PR.

## Language And Compiler

- Canonical syntax over stylistic flexibility.
- Deterministic parsing, validation, typechecking, and codegen over convenience.
- Tests and runnable examples are stronger evidence than prose docs.
- If syntax or semantics change, update the whole pipeline:
  - lexer or parser as needed
  - validator and typechecker assumptions
  - codegen behavior
  - CLI help or diagnostics
  - tests, fixtures, examples
  - docs and spec
- Do not accept parser-only syntax changes.

## Type Compatibility

- Aliases and named product types compare structurally by normalized canonical form everywhere.
- Sum types remain nominal unless the design explicitly changes.
- Do not accept checker-path-specific compatibility behavior.

## Canonicality

- Sigil should have one canonical surface form where possible.
- Reject syntax broadening that adds multiple acceptable spellings without a canonical form.
- Diagnostics should be corrective and show the canonical alternative.

## Repository Risk Surfaces

Treat these as high-risk even if the PR claims to be unrelated:

- `.github/workflows/**`
- `packaging/**`
- `tools/**`
- release automation
- install scripts
- dependency changes
- shell or subprocess execution
- network access
- filesystem writes outside expected runtime behavior
- environment-variable access
- `unsafe` Rust

## PR Hygiene

- One invariant or one behavior change per PR.
- Mixed compiler, docs, CI, and packaging changes should usually be split.
- Tests should pin the claimed behavior rather than weaken assertions.
- Docs-only PRs should not alter compiler or release surfaces.
