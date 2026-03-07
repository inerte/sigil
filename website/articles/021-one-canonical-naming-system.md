# One Canonical Naming System

**Date:** 2026-03-07
**Author:** Sigil Team
**Category:** Language Design, Canonical Forms

## Summary

Sigil now uses one naming system across the language:

- `UpperCamelCase` for types, constructors, and type variables
- `lowerCamelCase` for everything else

That includes functions, constants, parameters, locals, record fields, module path segments, and filenames.

## Why

This is not about copying another language's style. It is about reducing ambiguity.

LLMs and tools do better when a name tells you what category it belongs to with almost no context. `UserProfile` is obviously type-level. `userProfile` is obviously value-level. That is cheap lexical information, and Sigil should keep it.

At the same time, Sigil should not make you choose between `snake_case`, `kebab-case`, and `camelCase` depending on where the name appears. That multiplies alternatives for no real gain.

## What Changed

Old forms are gone:

- no `_`
- no `-`
- no leading digits

Examples:

- `fetchUser`
- `createdAt`
- `fileName`
- `userProfile`
- `UserProfile`
- `SomeValue`
- `example01Introduction.sigil`

Invalid:

- `fetch_user`
- `user-profile`
- `01-introduction.sigil`

## External Keys Are Different

String data is not a Sigil identifier.

So these stay exactly as they are when they come from external protocols:

- `"content-type"`
- `"created_at"`
- `"snake_case_from_api"`

Sigil identifiers should be canonical. External string keys should remain faithful to the data they represent.

## Why This Fits Sigil

Sigil is trying to remove needless representational choice.

One naming system everywhere means:

- fewer equivalent spellings
- less search pressure for tools
- less style drift across modules
- clearer local distinction between type-level and value-level names

That is a better default for a machine-first language than a mixed naming story inherited from other ecosystems.
