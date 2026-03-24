import { get_encoding } from 'tiktoken';

const openAiEncoding = get_encoding('cl100k_base');

const COMMON_WORDS = new Set([
  'and',
  'append',
  'bool',
  'char',
  'def',
  'false',
  'filter',
  'float',
  'fn',
  'fold',
  'function',
  'if',
  'import',
  'int',
  'lambda',
  'map',
  'match',
  'mock',
  'never',
  'not',
  'or',
  'reduce',
  'string',
  'switch',
  'test',
  'true',
  'unit',
  'with',
  'world'
]);

const ASCII_OPERATORS = new Set([
  '->',
  '=>',
  '!=',
  '<=',
  '>=',
  '++',
  '.',
  '::',
  '+',
  '-',
  '*',
  '/',
  '%',
  '=',
  '<',
  '>',
  ':',
  ';',
  ',',
  '(',
  ')',
  '[',
  ']',
  '{',
  '}',
  '|',
  '#'
]);

const UNICODE_TOKEN_COSTS = {
  llama_sentencepiece_proxy: {
    '⊤': 2,
    '⊥': 2,
    'λ': 2,
    '≡': 2,
    '¬': 2,
    '∧': 2,
    '∨': 2,
    'Int': 2,
    'Float': 2,
    'Bool': 2,
    'String': 2,
    'Char': 2,
    'Unit': 2,
    'Never': 2,
    '≠': 2,
    '≤': 2,
    '≥': 2,
    '⧺': 2,
    '↦': 2
  },
  anthropic_legacy_proxy: {
    '⊤': 2,
    '⊥': 2,
    'λ': 2,
    '≡': 2,
    '¬': 2,
    '∧': 2,
    '∨': 2,
    'Int': 2,
    'Float': 2,
    'Bool': 2,
    'String': 2,
    'Char': 2,
    'Unit': 2,
    'Never': 2,
    '≠': 2,
    '≤': 2,
    '≥': 2,
    '⧺': 2,
    '↦': 2
  }
};

const TOKENIZER_DEFS = [
  {
    id: 'openai_cl100k_base',
    label: 'OpenAI cl100k_base',
    exact: true,
    tokenize(source) {
      return openAiEncoding.encode(source).length;
    }
  },
  {
    id: 'llama_sentencepiece_proxy',
    label: 'Local SentencePiece/Llama heuristic proxy',
    exact: false,
    tokenize(source) {
      return approximateTokenCount(source, 'llama_sentencepiece_proxy');
    }
  },
  {
    id: 'anthropic_legacy_proxy',
    label: 'Local Anthropic legacy heuristic proxy',
    exact: false,
    tokenize(source) {
      return approximateTokenCount(source, 'anthropic_legacy_proxy');
    }
  }
];

function splitIdentifier(identifier) {
  return identifier
    .split(/[_-]+/g)
    .flatMap((part) => part.split(/(?=[A-Z])/g))
    .filter(Boolean)
    .map((part) => part.toLowerCase());
}

function approximateWordCost(word, profile) {
  const lower = word.toLowerCase();
  if (COMMON_WORDS.has(lower)) {
    return 1;
  }

  const parts = splitIdentifier(word);
  if (parts.length > 1) {
    return parts.reduce((total, part) => total + approximateWordCost(part, profile), 0);
  }

  const divisor = profile === 'llama_sentencepiece_proxy' ? 4 : 5;
  return Math.max(1, Math.ceil(word.length / divisor));
}

function approximateSegmentCost(segment, profile) {
  if (!segment || /^\s+$/u.test(segment)) {
    return 0;
  }

  if (ASCII_OPERATORS.has(segment)) {
    return 1;
  }

  if (/^[A-Za-z_][A-Za-z0-9_]*$/u.test(segment)) {
    return approximateWordCost(segment, profile);
  }

  if (/^\d+$/u.test(segment)) {
    return Math.max(1, Math.ceil(segment.length / 3));
  }

  if (UNICODE_TOKEN_COSTS[profile][segment] != null) {
    return UNICODE_TOKEN_COSTS[profile][segment];
  }

  return [...segment].reduce((total, char) => {
    if (ASCII_OPERATORS.has(char)) {
      return total + 1;
    }
    if (UNICODE_TOKEN_COSTS[profile][char] != null) {
      return total + UNICODE_TOKEN_COSTS[profile][char];
    }
    if (/[A-Za-z0-9_]/u.test(char)) {
      return total + 1;
    }
    return total + 2;
  }, 0);
}

function approximateTokenCount(source, profile) {
  const segments = source.match(/[A-Za-z_][A-Za-z0-9_]*|\d+|::|->|=>|!=|<=|>=|\+\+|\s+|./gu) || [];
  return segments.reduce((total, segment) => total + approximateSegmentCost(segment, profile), 0);
}

export function getTokenizerDefinitions() {
  return TOKENIZER_DEFS.map(({ id, label, exact }) => ({ id, label, exact }));
}

export function measureSourceWithAllTokenizers(source) {
  const counts = {};
  for (const tokenizer of TOKENIZER_DEFS) {
    counts[tokenizer.id] = tokenizer.tokenize(source);
  }
  return counts;
}

export function measureStringWithAllTokenizers(text) {
  return measureSourceWithAllTokenizers(text);
}

export function cleanupTokenizers() {
  openAiEncoding.free();
}
