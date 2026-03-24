#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { buildBoundarySnippet, rewriteFileForTokenType } from './sigil-rewrite.js';
import { buildSymbolInventory, getRepoRoot, loadSigilCorpus } from './sigil-inventory.js';
import {
  cleanupTokenizers,
  getTokenizerDefinitions,
  measureSourceWithAllTokenizers,
  measureStringWithAllTokenizers
} from './tokenizers.js';

const toolsDir = path.dirname(fileURLToPath(import.meta.url));
const candidatesConfig = JSON.parse(
  fs.readFileSync(path.join(toolsDir, 'unicode-candidates.json'), 'utf8')
);
const repoRoot = getRepoRoot();
const defaultResultsPath = path.join(repoRoot, 'language/benchmarks/tokens/results/unicode-replacements.json');

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const options = {};
  const positional = [];

  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index];
    if (arg === '--out') {
      options.out = rest[index + 1];
      index += 1;
    } else if (arg === '--symbol') {
      options.symbol = rest[index + 1];
      index += 1;
    } else {
      positional.push(arg);
    }
  }

  return { command, options, positional };
}

function ensureResultsDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function maybeWriteJson(output, outPath) {
  const json = JSON.stringify(output, null, 2);
  if (outPath) {
    const absolute = path.resolve(repoRoot, outPath);
    ensureResultsDir(absolute);
    fs.writeFileSync(absolute, `${json}\n`);
  } else {
    process.stdout.write(`${json}\n`);
  }
}

function buildCandidatesReport(inventory) {
  return inventory.map((entry) => {
    const config = candidatesConfig[entry.tokenType];
    return {
      symbol: entry.symbol,
      tokenType: entry.tokenType,
      category: entry.category,
      occurrences: entry.occurrences,
      filesAffected: entry.filesAffected,
      candidates: config.candidates
    };
  });
}

function pickRecommendedCandidate(candidateResults) {
  const sorted = [...candidateResults].sort((left, right) => {
    const leftScore = left.aggregateScore;
    const rightScore = right.aggregateScore;
    if (rightScore !== leftScore) {
      return rightScore - leftScore;
    }
    if (right.commonalityScore !== left.commonalityScore) {
      return right.commonalityScore - left.commonalityScore;
    }
    return left.replacement.localeCompare(right.replacement);
  });

  return sorted[0] || null;
}

function computeAgreement(candidate) {
  const deltas = Object.values(candidate.corpusDelta);
  const negative = deltas.filter((delta) => delta < 0).length;
  const positive = deltas.filter((delta) => delta > 0).length;

  if (negative === deltas.length) {
    return 'high';
  }
  if (negative >= 2 && positive === 0) {
    return 'medium';
  }
  if (negative >= 1 && positive === 0) {
    return 'low';
  }
  return 'mixed';
}

function computeAggregateScore(candidate) {
  const baseline = -candidate.corpusDelta.openai_cl100k_base;
  const llama = -candidate.corpusDelta.llama_sentencepiece_proxy;
  const anthropic = -candidate.corpusDelta.anthropic_legacy_proxy;
  return baseline + (llama * 0.5) + (anthropic * 0.25) + candidate.commonalityScore;
}

function measureCandidate(corpus, inventoryEntry, candidate) {
  const affectedFiles = corpus.filter((file) => file.symbols.some((token) => token.type === inventoryEntry.tokenType));
  const corpusDelta = {
    openai_cl100k_base: 0,
    llama_sentencepiece_proxy: 0,
    anthropic_legacy_proxy: 0
  };
  const fileImpacts = [];

  for (const file of affectedFiles) {
    const rewritten = rewriteFileForTokenType(file, inventoryEntry.tokenType, candidate.replacement);
    const beforeCounts = measureSourceWithAllTokenizers(file.source);
    const afterCounts = measureSourceWithAllTokenizers(rewritten);
    const perFileDelta = {};

    for (const tokenizerId of Object.keys(corpusDelta)) {
      const delta = afterCounts[tokenizerId] - beforeCounts[tokenizerId];
      corpusDelta[tokenizerId] += delta;
      perFileDelta[tokenizerId] = delta;
    }

    fileImpacts.push({
      file: file.relativePath,
      delta: perFileDelta
    });
  }

  fileImpacts.sort((left, right) => left.delta.openai_cl100k_base - right.delta.openai_cl100k_base);

  const sampleFile = affectedFiles[0];
  const sampleToken = sampleFile?.symbols.find((token) => token.type === inventoryEntry.tokenType);
  const boundarySnippet = sampleFile && sampleToken
    ? buildBoundarySnippet(sampleFile, sampleToken, candidate.replacement)
    : null;

  const snippetTokenCounts = boundarySnippet
    ? {
        before: measureStringWithAllTokenizers(boundarySnippet.before),
        after: measureStringWithAllTokenizers(boundarySnippet.after)
      }
    : null;

  const standaloneBefore = measureStringWithAllTokenizers(inventoryEntry.symbol);
  const standaloneAfter = measureStringWithAllTokenizers(candidate.replacement);

  const measured = {
    replacement: candidate.replacement,
    commonalityScore: candidate.commonalityScore,
    semanticFit: candidate.semanticFit,
    reason: candidate.reason,
    standaloneTokenCounts: Object.fromEntries(
      Object.keys(standaloneBefore).map((tokenizerId) => [
        tokenizerId,
        {
          before: standaloneBefore[tokenizerId],
          after: standaloneAfter[tokenizerId]
        }
      ])
    ),
    boundarySnippet,
    boundarySnippetTokenCounts: snippetTokenCounts,
    corpusDelta,
    topFileImpacts: fileImpacts.slice(0, 10)
  };

  measured.agreement = computeAgreement(measured);
  measured.aggregateScore = computeAggregateScore(measured);
  measured.decision = measured.corpusDelta.openai_cl100k_base < 0 ? 'candidate' : 'keep_current';
  return measured;
}

function measureReport(corpus, inventory, selectedSymbol = null) {
  const filteredInventory = selectedSymbol
    ? inventory.filter((entry) => entry.symbol === selectedSymbol || entry.tokenType === selectedSymbol)
    : inventory;

  const symbols = filteredInventory.map((entry) => {
    const config = candidatesConfig[entry.tokenType];
    const candidateResults = config.candidates.map((candidate) => measureCandidate(corpus, entry, candidate));
    const recommended = pickRecommendedCandidate(candidateResults);

    let confidence = 'low';
    if (recommended) {
      if (recommended.agreement === 'high' && recommended.corpusDelta.openai_cl100k_base < 0) {
        confidence = 'high';
      } else if (recommended.agreement !== 'mixed' && recommended.corpusDelta.openai_cl100k_base < 0) {
        confidence = 'medium';
      }
    }

    return {
      symbol: entry.symbol,
      tokenType: entry.tokenType,
      category: entry.category,
      occurrences: entry.occurrences,
      filesAffected: entry.filesAffected,
      sampleFiles: entry.sampleFiles,
      candidates: candidateResults,
      recommendation: recommended
        ? {
            replacement: recommended.replacement,
            confidence,
            reason: `${recommended.reason} Baseline delta: ${recommended.corpusDelta.openai_cl100k_base}. Agreement: ${recommended.agreement}.`
          }
        : null
    };
  });

  symbols.sort((left, right) => {
    const leftDelta = left.recommendation
      ? left.candidates.find((candidate) => candidate.replacement === left.recommendation.replacement)?.corpusDelta.openai_cl100k_base ?? 0
      : 0;
    const rightDelta = right.recommendation
      ? right.candidates.find((candidate) => candidate.replacement === right.recommendation.replacement)?.corpusDelta.openai_cl100k_base ?? 0
      : 0;
    return leftDelta - rightDelta;
  });

  return {
    version: 1,
    generatedAt: new Date().toISOString(),
    corpus: {
      files: corpus.length,
      paths: corpus.map((file) => file.relativePath)
    },
    tokenizers: getTokenizerDefinitions(),
    symbols
  };
}

function explainSymbol(report, symbol) {
  const match = report.symbols.find((entry) => entry.symbol === symbol || entry.tokenType === symbol);
  if (!match) {
    throw new Error(`No measured symbol found for '${symbol}'`);
  }
  return match;
}

function main() {
  const { command, options, positional } = parseArgs(process.argv.slice(2));

  if (!command || ['inventory', 'candidates', 'measure', 'explain'].includes(command) === false) {
    console.error('Usage: node language/benchmarks/tokens/tools/unicode-benchmark.js <inventory|candidates|measure|explain> [symbol] [--out file] [--symbol symbol]');
    process.exit(1);
  }

  const corpus = loadSigilCorpus(candidatesConfig);
  const inventory = buildSymbolInventory(corpus, candidatesConfig);

  try {
    if (command === 'inventory') {
      maybeWriteJson(
        {
          version: 1,
          generatedAt: new Date().toISOString(),
          tokenizers: getTokenizerDefinitions(),
          corpus: {
            files: corpus.length
          },
          symbols: inventory
        },
        options.out
      );
      return;
    }

    if (command === 'candidates') {
      maybeWriteJson(
        {
          version: 1,
          generatedAt: new Date().toISOString(),
          symbols: buildCandidatesReport(inventory)
        },
        options.out
      );
      return;
    }

    if (command === 'measure') {
      const report = measureReport(corpus, inventory, options.symbol || null);
      maybeWriteJson(report, options.out || defaultResultsPath);
      return;
    }

    if (command === 'explain') {
      const symbol = positional[0] || options.symbol;
      if (!symbol) {
        throw new Error('Usage: explain <symbol>');
      }
      const report = measureReport(corpus, inventory, symbol);
      maybeWriteJson(explainSymbol(report, symbol), options.out);
    }
  } finally {
    cleanupTokenizers();
  }
}

main();
