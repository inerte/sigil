# Sigil Token Benchmarks - Results

**Date:** 2026-03-24  
**Tokenizer:** `tiktoken` (`cl100k_base`)  
**Baseline:** TypeScript  
**Corpus:** 8 algorithms in `language/benchmarks/tokens/algorithms/`

## Executive Summary

Across the current published corpus, Sigil uses **314 tokens** where the
TypeScript baseline uses **437**. That is:

- **28.1% fewer tokens than TypeScript**
- **1.392x TypeScript efficiency** when measured as `TypeScript / Sigil`
- **40.7% fewer characters** than the same TypeScript corpus

Python is slightly larger than the TypeScript baseline in this corpus:

- **446 tokens total**
- **2.0% more tokens than TypeScript**

## Per-Algorithm Results

| Algorithm | Sigil | TypeScript | Python | TS / Sigil | Sigil Fewer Tokens vs TS |
|-----------|------:|-----------:|-------:|-----------:|-------------------------:|
| factorial | 44 | 52 | 48 | 1.182x | 15.4% |
| fibonacci | 57 | 60 | 60 | 1.053x | 5.0% |
| filter-even | 47 | 67 | 70 | 1.426x | 29.9% |
| gcd | 21 | 48 | 48 | 2.286x | 56.3% |
| is-palindrome | 31 | 49 | 48 | 1.581x | 36.7% |
| map-double | 44 | 59 | 62 | 1.341x | 25.4% |
| power | 44 | 52 | 52 | 1.182x | 15.4% |
| sum-list | 26 | 50 | 58 | 1.923x | 48.0% |
| **Average** | **39.3** | **54.6** | **55.8** | **1.392x** | **28.1%** |
| **Total** | **314** | **437** | **446** | **1.392x** | **28.1%** |

## Current Takeaways

### 1. The current corpus favors Sigil across all 8 examples

There are no TypeScript wins in the active published set. The smallest gap is
`fibonacci`, where Sigil is still 5.0% smaller. The largest is `gcd`, where
Sigil is 56.3% smaller.

### 2. Compact recursive and list-oriented forms still drive the best gains

The strongest gains come from examples where Sigil can stay inside a compact
single-expression shape:

- `gcd`: 21 vs 48 tokens
- `sum-list`: 26 vs 50 tokens
- `is-palindrome`: 31 vs 49 tokens

This aligns with the language goal: one canonical surface with very little
syntactic ceremony around expression-heavy code.

### 3. The previously published `sum-list` outlier is gone

Earlier published results had `sum-list` losing to TypeScript. In the current
corpus it is one of the strongest Sigil wins, at **48.0% fewer tokens**.

That means the old “reduce is less efficient in Sigil” conclusion is no longer
true for the current benchmark sources and should not be reused.

### 4. Character count drops harder than token count

The current corpus averages:

- **105.4 characters** per Sigil implementation
- **177.6 characters** per TypeScript implementation

That is **40.7% fewer characters**. Character density is not the primary metric
for LLM training, but it is a useful secondary check that the token results are
not just a tokenizer quirk.

## Interpretation

These numbers support the same high-level claim as before, but more strongly:

- Sigil's canonical syntax is materially more token-efficient than TypeScript in
  this corpus.
- Python does not beat TypeScript here despite being dynamically typed.
- Compact symbolic forms like `λ`, `=>`, `::`, and dense `match` syntax are not
  getting erased by `cl100k_base`; they still buy real compression.

## Limitations

- The sample is still small: **8 algorithms**
- The corpus is biased toward short expression-heavy examples
- This is a token-efficiency benchmark, not a runtime-performance benchmark
- Results should not be overgeneralized to production code without a larger
  corpus

## Reproduce

```bash
cd language/benchmarks/tokens/tools
npm install

node language/benchmarks/tokens/tools/compare.js language/benchmarks/tokens/algorithms/factorial
bash language/benchmarks/tokens/run-all.sh
node language/benchmarks/tokens/tools/unicode-benchmark.js measure --out language/benchmarks/tokens/results/unicode-replacements.json
```
