# Mint Testing (First-Class)

Mint tests are language declarations, not a separate framework.

## Canonical layout

- Tests must live under `./tests`
- `test` declarations outside `./tests` are compile errors
- Test files may include regular functions/types/constants

## Test syntax

```mint
test "adds numbers" {
  1+1=2
}
```

- Test body must evaluate to `𝔹`
- `⊤` passes, `⊥` fails

## Effectful tests

Use explicit effect annotations on tests (same model as functions):

```mint
test "writes log" →!IO {
  console.log("x")=()  ⟦ body still must be boolean ⟧
}
```

## Built-in mocking (scoped)

Mocks are explicit, lexical, and automatically restored.

- Allowed targets:
  - `extern` members (e.g. `axios.get`)
  - Mint functions marked `mockable`
- Not allowed:
  - pure functions
  - non-`mockable` Mint functions

### `mockable` adapter function

```mint
mockable λfetchUser(id:ℤ)→!Network 𝕊="real"
```

### `with_mock`

```mint
test "fallback on API failure" →!Network {
  with_mock(fetchUser, λ(id:ℤ)→!Network 𝕊="ERR") {
    fetchUser(1)="ERR"
  }
}
```

## CLI

JSON is the default output mode (agent-first).
Test files run in parallel by default (results are sorted deterministically in final output).

```bash
# Run all tests in ./tests
node language/compiler/dist/cli.js test

# Run a file or subdirectory under ./tests
node language/compiler/dist/cli.js test projects/algorithms/tests/basic-testing.mint

# Filter by test description substring
node language/compiler/dist/cli.js test --match "cache"

# Human-readable output
node language/compiler/dist/cli.js test --human
```

## JSON output (default)

`mintc test` prints a single JSON object to stdout with:

- `formatVersion`
- `ok`
- `summary`
- `results[]`

Each result includes:

- `id`
- `file`
- `name`
- `status` (`pass`, `fail`, `error`)
- `durationMs`
- `location`
- `declaredEffects`
- `failure` (for failures/errors)

Formal schema:
- `docs/TESTING_JSON_SCHEMA.md` (`formatVersion: 1`)
