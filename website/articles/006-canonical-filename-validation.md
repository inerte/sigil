# Canonical Filename Validation

**Date:** 2026-02-27
**Author:** Sigil Team
**Category:** Language Design, Canonical Forms

## Summary

Sigil now enforces canonical filename format: lowercase letters, numbers, and hyphens only. No more choosing between `UserService.sigil`, `user_service.sigil`, or `user-service.sigil` — there's exactly **one way**.

## The Problem

Before this change, Sigil had inconsistent filename conventions across the codebase:

```
language/stdlib/ffi_node_console.lib.sigil  ← underscores
language/stdlib/http_server.lib.sigil       ← underscores
projects/algorithms/src/binary_search.sigil ← underscores
language/examples/pattern-guards.sigil      ← hyphens
language/examples/sum-types-demo.sigil      ← hyphens
```

This created several issues:

### 1. Case-Insensitive Filesystem Confusion

On macOS and Windows (case-insensitive filesystems), these are **the same file**:
- `User.sigil`
- `user.sigil`
- `USER.sigil`

This causes silent bugs when:
- Cloning repos across different operating systems
- Importing modules with inconsistent casing
- Git treating renames as different files on Linux vs macOS

### 2. Stylistic Ambiguity

Developers had to choose between:
- `snake_case.sigil`
- `kebab-case.sigil`
- `camelCase.sigil`
- `PascalCase.sigil`

This violates Sigil's **ONE WAY** philosophy. Every decision point where humans can choose is a potential source of inconsistency, bikeshedding, and training data pollution.

### 3. Import Path Inconsistency

Without filename validation, import statements could reference modules with arbitrary naming:

```sigil
i stdlib⋅ffi_node_console  ← underscore in module name
i stdlib⋅http_server       ← underscore
i stdlib⋅pattern-guards    ← hyphen
```

Inconsistent naming makes it harder to predict module paths and creates cognitive overhead.

## The Solution: Canonical Filename Format

As of this release, Sigil enforces **canonical filename format**:

### Rules

**Allowed:**
- Lowercase letters: `a-z`
- Numbers: `0-9`
- Hyphens for word separation: `-`
- Required extensions: `.sigil` or `.lib.sigil`

**Basename format:** `^[a-z0-9]+(-[a-z0-9]+)*$`

### Valid Examples

```
✅ hello.sigil
✅ user-service.lib.sigil
✅ 01-introduction.sigil
✅ rot13.lib.sigil
✅ ffi-node-console.lib.sigil
```

### Invalid Examples

```
❌ User.sigil               → SIGIL-CANON-FILENAME-CASE
❌ user_service.lib.sigil   → SIGIL-CANON-FILENAME-INVALID-CHAR
❌ user service.sigil       → SIGIL-CANON-FILENAME-INVALID-CHAR
❌ user@service.sigil       → SIGIL-CANON-FILENAME-INVALID-CHAR
❌ -hello.sigil             → SIGIL-CANON-FILENAME-FORMAT
❌ hello-.sigil             → SIGIL-CANON-FILENAME-FORMAT
❌ hello--world.sigil       → SIGIL-CANON-FILENAME-FORMAT
```

## Error Messages

The compiler provides clear, actionable error messages:

### Uppercase Detection

```
SIGIL-CANON-FILENAME-CASE: Filenames must be lowercase

File: UserService.sigil
Found uppercase in: UserService
Rename to: userservice.{sigil,lib.sigil}

Sigil enforces ONE way: lowercase filenames with hyphens for word separation.
```

### Underscore Detection

```
SIGIL-CANON-FILENAME-INVALID-CHAR: Filenames cannot contain underscores

File: user_service.lib.sigil
Found underscores in: user_service
Rename to: user-service.{sigil,lib.sigil}

Sigil enforces ONE way: use hyphens (-) not underscores (_) for word separation.
```

### Format Violations

```
SIGIL-CANON-FILENAME-FORMAT: Filename cannot start or end with hyphen

File: -hello.sigil
Found: -hello
Hyphens must separate words, not appear at edges.
```

## Migration Guide

### Step 1: Find Files to Rename

```bash
find . -type f \( -name "*.sigil" -o -name "*.lib.sigil" \) | grep '_'
```

### Step 2: Rename with Git

Use `git mv` to preserve history:

```bash
git mv user_service.lib.sigil user-service.lib.sigil
git mv binary_search.sigil binary-search.sigil
git mv ffi_node_console.lib.sigil ffi-node-console.lib.sigil
```

### Step 3: Update Import Statements

Update imports to use the new module names:

**Before:**
```sigil
i stdlib⋅ffi_node_console
i stdlib⋅http_server
```

**After:**
```sigil
i stdlib⋅ffi-node-console
i stdlib⋅http-server
```

### Step 4: Update Documentation

Update any documentation or examples that reference the old filenames.

### Step 5: Compile and Test

Verify everything compiles:

```bash
find language/stdlib -name "*.lib.sigil" -type f | while read file; do
  node language/compiler/dist/cli.js compile "$file"
done
```

Run tests:

```bash
node language/compiler/dist/cli.js test projects/algorithms/tests
```

## Why Kebab-Case?

We chose lowercase-with-hyphens (kebab-case) for several reasons:

### 1. Case-Insensitive Filesystem Safety

Lowercase-only eliminates all case-sensitivity issues across Windows, macOS, and Linux.

### 2. URL and Web Friendliness

Hyphens work naturally in URLs without encoding:
- `example.com/user-service.sigil` ✅
- `example.com/user_service.sigil` (works but inconsistent with web conventions)
- `example.com/UserService.sigil` (case-sensitive, fragile)

### 3. Readability

Hyphens are more readable than underscores in filenames and paths:
- `ffi-node-console.lib.sigil` ← clear word boundaries
- `ffi_node_console.lib.sigil` ← underscores blend with underscores in code

### 4. Consistency with Modern Conventions

Modern tools and frameworks increasingly favor kebab-case for filenames:
- Web components: `user-profile.js`
- CSS modules: `button-primary.css`
- Markdown files: `getting-started.md`

## Impact

This change affects:
- **7 stdlib files** renamed (all `ffi_*` and `http_server`, `markdown_simple`)
- **1 project file** renamed (`binary_search.sigil`)
- **Import statements** updated to use hyphenated module names
- **Compiler validation** added in both TypeScript and Rust implementations

## Technical Details

### Implementation

Filename validation is implemented in two places:

**TypeScript Compiler:** `language/compiler/src/validator/canonical.ts`
```typescript
function validateFilenameFormat(filename?: string): void {
  // Extract basename, check for uppercase, underscores, invalid chars
  // Check format: no hyphens at edges, no consecutive hyphens
}
```

**Rust Compiler:** `language/compiler-rs/crates/sigil-validator/src/canonical.rs`
```rust
fn validate_filename_format(file_path: &str) -> Result<(), Vec<ValidationError>> {
  // Basename extraction and validation
}
```

### Error Codes

Three new canonical error codes:
- `SIGIL-CANON-FILENAME-CASE`
- `SIGIL-CANON-FILENAME-INVALID-CHAR`
- `SIGIL-CANON-FILENAME-FORMAT`

### Tests

**TypeScript:** 10 new test cases in `language/compiler/test/canonical-validation.test.ts`
**Rust:** 11 new test cases in `language/compiler-rs/crates/sigil-validator/tests/comprehensive.rs`

All tests validate rejection of invalid filenames and acceptance of valid kebab-case names.

## Design Philosophy: ONE WAY

This change reinforces Sigil's core principle: **there is exactly one canonical way to write valid Sigil code**.

Filename validation ensures:
- ✅ **No ambiguity** - one filename format, not multiple styles
- ✅ **Deterministic** - tools can predict filenames from module names
- ✅ **Machine-first** - LLMs generate consistent filenames
- ✅ **Future-proof** - no case-sensitivity bugs across platforms

Every convention in Sigil follows this principle:
- **One comment syntax:** `⟦ ... ⟧`
- **One namespace separator:** `⋅`
- **One pattern matching syntax:** `≡scrutinee{pattern→body|...}`
- **One filename format:** lowercase with hyphens

By eliminating choices, we eliminate entire classes of bugs, style debates, and training data inconsistencies.

## Related Documentation

- [Canonical Forms](../docs/CANONICAL_FORMS.md)
- [File Naming Conventions](../language/CLAUDE.md#file-naming-conventions)
- [CLI JSON Spec](../language/spec/cli-json.md)
- [Syntax Reference](../docs/syntax-reference.md)

## Future Work

Potential extensions:
- **Module path validation** - enforce kebab-case in module import paths
- **Directory naming** - extend validation to directory names
- **Auto-fix tooling** - `sigilc fix --filenames` to automatically rename files

---

**Discussion:** Have questions or feedback? [Open an issue](https://github.com/anthropics/sigil/issues)

🔗 Generated with [Claude Code](https://claude.com/claude-code)
