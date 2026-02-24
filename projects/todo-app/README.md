# Mint TODO (React + TypeScript Bridge)

This example demonstrates the recommended frontend integration pattern:

- `src/todo-domain.mint`: canonical Mint domain logic (Mint project source)
- `tests/todo-domain.mint`: Mint tests for the domain logic
- `web/src/generated/todo-domain.ts`: generated Mint TypeScript output
- `web/src/bridge.tsx`: React + localStorage adapter (lintable/prettifiable TypeScript)

## Why a bridge?

Mint stays canonical and deterministic for domain policy.
React stays idiomatic in TypeScript/JSX for UI rendering, list updates, hooks, events, and browser APIs.

## Run

```bash
pnpm install
pnpm mint:compile
pnpm dev
node ../../language/compiler/dist/cli.js test tests
```

## Recompile Mint after changing the domain logic

```bash
pnpm mint:compile
```
