# Sigil Testing Specification

Version: 1.1.1
Last Updated: 2026-03-24

## Overview

Sigil tests are first-class declarations in the language.
For the user-facing debugging workflow across `inspect`, `run`, `test`, replay,
and stepping, see `language/docs/DEBUGGING.md`.

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

```sigil program language/examples/addsNumbers.sigil
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

```sigil program language/examples/testWorld.sigil
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

`sigil.json` is the mode switch for test placement.

- in project mode, `test` declarations are only valid under `tests/`
- in standalone mode, `test` declarations may live in any ordinary `.sigil` file

Test files are ordinary `.sigil` files and may also declare:

- `λ`
- `c`
- `e`

In projects with `sigil.json`, project-defined named types still live in
`src/types.lib.sigil` and are referenced in tests through `µ...`.

Test-capable files are executable-oriented and must define `main`.

## Runtime Worlds

Sigil tests run inside a compiler-owned runtime world.

Baseline world:

- selected from `config/<env>.lib.sigil`
- exported as `c world=(...:†runtime.World)`
- the same config module may also expose selected env declarations through
  `•config.<name>`, for example `•config.flags`
- standalone files may instead provide a local top-level `c world=(...:†runtime.World)` with no selected env

Test-local derivation:

- `test ... world { ... } { ... }` overlays entries onto the selected env world
- singleton entries such as `†clock.*`, `†log.*`, or `†random.*` replace that kind
- topology-indexed entries such as `†fs.*Root`, `†fsWatch.*Root`, `†http.*`, `†process.*Handle`, `†pty.*Handle`, `†tcp.*`, and `†websocket.*Handle` replace by named boundary handle

Observation surface:

- `※observe::...` exposes raw traces
- `※check::...` exposes Bool helpers over those traces

Canonical example helpers include:

- `※observe::http.requests`
- `※observe::file.readTextAt`
- `※observe::fsWatch.eventsAt`
- `※observe::fsWatch.watchesAt`
- `※observe::log.entries`
- `※observe::log.entriesAt`
- `※observe::pty.spawnsAt`
- `※observe::pty.writesAt`
- `※observe::process.commandsAt`
- `※observe::websocket.receivedAt`
- `※observe::websocket.sentAt`
- `※check::http.calledOnce`
- `※check::file.existsAt`
- `※check::file.textEqualsAt`
- `※check::fsWatch.closedAt`
- `※check::fsWatch.watchingAt`
- `※check::log.contains`
- `※check::log.containsAt`
- `※check::pty.closedAt`
- `※check::pty.spawnedOnceAt`
- `※check::process.calledOnceAt`
- `※check::websocket.connectedOnceAt`
- `※check::websocket.sentAt`

For topology-aware labelled-boundary projects, these helpers are the canonical
testing surface. They assert the effect observed at the exact named boundary
rather than inferring it from ambient global state.

## CLI Surface

Current user-facing command shape:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test
```

Common modes:

```bash
# all tests in the current project tests/ directory
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test

# the self-testing language examples
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test language/examples

# one file or directory
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests

# filter by name substring
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test --match "cache"

```

For runtime-world projects, and for projects that read selected config
declarations such as `•config.flags`:

- `--env <name>` is required

For non-project directories, `sigil test <dir>` recursively finds `.sigil`
files and runs any embedded `test` declarations it discovers.

`sigil test` also enforces project source coverage:

- every function in project `src/*.lib.sigil` must be executed by the suite
- sum-returning project functions must observe each relevant output variant
- missing surface coverage is reported as ordinary failing test results
- suite-style runs (`sigil test`, `sigil test path/to/tests/`) enforce this gate
- focused single-file runs (`sigil test path/to/tests/file.sigil`) skip the project-wide coverage gate
- standalone non-project runs do not have a project coverage gate

## JSON Output

The machine contract is shared with `language/spec/cli-json.md`. This section
keeps the testing-specific semantics and result model.

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
  - `pass | fail | error | stopped`
- `durationMs`
- `location`
- optional `failure`
- optional `trace`
- optional `breakpoints`
- optional `replay`
- optional `exception`

`summary` also includes:

- `stopped`

When a test description contains newlines, current JSON output preserves those
newline characters in `id`, `name`, and `description`.

Debugger notes:

- `sigil test` supports `--trace`, `--trace-expr`, breakpoints, `--record`, and `--replay`
- stop-mode breakpoints stop only the current test and report `status: "stopped"`
- `sigil test --replay` is artifact-owned and cannot be combined with `--env`
- `sigil debug test` replays one exact `results[].id` through a file-backed stepping session
- debug sessions are JSON-only and currently support `snapshot`, `step-into`, `step-over`, `step-out`, `continue`, and `close`
- `sigil debug test start` also accepts repeatable `--watch local(.field)*` selectors and reports compact watch results on every snapshot

Current output does not include:

- `declaredEffects`
- assertion metadata
- raw coverage traces

Normative references:

- `language/docs/TESTING_JSON_SCHEMA.md`
- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
