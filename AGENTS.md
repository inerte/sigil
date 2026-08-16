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
   - for derivable named types, treat `derive json` as the only direct `encode*` / `decode*` / `parse*` / `stringify*` surface; custom JSON wire formats should go through explicit payload types
   - explicit named concurrent regions are the canonical widening surface; do not reintroduce a broad "concurrent by default" story in docs or code examples
   - machine-readable CLI output uses one `formatVersion: 1` envelope with `compilerVersion`, canonical `command`, `ok`, `phase`, `analysis`, object-valued `data`, and ordered `diagnostics`; do not add command-specific top-level exceptions
   - agents must check `ok` and `analysis.status` before trusting `data`, apply only `machineApplicable` fix-its automatically, and use `sigil inspect trust` when reviewing extern, protocol, boundary, codec, dependency, topology, or runtime-effect changes
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
- Use `pnpm sigil:quality` as the authoritative pre-release and full-repo gate
- Summarize what changed, what was verified, and any known unrelated failures

For release changes, preserve the five-platform artifact contract, generate
`release-manifest.json` and `SHA256SUMS` before publication, and never overwrite
published release assets. Homebrew is a retryable downstream channel, not the
authority for release validity.

## Commit Guidance

- Explain why the change matters (not just what changed)
- Use accurate verbs (`fix`, `update`, `docs`, `refactor`, `test`, `add`)
- Match existing repo style and tone in recent commits

## Escalation / Ambiguity

If a change affects language design (syntax, canonical forms, stdlib surface, codegen contracts), pause and clarify the intended invariant before implementing broad edits.

When working on Sigil type compatibility:
- unconstrained aliases and unconstrained named product types are structural everywhere in the checker
- constrained aliases and constrained named product types use refinement checking over their underlying type
- keep `where` as the type-refinement surface and `requires` / `decreases` (on total self-recursive declarations) / `ensures` as the function-contract surface, in that clause order
- compare structural types by their normalized canonical forms, not raw unresolved names
- sum types remain nominal unless the language design is explicitly changed

## Development tips

Don't give estimates about time or think a change is too big or will take a long time. Ignore complexity of implementation when proposing changes.

## Repository-specific Beads policy

- Treat Beads state as shared per repository, not per linked git worktree, unless a worktree was explicitly initialized with separate Beads state.
- In a linked worktree, run `bd` commands from the primary checkout that owns `.beads/`; use `git rev-parse --git-common-dir` or `git worktree list` when unsure.
- Keep code edits, builds, tests, and git commits in the active worktree. Keep issue tracking, Beads sync, and Beads cleanup in the primary checkout.
- Do not treat worktree-local `.beads/` diffs or exported `issues.jsonl` snapshots as intentional source edits unless the task concerns Beads storage or exports.

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
