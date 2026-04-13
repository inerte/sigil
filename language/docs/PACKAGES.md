# Packages

Sigil packages are canonical projects with one publishable entrypoint and one
package-management command family.

## Source Surface

External packages use the `☴...` root:

```text
λmain()=>Int=☴router.double(21)
```

Rules:

- `☴name` resolves only against direct dependencies declared in `sigil.json`
- transitive package imports are rejected
- package references stay rooted at use sites; there is no import declaration surface

## Project Layout

Publishable packages require:

- `sigil.json`
- `src/package.lib.sigil`
- `publish` in `sigil.json`

Optional package files keep the existing canonical meanings:

- `src/types.lib.sigil`
- `src/flags.lib.sigil`
- `src/effects.lib.sigil`
- `src/topology.lib.sigil`
- `tests/`
- `config/test.lib.sigil`

`src/package.lib.sigil` is the package root API. Additional public modules are
reached through nested package paths such as `☴router::matchers.segment`.
That same nested public surface is how packages expose shared feature flags:

```sigil expr
§featureFlags.get(
  context,
  ☴featureFlagStorefrontFlags::flags.NewCheckout,
  •config.flags
)
```

`src/flags.lib.sigil` is therefore a natural place for internal shared flag
contracts, while `src/package.lib.sigil` can still export helper constructors
or context builders.

## Manifest Rules

`sigil.json` is canonical:

- `name` is lowerCamel
- `version` is an exact UTC timestamp in `YYYY-MM-DDTHH-mm-ssZ`
- `dependencies` is a map from direct dependency name to exact UTC timestamp
- `publish` is required if and only if `src/package.lib.sigil` exists

Example:

```json
{
  "name": "router",
  "version": "2026-04-05T14-58-24Z",
  "dependencies": {
    "pathMatch": "2026-04-05T14-12-00Z"
  },
  "publish": {}
}
```

## Commands

Package management lives under `sigil package ...`:

```bash
sigil package add router
sigil package install
sigil package validate
sigil package update router
sigil package update
sigil package remove router
sigil package list
sigil package why router
sigil package publish
```

`sigil package update` rewrites `sigil.json`, rewrites `sigil.lock`, installs
the resolved dependency tree, runs project tests, and rolls back on failure by
default.

`sigil package validate` is the local publishability check. It requires:

- passing project tests before packaging
- a valid public package surface
- a successful local `npm pack`
- a successful compile of the unpacked transport artifact, including nested
  public modules such as `flags`

## npm Transport

Sigil uses npm only as transport and publishing infrastructure.

- Sigil-visible versions stay in `YYYY-MM-DDTHH-mm-ssZ`
- npm transport versions are derived canonically as `YYYYMMDD.HHMMSS.0`
- Sigil source never spells npm package versions
- resolution, exactness, locking, and no-transitive-import rules are owned by Sigil

## Example Package

`projects/router/` is the first publishable example package in this repo. It is
an ordered exact-path router with explicit `matched`, `methodNotAllowed`, and
`notFound` outcomes. It exists to demonstrate package-tier design space, not to
replace stdlib HTTP primitives.

`projects/featureFlagStorefrontFlags/` is the first publishable example package whose main
public surface is `src/flags.lib.sigil`. It demonstrates sharing typed
`featureFlag` declarations across projects while letting each consuming app keep
its own `config/<env>.lib.sigil` values.
