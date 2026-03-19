# Sigil Language Benchmarks

This directory contains benchmarks comparing Sigil to other programming languages.

## Goal

**Measure Sigil's token efficiency for LLM training.**

Sigil is designed to be machine-first with canonical forms. The primary metric is **LLM token count** (using OpenAI's tiktoken/GPT-4 tokenizer), which directly impacts:

1. **Training dataset size** - Fewer tokens = more code fits in training data
2. **Training cost** - Fewer tokens = cheaper to train
3. **Context efficiency** - Fewer tokens = more code fits in LLM context window
4. **Generation quality** - Canonical forms = more consistent generation

## Methodology

### Token Counting

We use **tiktoken** (OpenAI's tokenizer) with the GPT-4 encoding (`cl100k_base`) as the **official benchmark baseline**.

For Unicode replacement analysis we also run two fully local heuristic proxy tokenizers:
- `llama_sentencepiece_proxy` - a local SentencePiece-style heuristic approximation for non-OpenAI cross-checking
- `anthropic_legacy_proxy` - a local, explicitly approximate Claude-side heuristic proxy

**Policy**
- `cl100k_base` is the canonical reported baseline
- heuristic proxy tokenizers are directional robustness checks, not claims about exact vendor billing
- all tokenizer analysis in this repo is fully offline

**Why tiktoken as the baseline?**
- Industry standard for LLM token counting
- Same tokenizer family used by GPT-3.5/GPT-4-era tooling
- Reflects a real machine-facing optimization target
- Handles Unicode correctly (important for Sigil's symbols)

### Comparison Languages

- **Sigil** - Our canonical form language
- **TypeScript** - Baseline (popular typed language)
- **Python** - Common in ML/AI
- **Rust** - Modern systems language
- **Haskell** - Functional programming baseline

### Metrics Measured

1. **LLM Tokens** (tiktoken) - **PRIMARY METRIC**
2. Characters - Source code length
3. Lines - Line count
4. Tokens/Line - Token density
5. Bytes/Token - UTF-8 efficiency

### Algorithms

Each algorithm is implemented identically across all languages:

```
benchmarks/algorithms/
├── factorial/          # Recursive factorial
├── fibonacci/          # Recursive Fibonacci
├── quicksort/          # Sorting algorithm
├── binarySearch/      # Search algorithm
├── map-filter-reduce/  # Functional operations
└── ...
```

## Usage

### Install Dependencies

```bash
cd benchmarks/tools
npm install tiktoken
```

### Run Comparison

```bash
# Compare all implementations of factorial
node benchmarks/tools/compare.ts benchmarks/algorithms/factorial

# Output:
# | Metric | Sigil | Python | TypeScript |
# |--------|------|--------|------------|
# | LLM Tokens | 45 | 68 | 72 |
# | Characters | 89 | 145 | 178 |
# | ... | ... | ... | ... |
```

### Unicode Replacement Benchmark

This repo also includes a dedicated benchmark for asking:

> Should a given Unicode Sigil syntax element stay, or should it be replaced by a more common programming term?

The benchmark:
- inventories syntax-only Unicode usage in `.sigil` files
- proposes common replacement candidates like `λ -> function`
- rewrites whole source files in memory
- retokenizes the rewritten corpus under all configured tokenizers
- counts separator costs like `λname -> function name`

Commands:

```bash
node benchmarks/tools/unicode-benchmark.js inventory
node benchmarks/tools/unicode-benchmark.js candidates
node benchmarks/tools/unicode-benchmark.js measure
node benchmarks/tools/unicode-benchmark.js explain "λ"
```

The authoritative metric is **whole-file rewrite + retokenize**, not isolated symbol counts.
This matters because replacing a Unicode symbol with a word can introduce separators and change neighboring tokenization.
The default JSON report is written to `language/benchmarks/results/unicode-replacements.json`.

### Primitive Type Switch Benchmark

To measure the specific impact of switching primitive type spellings from legacy
Unicode glyphs to the current capitalized ASCII forms, run:

```bash
node language/benchmarks/tools/primitive-switch-benchmark.js
```

The script:

- rewrites the selected files in memory back to the old Unicode primitive spellings
- retokenizes both versions with the local tokenizer harness
- reports per-file before/after counts and deltas

The published baseline remains `openai_cl100k_base`.

Example:

```sigil module
λendsWith(s:String,suffix:String)=>Bool=false
```

may become:

```text
function ends_with(s:String,suffix:String)=>Bool=false
```

The inserted space is part of the real replacement cost and must be measured.

### Expected Results

**Hypothesis:** Sigil should have **20-40% fewer tokens** than TypeScript/Python due to:

1. **Compact canonical syntax** - `λ`, `=>`, `::`, and `match` compress common structure
2. **Canonical forms** - ONE way to write each construct
3. **No syntactic noise** - Minimal keywords/boilerplate
4. **Type annotations required** - More type info per token

**Example (factorial):**
```
Sigil:       λfactorial(n:Int)=>Int match n{0=>1|1=>1|n=>n*factorial(n-1)}
TypeScript: function factorial(n: number): number {
              if (n === 0 || n === 1) return 1;
              return n * factorial(n - 1);
            }
```

Sigil tokens: ~45
TypeScript tokens: ~72
**Efficiency: 1.6x** (60% more compact)

## Interpreting Results

### Efficiency Ratio

`Efficiency = Baseline Tokens / Sigil Tokens`

- **1.0** - Same as baseline (TypeScript)
- **1.5** - 50% more compact than baseline
- **2.0** - 100% more compact (half the tokens!)

### Training Impact

If Sigil achieves **1.5x efficiency**:
- 50% more Sigil code fits in training data
- 50% lower training costs for equivalent dataset size
- 50% more context fits in LLM windows

### Real-World Example

Training on 1 billion lines of code:
- TypeScript: ~50 billion tokens
- Sigil (1.5x): ~33 billion tokens
- **Savings: 17 billion tokens** = massive cost reduction!

## Contributing

To add a new algorithm:

1. Create directory: `benchmarks/algorithms/<name>/`
2. Implement in all languages: `<name>.sigil`, `<name>.ts`, `<name>.py`, etc.
3. Run comparison: `node benchmarks/tools/compare.ts benchmarks/algorithms/<name>`
4. Document results

## Limitations

- **Semantic equivalence** - We aim for identical algorithms, but language idioms differ
- **Code style** - We use idiomatic style for each language (not artificially verbose/terse)
- **Type annotations** - All languages use maximum type annotations where possible
- **Comments excluded** - Focus on executable code only

## References

- [tiktoken](https://github.com/openai/tiktoken) - OpenAI's tokenizer
- [GPT-4 tokenizer](https://platform.openai.com/tokenizer) - Online token counter
- Sigil documentation: `/docs/philosophy.md`
