---
title: Sigil Minesweep
description: Browser-runnable 6x6 Minesweep project with Sigil game logic, a React + TypeScript UI bridge, and 6 randomized mines by default.
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
- randomized `6x6` board on every load and restart with `6` mines by default
- zero-adjacent reveals cascade open the surrounding safe region
- source-first domain logic with room to grow into a richer game later

## Run

```bash
cd projects/minesweep
pnpm install
pnpm dev
cargo run -q -p sigil-cli --no-default-features -- test tests
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
cargo run -q -p sigil-cli --no-default-features -- run src/main.sigil
```
