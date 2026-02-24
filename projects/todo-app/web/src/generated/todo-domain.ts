const __mint_mocks = new Map();
function __mint_preview(value) {
  try { return JSON.stringify(value); } catch { return String(value); }
}
function __mint_diff_hint(actual, expected) {
  if (Array.isArray(actual) && Array.isArray(expected)) {
    if (actual.length !== expected.length) { return { kind: 'array_length', actualLength: actual.length, expectedLength: expected.length }; }
    for (let i = 0; i < actual.length; i++) { if (actual[i] !== expected[i]) { return { kind: 'array_first_diff', index: i, actual: __mint_preview(actual[i]), expected: __mint_preview(expected[i]) }; } }
    return null;
  }
  if (actual && expected && typeof actual === 'object' && typeof expected === 'object') {
    const actualKeys = Object.keys(actual).sort();
    const expectedKeys = Object.keys(expected).sort();
    if (actualKeys.join('|') !== expectedKeys.join('|')) { return { kind: 'object_keys', actualKeys, expectedKeys }; }
    for (const k of actualKeys) { if (actual[k] !== expected[k]) { return { kind: 'object_field', field: k, actual: __mint_preview(actual[k]), expected: __mint_preview(expected[k]) }; } }
    return null;
  }
  return null;
}
function __mint_test_bool_result(ok) {
  return ok === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to ⊥' } };
}
function __mint_test_compare_result(op, leftFn, rightFn) {
  const actual = leftFn();
  const expected = rightFn();
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
  return { ok: false, failure: { kind: 'comparison_mismatch', message: 'Comparison test failed', operator: op, actual: __mint_preview(actual), expected: __mint_preview(expected), diffHint: __mint_diff_hint(actual, expected) } };
}
function __mint_call(key, actualFn, args) {
  const mockFn = __mint_mocks.get(key);
  const fn = mockFn ?? actualFn;
  return fn(...args);
}
function __mint_with_mock(key, mockFn, body) {
  const had = __mint_mocks.has(key);
  const prev = __mint_mocks.get(key);
  __mint_mocks.set(key, mockFn);
  try {
    return body();
  } finally {
    if (had) { __mint_mocks.set(key, prev); } else { __mint_mocks.delete(key); }
  }
}
function __mint_with_mock_extern(key, actualFn, mockFn, body) {
  if (typeof actualFn !== 'function') { throw new Error('with_mock extern target is not callable'); }
  if (typeof mockFn !== 'function') { throw new Error('with_mock replacement must be callable'); }
  if (actualFn.length !== mockFn.length) { throw new Error(`with_mock extern arity mismatch for ${key}: expected ${actualFn.length}, got ${mockFn.length}`); }
  return __mint_with_mock(key, mockFn, body);
}
// type Todo (erased)

export function canAdd(text) {
  return (text !== "");
}

export function addTodo(todos, id, text) {
  return [{ "id": id, "text": text, "done": false }].concat(todos);
}

export function deleteTodo(todos, targetId) {
  return todos.filter(((todo) => (todo.id !== targetId)));
}

export function clearCompleted(todos) {
  return todos.filter(((todo) => !todo.done));
}

export function toggleTodo(todos, targetId) {
  return todos.map(((todo) => (() => {
  const __match = (todo.id === targetId);
  if (__match === true) {
    return { "id": todo.id, "text": todo.text, "done": !todo.done };
  }
  else if (__match === false) {
    return todo;
  }
  throw new Error('Match failed: no pattern matched');
})()));
}

export function editTodo(todos, targetId, newText) {
  return todos.map(((todo) => (() => {
  const __match = (todo.id === targetId);
  if (__match === true) {
    return { "id": todo.id, "text": newText, "done": todo.done };
  }
  else if (__match === false) {
    return todo;
  }
  throw new Error('Match failed: no pattern matched');
})()));
}

export function isVisible(filter, done) {
  return (() => {
  const __match = filter;
  if (__match === "all") {
    return true;
  }
  else if (__match === "active") {
    return !done;
  }
  else if (__match === "completed") {
    return done;
  }
  else {
    return true;
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export function completedCount(todos) {
  return todos.reduce(((acc, todo) => (() => {
  const __match = todo.done;
  if (__match === true) {
    return (acc + 1);
  }
  else if (__match === false) {
    return acc;
  }
  throw new Error('Match failed: no pattern matched');
})()), 0);
}

export function remainingCount(total, completed) {
  return (total - completed);
}

