# Canonical Enforcement in Sigil

Sigil does not treat canonical form as optional style.
The compiler toolchain rejects non-canonical source.

## Current Enforcement Model

Canonicality is now printer-first.
The compiler parses source, builds an AST, prints the canonical source for that
AST internally, and then compares the original file byte-for-byte against that
printed form.

Sigil comments are ignored for this comparison. They are valid syntax, but they
are not part of canonical source form.

If the bytes differ:

- `sigil compile` fails
- `sigil run` fails
- `sigil test` fails

There is no public formatter command. Sigil does not permit “almost canonical”
source to run and then normalize later.

Canonical enforcement now happens like this:

```text
Source
=> Tokenize
=> Parse
=> Canonical source print
=> Source == canonical print ?
=> Type check
=> Typed canonical validation
=> Codegen / Run / Test
```

### Lexer-Level Rejections

The lexer rejects some non-canonical source directly:

- tab characters
- standalone `\r`

### Parse-Time / Surface Constraints

The parser enforces current surface forms such as:

- no `export` token
- typed parameters
- required return types
- required `=` before non-`match` bodies
- forbidden `=` before `match` bodies

### Canonical Validator

The validator still enforces canonical rules that are not reducible to printing
alone, such as:

- filename rules
- declaration ordering
- file-purpose rules
- test location rules
- project-defined type and label declarations only in `src/types.lib.sigil`
- `src/types.lib.sigil` being limited to `t` and `label`
- project boundary rules and transforms only in `src/policies.lib.sigil`
- `src/types.lib.sigil` using only `§...` and `¶...` inside type definitions and constraints
- no dead extern declarations in executable `.sigil` files
- no dead top-level declarations in executable `.sigil` files
- no-shadowing
- record field ordering
- exact top-level wrappers around canonical `§...` helpers and direct `map` / `filter` / `reduce ... from ...` surfaces
- exact recursive list-plumbing bans where Sigil already has a canonical surface
- typed canonical restrictions like dead-binding rejection and single-use pure binding inlining

### Typed Canonical Validation

After type checking, the validator enforces typed canonical rules.

Current important examples:

- named local bindings used zero times are rejected
- pure single-use local bindings must be inlined
- unprovable promotions into constrained types are rejected

Executable note:

- `.sigil` files must keep top-level helper functions, consts, and types reachable from `main` or tests
- `.lib.sigil` files are still allowed to expose public API that is unused locally

Current list-processing examples:

- exact wrappers like `λsum1(xs)=>Int=§list.sum(xs)` are rejected in favor of `§list.sum(xs)` directly
- exact wrappers like `λproject(fn,xs)=>[U]=xs map fn` are rejected in favor of `xs map fn` directly
- recursive `all` clones are rejected in favor of `§list.all`
- recursive `any` clones are rejected in favor of `§list.any`
- `#(xs filter pred)` is rejected in favor of `§list.countIf`
- recursive `map` clones are rejected in favor of `map`
- recursive `filter` clones are rejected in favor of `filter`
- recursive `find` clones are rejected in favor of `§list.find`
- recursive `flatMap` clones are rejected in favor of `§list.flatMap`
- recursive `fold` clones are rejected in favor of `reduce ... from ...` / `§list.fold`
- recursive `reverse` clones are rejected in favor of `§list.reverse`
- recursive result-building of the form `self(rest)⧺rhs` is rejected

## Why This Matters

Traditional ecosystems often rely on:

- style guides
- optional formatter passes
- lints that can be ignored

Sigil instead makes canonicality part of the accepted language surface.

That gives:

- one accepted spelling for common constructs
- better machine generation loops
- one textual representation per valid program

## Practical Rule

If a doc claims “preferred style” but the compiler accepts multiple parseable
forms, that claim is not yet canonical enforcement.

For Sigil, canonicality means the toolchain actually rejects the alternative.

Current high-signal printer choices:

- `λfib(n:Int)=>Int match n{...}` is canonical; splitting the signature/body introducer is not
- delimited aggregate forms stay flat with `0` or `1` item and print multiline with `2+` items
- repeated `++`, `⧺`, `and`, and `or` chains print vertically one continued operand per line
- multi-arm `match` prints multiline
- newline-containing string values print as multiline `"` literals, not `\n`-escaped one-line strings
- spacing is a consequence of the printer, not a second style system
