# Canonical Enforcement in Sigil

Sigil does not treat canonical form as optional style.
The compiler toolchain rejects non-canonical source.

## Current Enforcement Model

Canonicality is now printer-first.
The compiler parses source, builds an AST, prints the canonical source for that
AST internally, and then compares the original file byte-for-byte against that
printed form.

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
- no-shadowing
- record field ordering
- typed canonical restrictions like single-use pure binding inlining

### Typed Canonical Validation

After type checking, the validator enforces typed canonical rules.

Current important example:

- pure single-use local bindings must be inlined

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
- multi-arm `match` prints multiline
- branching and non-trivial structure print multiline earlier than dense inline forms
- spacing is a consequence of the printer, not a second style system
