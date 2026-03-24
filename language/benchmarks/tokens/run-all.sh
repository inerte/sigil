#!/bin/bash
# Run all token-efficiency algorithm benchmarks from repo root or any cwd.

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
algorithms_dir="${script_dir}/algorithms"
compare_script="${script_dir}/tools/compare.js"

echo "# Sigil Language - Token Efficiency Benchmarks"
echo ""
echo "Using tiktoken (GPT-4 tokenizer) to count LLM tokens."
echo ""
echo "---"
echo ""

for dir in "${algorithms_dir}"/*/; do
  algorithm=$(basename "$dir")
  echo ""
  node "$compare_script" "$dir"
  echo ""
  echo "---"
  echo ""
done

echo ""
echo "# Summary"
echo ""
echo "All benchmarks use tiktoken (GPT-4's tokenizer) for LLM token counting."
echo "Lower token count = more compact = better for LLM training."
