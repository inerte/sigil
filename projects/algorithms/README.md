# Algorithms (Sigil Project)

Canonical pure-Sigil example project.

Layout:
- `sigil.json`
- `src/`
- `tests/`

Implemented algorithms:

- Sorting: `insertionSort.lib.sigil`, `mergeSort.lib.sigil`
- Number theory: `extendedGcd.lib.sigil`, `sieveOfEratosthenes.lib.sigil`, `modularExponentiation.lib.sigil`
- Graphs: `depthFirstSearch.lib.sigil`, `breadthFirstSearch.lib.sigil`, `topologicalSort.lib.sigil`
- Combinatorics: `permutations.lib.sigil`, `nQueens.lib.sigil`
- Search and distance: `linearSearch.lib.sigil`, `levenshteinDistance.lib.sigil`

Supporting modules:

- `graphTypes.lib.sigil`
- `graphHelpers.lib.sigil`
- `int-list-helpers.lib.sigil`

Commands (from repo root):

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile projects/algorithms/src/collatzConjecture.sigil
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/algorithms/tests
```

Phase 1 demo files:

- `src/insertionSortDemo.sigil`
- `src/mergeSortDemo.sigil`
- `src/extendedGcdDemo.sigil`
- `src/sieveOfEratosthenesDemo.sigil`
- `src/modularExponentiationDemo.sigil`
- `src/depthFirstSearchDemo.sigil`
- `src/breadthFirstSearchDemo.sigil`
- `src/topologicalSortDemo.sigil`
- `src/permutationsDemo.sigil`
- `src/nQueensDemo.sigil`
- `src/linearSearchDemo.sigil`
- `src/levenshteinDistanceDemo.sigil`

Planned next:

- `quicksort`
- `quickselect`
- `k-way-merge`
- `prime-factorization`
- `trial-division-primality`
- `tree-traversals`
- `connected-components`
- `combinations`
- `stable-matching`
- `fibonacci-search`
- `jump-search`
