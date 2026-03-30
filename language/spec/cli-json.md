# Sigil CLI JSON Contract

Sigil CLI commands are machine-first. JSON is the default output mode for:

- `sigilc lex`
- `sigilc parse`
- `sigilc compile`
- `sigilc inspect types`
- `sigilc inspect validate`
- `sigilc test`
- `sigilc` usage/unknown-command failures

`sigilc run` is split:

- plain `sigil run <file>` streams raw program stdout/stderr on success
- plain `sigil run <file>` emits structured JSON on failure
- `sigil run --json <file>` emits the structured JSON envelope on both success and failure

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
`sigilc inspect types` and `sigilc inspect validate` use inspect-specific envelopes.
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

## Current Notes

The current implementation uses:

- `"sigilc ..."` strings in JSON `command` fields
- successful `compile` output reports `.span.json` sidecars via `rootSpanMap` and per-module `spanMapFile`
- successful `run --json` output reports the entry module `.span.json` sidecar via `data.compile.spanMapFile`
- `inspect types` is top-level declaration-focused in v1; it does not report nested expression types yet
- `inspect validate` returns canonical printer output even when `validation.ok` is `false`, as long as lexing and parsing succeeded
- a specialized `test` result shape with `location: {line,column}`

If prose and runtime output disagree, the implementation and
`cli-json.schema.json` are the current source of truth.
