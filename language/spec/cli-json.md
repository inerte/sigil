# Sigil CLI JSON Contract

Sigil CLI commands are machine-first. JSON is the default output mode for:

- `sigilc lex`
- `sigilc parse`
- `sigilc compile`
- `sigilc inspect types`
- `sigilc inspect validate`
- `sigilc inspect world`
- `sigilc test`
- `sigilc` usage/unknown-command failures

`sigilc run` is split:

- plain `sigil run <file>` streams raw program stdout/stderr on success
- plain `sigil run <file>` emits structured JSON on failure
- `sigil run --json <file>` emits the structured JSON envelope on both success and failure
- `sigil run --json --trace <file>` adds a bounded inline execution trace to that envelope

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
`sigilc inspect types`, `sigilc inspect validate`, and `sigilc inspect world` use inspect-specific envelopes.
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

## Current Notes

The current implementation uses:

- `"sigilc ..."` strings in JSON `command` fields
- successful `compile` output reports `.span.json` sidecars via `rootSpanMap` and per-module `spanMapFile`
- successful `run --json` output reports the entry module `.span.json` sidecar via `data.compile.spanMapFile`
- successful `run --json --trace` output reports inline bounded trace events via `data.trace`
- runtime `run` failures may include declaration-level `sigilFrame` and generated TypeScript `generatedFrame` context when an uncaught exception stack is available
- traced `run` failures may include bounded inline trace events via `error.details.trace`
- `inspect types` is top-level declaration-focused in v1; it does not report nested expression types yet
- `inspect validate` returns canonical printer output even when `validation.ok` is `false`, as long as lexing and parsing succeeded
- `inspect world` is project-level in v1; it does not batch over directories or include test-local `world { ... }` overlays
- a specialized `test` result shape with `location: {line,column}`

If prose and runtime output disagree, the implementation and
`cli-json.schema.json` are the current source of truth.
