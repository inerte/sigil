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
├── ERROR_CODES.md
└── test-multimodule.sh
```

## Common Commands

From the repo root:

```bash
# Build the compiler
cargo build --manifest-path language/compiler/Cargo.toml -p sigil-cli

# Compile a file
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile language/examples/fibonacci.sigil

# Run a file
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run language/examples/fibonacci.sigil

# Run tests
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests

# Run a crate test suite
cargo test --manifest-path language/compiler/Cargo.toml -p sigil-parser
```

For automation after the build:

```bash
language/compiler/target/debug/sigil compile language/examples/fibonacci.sigil
```

## Notes

- `sigil-cli` is the command entrypoint.
- The compiler emits TypeScript/JavaScript output, but the compiler implementation itself is Rust-only.
- Some project/example inputs still fail under the Rust compiler; those are language/compiler gaps, not alternative compiler paths.
