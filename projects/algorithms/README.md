# Algorithms (Sigil Project)

Canonical pure-Sigil example project.

Layout:
- `sigil.json`
- `src/`
- `tests/`

Commands (from repo root):

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile projects/algorithms/src/collatz-conjecture.sigil
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests
```
