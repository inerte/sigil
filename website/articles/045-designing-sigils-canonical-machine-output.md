---
title: "Designing Sigil's Canonical Machine Output"
date: 2026-07-10
author: Sigil Language Team
slug: designing-sigils-canonical-machine-output
---

# Designing Sigil's Canonical Machine Output

Sigil is designed for agents to write and inspect, but its original command-line
interface did not apply the language's canonicality principle consistently to
compiler output. Most commands returned JSON, yet they used several related
envelopes. Tests placed results at the top level, ordinary commands used
`data`, failures used one `error` object, and some command identifiers still
referred to the old `sigilc` binary name.

That variation made every agent integration carry command-specific parsing
logic. It also made partial analysis difficult to represent honestly. A review
that fell back to parsing could return useful facts, but consumers had to know
where that limitation happened to be recorded.

Sigil now uses one machine envelope for every JSON-producing command.

## One Contract

```json
{
  "formatVersion": 1,
  "compilerVersion": "2026-07-10T15-00-00Z",
  "command": "sigil inspect trust",
  "ok": true,
  "phase": "typecheck",
  "analysis": {
    "status": "complete",
    "level": "typed"
  },
  "data": {},
  "diagnostics": []
}
```

The fields have deliberately narrow meanings. `ok` reports whether an error
diagnostic exists. `phase` identifies the phase responsible for the primary
result or failure. `analysis` says how far analysis completed. `data` is always
an object, and `diagnostics` is always an ordered array.

The contract keeps `formatVersion` at `1`. Sigil has no external output
consumers yet, so preserving several incompatible pre-user shapes would create
permanent complexity without protecting anyone. Once external consumers exist,
incompatible changes will require a new version.

## Correction Needs Structured Evidence

An agent should not need to extract a proof obligation from prose. A failed
call can report the goal, available assumptions, relevant source spans, and a
stable error code directly. Diagnostics also distinguish a mechanically safe
edit from one that requires semantic review.

```json
{
  "code": "SIGIL-PROOF-REQUIRES",
  "phase": "proof",
  "severity": "error",
  "message": "the call precondition could not be proven",
  "details": {
    "assumptions": ["count>=0"],
    "goal": "count>0"
  },
  "fixits": []
}
```

This supports a bounded correction loop: generate, validate, inspect the exact
failure, make one justified change, and validate again. Source token density is
only one part of agent efficiency; avoiding speculative repair calls matters as
well.

## Partial Analysis Is Not Failure Without Information

Directory commands analyze independent module groups even when one group
fails. They retain facts supported by completed phases, return all ordered
diagnostics, and mark the overall analysis `partial`. A parse-only declaration
is never presented as a typed declaration.

This distinction matters for semantic review and trust inspection. `absent`,
`unchanged`, and `not established` are different statements. The output model
keeps them different.

## Inspecting Trust

`sigil inspect trust` reports compiler-owned trust facts in categories:

- assumptions, including extern declarations and axiomatic protocol transitions
- controls, including boundary policies, topology boundaries, and derived codec validation
- dependencies from the project manifest and lockfile
- runtime context, including declaration effect requirements

Each fact carries an evidence level. The command does not guess that a function
is a decoder because of its name, and it does not treat a parsed declaration as
proof that a typed call path uses it.

The report complements `sigil review`. Review explains how a patch changes the
semantic surface; trust inspection explains which guarantees ultimately depend
on runtime implementations or declared assumptions.

## Human Modes Remain Explicit

Canonical machine output does not require wrapping every byte in JSON. Plain
`sigil run` still streams program output. `sigil review` still has a compact
human report, and `sigil review --llm` still produces grounded prompt text.
Their structured facts come from the same internal result used by JSON mode.

The rule is narrower: whenever Sigil promises machine-readable output, there
is one contract and one meaning for its fields.

## Tradeoffs

The common envelope adds a few fields to small results. That overhead buys
uniform routing, explicit evidence levels, stable diagnostics, and schema tests
that apply to every command. Volatile timings are excluded from the canonical
contract so repeated invocations remain comparable. Execution traces retain
their semantic event order; unordered inventories are sorted.

Canonical source reduces ambiguity when agents write Sigil. Canonical output
reduces ambiguity when agents decide what to do next.
