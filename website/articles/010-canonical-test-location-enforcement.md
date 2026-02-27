---
title: "Canonical Test Location: Enforcing ONE Way for Test Files"
date: February 27, 2026
author: Sigil Language Team
slug: 010-canonical-test-location-enforcement
---

# Canonical Test Location: Enforcing ONE Way for Test Files

**TL;DR:** Sigil now enforces that test blocks can ONLY appear in files under `tests/` directories, and test files MUST have a `main()` function. This completes Sigil's canonical file purpose enforcement: every `.sigil` file is either an executable OR a library (and tests are executables with test blocks).

## The Problem: Tests Everywhere

Before this change, Sigil allowed `test` blocks anywhere:

```sigil
// examples/fibonacci.sigil
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}

test "fibonacci works" {  // Allowed but not canonical
  fibonacci(5)=5
}
```

This violated Sigil's "ONE WAY" philosophy:

- Tests scattered throughout the codebase
- Unclear file purpose (example? test? both?)
- Non-canonical organization

For a language designed for AI code generation, this ambiguity was unacceptable.

## The Design Decision: Tests Are Executables

After exploring options, we chose a clean canonical rule:

**Test files are executables with test blocks.**

This means:

1. **Location enforcement**: Test blocks ONLY in `tests/` directories
2. **Purpose enforcement**: Test files MUST have `λmain()→𝕌=()`
3. **Export restriction**: Test files CANNOT have exports

This maintains Sigil's binary file classification:
- **Executable**: Has `main()`, no exports (may have test blocks if in `tests/`)
- **Library**: Has exports, no `main()`

Tests aren't a third category—they're executables that happen to contain test blocks.

## Canonical Test File Structure

```sigil
// tests/list-predicates.sigil
i stdlib⋅list

test "list.in_bounds checks valid indexes" {
  stdlib⋅list.in_bounds(0,[10,20,30])=⊤
}

test "list.in_bounds rejects negative indexes" {
  stdlib⋅list.in_bounds(-1,[10,20,30])=⊥
}

λmain()→𝕌=()  // Required: test files are executables
```

**Why `main()` if tests run independently?**

The `main()→𝕌` function is a **marker for executable status**, not the entry point for test execution. When you run `sigil test tests/`, the compiler:

1. Scans `tests/` for `.sigil` files
2. Compiles each file (building module graph)
3. Generates test runner code from `test` blocks
4. Executes tests via the test framework

The `main()` function is never called—it exists only to declare "this file is an executable, not a library."

## What Gets Rejected

### Test blocks outside `tests/` directory:

```sigil
// examples/fibonacci.sigil
λfibonacci(n:ℤ)→ℤ=...

test "fibonacci works" {  // ❌ ERROR: SIGIL-CANON-TEST-LOCATION
  fibonacci(5)=5
}
```

**Error:**
```
test blocks can only appear in files under tests/ directories.

This file contains test blocks but is not in a tests/ directory.

Move this file to a tests/ directory (e.g., tests/your-test.sigil).

Sigil enforces ONE way: tests live in tests/ directories.
```

### Test file without `main()`:

```sigil
// tests/my-test.sigil
i stdlib⋅list

test "example" {
  stdlib⋅list.length([1,2,3])=3
}

// ❌ ERROR: SIGIL-CANON-FILE-PURPOSE-NONE
// Hint: Test files are executables and must have a main() function.
// Add: λmain()→𝕌=()
```

### Test file with exports:

```sigil
// tests/my-test.sigil
export λhelper()→ℤ=42  // ❌ ERROR: SIGIL-CANON-TEST-NO-EXPORTS

test "example" { ⊤ }

λmain()→𝕌=()
```

**Error:**
```
Test files cannot have export declarations.

Test files are executables, not libraries.
Remove all export keywords from this file.
```

## Implementation in Both Compilers

This enforcement is implemented in **both** the TypeScript and Rust compilers:

### TypeScript Compiler

Added to `language/compiler/src/validator/canonical.ts`:

```typescript
function validateTestLocation(program: AST.Program, filePath: string): void {
  const hasTests = program.declarations.some(d => d.type === 'TestDecl');
  if (!hasTests) return;

  const normalizedPath = filePath.replace(/\\/g, '/');
  if (!normalizedPath.includes('/tests/')) {
    throw new CanonicalError('SIGIL-CANON-TEST-LOCATION', ...);
  }
}
```

### Rust Compiler

Added to `language/compiler-rs/crates/sigil-validator/src/canonical.rs`:

```rust
fn validate_test_location(program: &Program, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let has_tests = program.declarations.iter().any(|d| matches!(d, Declaration::Test(_)));
    if !has_tests { return Ok(()); }

    let normalized_path = file_path.replace('\\', "/");
    if !normalized_path.contains("/tests/") {
        return Err(vec![ValidationError::TestLocationInvalid { ... }]);
    }
    Ok(())
}
```

Both compilers also updated `validateFilePurpose()` to:
- Detect test files (`hasTests`)
- Require `main()` for test files
- Reject exports in test files
- Provide helpful error hints

## Migration: Fixing Existing Files

This change required updating all existing test files in the repository:

**Test files (7 files)** - Added `λmain()→𝕌=()`:
- `language/stdlib-tests/tests/list-predicates.sigil`
- `language/stdlib-tests/tests/numeric-predicates.sigil`
- `projects/algorithms/tests/99-bottles.sigil`
- `projects/algorithms/tests/basic-testing.sigil`
- `projects/algorithms/tests/rot13-encoder.sigil`
- `projects/todo-app/tests/todo-domain.sigil`

**Misplaced test file** - Moved to correct location:
- `language/test-fixtures/test-string-ops.sigil` → `language/tests/string-ops.sigil`

**Invalid files (22 files)** - Added either `main()` or `export`:
- Example files: added `main()` to make them executables
- Stdlib files: added `export` to make them libraries
- Test fixtures: added `main()` or restructured

## Why This Matters for AI Generation

For a language designed to be generated by AI:

1. **No ambiguity**: Tests always live in predictable locations
2. **Clear file purpose**: Every file is unambiguously executable or library
3. **Deterministic structure**: AI agents learn ONE pattern, not many
4. **Better training data**: Repository examples are consistently canonical

When an AI model sees Sigil test code, it sees:
- Tests in `tests/` directories
- Test files with `main()→𝕌=()`
- No exports in test files

This consistency reduces generation errors and improves model performance.

## Practical Outcome

Running tests:

```bash
# Rust compiler
cd language/compiler-rs
cargo build
./target/debug/sigil test ../tests/
./target/debug/sigil test ../stdlib-tests/tests/

# TypeScript compiler (via pnpm)
pnpm sigil:test:stdlib
pnpm sigil:test:all
```

Creating new test files:

```sigil
// tests/my-feature.sigil
i stdlib⋅list

test "my feature works" {
  stdlib⋅list.length([1,2,3])=3
}

λmain()→𝕌=()  // Required marker
```

## Error Message Quality

All validation errors include:
- Clear explanation of what's wrong
- Canonical form guidance
- Specific file path
- Actionable fix instructions

Example:

```
Error: SIGIL-CANON-TEST-LOCATION

test blocks can only appear in files under tests/ directories.

This file contains test blocks but is not in a tests/ directory.
File: examples/fibonacci.sigil

Move this file to a tests/ directory (e.g., tests/fibonacci.sigil).

Sigil enforces ONE way: tests live in tests/ directories.
```

## The Bigger Picture: Canonical Everything

This change completes Sigil's canonical file purpose enforcement:

**File classification (enforced by validator):**
- Executable: `main()`, no exports
- Library: exports, no `main()`
- Test: executable in `tests/` with test blocks

**Surface form (enforced by validator):**
- Final newline required
- No trailing whitespace
- Max one consecutive blank line
- Declaration ordering: `t → e → i → c → λ → test`

**Semantic form (enforced by validator):**
- No tail-call optimization
- No accumulator-passing style
- Primitive recursion only
- File purpose enforcement
- **Test location enforcement** ← NEW

Sigil now enforces ONE canonical way at every level: algorithms, file structure, formatting, and testing.

## Verification

All verification passed:

```bash
# Build both compilers
cd language/compiler-rs && cargo build
cd language/compiler && pnpm build

# All test files compile and run
./target/debug/sigil test ../stdlib-tests/tests/  ✅
./target/debug/sigil test ../../projects/algorithms/tests/  ✅
./target/debug/sigil test ../../projects/todo-app/tests/  ✅

# Location enforcement works
echo 'test "x" { ⊤ }' > /tmp/bad-test.sigil
./target/debug/sigil compile /tmp/bad-test.sigil
# ❌ ERROR: SIGIL-CANON-TEST-LOCATION ✅

# main() requirement works
mkdir -p /tmp/tests
echo 'test "x" { ⊤ }' > /tmp/tests/no-main.sigil
./target/debug/sigil compile /tmp/tests/no-main.sigil
# ❌ ERROR: SIGIL-CANON-FILE-PURPOSE-NONE ✅

# Export restriction works
echo -e 'export λf()→ℤ=1\ntest "x" { ⊤ }\nλmain()→𝕌=()' > /tmp/tests/with-export.sigil
./target/debug/sigil compile /tmp/tests/with-export.sigil
# ❌ ERROR: SIGIL-CANON-TEST-NO-EXPORTS ✅
```

---

**Takeaway:** Canonical language design extends beyond syntax. By enforcing that tests ONLY live in `tests/` directories and MUST be executables, Sigil eliminates organizational ambiguity. This makes the language more deterministic, more learnable for AI models, and more maintainable for humans.

For a machine-first language, there is no room for "tests scattered throughout the codebase." ONE way means ONE location, ONE structure, ONE pattern.
