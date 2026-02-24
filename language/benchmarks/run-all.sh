#!/bin/bash
# Run all algorithm benchmarks

echo "# Mint Language - Token Efficiency Benchmarks"
echo ""
echo "Using tiktoken (GPT-4 tokenizer) to count LLM tokens."
echo ""
echo "---"
echo ""

for dir in benchmarks/algorithms/*/; do
  algorithm=$(basename "$dir")
  echo ""
  node benchmarks/tools/compare.js "$dir"
  echo ""
  echo "---"
  echo ""
done

echo ""
echo "# Summary"
echo ""
echo "All benchmarks use tiktoken (GPT-4's tokenizer) for LLM token counting."
echo "Lower token count = more compact = better for LLM training."
