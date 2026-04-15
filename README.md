# Sigil Monorepo

This repo contains three distinct things:

- `language/` — the Sigil language implementation (compiler, stdlib, docs, tools)
- `projects/` — canonical Sigil projects and examples
- `website/` — the Sigil website (GitHub Pages target)

## Start Here

- **Website**: [inerte.github.io/sigil](https://inerte.github.io/sigil/)
- Language/compiler docs: `language/README.md`
- Pure Sigil example project: `projects/algorithms/`
- React + Sigil bridge example: `projects/todo-app/`

## Common Commands

```bash
# Build the compiler
pnpm build

# Compile a file through the root convenience wrapper
pnpm sigil -- compile projects/algorithms/src/fibonacci.sigil

# Run Sigil tests in the algorithms example project
pnpm sigil:test:algorithms

# Run Sigil tests in the todo-app Sigil domain
pnpm sigil:test:todo

# Build the website into website/.local/site
language/compiler/target/debug/sigil run projects/ssg/src/main.sigil
```

## Installation

Sigil is distributed as a native CLI binary through GitHub Releases.

- Download the archive for your platform from the latest release
- Extract `sigil`
- Put it on your `PATH`
- Run `sigil --version`
- Install Node.js if you want to use runtime-backed commands such as `sigil run`, `sigil test`, `sigil validate`, `sigil inspect world`, or `sigil debug ...`

Release versions use canonical UTC timestamps in the format `YYYY-MM-DDTHH-mm-ssZ`.

Homebrew packaging is generated from those release artifacts in `projects/homebrewPackaging` and mirrored through a separate tap repo when configured. The generated formula declares `node` as a runtime dependency. The release tarballs remain the source of truth.

Create a new standalone Sigil project with:

```bash
mkdir hello-sigil
cd hello-sigil
sigil init
```

`sigil init` creates a neutral project root with `sigil.json`, `src/`, `tests/`, and `.local/`.
Add `src/main.sigil` later if the project should be runnable, or add `src/package.lib.sigil`
plus `publish` later if it should be publishable as a package.

If you are contributing to the compiler itself, build from source instead:

```bash
pnpm install
pnpm build
./language/compiler/target/debug/sigil --version
```

## Notes

- Root `pnpm` scripts are convenience wrappers around the Rust compiler.
- `pnpm test` is for JS/workspace tests that exist; Sigil test runs are the explicit `sigil:test:*` scripts.
- Sigil user projects use canonical `src/`, `tests/`, and `.local/`; `sigil.json` marks the project root and must declare a lowerCamel `name` plus a UTC timestamp `version` in `YYYY-MM-DDTHH-mm-ssZ`.
- `sigil init [path]` scaffolds only that common project baseline; runnable and publishable surfaces are added later.
- Direct package dependencies are exact-only in `sigil.json`; publishable packages also require `src/package.lib.sigil` plus `publish`.
- Package workflows live under `sigil package ...`, with npm used only as the transport/publish registry.
- This monorepo mixes language implementation and projects intentionally, but the user-facing layout is demonstrated under `projects/`

## Website

The public site is built by `projects/ssg` and published from `website/.local/site` through GitHub Pages. The source markdown remains in `website/`, `language/docs/`, and `language/spec/`; the site generator renders those files directly so the repo does not carry a second docs tree.
