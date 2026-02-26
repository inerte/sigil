# Sigil Compiler Migration: TypeScript → Rust

## Overview

This document tracks the migration of the Sigil compiler from TypeScript (Node.js) to Rust. The goal is to achieve a high-performance, portable, single-binary compiler while maintaining 100% compatibility with the existing TypeScript implementation.

## Motivation

### Why Rust?

1. **Performance**: 10-100x faster compilation for large codebases
   - Critical for AI coding agents (like Claude Code) that need fast feedback loops
   - Sigil is designed as a machine-first language for AI code generation
   - Faster compilation = tighter AI development loops = more productive AI-assisted coding

2. **Portability**: Single-binary distribution
   - No Node.js/npm runtime dependency
   - Easy installation: `curl -O` and run
   - Smaller footprint: ~5MB binary vs. 50MB+ Node.js installation

3. **Type Safety**: Rust's ownership system and type guarantees
   - Prevents entire classes of bugs at compile time
   - Memory safety without garbage collection
   - Thread safety for potential parallel compilation

4. **Production Quality**: Zero-cost abstractions
   - Performance comparable to hand-written C++
   - Robust toolchain with excellent error messages
   - Growing ecosystem and community

### Why Not Just Keep TypeScript?

The TypeScript compiler works well, but:
- **Runtime Dependency**: Users must install Node.js (50MB+)
- **Performance**: V8 JIT overhead, GC pauses
- **Distribution**: npm packages are awkward for CLI tools
- **Memory**: Node.js base memory usage (~50MB) before compiler runs

## Migration Strategy

### Approach: Full Rewrite

We're doing a **full rewrite** rather than incremental migration because:

1. **Clean Separation**: Rust and TypeScript have fundamentally different idioms
2. **Performance**: Can optimize from scratch without TS constraints
3. **Testing**: Can run both compilers in parallel for differential testing
4. **Risk Mitigation**: TypeScript compiler remains canonical until Rust is proven

### Compatibility Guarantee

The Rust compiler will maintain **exact behavioral compatibility**:

- Same token stream from lexer
- Same AST structure from parser
- Same type inference results
- Same error messages (same locations, same text)
- Same generated JavaScript/TypeScript output (byte-for-byte)

If output differs, it's a bug unless explicitly documented as an improvement.

## Progress Tracking

### Phase 1: Foundation ✅ (80% Complete)

**Goal**: Set up infrastructure and core data structures

- [x] Cargo workspace structure
- [x] AST crate (`sigil-ast`)
  - [x] All declaration types
  - [x] All expression types (27 variants)
  - [x] All pattern types (7 variants)
  - [x] Type system nodes
  - [x] Source location tracking
- [x] Lexer crate (`sigil-lexer`)
  - [x] 99 token types
  - [x] Unicode symbol support
  - [x] String/char literals
  - [x] Multi-line comments
  - [x] Error handling
  - [x] Unit tests (5 passing)

**Remaining**:
- [ ] Differential lexer tests against TypeScript
- [ ] Performance benchmarks

### Phase 2: Parsing 🚧 (Not Started)

**Goal**: Build AST from token stream

- [ ] Parser crate (`sigil-parser`)
  - [ ] Recursive descent parser
  - [ ] All declaration parsing
  - [ ] All expression parsing
  - [ ] Pattern parsing
  - [ ] Error recovery
  - [ ] Location tracking
- [ ] Unit tests for each AST node type
- [ ] Differential parser tests (AST equality)
- [ ] Performance benchmarks

**Estimated Duration**: 1 week

### Phase 3: Validation 🚧 (Not Started)

**Goal**: Enforce canonical form rules

- [ ] Validator crate (`sigil-validator`)
  - [ ] Canonical form validator
  - [ ] Surface form validator
  - [ ] Extern/FFI validator
  - [ ] Helpful error messages with fixits
- [ ] Unit tests for validation rules
- [ ] Differential validation tests
- [ ] Performance benchmarks

**Estimated Duration**: 1 week

### Phase 4: Type Checking 🚧 (Not Started)

**Goal**: Bidirectional type inference with effect tracking

- [ ] Type checker crate (`sigil-typechecker`)
  - [ ] Hindley-Milner unification
  - [ ] Bidirectional type inference (⇒ and ⇐ modes)
  - [ ] Type environments (using `im::HashMap`)
  - [ ] Effect tracking (IO, Network, Async, Error, Mut)
  - [ ] Module-level inference
  - [ ] Type error messages
- [ ] Unit tests for unification
- [ ] Differential type checking tests
- [ ] Performance benchmarks

**Estimated Duration**: 2 weeks

### Phase 5: Code Generation 🚧 (Not Started)

**Goal**: Generate TypeScript output (byte-for-byte compatible)

- [ ] Code generator crate (`sigil-codegen`)
  - [ ] Async transformation (all functions → async)
  - [ ] Pattern matching compilation
  - [ ] Sum type constructors (__tag/__fields)
  - [ ] Mock runtime helpers (__sigil_mocks, etc.)
  - [ ] Test metadata export (__sigil_tests)
  - [ ] Comment preservation
- [ ] Unit tests for each codegen pattern
- [ ] **Critical**: Differential codegen tests (byte-for-byte)
- [ ] Performance benchmarks

**Estimated Duration**: 2 weeks

### Phase 6: CLI & Integration 🚧 (Not Started)

**Goal**: Command-line interface and end-to-end compilation

- [ ] CLI crate (`sigil-cli`)
  - [ ] `sigil compile <file>`
  - [ ] `sigil run <file>`
  - [ ] `sigil test <directory>`
  - [ ] `sigil typecheck <file>`
  - [ ] `sigil parse <file>`
  - [ ] Module resolution
  - [ ] Test runner (JSON and human output)
  - [ ] Exit codes
- [ ] Integration tests (full compilation pipeline)
- [ ] Differential CLI tests
- [ ] Performance benchmarks

**Estimated Duration**: 1 week

### Phase 7: Polish & Release 🚧 (Not Started)

**Goal**: Production-ready release

- [ ] Diagnostics crate (`sigil-diagnostics`)
  - [ ] Beautiful error messages (codespan-reporting)
  - [ ] Colored terminal output
  - [ ] JSON error output (for tooling)
- [ ] Cross-platform builds
  - [ ] Linux x86-64
  - [ ] Linux ARM64
  - [ ] macOS Intel
  - [ ] macOS Apple Silicon
  - [ ] Windows x86-64
- [ ] GitHub Actions CI/CD
- [ ] Binary releases
- [ ] Documentation
  - [ ] Migration guide
  - [ ] Performance comparison
  - [ ] Website article
- [ ] Deprecation notice for TypeScript compiler

**Estimated Duration**: 1 week

## Testing Strategy

### Unit Tests

Each crate has its own unit tests:

```bash
cargo test -p sigil-ast
cargo test -p sigil-lexer
cargo test -p sigil-parser
# ... etc
```

**Coverage Goal**: 80%+ per crate

### Integration Tests

End-to-end tests at workspace level:

```bash
cargo test --test integration
```

Tests:
- Full compilation pipeline (lex → parse → validate → typecheck → codegen)
- Error handling across phases
- Module resolution and imports
- FFI/extern validation

### Differential Tests (Critical!)

Run both compilers on the same input and compare outputs:

```bash
cargo test --test differential
```

**Test Corpus** (18+ files):
- `language/examples/*.sigil` (8 files)
- `language/test-fixtures/**/*.sigil`
- `projects/algorithms/src/*.sigil`
- `projects/todo-app/src/*.sigil`

**Comparisons**:
1. **Lexer**: Token stream equality
2. **Parser**: AST structure equality (ignoring location differences)
3. **Type Checker**: Inferred types equality
4. **Code Generator**: Generated .ts files (byte-for-byte comparison)
5. **CLI**: Exit codes and output format

**Acceptance Criteria**: 100% differential test pass rate

### Performance Benchmarks

Track performance against TypeScript compiler:

```bash
cargo bench
```

**Metrics**:
- Lexing speed (tokens/sec)
- Parsing speed (AST nodes/sec)
- Type checking speed (LOC/sec)
- Codegen speed (LOC/sec)
- End-to-end compilation time
- Memory usage (peak RSS)

**Target**: 10x+ faster than TypeScript across all phases

## Risk Mitigation

### Risk: Output Divergence

**Mitigation**:
- Differential testing on every commit
- Side-by-side comparison tools
- TypeScript compiler remains canonical until 100% pass rate

### Risk: Type System Complexity

**Mitigation**:
- Use TypeScript implementation as reference
- Extensive unit tests for unification
- Gradual rollout (typecheck-only mode first)

### Risk: Breaking User Workflows

**Mitigation**:
- Keep TypeScript compiler until Rust version is proven
- Provide migration guide
- Support both compilers for 1-2 releases
- Compatibility shim if needed

### Risk: Platform-Specific Bugs

**Mitigation**:
- CI/CD testing on all platforms
- Cross-compilation with `cross`
- Manual testing on each platform before release

## Success Criteria

✅ **Correctness**: All differential tests pass (100%)
✅ **Performance**: 10x+ faster than TypeScript
✅ **Distribution**: Single-binary releases for 5 platforms
✅ **Tests**: 80%+ coverage, zero failures
✅ **Documentation**: Migration guide, benchmarks, website article

## Timeline

| Phase | Duration | Start | End |
|-------|----------|-------|-----|
| Phase 1: Foundation | 2 weeks | Week 1 | Week 2 ✅ (80%) |
| Phase 2: Parsing | 1 week | Week 3 | Week 3 🚧 |
| Phase 3: Validation | 1 week | Week 4 | Week 4 🚧 |
| Phase 4: Type Checking | 2 weeks | Week 5 | Week 6 🚧 |
| Phase 5: Code Generation | 2 weeks | Week 7 | Week 8 🚧 |
| Phase 6: CLI & Integration | 1 week | Week 9 | Week 9 🚧 |
| Phase 7: Polish & Release | 1 week | Week 10 | Week 10 🚧 |
| **Total** | **10 weeks** | | |

**Current Status**: Week 2, Phase 1 (80% complete)

## Current Focus

### This Week (Phase 1)

- [x] Set up Cargo workspace
- [x] Implement AST crate
- [x] Implement lexer crate
- [ ] Write differential lexer tests
- [ ] Benchmark lexer performance

### Next Week (Phase 2)

- [ ] Implement parser crate
- [ ] Write parser unit tests
- [ ] Write differential parser tests
- [ ] Benchmark parser performance

## Open Questions

1. **Unicode Normalization**: Should we normalize Unicode input (e.g., NFD vs. NFC)?
   - TypeScript doesn't normalize
   - Decision: Match TypeScript (no normalization)

2. **Parallel Compilation**: Should we support multi-threaded compilation?
   - Not in initial release (complexity)
   - Consider for v2.0 if needed

3. **LSP Support**: Should Rust compiler power the LSP server?
   - Yes, eventually (Phase 8)
   - Keep TypeScript LSP for now

4. **WebAssembly**: Should we compile to WASM for browser use?
   - Not a priority for v1.0
   - Interesting for future (web playground)

## Resources

- **Plan Document**: `/language/compiler-rs/PLAN.md` (detailed technical plan)
- **TypeScript Compiler**: `/language/compiler/` (reference implementation)
- **Language Spec**: `/language/spec/` (formal specification)
- **Examples**: `/language/examples/` (test corpus)

## Contact

For questions or concerns about the migration:

1. Open a GitHub issue with `[migration]` tag
2. Tag @language-team in discussions
3. Check weekly migration status in team meetings

---

**Last Updated**: 2026-02-26
**Next Review**: 2026-03-05 (after Phase 2 completion)
