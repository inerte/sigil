# Router

`projects/router` is the first publishable Sigil package example.

It is intentionally small and opinionated:

- ordered exact-path route matching
- explicit `matched`, `methodNotAllowed`, and `notFound` outcomes
- canonical package entrypoint in `src/package.lib.sigil`

The project exists to exercise the package system, not to be a full web framework.

Run its tests from the repo root:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/router/tests
```

Publish it with the new package command:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- package publish
```
