---
title: "Canonical Test Location: Enforcing ONE Way for Test Files"
date: February 27, 2026
author: Sigil Language Team
slug: 010-canonical-test-location-enforcement
---

# Canonical Test Location: Enforcing ONE Way for Test Files

**TL;DR:** Sigil now enforces that test blocks can ONLY appear in files under `tests/` directories, and test files MUST have a `main()` function. This completes Sigil's canonical file purpose enforcement: every `.sigil` file is either an executable OR a library (and tests are executables with test blocks).

## The Problem: Organizational Ambiguity

Before this change, Sigil allowed `test` blocks anywhere:

```sigil
// examples/fibonacci.sigil
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}

test "fibonacci works" {  // Allowed but not canonical
  fibonacci(5)=5
}
```

**What's wrong with this?**

At first glance, nothing. Tests next to implementation code seems convenient. But this creates three problems:

1. **Scattered tests** - Where are all the tests? `grep -r "^test"` everywhere
2. **Ambiguous file purpose** - Is `fibonacci.sigil` an example, a test, or both?
3. **Non-deterministic organization** - Should tests go in the same file, adjacent files, or test directories?

For a language designed for AI code generation, **"it depends"** is unacceptable. AI models need ONE CLEAR PATTERN.

## The Solution: Tests Are Executables in tests/

We evaluated three options:

### Option 1: Test Annotations on Functions ❌
```sigil
#[test]
λtest_fibonacci()→𝔹=fibonacci(5)=5
```
**Problem:** Introduces decorators, adds syntax complexity, tests still scattered.

### Option 2: Separate Test Files Anywhere ❌
```sigil
// fibonacci.sigil → fibonacci.test.sigil
```
**Problem:** Non-deterministic location (next to source? in subdirs?), file naming conventions vary.

### Option 3: Canonical Test Directory + File Purpose ✅
```sigil
// tests/fibonacci.sigil
λmain()→𝕌=()
test "fibonacci works" { ... }
```
**Winner:** Single location rule, enforces file purpose, zero ambiguity.

## The Canonical Rule

**Test files are executables that live in `tests/` directories.**

This means:

1. **Location enforcement**: Test blocks ONLY in `tests/` directories
2. **Purpose enforcement**: Test files MUST have `λmain()→𝕌=()`
3. **Export restriction**: Test files CANNOT have exports (they're executables, not libraries)

This preserves Sigil's binary file classification:
- **Executable**: Has `main()`, no exports (may have test blocks if in `tests/`)
- **Library**: Has exports, no `main()`

Tests aren't a third category—they're executables that happen to contain test blocks.

## Canonical Test File Structure

Every test file follows the same pattern:

```sigil
// tests/list-predicates.sigil
i stdlib⋅list

λmain()→𝕌=()

test "list.in_bounds checks valid and invalid indexes" {
  stdlib⋅list.in_bounds(0,[10,20,30])=⊤∧stdlib⋅list.in_bounds(3,[10,20,30])=⊥
}

test "list.sorted_asc accepts ascending list" {
  stdlib⋅list.sorted_asc([1,2,3,4])=⊤
}

test "list.sorted_desc accepts descending list" {
  stdlib⋅list.sorted_desc([4,3,2,1])=⊤
}
```

**Structure:**
1. Imports (if needed)
2. `λmain()→𝕌=()` function (always)
3. Test blocks (one or more)

**Why `main()` if tests run independently?**

The `main()→𝕌` function is a **marker for executable status**, not the entry point for test execution. When you run `sigil test tests/`, the compiler:

1. Scans `tests/` for `.sigil` files
2. Compiles each file (building module graph)
3. Extracts `test` blocks and generates test runner code
4. Executes tests via the test framework

The `main()` function **is never called**—it exists only to declare "this file is an executable, not a library." This maintains Sigil's fundamental invariant: **every file has exactly one purpose**.

## What Gets Rejected (With Clear Error Messages)

The validator catches three violations with actionable error messages:

### Violation 1: Test Blocks Outside `tests/` Directory

```sigil
// examples/fibonacci.sigil
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}

test "fibonacci works" {  // ❌ ERROR
  fibonacci(5)=5
}
```

**Compiler error:**
```
Error: SIGIL-CANON-TEST-LOCATION

test blocks can only appear in files under tests/ directories.

This file contains test blocks but is not in a tests/ directory.
File: examples/fibonacci.sigil

Move this file to a tests/ directory (e.g., tests/fibonacci.sigil).

Sigil enforces ONE way: tests live in tests/ directories.
```

**Fix:** Move the entire file to `tests/fibonacci.sigil` and add `λmain()→𝕌=()`.

### Violation 2: Test File Without `main()` Function

```sigil
// tests/my-test.sigil
i stdlib⋅list

test "example" {
  stdlib⋅list.length([1,2,3])=3
}

// ❌ ERROR: File has no purpose (no main, no exports)
```

**Compiler error:**
```
Error: SIGIL-CANON-FILE-PURPOSE-NONE

This file has no purpose declaration.

Files must be EITHER:
  - Executable: λmain()→𝕌=() (no exports)
  - Library: export λ... or export t... (no main)

This file is in tests/ and contains test blocks.
Test files are executables.

Add: λmain()→𝕌=()

Sigil enforces ONE way: every file has exactly one purpose.
```

**Fix:** Add `λmain()→𝕌=()` to the file.

### Violation 3: Test File With Exports

```sigil
// tests/my-test.sigil
export λhelper()→ℤ=42  // ❌ ERROR: exports in test file

test "example" { ⊤ }

λmain()→𝕌=()
```

**Compiler error:**
```
Error: SIGIL-CANON-TEST-NO-EXPORTS

Test files cannot have export declarations.

Found: export λ helper at line 1

Test files are executables, not libraries.
Executables cannot export functions.

Remove all export keywords from this file, or move exported
functions to a separate library file.

Sigil enforces ONE way: tests are executables (not libraries).
```

**Fix:** Remove `export` keyword, or extract the helper to a separate library file if it needs to be shared.

**Notice:** Every error message provides:
1. What's wrong (found)
2. Why it's wrong (rule)
3. How to fix it (action)
4. Where to find more info (file path, line number)

## Implementation: Three-Part Validation

This enforcement is implemented in **both** the TypeScript and Rust compilers with three validation checks:

### Check 1: Test Location Validation

Ensures test blocks only appear in `tests/` directories:

**Rust implementation** (`language/compiler-rs/crates/sigil-validator/src/canonical.rs`):
```rust
fn validate_test_location(program: &Program, file_path: &str) -> Result<(), Vec<ValidationError>> {
    let has_tests = program.declarations.iter()
        .any(|d| matches!(d, Declaration::Test(_)));

    if !has_tests { return Ok(()); }

    let normalized_path = file_path.replace('\\', "/");
    if !normalized_path.contains("/tests/") {
        return Err(vec![ValidationError::TestLocationInvalid {
            file_path: file_path.to_string(),
        }]);
    }
    Ok(())
}
```

**TypeScript implementation** (`language/compiler/src/validator/canonical.ts`):
```typescript
function validateTestLocation(program: AST.Program, filePath: string): void {
  const hasTests = program.declarations.some(d => d.type === 'TestDecl');
  if (!hasTests) return;

  const normalizedPath = filePath.replace(/\\/g, '/');
  if (!normalizedPath.includes('/tests/')) {
    throw new CanonicalError('SIGIL-CANON-TEST-LOCATION',
      'test blocks can only appear in files under tests/ directories',
      filePath);
  }
}
```

### Check 2: File Purpose Validation

Ensures test files have `main()`, no exports:

```rust
fn validate_file_purpose(program: &Program) -> Result<(), ValidationError> {
    let has_main = program.declarations.iter()
        .any(|d| matches!(d, Declaration::Function(f) if f.name == "main"));
    let has_exports = program.declarations.iter()
        .any(|d| d.is_exported());
    let has_tests = program.declarations.iter()
        .any(|d| matches!(d, Declaration::Test(_)));

    match (has_main, has_exports, has_tests) {
        (false, false, true) => Err(ValidationError::FilePurposeNone {
            hint: "Test files must have λmain()→𝕌=()"
        }),
        (true, true, _) => Err(ValidationError::FilePurposeBoth),
        (_, true, true) => Err(ValidationError::TestNoExports),
        _ => Ok(())
    }
}
```

### Check 3: Export Restriction in Test Files

Rejects any `export` declarations in test files:

```rust
if has_tests && has_exports {
    return Err(ValidationError::TestNoExports {
        message: "Test files cannot have export declarations. \
                  Test files are executables, not libraries."
    });
}
```

**Integration point:** All three checks run in `validateCanonicalForm()` after parsing, before typechecking:

```rust
pub fn validate_canonical_form(program: &Program, file_path: &str)
    -> Result<(), Vec<ValidationError>>
{
    validate_recursive_functions(program)?;
    validate_canonical_patterns(program)?;
    validate_declaration_ordering(program)?;
    validate_test_location(program, file_path)?;    // NEW
    validate_file_purpose(program)?;                 // UPDATED
    Ok(())
}
```

**Result:** Compile-time guarantees that test files are well-formed, correctly located executables.

## Migration: Enforcing the Invariant Across the Codebase

This change required updating all existing Sigil files. Here's what we found:

### Test Files Missing `main()` (7 files)

These were already in `tests/` directories but lacked the executable marker:

```diff
  // tests/list-predicates.sigil
  i stdlib⋅list

+ λmain()→𝕌=()

  test "list.in_bounds checks valid indexes" {
    stdlib⋅list.in_bounds(0,[10,20,30])=⊤
  }
```

**Fixed:**
- `language/stdlib-tests/tests/list-predicates.sigil`
- `language/stdlib-tests/tests/numeric-predicates.sigil`
- `projects/algorithms/tests/99-bottles.sigil`
- `projects/algorithms/tests/basic-testing.sigil`
- `projects/algorithms/tests/rot13-encoder.sigil`
- `projects/todo-app/tests/todo-domain.sigil`

### Misplaced Test File (1 file)

Found a test file in the wrong location:

```bash
# Before
language/test-fixtures/test-string-ops.sigil  # ❌ not in tests/

# After
language/tests/string-ops.sigil               # ✅ canonical location
```

### Invalid Files Without Purpose (22 files)

Files that had neither `main()` nor exports:

- **Example files**: Added `λmain()→𝕌=()` to make them executables
- **Stdlib files**: Added `export` to make them libraries
- **Test fixtures**: Added `main()` or restructured

**Result:** Every `.sigil` file now has exactly one purpose. No ambiguity, no exceptions.

## Why This Matters for AI Generation

When you train an LLM on code repositories, every organizational inconsistency becomes noise in the training data.

### Before: Tests Anywhere (Training Data Chaos)

```
Repo A:
  src/fibonacci.sigil          ← implementation
  src/fibonacci.test.sigil     ← tests next to source

Repo B:
  lib/fibonacci.sigil          ← implementation with inline tests

Repo C:
  algorithms/fibonacci.sigil   ← implementation
  spec/fibonacci_spec.sigil    ← tests in separate directory

Repo D:
  fibonacci.sigil              ← everything in one file
```

**What the AI learns:** "Tests can go anywhere, use any naming convention, have any file purpose."

**Result:** Non-deterministic test generation. Same prompt, different locations every time.

### After: Canonical Test Location (Clean Training Data)

```
Every Sigil project:
  src/fibonacci.sigil          ← implementation (library OR executable)
  tests/fibonacci.sigil        ← tests (executable with test blocks)
```

**What the AI learns:** "Tests always go in `tests/`, always have `main()→𝕌=()`, always contain test blocks."

**Result:** Deterministic test generation. Same prompt, same structure, every time.

### The Training Data Impact

For a language designed to be generated by AI:

1. **No decision fatigue** - AI doesn't choose where to put tests, there's ONE location
2. **Clear file purpose** - Every file is unambiguously executable or library
3. **Deterministic output** - Same prompt generates same file structure
4. **Better model performance** - Less training data noise = faster learning

When Claude Code generates a Sigil test file, it knows:
- Location: `tests/` directory
- Structure: `main()` function + test blocks
- Purpose: Executable (no exports)

**Zero ambiguity. Zero variation. Maximum determinism.**

## Practical Workflow

### Running Tests

The validator ensures all test files are well-formed before execution:

```bash
# Rust compiler
cd language/compiler-rs
cargo build
./target/debug/sigil test ../tests/
./target/debug/sigil test ../stdlib-tests/tests/
./target/debug/sigil test ../../projects/algorithms/tests/

# TypeScript compiler (via pnpm)
pnpm sigil:test:stdlib
pnpm sigil:test:all
```

**What happens:**
1. Compiler scans `tests/` for `.sigil` files
2. Validates canonical form (location, purpose, ordering)
3. Compiles each test file with module graph
4. Extracts test blocks, generates runner code
5. Executes tests and reports results

### Creating New Test Files

The canonical pattern is now second nature:

```sigil
// tests/my-feature.sigil
i stdlib⋅list

λmain()→𝕌=()

test "my feature works correctly" {
  stdlib⋅list.length([1,2,3])=3
}

test "handles edge cases" {
  stdlib⋅list.length([])=0
}
```

**Remember:**
1. File location: `tests/` directory
2. Imports first (if needed)
3. `λmain()→𝕌=()` function
4. Test blocks

**That's it.** No other organization is valid. No decisions to make.

## Error Message Philosophy

Sigil error messages are designed for both humans and AI agents. Every error follows the same structure:

**1. What's wrong** - Specific violation found
**2. Where it's wrong** - File path, line number
**3. Why it's wrong** - The canonical rule being enforced
**4. How to fix it** - Concrete action to resolve the error

### Example: Test Location Error

```
Error: SIGIL-CANON-TEST-LOCATION

test blocks can only appear in files under tests/ directories.

This file contains test blocks but is not in a tests/ directory.
File: examples/fibonacci.sigil

Move this file to a tests/ directory (e.g., tests/fibonacci.sigil).

Sigil enforces ONE way: tests live in tests/ directories.
```

**Why this works:**
- **Humans** understand the rule and know how to fix it
- **AI agents** parse the error code (`SIGIL-CANON-TEST-LOCATION`) and apply the fix deterministically
- **Code review** is easier when errors are self-documenting

### Machine-Readable Error Format

Sigil also outputs JSON diagnostics for tooling:

```json
{
  "error": "SIGIL-CANON-TEST-LOCATION",
  "message": "test blocks can only appear in files under tests/ directories",
  "file": "examples/fibonacci.sigil",
  "line": 5,
  "suggestion": "Move this file to a tests/ directory",
  "canonical_form": "tests/fibonacci.sigil"
}
```

This enables IDE integration, CI/CD automation, and AI agent error recovery.

## The Bigger Picture: Canonical Everything

This change completes Sigil's canonical file purpose enforcement. Here's the full picture of what Sigil enforces at compile time:

### File Classification (enforced by validator)
- **Executable**: Has `main()`, no exports (may have test blocks if in `tests/`)
- **Library**: Has exports, no `main()`
- **Test**: Executable in `tests/` with test blocks ← **NEW**

### Surface Form (enforced by validator)
- Final newline required
- No trailing whitespace
- Max one consecutive blank line
- Declaration ordering: `t → e → i → c → λ → test`

### Semantic Form (enforced by validator)
- No tail-call optimization
- No accumulator-passing style
- Primitive recursion only
- **File purpose enforcement** ← Completed with test location rules
- **Test location enforcement** ← **NEW**

### Comparison: Most Languages vs Sigil

**Most languages:**
- Tests: Convention (pytest, jest, go test) - not enforced
- Organization: Linter suggestions - often ignored
- File purpose: Implicit - inferred from usage
- Formatting: Prettier/gofmt - external tool

**Sigil:**
- Tests: Compiler error if outside `tests/`
- Organization: Compile error if declarations out of order
- File purpose: Compile error if ambiguous
- Formatting: Compile error if non-canonical

**Sigil enforces ONE canonical way at every level: algorithms, file structure, formatting, testing, and organization.**

No linters. No formatters. No conventions. Just compiler guarantees.

## Verification: Testing the Tests

We verified all three enforcement rules work correctly:

### ✅ Test Location Enforcement

```bash
# Create test outside tests/ directory
echo 'test "x" { ⊤ }' > /tmp/bad-test.sigil
./target/debug/sigil compile /tmp/bad-test.sigil
```

**Output:**
```
Error: SIGIL-CANON-TEST-LOCATION
test blocks can only appear in files under tests/ directories.
File: /tmp/bad-test.sigil
```

**Verified:** ✅ Tests outside `tests/` are rejected.

### ✅ File Purpose Enforcement

```bash
# Create test file without main()
mkdir -p /tmp/tests
echo 'test "x" { ⊤ }' > /tmp/tests/no-main.sigil
./target/debug/sigil compile /tmp/tests/no-main.sigil
```

**Output:**
```
Error: SIGIL-CANON-FILE-PURPOSE-NONE
This file has no purpose declaration.
Test files must have λmain()→𝕌=()
```

**Verified:** ✅ Test files without `main()` are rejected.

### ✅ Export Restriction Enforcement

```bash
# Create test file with exports
cat > /tmp/tests/with-export.sigil << 'EOF'
export λf()→ℤ=1
test "x" { ⊤ }
λmain()→𝕌=()
EOF
./target/debug/sigil compile /tmp/tests/with-export.sigil
```

**Output:**
```
Error: SIGIL-CANON-TEST-NO-EXPORTS
Test files cannot have export declarations.
Test files are executables, not libraries.
```

**Verified:** ✅ Test files with exports are rejected.

### ✅ All Real Test Files Compile and Run

```bash
# Build both compilers
cd language/compiler-rs && cargo build
cd language/compiler && pnpm build

# Run all test suites
./target/debug/sigil test ../stdlib-tests/tests/           # ✅ 2 files, 5 tests
./target/debug/sigil test ../../projects/algorithms/tests/ # ✅ 3 files, 8 tests
./target/debug/sigil test ../../projects/todo-app/tests/   # ✅ 1 file, 3 tests
./target/debug/sigil test ../tests/                        # ✅ 1 file, 2 tests
```

**Result:** All 7 test files pass compilation and all 18 tests pass execution.

**Migration complete.** Every test file is now canonical.

## Try It Yourself

Want to see canonical test location enforcement in action? Here's a quick demo:

### Step 1: Clone the Sigil Repository

```bash
git clone https://github.com/sigil-lang/sigil.git
cd sigil/language/compiler-rs
cargo build
```

### Step 2: Try a Non-Canonical Test

```bash
# Create test in wrong location
cat > /tmp/wrong-location.sigil << 'EOF'
test "this should fail" {
  1+1=2
}
EOF

# Compile it
./target/debug/sigil compile /tmp/wrong-location.sigil
```

**You'll get:**
```
Error: SIGIL-CANON-TEST-LOCATION
test blocks can only appear in files under tests/ directories.
```

### Step 3: Fix It

```bash
# Create canonical test file
mkdir -p /tmp/tests
cat > /tmp/tests/my-test.sigil << 'EOF'
λmain()→𝕌=()

test "this will work" {
  1+1=2
}
EOF

# Compile it
./target/debug/sigil compile /tmp/tests/my-test.sigil
```

**Success!** Generates clean TypeScript output.

### Step 4: Run It

```bash
./target/debug/sigil test /tmp/tests/
```

**Output:**
```
Running tests in /tmp/tests/
✓ my-test.sigil: this will work
1 test passed
```

**That's the canonical test workflow.** No ambiguity, no decisions, just ONE way.

## Lessons Learned

### 1. File Purpose Enforcement Catches Real Bugs

During migration, we found:
- **7 test files** without `main()` - worked by accident, not by design
- **1 misplaced test file** - in test-fixtures instead of tests/
- **22 files with no purpose** - neither executable nor library

The validator caught all 30 violations. No runtime surprises.

### 2. Canonical Forms Compound

This isn't just about test location. It's about determinism at every level:

- **Syntax**: ONE way to write each construct
- **Declaration ordering**: ONE valid order (`t → e → i → c → λ → test`)
- **File purpose**: ONE purpose per file (executable OR library)
- **Test location**: ONE location for tests (`tests/` directory)

Each canonical form **reinforces the others**. Together, they eliminate organizational bikeshedding entirely.

### 3. AI Benefits Are Multiplicative

One canonical form helps AI 2x. Ten canonical forms help AI 100x (not 20x).

Why? Because each form removes decision points. Fewer decisions = exponentially better:

- **Generation**: No "where should this go?" questions
- **Training**: Clean, consistent corpus
- **Diffs**: Minimal, predictable changes
- **Review**: Focus on logic, not organization

### 4. Humans Benefit Too

This isn't just for AI. Human developers benefit from:

- **No searching** - All tests are in `tests/`, always
- **No bike-shedding** - Compiler decides organization
- **No style debates** - ONE way, enforced at compile time
- **Clearer code review** - Validator guarantees structure

**Determinism helps everyone.**

---

## Conclusion

**Takeaway:** Canonical language design extends beyond syntax. By enforcing that tests ONLY live in `tests/` directories and MUST be executables, Sigil eliminates organizational ambiguity. This makes the language more deterministic, more learnable for AI models, and more maintainable for humans.

For a machine-first language, there is no room for "tests scattered throughout the codebase."

**ONE way means ONE location, ONE structure, ONE pattern.**

When 93% of code is AI-generated (2026 stats), languages should optimize for determinism. Sigil does this by enforcing canonical forms at every level:

- ✅ Canonical syntax
- ✅ Canonical declaration ordering
- ✅ Canonical file purpose
- ✅ Canonical test location

**Zero flexibility. Maximum determinism. Better training data.**
