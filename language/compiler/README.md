# Sigil Compiler

This directory contains the only active Sigil compiler implementation. It is a Rust workspace with one CLI crate and supporting frontend/codegen crates.

## Layout

```text
compiler/
├── Cargo.toml
├── Cargo.lock
├── crates/
│   ├── sigil-ast/
│   ├── sigil-cli/
│   ├── sigil-codegen/
│   ├── sigil-diagnostics/
│   ├── sigil-lexer/
│   ├── sigil-parser/
│   ├── sigil-typechecker/
│   └── sigil-validator/
└── ERROR_CODES.md
```

## Common Commands

From the repo root:

```bash
# Build the compiler
cargo build -p sigil-cli --no-default-features

# Compile a file
cargo run -q -p sigil-cli --no-default-features -- compile projects/algorithms/src/fibonacci.sigil

# Compile a directory recursively
cargo run -q -p sigil-cli --no-default-features -- compile projects/algorithms --ignore .git --ignore-from .gitignore

# Run a file
cargo run -q -p sigil-cli --no-default-features -- run projects/algorithms/src/fibonacci.sigil

# Run tests
cargo run -q -p sigil-cli --no-default-features -- test projects/algorithms/tests

# Run a crate test suite
cargo test -p sigil-parser --no-default-features
```

CI and the root `pnpm` scripts use system Z3 with `--no-default-features`; install `z3` and `pkg-config` locally for that path. Omitting `--no-default-features` enables vendored Z3, which avoids a system dependency but makes a fresh build slower.

The repo root has a Cargo workspace shim for worktree-friendly commands. The original workspace manifest remains at `language/compiler/Cargo.toml`, so existing commands with `--manifest-path language/compiler/Cargo.toml` still work.

For automation, prefer `cargo run` or `pnpm sigil ...` so scripts continue to work when `CARGO_TARGET_DIR` is set. If you already built the default debug profile from the repo root and are not using a custom target directory, the binary is at:

```bash
target/debug/sigil compile projects/algorithms/src/fibonacci.sigil
target/debug/sigil compile . --ignore .git --ignore-from .gitignore
```

## Notes

- `sigil-cli` is the command entrypoint.
- The compiler emits TypeScript/JavaScript output, but the compiler implementation itself is Rust-only.
- Some project/example inputs still fail under the Rust compiler; those are language/compiler gaps, not alternative compiler paths.
