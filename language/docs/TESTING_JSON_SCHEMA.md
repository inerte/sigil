# `sigilc test` JSON Output Schema (Format Version 1)

This document defines the machine-readable JSON emitted by:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test
```

JSON is the default output mode for Sigil tests (agent-first design).

The canonical machine contract for all CLI commands (including `sigilc test`) is now:

- `language/spec/cli-json.schema.json`

This document focuses on the `sigilc test`-specific payload fields (`summary`, `results`).

## Contract

- Stdout contains a **single JSON object**
- `formatVersion` is currently `1`
- Consumers should branch on `formatVersion`

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
    "skipped": 0,
    "durationMs": 619
  },
  "results": []
}
```

## Fields

### `formatVersion`
- Type: `number`
- Current value: `1`

### `command`
- Type: `string`
- Current value: `"sigilc test"`

### `ok`
- Type: `boolean`
- `true` if no tests failed or errored

### `summary`
- Type: object
- Aggregate counters for the run

### `results`
- Type: array of `TestResult`
- Deterministically sorted by file + source location + name

### `error` (optional)
- Present for runner/config/compiler-level failures where a normal test result list is not available
- Uses the shared `Diagnostic` object shape from `language/spec/cli-json.schema.json`

Example:
```json
{
  "error": {
    "code": "SIGIL-CANON-TEST-PATH",
    "phase": "canonical",
    "message": "test declarations are only allowed under project tests/",
    "details": {
      "file": "src/example.sigil",
      "testsRoot": "/repo/tests"
    }
  }
}
```

## `summary` Object

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
- `discovered`: total tests discovered before filtering
- `selected`: tests selected after `--match` filtering
- `passed`: tests returning `true`
- `failed`: tests returning `false`
- `errored`: tests that threw exceptions / runner execution errors at test level
- `skipped`: reserved (currently always `0`)
- `durationMs`: total wall-clock duration for the `sigilc test` command

## `TestResult` Object

```json
{
  "id": "tests/todo-domain.sigil::todo add prepends item",
  "file": "tests/todo-domain.sigil",
  "name": "todo add prepends item",
  "status": "pass",
  "durationMs": 0,
  "location": {
    "start": { "line": 29, "column": 1, "offset": 738 },
    "end": { "line": 34, "column": 2, "offset": 859 }
  },
  "declaredEffects": [],
  "assertion": {
    "kind": "comparison",
    "operator": "=",
    "left": { "location": { "start": { "line": 30, "column": 3, "offset": 756 }, "end": { "line": 30, "column": 25, "offset": 778 } } },
    "right": { "location": { "start": { "line": 30, "column": 26, "offset": 779 }, "end": { "line": 30, "column": 55, "offset": 808 } } }
  }
}
```

### Fields
- `id`: stable identifier (`<file>::<test description>`)
- `file`: source file path
- `name`: test description string
- `status`: `"pass" | "fail" | "error"`
- `durationMs`: per-test execution duration
- `location`: source range of the `test` declaration
- `declaredEffects`: effect annotations declared on the test (e.g. `["IO","Network"]`)
- `assertion` (optional): compiler-emitted metadata for recognized assertions (currently top-level comparisons)
- `failure` (optional): present for `fail` and `error`

### `assertion` Object (comparison tests)

When a test body is a top-level comparison (e.g. `a=b`, `x<y`, `a≠b`), Sigil emits assertion metadata so agents can localize the failing operands faster.

## `failure` Object

### Boolean-false failure
```json
{
  "failure": {
    "kind": "assert_false",
    "message": "Test body evaluated to false"
  }
}
```

### Structured comparison mismatch (agent-oriented)
```json
{
  "failure": {
    "kind": "comparison_mismatch",
    "message": "Comparison test failed",
    "operator": "=",
    "actual": "\"Urybo\"",
    "expected": "\"Uryyb\"",
    "diffHint": null
  }
}
```

`diffHint` may contain a shallow machine hint:
- `{ "kind": "array_length", "actualLength": 2, "expectedLength": 3 }`
- `{ "kind": "array_first_diff", "index": 4, "actual": "7", "expected": "8" }`
- `{ "kind": "object_keys", "actualKeys": ["a"], "expectedKeys": ["a","b"] }`
- `{ "kind": "object_field", "field": "done", "actual": "false", "expected": "true" }`

### Exception/error failure
```json
{
  "failure": {
    "kind": "exception",
    "message": "with_mock extern arity mismatch for extern:axios.get: expected 1, got 0"
  }
}
```

## Agent Guidance

- Parse stdout as JSON directly (no human logs mixed in default mode)
- Check `ok` first
- Use `results[].location` for targeted edits
- Use `results[].id` + `--match` for focused reruns

## Related Docs

- `docs/TESTING.md`
- `spec/cli-json.schema.json`
- `spec/cli-json.md`
- `AGENTS.md`
