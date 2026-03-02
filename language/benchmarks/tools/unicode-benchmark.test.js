import assert from 'assert';
import { buildBoundarySnippet, rewriteFileForTokenType } from './sigil-rewrite.js';
import { measureStringWithAllTokenizers } from './tokenizers.js';

const lambdaFile = {
  source: 'λends_with(s:𝕊,suffix:𝕊)→𝔹=false\n',
  tokens: [
    { type: 'LAMBDA', start: { offset: 0, index: 0 }, end: { offset: 1, index: 1 } },
    { type: 'IDENTIFIER', start: { offset: 1, index: 1 }, end: { offset: 10, index: 10 } },
    { type: 'LPAREN', start: { offset: 10, index: 10 }, end: { offset: 11, index: 11 } }
  ],
  symbols: [
    { type: 'LAMBDA', start: { offset: 0, index: 0 }, end: { offset: 1, index: 1 } }
  ]
};

const rewrittenLambda = rewriteFileForTokenType(lambdaFile, 'LAMBDA', 'function');
assert.equal(rewrittenLambda, 'function ends_with(s:𝕊,suffix:𝕊)→𝔹=false\n');

const snippet = buildBoundarySnippet(lambdaFile, lambdaFile.symbols[0], 'fn', 8);
assert.equal(snippet.before, 'λends_wit');
assert.equal(snippet.after, 'fn ends_wit');

const exactCounts = measureStringWithAllTokenizers('λ');
assert.ok(exactCounts.openai_cl100k_base >= 1);

console.log('unicode-benchmark tests passed');
