---
title: Worlds, Not Mocks
date: 2026-03-23
author: Sigil Language Team
slug: worlds-not-mocks
---

# Worlds, Not Mocks

Worlds allow Sigil to specify how effects behave in a given context.

Sigil treats execution itself as world-dependent:

- effects stay explicit in function signatures
- topology declares dependency identity
- config builds one environment's runtime world
- tests may derive that world locally

## The Model

In Sigil, functions declare which effects they may use:

- `!Clock`
- `!Fs`
- `!Http`
- `!Log`
- `!Process`
- `!Random`
- `!Tcp`
- `!Timer`

Those effect names stay static. What changes across environments is the world
that interprets them.

That gives a cleaner split:

- topology declares what external dependencies exist
- config selects one environment's world
- the world says how effects behave in that environment
- tests may derive the world locally for one test

The language roots now reflect that split:

- `†...` builds runtime worlds and world entries
- `※observe::...` reads raw traces from the active test world
- `※check::...` exposes Bool helpers for tests

## Why This Is Better Than Mocking

Traditional mocking encourages a function-level mental model:

- replace this helper
- spy on that call
- assert this stub was used

Sigil wants a runtime model instead:

- code should always call code it depends on, unless it affects the outside world
- effects touch the world
- tests inspect what the world observed

That keeps the substitution boundary aligned with Sigil's effect system.
`Fs` stays `Fs`. `Http` stays `Http`. A test does not invent new capabilities;
it runs the same code in a different world.

This encourages higher-level tests (also known as integration or end-to-end tests). The only swappable part are effects.

## Config Exports `world`

The environment contract is now explicit and uniform:

```sigil module
c world=(†runtime.world(
  †clock.systemClock(),
  †fs.real(),
  †fsWatch.real(),
  [],
  †log.stdout(),
  †process.real(),
  †pty.real(),
  †random.real(),
  †sql.deny(),
  †stream.live(),
  †task.real(),
  [],
  †timer.real(),
  †websocket.real()
):†runtime.World)
```

Every environment config module exports `world`. There is no optional fallback,
because Sigil should not have to guess whether a given environment participates
in the runtime contract.

For topology-aware projects, that world includes one HTTP/TCP entry per
declared dependency handle. For ordinary projects, it still defines the
primitive effect behavior for that environment.

## Tests Derive Worlds

`sigil test --env test` starts from the `world` value exported by
`config/test.lib.sigil`.

A single test can then derive from that baseline:

```sigil program tests/worldTest.sigil
λmain()=>Unit=()

test "captured log contains line" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("captured"):Unit);
  ※check::log.contains("captured")
}
```

That one clause does three jobs:

- it states the local runtime override
- it gives the test a name for the installed world entry
- it keeps the test inside the same effect system as production code

The result is more coherent than `withMock(...)`, because the language is no
longer pretending that tests replace arbitrary functions. Tests change the
world instead.

## Observation, Not Spies

Tests still need ergonomics. A different world is only useful if the test can
inspect what happened inside it.

That is why the test surface is split in two:

- `※observe` returns raw recorded data
- `※check` returns Bool helpers over that data

For example:

- `※observe::http.requests(...)`
- `※observe::log.entries()`
- `※check::http.calledOnce(...)`
- `※check::log.contains(...)`

Those are not generic mock assertions. They are observations over recorded
effect traces.

## Coverage Fits the Same Model

Once execution is world-based, test coverage can be phrased at the public
surface instead of at lines or branches.

The current `sigil test` rule is:

- every project `src/*.lib.sigil` function must be executed by the suite
- sum-returning project functions must observe each relevant output variant
- full suite or directory runs enforce this gate
- focused single-file runs skip the project-wide gate so iteration can stay local

This is a much better fit for Sigil than line coverage. The unit of obligation
is the function contract and its output shape, not whether every implementation
detail flipped a counter.

## Result

Sigil now treats runtime behavior the same way in tests, local development, and
production: code runs in a world, and the world is explicit.
