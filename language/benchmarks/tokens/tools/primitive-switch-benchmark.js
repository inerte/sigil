#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { measureSourceWithAllTokenizers } from './tokenizers.js';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', '..');

const DEFAULT_FILES = [
  'language/benchmarks/tokens/algorithms/fibonacci/fibonacci.sigil',
  'language/benchmarks/tokens/algorithms/gcd/gcd.sigil',
  'language/benchmarks/tokens/algorithms/is-palindrome/isPalindrome.sigil',
  'projects/todo-app/src/todoDomain.lib.sigil',
  'language/examples/optionResultPractical.sigil'
];

const ASCII_TO_UNICODE = new Map([
  ['Int', 'ℤ'],
  ['Float', 'ℝ'],
  ['Bool', '𝔹'],
  ['String', '𝕊'],
  ['Char', 'ℂ'],
  ['Unit', '𝕌'],
  ['Never', '∅']
]);

function usage() {
  console.error('Usage: node language/benchmarks/tokens/tools/primitive-switch-benchmark.js [file ...]');
}

function toLegacyUnicode(source) {
  let rewritten = source;
  for (const [ascii, unicode] of ASCII_TO_UNICODE.entries()) {
    const pattern = new RegExp(`\\b${ascii}\\b`, 'g');
    rewritten = rewritten.replace(pattern, unicode);
  }
  return rewritten;
}

function selectExcerpt(source) {
  const lines = source.split('\n');
  const hitIndex = lines.findIndex((line) =>
    /\b(Int|Float|Bool|String|Char|Unit|Never)\b|[ℤℝ𝔹𝕊ℂ𝕌∅]/.test(line)
      && /^(t |λ|test )/.test(line.trim())
  );
  const start = Math.max(0, hitIndex === -1 ? 0 : hitIndex - 1);
  const end = Math.min(lines.length, start + 5);
  return lines.slice(start, end).join('\n').trimEnd();
}

function measureFile(relativePath) {
  const absolutePath = path.resolve(repoRoot, relativePath);
  const currentSource = fs.readFileSync(absolutePath, 'utf8');
  const legacySource = toLegacyUnicode(currentSource);
  const before = measureSourceWithAllTokenizers(legacySource);
  const after = measureSourceWithAllTokenizers(currentSource);

  return {
    file: relativePath,
    before,
    after,
    delta: Object.fromEntries(
      Object.keys(before).map((key) => [key, after[key] - before[key]])
    ),
    excerptBefore: selectExcerpt(legacySource),
    excerptAfter: selectExcerpt(currentSource)
  };
}

function formatPercent(before, after) {
  const change = ((after - before) / before) * 100;
  return `${change.toFixed(1)}%`;
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes('--help')) {
    usage();
    process.exit(0);
  }

  const files = args.length > 0 ? args : DEFAULT_FILES;
  const results = files.map(measureFile);

  console.log(JSON.stringify({
    baselineTokenizer: 'openai_cl100k_base',
    files: results,
    summary: results.map((result) => ({
      file: result.file,
      before: result.before.openai_cl100k_base,
      after: result.after.openai_cl100k_base,
      delta: result.delta.openai_cl100k_base,
      percent: formatPercent(result.before.openai_cl100k_base, result.after.openai_cl100k_base)
    }))
  }, null, 2));
}

main();
