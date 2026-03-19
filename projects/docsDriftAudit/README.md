# Docs Drift Audit

`projects/docsDriftAudit` keeps first-party Markdown examples aligned with the
language and the published stdlib surface.

It checks two things:

1. tracked Markdown Sigil fences use an explicit fence kind and validate the
   snippet according to that kind
2. documented stdlib functions have checked example or stdlib-test coverage

## Running it

From `projects/docsDriftAudit/`:

- full repo audit: `../../language/compiler/target/debug/sigil run src/main.sigil`
- targeted files: `../../language/compiler/target/debug/sigil run src/main.sigil -- language/docs/type-system.md`

## Fence kinds

Use one of these exact fence labels:

- `sigil program`
- `sigil module`
- `sigil expr`
- `sigil exprs`
- `sigil type`
- `sigil decl stdlib::module`
- `sigil invalid-expr`
- `sigil invalid-module`
- `sigil invalid-program`
- `sigil invalid-type`

What they mean:

- `program`: compile as a `.sigil` file
- `module`: compile as a `.lib.sigil` file
- `expr`: wrap one expression and parse it
- `exprs`: wrap multiple standalone expressions/examples and parse them
- `type`: wrap one type expression and parse it
- `decl`: declaration-only stdlib docs snippet used for stdlib coverage
- `invalid-*`: snippet is intentionally invalid and must fail

Doc-only annotation lines are ignored inside Sigil fences when they start with:

- `//`
- `⟦`
- `✅`
- `❌`

## Fix policy

When the audit fails, fix the Markdown first unless the checker is plainly
wrong.

Use this order:

1. decide whether the snippet is meant to be valid or invalid
2. choose the correct fence kind
3. update the snippet to current canonical Sigil if it is meant to be valid
4. only change the checker if the failure shows a real checker bug

Do not weaken the checker to accept stale docs.

## Stdlib coverage

The tool derives actual stdlib function exports from `sigil parse` JSON over
`language/stdlib/*.lib.sigil`.

A documented stdlib function is considered covered if it appears in either:

- a checked stdlib test under `language/stdlib-tests/tests/`
- a checked Markdown Sigil snippet

Coverage references use canonical qualified names like
`stdlib::list.reverse`.
