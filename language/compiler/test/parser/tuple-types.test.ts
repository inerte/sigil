import { describe, test } from 'node:test';
import assert from 'node:assert';
import { compileFromString } from '../../src/api.js';

describe('Tuple type parsing', () => {
  test('parses basic tuple type (ℤ, 𝕊)', () => {
    const code = 't Pair=(ℤ,𝕊)\n\nλmain()→Pair=(42,"hello")\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses three-element tuple type (ℤ, 𝕊, 𝔹)', () => {
    const code = 't Triple=(ℤ,𝕊,𝔹)\n\nλmain()→Triple=(1,"a",⊤)\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses nested tuple types ((ℤ, 𝕊), 𝔹)', () => {
    const code = 't Nested=((ℤ,𝕊),𝔹)\n\nλmain()→Nested=((1,"a"),⊤)\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses tuple in list type [(ℤ,𝕊)]', () => {
    const code = 't Pairs=[(ℤ,𝕊)]\n\nλmain()→Pairs=[(1,"a"),(2,"b")]\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses tuple with list elements ([ℤ], 𝕊)', () => {
    const code = 't T=([ℤ],𝕊)\n\nλmain()→T=([1,2],"hello")\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test.skip('parses single-element tuple with trailing comma (ℤ,)', () => {
    // Skip: Single-element tuple support needs type system work
    const code = 't Single=(ℤ,)\n\nλmain()→Single=(42,)\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses grouped type (ℤ) as just ℤ', () => {
    const code = 't UserId=(𝕊)\n\nλmain()→UserId="user123"\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses tuple in function parameter', () => {
    const code = 'λmain()→(𝕊,ℤ)=swap((1,"x"))\n\nλswap(pair:(ℤ,𝕊))→(𝕊,ℤ)≡pair{(a,b)→(b,a)}\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses tuple in function return type', () => {
    const code = 'λmain()→(ℤ,𝕊)=makePair(1,"a")\n\nλmakePair(x:ℤ,y:𝕊)→(ℤ,𝕊)=(x,y)\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses complex nested structures [(𝕊, 𝕊)]', () => {
    const code = 't Headers=[(𝕊,𝕊)]\n\nλmain()→Headers=[("Content-Type","application/json"),("Accept","text/html")]\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test.skip('parses tuple with map type ({𝕊:ℤ}, 𝕊)', () => {
    // Skip: Empty char literal issue
    const code = 't T=({𝕊:ℤ},𝕊)\n\nλmain()→T=({},\'\')\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses three-element tuple with ints and strings', () => {
    const code = 't Tri=(ℤ,𝕊,ℤ)\n\nλmain()→Tri=(1,"a",2)\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses tuple with function types (λ(ℤ)→𝕊, ℤ)', () => {
    const code = 't T=(λ(ℤ)→𝕊,ℤ)\n\nλid(x:ℤ)→𝕊=""\n\nλmain()→T=(id,42)\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses deeply nested tuples with ints and strings', () => {
    const code = 't Deep=(((ℤ,𝕊),𝔹),(ℤ,𝕊))\n\nλmain()→Deep=(((1,"a"),⊤),(2,"b"))\n';
    const result = compileFromString(code);

    assert.strictEqual(result.ok, true);
  });

  test('parses list of tuples as HTTP headers', () => {
    const code = 't Headers=[(𝕊,𝕊)]\n\nλmain()→Headers=[("Content-Type","application/json")]\n';
    const result = compileFromString(code);

    if (!result.ok) {
      console.log('Headers error:', result.error);
    }
    assert.strictEqual(result.ok, true);
  });
});
