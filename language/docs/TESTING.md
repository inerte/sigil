# Sigil Testing

Sigil tests are first-class language declarations, not a separate test framework.

Repo-level integration tests are ordinary Sigil test files under
`language/integrationTests/tests/`. They run through the same `sigil test`
machinery as project tests rather than through separate shell launchers.

## Canonical Layout

- tests live under `tests/`
- `test` declarations outside `tests/` are canonical errors
- test files are ordinary `.sigil` files
- test files may include helpers alongside `test` declarations

Application/library code should live under `src/` and be imported from tests with
normal Sigil imports.

## Importing Real Modules

Library code is file-based, not `export`-based:

```sigil module projects/todo-app/src/todoDomain.lib.sigil
t Todo={done:Bool,id:Int,text:String}

λcompletedCount(todos:[Todo])=>Int=todos⊕(λ(acc:Int,todo:Todo)=>Int match todo.done{
  true=>acc+1|
  false=>acc
})⊕0
```

```sigil program projects/todo-app/tests/todoDomain.sigil
i src::todoDomain

λmain()=>Unit=()

test "count completed todos" {
  src::todoDomain.completedCount([{done:true,id:1,text:"A"},{done:false,id:2,text:"B"}])=1
}
```

## Test Syntax

```sigil program tests/basic.sigil
λmain()=>Unit=()

test "adds numbers" {
  1+1=2
}
```

Rules:

- test body must evaluate to `Bool`
- `true` passes
- `false` fails

Effectful tests use explicit effects:

```sigil program language/test-fixtures/tests/effects.sigil
e console

λmain()=>Unit=()

test "writes log" =>!IO  {
  console.log("x")=()
}
```

## Mocking

Sigil includes built-in lexical mocking.

Allowed targets:

- extern members
- any Sigil function

Placement rule:

- `withMock(...)` is only valid directly inside `test` declaration bodies

Example:

```sigil program language/test-fixtures/tests/mocking.sigil
λfetchUser(id:Int)=>!Network String="real"

λmain()=>Unit=()

test "fallback on API failure" =>!Network  {
  withMock(fetchUser,λ(id:Int)=>!Network String="ERR"){fetchUser(1)="ERR"}
}
```

## CLI

Default output mode is JSON.

Examples:

```bash
# Run all tests in the current project tests/ directory
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test

# Run a specific file or subdirectory
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests/basicTesting.sigil

# Filter by test name substring
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test --match "cache"

```

For topology-aware projects, `--env <name>` is required.

For process-heavy harness code, prefer:
- `stdlib::process` for child processes
- `stdlib::file.makeTempDir` for scratch workspaces
- `stdlib::time.sleepMs` for retry loops

## JSON Output

`test` emits a single JSON object to stdout by default.

Top-level fields:

- `formatVersion`
- `command`
- `ok`
- `summary`
- `results`

Each result currently includes:

- `id`
- `file`
- `name`
- `status`
- `durationMs`
- `location`
- `failure` when the test fails or errors

Current aggregated test output does not include:

- `declaredEffects`
- structured `assertion` metadata

Formal references:

- `language/docs/TESTING_JSON_SCHEMA.md`
- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
