---
title: Simplifying Sigil - Merging SURFACE and CANON Validation
date: 2026-02-27
author: Sigil Language Team
slug: 007-unifying-canonical-validation
---

# Simplifying Sigil: Merging SURFACE and CANON Validation

## The Problem: Two Validators, One Philosophy

Until recently, Sigil had two separate validation phases:

1. **SURFACE validation** - Formatting rules (EOF newline, trailing whitespace, blank lines)
2. **CANON validation** - Semantic rules (recursion patterns, file purpose, ordering, etc.)

Both were mandatory. You couldn't ship code with either type of error. Both enforced the same philosophy: **ONE WAY** to write valid Sigil code.

So why were they separate?

## The Historical Split: A Design Mistake

When we first designed Sigil's validation system, we thought it made sense to separate "formatting concerns" from "semantic concerns":

- **SURFACE** felt like "surface-level formatting" - whitespace, newlines, visual presentation
- **CANON** felt like "deep semantic rules" - recursion patterns, file purpose, declaration ordering

This seemed logical at the time. Many languages separate linting (formatting) from compilation (semantics). We thought users might want to distinguish between "this code has trailing spaces" and "this function uses accumulator-passing style."

But this was wrong for Sigil.

## Why the Split Was Wrong

The boundary between SURFACE and CANON was **arbitrary and artificial**. Here's why:

### 1. Both Were Mandatory

Users couldn't ship code with SURFACE errors any more than they could ship code with CANON errors. Both blocked compilation. Both prevented deployment.

If both validators are mandatory gates to valid Sigil code, why pretend they're different concerns?

### 2. Same Philosophy: ONE WAY

Sigil's core principle is: **there is exactly one canonical way to write valid code**.

This applies to:
- Algorithm structure (no TCO, no accumulators)
- Declaration ordering (alphabetical)
- Whitespace (no trailing spaces)
- File endings (newline required)
- Filenames (lowercase with hyphens)

All of these are canonical forms. All are mandatory. All enforce the same philosophy.

Splitting them into SURFACE vs CANON suggested they were different kinds of rules with different priorities. They weren't.

### 3. The Boundary Was Arbitrary

Consider these rules:

**SURFACE rules (3 total):**
- File must end with newline (`SIGIL-SURFACE-EOF-NEWLINE`)
- No trailing whitespace (`SIGIL-SURFACE-TRAILING-WHITESPACE`)
- Maximum one blank line between declarations (`SIGIL-SURFACE-BLANK-LINES`)

**CANON rules (22+ total):**
- No duplicate declarations (`SIGIL-CANON-DUPLICATE-*`)
- Files must have clear purpose (`SIGIL-CANON-FILE-PURPOSE-*`)
- Declarations must be alphabetically ordered (`SIGIL-CANON-DECLARATION-ORDER`)
- No accumulator-passing style (`SIGIL-CANON-RECURSION-ACCUMULATOR`)
- Filenames must be lowercase kebab-case (`SIGIL-CANON-FILENAME-*`)

Why is "file must end with newline" a different category than "filename must be lowercase"? Both are byte-level formatting rules. Both are mandatory. Both enforce canonicality.

Why is "no trailing whitespace" different from "declarations alphabetically ordered"? Both enforce a specific, mandatory textual representation.

**The boundary made no sense.**

### 4. The Filename Validation Exposed the Problem

When we added filename validation (requiring lowercase with hyphens), the arbitrary boundary became obvious.

Is filename format a SURFACE concern or a CANON concern?

- It's about file representation (suggests SURFACE)
- But it affects import paths and semantics (suggests CANON)
- And it's mandatory like everything else (suggests: does it matter?)

We realized: **the distinction was meaningless**. Everything is canonical. Everything is mandatory. Everything is ONE WAY.

## The Solution: Unified Canonical Validation

We merged SURFACE and CANON into a single unified validator with consistent error codes.

### What Changed

**Before:**
```
SIGIL-SURFACE-EOF-NEWLINE
SIGIL-SURFACE-TRAILING-WHITESPACE
SIGIL-SURFACE-BLANK-LINES
SIGIL-CANON-DUPLICATE-FUNCTION
SIGIL-CANON-RECURSION-ACCUMULATOR
...
```

**After:**
```
SIGIL-CANON-EOF-NEWLINE
SIGIL-CANON-TRAILING-WHITESPACE
SIGIL-CANON-BLANK-LINES
SIGIL-CANON-DUPLICATE-FUNCTION
SIGIL-CANON-RECURSION-ACCUMULATOR
SIGIL-CANON-FILENAME-CASE
...
```

All canonical validation rules now share the `SIGIL-CANON-*` prefix. The validator is simpler, the mental model is clearer, and the error codes honestly reflect reality: **everything is canonical**.

### Implementation

The merge happened in both compiler implementations:

**TypeScript Compiler:**
- Removed `validator/surface.ts`
- Moved EOF/whitespace/blank line checks into `validator/canonical.ts`
- Updated error codes from `SIGIL-SURFACE-*` to `SIGIL-CANON-*`

**Rust Compiler:**
- Removed surface validation module
- Merged all checks into `crates/sigil-validator/src/canonical.rs`
- Updated error types and tests

### All Canonical Rules (Now Unified)

**File-level rules:**
- `SIGIL-CANON-EOF-NEWLINE` - File must end with newline
- `SIGIL-CANON-TRAILING-WHITESPACE` - No trailing whitespace on lines
- `SIGIL-CANON-BLANK-LINES` - Maximum one blank line between declarations
- `SIGIL-CANON-FILE-PURPOSE-NONE` - File must have clear purpose (executable or library)
- `SIGIL-CANON-FILE-PURPOSE-DUAL` - File cannot be both executable and library
- `SIGIL-CANON-LIB-NO-MAIN` - `.lib.sigil` files cannot have main()
- `SIGIL-CANON-EXEC-NEEDS-MAIN` - `.sigil` files must have main()

**Filename rules:**
- `SIGIL-CANON-FILENAME-CASE` - Filenames must be lowercase
- `SIGIL-CANON-FILENAME-INVALID-CHAR` - No underscores, spaces, or special characters
- `SIGIL-CANON-FILENAME-FORMAT` - Hyphens cannot be at edges or consecutive

**Declaration rules:**
- `SIGIL-CANON-DUPLICATE-TYPE` - No duplicate type declarations
- `SIGIL-CANON-DUPLICATE-EXTERN` - No duplicate extern declarations
- `SIGIL-CANON-DUPLICATE-IMPORT` - No duplicate import statements
- `SIGIL-CANON-DUPLICATE-CONST` - No duplicate constant declarations
- `SIGIL-CANON-DUPLICATE-FUNCTION` - No duplicate function declarations
- `SIGIL-CANON-DECLARATION-ORDER` - Declarations must be alphabetically ordered within category

**Algorithm rules:**
- `SIGIL-CANON-RECURSION-ACCUMULATOR` - No accumulator-passing style
- `SIGIL-CANON-RECURSION-TCO` - No tail-call optimization patterns
- `SIGIL-CANON-RECURSION-CPS` - No continuation-passing style
- `SIGIL-CANON-RECURSION-HELPER` - No iterative-style helper functions
- `SIGIL-CANON-RECURSION-CLOSURE` - No closure-based state accumulation
- `SIGIL-CANON-MATCH-BOOLEAN` - Use value matching instead of boolean patterns

**Test rules:**
- `SIGIL-CANON-TEST-LOCATION` - Test blocks only allowed in test files
- `SIGIL-CANON-TEST-PATH` - Test files must be under `tests/` directories

All of these are now part of the unified canonical validator. They're all mandatory. They all enforce ONE WAY.

## Migration Guide

If you have code that checks for validation error codes, update the prefixes:

**Before:**
```typescript
if (error.code === 'SIGIL-SURFACE-EOF-NEWLINE') {
  // handle missing newline
}
if (error.code.startsWith('SIGIL-SURFACE-')) {
  // handle formatting errors
}
```

**After:**
```typescript
if (error.code === 'SIGIL-CANON-EOF-NEWLINE') {
  // handle missing newline
}
if (error.code.startsWith('SIGIL-CANON-')) {
  // handle all canonical violations (formerly SURFACE + CANON)
}
```

The error messages and diagnostic information are unchanged - only the error code prefix changed.

## Benefits

### 1. Simpler Mental Model

**Before:** "Is this a surface rule or a canon rule? What's the difference again?"

**After:** "Everything is canonical. There's ONE WAY to write valid Sigil code."

### 2. Honest Error Codes

Error codes now accurately reflect reality: all these rules enforce canonical forms.

### 3. Less Cognitive Overhead

One validator to understand. One set of error codes. One philosophy consistently applied.

### 4. Better Aligned with Language Philosophy

Sigil's core principle is deterministic, canonical code generation. Having two validators suggested two different priorities or levels of strictness.

Now the implementation matches the philosophy: **everything is canonical, from algorithms down to whitespace**.

### 5. Easier to Extend

When we add new canonical rules (like filename validation), there's no question about which validator should enforce them. Everything goes in the canonical validator.

## The Key Insight: Everything Is Canonical

The fundamental realization was this:

**In Sigil, there is no distinction between "formatting" and "semantics" because both are mandatory parts of the canonical form.**

Other languages separate these concerns because:
- Formatting is optional (use a linter or don't)
- Style is subjective (Prettier vs StandardJS vs ESLint configs)
- Multiple valid forms exist (tabs vs spaces, semicolons vs not)

Sigil is different:
- Formatting is mandatory (no trailing whitespace, period)
- Style is objective (ONE WAY, no configuration)
- Only one valid form exists (canonical or invalid)

When everything is canonical and mandatory, having separate validators is just artificial complexity.

## Looking Forward

This simplification sets the foundation for future enhancements:

**Auto-fix tooling:**
```bash
sigilc fix myfile.sigil
# Fixes all canonical violations automatically
# - Adds missing EOF newline
# - Removes trailing whitespace
# - Reduces multiple blank lines to one
# - Reorders declarations alphabetically
```

**Better error messages:**
```
SIGIL-CANON-TRAILING-WHITESPACE
File: user-service.lib.sigil
Line: 23
Found: trailing spaces after "λadd(x:ℤ,y:ℤ)→ℤ=x+y   "
                                              ^^^

Sigil enforces ONE WAY: no trailing whitespace.
Run: sigilc fix user-service.lib.sigil
```

**Unified validation docs:**
All canonical rules documented in one place: `docs/CANONICAL_FORMS.md`

## Conclusion

Splitting validation into SURFACE and CANON was a design mistake born from conventional thinking about "formatting vs semantics."

Sigil is not conventional. In a language built on the principle of ONE WAY, everything is canonical:
- Whitespace
- Newlines
- Declaration order
- Algorithm structure
- Filenames
- File purpose

Unifying the validators makes the implementation honest, the mental model simpler, and the error codes accurate.

**There is ONE validator because there is ONE WAY.**

---

## Related Documentation

- [Canonical Forms](../language/docs/CANONICAL_FORMS.md) - Complete canonical validation reference
- [Filename Validation](./006-canonical-filename-validation.md) - The change that exposed the arbitrary boundary
- [Declaration Ordering](./005-canonical-declaration-ordering.md) - Another canonical rule
- [CLI JSON Spec](../language/spec/cli-json.md) - Error code format reference

## Discussion

Have questions or feedback about this change? The unification makes Sigil simpler and more honest about what canonical means.

If you disagree and think SURFACE rules should be optional or configurable, we'd love to hear your reasoning. The core question is: should **any** validation rules be optional in a language designed for deterministic, canonical code generation?

---

🔗 Generated with [Claude Code](https://claude.com/claude-code)
