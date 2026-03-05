# Sigil TODO (React + TypeScript Bridge)

This example demonstrates the recommended frontend integration pattern:

- `src/todo-domain.sigil`: canonical Sigil domain logic (Sigil project source)
- `src/todo-json.lib.sigil`: canonical Sigil JSON codec for persisted todo payloads
- `tests/todo-domain.sigil`: Sigil tests for the domain logic
- `tests/todo-json.sigil`: Sigil tests for codec roundtrip/error handling
- `web/src/generated/todo-domain.ts`: generated Sigil TypeScript output
- `web/src/bridge.tsx`: React + localStorage adapter (lintable/prettifiable TypeScript)

## Why a bridge?

Sigil stays canonical and deterministic for domain policy.
React stays idiomatic in TypeScript/JSX for UI rendering, list updates, hooks, events, and browser APIs.

## Run

```bash
pnpm install
pnpm sigil:compile
pnpm dev
cargo run -q -p sigil-cli --manifest-path ../../language/compiler/Cargo.toml -- test tests
```

## Recompile Sigil after changing the domain logic

```bash
pnpm sigil:compile
```
