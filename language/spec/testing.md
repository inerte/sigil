# Sigil Testing Specification

Version: 1.0.0
Last Updated: 2026-03-14

## Overview

Sigil tests are first-class declarations in the language.

Current implemented testing surface includes:

- top-level `test "name" { ... }`
- optional explicit test effects
- built-in `withMock(...) { ... }`
- CLI test discovery and execution

This spec describes the implemented system in the current compiler, not older
design ideas such as TDAI, semantic maps, coverage mode, or generated tests.

## Test Declaration

Canonical surface:

```sigil
test "adds numbers" {
  1+1=2
}
```

Effectful test:

```sigil
test "writes log" =>!IO {
  console.log("x")=()
}
```

Rules:

- test description is a string literal
- test body is an expression block
- test body must evaluate to `Bool`
- `true` means pass
- `false` means fail

## Test Location

`test` declarations are only valid in files under `tests/`.

Test files are ordinary `.sigil` files and may also declare:

- `λ`
- `c`
- `t`
- `i`

Test files are executable-oriented and must define `main`.

## Mocking

### `withMock`

Current built-in mocking form:

```sigil
test "fallback on API failure" =>!Network {
  withMock(fetchUser, λ(id:Int)=>!Network String="ERR") {
    fetchUser(1)="ERR"
  }
}
```

Allowed targets:

- extern members
- any Sigil function

Placement rule:

- `withMock(...)` is only valid directly inside `test` declaration bodies

## CLI Surface

Current user-facing command shape:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test
```

Common modes:

```bash
# all tests in current project tests/
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test

# one file or directory
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests

# filter by name substring
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test --match "cache"

```

For topology-aware projects:

- `--env <name>` is required

## JSON Output

Default test output is JSON.

Current top-level shape:

- `formatVersion`
- `command`
- `ok`
- `summary`
- `results`
- optional `error`

Per-test results currently include:

- `id`
- `file`
- `name`
- `status`
- `durationMs`
- `location`
- optional `failure`

Current output does not include:

- `declaredEffects`
- assertion metadata
- coverage data

Normative references:

- `language/docs/TESTING_JSON_SCHEMA.md`
- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
