# Mint Language Benchmarks

This directory contains benchmarks comparing Mint to other programming languages.

## Goal

**Measure Mint's token efficiency for LLM training.**

Mint is designed to be machine-first with canonical forms. The primary metric is **LLM token count** (using OpenAI's tiktoken/GPT-4 tokenizer), which directly impacts:

1. **Training dataset size** - Fewer tokens = more code fits in training data
2. **Training cost** - Fewer tokens = cheaper to train
3. **Context efficiency** - Fewer tokens = more code fits in LLM context window
4. **Generation quality** - Canonical forms = more consistent generation

## Methodology

### Token Counting

We use **tiktoken** (OpenAI's tokenizer) with the GPT-4 encoding (`cl100k_base`). This is the **actual tokenizer used by modern LLMs**, not language-specific syntax tokens.

**Why tiktoken?**
- Industry standard for LLM token counting
- Same tokenizer used for GPT-3.5/GPT-4 training
- Reflects real-world LLM training costs
- Handles Unicode correctly (important for Mint's symbols)

### Comparison Languages

- **Mint** - Our canonical form language
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
├── binary-search/      # Search algorithm
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
# | Metric | Mint | Python | TypeScript |
# |--------|------|--------|------------|
# | LLM Tokens | 45 | 68 | 72 |
# | Characters | 89 | 145 | 178 |
# | ... | ... | ... | ... |
```

### Expected Results

**Hypothesis:** Mint should have **20-40% fewer tokens** than TypeScript/Python due to:

1. **Dense Unicode operators** - `→` vs `function`, `≡` vs `switch/match`
2. **Canonical forms** - ONE way to write each construct
3. **No syntactic noise** - Minimal keywords/boilerplate
4. **Type annotations required** - More type info per token

**Example (factorial):**
```
Mint:       λfactorial(n:ℤ)→ℤ≡n{0→1|1→1|n→n*factorial(n-1)}
TypeScript: function factorial(n: number): number {
              if (n === 0 || n === 1) return 1;
              return n * factorial(n - 1);
            }
```

Mint tokens: ~45
TypeScript tokens: ~72
**Efficiency: 1.6x** (60% more compact)

## Interpreting Results

### Efficiency Ratio

`Efficiency = Baseline Tokens / Mint Tokens`

- **1.0** - Same as baseline (TypeScript)
- **1.5** - 50% more compact than baseline
- **2.0** - 100% more compact (half the tokens!)

### Training Impact

If Mint achieves **1.5x efficiency**:
- 50% more Mint code fits in training data
- 50% lower training costs for equivalent dataset size
- 50% more context fits in LLM windows

### Real-World Example

Training on 1 billion lines of code:
- TypeScript: ~50 billion tokens
- Mint (1.5x): ~33 billion tokens
- **Savings: 17 billion tokens** = massive cost reduction!

## Contributing

To add a new algorithm:

1. Create directory: `benchmarks/algorithms/<name>/`
2. Implement in all languages: `<name>.mint`, `<name>.ts`, `<name>.py`, etc.
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
- Mint documentation: `/docs/philosophy.md`
