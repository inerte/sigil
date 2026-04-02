# Developer-Experience Benchmarks

This benchmark family measures whether Sigil tooling changes actually improve a
coding agent's maintenance loop.

V1 is **Codex-first** and compares a `base` ref against a `candidate` ref on a
deterministic task corpus. The harness is designed to say **insufficient
coverage** when a feature does not have the right task support yet, instead of
forcing a winner.

## Layout

- `tasks/` — benchmark task manifests
- `features/` — feature manifests used for coverage gating
- `fixtures/` — deterministic starting projects copied into temporary workspaces
- `tools/` — TypeScript/Node runner and tests
- `results/` — tracked published summaries and aggregate history
- `.local/runs/` — ignored raw run history, transcripts, diffs, and compare bundles

## Commands

Validate manifests:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts validate
```

Check whether the current task set covers a proposed feature:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts coverage \
  --feature agent-edit-loop-smoke.json
```

Propose new tasks for an under-covered feature:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts propose-tasks \
  --feature inspect-types-typeids.json \
  --write
```

Run a single ref against the selected task set:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts run \
  --ref HEAD \
  --tasks syntax-compile-fix,multimodule-report
```

Compare a feature across `base` and `candidate` refs:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare \
  --feature agent-edit-loop-smoke.json \
  --base main \
  --candidate HEAD
```

Publish a selected compare run into tracked summaries:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts publish \
  --run <run-id> \
  --label 2026-04-01-agent-edit-loop
```

Run the harness tests:

```bash
node --import tsx --test language/benchmarks/developer-experience/tools/*.test.ts
```

## History Model

Every benchmark invocation writes a full local run bundle under:

```text
language/benchmarks/developer-experience/.local/runs/<timestamp>-<id>/
```

Those local bundles keep:

- `meta.json`
- `coverage.json`
- `run.json` or `compare.json`
- per-task result JSON
- transcripts
- diffs
- oracle logs

Only compact published summaries belong in `results/`. This keeps long-term
history without polluting the repo with every raw transcript.

## Current Seed Corpus

The initial deterministic task corpus includes:

- `syntax-compile-fix`
- `multimodule-report`
- `test-repair`
- `topology-world-fix`

These are enough to exercise the harness end to end, but they are not intended
to cover every future feature. New feature work should add or curate tasks
before relying on benchmark comparisons.

