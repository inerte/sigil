import fs from 'fs';
import path from 'path';
import { execFileSync, execSync } from 'child_process';
import { fileURLToPath } from 'url';

const toolsDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(toolsDir, '../../../..');
const compilerManifest = path.join(repoRoot, 'language/compiler/Cargo.toml');
const sigilBinary = path.join(repoRoot, 'language/compiler/target/debug/sigil');

let sigilReady = false;

function buildLineStartIndexMap(source) {
  const lineStarts = [0];

  for (let index = 0; index < source.length; index += 1) {
    if (source[index] === '\n') {
      lineStarts.push(index + 1);
    }
  }

  return lineStarts;
}

function indexFromLineColumn(lineStarts, position) {
  const lineIndex = position.line - 1;
  const lineStart = lineStarts[lineIndex];

  if (lineStart === undefined) {
    throw new Error(`Invalid lexer position line ${position.line}`);
  }

  return lineStart + position.column - 1;
}

function ensureSigilBinary() {
  if (!sigilReady) {
    if (!fs.existsSync(sigilBinary)) {
      execFileSync('cargo', ['build', '--quiet', '--manifest-path', compilerManifest, '-p', 'sigil-cli'], {
        cwd: repoRoot,
        stdio: 'inherit'
      });
    }
    sigilReady = true;
  }
  return sigilBinary;
}

export function getRepoRoot() {
  return repoRoot;
}

export function findSigilFiles() {
  const output = execSync(
    "find . \\( -path './node_modules' -o -path './target' -o -path './.git' -o -path '*/.local' -o -path '*/node_modules' -o -path '*/target' \\) -prune -o -name '*.sigil' -type f -print | sort",
    {
      cwd: repoRoot,
      encoding: 'utf8'
    }
  );

  return output
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((relativePath) => path.resolve(repoRoot, relativePath.replace(/^\.\//, '')));
}

export function lexSigilFile(filePath) {
  const binary = ensureSigilBinary();
  const output = execFileSync(binary, ['lex', filePath], {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024
  });
  return JSON.parse(output);
}

export function loadSigilCorpus(candidateConfig) {
  const files = findSigilFiles();
  return files.map((filePath) => {
    const source = fs.readFileSync(filePath, 'utf8');
    const lineStarts = buildLineStartIndexMap(source);
    const lexed = lexSigilFile(filePath);
    const tokens = lexed.data.tokens
      .filter((token) => token.type !== 'EOF')
      .map((token) => ({
        ...token,
        start: {
          ...token.start,
          index: indexFromLineColumn(lineStarts, token.start)
        },
        end: {
          ...token.end,
          index: indexFromLineColumn(lineStarts, token.end)
        },
        absolutePath: filePath,
        relativePath: path.relative(repoRoot, filePath)
      }));

    const symbols = tokens.filter((token) => candidateConfig[token.type]);

    return {
      filePath,
      relativePath: path.relative(repoRoot, filePath),
      source,
      tokens,
      symbols
    };
  });
}

export function buildSymbolInventory(corpus, candidateConfig) {
  const symbolMap = new Map();

  for (const file of corpus) {
    for (const token of file.symbols) {
      const config = candidateConfig[token.type];
      const key = token.type;
      if (!symbolMap.has(key)) {
        symbolMap.set(key, {
          tokenType: token.type,
          symbol: config.symbol,
          category: config.category,
          occurrences: 0,
          filesAffected: new Set(),
          sampleFiles: []
        });
      }

      const entry = symbolMap.get(key);
      entry.occurrences += 1;
      entry.filesAffected.add(file.relativePath);
      if (entry.sampleFiles.length < 5 && !entry.sampleFiles.includes(file.relativePath)) {
        entry.sampleFiles.push(file.relativePath);
      }
    }
  }

  return Array.from(symbolMap.values())
    .map((entry) => ({
      tokenType: entry.tokenType,
      symbol: entry.symbol,
      category: entry.category,
      occurrences: entry.occurrences,
      filesAffected: entry.filesAffected.size,
      sampleFiles: entry.sampleFiles
    }))
    .sort((left, right) => right.occurrences - left.occurrences || left.symbol.localeCompare(right.symbol));
}
