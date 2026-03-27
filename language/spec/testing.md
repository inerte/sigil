# Sigil Testing Specification

Version: 1.1.1
Last Updated: 2026-03-24

## Overview

Sigil tests are first-class declarations in the language.

Current implemented testing surface includes:

- top-level `test "name" { ... }`
- optional explicit test effects
- optional `world { ... }` clause on tests
- compiler-owned `†` and `※` roots
- CLI test discovery and execution

This spec describes the implemented system in the current compiler, not older
design ideas such as TDAI, semantic maps, coverage mode, or generated tests.

## Test Declaration

Canonical surface:

```sigil program tests/sampleTest.sigil
λmain()=>Unit=()

test "adds numbers" {
  1+1=2
}
```

Effectful test:

```sigil program tests/effectTest.sigil
λmain()=>Unit=()

test "writes log" =>!Log {
  l _=(§io.println("x"):Unit);
  true
}
```

World-derived test:

```sigil program tests/worldTest.sigil
λmain()=>Unit=()

test "captured log contains line" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("captured"):Unit);
  ※check::log.contains("captured")
}
```

Rules:

- test description is a string literal
- test descriptions may span lines; the newline becomes part of the description
- test body is an expression block
- test body must evaluate to `Bool`
- `true` means pass
- `false` means fail
- test-local `world { ... }` is declaration-only and contains `c` bindings of `†...` entry values
- test-local world bindings are visible only inside that test body

## Test Location

`test` declarations are only valid in files under `tests/`.

Test files are ordinary `.sigil` files and may also declare:

- `λ`
- `c`
- `e`

In projects with `sigil.json`, project-defined named types still live in
`src/types.lib.sigil` and are referenced in tests through `µ...`.

Test files are executable-oriented and must define `main`.

## Runtime Worlds

Sigil tests run inside a compiler-owned runtime world.

Baseline world:

- selected from `config/<env>.lib.sigil`
- exported as `c world=(...:†runtime.World)`

Test-local derivation:

- `test ... world { ... } { ... }` overlays entries onto the selected env world
- singleton entries such as `†clock.*` or `†log.*` replace that kind
- topology-indexed `†http.*` and `†tcp.*` replace by dependency handle

Observation surface:

- `※observe::...` exposes raw traces
- `※check::...` exposes Bool helpers over those traces

Canonical example helpers include:

- `※observe::http.requests`
- `※observe::log.entries`
- `※check::http.calledOnce`
- `※check::log.contains`

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

For runtime-world projects:

- `--env <name>` is required

`sigil test` also enforces project source coverage:

- every function in project `src/*.lib.sigil` must be executed by the suite
- sum-returning project functions must observe each relevant output variant
- missing surface coverage is reported as ordinary failing test results
- suite-style runs (`sigil test`, `sigil test path/to/tests/`) enforce this gate
- focused single-file runs (`sigil test path/to/tests/file.sigil`) skip the project-wide coverage gate

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

When a test description contains newlines, current JSON output preserves those
newline characters in `id`, `name`, and `description`.

Current output does not include:

- `declaredEffects`
- assertion metadata
- raw coverage traces

Normative references:

- `language/docs/TESTING_JSON_SCHEMA.md`
- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
