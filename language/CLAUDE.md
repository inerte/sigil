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
- `tests/` — language-level Sigil tests exercised by `sigil test`
- `docs/` and `spec/` — syntax/specification/reference docs

## Sigil Priorities (for language changes)

1. Canonical syntax over flexibility
2. Deterministic parsing/validation/codegen over convenience
3. Executable examples/tests over prose claims
4. Explicit errors with corrective guidance
5. Minimize syntax ambiguity, especially for AI generation

When in doubt: prefer fewer surface forms and better diagnostics.

For type-system changes, preserve this semantic invariant:
- unconstrained aliases and unconstrained named product types compare by normalized canonical form everywhere equality is checked
- constrained aliases and constrained named product types use refinement checking over their underlying type instead of raw structural equality
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
- runnable examples/tests

Current canonical boolean operators:
- `and`
- `or`
- `¬`

Module scope is declaration-only:
- valid top-level forms: `t`, `e`, `c`, `λ`, `test`
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

Do not land syntax changes that only update the parser.

### 2) Preserve canonicality

Sigil is not “many ways to do it.” If adding a feature:
- define the one canonical surface form
- reject obvious alternatives with helpful errors
- update docs to present only the canonical form

If a parser ambiguity appears, favor the interpretation that preserves globally expected meaning (e.g., arithmetic operators should behave like arithmetic).

Canonical source is now printer-first:
- the compiler owns an internal canonical source printer
- every valid AST has exactly one accepted textual representation
- `compile`, `run`, and `test` reject parseable-but-non-canonical source
- there is no public formatter; the compiler error is the enforcement point
- when updating syntax or source shape, think in terms of AST => one printed form

Current high-signal printer choices:
- delimited aggregate forms stay flat with `0` or `1` item and print multiline with `2+` items
- repeated `++`, `⧺`, `and`, and `or` chains print vertically one continued operand per line
- `requires` / `decreases` / `ensures` (when present) print on following lines in that order before the body
- direct `match` bodies begin on that same line
- direct `match` bodies stay `match ...` with no `=` even after contract lines
- multi-arm `match` is always multiline
- each arm starts as `pattern=>`
- no discretionary alternative layout for the same AST shape

Current constructor and list invariants:
- project-defined sum-type constructors from `src/types.lib.sigil` use `µ...` in both expressions and patterns
- canonical example: `µOrdering([1,2,3])`
- canonical rooted nullary pattern example: `µCycleDetected()`
- list literals preserve nesting exactly as written
- use `⧺` only for explicit concatenation; never rely on list literals to flatten values
- if a canonical helper exists in `stdlib`, prefer it over project-local reimplementation
- in first-party Sigil code outside `language/stdlib/`, do not locally redefine canonical stdlib helpers; use qualified calls like `§list.sum` and `§numeric.max`
- for safe integer list lookup/end access, prefer `§list.nth` and `§list.last`
- Sigil keeps one promise-shaped runtime model, and explicit widening uses named `concurrent` regions
- the canonical concurrent-region surface is `concurrent name@width{...}` or `concurrent name@width:{jitterMs:...,stopOn:...,windowMs:...}{...}`
- only `width` is required; omitted policy defaults to no jitter, no early stop, and no windowing
- ordinary `map` and `filter` are pure list transforms, not concurrency controls
- `map` and `filter` require pure callbacks; `reduce ... from ...` is the ordered reduction form
- `!Async` is not a valid effect annotation
- primitive effects are `Clock`, `Fs`, `Http`, `Log`, `Process`, `Random`, `Tcp`, and `Timer`
- project-defined named effects are allowed only in `src/effects.lib.sigil`
- named effects must expand to at least two primitive effects and should be used consistently instead of rewriting their primitive members across project code
- Sigil supports explicit parametric polymorphism on top-level declarations
- do not describe Sigil as using Hindley-Milner let-polymorphism
- prefer canonical `Option[T]` / `Result[T,E]` over monomorphic wrappers like `IntOption`
- generic lambdas and call-site type arguments like `f[Int](x)` are not part of Sigil's surface
- `ConcurrentOutcome`, `Option`, `Result`, `Aborted`, `Failure`, `Success`, `Some`, `None`, `Ok`, and `Err` are implicit core vocabulary from `¶prelude`
- `Map` is a core collection concept with type syntax `{K↦V}` and literal syntax `{key↦value,...}` / `{↦}`
- helper operations for foundational core types stay namespaced under `¶...`
- operational helpers live in canonical stdlib modules; use `language/docs/STDLIB.md` and `language/spec/stdlib-spec.md` as the source of truth for the current surface
- prefixes are not intrinsically valuable; canonical ownership is
- future changes should decide intentionally whether a concept belongs in:
  - implicit core vocabulary
  - a namespaced module surface
  - backend/runtime only
- prefer putting operational formats/protocols (json, time, url, http, markdown) in `stdlib`
- prefer `§process`, `§file.makeTempDir`, and `§time.sleepMs` for repo-local harness/tooling workflows before reaching for shell-specific orchestration
- promote concepts to core only when they are universal language-shaping vocabulary
- records and maps are distinct:
  - records are fixed-shape structural products using `:`
  - maps are dynamic keyed collections using `↦`
  - never blur them in syntax, docs, examples, or future features
  - records are exact and closed; Sigil does not have open records, row tails, or width subtyping
  - if a field may be absent, keep the record exact and use `Option[T]` for that field
  - project-defined named types in projects live in `src/types.lib.sigil` and are referenced elsewhere as `µTypeName`
  - `src/types.lib.sigil` is types-only and may reference only `§...` and `¶...` inside type definitions and constraints
  - `match` is the branching surface; do not reintroduce a separate public `if` story
  - exhaustiveness and dead-arm checking currently cover `Bool`, `Unit`, tuples, list shapes, exact record patterns, and nominal sum constructors
  - coverage, contracts, and refinement narrowing share the same canonical proof fragment
  - supported proof facts include Bool/Int literals, rooted or pattern-bound values, `value`, `result`, field access, `#` over strings/lists/maps, `+`, `-`, comparisons, `and`, `or`, `not`, direct boolean local aliases of those supported facts, and pattern-shape facts from tuples, lists, exact records, and nominal sum constructors
  - unsupported guards remain valid syntax but stay opaque to coverage and refinement narrowing
  - `where` on a type declaration defines a pure, world-independent refinement over an alias or named product type; compile-time promotion into that type requires proof in Sigil's canonical solver-backed refinement fragment, and `match` / internal branching propagate supported branch facts into that proof context
  - `requires`, `decreases`, and `ensures` are the canonical function-contract surface: `requires` is on parameters, `ensures` is on `result`, and both stay pure and world-independent
  - function declarations are ordinary by default; `mode total` sets a file default, and `total` / `ordinary` may override per declaration
  - `decreases` is reserved for total self-recursive functions; ordinary self-recursive functions may recurse without a termination proof, and total declarations may not call declarations marked `ordinary`
  - effectful total self-recursive functions still use `decreases` only for syntactic recursive-call termination; mutual top-level cycles in a module are rejected; see `language/AGENTS.md` and `language/compiler/ERROR_CODES.md`
  - direct boolean local aliases of supported facts participate in that same flow-sensitive refinement and coverage model
  - `where`, `requires`, and `ensures` do not imply runtime validation
  - prefer early boundary conversion with `§decode` instead of carrying raw `JsonValue` deep into business logic
  - when a validated boundary value should remain distinct from a raw primitive, prefer a named wrapper type like `Email` or `UserId`
  - topology-aware projects must declare external HTTP/TCP dependencies and environment names in `src/topology.lib.sigil`
  - topology-aware projects are validated against the selected `--env`, which must resolve to `config/<env>.lib.sigil`
  - topology-aware application code must use `•topology` dependency handles, not raw URLs, hosts, ports, or env-derived endpoints
  - `process.env` belongs only in `config/*.lib.sigil`, never in ordinary application code
  - tests run in explicit worlds; prefer `config/<env>.lib.sigil` baseline worlds plus test-local `world { ... }` derivation over ad hoc rewiring
  - unused extern declarations are non-canonical in executable `.sigil` files; `.lib.sigil` files may expose extern-based API surface that is unused locally
  - rooted module references are written directly at use sites; there is no separate import declaration surface
  - external packages use the `☴...` root and must be declared as direct exact dependencies in `sigil.json`
  - `☴...` never resolves transitively; if user code names a package, that package must be declared directly
  - publishable packages require both `src/package.lib.sigil` and `publish` in `sigil.json`
  - `sigil.json.name` is lowerCamel and `sigil.json.version` uses canonical UTC timestamp format `YYYY-MM-DDTHH-mm-ssZ`
  - inline single-use pure locals; keep bindings only for reuse, effects, destructuring, or syntax-required staging
  - reject dead named bindings; use `l _=(...)` when sequencing effects without keeping a reusable local
  - `.sigil` files must keep top-level functions, consts, and types reachable from `main` or tests; `.lib.sigil` files may still expose API that is unused locally
  - do not hand-roll recursive list plumbing when Sigil already has a canonical surface
  - use `map` for projection, `filter` for filtering, `reduce ... from ...` for reduction, `§list.reverse` for reversal, `§list.any` / `§list.all` / `§list.find` for existential, universal, and first-match search, `§list.flatMap` for flattening projection, and `§list.countIf` for predicate counting
  - do not build list results by appending to the recursive result (`self(rest)⧺rhs`); use a canonical operator or a wrapper plus accumulator helper with one final reverse

### 3) Keep user-facing errors actionable

Error messages should:
- state what was found
- state the canonical form
- give the required canonical form when possible

Prefer:
- `Use a root or type sigil only where needed (e.g., §list, µTodo, ※check::log, †runtime.World, ☴router)`

Over:
- vague parse failures with no remediation

### 4) Stdlib modules are typed interfaces, not just examples

`stdlib/` modules are consumed through typed rooted references.

When adding or relying on stdlib functions:
- ensure required functions are declared in the correct file kind (`.lib.sigil` for importable modules)
- keep module boundaries intentional (avoid duplicate public APIs across modules unless deliberate)
- update docs/spec references if canonical module names or public functions change

### 5) Comments/docs can be stale; compiler/tests are source of truth

Before assuming syntax is valid, verify with:
- `cargo run -q -p sigil-cli --no-default-features -- compile <file>`
- `cargo run -q -p sigil-cli --no-default-features -- compile <dir> --ignore .git --ignore-from .gitignore`
- parser/validator/typechecker tests

If docs disagree with implementation, either:
- fix docs if implementation is intended
- or fix implementation + tests if docs/spec is intended

## Language Change Protocol (Recommended)

For non-trivial language changes (syntax, semantics, codegen contracts):

1. Confirm current behavior with a minimal failing/working example
2. Implement frontend/compiler changes
3. Update examples/tests that exercise the changed syntax
4. Update docs/specs in the same change
5. Run targeted tests/compiles
6. Summarize unrelated failures explicitly

## Common Commands (from repo root)

Build compiler:

```bash
cargo build -p sigil-cli --no-default-features
```

Compile one Sigil file:

```bash
cargo run -q -p sigil-cli --no-default-features -- compile language/examples/listOperations.sigil
```

Compile a directory recursively:

```bash
cargo run -q -p sigil-cli --no-default-features -- compile language/examples --ignore .git --ignore-from .gitignore
```

Run one Sigil file:

```bash
cargo run -q -p sigil-cli --no-default-features -- run language/examples/listOperations.sigil
```

Run project tests:

```bash
cargo run -q -p sigil-cli --no-default-features -- test projects/algorithms/tests
cargo run -q -p sigil-cli --no-default-features -- test projects/todo-app/tests
```

Run compiler tests:

```bash
cargo test --workspace --no-default-features
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
- Consistent rooted-module path readability
- One canonical way (Sigil philosophy)

#### File Purpose (by extension)

**`.lib.sigil` files** (libraries):
- All functions are automatically visible to other modules (no `export` keyword)
- Cannot have main() function
- Used for reusable code, types, utilities

**`.sigil` files** (executables):
- Must have main() function
- Export nothing directly
- Used for programs, scripts, examples

**`tests/*.sigil` files** (tests):
- Must have main()=>Unit=() function
- Can have test blocks
- Must be in tests/ directory
- May reference `.lib.sigil` APIs directly and exercise executable behavior through `main`

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
cargo build -p sigil-cli --no-default-features
cargo run -q -p sigil-cli --no-default-features -- test projects/algorithms/tests
```

Coverage gate behavior:
- suite-style runs like `sigil test` or `sigil test path/to/tests/` enforce public-contract coverage and variant coverage for project source modules
- focused single-file runs like `sigil test path/to/tests/file.sigil` skip the project-wide coverage gate so iteration stays local

Create new test file:
```sigil program tests/myFeature.sigil
λmain()=>Unit=()

test "my feature works" {
  #[
    1,
    2,
    3
  ]=3
}
```

### Canonical Branching Recursion

Sigil rejects one narrow recursive shape as non-canonical:

- multiple sibling self-calls in the same expression
- each self-call directly reduces the same parameter, such as `n-1` and `n-2`
- the other arguments stay identical across the sibling calls
- error: `SIGIL-CANON-BRANCHING-SELF-RECURSION`

Blocked example:

```sigil invalid-module
λfib(n:Int)=>Int match n{
  0=>0|
  1=>1|
  value=>fib(value-1)+fib(value-2)
}
```

Sigil rejects this shape because it duplicates work instead of following one canonical recursion path.

Use one of these instead:
- wrapper + helper state threading
- accumulator helper recursion
- another canonical linear helper shape when the algorithm permits it

Canonical example:

```sigil module
λfib(n:Int)=>Int
requires n≥0
=fibHelper(
  0,
  1,
  n
)

total λfibHelper(a:Int,b:Int,n:Int)=>Int
requires n≥0
decreases n
match n{
  0=>a|
  count=>fibHelper(
    b,
    a+b,
    count-1
  )
}
```

This rule is intentionally narrow:
- single recursive calls are allowed
- recursion in different control-flow branches is allowed
- recursive calls with different non-reduced arguments are allowed
- Sigil does not attempt general complexity proofs or general exponential-recursion detection

**Termination** (orthogonal to branching): total self-recursive functions (except `Never` returns) need a provable `decreases` measure. Ordinary self-recursive functions may omit `decreases`, and ordinary functions may not declare it; see `language/compiler/ERROR_CODES.md` and `language/AGENTS.md`.

### Testing Invalid Code Patterns

**IMPORTANT**: All `.sigil` files in the repository should compile successfully.

To test that the compiler correctly rejects invalid code patterns (accumulator-passing style, CPS, non-canonical branching recursion, etc.), use Rust crate-level string-input tests instead of creating invalid `.sigil` files:

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
- `language/compiler/crates/sigil-validator/src/branching_recursion.rs` - narrow branching recursion string-input tests
- `language/compiler/crates/sigil-parser/tests/comprehensive.rs` - parser rejection tests

Run compiler tests:
```bash
cargo test --workspace --no-default-features
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
- If syntax/module naming changes affect rooted namespaces or module resolution, update user-facing error text to match canonical Sigil syntax.
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
- All functions in `.lib.sigil` files are automatically visible to other modules.

### `examples/`
- Example Sigil files demonstrating language features
- Run/compile examples to verify compiler behavior
- Keep examples simple and focused on specific features

### `docs/` and `spec/`
- `docs/` = current practical/canonical usage
- `spec/` = formal / broader design contracts
- If implementation intentionally diverges from spec, note it explicitly instead of silently drifting examples
- Markdown Sigil fences and related repo invariants are checked by `projects/repoAudit`
- Use explicit fence kinds only:
  - `sigil program`
  - `sigil module`
  - `sigil expr`
  - `sigil exprs`
  - `sigil type`
  - `sigil decl <context>`
  - `sigil invalid-program`
  - `sigil invalid-module`
  - `sigil invalid-expr`
  - `sigil invalid-type`

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
- cross-module type-resolution bug fixed
- diagnostics improved

Examples of useful verbs:
- `Fix` parser ambiguity for namespace/division parsing
- `Update` rooted-module syntax to use explicit root sigils
- `Expose` stdlib list utilities for cross-module type checking
- `Sync` docs/spec examples with parser behavior
