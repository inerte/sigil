# Developer-Experience Benchmarks

This benchmark family measures whether Sigil gets better for coding agents at
real work: writing, editing, and fixing Sigil programs.

The benchmark is **outcome-first**. By default, `compare` uses a clean `HEAD`
baseline and the **current working tree snapshot** as the candidate, and it
runs the full task corpus unless you explicitly narrow it with `--tasks`.

The official compare result is now **judge-first**:

- each task runs `3` repeated baseline-vs-candidate pairs
- after each repeat pair finishes, a fresh blinded Codex judge reads the full
  artifact bundle for both sides
- the judge returns `A`, `B`, or `TIE`
- task summaries count baseline repeat wins, compare repeat wins, and ties
- suite summaries count how many tasks lean baseline, lean compare, or tie

The old numeric metrics still exist in raw artifacts for diagnostics, but they
no longer decide the benchmark outcome.

Each task manifest now carries:

- `maxCommandExecutions`
- `maxEffectiveTokens`
- `maxWallClockMs`

`maxWallClockMs` remains a safety timeout. Command and token budgets are still
recorded in sample results, but they are now diagnostic rather than official.

## Layout

- `tasks/` — benchmark task manifests
- `fixtures/` — deterministic starting projects copied into temporary workspaces
- `tools/` — TypeScript/Node runner and tests
- `results/` — tracked published summaries and aggregate history
- `.local/runs/` — ignored raw run history, transcripts, diffs, and compare bundles

## Commands

Validate manifests:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts validate
```

Run a single ref against the selected task set:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts run \
  --ref HEAD \
  --tasks syntax-compile-fix,multimodule-report
```

Run the current working tree against the selected task set:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts run \
  --tasks syntax-compile-fix,multimodule-report
```

Compare clean `HEAD` against the current working tree across the full corpus:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare
```

Run a faster single-sample smoke compare:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare --repeats 1
```

Compare across explicit refs when needed:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare \
  --base main \
  --candidate feature-branch \
  --tasks syntax-compile-fix,test-repair \
  --repeats 3
```

Publish a selected compare run into tracked summaries:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts publish \
  --run <run-id> \
  --label 2026-04-01-devex-sample
```

Run the harness tests:

```bash
node --import tsx --test language/benchmarks/developer-experience/tools/*.test.ts
```

## Result Model

Every sample still records:

- oracle status
- command execution count
- effective tokens
- whether it stayed within the task's command budget
- whether it stayed within the task's token budget
- whether it completed successfully within all budgets

In `compare`, each baseline-vs-candidate repeat pair also records:

- a blinded `judge-input.json` that points to the full artifact bundle for both runs
- a `judge-result.json` with the judge's `A` / `B` / `TIE` verdict
- judge reasons and evidence citations

The official compare summary reports:

- per-task baseline repeat wins
- per-task compare repeat wins
- per-task ties
- per-task task lean: `base`, `candidate`, or `tie`
- suite totals for baseline-leaning tasks, compare-leaning tasks, and tied tasks

Raw pass counts, budget pass counts, command counts, token counts, and elapsed
time remain visible as diagnostics only.

## History Model

Every benchmark invocation writes a full local run bundle under:

```text
language/benchmarks/developer-experience/.local/runs/<timestamp>-<id>/
```

Those local bundles keep:

- `meta.json`
- `run.json` or `compare.json`
- per-task aggregate result JSON
- per-sample task result JSON under `samples/<n>/`
- per-repeat judge artifacts under `judgments/<n>/`
- transcripts
- diffs
- oracle logs

Only compact published summaries belong in `results/`. This keeps long-term
history without polluting the repo with every raw transcript.

## Current Seed Corpus

The initial deterministic task corpus includes:

- `canonical-record-order-repair`
- `canonical-stdlib-helper-repair`
- `event-import-pipeline-repair`
- `feed-description-propagation`
- `homebrew-formula-test-repair`
- `syntax-compile-fix`
- `multimodule-report`
- `repair-ingest-received-timestamp`
- `repair-feed-published-timestamp`
- `site-route-canonicalization-repair`
- `stats-summary-implementation`
- `test-repair`
- `todo-domain-test-repair`
- `todo-json-roundtrip-repair`
- `topology-status-client-feature`
- `topology-world-fix`

These are enough to exercise the harness end to end, but the corpus should grow
only when a common Sigil workflow is missing. Tasks should stay problem-oriented
and outcome-based rather than being tied to a specific feature under development.
