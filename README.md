# Mint Monorepo

This repo contains three distinct things:

- `language/` — the Mint language implementation (compiler, stdlib, docs, tools)
- `projects/` — canonical Mint projects and examples
- `website/` — the Mint website (GitHub Pages target)

## Start Here

- Language/compiler docs: `language/README.md`
- Pure Mint example project: `projects/algorithms/`
- React + Mint bridge example: `projects/todo-app/`

## Common Commands

```bash
# Build the compiler
pnpm build

# Run Mint tests in the algorithms example project
pnpm mint:test:algorithms

# Run Mint tests in the todo-app Mint domain
pnpm mint:test:todo
```

## Notes

- Mint user projects should use the canonical layout: `mint.json`, `src/`, `tests/` (and optional `web/`)
- This monorepo mixes language implementation and projects intentionally, but the user-facing layout is demonstrated under `projects/`
