---
title: Sigil Minesweep
description: Browser-runnable Minesweep project with Sigil game logic and a React + TypeScript UI bridge.
slug: sigil-minesweep
---

# Sigil Minesweep

This project follows the same integration pattern as the Todo app:

- `src/minesweepDomain.lib.sigil` owns the board state transitions
- `tests/minesweepDomain.sigil` keeps the core rules checked in Sigil
- `web/src/generated/minesweep-domain.ts` is generated for the browser app
- `web/src/bridge.tsx` owns the React UI and browser event wiring

Current status:

- browser-runnable demo published under `/projects/sigil-minesweep/demo/`
- deterministic fixed board so the project is stable in CI and on GitHub Pages
- source-first domain logic with room to grow into a richer game later

## Run

```bash
cd projects/minesweep
pnpm install
pnpm dev
cargo run -q -p sigil-cli --manifest-path ../../language/compiler/Cargo.toml -- test tests
```

`pnpm dev` regenerates `web/src/generated/minesweep-domain.ts` before starting Vite.

## Build the website demo bundle

```bash
cd projects/minesweep
pnpm install
SIGIL_MINESWEEP_BASE=/projects/sigil-minesweep/demo/ pnpm build
```

## CLI preview

If you only want the textual board preview:

```bash
cd projects/minesweep
cargo run -q -p sigil-cli --manifest-path ../../language/compiler/Cargo.toml -- run src/main.sigil
```
