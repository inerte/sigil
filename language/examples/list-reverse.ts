const __sigil_mocks = new Map();
function __sigil_preview(value) {
  try { return JSON.stringify(value); } catch { return String(value); }
}
function __sigil_diff_hint(actual, expected) {
  if (Array.isArray(actual) && Array.isArray(expected)) {
    if (actual.length !== expected.length) { return { kind: 'array_length', actualLength: actual.length, expectedLength: expected.length }; }
    for (let i = 0; i < actual.length; i++) { if (actual[i] !== expected[i]) { return { kind: 'array_first_diff', index: i, actual: __sigil_preview(actual[i]), expected: __sigil_preview(expected[i]) }; } }
    return null;
  }
  if (actual && expected && typeof actual === 'object' && typeof expected === 'object') {
    const actualKeys = Object.keys(actual).sort();
    const expectedKeys = Object.keys(expected).sort();
    if (actualKeys.join('|') !== expectedKeys.join('|')) { return { kind: 'object_keys', actualKeys, expectedKeys }; }
    for (const k of actualKeys) { if (actual[k] !== expected[k]) { return { kind: 'object_field', field: k, actual: __sigil_preview(actual[k]), expected: __sigil_preview(expected[k]) }; } }
    return null;
  }
  return null;
}
async function __sigil_test_bool_result(ok) {
  const result = await ok;
  return result === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to ⊥' } };
}
async function __sigil_test_compare_result(op, leftFn, rightFn) {
  const actual = await leftFn();
  const expected = await rightFn();
  let ok = false;
  switch (op) {
    case '=': ok = actual === expected; break;
    case '≠': ok = actual !== expected; break;
    case '<': ok = actual < expected; break;
    case '>': ok = actual > expected; break;
    case '≤': ok = actual <= expected; break;
    case '≥': ok = actual >= expected; break;
    default: throw new Error('Unsupported test comparison operator: ' + String(op));
  }
  if (ok) { return { ok: true }; }
  return { ok: false, failure: { kind: 'comparison_mismatch', message: 'Comparison test failed', operator: op, actual: __sigil_preview(actual), expected: __sigil_preview(expected), diffHint: __sigil_diff_hint(actual, expected) } };
}
async function __sigil_call(key, actualFn, args) {
  const mockFn = __sigil_mocks.get(key);
  const fn = mockFn ?? actualFn;
  return await fn(...args);
}
async function __sigil_with_mock(key, mockFn, body) {
  const had = __sigil_mocks.has(key);
  const prev = __sigil_mocks.get(key);
  __sigil_mocks.set(key, mockFn);
  try {
    return await body();
  } finally {
    if (had) { __sigil_mocks.set(key, prev); } else { __sigil_mocks.delete(key); }
  }
}
async function __sigil_with_mock_extern(key, actualFn, mockFn, body) {
  if (typeof actualFn !== 'function') { throw new Error('with_mock extern target is not callable'); }
  if (typeof mockFn !== 'function') { throw new Error('with_mock replacement must be callable'); }
  if (actualFn.length !== mockFn.length) { throw new Error(`with_mock extern arity mismatch for ${key}: expected ${actualFn.length}, got ${mockFn.length}`); }
  return await __sigil_with_mock(key, mockFn, body);
}
async function reverse(lst) {
  return (async () => {
  const __match = await lst;
  if (__match.length === 0) {
    return [];
  }
  else if (__match.length >= 1) {
    const x = __match[0]; const xs = __match.slice(1);
    return await reverse(xs).concat([].concat(x));
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export async function main() {
  return await reverse([].concat([1], [2], [3], [4], [5]));
}

