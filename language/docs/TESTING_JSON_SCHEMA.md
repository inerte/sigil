# `mintc test` JSON Output Schema (Format Version 1)

This document defines the machine-readable JSON emitted by:

```bash
node language/compiler/dist/cli.js test
```

JSON is the default output mode for Mint tests (agent-first design).

## Contract

- Stdout contains a **single JSON object**
- `formatVersion` is currently `1`
- Consumers should branch on `formatVersion`

## Top-Level Shape

```json
{
  "formatVersion": 1,
  "command": "mintc test",
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
- Current value: `"mintc test"`

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

Example:
```json
{
  "error": {
    "kind": "runner_error",
    "message": "mintc test only accepts paths under ./tests. Got: src"
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
- `files`: number of `.mint` test files executed
- `discovered`: total tests discovered before filtering
- `selected`: tests selected after `--match` filtering
- `passed`: tests returning `⊤`
- `failed`: tests returning `⊥`
- `errored`: tests that threw exceptions / runner execution errors at test level
- `skipped`: reserved (currently always `0`)
- `durationMs`: total wall-clock duration for the `mintc test` command

## `TestResult` Object

```json
{
  "id": "tests/todo-domain.mint::todo add prepends item",
  "file": "tests/todo-domain.mint",
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

When a test body is a top-level comparison (e.g. `a=b`, `x<y`, `a≠b`), Mint emits assertion metadata so agents can localize the failing operands faster.

## `failure` Object

### Boolean-false failure
```json
{
  "failure": {
    "kind": "assert_false",
    "message": "Test body evaluated to ⊥"
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
- `AGENTS.md`
