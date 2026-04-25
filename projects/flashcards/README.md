---
title: Sigil Flashcards
description: Browser-runnable flashcards project for Sigil feature selection, debugging, and architecture concepts, with direct links to docs, spec, and articles.
slug: sigil-flashcards
---

# Sigil Flashcards

This project is a curated study deck for the kinds of things humans ask when
directing Codex or Claude Code on Sigil projects.

It is intentionally not syntax trivia. The cards focus on:

- when to use `topology`, `world`, and tests
- how to choose between structural and nominal boundaries
- where canonical helper surfaces and trusted boundaries matter
- which docs, spec pages, or articles explain a concept in depth

Project structure:

- `src/flashcardsDomain.lib.sigil` owns card content, topic filtering, and study-session state
- `tests/flashcards.sigil` checks the session logic in Sigil
- `web/src/generated/flashcards-domain.ts` is generated for the browser app
- `web/src/bridge.tsx` renders the deck and links answer-side references into the website

## Run

```bash
cd projects/flashcards
pnpm install
pnpm dev
cargo run -q -p sigil-cli --no-default-features -- test tests
```

`pnpm dev` regenerates `web/src/generated/flashcards-domain.ts` before starting Vite.

## Build the website demo bundle

```bash
cd projects/flashcards
pnpm install
SIGIL_FLASHCARDS_BASE=/projects/sigil-flashcards/demo/ pnpm build
```
