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

- Prefer focused commits by concern (compiler, docs, examples, project app, etc.).
- Avoid changing generated outputs unless needed to validate or accompany source changes.
- When changing Sigil syntax or semantics, update all of:
   - compiler frontend (`lexer`/`parser`/validator/typechecker as applicable)
   - runnable examples/tests
   - canonical docs/specs
- Preserve the repo’s machine-first goals:
   - canonical syntax over stylistic flexibility
   - deterministic behavior and deterministic codegen where possible
   - tests/examples as source of truth over prose docs
   - canonical semantic equality for structural types (unconstrained aliases + unconstrained named products normalize before comparison)
   - keep `where` as the type-refinement surface, `label` as the type-classification surface, and boundary handling in `src/policies.lib.sigil`
   - first-party Sigil code outside `language/stdlib/` should use canonical stdlib helpers directly instead of locally redefining them
   - explicit named concurrent regions are the canonical widening surface; do not reintroduce a broad "concurrent by default" story in docs or code examples
- For website/docs/article writing:
   - prefer normal technical prose over punchy social-post style
   - do not write in "LinkedIn broetry" style with one-line dramatic paragraphs, hype-heavy binaries, or sloganized emphasis
   - explain the problem, decision, implementation, and tradeoffs directly
   - keep the tone technical, calm, and specific rather than performative
- Doing it right is better than taking the easy path. You're a fast editing machine, changing code is easy to you.

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

When working on Sigil type compatibility:
- unconstrained aliases and unconstrained named product types are structural everywhere in the checker
- constrained aliases and constrained named product types use refinement checking over their underlying type
- keep `where` as the type-refinement surface and `requires` / `ensures` as the function-contract surface
- compare structural types by their normalized canonical forms, not raw unresolved names
- sum types remain nominal unless the language design is explicitly changed

## Development tips

Don't give estimates about time or think a change is too big or will take a long time. Ignore complexity of implementation when proposing changes.
