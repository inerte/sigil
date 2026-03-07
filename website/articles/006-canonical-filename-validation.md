# Canonical Filename Validation

**Date:** 2026-02-27
**Author:** Sigil Team
**Category:** Language Design, Canonical Forms

## Summary

Sigil now enforces canonical filename format: `lowerCamelCase` only.

That means:

- no `_`
- no `-`
- no leading digits
- file stems must start with a lowercase letter

Examples:

- `userService.lib.sigil`
- `example01Introduction.sigil`
- `ffiNodeConsole.lib.sigil`

Invalid:

- `UserService.sigil`
- `user_service.sigil`
- `user-service.sigil`
- `01-introduction.sigil`

## Why

This is not about matching ecosystem fashion. It is about removing representational choice.

If Sigil wants one obvious way to write code, filenames cannot have three competing styles. The compiler now enforces the same value-level naming rule it expects elsewhere: `lowerCamelCase`.

That gives us:

- predictable import paths
- case-insensitive filesystem safety
- less style drift across modules
- less ambiguity for tools and LLMs

## Error Codes

- `SIGIL-CANON-FILENAME-CASE`
  filename does not start with a lowercase letter
- `SIGIL-CANON-FILENAME-INVALID-CHAR`
  filename contains `_`, `-`, or another invalid character
- `SIGIL-CANON-FILENAME-FORMAT`
  filename is not `lowerCamelCase` or starts with a digit

## Result

Sigil now has one canonical filename form instead of a mixed `snake_case` / `kebab-case` / `camelCase` story.
