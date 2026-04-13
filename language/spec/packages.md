# Package System

This document specifies the current Sigil package model.

## Package Identity

- package names are lowerCamel ASCII identifiers
- package versions are exact UTC timestamps in `YYYY-MM-DDTHH-mm-ssZ`
- direct dependencies are declared in `sigil.json.dependencies`
- version ranges are not part of the language surface

## Package Roots

- `☴...` is the rooted source surface for external packages
- `☴name` is valid only when `name` is a direct dependency of the current project
- transitive package imports are invalid

## Publishable Packages

- `src/package.lib.sigil` is the canonical package root module
- `sigil.json.publish` is required if and only if `src/package.lib.sigil` exists
- additional public modules are rooted beneath the package name, e.g. `☴router::matchers.segment`
- `src/flags.lib.sigil` is the canonical project/package home for first-class
  `featureFlag` declarations and may be consumed through nested public paths
  such as `☴featureFlagStorefrontFlags::flags.NewCheckout`

## Commands

The package command family is:

- `sigil package add <name>`
- `sigil package install`
- `sigil package validate`
- `sigil package update [name]`
- `sigil package remove <name>`
- `sigil package list`
- `sigil package why <name>`
- `sigil package publish`

`sigil package update` must:

1. select the newest exact direct dependency version
2. rewrite `sigil.json`
3. rewrite `sigil.lock`
4. install resolved artifacts
5. run project tests
6. roll back on failure unless the user explicitly opts to keep the failing update

`sigil package validate` must:

1. require `publish` in `sigil.json`
2. reject invalid public package imports
3. require passing project tests before packaging
4. produce a local npm transport archive
5. unpack that archive and compile the public package surface, including nested
   public modules under `src/`

## Locking and Transport

- `sigil.lock` records exact resolved package artifacts
- Sigil owns resolution and exactness semantics
- npm is transport only
- the canonical npm transport version is derived as `YYYYMMDD.HHMMSS.0`

## Public API Boundary

In v1, public package modules must not depend on transitive imports being
visible to consumers. Direct-only imports are a hard user-facing rule.
