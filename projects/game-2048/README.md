---
title: Sigil 2048
description: Browser-runnable 2048 project with Sigil board logic and a React + TypeScript UI bridge.
slug: sigil-2048
---

# Sigil 2048

This project uses the same source-first browser pattern as the other curated
Sigil demos:

- `src/game2048.lib.sigil` owns the move, merge, score, and status rules
- `tests/game2048.sigil` checks the core board transitions in Sigil
- `web/src/generated/game-2048.ts` is generated for the browser app
- `web/src/bridge.tsx` owns keyboard/buttons, random tile spawning, and rendering

Current status:

- browser-runnable demo published under `/projects/sigil-2048/demo/`
- classic `4x4` 2048 board with keyboard and on-screen controls
- score, restart, win, and lose states
- random tile spawning stays in the browser bridge while Sigil owns deterministic move logic

## Run

```bash
cd projects/game-2048
pnpm install
pnpm dev
cargo run -q -p sigil-cli --manifest-path ../../language/compiler/Cargo.toml -- test tests
```

`pnpm dev` regenerates `web/src/generated/game-2048.ts` before starting Vite.

## Build the website demo bundle

```bash
cd projects/game-2048
pnpm install
SIGIL_2048_BASE=/projects/sigil-2048/demo/ pnpm build
```
