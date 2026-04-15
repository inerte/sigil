# Sigil Debugging

Sigil debugging is built around machine-readable compiler and runtime surfaces.
The compiler, runner, and test harness expose the state an LLM or a human
actually needs instead of assuming a traditional IDE debugger.

Examples below use the installed `sigil` CLI. When working from a source build,
the equivalent form is:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- <args...>
```

## The Model

Sigil debugging is split into four surfaces:

- `inspect`: what the compiler or runtime setup believes
- `run`: what one program execution did
- `test`: what one test suite execution did
- `debug`: replay-backed stepping over one recorded run or one recorded test

The important design choices are:

- JSON is the primary debugging surface
- replay is the determinism backbone
- runtime failures prefer exact Sigil expression blame when available
- stepping is replay-backed rather than attached to a live debugger process
- watches, traces, and breakpoints all use compact machine-oriented summaries

## Choose The Right Command

| Question | Command |
| --- | --- |
| Did the compiler reject the source shape? | `sigil inspect validate <file-or-dir>` |
| What top-level types did the checker solve? | `sigil inspect types <file-or-dir>` |
| Which proof surfaces and branch gates exist here? | `sigil inspect proof <file-or-dir>` |
| What runtime world will this env or standalone file use? | `sigil inspect world <path> [--env <name>]` |
| What TypeScript did this compile to? | `sigil inspect codegen <file-or-dir>` |
| Where did one run fail? | `sigil run --json <file>` |
| How did execution flow? | `sigil run --json --trace <file>` |
| Which exact expression failed? | `sigil run --json <file>` and inspect `error.details.exception.sigilExpression` |
| Can I reproduce the same run exactly? | `sigil run --json --record <artifact> <file>` then `--replay <artifact>` |
| What did one test suite do? | `sigil test ...` |
| Can I replay one failing test? | `sigil test --record <artifact> ...` then `sigil debug test start --replay <artifact> --test <results[].id> ...` |
| Can I step through a recorded execution? | `sigil debug run ...` or `sigil debug test ...` |

## Inspect Surfaces

### `sigil inspect validate`

Use this when you suspect canonical-shape or source-form issues.

```bash
sigil inspect validate language/examples/genericFunctions.sigil
```

This returns:

- whether validation succeeded
- the canonical source printer output
- validation diagnostics when the source is parseable but non-canonical

Use it first for:

- declaration ordering problems
- canonical helper-surface violations
- rejected top-level shapes
- other “the compiler says this source is wrong” issues

### `sigil inspect types`

Use this when you need solved top-level types without compiling or running.

```bash
sigil inspect types language/examples/genericFunctions.sigil
```

Current scope:

- top-level declaration-focused
- useful for exported/library surfaces and entry declarations
- not a nested-expression type explorer

### `sigil inspect proof`

Use this when you need the declared proof surface without waiting for a failing
compile.

```bash
sigil inspect proof language/examples/functionContracts.sigil
```

Current scope:

- type `where` constraints
- function `requires` clauses
- function `ensures` clauses
- `match` arms and guards
- `if` conditions

This surface inventories proof sites. It does not yet replay every solver step.
Use it to answer questions like:

- where does this module introduce refinements or contracts?
- which branches participate in narrowing?
- how many proof-bearing sites exist in this file or directory?

The canonical runnable proof examples are:

- `language/examples/functionContracts.sigil`
- `language/examples/proofMeasures.sigil`

### `sigil inspect world`

Use this for topology/config/runtime-world questions.

```bash
sigil inspect world projects/topology-http --env test
```

Standalone files can also inspect a local top-level `c world` with no `--env`:

```bash
sigil inspect world path/to/file.sigil
```

This returns:

- the selected environment
- topology presence and declared dependencies
- a compact summary of singleton world entries
- the normalized runtime world template Sigil will actually use

Use it when debugging:

- missing or wrong env bindings
- HTTP/TCP dependency setup
- random/timer/log/process/fs backend differences between environments

`inspect world` has two scopes:

- project env inspection requires `--env`
- standalone single-file inspection rejects `--env`
- success output does not restate derivable canonical config/topology paths
- test-local `world { ... }` overlays are not part of this surface

### `sigil inspect codegen`

Use this when you need the emitted TypeScript or the derived output/span-map
paths without writing artifacts.

```bash
sigil inspect codegen language/examples/genericFunctions.sigil
```

This returns:

- inline generated TypeScript for the requested file
- derived `.ts` and `.span.json` paths
- span-map summary counts
- full module inventory for the resolved compile graph

It is useful when a runtime trace or exception already points into generated
code and you need to understand the emitted shape.

## Runtime Debugging With `sigil run`

### Baseline JSON

Use:

```bash
sigil run --json <file>
```

This gives one structured success or failure envelope. On runtime failures,
inspect:

- top-level `phase`
- top-level `error.code`
- `error.location`
- `error.details.runtime`
- `error.details.exception.sigilFrame`
- `error.details.exception.sigilExpression`

`sigilExpression` is the precise runtime-blame surface:

- exact expression span
- exact source location
- compact error/value summary
- current-frame locals and stack when available

### Trace

Use:

```bash
sigil run --json --trace <file>
sigil run --json --trace --trace-expr <file>
```

Rules:

- `--trace` requires `--json`
- `--trace-expr` requires both `--trace` and `--json`

`--trace` adds bounded inline events such as:

- `call`
- `return`
- `branch_if`
- `branch_match`
- `effect_call`
- `effect_result`

`--trace-expr` adds:

- `expr_enter`
- `expr_return`
- `expr_throw`

Use ordinary trace first. Add expression trace only when you need finer control
flow or value flow.

### Breakpoints

Use:

```bash
sigil run --json --break <file:line> <file>
sigil run --json --break-fn <name> <file>
sigil run --json --break-span <id> <file>
sigil run --json --break-fn helper --break-mode collect --break-max-hits 8 <file>
```

Rules:

- breakpoint selectors require `--json`
- `stop` mode pauses the run early and returns `ok: true`
- `collect` mode keeps running and returns bounded hit snapshots

Each hit includes:

- resolved span id and span kind
- source location
- declaration context
- current-frame locals
- stack summaries
- recent trace window

### Record And Replay

Use:

```bash
sigil run --json --record .local/run.replay.json <file>
sigil run --json --replay .local/run.replay.json <file>
```

Replay is strict and artifact-owned:

- `--record` and `--replay` are mutually exclusive
- `--replay` cannot be combined with `--env`
- replay is bound to the recorded entry path, argv, and source fingerprint

Current replay coverage:

- `random`
- `timer` and `time.now`
- `process`
- `http`
- `tcp`
- `file`

Replay artifacts preserve enough information to reproduce:

- successful runs
- child exits
- exact file-operation failures
- recorded effect ordering

Use replay whenever:

- a run is flaky
- an effectful run is expensive to recreate
- you want stepping or repeated breakpoint sessions over one exact execution

## Test Debugging With `sigil test`

`sigil test` is already JSON-first. Use it directly for suite-level debugging:

```bash
sigil test --trace projects/algorithms/tests/basicTesting.sigil
sigil test --break-fn helper projects/algorithms/tests/basicTesting.sigil
sigil test --record .local/tests.replay.json projects/algorithms/tests/basicTesting.sigil
```

Current debug-capable test flags include:

- `--trace`
- `--trace-expr`
- `--break`
- `--break-fn`
- `--break-span`
- `--break-mode stop|collect`
- `--break-max-hits <n>`
- `--record <artifact>`
- `--replay <artifact>`

Test-specific behavior:

- stop-mode breakpoints stop only the current test
- a stopped test result uses `status: "stopped"`
- the suite continues with later selected tests
- `sigil test --replay` cannot be combined with `--env`
- replay artifacts store the resolved per-test world after local `world { ... }` overlays

Per-test results may include:

- `trace`
- `breakpoints`
- `replay`
- `exception`

When replaying a failing test, use the exact stable `results[].id` later with
`sigil debug test start`.

## Replay-Backed Debug Sessions With `sigil debug`

Stepping is replay-backed. There is no long-lived debugger process.

### Debug One Run

```bash
sigil run --json --record .local/run.replay.json app.sigil
sigil debug run start --replay .local/run.replay.json --watch user.score --watch result.value app.sigil
sigil debug run snapshot .local/debug/<session>.json
sigil debug run step-into .local/debug/<session>.json
sigil debug run step-over .local/debug/<session>.json
sigil debug run step-out .local/debug/<session>.json
sigil debug run continue .local/debug/<session>.json
sigil debug run close .local/debug/<session>.json
```

### Debug One Exact Test

```bash
sigil test --record .local/tests.replay.json projects/algorithms/tests/basicTesting.sigil
sigil debug test start --replay .local/tests.replay.json --test "projects/algorithms/tests/basicTesting.sigil::cache hit returns cached value" --watch result.value projects/algorithms/tests/basicTesting.sigil
sigil debug test continue .local/debug/<session>.json
```

Rules:

- `sigil debug` is JSON-only
- `start` currently requires `--replay`
- `start` may also preload repeatable `--break`, `--break-fn`, and `--break-span` selectors
- `sigil debug test --test <id>` requires one exact `results[].id`
- watch selectors are `local(.field)*`
- `snapshot` reads the current stored session state without advancing execution
- `step-into` advances to the next source-level pause point
- `step-over` stays at the current frame when possible
- `step-out` runs until the current frame returns or throws
- `continue` runs until the next breakpoint, failure, or normal exit
- `close` ends the session and removes the session file

Every debug snapshot includes:

- current state and pause reason
- source file, span id, span kind, and location
- declaration or test context
- current-frame locals
- stack summaries
- recent trace
- replay progress
- stdout/stderr so far
- ordered `watches`

Watches are session-scoped and recomputed on every pause:

- `ok`: selector resolved successfully
- `not_in_scope`: root local is not visible in the current frame
- `path_missing`: a later field is missing or traversal hit a non-record value

## Worked Workflow

### 1. Compiler Or Canonical Failure

Use:

```bash
sigil inspect validate src/main.sigil
sigil inspect types src/main.sigil
```

Do not start from emitted TypeScript or runtime behavior if the source does not
pass canonical validation and typechecking first.

### 2. Runtime Crash

Use:

```bash
sigil run --json --trace src/main.sigil
```

Read in this order:

1. `phase`
2. `error.code`
3. `error.location`
4. `error.details.exception.sigilExpression`
5. `error.details.trace.events`

If the failure looks effectful or flaky, record it next:

```bash
sigil run --json --trace --record .local/crash.replay.json src/main.sigil
sigil run --json --trace --replay .local/crash.replay.json src/main.sigil
```

### 3. Topology Or Config Problem

Use:

```bash
sigil inspect world . --env test
```

Check:

- declared dependency handles
- singleton backend kinds
- normalized runtime world entries

Only move to `run` once the selected env world looks correct.

### 4. Step Through A Recorded Run

Use:

```bash
sigil run --json --record .local/run.replay.json src/main.sigil
sigil debug run start --replay .local/run.replay.json --watch state.count src/main.sigil
sigil debug run step-into .local/debug/<session>.json
sigil debug run step-over .local/debug/<session>.json
```

This is the right path when:

- a trace is too dense
- you need to compare state across pauses
- you want one stable watched value across several steps

### 5. Debug One Failing Test

Use:

```bash
sigil test --record .local/tests.replay.json tests/
```

Then take the exact `results[].id` for the failing test and start:

```bash
sigil debug test start --replay .local/tests.replay.json --test "<results[].id>" --watch result.value tests/
```

This lets you step only that test's recorded execution without rerunning the
whole suite in a live mode.

## Human + LLM Workflow

The intended workflow is the same for a person and an LLM, but the machine
surfaces mean the LLM can stay precise instead of guessing from prose.

If the question is about the Sigil language surface itself rather than one
execution, start with `sigil docs ...` instead of a debug command. Use
`sigil docs context --list` to discover curated bundles, or `sigil docs search`
to jump to a specific syntax, stdlib, or package topic.

A good loop is:

1. run one machine-readable command first
2. branch on `phase`
3. inspect the smallest relevant surface next
4. replay if the behavior may vary
5. step only after the run is deterministic

For an LLM, that usually means:

1. `sigil run --json --trace ...` or `sigil test ...`
2. inspect the returned code, phase, location, and exact expression
3. use `inspect world` or `inspect codegen` only if the failure suggests it
4. record/replay before suggesting fixes for effectful behavior
5. use watches to pin the few values that matter across steps

For a human, the same rule applies:

- prefer one structured run over ad hoc logging first
- use replay instead of hoping the same bad run happens again
- use debug sessions when you need state transitions, not just one failure site

## Related References

- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
- `language/spec/testing.md`
- `language/spec/run-replay.schema.json`
- `language/spec/test-replay.schema.json`
- `language/spec/debug-session.schema.json`
- `language/docs/TESTING.md`
- `language/docs/TESTING_JSON_SCHEMA.md`
