# Sigil Devex Task Author

Use this skill when the developer-experience benchmark harness reports
`insufficient_coverage` and you need to draft new tasks for a proposed Sigil
tooling feature.

## Goal

Turn a coverage gap into draft benchmark tasks that are:

- deterministic
- fixture-backed
- oracle-driven
- scoped to the proposed feature's real capability surface

## Inputs

Start from:

- `language/benchmarks/developer-experience/.local/proposals/<featureId>/coverage.json`
- any generated proposal JSON stubs in that same directory
- the existing task corpus in `language/benchmarks/developer-experience/tasks/`

## Workflow

1. Read the coverage report and identify which capability tags or surface tags are missing.
2. Inspect the current task corpus to avoid duplicating existing tasks.
3. Draft new task manifests that add the missing coverage with deterministic fixtures and executable oracles.
4. Prefer small Sigil projects or single-purpose fixture directories over open-ended repo tasks.
5. Keep `allowedEditPaths` narrow and make the oracle hard to game by editing tests.

## Output

Write draft manifests and any supporting fixture notes into:

```text
language/benchmarks/developer-experience/.local/proposals/<featureId>/
```

Do not place new tracked benchmark tasks directly into `tasks/` without human review.

