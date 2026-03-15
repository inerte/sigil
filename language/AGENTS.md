# AGENTS.md (language/)

This guide is for AI coding agents working inside `language/` (the Sigil language implementation).

We are **designing** this new programming language together. Feel free to propose changes that still adhere to the overall goals.

Wear your PhD Computer Scientist and Programming Language Designer Expert hats when working in this repo.

Use the repo root guide for cross-repo coordination:
- `../AGENTS.md`

This file is the local authority for:
- compiler/frontend changes
- canonical syntax changes
- stdlib changes under `language/stdlib/`
- language docs/spec sync inside `language/docs/` and `language/spec/`

## Scope

`language/` contains:
- `compiler/` — lexer, parser, validator, typechecker, codegen, CLI
- `stdlib/` — canonical Sigil modules
- `examples/` — runnable/demo Sigil snippets
- `test-fixtures/` — compile/run regression fixtures
- `docs/` and `spec/` — syntax/specification/reference docs
- `tools/` — LSP / VS Code extension (language tooling)

## Sigil Priorities (for language changes)

1. Canonical syntax over flexibility
2. Deterministic parsing/validation/codegen over convenience
3. Executable examples/tests over prose claims
4. Explicit errors with corrective guidance
5. Minimize syntax ambiguity, especially for AI generation

When in doubt: prefer fewer surface forms and better diagnostics.

For type-system changes, preserve this semantic invariant:
- aliases and named product types compare by normalized canonical form everywhere equality is checked
- do not introduce checker-path-specific structural equality behavior
- sum types remain nominal unless the design explicitly changes

## Working Rules for Language Development

### 1) Change the whole pipeline when syntax changes

If you change syntax, audit all impacted layers:
- lexer tokens/scanning
- parser grammar + AST construction
- canonical validation (if applicable)
- typechecker assumptions/messages
- codegen expectations/comments
- CLI/help/error messages
- docs/spec examples
- runnable examples/tests/fixtures
- editor grammar (`tools/vscode-extension`)

Current canonical boolean operators:
- `and`
- `or`
- `¬`

Module scope is declaration-only:
- valid top-level forms: `t`, `e`, `i`, `c`, `λ`, `mockable λ`, `test`
- never generate top-level `l`
- use `c` for immutable module-level values
- move setup bindings inside `main()` or another function body
- module scope is declaration-only
- local names must not shadow names from the same or any enclosing lexical scope
- prefer fresh descriptive names like `normalized_name`, `next_result`, or `item_count`

Record fields are alphabetical everywhere:
- product type declarations
- record literals
- typed record constructors
- record patterns

Do not land syntax changes that only update the parser.

### 2) Preserve canonicality

Sigil is not “many ways to do it.” If adding a feature:
- define the one canonical surface form
- reject obvious alternatives with helpful errors
- update docs to present only the canonical form

If a parser ambiguity appears, favor the interpretation that preserves globally expected meaning (e.g., arithmetic operators should behave like arithmetic).

Current canonical layout rules:
- function and lambda signatures stay on one line
- direct `match` bodies begin on that same line
- multi-arm `match` is always multiline
- each arm starts as `pattern=>`
- the body must begin on that same line, though it may continue on following indented lines
- no spaces around `:`, `=>`, `=`, `|`, `+`, `-`, `*`, `/`, or `%`
- no spaces just inside delimiters
- no blank lines inside `match`

Current constructor and list invariants:
- imported sum-type constructors use fully qualified module syntax in both expressions and patterns
- canonical example: `src::graphTypes.Ordering([1,2,3])`
- canonical imported nullary pattern example: `src::graphTypes.CycleDetected()`
- list literals preserve nesting exactly as written
- use `⧺` only for explicit concatenation; never rely on list literals to flatten values
- if a canonical helper exists in `stdlib`, prefer it over project-local reimplementation
- for safe integer list lookup/end access, prefer `stdlib::list.nth` and `stdlib::list.last`
- Sigil is concurrent by default; do not describe it as "await every call"
- effectful operations start in source order, even when their resolution overlaps
- `↦` and `⊳` require pure callbacks; `⊕` is the ordered reduction form
- `!Async` is not a valid effect annotation
- Sigil supports explicit parametric polymorphism on top-level declarations
- do not describe Sigil as using Hindley-Milner let-polymorphism
- prefer canonical `Option[T]` / `Result[T,E]` over monomorphic wrappers like `IntOption`
- generic lambdas and call-site type arguments like `f[Int](x)` are not part of Sigil's surface
- `Option`, `Result`, `Some`, `None`, `Ok`, and `Err` are implicit core vocabulary from `core::prelude`
- `Map` is a core collection concept with type syntax `{K↦V}` and literal syntax `{key↦value,...}` / `{↦}`
- helper operations for foundational core types stay namespaced under `core::...`
- operational helpers live in canonical stdlib modules; use `language/docs/STDLIB.md` and `language/spec/stdlib-spec.md` as the source of truth for the current surface
- prefixes are not intrinsically valuable; canonical ownership is
- future changes should decide intentionally whether a concept belongs in:
  - implicit core vocabulary
  - a namespaced module surface
  - backend/runtime only
- prefer putting operational formats/protocols (json, time, url, http, markdown) in `stdlib`
- prefer `stdlib::process`, `stdlib::file.makeTempDir`, and `stdlib::time.sleepMs` for repo-local harness/tooling workflows before reaching for shell-specific orchestration
- promote concepts to core only when they are universal language-shaping vocabulary
- records and maps are distinct:
  - records are fixed-shape structural products using `:`
  - maps are dynamic keyed collections using `↦`
  - never blur them in syntax, docs, examples, or future features
  - records are exact and closed; Sigil does not have open records, row tails, or width subtyping
  - if a field may be absent, keep the record exact and use `Option[T]` for that field
  - prefer early boundary conversion with `stdlib::decode` instead of carrying raw `JsonValue` deep into business logic
  - when a validated boundary value should remain distinct from a raw primitive, prefer a named wrapper type like `Email` or `UserId`
  - topology-aware projects must declare external HTTP/TCP dependencies and environment names in `src/topology.lib.sigil`
  - topology-aware projects are validated against the selected `--env`, which must resolve to `config/<env>.lib.sigil`
  - topology-aware application code must use `src::topology` dependency handles, not raw URLs, hosts, ports, or env-derived endpoints
  - `process.env` belongs only in `config/*.lib.sigil`, never in ordinary application code
  - tests are environments; prefer `config/test.lib.sigil` over ad hoc runtime rewiring
  - inline single-use pure locals; keep bindings only for reuse, effects, destructuring, or syntax-required staging

### 3) Keep user-facing errors actionable

Error messages should:
- state what was found
- state the canonical form
- give a minimal example fix when possible

Prefer:
- `Use "::" (e.g., i stdlib::list)`

Over:
- vague parse failures with no remediation

### 4) Stdlib modules are typed interfaces, not just examples

`stdlib/` modules are consumed through typed imports.

When adding or relying on stdlib functions:
- ensure required functions are declared in the correct file kind (`.lib.sigil` for importable modules)
- keep module boundaries intentional (avoid duplicate public APIs across modules unless deliberate)
- update docs/spec references if canonical module names or public functions change

### 5) Comments/docs can be stale; compiler/tests are source of truth

Before assuming syntax is valid, verify with:
- `cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile <file>`
- parser/validator/typechecker tests

If docs disagree with implementation, either:
- fix docs if implementation is intended
- or fix implementation + tests if docs/spec is intended

## Language Change Protocol (Recommended)

For non-trivial language changes (syntax, semantics, codegen contracts):

1. Confirm current behavior with a minimal failing/working example
2. Implement frontend/compiler changes
3. Update fixtures/examples that exercise the changed syntax
4. Update docs/specs in the same change
5. Run targeted tests/compiles
6. Summarize unrelated failures explicitly

## Common Commands (from repo root)

Build compiler:

```bash
cargo build --manifest-path language/compiler/Cargo.toml -p sigil-cli
```

Compile one Sigil file:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile language/examples/fibonacci.sigil
```

Run one Sigil file:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run language/examples/fibonacci.sigil
```

Run project tests:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/todo-app/tests
```

Run compiler tests:

```bash
cargo test --manifest-path language/compiler/Cargo.toml
```

### File Naming Conventions

Sigil enforces canonical filename format and uses file extensions to distinguish file purpose:

#### Filename Format Rules

Sigil enforces canonical filename format:

**Rules:**
- **lowerCamelCase only** - must start with lowercase, then letters/digits only
- **No underscores or hyphens**
- **Allowed characters**: `a-z`, `A-Z`, `0-9`
- **Must end with** `.sigil` or `.lib.sigil`

**Valid examples:**
- `userService.lib.sigil` ✅
- `example01Introduction.sigil` ✅
- `ffiNodeConsole.lib.sigil` ✅

**Invalid examples:**
- `UserService.lib.sigil` ❌ (uppercase)
- `user_service.lib.sigil` ❌ (underscore)
- `user-service.lib.sigil` ❌ (hyphen)
- `user service.sigil` ❌ (space)

**Error codes:**
- `SIGIL-CANON-FILENAME-CASE` - Does not start with lowercase
- `SIGIL-CANON-FILENAME-INVALID-CHAR` - Contains `_`, `-`, or other invalid characters
- `SIGIL-CANON-FILENAME-FORMAT` - Not lowerCamelCase or starts with a digit

**Why?**
- Case-insensitive filesystem safety (macOS/Windows)
- Consistent import path readability
- One canonical way (Sigil philosophy)

#### File Purpose (by extension)

**`.lib.sigil` files** (libraries):
- All functions are automatically visible to importers (no `export` keyword)
- Cannot have main() function
- Used for reusable code, types, utilities

**`.sigil` files** (executables):
- Must have main() function
- Cannot be imported (except by test files)
- Used for programs, scripts, examples

**`tests/*.sigil` files** (tests):
- Must have main()=>Unit=() function
- Can have test blocks
- Must be in tests/ directory
- Special privilege: can import from ANY file and see ALL functions

When creating new files:
- Library? => Use `.lib.sigil`, all functions auto-visible
- Executable? => Use `.sigil` and add main()
- Test? => Create in tests/ directory with main()

### Working with Tests

Test files must:
1. Live in `tests/` directories
2. Have a `main()=>Unit=()` function (executable marker)
3. Use `.sigil` extension (executables, not libraries)

Run tests:
```bash
cargo build --manifest-path language/compiler/Cargo.toml -p sigil-cli
language/compiler/target/debug/sigil test projects/algorithms/tests
```

Create new test file:
```sigil
// tests/my-feature.sigil
i stdlib::list

λmain()=>Unit=()

test "my feature works" {
  #[1,2,3]=3
}
```

### Testing Invalid Code Patterns

**IMPORTANT**: All `.sigil` files in the repository should compile successfully.

To test that the compiler correctly rejects invalid code patterns (accumulator-passing style, CPS, etc.), use Rust crate-level string-input tests instead of creating invalid `.sigil` files:

```rust
use sigil_lexer::tokenize;
use sigil_parser::parse;
use sigil_validator::{validate_canonical_form, ValidationError};

#[test]
fn test_accumulator_blocked() {
    let source = "λfactorial(n:Int,acc:Int)=>Int match n{0=>acc|n=>factorial(n-1,n*acc)}";
    let tokens = tokenize(source).unwrap();
    let program = parse(tokens, "test.sigil").unwrap();

    let result = validate_canonical_form(&program);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err()[0], ValidationError::AccumulatorParameter { .. }));
}
```

Rust tests go in:
- `language/compiler/crates/sigil-validator/tests/comprehensive.rs` - canonical form validation
- `language/compiler/crates/sigil-parser/tests/comprehensive.rs` - parser rejection tests

Run compiler tests:
```bash
cargo test --manifest-path language/compiler/Cargo.toml
```

## Directory-Specific Notes

### `compiler/crates/sigil-lexer` and `compiler/crates/sigil-parser`
- Syntax changes usually start here.
- Be explicit about token meaning and precedence.
- Avoid introducing context-sensitive parsing when a dedicated token/form can remove ambiguity.

### `compiler/crates/sigil-validator`
- Canonical form rules live here.
- If parser accepts multiple forms but Sigil only allows one, validator must reject non-canonical forms clearly.

### `compiler/crates/sigil-typechecker`
- If syntax/module naming changes affect namespaces/imports, update user-facing error text to match canonical Sigil syntax.
- Keep internal representations stable when possible (e.g., filesystem/module resolution formats).
- The typechecker-to-codegen contract is `TypeCheckResult`, not a raw declaration-type map.
- Build new semantic facts into the typed IR (`typed_program`) instead of teaching codegen to rediscover them from raw AST shape.

### `compiler/crates/sigil-codegen`
- Generated output should remain deterministic.
- Comments/examples in codegen should reflect current Sigil syntax even when emitted JS uses different separators/conventions.
- Codegen consumes typed semantic IR. Prefer lowering `TypedExprKind` directly over adding new AST-shape heuristics.

### `stdlib/`
- Prefer small, canonical modules.
- Avoid duplicate overlapping functions across modules unless there is a clear module-boundary reason.
- All stdlib modules use `.lib.sigil` extension.
- All functions in `.lib.sigil` files are automatically visible to importers.

### `examples/`
- Example Sigil files demonstrating language features
- Run/compile examples to verify compiler behavior
- Keep examples simple and focused on specific features

### `docs/` and `spec/`
- `docs/` = current practical/canonical usage
- `spec/` = formal / broader design contracts
- If implementation intentionally diverges from spec, note it explicitly instead of silently drifting examples

### `tools/vscode-extension`
- Update syntax highlighting patterns when syntax tokens/operators change.
- Highlighting is secondary to compiler correctness, but should ship in the same change for syntax updates.

## What to Include in Change Summaries

For language work, summarize:
- language invariant changed (what is now canonical)
- compiler layers touched
- docs/spec/examples updated
- verification commands run
- known unrelated failures (if any)

## Commit Guidance (language/)

Good commit messages explain why the language/compiler change matters:
- ambiguity removed
- canonical form enforced
- typed import/export bug fixed
- diagnostics improved

Examples of useful verbs:
- `Fix` parser ambiguity for namespace/division parsing
- `Update` canonical import syntax to use :: separators
- `Export` stdlib list utilities for typed imports
- `Sync` docs/spec examples with parser behavior
