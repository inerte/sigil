# Sigil CLI JSON Contract

Sigil CLI commands are machine-first. JSON is the default output mode for:

- `sigilc lex`
- `sigilc parse`
- `sigilc compile`
- `sigilc inspect types`
- `sigilc inspect proof`
- `sigilc inspect validate`
- `sigilc inspect codegen`
- `sigilc inspect world`
- `sigil docs list`
- `sigil docs search`
- `sigil docs show`
- `sigil docs context`
- `sigil featureFlag audit`
- `sigilc debug run`
- `sigilc debug test`
- `sigilc test`
- `sigilc` usage/unknown-command failures

`sigilc run` is split:

- plain `sigil run <file>` streams raw program stdout/stderr on success
- plain `sigil run <file>` emits structured JSON on failure
- `sigil run --json <file>` emits the structured JSON envelope on both success and failure
- `sigil run --json --trace <file>` adds a bounded inline execution trace to that envelope
- `sigil run --json --trace --trace-expr <file>` adds expression enter/return/throw events to that trace
- `sigil run --json --break-fn <name> <file>` adds machine-readable breakpoint snapshots
- `sigil run --json --record <artifact> <file>` adds replay recording metadata and writes a replay artifact
- `sigil run --json --replay <artifact> <file>` replays a prior artifact and reports replay consumption metadata

## Canonical Schema

The normative machine contract is:

- `language/spec/cli-json.schema.json`

Consumers should validate against that schema, not this Markdown file.
For the user-facing debugging workflow, examples, and command selection guide,
see `language/docs/DEBUGGING.md`.

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
`sigilc inspect types`, `sigilc inspect proof`, `sigilc inspect validate`, `sigilc inspect codegen`, and `sigilc inspect world` use inspect-specific envelopes.
`sigil docs list`, `sigil docs search`, `sigil docs show`, and `sigil docs context` use docs-specific envelopes with `phase: "docs"` on success.
`sigil featureFlag audit` uses a query-style envelope with `data.summary` and `data.flags`.
`sigilc run` uses the `runEnvelope` schema in `--json` mode and for failure payloads.
`sigilc debug run` and `sigilc debug test` use replay-backed debug envelopes with
`data.session` and `data.snapshot`.

## Docs Retrieval Surface

`sigil docs ...` is the machine-first local knowledge surface for installed
Sigil binaries.

Use it when an assistant or human needs the language guides, syntax reference,
stdlib ownership, package rules, or design rationale without depending on web
search or model priors.

Current commands:

- `sigil docs list`
- `sigil docs search <query>`
- `sigil docs show <docId> [--start-line N] [--end-line N]`
- `sigil docs context --list`
- `sigil docs context <id>`

### `sigil docs list`

Returns:

- `data.documents`

Each `documents[]` entry includes:

- `docId`
- `kind`
- `title`
- `path`
- `description`
- `lineCount`

### `sigil docs search`

Returns:

- `data.query`
- `data.results`

Each `results[]` entry includes:

- `docId`
- `kind`
- `title`
- `path`
- `section`
- `line`
- `before`
- `match`
- `after`
- `isExactPhrase`

`before`, `match`, and `after` are numbered source-line windows over the
embedded document.

### `sigil docs show`

Returns:

- `data.document`
- `data.range`

`data.document` includes:

- `docId`
- `kind`
- `title`
- `path`
- `description`
- `lineCount`
- `lines`

Each `lines[]` entry includes:

- `line`
- `text`
- `section`

### `sigil docs context`

`sigil docs context --list` returns:

- `data.contexts`

Each `contexts[]` entry includes:

- `id`
- `title`
- `description`
- `includedDocs`

`sigil docs context <id>` returns:

- `data.context`

`data.context.includedDocs[]` uses the same document summary shape as
`sigil docs list`.

## Debug Surface Index

Use the current surfaces like this:

- `sigil docs context --list`: discover the curated local knowledge bundles
- `sigil docs search`: find the exact doc and line for a language question
- `sigil docs show`: read the exact local source material by id and line range
- `sigil inspect validate`: canonical source and validation result
- `sigil inspect types`: solved top-level declaration types plus named type inventory
- `sigil inspect proof`: declared proof-bearing surfaces and branch gates
- `sigil inspect world`: normalized runtime world for one project env or one standalone file
- `sigil featureFlag audit`: first-class feature flag declarations, optionally filtered by age
- `sigil inspect codegen`: generated TypeScript plus span-map summary
- `sigil run --json`: one structured run success/failure envelope
- `sigil run --json --trace [--trace-expr]`: bounded runtime trace
- `sigil run --json --break...`: inline breakpoint snapshots
- `sigil run --json --record|--replay`: record/replay summary plus standalone replay artifacts
- `sigil test`: suite-level JSON results with optional per-test debug blocks
- `sigil debug run` and `sigil debug test`: replay-backed stepping sessions with snapshots and watches

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

Proof-oriented typecheck failures may also enrich `error.details` with:

- `proof`
  - `assumptions`
  - `goal`
  - `outcome`
- `proofKind`
- `proofSummary`

## Inspect Types

`sigil inspect types` now reports two complementary views per analyzed file:

- `declarations`: solved top-level `function | const | test` signatures
- `types`: named source-declared types in that module

Each `types[]` entry includes:

- `typeId`
- `name`
- `moduleId`
- `kind`
- `typeParams`
- `definitionSource`
- `definitionAst`
- `constrained`
- `constraintSource`
- `constraintAst`
- `equalityMode`
- `spanId`
- `location`

`definitionAst` and `constraintAst` are normalized semantic nodes with a required
`kind` field and command-specific properties.

`equalityMode` is one of:

- `structural` for unconstrained aliases and named product types
- `refinement` for constrained aliases and named product types
- `nominal` for sums

Focused debug value payloads may also include `typeId` when the value has a
statically known named Sigil type. In v1 this is surfaced on breakpoint locals,
watch results, and expression value/error payloads rather than every generic
trace value summary. Breakpoint and debug locals also expose an optional root
`typeId` when the local itself is statically known to be a named Sigil type.

## Inspect Proof

`sigil inspect proof` inventories the proof-bearing surfaces in one file or
directory.

Single-file output includes:

- `input`
- `moduleId`
- `sourceFile`
- `project`
- `proofFragment`
  - `constructs`
- `summary`
  - `sites`
  - `typeConstraints`
  - `requires`
  - `ensures`
  - `matchArms`
  - `ifConditions`
- `sites`

Each `sites[]` entry includes:

- `kind`
  - `typeConstraint`
  - `requires`
  - `ensures`
  - `matchArm`
  - `ifCondition`
- `ownerKind`
- `ownerName`
- `location`
- optional `predicateSource` / `predicateAst`
- optional `patternSource` / `patternAst`

This is currently a proof-surface inventory, not a full solver transcript.

## Feature Flag Audit

`sigil featureFlag audit [path]` reports first-class `featureFlag` declarations
discovered under the target path.

Current command-specific fields:

- `data.input`
- `data.summary`
  - `discoveredFiles`
  - `flags`
  - `matched`
  - `olderThanDays`
- `data.flags`

Each `data.flags[]` entry currently includes:

- `name`
- `type`
- `createdAt`
- `ageDays`
- `file`
- `line`

Current filtering surface:

- `sigil featureFlag audit`
- `sigil featureFlag audit --older-than Nd`

`Nd` uses a required `d` suffix such as `180d`.

## Docs Error Codes

The docs retrieval surface currently introduces these explicit CLI codes:

- `SIGIL-CLI-DOC-NOT-FOUND`
- `SIGIL-CLI-DOC-CONTEXT-NOT-FOUND`
- `SIGIL-CLI-DOC-INVALID-LINE-RANGE`

Blank or otherwise invalid docs-command usage continues to use the normal CLI
usage surface.

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
  - optional `sigilExpression`

## Run Trace Details

`sigil run --trace` currently requires `--json`.
`sigil run --trace-expr` currently requires both `--trace` and `--json`.

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

When `--trace-expr` is enabled, trace may also include:

- `expr_enter`
- `expr_return`
- `expr_throw`

Every trace event includes:

- `seq`
- `kind`
- `depth`
- `moduleId`
- `sourceFile`
- `spanId`

Events may also include:

- declaration context such as `declarationKind` / `declarationLabel`
- `spanKind`
- `functionName`
- `args`
- `result`
- `value`
- `error`
- branch selection details such as `taken`, `armSpanId`, `armIndex`, `hasGuard`
- effect details such as `effectFamily` and `operation`

`sigilFrame` remains declaration-level context:

- it identifies the owning top-level Sigil declaration using the generated `.span.json` sidecar
- it may include a tiny declaration-header excerpt

`sigilExpression` is the exact failing expression when runtime capture can resolve it:

- it identifies the concrete expression span that threw or was active at failure time
- it may include compact `value` or `error` summaries
- it may include current-frame `locals` and stack summaries when that state is available

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
- `spanKind`
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

Current replay coverage:

- `random`
- `timer` / `time.now`
- `process`
- `http`
- `tcp`
- `file`

Replay artifacts are strict:

- bound to the original entry file
- bound to the original argv
- bound to a source-graph fingerprint
- consumed in exact recorded sequence

Standalone replay artifact schema:

- `language/spec/run-replay.schema.json`

## Test Debug Details

`sigil test` remains JSON-first and now supports the same debugging surface as
`sigil run`:

- `--trace`
- `--trace-expr` requiring `--trace`
- `--break`, `--break-fn`, `--break-span`
- `--break-mode stop|collect`
- `--break-max-hits <n>`
- `--record <artifact>`
- `--replay <artifact>`

Per-test results may now include:

- `trace`
- `breakpoints`
- `replay`
- `exception`

`status` may now be:

- `pass`
- `fail`
- `error`
- `stopped`

Stop-mode breakpoints are test-scoped:

- the current test becomes `status: "stopped"`
- the suite continues with later selected tests
- top-level `ok` becomes `false`
- `summary.stopped` counts stopped tests

`sigil test --replay` is artifact-owned like `sigil run --replay`:

- it cannot be combined with `--env`
- per-test replay uses the recorded resolved test world after local `world { ... }` overlays

Standalone test replay artifact schema:

- `language/spec/test-replay.schema.json`

## Debug Session Details

`sigil debug` is JSON-only in v1 and replay-backed:

- `sigil debug run start --replay <artifact> [--watch <selector> ...] <file>`
- `sigil debug run snapshot <session>`
- `sigil debug run step-into <session>`
- `sigil debug run step-over <session>`
- `sigil debug run step-out <session>`
- `sigil debug run continue <session>`
- `sigil debug run close <session>`
- `sigil debug test start --replay <artifact> --test <id> [--watch <selector> ...] <path>`
- `sigil debug test ... <session>` for the same control verbs

Successful debug commands return:

- `data.session`
  - `id`
  - `file`
  - `targetKind`
  - `state`
  - `replayFile`
  - `programPath` or `testPath`
  - optional `testId`
  - `watches`
- `data.snapshot`
  - `state`
  - `pauseReason`
  - `eventKind`
  - `seq`
  - current source/span/declaration context when available
  - current-frame `locals`
  - `watches`
  - stack summaries
  - bounded `recentTrace`
  - `stdoutSoFar`
  - `stderrSoFar`
  - replay progress
  - optional `lastCompleted`
  - optional `exception`

Current step events are source-shaped:

- `function_enter`
- `function_return`
- `test_enter`
- `test_return`
- `expr_enter`
- `expr_return`
- `expr_throw`
- `breakpoint`
- `program_exit`
- `test_exit`
- `uncaught_exception`

Standalone debug-session schema:

- `language/spec/debug-session.schema.json`

Current watch selector shape:

- `local`
- `local.field.subfield`

Watch roots must match current-frame locals, params, or pattern-bound names.
Nested segments only traverse record/object fields. Each watch result reports:

- `status: "ok"` with a compact summarized `value`
- `status: "not_in_scope"` when the root binding is absent
- `status: "path_missing"` when a nested field is missing or traversal leaves a record value

## Inspect World Details

`sigil inspect world` has two modes:

- project mode: `sigil inspect world <path> --env <name>`
- standalone mode: `sigil inspect world <file.sigil>`

Successful output reports:

- `input`
- `environment`
- optional `project`
- optional `projectRoot`
- `topology`
  - `present`
  - `declaredEnvs`
  - `httpDependencies`
  - `tcpDependencies`
- `summary`
  - singleton entry kinds plus HTTP/TCP binding counts
- `normalizedWorld`

`normalizedWorld` is the runtime-normalized template Sigil will use for that
environment or standalone file, not the raw exported Sigil `world` value.

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
- runtime `run` failures may include declaration-level `sigilFrame` and generated runtime-module `generatedFrame` context when an uncaught exception stack is available
- traced `run` failures may include bounded inline trace events via `error.details.trace`
- breakpoint-enabled `run` failures may include bounded snapshot data via `error.details.breakpoints`
- recorded or replayed `run` failures may include replay summary data via `error.details.replay`
- `inspect types` is top-level declaration-focused in v1; it does not report nested expression types yet
- `inspect types` now includes named type metadata, constraints, and equality mode for source-declared types; it still does not report nested expression types in v1
- `inspect validate` returns canonical printer output even when `validation.ok` is `false`, as long as lexing and parsing succeeded
- `inspect codegen` returns generated TypeScript inline for the requested file and only inventories imported modules
- `inspect world` supports project env inspection and standalone single-file inspection; non-project directory batching and test-local `world { ... }` overlays are out of scope
- `sigil test` now has a specialized result shape with `location: {line,column}`, optional per-test debug blocks, and a `stopped` status for stop-mode breakpoint hits

If prose and runtime output disagree, the implementation and
`cli-json.schema.json` are the current source of truth.
