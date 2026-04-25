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

# Run the compiler test suite with the same Cargo feature flags used in CI
pnpm sigil:test:compiler

# Compile a file through the root convenience wrapper
pnpm sigil compile projects/algorithms/src/fibonacci.sigil

# Run Sigil tests in the algorithms example project
pnpm sigil:test:algorithms

# Run Sigil tests in the todo-app Sigil domain
pnpm sigil:test:todo

# Build the website into website/.local/site
pnpm sigil:build:website
```

## Fresh Worktree Setup

Fresh git worktrees do not share generated build artifacts with your main checkout unless you explicitly share a Cargo target directory. From a new worktree, use:

```bash
# Rust is pinned by rust-toolchain.toml.
rustup show

# Node 24 is pinned for humans and matches CI.
corepack enable
corepack prepare pnpm@10.0.0 --activate
pnpm install --frozen-lockfile

# CI-style compiler check.
pnpm sigil:test:compiler
```

The repo root now has a Cargo workspace shim, so `cargo test -p sigil-typechecker` works from the repo root. The original compiler workspace at `language/compiler/Cargo.toml` still works for commands that use `--manifest-path`.

CI and root `pnpm` scripts use system Z3 with Cargo `--no-default-features`. Install it locally before running those scripts:

```bash
# macOS
brew install z3 pkg-config

# Debian/Ubuntu
sudo apt-get install libz3-dev pkg-config
```

If you do not have system Z3 installed, direct Cargo commands without `--no-default-features` use the vendored Z3 feature instead, but the first build is much slower.

For multiple worktrees, you can share build outputs with `CARGO_TARGET_DIR=/path/to/shared/target`; otherwise each worktree gets its own cold `target/`.

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

If Claude Code, Codex, or another assistant is starting from a fresh Sigil install,
bootstrap it from the binary itself:

```bash
sigil help
sigil docs context --list
sigil docs context overview
sigil docs search "syntax reference"
```

`sigil docs ...` ships an embedded local corpus of guides, language docs, specs,
grammar, and design articles. That gives assistants a version-matched Sigil
knowledge surface immediately, without depending on web search or model priors.

If you are contributing to the compiler itself, build from source instead:

```bash
corepack enable
corepack prepare pnpm@10.0.0 --activate
pnpm install --frozen-lockfile
pnpm build
pnpm sigil help
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
