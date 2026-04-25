# Algorithms (Sigil Project)

Canonical practical Sigil corpus for algorithms and data-processing examples.

Layout:
- `sigil.json` (project root marker with required `name` and `version`)
- `src/`
- `tests/`

Implemented algorithms:

- Sorting and selection: `insertionSort.lib.sigil`, `kWayMerge.lib.sigil`, `mergeSort.lib.sigil`, `quickSelect.lib.sigil`, `quickSort.lib.sigil`
- Number theory: `extendedGcd.lib.sigil`, `modularExponentiation.lib.sigil`, `primeFactorization.lib.sigil`, `sieveOfEratosthenes.lib.sigil`, `trialDivisionPrimality.lib.sigil`
- Graphs and trees: `breadthFirstSearch.lib.sigil`, `connectedComponents.lib.sigil`, `depthFirstSearch.lib.sigil`, `topologicalSort.lib.sigil`, `treeTraversals.lib.sigil`
- Combinatorics: `combinations.lib.sigil`, `nQueens.lib.sigil`, `permutations.lib.sigil`
- Search and distance: `fibonacciSearch.lib.sigil`, `jumpSearch.lib.sigil`, `levenshteinDistance.lib.sigil`, `linearSearch.lib.sigil`
- Data processing: `histogram.lib.sigil`, `wordFrequency.lib.sigil`

Supporting modules:

- `graphTypes.lib.sigil`
- `graphHelpers.lib.sigil`

Standalone algorithm/demo entrypoints:

- `factorial.sigil`, `factorialFold.sigil`, `factorialHelper.sigil`, `factorialMutual.sigil`, `factorialValid.sigil`
- `fibonacci.sigil`, `gcd.sigil`, `power.sigil`
- `filterEven.sigil`, `isPalindrome.sigil`, `listLength.sigil`, `listReverse.sigil`, `mapDouble.sigil`, `sumList.sigil`

Commands (from repo root):

```bash
cargo run -q -p sigil-cli --no-default-features -- run projects/algorithms/src/main.sigil
cargo run -q -p sigil-cli --no-default-features -- compile projects/algorithms/src/collatzConjecture.sigil
cargo run -q -p sigil-cli --no-default-features -- test projects/algorithms/tests
```

`src/main.sigil` is the default project entrypoint and prints the available
standalone/demo executables in this project.

These files are the canonical home for the practical algorithm corpus that used to be split across `language/examples/` and `projects/algorithms/`.

The published token benchmark corpus under `language/benchmarks/tokens/`
reuses Sigil source files from this project rather than keeping duplicate
benchmark-only `.sigil` copies.

Demo files:

- `src/combinationsDemo.sigil`
- `src/connectedComponentsDemo.sigil`
- `src/insertionSortDemo.sigil`
- `src/fibonacciSearchDemo.sigil`
- `src/histogramDemo.sigil`
- `src/jumpSearchDemo.sigil`
- `src/kWayMergeDemo.sigil`
- `src/mergeSortDemo.sigil`
- `src/extendedGcdDemo.sigil`
- `src/primeFactorizationDemo.sigil`
- `src/quickSelectDemo.sigil`
- `src/quickSortDemo.sigil`
- `src/sieveOfEratosthenesDemo.sigil`
- `src/modularExponentiationDemo.sigil`
- `src/depthFirstSearchDemo.sigil`
- `src/breadthFirstSearchDemo.sigil`
- `src/topologicalSortDemo.sigil`
- `src/permutationsDemo.sigil`
- `src/nQueensDemo.sigil`
- `src/linearSearchDemo.sigil`
- `src/levenshteinDistanceDemo.sigil`
- `src/treeTraversalsDemo.sigil`
- `src/trialDivisionPrimalityDemo.sigil`
- `src/wordFrequencyDemo.sigil`
