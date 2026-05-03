---
title: "sigil review: Semantic Diffs for Agents and Reviewers"
date: 2026-05-01
author: Sigil Language Team
slug: sigil-review
---

# `sigil review`: Semantic Diffs for Agents and Reviewers

`git diff` shows which lines changed. That is useful for tracking edits but it
does not answer the questions that matter most during code review: did a function
gain a side effect? did a contract weaken? did a public signature change in a
way callers need to adapt to?

`sigil review` answers those questions directly.

## A Concrete Example

Before:

```sigil module
e api:{fetchUser:λ(String)=>!Http Result[
  String,
  String
]}

λfetchUser(id:String)=>Result[
  String,
  String
]=Ok("user:"++id)
```

After:

```sigil module
e api:{fetchUser:λ(String)=>!Http Result[
  String,
  String
]}

λfetchUser(id:String)=>!Http Result[
  String,
  String
]
requires #id>0
=api.fetchUser(id)
```

`git diff` reports:

```text
-λfetchUser(id:String)=>Result[
+λfetchUser(id:String)=>!Http Result[
   String,
   String
-]=Ok("user:"++id)
+]
+requires #id>0
+=api.fetchUser(id)
```

`sigil review` reports:

```text
## Sigil Review

Summary
- changed declarations: 1
- signature changes: 0
- contract changes: 1
- effect changes: 1
- type/refinement changes: 0
- trust surface changes: 0
- changed test files: 0

Effect Changes
- ~ function `fetchUser` in `src/api.lib.sigil:1`
  - effects: `<none>` -> `!Http`
  - requires: `<none>` -> `#id>0`

Contract Changes
- ~ function `fetchUser` in `src/api.lib.sigil:1`
  - effects: `<none>` -> `!Http`
  - requires: `<none>` -> `#id>0`

Test Evidence
- changed test files: none
- changed coverage targets: none
```

The semantic reading is immediate: `fetchUser` widened its public effect
contract to `!Http`, now calls an `!Http` callee, and now imposes a
precondition on callers. No test files changed despite a coverage target being
modified — a warning worth acting on.

## What It Does

`sigil review` snapshots the before and after versions of changed Sigil files,
compiles each side, and compares them at the declaration level. For every
function, type, extern, effect alias, feature flag, const, and test that changed,
it reports what specifically changed and where the declaration starts: not which
diff hunk lines changed, but which semantic properties did.

For functions and transforms, the tracked properties are:

- **signature** — parameter types or return type changed
- **mode** — `ordinary` vs `total` changed
- **effects** — the declared effect set changed
- **requires** — precondition changed
- **decreases** — termination measure changed
- **ensures** — postcondition changed
- **implementation** — body changed with no surface-level delta

For types: definition and constraint changes. For externs: trust surface changes
(module path or member list). For effect aliases: the expanded primitive set.
For feature flags: value type, creation date, or default changed.

If a function's effects changed but nothing else did, the review reports only
an effect change — not a wall of line noise around it.

## Test Evidence

`sigil review` tracks which public functions changed and whether the test suite
moved to match. If coverage targets changed but no test files did, it surfaces
a warning. This is not a coverage percentage. It is a direct check: did the
function that changed have corresponding test activity?

## Usage

```bash
sigil review                       # worktree vs index (unstaged changes)
sigil review --staged              # index vs HEAD (staged changes)
sigil review --base HEAD~1         # last commit
sigil review --base main --head feature-branch
sigil review -- HEAD~3 HEAD        # raw git diff passthrough
sigil review --path src/api.lib.sigil  # limit to one file
```

All modes produce the same output structure. The default mode is the most useful
during active development: it compares the current working tree against the index,
so it reflects what you are about to stage.

## Output Modes

**Default (human-readable markdown):**
Summary counts, then per-declaration changes grouped by kind: Signature Changes,
Effect Changes, Contract Changes, Termination Changes, Trust Surface Changes,
Implementation Changes. Followed by Test Evidence.

**`--json`:**
The same structured data as a JSON envelope, versioned with `"formatVersion": 1`.
Suitable for agent loops that parse and route the output themselves.

**`--llm`:**
The JSON facts wrapped in a prompt preamble:

```text
You are reviewing a Sigil semantic diff.

Use only the facts below.
Do not infer behavior that is not explicitly listed.
If analysisMode is `parseOnly`, call out that limitation.
If any issue has severity `error`, list it first.

Facts:
{ ... }
```

`--llm` is the mode for handing a diff directly to a language model as a
review task. The preamble instructs the model to stay grounded in the listed
facts rather than inferring from the surrounding codebase.

## Why This Is Different

Most diff tools operate on text. `sigil review` operates on typed, compiled
programs. It knows the difference between a signature change and an
implementation change because it has run the typechecker on both sides.

When a function's body changes but its signature, effects, and contracts stay
the same, the review reports an implementation change — internal, lower risk.
When a function gains `!Http`, the review reports an effect change — higher
risk, callers need to propagate the annotation, and under current Sigil rules
the body must justify that effect by performing it or calling a callee that
does.
These are different categories of change that line diffs treat identically.

For a human reviewer, this is the difference between scanning a wall of line
noise and opening directly on the declaration that changed. For a coding agent,
it is a structured fact set that distinguishes safe refactors from semantic
contract changes without reading every modified line.
