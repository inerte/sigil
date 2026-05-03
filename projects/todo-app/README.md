---
title: Sigil TODO
description: Browser-runnable TODO app with Sigil domain logic and a React + TypeScript UI bridge.
slug: sigil-todo
---

# Sigil TODO (React + TypeScript Bridge)

This example demonstrates the recommended frontend integration pattern:

- `src/todoDomain.lib.sigil`: canonical Sigil domain logic (Sigil project source)
- `src/todoJson.lib.sigil`: compiler-derived JSON codec for persisted todo payloads
- `tests/todoDomain.sigil`: Sigil tests for the domain logic
- `tests/todoJson.sigil`: Sigil tests for codec roundtrip/error handling
- `web/src/generated/todo-domain.ts`: generated Sigil TypeScript output (regenerated locally, not tracked)
- `web/src/bridge.tsx`: React + localStorage adapter (lintable/prettifiable TypeScript)

## Why a bridge?

Sigil stays canonical and deterministic for domain policy.
React stays idiomatic in TypeScript/JSX for UI rendering, list updates, hooks, events, and browser APIs.

## Run

```bash
cd projects/todo-app
pnpm install
pnpm dev
cargo run -q -p sigil-cli --no-default-features -- test tests
```

`pnpm dev` regenerates `web/src/generated/todo-domain.ts` before starting Vite.

## Recompile Sigil after changing the domain logic

```bash
cd projects/todo-app
pnpm sigil:compile
```

## Build the website demo bundle

To publish this app under the Sigil site as `/projects/sigil-todo/demo/`, build it with a matching base path:

```bash
cd projects/todo-app
pnpm install
SIGIL_TODO_BASE=/projects/sigil-todo/demo/ pnpm build
```

`pnpm build` also regenerates the Sigil bridge before producing `web/dist/`.

## Persisted JSON shape

`src/todoJson.lib.sigil` now contains one declaration:

```sigil module
derive json µPersistedState
```

That generates the persisted-state helpers in the same module:

- `encodePersistedState`
- `decodePersistedState`
- `parsePersistedState`
- `stringifyPersistedState`

The Sigil tests pin the canonical wire format: todo ids and `nextId` are JSON
numbers, records stay exact JSON objects, and invalid payloads fail through
`§decode.DecodeError`.
