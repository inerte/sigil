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
- keep `where` as the type-refinement surface and `requires` / `decreases` (when self-recursive) / `ensures` as the function-contract surface, in that clause order
- compare structural types by their normalized canonical forms, not raw unresolved names
- sum types remain nominal unless the language design is explicitly changed

## Development tips

Don't give estimates about time or think a change is too big or will take a long time. Ignore complexity of implementation when proposing changes.

<!-- BEGIN BEADS INTEGRATION v:1 profile:full hash:f65d5d33 -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Quality
- Use `--acceptance` and `--design` fields when creating issues
- Use `--validate` to check description completeness

### Lifecycle
- `bd defer <id>` / `bd supersede <id>` for issue management
- `bd stale` / `bd orphans` / `bd lint` for hygiene
- `bd human <id>` to flag for human decisions
- `bd formula list` / `bd mol pour <name>` for structured workflows

### Auto-Sync

bd automatically syncs via Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- No manual export/import needed!

### Git Worktrees

- Treat Beads state as shared per repository, not per linked git worktree, unless a worktree was explicitly initialized with its own separate Beads state.
- When working in a linked worktree, run `bd` commands from the primary checkout that owns the shared `.beads/` state. If unsure, resolve it with `git rev-parse --git-common-dir` or inspect `git worktree list`.
- Keep code edits, builds, tests, and git commits in the active worktree. Keep issue tracking, Beads sync, and Beads cleanup in the primary checkout.
- Do not treat worktree-local `.beads/` diffs or exported `issues.jsonl` snapshots as intentional source edits unless the task is specifically about Beads storage or export behavior.

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Session Completion

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Hand off** - Provide context for next session

<!-- END BEADS INTEGRATION -->
