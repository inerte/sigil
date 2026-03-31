# Sigil CLI JSON Contract

Sigil CLI commands are machine-first. JSON is the default output mode for:

- `sigilc lex`
- `sigilc parse`
- `sigilc compile`
- `sigilc inspect types`
- `sigilc inspect validate`
- `sigilc inspect codegen`
- `sigilc inspect world`
- `sigilc test`
- `sigilc` usage/unknown-command failures

`sigilc run` is split:

- plain `sigil run <file>` streams raw program stdout/stderr on success
- plain `sigil run <file>` emits structured JSON on failure
- `sigil run --json <file>` emits the structured JSON envelope on both success and failure
- `sigil run --json --trace <file>` adds a bounded inline execution trace to that envelope
- `sigil run --json --break-fn <name> <file>` adds machine-readable breakpoint snapshots
- `sigil run --json --record <artifact> <file>` adds replay recording metadata and writes a replay artifact
- `sigil run --json --replay <artifact> <file>` replays a prior artifact and reports replay consumption metadata

## Canonical Schema

The normative machine contract is:

- `language/spec/cli-json.schema.json`

Consumers should validate against that schema, not this Markdown file.

## Versioning

- `formatVersion` is the payload format version
- current version: `1`
- backward-incompatible output changes require incrementing `formatVersion`

## Common Envelope Pattern

Most commands emit:

```json
{
  "formatVersion": 1,
  "command": "sigilc compile",
  "ok": true,
  "phase": "codegen",
  "data": { "...": "..." }
}
```

Failures emit:

```json
{
  "formatVersion": 1,
  "command": "sigilc compile",
  "ok": false,
  "phase": "parser",
  "error": {
    "code": "SIGIL-PARSE-NS-SEP",
    "phase": "parser",
    "message": "invalid namespace separator"
  }
}
```

`sigilc test` keeps a specialized top-level `summary` / `results` envelope.
`sigilc inspect types`, `sigilc inspect validate`, `sigilc inspect codegen`, and `sigilc inspect world` use inspect-specific envelopes.
`sigilc run` uses the `runEnvelope` schema in `--json` mode and for failure payloads.

## Diagnostics

Diagnostics are structured and machine-oriented:

- `code`
- `phase`
- `message`
- `location` when available
- `found` / `expected` when useful
- `details`
- `fixits`
- `suggestions`

## Run Failure Details

When `sigil run` fails after compilation and runner launch, the failure envelope keeps the
usual top-level diagnostic shape and may enrich `error.details` with:

- `compile`
  - `input`
  - `output`
  - `runnerFile`
  - `spanMapFile`
- `runtime`
  - `engine`
  - `exitCode`
  - `durationMs`
  - `stdout`
  - `stderr`
- optional `trace`
  - `enabled`
  - `truncated`
  - `totalEvents`
  - `returnedEvents`
  - `droppedEvents`
  - `events`
- optional `breakpoints`
  - `enabled`
  - `mode`
  - `stopped`
  - `truncated`
  - `totalHits`
  - `returnedHits`
  - `droppedHits`
  - `maxHits`
  - `hits`
- optional `replay`
  - `mode`
  - `file`
  - `recordedEvents`
  - `consumedEvents`
  - `remainingEvents`
  - `partial`
- `exception` for uncaught runtime exceptions
  - `name`
  - `message`
  - `rawStack`
  - optional `generatedFrame`
  - optional `sigilFrame`

## Run Trace Details

`sigil run --trace` currently requires `--json`.

When enabled, `sigil run` includes a bounded rolling trace window:

- only the most recent `256` events are returned inline
- older events are dropped and reflected through:
  - `truncated`
  - `totalEvents`
  - `returnedEvents`
  - `droppedEvents`

Current trace event kinds:

- `call`
- `return`
- `branch_if`
- `branch_match`
- `effect_call`
- `effect_result`

Every trace event includes:

- `seq`
- `kind`
- `depth`
- `moduleId`
- `sourceFile`
- `spanId`

Events may also include:

- declaration context such as `declarationKind` / `declarationLabel`
- `functionName`
- `args`
- `result`
- branch selection details such as `taken`, `armSpanId`, `armIndex`, `hasGuard`
- effect details such as `effectFamily` and `operation`

`sigilFrame` is declaration-level in v1:

- it identifies the owning top-level Sigil declaration using the generated `.span.json` sidecar
- it may include a tiny declaration-header excerpt
- it does not yet promise exact nested-expression blame inside the declaration body

## Run Breakpoint Details

Breakpoint selectors currently require `--json`.

Supported selectors:

- `--break <file:line>`
- `--break-fn <name>`
- `--break-span <id>`

Supported controls:

- `--break-mode stop|collect`
- `--break-max-hits <n>`

The inline `breakpoints` block reports:

- `enabled`
- `mode`
- `stopped`
- `truncated`
- `totalHits`
- `returnedHits`
- `droppedHits`
- `maxHits`
- `hits`

Each hit currently includes:

- matched selector summaries
- `moduleId`
- `sourceFile`
- `spanId`
- optional declaration context such as `declarationKind` / `declarationLabel`
- resolved Sigil `location` when the span map can provide it
- current-frame `locals`
- stack-frame summaries
- a bounded `recentTrace` window using the same event schema as `data.trace`

Stop mode is a successful early stop:

- the top-level envelope still uses `ok: true`
- `data.breakpoints.stopped` is `true`
- `runtime.stdout` / `runtime.stderr` contain only output produced before the stop

Collect mode keeps running:

- `stopped` remains `false`
- only the most recent `maxHits` snapshots are returned
- older hits are reflected through `truncated`, `totalHits`, `returnedHits`, and `droppedHits`

## Run Replay Details

`sigil run` now supports first-class record/replay:

- `--record <artifact>` writes a standalone replay artifact file
- `--replay <artifact>` reuses that artifact as the runtime world/effect source
- `--record` and `--replay` are mutually exclusive
- `--replay` cannot be combined with `--env`; the artifact owns replay-world resolution

The inline `replay` block in the `run` envelope is intentionally small:

- `mode`
- `file`
- `recordedEvents`
- `consumedEvents`
- `remainingEvents`
- `partial`

Current replay coverage in v1:

- `random`
- `timer` / `time.now`
- `process`
- `http`
- `tcp`

Replay artifacts are strict:

- bound to the original entry file
- bound to the original argv
- bound to a source-graph fingerprint
- consumed in exact recorded sequence

Standalone replay artifact schema:

- `language/spec/run-replay.schema.json`

## Inspect World Details

`sigil inspect world <path> --env <name>` is project-env scoped.

Successful output reports:

- `input`
- `project`
- `projectRoot`
- `environment`
- `topology`
  - `present`
  - `declaredEnvs`
  - `httpDependencies`
  - `tcpDependencies`
- `summary`
  - singleton entry kinds plus HTTP/TCP binding counts
- `normalizedWorld`

`normalizedWorld` is the runtime-normalized template Sigil will use for that
environment, not the raw exported Sigil `world` value.

## Inspect Codegen Details

`sigil inspect codegen <path>` mirrors the real compile pipeline but keeps the
results in memory.

Successful single-file output reports:

- `input`
- `moduleId`
- `sourceFile`
- `project`
- `summary`
  - `modules`
  - `lineCount`
  - `spans`
  - `generatedRanges`
  - `topLevelAnchors`
- `codegen`
  - `outputFile`
  - `spanMapFile`
  - `source`
  - `lineCount`
  - `spanMapSummary`
- `modules`
  - per-module inventory for the resolved compile graph

Directory mode reports:

- `input`
- `summary`
  - `discovered`
  - `inspected`
  - `groups`
  - `modules`
  - `durationMs`
- `files`
  - one single-file-style result per requested file

Only the requested file gets inline generated TypeScript in v1. Imported modules
appear in `modules` inventory only.

`inspect codegen` does not write the derived `.ts` or `.span.json` files to
disk. The reported `outputFile` and `spanMapFile` values are the same derived
paths that `sigil compile` would use.

## Current Notes

The current implementation uses:

- `"sigilc ..."` strings in JSON `command` fields
- successful `compile` output reports `.span.json` sidecars via `rootSpanMap` and per-module `spanMapFile`
- successful `run --json` output reports the entry module `.span.json` sidecar via `data.compile.spanMapFile`
- successful `run --json --trace` output reports inline bounded trace events via `data.trace`
- breakpoint-enabled `run --json` output may report inline snapshots via `data.breakpoints`
- successful `run --json --record` and `run --json --replay` output may report inline replay summary data via `data.replay`
- runtime `run` failures may include declaration-level `sigilFrame` and generated TypeScript `generatedFrame` context when an uncaught exception stack is available
- traced `run` failures may include bounded inline trace events via `error.details.trace`
- breakpoint-enabled `run` failures may include bounded snapshot data via `error.details.breakpoints`
- recorded or replayed `run` failures may include replay summary data via `error.details.replay`
- `inspect types` is top-level declaration-focused in v1; it does not report nested expression types yet
- `inspect validate` returns canonical printer output even when `validation.ok` is `false`, as long as lexing and parsing succeeded
- `inspect codegen` returns generated TypeScript inline for the requested file and only inventories imported modules
- `inspect world` is project-level in v1; it does not batch over directories or include test-local `world { ... }` overlays
- a specialized `test` result shape with `location: {line,column}`

If prose and runtime output disagree, the implementation and
`cli-json.schema.json` are the current source of truth.
