---
name: sigil-devex-eval
description: Benchmark real Sigil write, edit, and repair outcomes with the developer-experience harness. Use when Codex should compare clean HEAD against the current working tree across the benchmark task suite and report which Sigil tasks leaned baseline, leaned compare, or tied under blinded repeat judging.
---

# Sigil Devex Eval

Run the Sigil developer-experience benchmark harness against the current work in
progress or against explicit refs when the user asks.

## Default flow

1. Validate the harness.
2. Run `compare` across the benchmark task suite.
3. Summarize per-task judged repeat wins first, then the suite task-lean counts.
4. Narrow to `--tasks`, explicit refs, or `--repeats 1` only when the user asks for a focused or faster run.

Default comparison mode:

- base: clean `HEAD`
- candidate: current working tree snapshot
- tasks: all task manifests
- repeats: `3`
- task scheduling: up to `2` tasks in flight at a time
- repeat scheduling: up to `3` base/candidate repeat pairs in flight per task
- benchmark truth: one blinded Codex judge reads the full artifact bundle for each repeat pair and returns `A`, `B`, or `TIE`
- official report: per-task baseline repeat wins, compare repeat wins, ties, and task lean
- diagnostics: raw pass counts, command/token counts, budget flags, and elapsed time remain visible in artifacts but do not decide the verdict

## Commands

Validate:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts validate
```

Compare current work in progress across all tasks:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare
```

Run a faster single-sample smoke compare:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare --repeats 1
```

Focus on selected tasks only:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare --tasks <task-id,task-id>
```

Explicit ref compare:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare --base <base-ref> --candidate <candidate-ref>
```

Publish a selected run:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts publish --run <run-id> --label <label>
```

## Notes

- Read raw run bundles from `language/benchmarks/developer-experience/.local/runs/`.
- Read tracked summaries from `language/benchmarks/developer-experience/results/`.
- Treat the suite as outcome-first: the benchmark measures whether Codex gets
  better at real Sigil work, not whether a specific internal feature is used.
- Treat the blinded judge result as the primary signal. Numeric budget and
  timing fields are diagnostics only.
- A tied task means the 3 repeat judgments did not lean baseline or compare.
