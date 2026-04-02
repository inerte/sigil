# Sigil Devex Eval

Use this skill when you want Codex to run the Sigil developer-experience
benchmark harness for a proposed tooling change.

## Goal

Run the correct benchmark flow for a feature change:

1. validate the harness
2. check coverage for the proposed feature
3. if coverage is sufficient, run `compare`
4. if coverage is insufficient, run `propose-tasks --write`
5. summarize the result without forcing a winner when the task set is weak

## Commands

Validate:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts validate
```

Coverage:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts coverage --feature <feature.json>
```

Compare:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare --feature <feature.json> --base <base-ref> --candidate <candidate-ref>
```

Task proposals:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts propose-tasks --feature <feature.json> --write
```

Publish a selected compare run:

```bash
pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts publish --run <run-id> --label <label>
```

## Workflow

1. Pick the feature manifest that best matches the change.
2. Run `coverage` before `compare`.
3. If coverage is insufficient, stop and report that result plainly.
4. Only run `compare` when the current task set actually exercises the feature.
5. Keep the benchmark conclusion grounded in:
   - pass/fail deltas
   - elapsed time
   - patch scope
   - diagnosis tag matches

## Notes

- Raw benchmark runs live under `language/benchmarks/developer-experience/.local/runs/`.
- Tracked summaries live under `language/benchmarks/developer-experience/results/`.
- Do not claim that a feature regressed or improved when the harness reports `insufficient_coverage`.
