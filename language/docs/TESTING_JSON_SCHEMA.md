# `sigilc test` JSON Output Schema (Format Version 1)

This document describes the current machine-readable JSON emitted by:

```bash
cargo run -q -p sigil-cli --no-default-features -- test
```

JSON is the default output mode for Sigil tests.
For the higher-level debugging workflow and command selection guide, see
`language/docs/DEBUGGING.md`.

The normative shared schema lives at:

- `language/spec/cli-json.schema.json`

This page focuses on the current `test`-specific envelope shape.

## Contract

- stdout contains a single JSON object
- `formatVersion` is currently `1`
- consumers should branch on `formatVersion`

## Top-Level Shape

```json
{
  "formatVersion": 1,
  "command": "sigilc test",
  "ok": true,
  "summary": {
    "files": 4,
    "discovered": 13,
    "selected": 13,
    "passed": 13,
    "failed": 0,
    "errored": 0,
    "stopped": 0,
    "skipped": 0,
    "durationMs": 619
  },
  "results": []
}
```

## Fields

### `formatVersion`

- type: `number`
- current value: `1`

### `command`

- type: `string`
- current value: `"sigilc test"`

### `ok`

- type: `boolean`
- `true` when no tests failed or errored

### `summary`

- aggregate counters for the run

### `results`

- array of per-test results
- sorted deterministically by file, then location, then name

### `error` (optional)

- present for runner/config/compiler-level failures where a normal test list is
  not available
- uses the shared diagnostic envelope shape

## `summary`

```json
{
  "files": 4,
  "discovered": 13,
  "selected": 13,
  "passed": 13,
  "failed": 0,
  "errored": 0,
  "skipped": 0,
  "durationMs": 619
}
```

Field meanings:

- `files`: number of `.sigil` test files executed
- `discovered`: tests discovered before filtering
- `selected`: tests selected after `--match`
- `passed`: tests that returned `true`
- `failed`: tests that returned `false`
- `errored`: tests that threw at runtime
- `stopped`: tests intentionally halted by stop-mode breakpoints
- `skipped`: reserved, currently `0`
- `durationMs`: total wall-clock duration

## `TestResult`

Current aggregated result shape:

```json
{
  "id": "tests/todoDomain.sigil::todo add prepends item",
  "file": "tests/todoDomain.sigil",
  "name": "todo add prepends item",
  "status": "pass",
  "durationMs": 0,
  "location": {
    "line": 29,
    "column": 1
  }
}
```

Fields:

- `id`: stable identifier `<file>::<test description>`
- `file`: source file path
- `name`: test description string
- `name` may contain newline characters when the source test description is multiline
- `status`: `"pass" | "fail" | "error"`
- `status`: `"pass" | "fail" | "error" | "stopped"`
- `durationMs`: per-test execution duration
- `location`: current aggregated location object with `line` and `column`
- `failure` (optional): present for `fail` and `error`
- `trace` (optional): bounded inline trace data for that test
- `breakpoints` (optional): bounded inline breakpoint hit data for that test
- `replay` (optional): record/replay summary data for that test
- `exception` (optional): exact runtime exception context for errored tests

Stop-mode breakpoint hits are represented as ordinary test results:

```json
{
  "status": "stopped",
  "breakpoints": {
    "enabled": true,
    "mode": "stop",
    "stopped": true
  }
}
```

Current output does not include:

- `declaredEffects`
- structured `assertion` metadata

## `failure`

### Boolean-false failure

```json
{
  "failure": {
    "kind": "assert_false",
    "message": "Test body evaluated to false"
  }
}
```

### Runtime exception

```json
{
  "failure": "Fs is denied by the current world",
  "exception": {
    "name": "Error",
    "message": "Fs is denied by the current world",
    "sigilExpression": {
      "kind": "expr_extern_call"
    }
  }
}
```

## Agent Guidance

- parse stdout as JSON directly
- check `ok` first
- use `results[].id` with `--match` for focused reruns
- use `results[].location` for targeted edits

## Related Docs

- `language/docs/DEBUGGING.md`
- `language/docs/TESTING.md`
- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
- `language/spec/test-replay.schema.json`
- `language/spec/debug-session.schema.json`

## Replay-Backed Stepping

`sigil test` replay artifacts can now drive `sigil debug test` sessions:

- `sigil debug test start --replay <artifact> --test <results[].id> [--watch <selector> ...] <path>`
- `sigil debug test snapshot <session>`
- `sigil debug test step-into <session>`
- `sigil debug test step-over <session>`
- `sigil debug test step-out <session>`
- `sigil debug test continue <session>`
- `sigil debug test close <session>`

Those commands use the shared debug envelope documented in `cli-json.md`.
Each debug snapshot also includes ordered `watches` results for any configured
`--watch local(.field)*` selectors.
