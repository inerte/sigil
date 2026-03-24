# Sigil Token Benchmarks

This benchmark family measures Sigil's token efficiency against a small
cross-language corpus and tests tokenizer-sensitive syntax choices.

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

### Active Comparison Corpus

The current published corpus compares:

- **Sigil** - canonical Sigil source
- **TypeScript** - baseline
- **Python** - secondary comparison point

The active corpus currently contains 8 algorithms:

- `factorial`
- `fibonacci`
- `filter-even`
- `gcd`
- `is-palindrome`
- `map-double`
- `power`
- `sum-list`

Future benchmark families can live alongside this one under
`language/benchmarks/`, but today `tokens/` is the only active family.

### Metrics Measured

1. **LLM Tokens** (tiktoken) - **PRIMARY METRIC**
2. Characters - Source code length
3. Lines - Line count
4. Tokens/Line - Token density
5. Bytes/Token - UTF-8 efficiency

## Usage

### Install Dependencies

```bash
cd language/benchmarks/tokens/tools
npm install
```

### Run Comparison

```bash
# Compare one algorithm
node language/benchmarks/tokens/tools/compare.js language/benchmarks/tokens/algorithms/factorial

# Run the full published corpus
bash language/benchmarks/tokens/run-all.sh

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
node language/benchmarks/tokens/tools/unicode-benchmark.js inventory
node language/benchmarks/tokens/tools/unicode-benchmark.js candidates
node language/benchmarks/tokens/tools/unicode-benchmark.js measure
node language/benchmarks/tokens/tools/unicode-benchmark.js explain "λ"
```

The authoritative metric is **whole-file rewrite + retokenize**, not isolated symbol counts.
This matters because replacing a Unicode symbol with a word can introduce separators and change neighboring tokenization.
The default JSON report is written to `language/benchmarks/tokens/results/unicode-replacements.json`.

### Primitive Type Switch Benchmark

To measure the specific impact of switching primitive type spellings from legacy
Unicode glyphs to the current capitalized ASCII forms, run:

```bash
node language/benchmarks/tokens/tools/primitive-switch-benchmark.js
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

### What To Expect

The current corpus shows Sigil as more token-efficient than TypeScript overall,
but the exact gap varies a lot by construct. The strongest gains come from
compact recursive and list-processing examples, while some more imperative or
mixed-style examples are closer.

The underlying hypothesis is still:

1. **Compact canonical syntax** - `λ`, `=>`, `::`, and `match` compress common structure
2. **Canonical forms** - ONE way to write each construct
3. **No syntactic noise** - Minimal keywords/boilerplate
4. **Type annotations required** - More type info per token

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

### Current Published Snapshot

See `RESULTS.md`
for the current corpus totals and per-algorithm table.

## Contributing

To add a new algorithm:

1. Create directory: `language/benchmarks/tokens/algorithms/<name>/`
2. Implement the algorithm in the active comparison languages
3. Run comparison: `node language/benchmarks/tokens/tools/compare.js language/benchmarks/tokens/algorithms/<name>`
4. Refresh `RESULTS.md` if the published corpus changed

## Limitations

- **Semantic equivalence** - We aim for identical algorithms, but language idioms differ
- **Code style** - We use idiomatic style for each language (not artificially verbose/terse)
- **Type annotations** - Sigil and TypeScript are explicitly typed; Python uses type hints
- **Comments excluded** - Focus on executable code only

## References

- [tiktoken](https://github.com/openai/tiktoken) - OpenAI's tokenizer
- [GPT-4 tokenizer](https://platform.openai.com/tokenizer) - Online token counter
- Sigil documentation: `/docs/philosophy.md`
