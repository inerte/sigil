# AGENTS.md (Repo Root)

## Scope

Use this file for repo-wide coordination and navigation.

For language/compiler work, prefer the deeper guide:
- `language/AGENTS.md` (authoritative for Sigil language/compiler/parser/typechecker/docs inside `language/`)

## Repository Layout

- `language/` — Sigil programming language source, compiler, specs, stdlib, tools
- `projects/` — example/demo projects using Sigil
- `website/` — website/docs site work (if present)
- `tools/` — repo tooling scripts/utilities

## Working Rules (Root-Level)

1. Prefer focused commits by concern (compiler, docs, examples, project app, etc.).
2. Avoid changing generated outputs unless needed to validate or accompany source changes.
3. When changing Sigil syntax or semantics, update all of:
   - compiler frontend (`lexer`/`parser`/validator/typechecker as applicable)
   - runnable examples/tests
   - canonical docs/specs
4. Preserve the repo’s machine-first goals:
   - canonical syntax over stylistic flexibility
   - deterministic behavior and deterministic codegen where possible
   - tests/examples as source of truth over prose docs

## Practical Workflow

- Start with discovery (`rg`, targeted file reads)
- Make the smallest coherent change
- Run relevant checks (build/compile/tests) for touched areas
- Summarize what changed, what was verified, and any known unrelated failures

## Commit Guidance

- Explain why the change matters (not just what changed)
- Use accurate verbs (`fix`, `update`, `docs`, `refactor`, `test`, `add`)
- Match existing repo style and tone in recent commits

## Escalation / Ambiguity

If a change affects language design (syntax, canonical forms, stdlib surface, codegen contracts), pause and clarify the intended invariant before implementing broad edits.

## Development tips

Don't give estimates about time or think a change is too big or will take a long time. Ignore complexity of implementation when proposing changes.
