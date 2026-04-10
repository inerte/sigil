# Repo Audit

`projects/repoAudit` keeps first-party repository invariants checked from one
machine-readable runner.

V1 owns three checks:

1. `docs-drift` validates tracked Markdown Sigil fences and stdlib doc coverage
2. `canonical-stdlib` rejects local wrappers around canonical stdlib helpers
3. `repo-compile` batch-compiles non-ignored Sigil source across the repo

`docs-drift` is also where `repoAudit` currently uses Sigil's named concurrent
regions: tracked Markdown files are audited in parallel, but the merged issue
order still follows the canonical path order.

## Running it

From the repo root:

- full audit:
  `language/compiler/target/debug/sigil run projects/repoAudit/src/main.sigil`
- one check:
  `language/compiler/target/debug/sigil run projects/repoAudit/src/main.sigil -- --check docs-drift`

The runner always prints one JSON object to stdout and exits with:

- `0` when no issues are found
- `1` when issues are found or when the runner itself fails

Supported v1 check ids:

- `docs-drift`
- `canonical-stdlib`
- `repo-compile`

## Docs Drift Fence Kinds

Use one of these exact fence labels:

- `sigil program`
- `sigil module`
- `sigil expr`
- `sigil exprs`
- `sigil type`
- `sigil decl stdlib::module`
- `sigil invalid-expr`
- `sigil invalid-module`
- `sigil invalid-program`
- `sigil invalid-type`

What they mean:

- `program`: compile as a `.sigil` file
- `module`: compile as a `.lib.sigil` file
- `expr`: wrap one expression and parse it
- `exprs`: wrap multiple standalone expressions/examples and parse them
- `type`: wrap one type expression and parse it
- `decl`: declaration-only stdlib docs snippet used for stdlib coverage
- `invalid-*`: snippet is intentionally invalid and must fail

Valid checked `sigil module` and `sigil program` fences may contain real Sigil
comments. Docs drift mirrors the compiler: comments are ignored for canonical
source comparison and for snippet ref coverage extraction.

Doc-only annotation lines are ignored inside Sigil fences when they start with:

- `//`
- `⟦`
- `✅`
- `❌`

## Fix Policy

When the audit fails, fix the source material first unless the checker is
plainly wrong.

Use this order:

1. decide whether the snippet or helper is meant to be valid
2. update the source to current canonical Sigil if it is meant to be valid
3. only change the checker if the failure shows a real checker bug

Do not weaken the audit to accept stale repo surfaces.
