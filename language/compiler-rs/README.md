# Sigil Compiler (Rust Implementation)

This directory contains the Rust implementation of the Sigil programming language compiler. The goal is to provide a high-performance, portable, single-binary compiler that maintains exact compatibility with the TypeScript implementation.

## Project Status

### ✅ Phase 1: Foundation (In Progress)

- [x] **Cargo Workspace Setup**: Multi-crate workspace structure with proper dependency management
- [x] **AST Crate (`sigil-ast`)**: Complete AST definitions matching TypeScript implementation
  - All declaration types (Function, Type, Import, Const, Test, Extern)
  - All expression types (27 variants including Lambda, Match, Application, etc.)
  - All pattern types (7 variants for pattern matching)
  - Type system nodes (Primitive, List, Map, Function, etc.)
  - Source location tracking for all nodes
- [x] **Lexer Crate (`sigil-lexer`)**: Complete lexer implementation
  - 99 token types matching TypeScript exactly
  - Unicode symbol support (λ, →, ≡, ⋅, ∧, ∨, ¬, etc.)
  - Canonical formatting enforcement (no tabs, precise whitespace)
  - String and character literal parsing with escape sequences
  - Multi-line comment support (⟦...⟧)
  - Comprehensive error handling with source locations
  - 5 passing unit tests

### 🚧 Next Steps

- [ ] **Parser Crate (`sigil-parser`)**: Recursive descent parser
- [ ] **Validator Crate (`sigil-validator`)**: Canonical form validation
- [ ] **Type Checker Crate (`sigil-typechecker`)**: Bidirectional type inference
- [ ] **Code Generator Crate (`sigil-codegen`)**: TypeScript output generation
- [ ] **Diagnostics Crate (`sigil-diagnostics`)**: Beautiful error messages
- [ ] **CLI Crate (`sigil-cli`)**: Command-line interface

## Architecture

### Crate Structure

```
compiler-rs/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── sigil-ast/            # ✅ AST definitions (pure data structures)
│   ├── sigil-lexer/          # ✅ Lexer (tokenization)
│   ├── sigil-parser/         # 🚧 Parser (AST construction)
│   ├── sigil-validator/      # 🚧 Canonical form validation
│   ├── sigil-typechecker/    # 🚧 Bidirectional type inference
│   ├── sigil-codegen/        # 🚧 JavaScript/TypeScript code generation
│   ├── sigil-diagnostics/    # 🚧 Error reporting and fixits
│   └── sigil-cli/            # 🚧 CLI binary (sigil compile/run/test)
└── tests/                    # Integration tests
```

### Design Principles

1. **Exact TypeScript Compatibility**: Output must be byte-for-byte identical (or intentional improvements documented)
2. **Performance First**: Leverage Rust's zero-cost abstractions for 10-100x speedup
3. **Single Binary Distribution**: No runtime dependencies (vs. Node.js/npm for TypeScript version)
4. **Crate Isolation**: Clear separation of concerns, minimal circular dependencies
5. **Comprehensive Testing**: Unit tests per crate, integration tests at workspace level

## Building and Testing

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo

### Build All Crates

```bash
cargo build
```

### Build Specific Crate

```bash
cargo build -p sigil-ast
cargo build -p sigil-lexer
```

### Run Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p sigil-lexer

# With output
cargo test -- --nocapture
```

### Run Examples

```bash
# Lexer example
cargo run --example lex_example

# Parser example (coming soon)
cargo run --example parse_example
```

## Dependencies

### Workspace-Wide

- **logos** (0.13): Fast lexer generator with macros
- **thiserror** (2.0): Error derive macros for clean error types
- **clap** (4.4): CLI argument parsing with derive macros
- **tokio** (1.35): Async runtime (for CLI execution)
- **codespan-reporting** (0.11): Professional error messages
- **im** (15.1): Immutable data structures (for type environments)
- **serde** (1.0): Serialization (optional, for JSON output)

## Performance Goals

### Target Metrics (vs. TypeScript Compiler)

| Operation | TypeScript (Node.js) | Rust (Target) | Speedup |
|-----------|---------------------|---------------|---------|
| Lexing    | ~10ms for 1000 LOC | <1ms | 10x+ |
| Parsing   | ~50ms for 1000 LOC | <5ms | 10x+ |
| Type Checking | ~100ms for 1000 LOC | <10ms | 10x+ |
| Code Generation | ~20ms for 1000 LOC | <2ms | 10x+ |
| **Total** | **~180ms** | **<20ms** | **10x+** |

### Memory Usage

- **TypeScript**: ~50MB base (Node.js runtime) + compiler overhead
- **Rust**: <10MB for compiler binary, <5MB runtime memory

## Compatibility Testing

### Differential Testing Strategy

For every change, we run differential tests against the TypeScript compiler:

```bash
# Compare lexer output
cargo test --test differential_lexer

# Compare parser AST
cargo test --test differential_parser

# Compare codegen output (byte-for-byte)
cargo test --test differential_codegen

# Run all differential tests
cargo test --test differential
```

### Test Corpus

- `language/examples/*.sigil` (8 example files)
- `language/test-fixtures/**/*.sigil` (comprehensive test cases)
- `projects/algorithms/src/*.sigil` (real-world code)
- `projects/todo-app/src/*.sigil` (real-world code)

## Cross-Platform Builds

### Target Platforms

- `x86_64-unknown-linux-gnu` (Linux x86-64)
- `aarch64-unknown-linux-gnu` (Linux ARM64)
- `x86_64-apple-darwin` (macOS Intel)
- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-pc-windows-msvc` (Windows x86-64)

### Building for All Platforms

```bash
# Install cross (for cross-compilation)
cargo install cross

# Build for all targets
./scripts/build-all-targets.sh

# Output: target/{platform}/release/sigil
```

## Release Process

1. **Version Bump**: Update version in `Cargo.toml` files
2. **Run Tests**: `cargo test --all`
3. **Run Differential Tests**: Ensure 100% compatibility with TypeScript compiler
4. **Build Binaries**: Cross-compile for all platforms
5. **Create GitHub Release**: Upload binaries as release artifacts
6. **Update Documentation**: Update README, website, migration guide

## Migration Timeline

| Phase | Duration | Status |
|-------|----------|--------|
| Phase 1: Foundation (AST + Lexer) | 1-2 weeks | ✅ In Progress (80%) |
| Phase 2: Parsing | 1 week | 🚧 Not Started |
| Phase 3: Validation | 1 week | 🚧 Not Started |
| Phase 4: Type Checking | 2 weeks | 🚧 Not Started |
| Phase 5: Code Generation | 2 weeks | 🚧 Not Started |
| Phase 6: CLI & Integration | 1 week | 🚧 Not Started |
| Phase 7: Polish & Release | 1 week | 🚧 Not Started |
| **Total** | **8-10 weeks** | **10% Complete** |

## Contributing

### Code Style

- Follow Rust 2021 edition conventions
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write tests for all public APIs
- Document public functions with `///` doc comments

### Testing Requirements

- Unit tests for each module
- Integration tests for cross-crate functionality
- Differential tests against TypeScript compiler
- 80%+ code coverage per crate

### Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Write code with tests
4. Run `cargo test` and `cargo clippy`
5. Submit PR with description of changes
6. Ensure CI/CD passes (all tests + differential tests)

## Resources

- **TypeScript Compiler**: `../compiler/` (reference implementation)
- **Language Spec**: `../spec/` (formal language specification)
- **Examples**: `../examples/` (canonical Sigil code)
- **Website**: `../../website/` (documentation and guides)

## License

MIT License - See `../../LICENSE` for details

## Contact

For questions or issues, please open a GitHub issue or contact the Sigil language team.

---

**Note**: This is an active migration project. The TypeScript compiler (`../compiler/`) remains the authoritative implementation until the Rust compiler reaches feature parity and passes all differential tests.
