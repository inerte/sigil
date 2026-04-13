# AGENTS.md (projects/)

This guide is for AI coding agents working inside `projects/` (example apps and demo Sigil projects).

Use parent guides for broader context:
- `../AGENTS.md` (repo-wide workflow)
- `../language/AGENTS.md` (language/compiler development)

## Scope

`projects/` contains runnable/demo applications that use Sigil, such as:
- algorithm examples
- app demos (e.g. todo app)
- experiments / prototypes

These are not the language implementation itself.

## Project Priorities

1. Keep demos runnable
2. Prefer canonical Sigil syntax used by the current compiler
3. Keep tests/examples aligned with the current stdlib/export surface
4. Avoid unnecessary changes to generated files unless required

## Typical Sigil Project Layout

Most Sigil projects in this repo use:
- `sigil.json` (project root marker; required lowerCamel `name` and UTC timestamp `version`)
- `src/`
- `tests/`
- `.local/` (generated output)

Publishable packages additionally use:
- `src/package.lib.sigil`
- `publish` in `sigil.json`
- exact-only direct `dependencies` in `sigil.json`

Feature-flag projects and packages additionally use:
- `src/flags.lib.sigil`
- `config/<env>.lib.sigil` declarations such as `flags`, read through `•config.<name>`

When creating new projects, follow that layout unless there is a clear reason not to.

Default runnable entrypoint:
- if a project has any executable `src/*.sigil` files, it must provide `src/main.sigil`
- `src/main.sigil` is the canonical default runnable entrypoint for that project
- projects may still expose other executable `src/*.sigil` files, but `src/main.sigil` is the one LLMs and humans should check first
- library-only projects may omit `src/main.sigil`

## Working Rules for `projects/`

### 1) Treat project code as consumer code

Projects should reflect how users would actually write Sigil today.

If a project breaks due to language changes:
- update project syntax/usages to the new canonical form
- only change the compiler if the project exposes a real regression

### 2) Minimize generated file churn

Prefer editing source files:
- `.sigil`
- project docs/tests
- config files

Regenerate `.local/` outputs only when needed for validation or when tracked artifacts are expected to change.

### 3) Keep examples pedagogically clean

Projects are examples, so prefer:
- direct, readable examples of current canonical syntax
- small focused tests
- minimal FFI surface unless the project specifically demonstrates FFI

### 4) Respect project boundaries

Do not put language/compiler implementation changes under `projects/`.
If a fix requires compiler work, make the compiler change in `language/` and then update the project as a consumer.

## Validation Workflow (from repo root)

Compile or run a single project file:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile projects/<project>/src/<file>.sigil
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run projects/<project>/src/<file>.sigil
```

Run the default project entrypoint when present:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run projects/<project>/src/main.sigil
```

Run project tests:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/todo-app/tests
```

If a project declares direct package dependencies in `sigil.json`, install them
before compile/run/test on a fresh clone:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- package install projects/featureFlagStorefront
```

## Common Pitfalls in `projects/`

- Using syntax shown in older docs/examples instead of current parser behavior
- Relying on stdlib functions that exist but are not exported
- Mixing generated `.local/` artifacts into source edits unintentionally
- Changing project code to “work around” a compiler bug that should be fixed in `language/`

## What to Include in Summaries

When changing `projects/`, summarize:
- which project(s) changed
- whether the change is syntax migration, bugfix, or new demo functionality
- what commands were run (compile/run/test)
- any compiler/stdlib issues discovered while updating the project
