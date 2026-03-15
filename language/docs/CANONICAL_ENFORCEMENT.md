# Canonical Enforcement in Sigil

Sigil does not treat canonical form as optional style.
The compiler toolchain rejects non-canonical source.

## Current Enforcement Model

Canonical enforcement happens in multiple phases:

```text
Source
=> Tokenize
=> Parse
=> Canonical validation
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

The validator enforces canonical structural rules such as:

- filename rules
- declaration ordering
- file-purpose rules
- test location rules
- formatting rules
- one-line signatures for functions and lambdas
- canonical `match` layout and arm headers
- no non-canonical operator/delimiter spacing

### Typed Canonical Validation

After type checking, the validator enforces typed canonical rules.

Current important example:

- pure single-use local bindings must be inlined

## Why This Matters

Traditional ecosystems often rely on:

- style guides
- formatter preferences
- lints that can be ignored

Sigil instead makes canonicality part of the accepted language surface.

That gives:

- one accepted spelling for common constructs
- better machine generation loops
- corrective diagnostics instead of review-time style debates

## Practical Rule

If a doc claims “preferred style” but the compiler accepts multiple forms, that
claim is not yet canonical enforcement.

For Sigil, canonicality means the toolchain actually rejects the alternative.

Current high-signal formatting constraints:

- `λfib(n:Int)=>Int match n{...}` is valid; splitting the signature/body introducer across lines is not
- multi-arm `match` must be multiline
- each arm starts as `pattern=>`
- the body must begin on that same line, though it may continue on following indented lines
