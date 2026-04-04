function __sigil_ready(value) {
  return Promise.resolve(value);
}
function __sigil_all(values) {
  return values.reduce(async (__sigil_acc_promise, __sigil_thunk) => {
    const __sigil_acc = await __sigil_acc_promise;
    __sigil_acc.push(await __sigil_thunk());
    return __sigil_acc;
  }, Promise.resolve([]));
}
function __sigil_sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, Math.max(0, ms)));
}
function __sigil_option_value(option) {
  return option && option.__tag === 'Some' ? option.__fields[0] : null;
}
function __sigil_record_coverage_call(moduleId, functionName) {
  const state = globalThis.__sigil_coverage_current;
  if (!state) {
    return;
  }
  const key = `${String(moduleId)}::${String(functionName)}`;
  state.calls[key] = Number(state.calls[key] ?? 0) + 1;
}
function __sigil_record_coverage_variant(moduleId, functionName, value) {
  const state = globalThis.__sigil_coverage_current;
  if (!state || !value || typeof value !== 'object' || typeof value.__tag !== 'string') {
    return;
  }
  const key = `${String(moduleId)}::${String(functionName)}`;
  if (!Array.isArray(state.variants[key])) {
    state.variants[key] = [];
  }
  const tag = String(value.__tag);
  if (!state.variants[key].includes(tag)) {
    state.variants[key].push(tag);
  }
}
function __sigil_record_coverage_result(moduleId, functionName, result) {
  if (result && typeof result.then === 'function') {
    return result.then((value) => {
      __sigil_record_coverage_variant(moduleId, functionName, value);
      return value;
    });
  }
  __sigil_record_coverage_variant(moduleId, functionName, result);
  return result;
}
async function __sigil_map_list(items, fn) {
  const results = [];
  for (const item of items) {
    results.push(await fn(item));
  }
  return results;
}
async function __sigil_filter_list(items, predicate) {
  const results = [];
  for (const item of items) {
    if (await predicate(item)) {
      results.push(item);
    }
  }
  return results;
}
async function __sigil_concurrent_region(name, config, tasks) {
  const concurrency = Math.max(1, Number(config.concurrency));
  const jitter = __sigil_option_value(config.jitterMs);
  const stopOn = config.stopOn;
  const windowMs = __sigil_option_value(config.windowMs);
  const outcomes = new Array(tasks.length);
  const startTimes = [];
  let nextIndex = 0;
  let stopRequested = false;
  function abortedOutcome() { return { __tag: 'Aborted', __fields: [] }; }
  function failureOutcome(errorValue) { return { __tag: 'Failure', __fields: [errorValue] }; }
  function successOutcome(value) { return { __tag: 'Success', __fields: [value] }; }
  async function waitForWindowSlot() {
    if (windowMs === null) return;
    while (true) {
      const now = Date.now();
      while (startTimes.length > 0 && now - startTimes[0] >= windowMs) {
        startTimes.shift();
      }
      if (startTimes.length < concurrency) return;
      await __sigil_sleep(startTimes[0] + windowMs - now);
    }
  }
  function jitterDelayMs() {
    if (jitter === null) return 0;
    const min = Number(jitter.min);
    const max = Number(jitter.max);
    if (!Number.isFinite(min) || !Number.isFinite(max)) return 0;
    if (max <= min) return Math.max(0, min);
    return Math.floor(Math.random() * (max - min + 1)) + min;
  }
  async function worker() {
    while (true) {
      const index = nextIndex;
      if (index >= tasks.length) return;
      nextIndex += 1;
      if (stopRequested) {
        outcomes[index] = abortedOutcome();
        continue;
      }
      await waitForWindowSlot();
      if (stopRequested) {
        outcomes[index] = abortedOutcome();
        continue;
      }
      const delay = jitterDelayMs();
      if (delay > 0) {
        await __sigil_sleep(delay);
      }
      startTimes.push(Date.now());
      const result = await tasks[index]();
      if (result && result.__tag === 'Ok') {
        outcomes[index] = successOutcome(result.__fields[0]);
        continue;
      }
      if (!result || result.__tag !== 'Err') {
        throw new Error(`Concurrent region ${name} child returned a non-Result value`);
      }
      const errorValue = result.__fields[0];
      outcomes[index] = failureOutcome(errorValue);
      if (await stopOn(errorValue)) {
        stopRequested = true;
      }
    }
  }
  await Promise.all(Array.from({ length: concurrency }, () => worker()));
  return outcomes.map((outcome) => outcome ?? abortedOutcome());
}
function __sigil_map_empty() {
  return { __sigil_map: [] };
}
function __sigil_map_from_entries(entries) {
  let current = __sigil_map_empty();
  for (const [key, value] of entries) { current = __sigil_map_insert(current, key, value); }
  return current;
}
function __sigil_map_get(map, key) {
  for (const [entryKey, entryValue] of map.__sigil_map) { if (__sigil_deep_equal(entryKey, key)) return { __tag: "Some", __fields: [entryValue] }; }
  return { __tag: "None", __fields: [] };
}
function __sigil_map_has(map, key) {
  for (const [entryKey] of map.__sigil_map) { if (__sigil_deep_equal(entryKey, key)) return true; }
  return false;
}
function __sigil_map_insert(map, key, value) {
  const next = [];
  let replaced = false;
  for (const [entryKey, entryValue] of map.__sigil_map) {
    if (__sigil_deep_equal(entryKey, key)) { if (!replaced) { next.push([key, value]); replaced = true; } } else { next.push([entryKey, entryValue]); }
  }
  if (!replaced) next.push([key, value]);
  return { __sigil_map: next };
}
function __sigil_map_remove(map, key) {
  return { __sigil_map: map.__sigil_map.filter(([entryKey]) => !__sigil_deep_equal(entryKey, key)) };
}
function __sigil_map_entries(map) {
  return map.__sigil_map.slice();
}
function __sigil_json_from_js(value) {
  if (value === null) return { __tag: "JsonNull", __fields: [] };
  if (Array.isArray(value)) return { __tag: "JsonArray", __fields: [value.map(__sigil_json_from_js)] };
  if (typeof value === 'boolean') return { __tag: "JsonBool", __fields: [value] };
  if (typeof value === 'number') return { __tag: "JsonNumber", __fields: [value] };
  if (typeof value === 'string') return { __tag: "JsonString", __fields: [value] };
  if (typeof value === 'object') {
    return { __tag: "JsonObject", __fields: [__sigil_map_from_entries(Object.entries(value).map(([k, v]) => [k, __sigil_json_from_js(v)]))] };
  }
  return { __tag: "JsonNull", __fields: [] };
}
function __sigil_json_to_js(value) {
  if (!value || typeof value !== 'object') return null;
  switch (value.__tag) {
    case 'JsonArray': return (value.__fields[0] ?? []).map(__sigil_json_to_js);
    case 'JsonBool': return !!value.__fields[0];
    case 'JsonNull': return null;
    case 'JsonNumber': return Number(value.__fields[0]);
    case 'JsonObject': {
      const result = {};
      for (const [k, v] of __sigil_map_entries(value.__fields[0] ?? __sigil_map_empty())) { result[String(k)] = __sigil_json_to_js(v); }
      return result;
    }
    case 'JsonString': return String(value.__fields[0] ?? '');
    default: return null;
  }
}
function __sigil_json_parse_result(input) {
  try {
    return { __tag: "Ok", __fields: [__sigil_json_from_js(JSON.parse(input))] };
  } catch (error) {
    return { __tag: "Err", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };
  }
}
function __sigil_json_stringify_value(value) {
  return JSON.stringify(__sigil_json_to_js(value));
}
function __sigil_time_is_iso(input) {
  return /^\d{4}-\d{2}-\d{2}(?:T\d{2}:\d{2}:\d{2}(?:\.\d{3})?(?:Z|[+-]\d{2}:\d{2}))?$/.test(input);
}
function __sigil_time_parse_iso_result(input) {
  if (!__sigil_time_is_iso(input)) {
    return { __tag: "Err", __fields: [{ message: "invalid ISO-8601 timestamp" }] };
  }
  const millis = Date.parse(input);
  if (Number.isNaN(millis)) {
    return { __tag: "Err", __fields: [{ message: "invalid ISO-8601 timestamp" }] };
  }
  return { __tag: "Ok", __fields: [{ epochMillis: millis }] };
}
function __sigil_time_format_iso(instant) {
  return new Date(instant.epochMillis).toISOString();
}
function __sigil_time_now_instant() {
  return { epochMillis: Date.now() };
}
function __sigil_regex_compile_result(flags, pattern) {
  try {
    const normalizedFlags = String(flags ?? '');
    const normalizedPattern = String(pattern ?? '');
    new RegExp(normalizedPattern, normalizedFlags);
    return { __tag: "Ok", __fields: [{ flags: normalizedFlags, pattern: normalizedPattern }] };
  } catch (error) {
    return { __tag: "Err", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };
  }
}
function __sigil_regex_find(regex, input) {
  try {
    const compiled = new RegExp(String(regex?.pattern ?? ''), String(regex?.flags ?? ''));
    const source = String(input ?? '');
    const match = compiled.exec(source);
    if (!match) { return { __tag: "None", __fields: [] }; }
    return { __tag: "Some", __fields: [{ captures: match.slice(1).map((value) => value ?? ''), end: match.index + match[0].length, full: match[0], start: match.index }] };
  } catch (_) {
    return { __tag: "None", __fields: [] };
  }
}
function __sigil_regex_is_match(regex, input) {
  try {
    return new RegExp(String(regex?.pattern ?? ''), String(regex?.flags ?? '')).test(String(input ?? ''));
  } catch (_) {
    return false;
  }
}
function __sigil_url_query_map_from_search(search) {
  const params = new URLSearchParams(search);
  return __sigil_map_from_entries(Array.from(params.entries()));
}
function __sigil_url_from_absolute(absolute) {
  const protocol = absolute.protocol.endsWith(':') ? absolute.protocol.slice(0, -1) : absolute.protocol;
  const port = absolute.port.length > 0 ? { __tag: "Some", __fields: [Number(absolute.port)] } : { __tag: "None", __fields: [] };
  return {
    fragment: absolute.hash || '',
    host: absolute.hostname || '',
    path: absolute.pathname || '',
    port,
    protocol,
    query: __sigil_url_query_map_from_search(absolute.search || ''),
    query_string: absolute.search || ''
  };
}
function __sigil_url_from_relative(input) {
  const fragmentIndex = input.indexOf('#');
  const fragment = fragmentIndex >= 0 ? input.slice(fragmentIndex) : '';
  const withoutFragment = fragmentIndex >= 0 ? input.slice(0, fragmentIndex) : input;
  const queryIndex = withoutFragment.indexOf('?');
  const path = queryIndex >= 0 ? withoutFragment.slice(0, queryIndex) : withoutFragment;
  const queryString = queryIndex >= 0 ? withoutFragment.slice(queryIndex) : '';
  return {
    fragment,
    host: '',
    path,
    port: { __tag: "None", __fields: [] },
    protocol: '',
    query: __sigil_url_query_map_from_search(queryString),
    query_string: queryString
  };
}
function __sigil_url_parse_result(input) {
  try {
    const absolutePattern = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;
    if (absolutePattern.test(input)) {
      return { __tag: "Ok", __fields: [__sigil_url_from_absolute(new URL(input))] };
    }
    return { __tag: "Ok", __fields: [__sigil_url_from_relative(input)] };
  } catch (error) {
    return { __tag: "Err", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };
  }
}
function __sigil_http_error(kind, message) {
  return { kind: { __tag: kind, __fields: [] }, message: String(message) };
}
function __sigil_http_headers_from_entries(entries) {
  return __sigil_map_from_entries(entries.map(([key, value]) => [String(key).toLowerCase(), String(value)]));
}
function __sigil_http_header_value(value) {
  if (Array.isArray(value)) return value.map((item) => String(item)).join(', ');
  if (value === undefined || value === null) return null;
  return String(value);
}
function __sigil_http_headers_from_node(headers) {
  return __sigil_http_headers_from_entries(Object.entries(headers ?? {}).flatMap(([key, value]) => {
    const normalized = __sigil_http_header_value(value);
    return normalized === null ? [] : [[key, normalized]];
  }));
}
function __sigil_http_headers_from_web(headers) {
  return __sigil_http_headers_from_entries(Array.from(headers.entries()));
}
function __sigil_http_headers_to_js(headers) {
  const result = {};
  for (const [key, value] of __sigil_map_entries(headers ?? __sigil_map_empty())) { result[String(key)] = String(value); }
  return result;
}
function __sigil_http_method_to_string(method) {
  switch (method?.__tag) {
    case 'Delete': return 'DELETE';
    case 'Get': return 'GET';
    case 'Patch': return 'PATCH';
    case 'Post': return 'POST';
    case 'Put': return 'PUT';
    default: return 'GET';
  }
}
function __sigil_http_request_path(url) {
  try {
    const parsed = new URL(String(url ?? '/'), 'http://127.0.0.1');
    return parsed.pathname || '/';
  } catch (_) {
    return '/';
  }
}
function __sigil_tcp_error(kind, message) {
  return { kind: { __tag: kind, __fields: [] }, message: String(message) };
}
function __sigil_tcp_is_valid_host(host) {
  return typeof host === 'string' && host.length > 0;
}
function __sigil_tcp_is_valid_port(port) {
  return Number.isInteger(port) && port > 0 && port <= 65535;
}
function __sigil_tcp_first_line(buffer) {
  const index = buffer.indexOf('\n');
  return index === -1 ? null : buffer.slice(0, index).replace(/\r$/, '');
}
function __sigil_is_map(value) {
  return !!value && typeof value === 'object' && Array.isArray(value.__sigil_map);
}
function __sigil_deep_equal(a, b) {
  if (a === b) return true;
  if (a == null || b == null) return false;
  if (typeof a !== typeof b) return false;
  if (__sigil_is_map(a) && __sigil_is_map(b)) {
    if (a.__sigil_map.length !== b.__sigil_map.length) return false;
    for (const [aKey, aValue] of a.__sigil_map) {
      let matched = false;
      for (const [bKey, bValue] of b.__sigil_map) {
        if (__sigil_deep_equal(aKey, bKey)) {
          if (!__sigil_deep_equal(aValue, bValue)) return false;
          matched = true;
          break;
        }
      }
      if (!matched) return false;
    }
    return true;
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!__sigil_deep_equal(a[i], b[i])) return false;
    }
    return true;
  }
  if (typeof a === 'object' && typeof b === 'object') {
    const aKeys = Object.keys(a).sort();
    const bKeys = Object.keys(b).sort();
    if (aKeys.length !== bKeys.length) return false;
    for (let i = 0; i < aKeys.length; i++) {
      if (aKeys[i] !== bKeys[i]) return false;
      if (!__sigil_deep_equal(a[aKeys[i]], b[bKeys[i]])) return false;
    }
    return true;
  }
  return false;
}
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
  return result === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to false' } };
}
async function __sigil_test_compare_result(op, leftFn, rightFn) {
  const actual = await leftFn();
  const expected = await rightFn();
  let ok = false;
  switch (op) {
    case '=': ok = __sigil_deep_equal(actual, expected); break;
    case '≠': ok = !__sigil_deep_equal(actual, expected); break;
    case '<': ok = actual < expected; break;
    case '>': ok = actual > expected; break;
    case '≤': ok = actual <= expected; break;
    case '≥': ok = actual >= expected; break;
    default: throw new Error('Unsupported test comparison operator: ' + String(op));
  }
  if (ok) { return { ok: true }; }
  return { ok: false, failure: { kind: 'comparison_mismatch', message: 'Comparison test failed', operator: op, actual: __sigil_preview(actual), expected: __sigil_preview(expected), diffHint: __sigil_diff_hint(actual, expected) } };
}
function __sigil_call(_key, actualFn, args = []) {
  const __sigil_run = () => Promise.resolve().then(() => {
    switch (_key) {
      default:
        return actualFn(...args);
    }
  });
  return __sigil_run();
}
import { Some, None, Ok, Err, Aborted, Failure, Success } from './core/prelude.js';

export function addTodo(id, text, todos) {
  __sigil_record_coverage_call("src::todoDomain", "addTodo");
  return __sigil_record_coverage_result("src::todoDomain", "addTodo", __sigil_all([() => __sigil_all([() => __sigil_all([() => __sigil_ready(false), () => __sigil_ready(id), () => __sigil_ready(text)]).then((__values) => ({ "done": __values[0], "id": __values[1], "text": __values[2] }))]).then((__items) => __items), () => __sigil_ready(todos)]).then(([__left, __right]) => __left.concat(__right)));
}

export function canAdd(text) {
  __sigil_record_coverage_call("src::todoDomain", "canAdd");
  return __sigil_record_coverage_result("src::todoDomain", "canAdd", __sigil_all([() => __sigil_ready(text), () => __sigil_ready("")]).then(([__left, __right]) => !__sigil_deep_equal(__left, __right)));
}

export function clearCompleted(todos) {
  __sigil_record_coverage_call("src::todoDomain", "clearCompleted");
  return __sigil_record_coverage_result("src::todoDomain", "clearCompleted", __sigil_all([() => __sigil_ready(todos), () => ((todo) => __sigil_ready(todo).then((__value) => __value.done ).then((__value) => (!__value)))]).then(([__items, __predicate]) => __sigil_filter_list(__items, __predicate)));
}

export function completedCount(todos) {
  __sigil_record_coverage_call("src::todoDomain", "completedCount");
  return __sigil_record_coverage_result("src::todoDomain", "completedCount", __sigil_all([() => __sigil_ready(todos), () => ((acc, todo) => (async () => {
  const __match = await __sigil_ready(todo).then((__value) => __value.done );
  if (__match === true) {
    return __sigil_all([() => __sigil_ready(acc), () => __sigil_ready(1)]).then(([__left, __right]) => (__left + __right));
  }
  if (__match === false) {
    return __sigil_ready(acc);
  }
  throw new Error('Match failed: no pattern matched');
})()), () => __sigil_ready(0)]).then(([__items, __fn, __init]) => __items.reduce((__acc, x) => __acc.then((acc) => __fn(acc, x)), Promise.resolve(__init))));
}

export function deleteTodo(targetId, todos) {
  __sigil_record_coverage_call("src::todoDomain", "deleteTodo");
  return __sigil_record_coverage_result("src::todoDomain", "deleteTodo", __sigil_all([() => __sigil_ready(todos), () => ((todo) => __sigil_all([() => __sigil_ready(todo).then((__value) => __value.id ), () => __sigil_ready(targetId)]).then(([__left, __right]) => !__sigil_deep_equal(__left, __right)))]).then(([__items, __predicate]) => __sigil_filter_list(__items, __predicate)));
}

export function editTodo(newText, targetId, todos) {
  __sigil_record_coverage_call("src::todoDomain", "editTodo");
  return __sigil_record_coverage_result("src::todoDomain", "editTodo", __sigil_all([() => __sigil_ready(todos), () => ((todo) => (async () => {
  const __match = await __sigil_all([() => __sigil_ready(todo).then((__value) => __value.id ), () => __sigil_ready(targetId)]).then(([__left, __right]) => __sigil_deep_equal(__left, __right));
  if (__match === true) {
    return __sigil_all([() => __sigil_ready(todo).then((__value) => __value.done ), () => __sigil_ready(todo).then((__value) => __value.id ), () => __sigil_ready(newText)]).then((__values) => ({ "done": __values[0], "id": __values[1], "text": __values[2] }));
  }
  if (__match === false) {
    return __sigil_ready(todo);
  }
  throw new Error('Match failed: no pattern matched');
})())]).then(([__items, __fn]) => __sigil_map_list(__items, __fn)));
}

export function isVisible(done, filter) {
  __sigil_record_coverage_call("src::todoDomain", "isVisible");
  return __sigil_record_coverage_result("src::todoDomain", "isVisible", (async () => {
  const __match = await __sigil_ready(filter);
  if (__match === "all") {
    return __sigil_ready(true);
  }
  if (__match === "active") {
    return __sigil_ready(done).then((__value) => (!__value));
  }
  if (__match === "completed") {
    return __sigil_ready(done);
  }
  if (true) {
    return __sigil_ready(true);
  }
  throw new Error('Match failed: no pattern matched');
})());
}

export function remainingCount(completed, total) {
  __sigil_record_coverage_call("src::todoDomain", "remainingCount");
  return __sigil_record_coverage_result("src::todoDomain", "remainingCount", __sigil_all([() => __sigil_ready(total), () => __sigil_ready(completed)]).then(([__left, __right]) => (__left - __right)));
}

export function toggleTodo(targetId, todos) {
  __sigil_record_coverage_call("src::todoDomain", "toggleTodo");
  return __sigil_record_coverage_result("src::todoDomain", "toggleTodo", __sigil_all([() => __sigil_ready(todos), () => ((todo) => (async () => {
  const __match = await __sigil_all([() => __sigil_ready(todo).then((__value) => __value.id ), () => __sigil_ready(targetId)]).then(([__left, __right]) => __sigil_deep_equal(__left, __right));
  if (__match === true) {
    return __sigil_all([() => __sigil_ready(todo).then((__value) => __value.done ).then((__value) => (!__value)), () => __sigil_ready(todo).then((__value) => __value.id ), () => __sigil_ready(todo).then((__value) => __value.text )]).then((__values) => ({ "done": __values[0], "id": __values[1], "text": __values[2] }));
  }
  if (__match === false) {
    return __sigil_ready(todo);
  }
  throw new Error('Match failed: no pattern matched');
})())]).then(([__items, __fn]) => __sigil_map_list(__items, __fn)));
}

