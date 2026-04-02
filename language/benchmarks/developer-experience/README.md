# Developer-Experience Benchmarks

This benchmark family measures whether Sigil gets better for coding agents at
real work: writing, editing, and fixing Sigil programs.

The benchmark is **outcome-first**. By default, `compare` uses a clean `HEAD`
baseline and the **current working tree snapshot** as the candidate, and it
runs the full task corpus unless you explicitly narrow it with `--tasks`.

The benchmark truth is:

- did the agent complete the task successfully
- within that task's **command budget**
- within that task's **effective-token budget**
- across **3 repeated trials**

Elapsed time is still recorded in raw sample artifacts for diagnostics, but it
does not decide the benchmark outcome.

Each task manifest now carries:

- `maxCommandExecutions`
- `maxEffectiveTokens`
- `maxWallClockMs`

`maxWallClockMs` remains a safety timeout. Command and token budgets are scored
post-hoc from the finished run.

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

Every sample records:

- oracle status
- command execution count
- effective tokens
- whether it stayed within the task's command budget
- whether it stayed within the task's token budget
- whether it completed successfully within all budgets

Each task aggregate reports:

- raw pass count and pass rate
- command-budget pass count and pass rate
- token-budget pass count and pass rate
- all-budget pass count and pass rate
- median command count
- median effective tokens

Per-task compare direction is driven only by **all-budget pass count**:

- higher candidate count => `improved`
- lower candidate count => `regressed`
- equal counts => `neutral`

Raw pass counts remain visible as diagnostics, but they do not change the
benchmark verdict.

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
- transcripts
- diffs
- oracle logs

Only compact published summaries belong in `results/`. This keeps long-term
history without polluting the repo with every raw transcript.

## Current Seed Corpus

The initial deterministic task corpus includes:

- `canonical-record-order-repair`
- `canonical-stdlib-helper-repair`
- `homebrew-formula-test-repair`
- `syntax-compile-fix`
- `multimodule-report`
- `repair-ingest-received-timestamp`
- `repair-feed-published-timestamp`
- `stats-summary-implementation`
- `test-repair`
- `todo-domain-test-repair`
- `todo-json-roundtrip-repair`
- `topology-world-fix`

These are enough to exercise the harness end to end, but the corpus should grow
only when a common Sigil workflow is missing. Tasks should stay problem-oriented
and outcome-based rather than being tied to a specific feature under development.
