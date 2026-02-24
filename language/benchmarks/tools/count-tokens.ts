/**
 * Multi-Language Token Counter
 *
 * Counts tokens for comparing code density across languages.
 * Uses tiktoken (OpenAI's tokenizer) for LLM-relevant token counting.
 */

import * as fs from 'fs';
import * as path from 'path';
import { encoding_for_model } from 'tiktoken';

// Use GPT-4 tokenizer (cl100k_base encoding)
const encoder = encoding_for_model('gpt-4');

export interface TokenMetrics {
  language: string;
  file: string;
  llmTokens: number;        // tiktoken (GPT-4) tokens - MOST IMPORTANT for LLM training
  totalChars: number;
  totalLines: number;
  tokensPerLine: number;
  charsPerToken: number;
  bytesPerToken: number;    // UTF-8 bytes per LLM token
}

/**
 * Count LLM tokens using tiktoken (OpenAI's tokenizer)
 * This is the REAL metric for LLM training efficiency!
 */
export function countLLMTokens(source: string, language: string, filename: string): TokenMetrics {
  const tokens = encoder.encode(source);
  const lines = source.split('\n').length;
  const bytes = Buffer.byteLength(source, 'utf-8');

  return {
    language,
    file: filename,
    llmTokens: tokens.length,
    totalChars: source.length,
    totalLines: lines,
    tokensPerLine: tokens.length / lines,
    charsPerToken: source.length / tokens.length,
    bytesPerToken: bytes / tokens.length
  };
}

/**
 * Auto-detect language and count tokens
 */
export function countTokens(filepath: string): TokenMetrics {
  const source = fs.readFileSync(filepath, 'utf-8');
  const ext = path.extname(filepath);
  const filename = path.basename(filepath);

  const languageMap: Record<string, string> = {
    '.mint': 'Mint',
    '.ts': 'TypeScript',
    '.js': 'JavaScript',
    '.py': 'Python',
    '.rs': 'Rust',
    '.hs': 'Haskell',
    '.go': 'Go',
    '.java': 'Java'
  };

  const language = languageMap[ext];
  if (!language) {
    throw new Error(`Unsupported file extension: ${ext}`);
  }

  return countLLMTokens(source, language, filename);
}

/**
 * Compare multiple implementations
 */
export function compareImplementations(files: string[]): Map<string, TokenMetrics> {
  const results = new Map<string, TokenMetrics>();

  for (const file of files) {
    try {
      const metrics = countTokens(file);
      results.set(metrics.language, metrics);
    } catch (error) {
      console.error(`Error processing ${file}:`, error);
    }
  }

  return results;
}

/**
 * Generate comparison table
 */
export function generateComparisonTable(results: Map<string, TokenMetrics>): string {
  const languages = Array.from(results.keys()).sort();

  let table = '| Metric | ' + languages.join(' | ') + ' |\n';
  table += '|--------|' + languages.map(() => '------').join('|') + '|\n';

  // LLM Tokens (GPT-4 tiktoken) - MOST IMPORTANT
  table += '| **LLM Tokens** (tiktoken) | ';
  table += languages.map(lang => results.get(lang)!.llmTokens).join(' | ');
  table += ' |\n';

  // Total characters
  table += '| **Characters** | ';
  table += languages.map(lang => results.get(lang)!.totalChars).join(' | ');
  table += ' |\n';

  // Total lines
  table += '| **Lines** | ';
  table += languages.map(lang => results.get(lang)!.totalLines).join(' | ');
  table += ' |\n';

  // Tokens per line
  table += '| **Tokens/Line** | ';
  table += languages.map(lang => results.get(lang)!.tokensPerLine.toFixed(2)).join(' | ');
  table += ' |\n';

  // Bytes per token
  table += '| **Bytes/Token** | ';
  table += languages.map(lang => results.get(lang)!.bytesPerToken.toFixed(2)).join(' | ');
  table += ' |\n';

  return table;
}

/**
 * Calculate efficiency vs baseline (TypeScript)
 */
export function calculateEfficiency(results: Map<string, TokenMetrics>): Map<string, number> {
  const baseline = results.get('TypeScript');
  if (!baseline) {
    throw new Error('TypeScript baseline not found');
  }

  const efficiency = new Map<string, number>();

  for (const [lang, metrics] of results) {
    if (lang === 'TypeScript') {
      efficiency.set(lang, 1.0);
    } else {
      // Lower LLM token count = higher efficiency for training
      efficiency.set(lang, baseline.llmTokens / metrics.llmTokens);
    }
  }

  return efficiency;
}

/**
 * Cleanup encoder on exit
 */
export function cleanup() {
  encoder.free();
}
