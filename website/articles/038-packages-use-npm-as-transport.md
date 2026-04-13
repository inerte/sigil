---
title: Packages Use npm as Transport, Not as the Semantic Model
date: 2026-04-05
author: Sigil Language Team
slug: packages-use-npm-as-transport
---

# Packages Use npm as Transport, Not as the Semantic Model

Sigil now has a first-class package system.

That immediately raises an obvious question: if the compiler is written in
Rust, why use npm at all?

The short answer is that npm is the transport layer, not the package model.

## Why Not Cargo

Cargo is a good fit for Rust crates. Sigil packages are not Rust crates.

The current runtime and FFI story for Sigil user programs already lives much
closer to Node than to Rust:

- Sigil emits TypeScript and JavaScript
- Sigil FFI already composes with installed npm packages
- user projects are not built by `cargo`

Choosing cargo just because the compiler happens to be written in Rust would
mix up implementation language and user-package ecosystem.

## Why Not Let npm Own the Model

Because the parts Sigil cares about are exactly the parts npm does *not* make
canonical:

- exact-only dependency versions
- direct-only package imports
- one source-level package root: `☴...`
- one package entrypoint: `src/package.lib.sigil`
- one publishability rule: `publish` if and only if `src/package.lib.sigil`
- one update behavior: install, test, and roll back on failure by default

Those are language decisions. They should not be inherited accidentally from a
general-purpose JavaScript package manager.

So Sigil keeps ownership of:

- `sigil.json`
- `sigil.lock`
- dependency resolution
- no-transitive-import rules
- publish/install/update behavior

npm only stores and ships the archive.

## Exact Timestamp Versions

Sigil package versions remain canonical UTC timestamps:

`2026-04-05T14-58-24Z`

That is the real user-facing version identity.

npm still needs a semver-compatible transport string, so Sigil derives one
canonically:

`20260405.145824.0`

There is no user choice here.

- Sigil manifests use the timestamp
- npm transport uses the derived semver form
- the mapping is mechanical and reversible

That keeps package identity stable without adopting semver ranges or npm's
prerelease conventions as part of the language.

## Direct Imports Only

User code may only import direct dependencies:

```text
λmain()=>Int=☴router.double(21)
```

If `router` depends on `helper`, that does **not** make this valid:

```text
λmain()=>Int=☴helper.double(21)
```

That is a hard compile-time error unless `helper` is declared directly in the
current project's `sigil.json`.

This keeps source dependencies explicit and lets transitive implementation
details change without silently rebinding user code.

## Why the First Example Package Is a Router

The right first package is not a utility bag. It is a design space where
multiple valid answers already exist.

Routers are a good example:

- ordered route tables
- trie-based matchers
- combinator DSLs
- data-driven route specs
- framework-heavy middleware pipelines

Those are real design choices, not just missing stdlib functions.

That is why `projects/router` is the first publishable example package in the
repo. It is intentionally small: ordered exact-path matching with explicit
`matched`, `methodNotAllowed`, and `notFound` outcomes.

The point is not to ship a full framework in the stdlib.

The point is to make plural design spaces live in packages while the language
itself keeps a canonical core.
