//! Sigil to TypeScript Code Generator
//!
//! Compiles Sigil AST to runnable TypeScript (ES2022-compatible output).
//!
//! Key transformations:
//! - All functions return promise-shaped values
//! - Ordinary function calls compose without eager `await`
//! - Pattern matching compiles to if/else chains with `__match` variables
//! - Sum type constructors compile to objects with __tag and __fields
//! - Runtime helpers emitted at top of file

use sigil_ast::{
    BinaryOperator, ExternDecl, LiteralExpr, LiteralValue, Pattern, PatternLiteralValue,
    PipelineOperator, SourceLocation, Type, TypeDecl, TypeDef, UnaryOperator,
};
use sigil_typechecker::typed_ir::{
    MethodSelector, TypedBinaryExpr, TypedCallExpr, TypedConcurrentExpr, TypedConcurrentStep,
    TypedConstDecl, TypedConstructorCallExpr, TypedDeclaration, TypedExpr, TypedExprKind,
    TypedExternCallExpr, TypedFieldAccessExpr, TypedFilterExpr, TypedFoldExpr, TypedFunctionDecl,
    TypedIfExpr, TypedIndexExpr, TypedLambdaExpr, TypedLetExpr, TypedListExpr, TypedMapExpr,
    TypedMapLiteralExpr, TypedMatchExpr, TypedMethodCallExpr, TypedPipelineExpr, TypedProgram,
    TypedRecordExpr, TypedTestDecl, TypedTupleExpr, TypedUnaryExpr, TypedUsingExpr,
};
use sigil_typechecker::types::InferenceType;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

mod span_map;

pub use span_map::{
    collect_module_span_map, CollectedModuleSpanMap, DebugSpanKind, DebugSpanRecord,
    GeneratedLineRange, ModuleSpanMap, SPAN_MAP_FORMAT_VERSION,
};

const REPLAY_RUNTIME_HELPERS: &str = r#"function __sigil_world_clone(value) {
  if (typeof globalThis.structuredClone === 'function') {
    return globalThis.structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value));
}
const __sigil_replay_format_version = 2;
const __sigil_replay_invalid_artifact_code = 'SIGIL-RUNTIME-REPLAY-INVALID-ARTIFACT';
const __sigil_replay_diverged_code = 'SIGIL-RUNTIME-REPLAY-DIVERGED';
function __sigil_replay_error(code, message) {
  const error = new Error(`${String(code)}: ${String(message)}`);
  error.sigilCode = String(code);
  return error;
}
function __sigil_replay_throw(code, message) {
  throw __sigil_replay_error(code, message);
}
function __sigil_replay_error_summary(error) {
  const summary = {
    name: error instanceof Error && error.name ? String(error.name) : 'Error',
    message: error instanceof Error ? String(error.message ?? '') : String(error)
  };
  if (error && typeof error === 'object') {
    if ('sigilCode' in error && error.sigilCode != null) {
      summary.sigilCode = String(error.sigilCode);
    }
    if ('code' in error && error.code != null) {
      summary.code = String(error.code);
    }
  }
  return summary;
}
function __sigil_replay_error_from_summary(summary, family, operation) {
  const message =
    typeof summary?.message === 'string' && summary.message
      ? String(summary.message)
      : `missing replay error for ${String(family)}.${String(operation)}`;
  const error = new Error(message);
  error.name =
    typeof summary?.name === 'string' && summary.name
      ? String(summary.name)
      : 'Error';
  if (summary && typeof summary === 'object') {
    if (summary.sigilCode != null) {
      error.sigilCode = String(summary.sigilCode);
    }
    if (summary.code != null) {
      error.code = String(summary.code);
    }
  }
  return error;
}
function __sigil_replay_enabled() {
  return (
    !!globalThis.__sigil_replay_config &&
    (globalThis.__sigil_replay_config.mode === 'record' ||
      globalThis.__sigil_replay_config.mode === 'replay')
  );
}
function __sigil_replay_state_init() {
  const config = globalThis.__sigil_replay_config;
  if (!config || typeof config !== 'object' || !config.mode) {
    return null;
  }
  if (config.mode === 'record') {
    return {
      mode: 'record',
      effectCounts: Object.create(null),
      events: [],
      failure: null,
      nextHandleToken: 1,
      nextSeq: 1,
      startedAtEpochMs: Date.now(),
      world: null
    };
  }
  if (config.mode === 'replay') {
    const artifact = config.artifact;
    if (
      !artifact ||
      typeof artifact !== 'object' ||
      artifact.kind !== 'sigilRunReplay' ||
      artifact.formatVersion !== __sigil_replay_format_version ||
      !Array.isArray(artifact.events)
    ) {
      __sigil_replay_throw(__sigil_replay_invalid_artifact_code, 'invalid replay artifact');
    }
    return {
      mode: 'replay',
      events: artifact.events,
      nextIndex: 0,
      totalEvents: artifact.events.length
    };
  }
  return null;
}
function __sigil_replay_state() {
  if (!__sigil_replay_enabled()) {
    return null;
  }
  if (!globalThis.__sigil_replay_current || typeof globalThis.__sigil_replay_current !== 'object') {
    globalThis.__sigil_replay_current = __sigil_replay_state_init();
  }
  return globalThis.__sigil_replay_current;
}
function __sigil_replay_record_world(template) {
  const state = __sigil_replay_state();
  if (!state || state.mode !== 'record' || state.world !== null) {
    return;
  }
  state.world = __sigil_world_clone(template);
}
function __sigil_replay_increment_count(state, family) {
  state.effectCounts[family] = Number(state.effectCounts[family] ?? 0) + 1;
}
function __sigil_replay_record_event(family, operation, request, outcome) {
  const state = __sigil_replay_state();
  if (!state || state.mode !== 'record') {
    return;
  }
  const event = {
    family: String(family),
    operation: String(operation),
    request: __sigil_world_clone(request ?? {}),
    outcome: __sigil_world_clone(outcome ?? { kind: 'return', value: null }),
    seq: Number(state.nextSeq)
  };
  state.nextSeq += 1;
  state.events.push(event);
  __sigil_replay_increment_count(state, event.family);
}
function __sigil_replay_record_return(family, operation, request, value) {
  __sigil_replay_record_event(family, operation, request, {
    kind: 'return',
    value
  });
}
function __sigil_replay_record_throw(family, operation, request, error) {
  __sigil_replay_record_event(family, operation, request, {
    kind: 'throw',
    error: __sigil_replay_error_summary(error)
  });
}
function __sigil_replay_take_event(family, operation) {
  const state = __sigil_replay_state();
  if (!state || state.mode !== 'replay') {
    return { active: false, event: null };
  }
  const event = state.events[state.nextIndex];
  if (!event) {
    __sigil_replay_throw(
      __sigil_replay_diverged_code,
      `replay exhausted before ${String(family)}.${String(operation)}`
    );
  }
  if (event.family !== family || event.operation !== operation) {
    __sigil_replay_throw(
      __sigil_replay_diverged_code,
      `replay diverged at ${String(family)}.${String(operation)}`
    );
  }
  state.nextIndex += 1;
  return { active: true, event: __sigil_world_clone(event) };
}
function __sigil_replay_resolve_event_value(replay, family, operation, fallbackValue) {
  if (!replay.active) {
    return fallbackValue;
  }
  const outcome = replay.event?.outcome;
  if (outcome?.kind === 'throw') {
    throw __sigil_replay_error_from_summary(outcome.error, family, operation);
  }
  if (outcome && Object.prototype.hasOwnProperty.call(outcome, 'value')) {
    return outcome.value;
  }
  return fallbackValue;
}
function __sigil_replay_next_handle_token() {
  const state = __sigil_replay_state();
  if (!state || state.mode !== 'record') {
    return null;
  }
  const token = `h${state.nextHandleToken}`;
  state.nextHandleToken += 1;
  return token;
}
function __sigil_replay_require_handle_token(expectedToken, actualToken, operation) {
  const expected = expectedToken == null ? null : String(expectedToken);
  const actual = actualToken == null ? null : String(actualToken);
  if (expected !== actual) {
    __sigil_replay_throw(
      __sigil_replay_diverged_code,
      `replay handle mismatch during process.${String(operation)}`
    );
  }
}
function __sigil_replay_record_failure(code, message, stack) {
  const state = __sigil_replay_state();
  if (!state || state.mode !== 'record') {
    return;
  }
  state.failure = {
    code: String(code ?? 'SIGIL-RUNTIME-UNCAUGHT-EXCEPTION'),
    message: String(message ?? ''),
    stack: typeof stack === 'string' && stack ? stack : null
  };
}
function __sigil_replay_snapshot() {
  const config = globalThis.__sigil_replay_config ?? {};
  const state = __sigil_replay_state();
  if (!state) {
    return {
      mode: String(config.mode ?? ''),
      file: String(config.file ?? ''),
      recordedEvents: 0,
      consumedEvents: 0,
      remainingEvents: 0,
      partial: false
    };
  }
  if (state.mode === 'record') {
    return {
      mode: 'record',
      file: String(config.file ?? ''),
      recordedEvents: state.events.length,
      consumedEvents: 0,
      remainingEvents: 0,
      partial: !!state.failure
    };
  }
  return {
    mode: 'replay',
    file: String(config.file ?? ''),
    recordedEvents: Number(state.totalEvents ?? 0),
    consumedEvents: Number(state.nextIndex ?? 0),
    remainingEvents: Math.max(0, Number(state.totalEvents ?? 0) - Number(state.nextIndex ?? 0)),
    partial: false
  };
}
function __sigil_replay_artifact() {
  const config = globalThis.__sigil_replay_config ?? {};
  const state = __sigil_replay_state();
  if (!state || state.mode !== 'record') {
    return null;
  }
  return {
    formatVersion: __sigil_replay_format_version,
    kind: 'sigilRunReplay',
    entry: __sigil_world_clone(config.entry ?? { sourceFile: '', argv: [] }),
    binding: __sigil_world_clone(config.binding ?? { algorithm: 'sha256', fingerprint: '', modules: [] }),
    world: {
      normalizedWorld: state.world === null ? null : __sigil_world_clone(state.world),
      startedAtEpochMs: Number.isFinite(state.startedAtEpochMs) ? Number(state.startedAtEpochMs) : null
    },
    summary: {
      failed: !!state.failure,
      recordedEvents: state.events.length,
      effectCounts: __sigil_world_clone(state.effectCounts)
    },
    events: __sigil_world_clone(state.events),
    failure: state.failure ? __sigil_world_clone(state.failure) : undefined
  };
}
globalThis.__sigil_replay_snapshot = __sigil_replay_snapshot;
globalThis.__sigil_replay_artifact = __sigil_replay_artifact;
globalThis.__sigil_replay_record_failure = __sigil_replay_record_failure;
"#;

const WORLD_RUNTIME_HELPERS: &str = r#"function __sigil_world_error(message) {
  throw new Error(String(message));
}
function __sigil_world_host_template() {
  return {
    clock: { kind: 'system' },
    fs: { kind: 'real' },
    fsWatch: { kind: 'real', rules: [] },
    fsWatchRoots: Object.create(null),
    fsRoots: Object.create(null),
    http: Object.create(null),
    log: { kind: 'stdout' },
    logSinks: Object.create(null),
    pty: { kind: 'real' },
    ptyHandles: Object.create(null),
    process: { kind: 'real' },
    processHandles: Object.create(null),
    random: { kind: 'real' },
    sql: { kind: 'deny' },
    sqlHandles: Object.create(null),
    stream: { kind: 'live' },
    task: { kind: 'real' },
    tcp: Object.create(null),
    timer: { kind: 'real' },
    websocket: { kind: 'real' },
    websocketHandles: Object.create(null)
  };
}
function __sigil_world_collect_topology(topologyExports) {
  const envs = new Set();
  const fsRoots = new Set();
  const http = new Set();
  const logSinks = new Set();
  const ptyHandles = new Set();
  const processHandles = new Set();
  const sqlHandles = new Set();
  const tcp = new Set();
  const websocketHandles = new Set();
  if (!topologyExports || typeof topologyExports !== 'object') {
    return { envs, fsRoots, http, logSinks, ptyHandles, processHandles, sqlHandles, tcp, websocketHandles };
  }
  for (const value of Object.values(topologyExports)) {
    if (value?.__tag === 'Environment') {
      envs.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'FsRoot') {
      fsRoots.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'HttpServiceDependency') {
      http.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'LogSink') {
      logSinks.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'PtyHandle') {
      ptyHandles.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'ProcessHandle') {
      processHandles.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'SqlHandle') {
      sqlHandles.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'TcpServiceDependency') {
      tcp.add(String(value.__fields?.[0] ?? ''));
    } else if (value?.__tag === 'WebSocketHandle') {
      websocketHandles.add(String(value.__fields?.[0] ?? ''));
    }
  }
  return { envs, fsRoots, http, logSinks, ptyHandles, processHandles, sqlHandles, tcp, websocketHandles };
}
function __sigil_world_parse_clock(value) {
  if (value?.__tag === 'SystemClock') {
    return { kind: 'system' };
  }
  if (value?.__tag === 'FixedClock') {
    const iso = String(value.__fields?.[0] ?? '');
    const millis = Date.parse(iso);
    if (!iso || Number.isNaN(millis)) {
      __sigil_world_error(`invalid fixed clock ISO timestamp '${iso}'`);
    }
    return { kind: 'fixed', iso, millis };
  }
  __sigil_world_error('world.clock must be world::clock.ClockEntry');
}
function __sigil_world_parse_fs(value) {
  if (value?.__tag === 'DenyFs') return { kind: 'deny' };
  if (value?.__tag === 'RealFs') return { kind: 'real' };
  if (value?.__tag === 'SandboxFs') {
    const root = String(value.__fields?.[0] ?? '');
    if (!root) {
      __sigil_world_error('sandbox fs root must be a non-empty string');
    }
    return { kind: 'sandbox', root };
  }
  __sigil_world_error('world.fs must be world::fs.FsEntry');
}
function __sigil_world_parse_fswatch(value) {
  if (value?.__tag === 'DenyFsWatch') return { kind: 'deny', rules: [] };
  if (value?.__tag === 'FixtureFsWatch') {
    return {
      kind: 'fixture',
      rules: (value.__fields?.[0] ?? []).map((rule) => ({
        events: Array.isArray(rule?.events) ? rule.events.slice() : [],
        path: String(rule?.path ?? '')
      }))
    };
  }
  if (value?.__tag === 'RealFsWatch') return { kind: 'real', rules: [] };
  __sigil_world_error('world.fsWatch must be world::fsWatch.FsWatchEntry');
}
function __sigil_world_parse_log(value) {
  if (value?.__tag === 'CaptureLog') return { kind: 'capture' };
  if (value?.__tag === 'StdoutLog') return { kind: 'stdout' };
  __sigil_world_error('world.log must be world::log.LogEntry');
}
function __sigil_world_parse_pty(value) {
  if (value?.__tag === 'DenyPty') return { kind: 'deny' };
  if (value?.__tag === 'FixturePty') {
    return {
      kind: 'fixture',
      rules: (value.__fields?.[0] ?? []).map((rule) => ({
        events: Array.isArray(rule?.events) ? rule.events.slice() : [],
        request: rule?.request ?? null,
        writes: Array.isArray(rule?.writes) ? rule.writes.map((item) => String(item)) : []
      }))
    };
  }
  if (value?.__tag === 'RealPty') return { kind: 'real' };
  __sigil_world_error('world.pty must be world::pty.PtyEntry');
}
function __sigil_world_parse_websocket(value) {
  if (value?.__tag === 'DenyWebSocket') return { kind: 'deny', rules: [] };
  if (value?.__tag === 'FixtureWebSocket') {
    return {
      kind: 'fixture',
      rules: (value.__fields?.[0] ?? []).map((rule) => ({
        messages: Array.isArray(rule?.messages) ? rule.messages.map((item) => String(item)) : []
      }))
    };
  }
  if (value?.__tag === 'RealWebSocket') return { kind: 'real', rules: [] };
  __sigil_world_error('world.websocket must be world::websocket.WebSocketEntry');
}
function __sigil_world_parse_process(value) {
  if (value?.__tag === 'DenyProcess') return { kind: 'deny' };
  if (value?.__tag === 'FixtureProcess') {
    return {
      kind: 'fixture',
      rules: (value.__fields?.[0] ?? []).map((rule) => ({
        argv: Array.isArray(rule?.argv) ? rule.argv.map((item) => String(item)) : [],
        cwd: rule?.cwd?.__tag === 'Some' ? String(rule.cwd.__fields?.[0] ?? '') : null,
        result: {
          code: Number(rule?.result?.code ?? -1),
          stderr: String(rule?.result?.stderr ?? ''),
          stdout: String(rule?.result?.stdout ?? '')
        }
      }))
    };
  }
  if (value?.__tag === 'RealProcess') return { kind: 'real' };
  __sigil_world_error('world.process must be world::process.ProcessEntry');
}
function __sigil_world_parse_sql(value) {
  if (value?.__tag === 'DenySql') return { kind: 'deny' };
  if (value?.__tag === 'FixtureSql') {
    const fixture = value.__fields?.[0] ?? {};
    return {
      kind: 'fixture',
      seed: Array.isArray(fixture?.seed)
        ? fixture.seed.map((statement) => ({
            params: statement?.params ?? {},
            sql: String(statement?.sql ?? '')
          }))
        : []
    };
  }
  if (value?.__tag === 'SqliteSql') {
    const path = String(value.__fields?.[0] ?? '');
    if (!path) {
      __sigil_world_error('sqlite sql path must be a non-empty string');
    }
    return { kind: 'sqlite', path };
  }
  if (value?.__tag === 'PostgresSql') {
    const connection = String(value.__fields?.[0] ?? '');
    if (!connection) {
      __sigil_world_error('postgres sql connection must be a non-empty string');
    }
    return { kind: 'postgres', connection };
  }
  __sigil_world_error('world.sql must be world::sql.SqlEntry');
}
function __sigil_world_parse_fs_root_entry(value) {
  if (value?.__tag !== 'FsRootEntry') {
    __sigil_world_error('FS root overrides must be world::fs.FsRootEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const rootName = String(payload?.rootName ?? '');
  if (!rootName) {
    __sigil_world_error('world::fs.FsRootEntry rootName must be a non-empty string');
  }
  return {
    rootName,
    ...__sigil_world_parse_fs(payload?.mode ?? null)
  };
}
function __sigil_world_parse_fswatch_root_entry(value) {
  if (value?.__tag !== 'FsWatchRootEntry') {
    __sigil_world_error('FS watch root overrides must be world::fsWatch.FsWatchRootEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const rootName = String(payload?.rootName ?? '');
  if (!rootName) {
    __sigil_world_error('world::fsWatch.FsWatchRootEntry rootName must be a non-empty string');
  }
  return {
    rootName,
    ...__sigil_world_parse_fswatch(payload?.mode ?? null)
  };
}
function __sigil_world_parse_log_sink_entry(value) {
  if (value?.__tag !== 'LogSinkEntry') {
    __sigil_world_error('Log sink overrides must be world::log.LogSinkEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const sinkName = String(payload?.sinkName ?? '');
  if (!sinkName) {
    __sigil_world_error('world::log.LogSinkEntry sinkName must be a non-empty string');
  }
  return {
    sinkName,
    ...__sigil_world_parse_log(payload?.mode ?? null)
  };
}
function __sigil_world_parse_pty_handle_entry(value) {
  if (value?.__tag !== 'PtyHandleEntry') {
    __sigil_world_error('Pty handle overrides must be world::pty.PtyHandleEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const handleName = String(payload?.handleName ?? '');
  if (!handleName) {
    __sigil_world_error('world::pty.PtyHandleEntry handleName must be a non-empty string');
  }
  return {
    handleName,
    ...__sigil_world_parse_pty(payload?.mode ?? null)
  };
}
function __sigil_world_parse_websocket_handle_entry(value) {
  if (value?.__tag !== 'WebSocketHandleEntry') {
    __sigil_world_error('WebSocket handle overrides must be world::websocket.WebSocketHandleEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const handleName = String(payload?.handleName ?? '');
  if (!handleName) {
    __sigil_world_error('world::websocket.WebSocketHandleEntry handleName must be a non-empty string');
  }
  return {
    handleName,
    ...__sigil_world_parse_websocket(payload?.mode ?? null)
  };
}
function __sigil_world_parse_process_handle_entry(value) {
  if (value?.__tag !== 'ProcessHandleEntry') {
    __sigil_world_error('Process handle overrides must be world::process.ProcessHandleEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const handleName = String(payload?.handleName ?? '');
  if (!handleName) {
    __sigil_world_error('world::process.ProcessHandleEntry handleName must be a non-empty string');
  }
  return {
    handleName,
    ...__sigil_world_parse_process(payload?.mode ?? null)
  };
}
function __sigil_world_parse_sql_handle_entry(value) {
  if (value?.__tag !== 'SqlHandleEntry') {
    __sigil_world_error('SQL handle overrides must be world::sql.SqlHandleEntry');
  }
  const payload = value.__fields?.[0] ?? {};
  const handleName = String(payload?.handleName ?? '');
  if (!handleName) {
    __sigil_world_error('world::sql.SqlHandleEntry handleName must be a non-empty string');
  }
  return {
    handleName,
    ...__sigil_world_parse_sql(payload?.mode ?? null)
  };
}
function __sigil_world_random_normalize_seed(seed) {
  const modulus = 2147483647;
  let value = Math.trunc(Number(seed));
  if (!Number.isFinite(value)) {
    return 1;
  }
  value %= modulus;
  if (value <= 0) {
    value += modulus - 1;
  }
  return value;
}
function __sigil_world_parse_random(value) {
  if (value?.__tag === 'FixtureRandom') {
    return {
      kind: 'fixture',
      draws: Array.isArray(value.__fields?.[0])
        ? value.__fields[0].map((item) => {
            const normalized = Math.trunc(Number(item));
            return Number.isFinite(normalized) ? normalized : 0;
          })
        : [],
      index: 0
    };
  }
  if (value?.__tag === 'RealRandom') {
    return { kind: 'real' };
  }
  if (value?.__tag === 'SeededRandom') {
    return {
      kind: 'seeded',
      state: __sigil_world_random_normalize_seed(value.__fields?.[0] ?? 1)
    };
  }
  __sigil_world_error('world.random must be world::random.RandomEntry');
}
function __sigil_world_parse_stream(value) {
  if (value?.__tag === 'LiveStream') {
    return { kind: 'live' };
  }
  __sigil_world_error('world.stream must be world::stream.StreamEntry');
}
function __sigil_world_parse_task(value) {
  if (value?.__tag === 'RealTask') {
    return { kind: 'real' };
  }
  __sigil_world_error('world.task must be world::task.TaskEntry');
}
function __sigil_world_parse_timer(value) {
  if (value?.__tag === 'RealTimer') return { kind: 'real', nowMs: null };
  if (value?.__tag === 'VirtualTimer') return { kind: 'virtual', nowMs: null };
  __sigil_world_error('world.timer must be world::timer.TimerEntry');
}
function __sigil_world_parse_http_rule(rule) {
  const bodyMatch = rule?.bodyMatch;
  let parsedBodyMatch;
  if (bodyMatch?.__tag === 'AnyRequest') {
    parsedBodyMatch = { kind: 'any' };
  } else if (bodyMatch?.__tag === 'BodyContains') {
    parsedBodyMatch = { kind: 'contains', fragment: String(bodyMatch.__fields?.[0] ?? '') };
  } else {
    __sigil_world_error('invalid world::http.HttpRule bodyMatch');
  }
  const response = rule?.response;
  let parsedResponse;
  if (response?.__tag === 'Timeout') {
    parsedResponse = { kind: 'timeout' };
  } else if (response?.__tag === 'Respond') {
    const value = response.__fields?.[0] ?? {};
    parsedResponse = { kind: 'respond', body: String(value.body ?? ''), status: Number(value.status ?? 0) };
  } else {
    __sigil_world_error('invalid world::http.HttpRule response');
  }
  return {
    bodyMatch: parsedBodyMatch,
    method: String(rule?.method ?? 'GET').toUpperCase(),
    path: String(rule?.path ?? '/'),
    response: parsedResponse
  };
}
function __sigil_world_parse_http_entry(value) {
  if (value?.__tag !== 'HttpEntry') {
    __sigil_world_error('HTTP world overrides must be world::http.HttpEntry');
  }
  const entry = value.__fields?.[0] ?? {};
  const dependencyName = String(entry.dependencyName ?? '');
  if (!dependencyName) {
    __sigil_world_error('HTTP world entries must name a dependency');
  }
  const mode = entry.mode;
  if (mode?.__tag === 'Deny') {
    return { dependencyName, kind: 'deny' };
  }
  if (mode?.__tag === 'Proxy') {
    const baseUrl = String(mode.__fields?.[0] ?? '');
    if (!baseUrl) {
      __sigil_world_error(`HTTP dependency '${dependencyName}' requires a non-empty base URL`);
    }
    return { dependencyName, kind: 'proxy', baseUrl };
  }
  if (mode?.__tag === 'Fixture') {
    return {
      dependencyName,
      kind: 'fixture',
      rules: (mode.__fields?.[0] ?? []).map(__sigil_world_parse_http_rule)
    };
  }
  __sigil_world_error(`invalid HTTP world mode for dependency '${dependencyName}'`);
}
function __sigil_world_parse_tcp_rule(rule) {
  const response = rule?.response;
  let parsedResponse;
  if (response?.__tag === 'Timeout') {
    parsedResponse = { kind: 'timeout' };
  } else if (response?.__tag === 'Respond') {
    parsedResponse = { kind: 'respond', body: String(response.__fields?.[0] ?? '') };
  } else {
    __sigil_world_error('invalid world::tcp.TcpRule response');
  }
  return {
    request: String(rule?.request ?? ''),
    response: parsedResponse
  };
}
function __sigil_world_parse_tcp_entry(value) {
  if (value?.__tag !== 'TcpEntry') {
    __sigil_world_error('TCP world overrides must be world::tcp.TcpEntry');
  }
  const entry = value.__fields?.[0] ?? {};
  const dependencyName = String(entry.dependencyName ?? '');
  if (!dependencyName) {
    __sigil_world_error('TCP world entries must name a dependency');
  }
  const mode = entry.mode;
  if (mode?.__tag === 'Deny') {
    return { dependencyName, kind: 'deny' };
  }
  if (mode?.__tag === 'Proxy') {
    const target = mode.__fields?.[0] ?? {};
    const host = String(target.host ?? '');
    const port = Number(target.port ?? 0);
    if (!host || !Number.isInteger(port) || port <= 0 || port > 65535) {
      __sigil_world_error(`TCP dependency '${dependencyName}' requires a valid host and port`);
    }
    return { dependencyName, kind: 'proxy', host, port };
  }
  if (mode?.__tag === 'Fixture') {
    return {
      dependencyName,
      kind: 'fixture',
      rules: (mode.__fields?.[0] ?? []).map(__sigil_world_parse_tcp_rule)
    };
  }
  __sigil_world_error(`invalid TCP world mode for dependency '${dependencyName}'`);
}
function __sigil_world_prepare_template(worldValue, topologyExports, envName) {
  if (!worldValue || typeof worldValue !== 'object') {
    __sigil_world_error("config module must export a 'world' value");
  }
  const topology = __sigil_world_collect_topology(topologyExports);
  if (envName && topology.envs.size > 0 && !topology.envs.has(envName)) {
    __sigil_world_error(`environment '${envName}' not declared in src/topology.lib.sigil`);
  }
  const template = __sigil_world_host_template();
  template.clock = __sigil_world_parse_clock(worldValue.clock);
  template.fs = __sigil_world_parse_fs(worldValue.fs);
  template.fsWatch = __sigil_world_parse_fswatch(worldValue.fsWatch);
  template.fsWatchRoots = Object.create(null);
  for (const value of worldValue.fsWatchRoots ?? []) {
    const entry = __sigil_world_parse_fswatch_root_entry(value);
    template.fsWatchRoots[entry.rootName] = entry;
  }
  template.fsRoots = Object.create(null);
  for (const value of worldValue.fsRoots ?? []) {
    const entry = __sigil_world_parse_fs_root_entry(value);
    template.fsRoots[entry.rootName] = entry;
  }
  template.log = __sigil_world_parse_log(worldValue.log);
  template.logSinks = Object.create(null);
  for (const value of worldValue.logSinks ?? []) {
    const entry = __sigil_world_parse_log_sink_entry(value);
    template.logSinks[entry.sinkName] = entry;
  }
  template.pty = __sigil_world_parse_pty(worldValue.pty);
  template.ptyHandles = Object.create(null);
  for (const value of worldValue.ptyHandles ?? []) {
    const entry = __sigil_world_parse_pty_handle_entry(value);
    template.ptyHandles[entry.handleName] = entry;
  }
  template.process = __sigil_world_parse_process(worldValue.process);
  template.processHandles = Object.create(null);
  for (const value of worldValue.processHandles ?? []) {
    const entry = __sigil_world_parse_process_handle_entry(value);
    template.processHandles[entry.handleName] = entry;
  }
  template.random = __sigil_world_parse_random(worldValue.random);
  template.sql = __sigil_world_parse_sql(worldValue.sql);
  template.sqlHandles = Object.create(null);
  for (const value of worldValue.sqlHandles ?? []) {
    const entry = __sigil_world_parse_sql_handle_entry(value);
    template.sqlHandles[entry.handleName] = entry;
  }
  template.stream = __sigil_world_parse_stream(worldValue.stream);
  template.task = __sigil_world_parse_task(worldValue.task);
  template.timer = __sigil_world_parse_timer(worldValue.timer);
  template.websocket = __sigil_world_parse_websocket(worldValue.websocket);
  template.websocketHandles = Object.create(null);
  for (const value of worldValue.websocketHandles ?? []) {
    const entry = __sigil_world_parse_websocket_handle_entry(value);
    template.websocketHandles[entry.handleName] = entry;
  }
  template.http = Object.create(null);
  for (const value of worldValue.http ?? []) {
    const entry = __sigil_world_parse_http_entry(value);
    template.http[entry.dependencyName] = entry;
  }
  template.tcp = Object.create(null);
  for (const value of worldValue.tcp ?? []) {
    const entry = __sigil_world_parse_tcp_entry(value);
    template.tcp[entry.dependencyName] = entry;
  }
  if (topology.http.size > 0) {
    for (const name of topology.http) {
      if (!(name in template.http)) {
        __sigil_world_error(`missing HTTP world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.http)) {
      if (!topology.http.has(name)) {
        __sigil_world_error(`HTTP world entry references undeclared dependency '${name}'`);
      }
    }
  }
  if (topology.fsRoots.size > 0) {
    for (const name of topology.fsRoots) {
      if (!(name in template.fsWatchRoots)) {
        __sigil_world_error(`missing fsWatch root world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.fsWatchRoots)) {
      if (!topology.fsRoots.has(name)) {
        __sigil_world_error(`fsWatch root world entry references undeclared boundary '${name}'`);
      }
    }
    for (const name of topology.fsRoots) {
      if (!(name in template.fsRoots)) {
        __sigil_world_error(`missing FS root world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.fsRoots)) {
      if (!topology.fsRoots.has(name)) {
        __sigil_world_error(`FS root world entry references undeclared boundary '${name}'`);
      }
    }
  }
  if (topology.logSinks.size > 0) {
    for (const name of topology.logSinks) {
      if (!(name in template.logSinks)) {
        __sigil_world_error(`missing log sink world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.logSinks)) {
      if (!topology.logSinks.has(name)) {
        __sigil_world_error(`log sink world entry references undeclared boundary '${name}'`);
      }
    }
  }
  if (topology.ptyHandles.size > 0) {
    for (const name of topology.ptyHandles) {
      if (!(name in template.ptyHandles)) {
        __sigil_world_error(`missing pty handle world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.ptyHandles)) {
      if (!topology.ptyHandles.has(name)) {
        __sigil_world_error(`pty handle world entry references undeclared boundary '${name}'`);
      }
    }
  }
  if (topology.processHandles.size > 0) {
    for (const name of topology.processHandles) {
      if (!(name in template.processHandles)) {
        __sigil_world_error(`missing process handle world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.processHandles)) {
      if (!topology.processHandles.has(name)) {
        __sigil_world_error(`process handle world entry references undeclared boundary '${name}'`);
      }
    }
  }
  if (topology.sqlHandles.size > 0) {
    for (const name of topology.sqlHandles) {
      if (!(name in template.sqlHandles)) {
        __sigil_world_error(`missing sql handle world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.sqlHandles)) {
      if (!topology.sqlHandles.has(name)) {
        __sigil_world_error(`sql handle world entry references undeclared boundary '${name}'`);
      }
    }
  }
  if (topology.tcp.size > 0) {
    for (const name of topology.tcp) {
      if (!(name in template.tcp)) {
        __sigil_world_error(`missing TCP world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.tcp)) {
      if (!topology.tcp.has(name)) {
        __sigil_world_error(`TCP world entry references undeclared dependency '${name}'`);
      }
    }
  }
  if (topology.websocketHandles.size > 0) {
    for (const name of topology.websocketHandles) {
      if (!(name in template.websocketHandles)) {
        __sigil_world_error(`missing websocket handle world entry for '${name}' in environment '${envName ?? '<unknown>'}'`);
      }
    }
    for (const name of Object.keys(template.websocketHandles)) {
      if (!topology.websocketHandles.has(name)) {
        __sigil_world_error(`websocket handle world entry references undeclared boundary '${name}'`);
      }
    }
  }
  return template;
}
function __sigil_world_base_template() {
  if (globalThis.__sigil_world_template_cache) {
    return globalThis.__sigil_world_template_cache;
  }
  let template;
  if (globalThis.__sigil_replay_config?.mode === 'replay') {
    const replayWorld = globalThis.__sigil_replay_config?.artifact?.world?.normalizedWorld;
    if (replayWorld == null) {
      __sigil_replay_throw(__sigil_replay_invalid_artifact_code, 'replay artifact is missing normalizedWorld');
    }
    template = __sigil_world_clone(replayWorld);
  } else {
    template = globalThis.__sigil_world_value
      ? __sigil_world_prepare_template(
          globalThis.__sigil_world_value,
          globalThis.__sigil_topology_exports ?? null,
          globalThis.__sigil_world_env_name ?? null
        )
      : __sigil_world_host_template();
  }
  __sigil_replay_record_world(template);
  globalThis.__sigil_world_template_cache = template;
  return template;
}
function __sigil_world_fresh(template) {
  const world = __sigil_world_clone(template);
  world.fsWatchNextId = 1;
  world.fsWatches = new Map();
  world.ptyNextId = 1;
  world.ptyManagedNextId = 1;
  world.ptyManagedRefs = new Map();
  world.ptySessions = new Map();
  world.httpServers = new Map();
  world.sqlBackends = new Map();
  world.sqlNextTransactionId = 1;
  world.sqlTransactions = new Map();
  world.websocketNextClientId = 1;
  world.websocketNextServerId = 1;
  world.websocketClients = new Map();
  world.websocketServers = new Map();
  world.traces = {
    http: Object.create(null),
    fsWatch: [],
    log: [],
    pty: [],
    process: [],
    tcp: Object.create(null),
    timer: { sleeps: [] },
    websocket: []
  };
  if (world.stream.kind === 'live') {
    world.stream.nextHubId = 1;
    world.stream.nextId = 1;
    world.stream.hubs = Object.create(null);
    world.stream.sources = Object.create(null);
  }
  world.taskNextId = 1;
  world.tasks = new Map();
  if (world.timer.kind === 'virtual') {
    world.timer.nowMs = world.clock.kind === 'fixed' ? world.clock.millis : Date.now();
  }
  return world;
}
function __sigil_world_apply_overrides(world, overrides) {
  for (const value of overrides ?? []) {
    if (!value || typeof value !== 'object') {
      continue;
    }
    switch (value.__tag) {
      case 'SystemClock':
      case 'FixedClock':
        world.clock = __sigil_world_parse_clock(value);
        if (world.timer.kind === 'virtual') {
          world.timer.nowMs = world.clock.kind === 'fixed' ? world.clock.millis : Date.now();
        }
        break;
      case 'DenyFs':
      case 'RealFs':
      case 'SandboxFs':
        world.fs = __sigil_world_parse_fs(value);
        break;
      case 'DenyFsWatch':
      case 'FixtureFsWatch':
      case 'RealFsWatch':
        world.fsWatch = __sigil_world_parse_fswatch(value);
        break;
      case 'FsWatchRootEntry': {
        const entry = __sigil_world_parse_fswatch_root_entry(value);
        world.fsWatchRoots[entry.rootName] = entry;
        break;
      }
      case 'FsRootEntry': {
        const entry = __sigil_world_parse_fs_root_entry(value);
        world.fsRoots[entry.rootName] = entry;
        break;
      }
      case 'CaptureLog':
      case 'StdoutLog':
        world.log = __sigil_world_parse_log(value);
        break;
      case 'LogSinkEntry': {
        const entry = __sigil_world_parse_log_sink_entry(value);
        world.logSinks[entry.sinkName] = entry;
        break;
      }
      case 'DenyPty':
      case 'FixturePty':
      case 'RealPty':
        world.pty = __sigil_world_parse_pty(value);
        break;
      case 'PtyHandleEntry': {
        const entry = __sigil_world_parse_pty_handle_entry(value);
        world.ptyHandles[entry.handleName] = entry;
        break;
      }
      case 'DenyProcess':
      case 'FixtureProcess':
      case 'RealProcess':
        world.process = __sigil_world_parse_process(value);
        break;
      case 'ProcessHandleEntry': {
        const entry = __sigil_world_parse_process_handle_entry(value);
        world.processHandles[entry.handleName] = entry;
        break;
      }
      case 'FixtureRandom':
      case 'RealRandom':
      case 'SeededRandom':
        world.random = __sigil_world_parse_random(value);
        break;
      case 'DenySql':
      case 'FixtureSql':
      case 'PostgresSql':
      case 'SqliteSql':
        world.sql = __sigil_world_parse_sql(value);
        break;
      case 'SqlHandleEntry': {
        const entry = __sigil_world_parse_sql_handle_entry(value);
        world.sqlHandles[entry.handleName] = entry;
        break;
      }
      case 'LiveStream':
        world.stream = __sigil_world_parse_stream(value);
        break;
      case 'RealTask':
        world.task = __sigil_world_parse_task(value);
        break;
      case 'RealTimer':
      case 'VirtualTimer':
        world.timer = __sigil_world_parse_timer(value);
        if (world.timer.kind === 'virtual') {
          world.timer.nowMs = world.clock.kind === 'fixed' ? world.clock.millis : Date.now();
        }
        break;
      case 'DenyWebSocket':
      case 'FixtureWebSocket':
      case 'RealWebSocket':
        world.websocket = __sigil_world_parse_websocket(value);
        break;
      case 'WebSocketHandleEntry': {
        const entry = __sigil_world_parse_websocket_handle_entry(value);
        world.websocketHandles[entry.handleName] = entry;
        break;
      }
      case 'HttpEntry': {
        const entry = __sigil_world_parse_http_entry(value);
        world.http[entry.dependencyName] = entry;
        break;
      }
      case 'TcpEntry': {
        const entry = __sigil_world_parse_tcp_entry(value);
        world.tcp[entry.dependencyName] = entry;
        break;
      }
      default:
        __sigil_world_error(`invalid test world override '${String(value.__tag)}'`);
    }
  }
  return world;
}
function __sigil_current_world() {
  if (!globalThis.__sigil_world_current) {
    globalThis.__sigil_world_current = __sigil_world_fresh(__sigil_world_base_template());
  }
  return globalThis.__sigil_world_current;
}
async function __sigil_with_world(world, body) {
  const previous = globalThis.__sigil_world_current;
  globalThis.__sigil_world_current = world;
  try {
    return await body();
  } finally {
    globalThis.__sigil_world_current = previous;
  }
}
async function __sigil_run_test_world(overrides, body) {
  const world = __sigil_world_fresh(__sigil_world_base_template());
  __sigil_world_apply_overrides(world, overrides);
  globalThis.__sigil_last_test_world = __sigil_world_clone(world);
  return await __sigil_with_world(world, body);
}
function __sigil_world_stream_done() {
  return { __tag: 'Done', __fields: [] };
}
function __sigil_world_stream_item(value) {
  return { __tag: 'Item', __fields: [value] };
}
function __sigil_world_stream_resolve_state(source) {
  const world = __sigil_current_world();
  if (world.stream.kind !== 'live') {
    __sigil_world_error('world.stream must be live to use §stream');
  }
  const sourceId = Number(source?.__fields?.[0] ?? Number.NaN);
  if (source?.__tag !== 'StreamSource' || !Number.isInteger(sourceId) || sourceId <= 0) {
    __sigil_world_error('stream source must be a valid §stream.Source');
  }
  const state = world.stream.sources?.[String(sourceId)] ?? null;
  if (!state) {
    __sigil_world_error(`unknown stream source '${sourceId}' in current world`);
  }
  return state;
}
function __sigil_world_stream_resolve_hub(hub) {
  const world = __sigil_current_world();
  if (world.stream.kind !== 'live') {
    __sigil_world_error('world.stream must be live to use §stream hubs');
  }
  const hubId = Number(hub?.__fields?.[0] ?? Number.NaN);
  if (hub?.__tag !== 'StreamHub' || !Number.isInteger(hubId) || hubId <= 0) {
    __sigil_world_error('stream hub must be a valid §stream.Hub');
  }
  const state = world.stream.hubs?.[String(hubId)] ?? null;
  if (!state) {
    __sigil_world_error(`unknown stream hub '${hubId}' in current world`);
  }
  return state;
}
function __sigil_world_stream_open() {
  const world = __sigil_current_world();
  if (world.stream.kind !== 'live') {
    __sigil_world_error('world.stream must be live to create sources');
  }
  const sourceId = Number(world.stream.nextId ?? 1);
  world.stream.nextId = sourceId + 1;
  world.stream.sources[String(sourceId)] = {
    closed: false,
    queue: [],
    waiters: []
  };
  return { __tag: 'StreamSource', __fields: [sourceId] };
}
function __sigil_world_stream_open_hub() {
  const world = __sigil_current_world();
  if (world.stream.kind !== 'live') {
    __sigil_world_error('world.stream must be live to create hubs');
  }
  const hubId = Number(world.stream.nextHubId ?? 1);
  world.stream.nextHubId = hubId + 1;
  world.stream.hubs[String(hubId)] = {
    closed: false,
    subscribers: new Set()
  };
  const hub = { __tag: 'StreamHub', __fields: [hubId] };
  return __sigil_owned_wrap(hub, async () => {
    await __sigil_world_stream_close_hub(hub);
    return null;
  });
}
function __sigil_world_stream_push(source, value) {
  const state = __sigil_world_stream_resolve_state(source);
  if (state.closed) {
    return;
  }
  if (state.waiters.length > 0) {
    const resolve = state.waiters.shift();
    resolve(__sigil_world_stream_item(value));
    return;
  }
  state.queue.push(value);
}
function __sigil_world_stream_finish(source) {
  const state = __sigil_world_stream_resolve_state(source);
  if (state.closed) {
    return;
  }
  state.closed = true;
  if (state.queue.length === 0) {
    while (state.waiters.length > 0) {
      const resolve = state.waiters.shift();
      resolve(__sigil_world_stream_done());
    }
  }
}
function __sigil_world_stream_test_source(items) {
  const source = __sigil_world_stream_open();
  for (const item of Array.isArray(items) ? items : []) {
    __sigil_world_stream_push(source, item);
  }
  __sigil_world_stream_finish(source);
  return source;
}
async function __sigil_world_stream_next(source) {
  const state = __sigil_world_stream_resolve_state(source);
  if (state.queue.length > 0) {
    const nextValue = state.queue.shift();
    return __sigil_world_stream_item(nextValue);
  }
  if (state.closed) {
    return __sigil_world_stream_done();
  }
  return await new Promise((resolve) => {
    state.waiters.push(resolve);
  });
}
async function __sigil_world_stream_close(source) {
  const state = __sigil_world_stream_resolve_state(source);
  state.queue = [];
  if (!state.closed) {
    state.closed = true;
    while (state.waiters.length > 0) {
      const resolve = state.waiters.shift();
      resolve(__sigil_world_stream_done());
    }
  }
  return null;
}
async function __sigil_world_stream_publish(hub, value) {
  const state = __sigil_world_stream_resolve_hub(hub);
  if (state.closed) {
    return null;
  }
  for (const source of Array.from(state.subscribers ?? [])) {
    __sigil_world_stream_push(source, value);
  }
  return null;
}
async function __sigil_world_stream_subscribe(hub) {
  const state = __sigil_world_stream_resolve_hub(hub);
  if (state.closed) {
    const source = __sigil_world_stream_open();
    __sigil_world_stream_finish(source);
    return __sigil_owned_wrap(source, async () => {
      await __sigil_world_stream_close(source);
      return null;
    });
  }
  const source = __sigil_world_stream_open();
  state.subscribers.add(source);
  return __sigil_owned_wrap(source, async () => {
    state.subscribers.delete(source);
    await __sigil_world_stream_close(source);
    return null;
  });
}
async function __sigil_world_stream_close_hub(hub) {
  const world = __sigil_current_world();
  const hubId = Number(hub?.__fields?.[0] ?? Number.NaN);
  const state = __sigil_world_stream_resolve_hub(hub);
  if (state.closed) {
    return null;
  }
  state.closed = true;
  for (const source of Array.from(state.subscribers ?? [])) {
    await __sigil_world_stream_close(source);
  }
  state.subscribers.clear();
  delete world.stream.hubs[String(hubId)];
  return null;
}
async function __sigil_world_stream_finish_hub(hub) {
  const world = __sigil_current_world();
  const hubId = Number(hub?.__fields?.[0] ?? Number.NaN);
  const state = __sigil_world_stream_resolve_hub(hub);
  if (state.closed) {
    return null;
  }
  state.closed = true;
  for (const source of Array.from(state.subscribers ?? [])) {
    __sigil_world_stream_finish(source);
  }
  state.subscribers.clear();
  delete world.stream.hubs[String(hubId)];
  return null;
}
function __sigil_owned_wrap(value, dispose) {
  return {
    __sigil_dispose: typeof dispose === 'function' ? dispose : async () => null,
    __sigil_released: false,
    __sigil_value: value
  };
}
function __sigil_owned_take(owned) {
  return owned?.__sigil_value;
}
async function __sigil_owned_dispose(owned) {
  if (!owned || owned.__sigil_released) {
    return null;
  }
  owned.__sigil_released = true;
  if (typeof owned.__sigil_dispose === 'function') {
    await owned.__sigil_dispose();
  }
  return null;
}
async function __sigil_subscription_dispose(disposer) {
  if (typeof disposer === 'function') {
    return await disposer();
  }
  if (disposer && typeof disposer.unsubscribe === 'function') {
    return await disposer.unsubscribe();
  }
  if (disposer && typeof disposer.dispose === 'function') {
    return await disposer.dispose();
  }
  return null;
}
async function __sigil_extern_subscribe(_key, actualFn, args = []) {
  const source = __sigil_world_stream_open();
  let closed = false;
  const emit = (value) => {
    if (closed) {
      return;
    }
    __sigil_world_stream_push(source, value === undefined ? null : value);
  };
  const disposer = await Promise.resolve().then(() => actualFn(...args, emit));
  return __sigil_owned_wrap(source, async () => {
    if (closed) {
      return null;
    }
    closed = true;
    try {
      await __sigil_subscription_dispose(disposer);
    } finally {
      await __sigil_world_stream_close(source);
    }
    return null;
  });
}
async function __sigil_world_pty_source(events) {
  const source = __sigil_world_stream_open();
  for (const event of Array.isArray(events) ? events : []) {
    __sigil_world_stream_push(source, event);
  }
  __sigil_world_stream_finish(source);
  return source;
}
function __sigil_world_pty_exit_code(events) {
  let exitCode = -1;
  for (const event of Array.isArray(events) ? events : []) {
    if (event?.__tag === 'Exit') {
      exitCode = Number(event.__fields?.[0] ?? -1);
    }
  }
  return exitCode;
}
function __sigil_world_pty_session_from_state(pid) {
  return { pid: Number(pid) };
}
function __sigil_world_pty_managed_ref_from_state(refId) {
  return { id: String(refId) };
}
function __sigil_world_pty_managed_ref_state(sessionRef) {
  const world = __sigil_current_world();
  const refId = String(sessionRef?.id ?? '');
  const state = world.ptyManagedRefs?.get(refId) ?? null;
  if (!state) {
    __sigil_world_error(`unknown managed pty session '${refId}' in current world`);
  }
  return state;
}
async function __sigil_world_pty_finish_managed_hub(state) {
  if (!state?.managedHub) {
    return null;
  }
  const hub = state.managedHub;
  state.managedHub = null;
  await __sigil_world_stream_finish_hub(hub);
  return null;
}
function __sigil_world_pty_queue_managed_event(state, event) {
  if (state?.managedHub) {
    void __sigil_world_stream_publish(state.managedHub, event);
    return;
  }
  if (!Array.isArray(state.pendingManagedEvents)) {
    state.pendingManagedEvents = [];
  }
  state.pendingManagedEvents.push(event);
}
function __sigil_world_pty_track_session(world, handleName, command, source, state) {
  const pid = Number(state?.pid ?? -1);
  world.ptySessions.set(pid, {
    ...state,
    command: __sigil_world_clone(command),
    handleName: handleName == null ? null : String(handleName),
    managedHub: null,
    pendingManagedEvents: Array.isArray(state?.pendingManagedEvents)
      ? state.pendingManagedEvents.slice()
      : [],
    source
  });
  __sigil_world_pty_trace(world, 'spawn', handleName, {
    spawn: command
  });
  return __sigil_world_pty_session_from_state(pid);
}
function __sigil_world_pty_session_state(session) {
  const world = __sigil_current_world();
  const pid = Number(session?.pid ?? -1);
  const state = world.ptySessions?.get(pid) ?? null;
  if (!state) {
    __sigil_world_error(`unknown pty session '${pid}' in current world`);
  }
  return state;
}
async function __sigil_world_pty_spawn_from_entry(entry, handleName, command) {
  const world = __sigil_current_world();
  if (entry.kind === 'deny') {
    __sigil_world_error('Pty is denied by the current world');
  }
  if (entry.kind === 'fixture') {
    const rule = __sigil_world_pty_fixture_rule(entry, command);
    const events = Array.isArray(rule.events) ? rule.events.slice() : [];
    const source = await __sigil_world_pty_source(events);
    const exitCode = __sigil_world_pty_exit_code(events);
    const pid = -Math.max(1, Number(world.ptyNextId ?? 1));
    world.ptyNextId = Math.abs(pid) + 1;
    return __sigil_world_pty_track_session(world, handleName, command, source, {
      done: Promise.resolve(exitCode),
      exitCode,
      expectedWrites: Array.isArray(rule.writes) ? rule.writes.slice() : [],
      fixture: true,
      pendingManagedEvents: events,
      pid
    });
  }
  let ptyRuntime = null;
  try {
    ptyRuntime =
      typeof globalThis.__sigil_load_pty_runtime === 'function'
        ? await globalThis.__sigil_load_pty_runtime()
        : null;
  } catch (error) {
    __sigil_world_error(`§pty runtime helper is unavailable: ${String(error?.message ?? error ?? '')}`);
  }
  if (!ptyRuntime || typeof ptyRuntime.spawnPty !== 'function') {
    __sigil_world_error('§pty runtime helper is unavailable');
  }
  const source = __sigil_world_stream_open();
  const pty = await ptyRuntime.spawnPty(command);
  let pid = Number(pty?.pid ?? Math.floor(Math.random() * 2147483647));
  if (!Number.isInteger(pid) || pid === 0 || world.ptySessions.has(pid)) {
    pid = Math.max(1, Number(world.ptyNextId ?? 1));
  }
  world.ptyNextId = Math.max(Number(world.ptyNextId ?? 1), Math.abs(pid) + 1);
  const state = {
    closed: false,
    done: null,
    exitCode: null,
    fixture: false,
    pendingManagedEvents: [],
    pid,
    pty
  };
  state.done = new Promise((resolve) => {
    let settled = false;
    const emitEvent = (event) => {
      __sigil_world_stream_push(source, event);
      __sigil_world_pty_queue_managed_event(state, event);
    };
    const finish = (code) => {
      if (settled) {
        return;
      }
      settled = true;
      state.exitCode = Number(code);
      emitEvent({ __tag: 'Exit', __fields: [state.exitCode] });
      __sigil_world_stream_finish(source);
      void __sigil_world_pty_finish_managed_hub(state);
      resolve(state.exitCode);
    };
    pty.onData?.((chunk) => {
      emitEvent({ __tag: 'Output', __fields: [String(chunk)] });
    });
    pty.onExit?.((event) => {
      finish(Number(event?.exitCode ?? -1));
    });
  });
  return __sigil_world_pty_track_session(world, handleName, command, source, state);
}
async function __sigil_world_pty_spawn(command) {
  return await __sigil_world_pty_spawn_from_entry(__sigil_current_world().pty, null, command);
}
async function __sigil_world_pty_spawn_at(handleName, command) {
  return await __sigil_world_pty_spawn_from_entry(
    __sigil_world_named_pty_handle(handleName),
    handleName,
    command
  );
}
async function __sigil_world_pty_register_managed(session) {
  const world = __sigil_current_world();
  const sessionState = __sigil_world_pty_session_state(session);
  const refId = `pty-${String(world.ptyManagedNextId ?? 1)}`;
  world.ptyManagedNextId = Number(world.ptyManagedNextId ?? 1) + 1;
  world.ptyManagedRefs.set(refId, {
    disposed: false,
    pid: Number(sessionState.pid),
    refId
  });
  return __sigil_world_pty_managed_ref_from_state(refId);
}
async function __sigil_world_pty_spawn_managed(command) {
  const session = await __sigil_world_pty_spawn(command);
  return await __sigil_world_pty_register_managed(session);
}
async function __sigil_world_pty_spawn_managed_at(handleName, command) {
  const session = await __sigil_world_pty_spawn_at(handleName, command);
  return await __sigil_world_pty_register_managed(session);
}
async function __sigil_world_pty_events(session) {
  return __sigil_world_pty_session_state(session).source;
}
async function __sigil_world_pty_events_managed(sessionRef) {
  const refState = __sigil_world_pty_managed_ref_state(sessionRef);
  if (refState.disposed) {
    __sigil_world_error(`managed pty session '${refState.refId}' has been disposed`);
  }
  const sessionState = __sigil_current_world().ptySessions?.get(Number(refState.pid)) ?? null;
  if (!sessionState) {
    __sigil_world_error(`managed pty session '${refState.refId}' is unavailable`);
  }
  if (!sessionState.managedHub) {
    const pendingEvents = Array.isArray(sessionState.pendingManagedEvents)
      ? sessionState.pendingManagedEvents.slice()
      : [];
    sessionState.pendingManagedEvents = [];
    if (sessionState.closed || sessionState.exitCode != null) {
      const source = await __sigil_world_pty_source(pendingEvents);
      return __sigil_owned_wrap(source, async () => {
        await __sigil_world_stream_close(source);
        return null;
      });
    }
    const hubOwned = __sigil_world_stream_open_hub();
    sessionState.managedHub = __sigil_owned_take(hubOwned);
    const subscription = await __sigil_world_stream_subscribe(sessionState.managedHub);
    for (const event of pendingEvents) {
      await __sigil_world_stream_publish(sessionState.managedHub, event);
    }
    return subscription;
  }
  return await __sigil_world_stream_subscribe(sessionState.managedHub);
}
async function __sigil_world_pty_write(session, input) {
  const world = __sigil_current_world();
  const state = __sigil_world_pty_session_state(session);
  const text = String(input ?? '');
  __sigil_world_pty_trace(world, 'write', state.handleName, {
    input: text
  });
  if (state.fixture) {
    const expected = Array.isArray(state.expectedWrites) ? state.expectedWrites.shift() : undefined;
    if (expected !== text) {
      __sigil_world_error(`pty fixture write mismatch: expected '${String(expected ?? '')}' but received '${text}'`);
    }
    return null;
  }
  if (!state.closed) {
    state.pty?.write?.(text);
  }
  return null;
}
async function __sigil_world_pty_write_managed(sessionRef, input) {
  const refState = __sigil_world_pty_managed_ref_state(sessionRef);
  if (refState.disposed) {
    __sigil_world_error(`managed pty session '${refState.refId}' has been disposed`);
  }
  return __sigil_world_pty_write(__sigil_world_pty_session_from_state(refState.pid), input);
}
async function __sigil_world_pty_resize(session, cols, rows) {
  const world = __sigil_current_world();
  const state = __sigil_world_pty_session_state(session);
  const width = Math.max(1, Math.trunc(Number(cols)));
  const height = Math.max(1, Math.trunc(Number(rows)));
  __sigil_world_pty_trace(world, 'resize', state.handleName, {
    cols: width,
    rows: height
  });
  if (!state.fixture && !state.closed) {
    state.pty?.resize?.(width, height);
  }
  return null;
}
async function __sigil_world_pty_resize_managed(sessionRef, cols, rows) {
  const refState = __sigil_world_pty_managed_ref_state(sessionRef);
  if (refState.disposed) {
    __sigil_world_error(`managed pty session '${refState.refId}' has been disposed`);
  }
  return __sigil_world_pty_resize(
    __sigil_world_pty_session_from_state(refState.pid),
    cols,
    rows
  );
}
async function __sigil_world_pty_close(session) {
  const world = __sigil_current_world();
  const state = __sigil_world_pty_session_state(session);
  if (state.closed) {
    return null;
  }
  state.closed = true;
  __sigil_world_pty_trace(world, 'close', state.handleName, {});
  await __sigil_world_stream_close(state.source);
  await __sigil_world_pty_finish_managed_hub(state);
  if (!state.fixture) {
    try {
      state.pty?.kill?.();
    } catch (_) {}
  }
  return null;
}
async function __sigil_world_pty_close_managed(sessionRef) {
  const refState = __sigil_world_pty_managed_ref_state(sessionRef);
  if (refState.disposed) {
    return null;
  }
  refState.disposed = true;
  return __sigil_world_pty_close(__sigil_world_pty_session_from_state(refState.pid));
}
async function __sigil_world_pty_wait(session) {
  const state = __sigil_world_pty_session_state(session);
  return Number(await state.done);
}
async function __sigil_world_pty_wait_managed(sessionRef) {
  const refState = __sigil_world_pty_managed_ref_state(sessionRef);
  if (refState.disposed) {
    __sigil_world_error(`managed pty session '${refState.refId}' has been disposed`);
  }
  return __sigil_world_pty_wait(__sigil_world_pty_session_from_state(refState.pid));
}
function __sigil_world_websocket_client_from_state(clientId) {
  return {
    __sigil_websocket_client_id: String(clientId),
    id: String(clientId)
  };
}
function __sigil_world_websocket_client_state(client) {
  const world = __sigil_current_world();
  const clientId = String(client?.__sigil_websocket_client_id ?? client?.id ?? '');
  const state = world.websocketClients?.get(clientId) ?? null;
  if (!state) {
    __sigil_world_error(`unknown websocket client '${clientId}' in current world`);
  }
  return state;
}
function __sigil_world_websocket_entry_for_handle(world, handleName) {
  return world.websocketHandles?.[String(handleName)] ?? world.websocket;
}
function __sigil_world_websocket_message_text(value) {
  if (typeof value === 'string') {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value).toString('utf8');
  }
  if (Array.isArray(value)) {
    return Buffer.concat(value.map((item) => Buffer.from(item))).toString('utf8');
  }
  return String(value ?? '');
}
function __sigil_world_websocket_normalize_port(port) {
  const normalized = Math.trunc(Number(port ?? NaN));
  if (!Number.isInteger(normalized) || normalized < 0 || normalized > 65535) {
    __sigil_world_error(`invalid websocket listen port '${String(port ?? '')}'`);
  }
  return normalized;
}
function __sigil_world_websocket_normalize_routes(routes) {
  if (!Array.isArray(routes)) {
    __sigil_world_error('§websocket.listen requires a list of websocket routes');
  }
  const normalized = [];
  const paths = new Set();
  const handles = new Set();
  for (const route of routes) {
    const handleName = String(route?.handle?.__fields?.[0] ?? '');
    const path = String(route?.path ?? '');
    if (!handleName) {
      __sigil_world_error('websocket routes must use named WebSocketHandle values');
    }
    if (!path || !path.startsWith('/')) {
      __sigil_world_error(`websocket route '${handleName}' must use an exact path that starts with '/'`);
    }
    if (paths.has(path)) {
      __sigil_world_error(`duplicate websocket route path '${path}'`);
    }
    if (handles.has(handleName)) {
      __sigil_world_error(`duplicate websocket route handle '${handleName}'`);
    }
    paths.add(path);
    handles.add(handleName);
    normalized.push({ handleName, path });
  }
  return normalized;
}
function __sigil_world_websocket_register_client(world, handleName, source, state) {
  const clientId = `ws-${String(world.websocketNextClientId ?? 1)}`;
  world.websocketNextClientId = Number(world.websocketNextClientId ?? 1) + 1;
  world.websocketClients.set(clientId, {
    ...state,
    clientId,
    handleName: String(handleName),
    source
  });
  __sigil_world_websocket_trace(world, 'connect', handleName, {
    clientId
  });
  return __sigil_world_websocket_client_from_state(clientId);
}
function __sigil_world_websocket_route_state(serverState, handleName) {
  const state = serverState?.routeStates?.[String(handleName)] ?? null;
  if (!state) {
    __sigil_world_error(`websocket server does not expose route handle '${String(handleName)}'`);
  }
  return state;
}
function __sigil_world_websocket_server_from_state(port, serverId) {
  return {
    __sigil_websocket_server_id: Number(serverId),
    port: Number(port)
  };
}
function __sigil_world_websocket_server_state(server) {
  const world = __sigil_current_world();
  const serverId = Number(server?.__sigil_websocket_server_id ?? NaN);
  const state = world.websocketServers?.get(serverId) ?? null;
  if (!state) {
    __sigil_world_error(`unknown websocket server '${String(serverId)}' in current world`);
  }
  return state;
}
function __sigil_world_websocket_finish_routes(serverState) {
  for (const routeState of Object.values(serverState?.routeStates ?? {})) {
    __sigil_world_stream_finish(routeState.source);
  }
}
async function __sigil_world_websocket_fixture_messages(world, handleName, messages) {
  const source = __sigil_world_stream_open();
  for (const message of Array.isArray(messages) ? messages : []) {
    const text = String(message ?? '');
    __sigil_world_websocket_trace(world, 'received', handleName, {
      text
    });
    __sigil_world_stream_push(source, text);
  }
  __sigil_world_stream_finish(source);
  return source;
}
async function __sigil_world_websocket_listen(port, routes) {
  const world = __sigil_current_world();
  const normalizedPort = __sigil_world_websocket_normalize_port(port);
  const normalizedRoutes = __sigil_world_websocket_normalize_routes(routes);
  const routeStates = Object.create(null);
  const realRoutes = [];
  for (const route of normalizedRoutes) {
    const entry = __sigil_world_websocket_entry_for_handle(world, route.handleName);
    if (entry.kind === 'deny') {
      __sigil_world_error(`WebSocketHandle '${route.handleName}' is denied by the current world`);
    }
    routeStates[route.handleName] = {
      handleName: route.handleName,
      path: route.path,
      source: __sigil_world_stream_open()
    };
    if (entry.kind === 'fixture') {
      for (const rule of entry.rules ?? []) {
        const source = await __sigil_world_websocket_fixture_messages(
          world,
          route.handleName,
          rule.messages ?? []
        );
        const client = __sigil_world_websocket_register_client(world, route.handleName, source, {
          closed: false,
          fixture: true,
          socket: null
        });
        __sigil_world_stream_push(routeStates[route.handleName].source, client);
      }
      __sigil_world_stream_finish(routeStates[route.handleName].source);
    } else {
      realRoutes.push(route);
    }
  }
  const serverId = Number(world.websocketNextServerId ?? 1);
  world.websocketNextServerId = serverId + 1;
  const serverState = {
    done: Promise.resolve(null),
    id: serverId,
    port: normalizedPort,
    routeStates,
    runtime: null
  };
  world.websocketServers.set(serverId, serverState);
  if (realRoutes.length > 0) {
    let websocketRuntime = null;
    try {
      websocketRuntime =
        typeof globalThis.__sigil_load_websocket_runtime === 'function'
          ? await globalThis.__sigil_load_websocket_runtime()
          : null;
    } catch (error) {
      __sigil_world_error(`§websocket runtime helper is unavailable: ${String(error?.message ?? error ?? '')}`);
    }
    if (!websocketRuntime || typeof websocketRuntime.listenServer !== 'function') {
      __sigil_world_error('§websocket runtime helper is unavailable');
    }
    const runtime = await websocketRuntime.listenServer(
      normalizedPort,
      realRoutes,
      (handleName, socket) => {
        const routeState = __sigil_world_websocket_route_state(serverState, handleName);
        const source = __sigil_world_stream_open();
        const client = __sigil_world_websocket_register_client(world, handleName, source, {
          closed: false,
          fixture: false,
          socket
        });
        socket.on?.('message', (value) => {
          const text = __sigil_world_websocket_message_text(value);
          __sigil_world_websocket_trace(world, 'received', handleName, {
            clientId: client.id,
            text
          });
          __sigil_world_stream_push(source, text);
        });
        const finish = () => {
          const state = world.websocketClients?.get(client.id) ?? null;
          if (state) {
            state.closed = true;
          }
          __sigil_world_stream_finish(source);
        };
        socket.once?.('close', finish);
        socket.once?.('error', finish);
        __sigil_world_stream_push(routeState.source, client);
      }
    );
    serverState.port = Number(runtime?.port ?? normalizedPort);
    serverState.runtime = runtime;
    serverState.done = Promise.resolve(runtime?.wait?.()).then(() => {
      __sigil_world_websocket_finish_routes(serverState);
      return null;
    });
  }
  return __sigil_world_websocket_server_from_state(serverState.port, serverId);
}
async function __sigil_world_websocket_connections(handleName, server) {
  const serverState = __sigil_world_websocket_server_state(server);
  return __sigil_world_websocket_route_state(serverState, handleName).source;
}
async function __sigil_world_websocket_messages(client) {
  return __sigil_world_websocket_client_state(client).source;
}
async function __sigil_world_websocket_send(client, text) {
  const world = __sigil_current_world();
  const state = __sigil_world_websocket_client_state(client);
  const message = String(text ?? '');
  if (state.closed) {
    if (state.fixture) {
      __sigil_world_error(`websocket fixture client '${state.clientId}' is already closed`);
    }
    throw new Error(`websocket client '${state.clientId}' is already closed`);
  }
  __sigil_world_websocket_trace(world, 'sent', state.handleName, {
    clientId: state.clientId,
    text: message
  });
  if (state.fixture) {
    return null;
  }
  await new Promise((resolve, reject) => {
    state.socket?.send?.(message, (error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve(undefined);
    });
  });
  return null;
}
async function __sigil_world_websocket_close(client) {
  const world = __sigil_current_world();
  const state = __sigil_world_websocket_client_state(client);
  if (state.closed) {
    return null;
  }
  state.closed = true;
  __sigil_world_websocket_trace(world, 'close', state.handleName, {
    clientId: state.clientId
  });
  await __sigil_world_stream_close(state.source);
  if (!state.fixture) {
    try {
      state.socket?.close?.();
    } catch (_) {}
  }
  return null;
}
async function __sigil_world_websocket_wait(server) {
  const state = __sigil_world_websocket_server_state(server);
  await Promise.resolve(state.done);
  return null;
}
async function __sigil_world_websocket_close_server(server) {
  const state = __sigil_world_websocket_server_state(server);
  if (state.closed) {
    return null;
  }
  state.closed = true;
  try {
    await Promise.resolve(state.runtime?.close?.());
  } catch (_) {}
  __sigil_world_websocket_finish_routes(state);
  return null;
}
function __sigil_world_fswatch_watch_from_state(watchId) {
  return {
    __sigil_fswatch_id: String(watchId),
    id: String(watchId)
  };
}
function __sigil_world_fswatch_watch_state(watch) {
  const world = __sigil_current_world();
  const watchId = String(watch?.__sigil_fswatch_id ?? watch?.id ?? '');
  const state = world.fsWatches?.get(watchId) ?? null;
  if (!state) {
    __sigil_world_error(`unknown fsWatch '${watchId}' in current world`);
  }
  return state;
}
function __sigil_world_fswatch_trace(world, kind, rootName, payload) {
  world.traces.fsWatch.push({
    kind: String(kind),
    rootName: rootName == null ? null : String(rootName),
    ...__sigil_world_clone(payload ?? {})
  });
}
function __sigil_world_named_fswatch_root(rootName) {
  const world = __sigil_current_world();
  const entry = world.fsWatchRoots?.[String(rootName)] ?? null;
  if (!entry) {
    __sigil_world_error(`FsRoot '${String(rootName)}' is not configured for fsWatch in the current world`);
  }
  return entry;
}
function __sigil_world_fswatch_normalize_path(pathValue) {
  const value = String(pathValue ?? '');
  return value === '' ? '.' : value;
}
function __sigil_world_fswatch_fixture_rule(entry, pathValue) {
  const watchPath = __sigil_world_fswatch_normalize_path(pathValue);
  for (const rule of entry?.rules ?? []) {
    if (String(rule?.path ?? '') === watchPath) {
      return __sigil_world_clone(rule);
    }
  }
  __sigil_world_error(`no fsWatch fixture matched path '${watchPath}'`);
}
async function __sigil_world_fswatch_open(entry, rootName, pathValue, fsEntry) {
  const world = __sigil_current_world();
  const watchPath = __sigil_world_fswatch_normalize_path(pathValue);
  if (entry.kind === 'deny') {
    if (rootName == null) {
      __sigil_world_error('FsWatch is denied by the current world');
    }
    __sigil_world_error(`FsWatch for FsRoot '${String(rootName)}' is denied by the current world`);
  }
  const request = await __sigil_world_file_request_path(
    watchPath,
    rootName == null ? null : { rootName: String(rootName) },
    fsEntry ?? null
  );
  const source = __sigil_world_stream_open();
  const watchId = `fswatch-${String(world.fsWatchNextId ?? 1)}`;
  world.fsWatchNextId = Number(world.fsWatchNextId ?? 1) + 1;
  const state = {
    closed: false,
    path: watchPath,
    rootName: rootName == null ? null : String(rootName),
    runtime: null,
    source
  };
  world.fsWatches.set(watchId, state);
  __sigil_world_fswatch_trace(world, 'watch', rootName, {
    path: watchPath
  });
  if (entry.kind === 'fixture') {
    const rule = __sigil_world_fswatch_fixture_rule(entry, watchPath);
    for (const event of rule.events ?? []) {
      __sigil_world_fswatch_trace(world, 'event', rootName, {
        event
      });
      __sigil_world_stream_push(source, event);
    }
    __sigil_world_stream_finish(source);
    return __sigil_world_fswatch_watch_from_state(watchId);
  }
  let fswatchRuntime = null;
  try {
    fswatchRuntime =
      typeof globalThis.__sigil_load_fswatch_runtime === 'function'
        ? await globalThis.__sigil_load_fswatch_runtime()
        : null;
  } catch (error) {
    __sigil_world_error(`§fsWatch runtime helper is unavailable: ${String(error?.message ?? error ?? '')}`);
  }
  if (!fswatchRuntime || typeof fswatchRuntime.watchPath !== 'function') {
    __sigil_world_error('§fsWatch runtime helper is unavailable');
  }
  state.runtime = await fswatchRuntime.watchPath(
    String(request.resolvedPath ?? ''),
    (event) => {
      __sigil_world_fswatch_trace(world, 'event', rootName, {
        event
      });
      __sigil_world_stream_push(source, event);
    }
  );
  return __sigil_world_fswatch_watch_from_state(watchId);
}
async function __sigil_world_fswatch_watch(pathValue) {
  return await __sigil_world_fswatch_open(__sigil_current_world().fsWatch, null, pathValue, null);
}
async function __sigil_world_fswatch_watch_at(rootName, pathValue) {
  return await __sigil_world_fswatch_open(
    __sigil_world_named_fswatch_root(rootName),
    rootName,
    pathValue,
    __sigil_world_named_fs_entry(rootName)
  );
}
async function __sigil_world_fswatch_events(watch) {
  return __sigil_world_fswatch_watch_state(watch).source;
}
async function __sigil_world_fswatch_close(watch) {
  const world = __sigil_current_world();
  const state = __sigil_world_fswatch_watch_state(watch);
  if (state.closed) {
    return null;
  }
  state.closed = true;
  __sigil_world_fswatch_trace(world, 'close', state.rootName, {
    path: state.path
  });
  await __sigil_world_stream_close(state.source);
  try {
    state.runtime?.close?.();
  } catch (_) {}
  return null;
}
function __sigil_world_now_ms(world) {
  if (world.clock.kind === 'fixed') {
    return Number(world.clock.millis);
  }
  if (world.timer.kind === 'virtual' && Number.isFinite(world.timer.nowMs)) {
    return Number(world.timer.nowMs);
  }
  return Date.now();
}
function __sigil_world_random_next_int(world) {
  if (world.random.kind === 'fixture') {
    if (world.random.index >= world.random.draws.length) {
      __sigil_world_error('random fixture exhausted');
    }
    const draw = world.random.draws[world.random.index];
    world.random.index += 1;
    return Math.trunc(Number(draw));
  }
  if (world.random.kind === 'seeded') {
    world.random.state = (world.random.state * 48271) % 2147483647;
    return world.random.state;
  }
  return Math.floor(Math.random() * 2147483646) + 1;
}
function __sigil_world_random_int_between(left, right) {
  const replay = __sigil_replay_take_event('random', 'intBetween');
  if (replay.active) {
    return Number(
      __sigil_replay_resolve_event_value(replay, 'random', 'intBetween', {
        result: 0
      })?.result ?? 0
    );
  }
  const world = __sigil_current_world();
  const min = Math.min(Math.trunc(Number(left)), Math.trunc(Number(right)));
  const max = Math.max(Math.trunc(Number(left)), Math.trunc(Number(right)));
  const width = max - min + 1;
  const raw = __sigil_world_random_next_int(world);
  const offset = ((raw % width) + width) % width;
  const result = min + offset;
  __sigil_replay_record_return('random', 'intBetween', {
    args: [Math.trunc(Number(left)), Math.trunc(Number(right))],
  }, {
    result
  });
  return result;
}
function __sigil_world_random_pick(items) {
  if (!Array.isArray(items) || items.length === 0) {
    return { __tag: 'None', __fields: [] };
  }
  const index = __sigil_world_random_int_between(0, items.length - 1);
  return { __tag: 'Some', __fields: [items[index]] };
}
function __sigil_world_random_shuffle(items) {
  const values = Array.isArray(items) ? items.slice() : [];
  for (let index = values.length - 1; index > 0; index -= 1) {
    const swapIndex = __sigil_world_random_int_between(0, index);
    const nextValue = values[index];
    values[index] = values[swapIndex];
    values[swapIndex] = nextValue;
  }
  return values;
}
function __sigil_world_http_entry_name(entry) {
  if (entry?.__tag !== 'HttpEntry') {
    __sigil_world_error('test HTTP helpers require world::http.HttpEntry');
  }
  return String(entry.__fields?.[0]?.dependencyName ?? '');
}
function __sigil_world_tcp_entry_name(entry) {
  if (entry?.__tag !== 'TcpEntry') {
    __sigil_world_error('test TCP helpers require world::tcp.TcpEntry');
  }
  return String(entry.__fields?.[0]?.dependencyName ?? '');
}
function __sigil_world_http_trace(world, dependencyName, request) {
  if (!world.traces.http[dependencyName]) {
    world.traces.http[dependencyName] = [];
  }
  world.traces.http[dependencyName].push(request);
}
function __sigil_world_tcp_trace(world, dependencyName, request) {
  if (!world.traces.tcp[dependencyName]) {
    world.traces.tcp[dependencyName] = [];
  }
  world.traces.tcp[dependencyName].push(String(request));
}
function __sigil_world_log_trace(world, sinkName, message) {
  world.traces.log.push({
    message: String(message),
    sinkName: sinkName == null ? null : String(sinkName)
  });
}
function __sigil_world_pty_trace(world, kind, handleName, payload) {
  world.traces.pty.push({
    handleName: handleName == null ? null : String(handleName),
    kind: String(kind),
    ...__sigil_world_clone(payload ?? {})
  });
}
function __sigil_world_websocket_trace(world, kind, handleName, payload) {
  world.traces.websocket.push({
    handleName: handleName == null ? null : String(handleName),
    kind: String(kind),
    ...__sigil_world_clone(payload ?? {})
  });
}
function __sigil_world_process_trace(world, handleName, command) {
  world.traces.process.push({
    command,
    handleName: handleName == null ? null : String(handleName)
  });
}
function __sigil_world_pty_matches(rule, command) {
  const expected = rule?.request ?? null;
  const expectedArgv = Array.isArray(expected?.argv) ? expected.argv.map((item) => String(item)) : [];
  const argv = Array.isArray(command?.argv) ? command.argv.map((item) => String(item)) : [];
  if (argv.length !== expectedArgv.length) {
    return false;
  }
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] !== expectedArgv[index]) {
      return false;
    }
  }
  const expectedCwd = expected?.cwd?.__tag === 'Some' ? String(expected.cwd.__fields?.[0] ?? '') : null;
  const cwd = command?.cwd?.__tag === 'Some' ? String(command.cwd.__fields?.[0] ?? '') : null;
  if (cwd !== expectedCwd) {
    return false;
  }
  const expectedCols = Math.trunc(Number(expected?.cols ?? NaN));
  const expectedRows = Math.trunc(Number(expected?.rows ?? NaN));
  const cols = Math.trunc(Number(command?.cols ?? NaN));
  const rows = Math.trunc(Number(command?.rows ?? NaN));
  return cols === expectedCols && rows === expectedRows;
}
function __sigil_world_pty_fixture_rule(entry, command) {
  for (const rule of entry?.rules ?? []) {
    if (__sigil_world_pty_matches(rule, command)) {
      return __sigil_world_clone(rule);
    }
  }
  const argv = Array.isArray(command?.argv) ? command.argv.map((item) => String(item)) : [];
  __sigil_world_error(`no pty fixture matched spawn '${argv.join(" ")}'`);
}
function __sigil_world_process_matches(rule, command) {
  const argv = Array.isArray(command?.argv) ? command.argv.map((item) => String(item)) : [];
  if (argv.length !== rule.argv.length) {
    return false;
  }
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] !== rule.argv[i]) {
      return false;
    }
  }
  const cwd = command?.cwd?.__tag === 'Some' ? String(command.cwd.__fields?.[0] ?? '') : null;
  return cwd === rule.cwd;
}
function __sigil_world_process_fixture_result(world, command) {
  for (const rule of world.process.rules ?? []) {
    if (__sigil_world_process_matches(rule, command)) {
      return __sigil_world_clone(rule.result);
    }
  }
  const argv = Array.isArray(command?.argv) ? command.argv.map((item) => String(item)) : [];
  return {
    code: -1,
    stderr: `no process fixture matched command '${argv.join(" ")}'`,
    stdout: ''
  };
}
function __sigil_world_timer_trace(world, ms) {
  world.traces.timer.sleeps.push(Number(ms));
}
function __sigil_world_named_fs_entry(rootName) {
  const world = __sigil_current_world();
  const entry = world.fsRoots?.[String(rootName)] ?? null;
  if (!entry) {
    __sigil_world_error(`FsRoot '${String(rootName)}' is not configured in the current world`);
  }
  return entry;
}
function __sigil_world_named_log_sink(sinkName) {
  const world = __sigil_current_world();
  const entry = world.logSinks?.[String(sinkName)] ?? null;
  if (!entry) {
    __sigil_world_error(`LogSink '${String(sinkName)}' is not configured in the current world`);
  }
  return entry;
}
function __sigil_world_named_pty_handle(handleName) {
  const world = __sigil_current_world();
  const entry = world.ptyHandles?.[String(handleName)] ?? null;
  if (!entry) {
    __sigil_world_error(`PtyHandle '${String(handleName)}' is not configured in the current world`);
  }
  return entry;
}
function __sigil_world_named_websocket_handle(handleName) {
  const world = __sigil_current_world();
  const entry = world.websocketHandles?.[String(handleName)] ?? null;
  if (!entry) {
    __sigil_world_error(`WebSocketHandle '${String(handleName)}' is not configured in the current world`);
  }
  return entry;
}
function __sigil_world_named_process_handle(handleName) {
  const world = __sigil_current_world();
  const entry = world.processHandles?.[String(handleName)] ?? null;
  if (!entry) {
    __sigil_world_error(`ProcessHandle '${String(handleName)}' is not configured in the current world`);
  }
  return entry;
}
function __sigil_world_named_sql_handle(handleName) {
  const world = __sigil_current_world();
  const entry = world.sqlHandles?.[String(handleName)] ?? null;
  if (!entry) {
    return null;
  }
  return entry;
}
async function __sigil_world_file_text_summary(content) {
  const { createHash } = await import('node:crypto');
  const text = String(content ?? '');
  return {
    kind: 'textSummary',
    length: text.length,
    sha256: createHash('sha256').update(text, 'utf8').digest('hex')
  };
}
function __sigil_world_file_attach_request(error, request) {
  if (!error || typeof error !== 'object') {
    return;
  }
  try {
    error.__sigilReplayRequest = __sigil_world_clone(request ?? {});
  } catch (_) {
    error.__sigilReplayRequest = request ?? {};
  }
}
function __sigil_world_file_request_from_error(error) {
  if (
    error &&
    typeof error === 'object' &&
    error.__sigilReplayRequest &&
    typeof error.__sigilReplayRequest === 'object'
  ) {
    return __sigil_world_clone(error.__sigilReplayRequest);
  }
  return {};
}
async function __sigil_world_file_resolved_path_live(pathValue, entry) {
  const path = await import('node:path');
  const world = __sigil_current_world();
  const fsEntry = entry ?? world.fs;
  const raw = String(pathValue ?? '');
  if (fsEntry.kind === 'deny') {
    __sigil_world_error('Fs is denied by the current world');
  }
  if (fsEntry.kind === 'real') {
    return raw;
  }
  const relative = raw.startsWith('/') ? raw.slice(1) : raw;
  return path.resolve(String(fsEntry.root), relative);
}
async function __sigil_world_file_temp_base_dir(entry) {
  const os = await import('node:os');
  const path = await import('node:path');
  const world = __sigil_current_world();
  const fsEntry = entry ?? world.fs;
  if (fsEntry.kind === 'deny') {
    __sigil_world_error('Fs is denied by the current world');
  }
  if (fsEntry.kind === 'sandbox') {
    return path.resolve(String(fsEntry.root));
  }
  return os.tmpdir();
}
async function __sigil_world_file_request_path(pathValue, extras, entry) {
  const request = {
    path: String(pathValue ?? ''),
    ...(extras ?? {})
  };
  try {
    request.resolvedPath = await __sigil_world_file_resolved_path_live(request.path, entry);
    return request;
  } catch (error) {
    __sigil_world_file_attach_request(error, request);
    throw error;
  }
}
async function __sigil_world_file_request_temp_dir(prefix, extras, entry) {
  const request = {
    prefix: String(prefix ?? ''),
    ...(extras ?? {})
  };
  try {
    request.baseDir = await __sigil_world_file_temp_base_dir(entry);
    return request;
  } catch (error) {
    __sigil_world_file_attach_request(error, request);
    throw error;
  }
}
async function __sigil_world_file_capture(operation, requestBuilder, thunk) {
  const replay = __sigil_replay_take_event('file', operation);
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'file', operation, null);
  }
  let request = null;
  try {
    request = await requestBuilder();
    const value = await thunk(request);
    __sigil_replay_record_return('file', operation, request ?? {}, value);
    return value;
  } catch (error) {
    __sigil_replay_record_throw(
      'file',
      operation,
      request ?? __sigil_world_file_request_from_error(error),
      error
    );
    throw error;
  }
}
async function __sigil_world_file_appendText(content, pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'appendText',
    async () => __sigil_world_file_request_path(pathValue, {
      content: await __sigil_world_file_text_summary(content)
    }),
    async (request) => {
      await fs.appendFile(String(request.resolvedPath ?? ''), String(content), 'utf8');
      return null;
    }
  );
}
async function __sigil_world_file_exists(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture('exists', () => __sigil_world_file_request_path(pathValue), async (request) => {
    try {
      await fs.access(String(request.resolvedPath ?? ''));
      return true;
    } catch (_) {
      return false;
    }
  });
}
async function __sigil_world_file_listDir(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'listDir',
    () => __sigil_world_file_request_path(pathValue),
    async (request) => await fs.readdir(String(request.resolvedPath ?? ''))
  );
}
async function __sigil_world_file_makeDir(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'makeDir',
    () => __sigil_world_file_request_path(pathValue),
    async (request) => {
      await fs.mkdir(String(request.resolvedPath ?? ''), { recursive: false });
      return null;
    }
  );
}
async function __sigil_world_file_makeDirs(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'makeDirs',
    () => __sigil_world_file_request_path(pathValue),
    async (request) => {
      await fs.mkdir(String(request.resolvedPath ?? ''), { recursive: true });
      return null;
    }
  );
}
async function __sigil_world_file_makeTempDir(prefix) {
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  return await __sigil_world_file_capture(
    'makeTempDir',
    () => __sigil_world_file_request_temp_dir(prefix, null, null),
    async (request) => {
      const world = __sigil_current_world();
      const base = String(request.baseDir ?? '');
      if (world.fs.kind === 'sandbox') {
        await fs.mkdir(base, { recursive: true });
      }
      return await fs.mkdtemp(path.join(base, String(request.prefix ?? '')));
    }
  );
}
async function __sigil_world_file_readText(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'readText',
    () => __sigil_world_file_request_path(pathValue),
    async (request) => await fs.readFile(String(request.resolvedPath ?? ''), 'utf8')
  );
}
async function __sigil_world_file_remove(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'remove',
    () => __sigil_world_file_request_path(pathValue),
    async (request) => {
      await fs.unlink(String(request.resolvedPath ?? ''));
      return null;
    }
  );
}
async function __sigil_world_file_removeTree(pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'removeTree',
    () => __sigil_world_file_request_path(pathValue),
    async (request) => {
      await fs.rm(String(request.resolvedPath ?? ''), { force: true, recursive: true });
      return null;
    }
  );
}
async function __sigil_world_file_writeText(content, pathValue) {
  const fs = await import('node:fs/promises');
  return await __sigil_world_file_capture(
    'writeText',
    async () => __sigil_world_file_request_path(pathValue, {
      content: await __sigil_world_file_text_summary(content)
    }),
    async (request) => {
      await fs.writeFile(String(request.resolvedPath ?? ''), String(content), 'utf8');
      return null;
    }
  );
}
async function __sigil_world_file_appendTextAt(rootName, content, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'appendTextAt',
    async () => __sigil_world_file_request_path(pathValue, {
      content: await __sigil_world_file_text_summary(content),
      rootName: String(rootName)
    }, entry),
    async (request) => {
      await fs.appendFile(String(request.resolvedPath ?? ''), String(content), 'utf8');
      return null;
    }
  );
}
async function __sigil_world_file_existsAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'existsAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => {
      try {
        await fs.access(String(request.resolvedPath ?? ''));
        return true;
      } catch (_) {
        return false;
      }
    }
  );
}
async function __sigil_world_file_listDirAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'listDirAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => await fs.readdir(String(request.resolvedPath ?? ''))
  );
}
async function __sigil_world_file_makeDirAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'makeDirAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => {
      await fs.mkdir(String(request.resolvedPath ?? ''), { recursive: false });
      return null;
    }
  );
}
async function __sigil_world_file_makeDirsAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'makeDirsAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => {
      await fs.mkdir(String(request.resolvedPath ?? ''), { recursive: true });
      return null;
    }
  );
}
async function __sigil_world_file_makeTempDirAt(rootName, prefix) {
  const fs = await import('node:fs/promises');
  const path = await import('node:path');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'makeTempDirAt',
    () => __sigil_world_file_request_temp_dir(prefix, { rootName: String(rootName) }, entry),
    async (request) => {
      const base = String(request.baseDir ?? '');
      if (entry.kind === 'sandbox') {
        await fs.mkdir(base, { recursive: true });
      }
      return await fs.mkdtemp(path.join(base, String(request.prefix ?? '')));
    }
  );
}
async function __sigil_world_file_readTextAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'readTextAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => await fs.readFile(String(request.resolvedPath ?? ''), 'utf8')
  );
}
async function __sigil_world_file_removeAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'removeAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => {
      await fs.unlink(String(request.resolvedPath ?? ''));
      return null;
    }
  );
}
async function __sigil_world_file_removeTreeAt(rootName, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'removeTreeAt',
    () => __sigil_world_file_request_path(pathValue, { rootName: String(rootName) }, entry),
    async (request) => {
      await fs.rm(String(request.resolvedPath ?? ''), { force: true, recursive: true });
      return null;
    }
  );
}
async function __sigil_world_file_writeTextAt(rootName, content, pathValue) {
  const fs = await import('node:fs/promises');
  const entry = __sigil_world_named_fs_entry(rootName);
  return await __sigil_world_file_capture(
    'writeTextAt',
    async () => __sigil_world_file_request_path(pathValue, {
      content: await __sigil_world_file_text_summary(content),
      rootName: String(rootName)
    }, entry),
    async (request) => {
      await fs.writeFile(String(request.resolvedPath ?? ''), String(content), 'utf8');
      return null;
    }
  );
}
function __sigil_world_log_debug(message) {
  const world = __sigil_current_world();
  const text = String(message);
  __sigil_world_log_trace(world, null, text);
  if (world.log.kind === 'stdout') {
    console.debug(text);
  }
  return null;
}
function __sigil_world_log_eprintln(message) {
  const world = __sigil_current_world();
  const text = String(message);
  __sigil_world_log_trace(world, null, text);
  if (world.log.kind === 'stdout') {
    console.error(text);
  }
  return null;
}
function __sigil_world_log_print(message) {
  const world = __sigil_current_world();
  const text = String(message);
  __sigil_world_log_trace(world, null, text);
  if (world.log.kind === 'stdout') {
    process.stdout.write(text);
  }
  return null;
}
function __sigil_world_log_println(message) {
  const world = __sigil_current_world();
  const text = String(message);
  __sigil_world_log_trace(world, null, text);
  if (world.log.kind === 'stdout') {
    console.log(text);
  }
  return null;
}
function __sigil_world_log_warn(message) {
  const world = __sigil_current_world();
  const text = String(message);
  __sigil_world_log_trace(world, null, text);
  if (world.log.kind === 'stdout') {
    console.warn(text);
  }
  return null;
}
function __sigil_world_log_write_to(sinkName, message) {
  const world = __sigil_current_world();
  const entry = __sigil_world_named_log_sink(sinkName);
  const text = String(message);
  __sigil_world_log_trace(world, sinkName, text);
  if (entry.kind === 'stdout') {
    process.stdout.write(text);
  }
  return null;
}
let __sigil_terminal_cleanup_installed = false;
let __sigil_terminal_raw_enabled = false;
let __sigil_terminal_cursor_hidden = false;
function __sigil_terminal_is_interactive() {
  return !!(process.stdin && process.stdout && process.stdin.isTTY && process.stdout.isTTY);
}
function __sigil_terminal_install_cleanup() {
  if (__sigil_terminal_cleanup_installed) {
    return;
  }
  __sigil_terminal_cleanup_installed = true;
  const restore = () => {
    if (__sigil_terminal_cursor_hidden) {
      try {
        process.stdout.write('\u001b[?25h');
      } catch (_) {}
      __sigil_terminal_cursor_hidden = false;
    }
    if (__sigil_terminal_raw_enabled && __sigil_terminal_is_interactive()) {
      try {
        process.stdin.setRawMode(false);
      } catch (_) {}
      try {
        process.stdin.pause();
      } catch (_) {}
      __sigil_terminal_raw_enabled = false;
    }
  };
  process.once('exit', restore);
  process.once('SIGINT', () => {
    restore();
    process.exit(130);
  });
}
async function __sigil_world_terminal_enable_raw_mode() {
  __sigil_terminal_install_cleanup();
  if (!__sigil_terminal_is_interactive()) {
    return null;
  }
  process.stdin.setEncoding('utf8');
  process.stdin.setRawMode(true);
  process.stdin.resume();
  __sigil_terminal_raw_enabled = true;
  return null;
}
async function __sigil_world_terminal_clear_screen() {
  __sigil_terminal_install_cleanup();
  if (!process.stdout) {
    return null;
  }
  process.stdout.write('\u001b[2J\u001b[H');
  return null;
}
async function __sigil_world_terminal_disable_raw_mode() {
  if (!__sigil_terminal_is_interactive() || !__sigil_terminal_raw_enabled) {
    return null;
  }
  process.stdin.setRawMode(false);
  process.stdin.pause();
  __sigil_terminal_raw_enabled = false;
  return null;
}
async function __sigil_world_terminal_hide_cursor() {
  __sigil_terminal_install_cleanup();
  if (!process.stdout || __sigil_terminal_cursor_hidden) {
    return null;
  }
  process.stdout.write('\u001b[?25l');
  __sigil_terminal_cursor_hidden = true;
  return null;
}
async function __sigil_world_terminal_show_cursor() {
  if (!process.stdout || !__sigil_terminal_cursor_hidden) {
    return null;
  }
  process.stdout.write('\u001b[?25h');
  __sigil_terminal_cursor_hidden = false;
  return null;
}
async function __sigil_world_terminal_write(text) {
  __sigil_terminal_install_cleanup();
  if (!process.stdout) {
    return null;
  }
  process.stdout.write(String(text));
  return null;
}
function __sigil_terminal_key_escape() {
  return { __tag: 'Escape', __fields: [] };
}
function __sigil_terminal_key_text(value) {
  return { __tag: 'Text', __fields: [String(value)] };
}
function __sigil_terminal_decode_key(chunk) {
  const text = String(chunk ?? '');
  if (!text) {
    return __sigil_terminal_key_escape();
  }
  if (text === '\u0003') {
    throw Object.assign(new Error('SIGINT'), { __sigil_terminal_sigint: true });
  }
  if (text.startsWith('\u001b')) {
    return __sigil_terminal_key_escape();
  }
  return __sigil_terminal_key_text(text[0].toLowerCase());
}
async function __sigil_world_terminal_read_key() {
  __sigil_terminal_install_cleanup();
  if (!process.stdin) {
    return __sigil_terminal_key_escape();
  }
  process.stdin.setEncoding('utf8');
  process.stdin.resume();
  return await new Promise((resolve) => {
    const cleanup = () => {
      process.stdin.off('data', onData);
      process.stdin.off('error', onError);
    };
    const onData = (chunk) => {
      cleanup();
      try {
        resolve(__sigil_terminal_decode_key(chunk));
      } catch (error) {
        if (error && error.__sigil_terminal_sigint) {
          __sigil_world_terminal_show_cursor();
          __sigil_world_terminal_disable_raw_mode();
          process.exit(130);
          return;
        }
        resolve(__sigil_terminal_key_escape());
      }
    };
    const onError = () => {
      cleanup();
      resolve(__sigil_terminal_key_escape());
    };
    process.stdin.once('data', onData);
    process.stdin.once('error', onError);
  });
}
function __sigil_world_time_now_instant() {
  const replay = __sigil_replay_take_event('timer', 'nowInstant');
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'timer', 'nowInstant', {
      result: { epochMillis: 0 }
    })?.result ?? { epochMillis: 0 };
  }
  const result = { epochMillis: __sigil_world_now_ms(__sigil_current_world()) };
  __sigil_replay_record_return('timer', 'nowInstant', {}, { result });
  return result;
}
async function __sigil_world_timer_sleep(ms) {
  const replay = __sigil_replay_take_event('timer', 'sleepMs');
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'timer', 'sleepMs', {
      result: null
    })?.result ?? null;
  }
  const world = __sigil_current_world();
  const delay = Math.max(0, Number(ms));
  __sigil_world_timer_trace(world, delay);
  if (world.timer.kind === 'virtual') {
    world.timer.nowMs = (Number.isFinite(world.timer.nowMs) ? world.timer.nowMs : Date.now()) + delay;
    if (world.clock.kind === 'fixed') {
      world.clock.millis = world.timer.nowMs;
    }
    __sigil_replay_record_return('timer', 'sleepMs', { delay }, { result: null });
    return null;
  }
  const result = await __sigil_sleep(delay);
  __sigil_replay_record_return('timer', 'sleepMs', { delay }, { result });
  return result;
}
async function __sigil_world_timer_after(ms) {
  const world = __sigil_current_world();
  const delay = Math.max(0, Number(ms));
  const source = __sigil_world_stream_open();
  if (world.timer.kind === 'virtual') {
    world.timer.nowMs = (Number.isFinite(world.timer.nowMs) ? world.timer.nowMs : Date.now()) + delay;
    if (world.clock.kind === 'fixed') {
      world.clock.millis = world.timer.nowMs;
    }
    __sigil_world_stream_push(source, null);
    __sigil_world_stream_finish(source);
    return __sigil_owned_wrap(source, async () => {
      await __sigil_world_stream_close(source);
      return null;
    });
  }
  const timeoutId = setTimeout(() => {
    __sigil_world_stream_push(source, null);
    __sigil_world_stream_finish(source);
  }, delay);
  return __sigil_owned_wrap(source, async () => {
    clearTimeout(timeoutId);
    await __sigil_world_stream_close(source);
    return null;
  });
}
async function __sigil_world_timer_every(ms) {
  const world = __sigil_current_world();
  const delay = Math.max(1, Number(ms));
  const source = __sigil_world_stream_open();
  if (world.timer.kind === 'virtual') {
    __sigil_world_stream_push(source, null);
    return __sigil_owned_wrap(source, async () => {
      await __sigil_world_stream_close(source);
      return null;
    });
  }
  const intervalId = setInterval(() => {
    __sigil_world_stream_push(source, null);
  }, delay);
  return __sigil_owned_wrap(source, async () => {
    clearInterval(intervalId);
    await __sigil_world_stream_close(source);
    return null;
  });
}
function __sigil_task_result_cancelled() {
  return { __tag: 'Cancelled', __fields: [] };
}
function __sigil_task_result_failed(message) {
  return { __tag: 'Failed', __fields: [String(message)] };
}
function __sigil_task_result_succeeded(value) {
  return { __tag: 'Succeeded', __fields: [value] };
}
function __sigil_task_error_message(error) {
  return error instanceof Error ? String(error.message ?? error) : String(error);
}
async function __sigil_world_task_spawn(work) {
  const world = __sigil_current_world();
  const taskId = Number(world.taskNextId ?? 1);
  world.taskNextId = taskId + 1;
  const state = {
    cancelled: false,
    id: taskId,
    result: __sigil_task_result_cancelled(),
    settled: false
  };
  state.promise = Promise.resolve()
    .then(() => work())
    .then(
      (value) => {
        state.settled = true;
        state.result = state.cancelled
          ? __sigil_task_result_cancelled()
          : __sigil_task_result_succeeded(value);
        return state.result;
      },
      (error) => {
        state.settled = true;
        state.result = state.cancelled
          ? __sigil_task_result_cancelled()
          : __sigil_task_result_failed(__sigil_task_error_message(error));
        return state.result;
      }
    );
  world.tasks.set(taskId, state);
  const task = { id: taskId };
  return __sigil_owned_wrap(task, async () => {
    state.cancelled = true;
    return null;
  });
}
async function __sigil_world_task_cancel(task) {
  const world = __sigil_current_world();
  const taskId = Number(task?.id ?? -1);
  const state = world.tasks.get(taskId) ?? null;
  if (state) {
    state.cancelled = true;
  }
  return null;
}
async function __sigil_world_task_wait(task) {
  const world = __sigil_current_world();
  const taskId = Number(task?.id ?? -1);
  const state = world.tasks.get(taskId) ?? null;
  if (!state) {
    return __sigil_task_result_failed(`unknown task '${String(taskId)}'`);
  }
  const result = await state.promise;
  return state.cancelled ? __sigil_task_result_cancelled() : result;
}
async function __sigil_world_process_argv() {
  return process.argv.slice(2);
}
async function __sigil_world_process_exit(code) {
  const replay = __sigil_replay_take_event('process', 'exit');
  if (replay.active) {
    const payload = __sigil_replay_resolve_event_value(replay, 'process', 'exit', {
      code: Number(code),
      exited: false,
      result: null
    });
    if (payload?.exited) {
      process.exit(Number(payload.code ?? code));
    }
    return payload?.result ?? null;
  }
  const world = __sigil_current_world();
  if (world.process.kind === 'deny') {
    __sigil_world_process_trace(world, null, { argv: ['exit', String(code)], cwd: { __tag: 'None', __fields: [] }, env: __sigil_map_empty() });
    __sigil_replay_record_return('process', 'exit', { code: Number(code) }, { code: Number(code), exited: false, result: null });
    return null;
  }
  if (world.process.kind === 'fixture') {
    __sigil_world_process_trace(world, null, { argv: ['exit', String(code)], cwd: { __tag: 'None', __fields: [] }, env: __sigil_map_empty() });
    __sigil_replay_record_return('process', 'exit', { code: Number(code) }, { code: Number(code), exited: false, result: null });
    return null;
  }
  __sigil_replay_record_return('process', 'exit', { code: Number(code) }, { code: Number(code), exited: true, result: null });
  __sigil_replay_record_failure('SIGIL-RUNTIME-CHILD-EXIT', `process exited with code ${Number(code)}`, null);
  process.exit(Number(code));
  return null;
}
async function __sigil_world_process_spawn(command) {
  const replay = __sigil_replay_take_event('process', 'spawn');
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'process', 'spawn', {
      handle: { pid: -1 }
    })?.handle ?? { pid: -1 };
  }
  const world = __sigil_current_world();
  __sigil_world_process_trace(world, null, command);
  if (world.process.kind === 'deny') {
    const handle = { pid: -1 };
    const token = __sigil_replay_next_handle_token();
    if (token) handle.__sigil_replay_token = token;
    __sigil_replay_record_return('process', 'spawn', { command }, { handle, token });
    return handle;
  }
  if (world.process.kind === 'fixture') {
    const handle = { pid: -1, __sigil_fixture_result: __sigil_world_process_fixture_result(world, command) };
    const token = __sigil_replay_next_handle_token();
    if (token) handle.__sigil_replay_token = token;
    __sigil_replay_record_return('process', 'spawn', { command }, { handle, token });
    return handle;
  }
  const handle = await __sigil_process_spawn(command);
  const token = __sigil_replay_next_handle_token();
  if (token && handle && typeof handle === 'object') {
    handle.__sigil_replay_token = token;
  }
  __sigil_replay_record_return('process', 'spawn', { command }, { handle, token });
  return handle;
}
async function __sigil_world_process_wait(processHandle) {
  const replay = __sigil_replay_take_event('process', 'wait');
  if (replay.active) {
    const payload = __sigil_replay_resolve_event_value(replay, 'process', 'wait', {
      token: null,
      result: __sigil_process_result(-1, 'missing replay process result', '')
    });
    __sigil_replay_require_handle_token(
      payload?.token ?? null,
      processHandle?.__sigil_replay_token ?? null,
      'wait'
    );
    return payload?.result ?? __sigil_process_result(-1, 'missing replay process result', '');
  }
  const world = __sigil_current_world();
  let result;
  if (world.process.kind === 'deny') {
    result = __sigil_process_result(-1, 'process denied by current world', '');
  } else if (world.process.kind === 'fixture') {
    result = __sigil_world_clone(
      processHandle?.__sigil_fixture_result ?? {
        code: -1,
        stderr: 'missing process fixture result',
        stdout: ''
      }
    );
  } else {
    result = await __sigil_process_wait(processHandle);
  }
  __sigil_replay_record_return('process', 'wait', {
    token: processHandle?.__sigil_replay_token ?? null
  }, {
    token: processHandle?.__sigil_replay_token ?? null,
    result
  });
  return result;
}
async function __sigil_world_process_run(command) {
  const handle = await __sigil_world_process_spawn(command);
  return await __sigil_world_process_wait(handle);
}
function __sigil_sql_failure(kind, message) {
  return { kind: { __tag: String(kind), __fields: [] }, message: String(message ?? '') };
}
function __sigil_sql_err(kind, message) {
  return { __tag: 'Err', __fields: [__sigil_sql_failure(kind, message)] };
}
function __sigil_sql_ok(value) {
  return { __tag: 'Ok', __fields: [value] };
}
function __sigil_sql_column(field, name, scalar) {
  return {
    field: String(field ?? ''),
    name: String(name ?? ''),
    nullable: false,
    scalar: String(scalar ?? '')
  };
}
function __sigil_sql_nullable(column) {
  return {
    field: String(column?.field ?? ''),
    name: String(column?.name ?? ''),
    nullable: true,
    scalar: String(column?.scalar ?? '')
  };
}
function __sigil_sql_table(name, columns) {
  return {
    columns: [...(Array.isArray(columns) ? columns : [])].sort((left, right) =>
      String(left?.field ?? '').localeCompare(String(right?.field ?? ''))
    ),
    name: String(name ?? '')
  };
}
function __sigil_sql_begin_wrap(result) {
  if (result?.__tag !== 'Ok') {
    return result;
  }
  const transaction = result.__fields?.[0] ?? { id: '' };
  return {
    __tag: 'Ok',
    __fields: [
      __sigil_owned_wrap(transaction, async () => {
        await __sigil_world_sql_transaction_cleanup(transaction);
        return null;
      })
    ]
  };
}
function __sigil_sql_runtime_failure(runtime, error) {
  const kind = typeof runtime?.failureKind === 'function' ? runtime.failureKind(error) : 'InvalidQuery';
  const message = typeof runtime?.failureMessage === 'function' ? runtime.failureMessage(error) : String(error instanceof Error ? error.message ?? error : error);
  return __sigil_sql_err(kind, message);
}
async function __sigil_world_sql_backend_for_handle(handleName) {
  const world = __sigil_current_world();
  const entry = __sigil_world_named_sql_handle(handleName);
  if (!entry) {
    return __sigil_sql_err('MissingHandle', `SqlHandle '${String(handleName)}' is not configured in the current world`);
  }
  if (entry.kind === 'deny') {
    return __sigil_sql_err('Denied', `SqlHandle '${String(handleName)}' is denied by the current world`);
  }
  let backend = world.sqlBackends.get(String(handleName));
  if (!backend) {
    const runtime = await globalThis.__sigil_load_sql_runtime();
    try {
      backend = await runtime.connect(entry);
    } catch (error) {
      return __sigil_sql_runtime_failure(runtime, error);
    }
    world.sqlBackends.set(String(handleName), backend);
  }
  return __sigil_sql_ok(backend);
}
function __sigil_world_sql_transaction_state(transaction) {
  const world = __sigil_current_world();
  const transactionId = String(transaction?.id ?? '');
  const state = world.sqlTransactions.get(transactionId) ?? null;
  if (!state) {
    return __sigil_sql_err('Transaction', `unknown SQL transaction '${transactionId}'`);
  }
  return __sigil_sql_ok(state);
}
async function __sigil_world_sql_transaction_cleanup(transaction) {
  const world = __sigil_current_world();
  const transactionId = String(transaction?.id ?? '');
  const state = world.sqlTransactions.get(transactionId) ?? null;
  if (!state) {
    return null;
  }
  world.sqlTransactions.delete(transactionId);
  if (state.active !== true) {
    return null;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    await runtime.rollback(state);
  } catch {
    // best-effort cleanup
  }
  return null;
}
async function __sigil_world_sql_all(handleName, select) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.all(backendResult.__fields?.[0], select));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_one(handleName, select) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.one(backendResult.__fields?.[0], select));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_exec_insert(handleName, statement) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.execInsert(backendResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_exec_update(handleName, statement) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.execUpdate(backendResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_exec_delete(handleName, statement) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.execDelete(backendResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_raw_exec(handleName, statement) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.rawExec(backendResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_raw_query(handleName, statement) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.rawQuery(backendResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_raw_query_one(handleName, statement) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.rawQueryOne(backendResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_begin(handleName) {
  const backendResult = await __sigil_world_sql_backend_for_handle(handleName);
  if (backendResult?.__tag !== 'Ok') {
    return backendResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    const state = await runtime.begin(backendResult.__fields?.[0]);
    const world = __sigil_current_world();
    const transactionId = `sql-${String(world.sqlNextTransactionId ?? 1)}`;
    world.sqlNextTransactionId = Number(world.sqlNextTransactionId ?? 1) + 1;
    world.sqlTransactions.set(transactionId, state);
    return __sigil_sql_ok({ id: transactionId });
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_commit(transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    await runtime.commit(stateResult.__fields?.[0]);
    __sigil_current_world().sqlTransactions.delete(String(transaction?.id ?? ''));
    return __sigil_sql_ok(null);
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_rollback(transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    await runtime.rollback(stateResult.__fields?.[0]);
    __sigil_current_world().sqlTransactions.delete(String(transaction?.id ?? ''));
    return __sigil_sql_ok(null);
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_all_in(select, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.allIn(stateResult.__fields?.[0], select));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_one_in(select, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.oneIn(stateResult.__fields?.[0], select));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_exec_insert_in(statement, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.execInsertIn(stateResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_exec_update_in(statement, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.execUpdateIn(stateResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_exec_delete_in(statement, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.execDeleteIn(stateResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_raw_exec_in(statement, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.rawExecIn(stateResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_raw_query_in(statement, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.rawQueryIn(stateResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_sql_raw_query_one_in(statement, transaction) {
  const stateResult = __sigil_world_sql_transaction_state(transaction);
  if (stateResult?.__tag !== 'Ok') {
    return stateResult;
  }
  const runtime = await globalThis.__sigil_load_sql_runtime();
  try {
    return __sigil_sql_ok(await runtime.rawQueryOneIn(stateResult.__fields?.[0], statement));
  } catch (error) {
    return __sigil_sql_runtime_failure(runtime, error);
  }
}
async function __sigil_world_process_spawn_at(handleName, command) {
  const replay = __sigil_replay_take_event('process', 'spawnAt');
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'process', 'spawnAt', {
      handle: { pid: -1 }
    })?.handle ?? { pid: -1 };
  }
  const world = __sigil_current_world();
  const entry = __sigil_world_named_process_handle(handleName);
  __sigil_world_process_trace(world, handleName, command);
  if (entry.kind === 'deny') {
    const handle = { pid: -1 };
    const token = __sigil_replay_next_handle_token();
    if (token) handle.__sigil_replay_token = token;
    __sigil_replay_record_return('process', 'spawnAt', { command, handleName: String(handleName) }, { handle, token });
    return handle;
  }
  if (entry.kind === 'fixture') {
    const handle = { pid: -1, __sigil_fixture_result: __sigil_world_clone(__sigil_world_process_fixture_result({ process: entry }, command)) };
    const token = __sigil_replay_next_handle_token();
    if (token) handle.__sigil_replay_token = token;
    __sigil_replay_record_return('process', 'spawnAt', { command, handleName: String(handleName) }, { handle, token });
    return handle;
  }
  const handle = await __sigil_process_spawn(command);
  const token = __sigil_replay_next_handle_token();
  if (token && handle && typeof handle === 'object') {
    handle.__sigil_replay_token = token;
  }
  __sigil_replay_record_return('process', 'spawnAt', { command, handleName: String(handleName) }, { handle, token });
  return handle;
}
async function __sigil_world_process_run_at(handleName, command) {
  const handle = await __sigil_world_process_spawn_at(handleName, command);
  return await __sigil_world_process_wait(handle);
}
async function __sigil_world_process_kill(processHandle) {
  const replay = __sigil_replay_take_event('process', 'kill');
  if (replay.active) {
    const payload = __sigil_replay_resolve_event_value(replay, 'process', 'kill', {
      token: null,
      result: null
    });
    __sigil_replay_require_handle_token(
      payload?.token ?? null,
      processHandle?.__sigil_replay_token ?? null,
      'kill'
    );
    return payload?.result ?? null;
  }
  const world = __sigil_current_world();
  if (world.process.kind === 'deny') {
    __sigil_replay_record_return('process', 'kill', {
      token: processHandle?.__sigil_replay_token ?? null
    }, {
      token: processHandle?.__sigil_replay_token ?? null,
      result: null
    });
    return null;
  }
  if (world.process.kind === 'fixture') {
    __sigil_replay_record_return('process', 'kill', {
      token: processHandle?.__sigil_replay_token ?? null
    }, {
      token: processHandle?.__sigil_replay_token ?? null,
      result: null
    });
    return null;
  }
  const result = await __sigil_process_kill(processHandle);
  __sigil_replay_record_return('process', 'kill', {
    token: processHandle?.__sigil_replay_token ?? null
  }, {
    token: processHandle?.__sigil_replay_token ?? null,
    result
  });
  return result;
}
async function __sigil_world_http_request(request) {
  const world = __sigil_current_world();
  const dependencyName = String(request?.dependency?.__fields?.[0] ?? '');
  const replay = __sigil_replay_take_event('http', 'request');
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'http', 'request', {
      result: { __tag: 'Err', __fields: [__sigil_http_error('Network', 'missing replay HTTP result')] }
    })?.result ?? { __tag: 'Err', __fields: [__sigil_http_error('Network', 'missing replay HTTP result')] };
  }
  const entry = world.http[dependencyName];
  if (!entry) {
    const result = { __tag: 'Err', __fields: [__sigil_http_error('Topology', `missing HTTP world entry for '${dependencyName}'`)] };
    __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
    return result;
  }
  __sigil_world_http_trace(world, dependencyName, request);
  if (entry.kind === 'deny') {
    const result = { __tag: 'Err', __fields: [__sigil_http_error('Topology', `HTTP dependency '${dependencyName}' is denied by the current world`)] };
    __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
    return result;
  }
  if (entry.kind === 'fixture') {
    const method = __sigil_http_method_to_string(request?.method);
    const path = String(request?.path ?? '/');
    const body = request?.body?.__tag === 'Some' ? String(request.body.__fields[0]) : '';
    const rule = entry.rules.find((candidate) =>
      candidate.method === method &&
      candidate.path === path &&
      (candidate.bodyMatch.kind === 'any' || body.includes(candidate.bodyMatch.fragment))
    );
    if (!rule) {
      const result = { __tag: 'Err', __fields: [__sigil_http_error('Network', `no HTTP fixture matched ${method} ${path} for '${dependencyName}'`)] };
      __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
      return result;
    }
    if (rule.response.kind === 'timeout') {
      const result = { __tag: 'Err', __fields: [__sigil_http_error('Timeout', `HTTP fixture timed out for '${dependencyName}'`)] };
      __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
      return result;
    }
    const result = {
      __tag: 'Ok',
      __fields: [{
        body: rule.response.body,
        headers: __sigil_map_empty(),
        status: rule.response.status,
        url: `fixture://${dependencyName}${path}`
      }]
    };
    __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
    return result;
  }
  try {
    const parsed = new URL(String(request?.path ?? '/'), entry.baseUrl);
    const init = { headers: __sigil_http_headers_to_js(request?.headers), method: __sigil_http_method_to_string(request?.method) };
    if (request?.body?.__tag === 'Some') {
      init.body = request.body.__fields[0];
    }
    const response = await fetch(parsed, init);
    const body = await response.text();
    const result = { __tag: 'Ok', __fields: [{ body, headers: __sigil_http_headers_from_web(response.headers), status: response.status, url: response.url }] };
    __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
    return result;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const result = { __tag: 'Err', __fields: [__sigil_http_error(message.includes('Invalid URL') ? 'InvalidUrl' : 'Network', message)] };
    __sigil_replay_record_return('http', 'request', { dependencyName, request }, { result });
    return result;
  }
}
async function __sigil_world_tcp_request(request) {
  const world = __sigil_current_world();
  const dependencyName = String(request?.dependency?.__fields?.[0] ?? '');
  const replay = __sigil_replay_take_event('tcp', 'request');
  if (replay.active) {
    return __sigil_replay_resolve_event_value(replay, 'tcp', 'request', {
      result: { __tag: 'Err', __fields: [__sigil_tcp_error('Protocol', 'missing replay TCP result')] }
    })?.result ?? { __tag: 'Err', __fields: [__sigil_tcp_error('Protocol', 'missing replay TCP result')] };
  }
  const entry = world.tcp[dependencyName];
  if (!entry) {
    const result = { __tag: 'Err', __fields: [__sigil_tcp_error('Topology', `missing TCP world entry for '${dependencyName}'`)] };
    __sigil_replay_record_return('tcp', 'request', { dependencyName, request }, { result });
    return result;
  }
  __sigil_world_tcp_trace(world, dependencyName, request?.message ?? '');
  if (entry.kind === 'deny') {
    const result = { __tag: 'Err', __fields: [__sigil_tcp_error('Topology', `TCP dependency '${dependencyName}' is denied by the current world`)] };
    __sigil_replay_record_return('tcp', 'request', { dependencyName, request }, { result });
    return result;
  }
  if (entry.kind === 'fixture') {
    const message = String(request?.message ?? '');
    const rule = entry.rules.find((candidate) => candidate.request === message);
    if (!rule) {
      const result = { __tag: 'Err', __fields: [__sigil_tcp_error('Protocol', `no TCP fixture matched '${message}' for '${dependencyName}'`)] };
      __sigil_replay_record_return('tcp', 'request', { dependencyName, request }, { result });
      return result;
    }
    if (rule.response.kind === 'timeout') {
      const result = { __tag: 'Err', __fields: [__sigil_tcp_error('Timeout', `TCP fixture timed out for '${dependencyName}'`)] };
      __sigil_replay_record_return('tcp', 'request', { dependencyName, request }, { result });
      return result;
    }
    const result = { __tag: 'Ok', __fields: [{ message: rule.response.body }] };
    __sigil_replay_record_return('tcp', 'request', { dependencyName, request }, { result });
    return result;
  }
  const { Socket } = await import('node:net');
  return await new Promise((resolve) => {
    const socket = new Socket();
    let settled = false;
    let received = '';
    const finish = (value) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      __sigil_replay_record_return('tcp', 'request', { dependencyName, request }, { result: value });
      resolve(value);
    };
    socket.setEncoding('utf8');
    socket.setTimeout(5000);
    socket.once('connect', () => {
      socket.write(`${String(request?.message ?? '')}\n`);
    });
    socket.on('data', (chunk) => {
      received += String(chunk);
      const line = __sigil_tcp_first_line(received);
      if (line !== null) {
        finish({ __tag: 'Ok', __fields: [{ message: line }] });
      }
    });
    socket.once('timeout', () => {
      finish({ __tag: 'Err', __fields: [__sigil_tcp_error('Timeout', 'TCP request timed out')] });
    });
    socket.once('error', (error) => {
      finish({ __tag: 'Err', __fields: [__sigil_tcp_error('Connection', error instanceof Error ? error.message : String(error))] });
    });
    socket.once('close', () => {
      if (!settled) {
        finish({ __tag: 'Err', __fields: [__sigil_tcp_error('Protocol', 'TCP response closed before a newline-delimited message was received')] });
      }
    });
    socket.connect(entry.port, entry.host);
  });
}
function __sigil_test_http_requests(entry) {
  const world = __sigil_current_world();
  return (world.traces.http[__sigil_world_http_entry_name(entry)] ?? []).slice();
}
function __sigil_test_http_last_request(entry) {
  const requests = __sigil_test_http_requests(entry);
  return requests.length === 0 ? { __tag: 'None', __fields: [] } : { __tag: 'Some', __fields: [requests[requests.length - 1]] };
}
function __sigil_test_http_last_path(entry) {
  const last = __sigil_test_http_last_request(entry);
  return last.__tag === 'Some' ? { __tag: 'Some', __fields: [String(last.__fields[0]?.path ?? '/')] } : { __tag: 'None', __fields: [] };
}
function __sigil_test_tcp_requests(entry) {
  const world = __sigil_current_world();
  return (world.traces.tcp[__sigil_world_tcp_entry_name(entry)] ?? []).slice();
}
function __sigil_test_tcp_last_request(entry) {
  const requests = __sigil_test_tcp_requests(entry);
  return requests.length === 0 ? { __tag: 'None', __fields: [] } : { __tag: 'Some', __fields: [requests[requests.length - 1]] };
}
async function __sigil_test_file_exists(pathValue) {
  return await __sigil_world_file_exists(pathValue);
}
async function __sigil_test_file_exists_at(rootName, pathValue) {
  return await __sigil_world_file_existsAt(rootName, pathValue);
}
async function __sigil_test_file_list_dir(pathValue) {
  return await __sigil_world_file_listDir(pathValue);
}
async function __sigil_test_file_list_dir_at(rootName, pathValue) {
  return await __sigil_world_file_listDirAt(rootName, pathValue);
}
async function __sigil_test_file_read_text(pathValue) {
  return await __sigil_world_file_readText(pathValue);
}
async function __sigil_test_file_read_text_at(rootName, pathValue) {
  return await __sigil_world_file_readTextAt(rootName, pathValue);
}
function __sigil_test_fswatch_watches() {
  return __sigil_current_world().traces.fsWatch
    .filter((entry) => entry.kind === 'watch')
    .map((entry) => String(entry.path ?? ''));
}
function __sigil_test_fswatch_watches_at(rootName) {
  return __sigil_current_world().traces.fsWatch
    .filter((entry) => entry.kind === 'watch' && entry.rootName === String(rootName))
    .map((entry) => String(entry.path ?? ''));
}
function __sigil_test_fswatch_events() {
  return __sigil_current_world().traces.fsWatch
    .filter((entry) => entry.kind === 'event')
    .map((entry) => __sigil_world_clone(entry.event));
}
function __sigil_test_fswatch_events_at(rootName) {
  return __sigil_current_world().traces.fsWatch
    .filter((entry) => entry.kind === 'event' && entry.rootName === String(rootName))
    .map((entry) => __sigil_world_clone(entry.event));
}
function __sigil_test_fswatch_close_count() {
  return __sigil_current_world().traces.fsWatch
    .filter((entry) => entry.kind === 'close')
    .length;
}
function __sigil_test_fswatch_close_count_at(rootName) {
  return __sigil_current_world().traces.fsWatch
    .filter((entry) => entry.kind === 'close' && entry.rootName === String(rootName))
    .length;
}
function __sigil_test_log_entries() {
  return __sigil_current_world().traces.log.map((entry) => entry.message);
}
function __sigil_test_log_entries_at(sinkName) {
  return __sigil_current_world().traces.log
    .filter((entry) => entry.sinkName === String(sinkName))
    .map((entry) => entry.message);
}
function __sigil_test_pty_spawns() {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'spawn')
    .map((entry) => entry.spawn);
}
function __sigil_test_pty_spawns_at(handleName) {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'spawn' && entry.handleName === String(handleName))
    .map((entry) => entry.spawn);
}
function __sigil_test_pty_spawn_count() {
  return __sigil_test_pty_spawns().length;
}
function __sigil_test_pty_spawn_count_at(handleName) {
  return __sigil_test_pty_spawns_at(handleName).length;
}
function __sigil_test_pty_writes() {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'write')
    .map((entry) => String(entry.input ?? ''));
}
function __sigil_test_pty_writes_at(handleName) {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'write' && entry.handleName === String(handleName))
    .map((entry) => String(entry.input ?? ''));
}
function __sigil_test_pty_resizes() {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'resize')
    .map((entry) => ({ cols: Number(entry.cols ?? 0), rows: Number(entry.rows ?? 0) }));
}
function __sigil_test_pty_resizes_at(handleName) {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'resize' && entry.handleName === String(handleName))
    .map((entry) => ({ cols: Number(entry.cols ?? 0), rows: Number(entry.rows ?? 0) }));
}
function __sigil_test_pty_close_count() {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'close')
    .length;
}
function __sigil_test_pty_close_count_at(handleName) {
  return __sigil_current_world().traces.pty
    .filter((entry) => entry.kind === 'close' && entry.handleName === String(handleName))
    .length;
}
function __sigil_test_websocket_connection_count() {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'connect')
    .length;
}
function __sigil_test_websocket_connection_count_at(handleName) {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'connect' && entry.handleName === String(handleName))
    .length;
}
function __sigil_test_websocket_received() {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'received')
    .map((entry) => String(entry.text ?? ''));
}
function __sigil_test_websocket_received_at(handleName) {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'received' && entry.handleName === String(handleName))
    .map((entry) => String(entry.text ?? ''));
}
function __sigil_test_websocket_sent() {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'sent')
    .map((entry) => String(entry.text ?? ''));
}
function __sigil_test_websocket_sent_at(handleName) {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'sent' && entry.handleName === String(handleName))
    .map((entry) => String(entry.text ?? ''));
}
function __sigil_test_websocket_close_count() {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'close')
    .length;
}
function __sigil_test_websocket_close_count_at(handleName) {
  return __sigil_current_world().traces.websocket
    .filter((entry) => entry.kind === 'close' && entry.handleName === String(handleName))
    .length;
}
function __sigil_test_process_call_count() {
  return __sigil_current_world().traces.process.length;
}
function __sigil_test_process_call_count_at(handleName) {
  return __sigil_current_world().traces.process
    .filter((entry) => entry.handleName === String(handleName))
    .length;
}
function __sigil_test_process_commands() {
  return __sigil_current_world().traces.process.map((entry) => entry.command);
}
function __sigil_test_process_commands_at(handleName) {
  return __sigil_current_world().traces.process
    .filter((entry) => entry.handleName === String(handleName))
    .map((entry) => entry.command);
}
function __sigil_test_current_iso() {
  return new Date(__sigil_world_now_ms(__sigil_current_world())).toISOString();
}
function __sigil_test_timer_sleep_count() {
  return __sigil_current_world().traces.timer.sleeps.length;
}
function __sigil_test_timer_last_sleep_ms() {
  const sleeps = __sigil_current_world().traces.timer.sleeps;
  return sleeps.length === 0 ? { __tag: 'None', __fields: [] } : { __tag: 'Some', __fields: [sleeps[sleeps.length - 1]] };
}"#;

const COVERAGE_RUNTIME_HELPERS: &str = r#"function __sigil_record_coverage_call(moduleId, functionName) {
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
}"#;

pub fn world_runtime_helpers_source() -> String {
    format!("{REPLAY_RUNTIME_HELPERS}{WORLD_RUNTIME_HELPERS}")
}

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("Codegen error: {0}")]
    General(String),
}

pub struct CodegenOptions {
    pub module_id: Option<String>,
    pub source_file: Option<String>,
    pub output_file: Option<String>,
    pub import_extension: String,
    pub fswatch_runtime_import_specifier: Option<String>,
    pub pty_runtime_import_specifier: Option<String>,
    pub sql_runtime_import_specifier: Option<String>,
    pub websocket_runtime_import_specifier: Option<String>,
    pub lazy_extern_namespaces: bool,
    pub trace: bool,
    pub breakpoints: bool,
    pub expression_debug: bool,
}

impl Default for CodegenOptions {
    fn default() -> Self {
        Self {
            module_id: None,
            source_file: None,
            output_file: None,
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        }
    }
}

#[derive(Debug, Clone)]
struct TraceOwner {
    declaration_kind: &'static str,
    declaration_label: String,
}

pub struct TypeScriptGenerator {
    current_trace_owner: Option<TraceOwner>,
    indent: usize,
    declaration_span_ids: Vec<Option<String>>,
    local_type_names: BTreeSet<String>,
    module_id: Option<String>,
    output: Vec<String>,
    span_map: Option<ModuleSpanMap>,
    source_file: Option<String>,
    output_file: Option<String>,
    import_extension: String,
    fswatch_runtime_import_specifier: Option<String>,
    pty_runtime_import_specifier: Option<String>,
    sql_runtime_import_specifier: Option<String>,
    websocket_runtime_import_specifier: Option<String>,
    lazy_extern_namespaces: bool,
    test_meta_entries: Vec<String>,
    trace_enabled: bool,
    breakpoints_enabled: bool,
    expression_debug_enabled: bool,
}

impl TypeScriptGenerator {
    pub fn new(options: CodegenOptions) -> Self {
        let debug_enabled = options.trace || options.breakpoints || options.expression_debug;
        Self {
            current_trace_owner: None,
            indent: 0,
            declaration_span_ids: Vec::new(),
            local_type_names: BTreeSet::new(),
            module_id: options.module_id,
            output: Vec::new(),
            span_map: None,
            source_file: options.source_file,
            output_file: options.output_file,
            import_extension: options.import_extension,
            fswatch_runtime_import_specifier: options.fswatch_runtime_import_specifier,
            pty_runtime_import_specifier: options.pty_runtime_import_specifier,
            sql_runtime_import_specifier: options.sql_runtime_import_specifier,
            websocket_runtime_import_specifier: options.websocket_runtime_import_specifier,
            lazy_extern_namespaces: options.lazy_extern_namespaces,
            test_meta_entries: Vec::new(),
            trace_enabled: debug_enabled,
            breakpoints_enabled: options.breakpoints,
            expression_debug_enabled: options.expression_debug,
        }
    }

    /// Determine if declarations should be exported based on source file extension
    /// - .lib.sigil files: export ALL top-level declarations
    /// - .sigil files: export NOTHING (executables)
    fn should_export_from_lib(&self) -> bool {
        if let Some(ref source_file) = self.source_file {
            source_file.ends_with(".lib.sigil")
        } else {
            false
        }
    }

    pub fn generate(&mut self, program: &TypedProgram) -> Result<String, CodegenError> {
        self.output.clear();
        self.indent = 0;
        self.current_trace_owner = None;
        self.declaration_span_ids.clear();
        self.local_type_names = program
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                TypedDeclaration::Type(type_decl) => Some(type_decl.ast.name.clone()),
                _ => None,
            })
            .collect();
        self.span_map = self.build_span_map(program);
        self.test_meta_entries.clear();
        let runtime_modules = collect_runtime_module_ids(program);
        // Emit runtime helpers first
        let include_world_runtime = requires_world_runtime(program, &runtime_modules)
            || self.requires_world_runtime_for_source();
        self.emit_runtime_helpers(&runtime_modules, include_world_runtime);

        // Implicit core prelude constructors are available unqualified in every
        // module except the prelude module itself, so runtime code needs the same
        // bindings even though the typechecker injected them implicitly.
        self.emit_core_prelude_runtime_import()?;
        self.emit_runtime_module_imports(&runtime_modules)?;

        // Generate code for all declarations
        for (index, decl) in program.declarations.iter().enumerate() {
            let start_line = self.current_generated_line();
            self.generate_declaration(decl)?;
            let end_line = self.current_generated_line().saturating_sub(1);
            self.annotate_declaration_generated_range(index, start_line, end_line);
            self.output.push("\n".to_string());
        }

        // Emit test metadata if any tests were found
        if !self.test_meta_entries.is_empty() {
            self.emit("export const __sigil_tests = [");
            self.indent += 1;
            let entries = self.test_meta_entries.clone();
            for entry in &entries {
                self.emit(&format!("{},", entry));
            }
            self.indent -= 1;
            self.emit("];");
            self.output.push("\n".to_string());
        }

        Ok(self.output.join(""))
    }

    pub fn generated_span_map(&self) -> Option<&ModuleSpanMap> {
        self.span_map.as_ref()
    }

    fn emit_core_prelude_runtime_import(&mut self) -> Result<(), CodegenError> {
        let Some(source_file) = self.source_file.as_deref() else {
            return Ok(());
        };

        if source_file.ends_with("language/core/prelude.lib.sigil") {
            return Ok(());
        }

        let import_path = if let Some(ref output_file) = self.output_file {
            let output_path = Path::new(output_file);
            if let Some(local_root) = find_output_root(output_path) {
                let target_abs = local_root.join(format!("core/prelude.{}", self.import_extension));
                relative_import_path(
                    output_path.parent().unwrap_or_else(|| Path::new(".")),
                    &target_abs,
                )
            } else {
                format!("./core/prelude.{}", self.import_extension)
            }
        } else {
            format!("./core/prelude.{}", self.import_extension)
        };

        self.emit(&format!(
            "import {{ Some, None, Ok, Err, Aborted, Failure, Success }} from '{}';",
            import_path
        ));
        self.output.push("\n".to_string());
        Ok(())
    }

    fn emit_runtime_module_imports(
        &mut self,
        runtime_modules: &BTreeSet<String>,
    ) -> Result<(), CodegenError> {
        for module_id in runtime_modules {
            if module_id == "core::prelude" {
                continue;
            }
            self.emit_module_import(&module_id)?;
        }
        if !runtime_modules.is_empty() {
            self.output.push("\n".to_string());
        }
        Ok(())
    }

    fn emit(&mut self, line: &str) {
        let indentation = "  ".repeat(self.indent);
        self.output.push(format!("{}{}\n", indentation, line));
    }

    fn current_generated_line(&self) -> usize {
        self.output
            .iter()
            .map(|segment| segment.bytes().filter(|byte| *byte == b'\n').count())
            .sum::<usize>()
            + 1
    }

    fn build_span_map(&mut self, program: &TypedProgram) -> Option<ModuleSpanMap> {
        let (Some(module_id), Some(source_file), Some(output_file)) = (
            self.module_id.as_deref(),
            self.source_file.as_deref(),
            self.output_file.as_deref(),
        ) else {
            return None;
        };
        let collected = collect_module_span_map(module_id, source_file, output_file, program);
        self.declaration_span_ids = collected.declaration_span_ids;
        Some(collected.span_map)
    }

    fn annotate_declaration_generated_range(
        &mut self,
        declaration_index: usize,
        start_line: usize,
        end_line: usize,
    ) {
        let Some(span_map) = self.span_map.as_mut() else {
            return;
        };
        let Some(Some(span_id)) = self.declaration_span_ids.get(declaration_index) else {
            return;
        };
        span_map.annotate_generated_range(span_id, start_line, end_line);
    }

    fn with_trace_owner<T, F>(
        &mut self,
        declaration_kind: &'static str,
        declaration_label: String,
        f: F,
    ) -> Result<T, CodegenError>
    where
        F: FnOnce(&mut Self) -> Result<T, CodegenError>,
    {
        let previous = self.current_trace_owner.clone();
        self.current_trace_owner = Some(TraceOwner {
            declaration_kind,
            declaration_label,
        });
        let result = f(self);
        self.current_trace_owner = previous;
        result
    }

    fn source_file_is(&self, suffix: &str) -> bool {
        self.source_file
            .as_deref()
            .is_some_and(|path| path.ends_with(suffix))
    }

    fn source_file_is_test_observe_module(&self) -> bool {
        self.source_file
            .as_deref()
            .is_some_and(|path| path.contains("/language/test/observe/"))
    }

    fn requires_process_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::process")
            || self.source_file_is("language/stdlib/process.lib.sigil")
    }

    fn requires_fswatch_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::fsWatch")
            || self.source_file_is("language/stdlib/fsWatch.lib.sigil")
    }

    fn requires_pty_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::pty")
            || self.source_file_is("language/stdlib/pty.lib.sigil")
    }

    fn requires_sql_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::sql")
            || self.source_file_is("language/stdlib/sql.lib.sigil")
    }

    fn requires_websocket_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::websocket")
            || runtime_modules.contains("stdlib::httpServer")
            || self.source_file_is("language/stdlib/websocket.lib.sigil")
            || self.source_file_is("language/stdlib/httpServer.lib.sigil")
    }

    fn requires_http_server_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::httpServer")
            || self.source_file_is("language/stdlib/httpServer.lib.sigil")
    }

    fn requires_cli_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::cli")
            || self.source_file_is("language/stdlib/cli.lib.sigil")
    }

    fn requires_tcp_server_runtime(&self, runtime_modules: &BTreeSet<String>) -> bool {
        runtime_modules.contains("stdlib::tcpServer")
            || self.source_file_is("language/stdlib/tcpServer.lib.sigil")
    }

    fn requires_world_runtime_for_source(&self) -> bool {
        self.source_file_is("language/stdlib/cli.lib.sigil")
            || self.source_file_is("language/stdlib/file.lib.sigil")
            || self.source_file_is("language/stdlib/fsWatch.lib.sigil")
            || self.source_file_is("language/stdlib/httpClient.lib.sigil")
            || self.source_file_is("language/stdlib/httpServer.lib.sigil")
            || self.source_file_is("language/stdlib/io.lib.sigil")
            || self.source_file_is("language/stdlib/log.lib.sigil")
            || self.source_file_is("language/stdlib/process.lib.sigil")
            || self.source_file_is("language/stdlib/pty.lib.sigil")
            || self.source_file_is("language/stdlib/random.lib.sigil")
            || self.source_file_is("language/stdlib/sql.lib.sigil")
            || self.source_file_is("language/stdlib/stream.lib.sigil")
            || self.source_file_is("language/stdlib/task.lib.sigil")
            || self.source_file_is("language/stdlib/tcpClient.lib.sigil")
            || self.source_file_is("language/stdlib/tcpServer.lib.sigil")
            || self.source_file_is("language/stdlib/terminal.lib.sigil")
            || self.source_file_is("language/stdlib/timer.lib.sigil")
            || self.source_file_is("language/stdlib/time.lib.sigil")
            || self.source_file_is("language/stdlib/websocket.lib.sigil")
            || self.source_file_is_test_observe_module()
            || self.source_file.as_deref().is_some_and(|path| {
                path.ends_with("language/test/check/fsWatch.lib.sigil")
                    || path.ends_with("language/test/check/pty.lib.sigil")
                    || path.ends_with("language/test/check/websocket.lib.sigil")
            })
    }

    fn json_string_literal(&self, value: &str) -> Result<String, CodegenError> {
        serde_json::to_string(value).map_err(|error| {
            CodegenError::General(format!("Failed to JSON-encode string: {error}"))
        })
    }

    fn json_string_or_null(&self, value: Option<&str>) -> Result<String, CodegenError> {
        match value {
            Some(value) => self.json_string_literal(value),
            None => Ok("null".to_string()),
        }
    }

    fn qualify_named_type_id(&self, name: &str) -> Option<String> {
        if name.contains('.') {
            return Some(name.to_string());
        }
        if self.local_type_names.contains(name) {
            return self
                .module_id
                .as_ref()
                .map(|module_id| format!("{}.{}", module_id, name));
        }
        None
    }

    fn named_type_id_for_inference_type(&self, typ: &InferenceType) -> Option<String> {
        match typ {
            InferenceType::Constructor(constructor) => {
                self.qualify_named_type_id(&constructor.name)
            }
            _ => None,
        }
    }

    fn named_type_id_for_surface_type(&self, typ: &Type) -> Option<String> {
        match typ {
            Type::Constructor(constructor) => self.qualify_named_type_id(&constructor.name),
            Type::Variable(variable) => self.qualify_named_type_id(&variable.name),
            Type::Qualified(qualified) => Some(format!(
                "{}.{}",
                qualified.module_path.join("::"),
                qualified.type_name
            )),
            _ => None,
        }
    }

    fn span_id_for_expr(&self, kind: DebugSpanKind, location: SourceLocation) -> Option<&str> {
        self.span_map
            .as_ref()?
            .spans
            .iter()
            .find(|span| span.kind == kind && debug_span_matches_location(span, location))
            .map(|span| span.span_id.as_str())
    }

    fn span_id_for_match_arm(&self, location: SourceLocation) -> Option<&str> {
        self.span_map
            .as_ref()?
            .spans
            .iter()
            .find(|span| {
                span.kind == DebugSpanKind::MatchArm && debug_span_matches_location(span, location)
            })
            .map(|span| span.span_id.as_str())
    }

    fn span_kind_literal(&self, kind: DebugSpanKind) -> Result<String, CodegenError> {
        serde_json::to_string(&kind).map_err(|error| {
            CodegenError::General(format!("Failed to JSON-encode span kind: {error}"))
        })
    }

    fn trace_meta_literal(
        &self,
        span_id: Option<&str>,
        extra_fields: &[(&str, String)],
    ) -> Result<String, CodegenError> {
        let mut fields = vec![
            format!(
                "moduleId: {}",
                self.json_string_literal(self.module_id.as_deref().unwrap_or("<unknown>"))?
            ),
            format!(
                "sourceFile: {}",
                self.json_string_literal(self.source_file.as_deref().unwrap_or("<unknown>"))?
            ),
            format!(
                "spanId: {}",
                self.json_string_literal(span_id.unwrap_or(""))?
            ),
        ];
        if let Some(owner) = &self.current_trace_owner {
            fields.push(format!(
                "declarationKind: {}",
                self.json_string_literal(owner.declaration_kind)?
            ));
            fields.push(format!(
                "declarationLabel: {}",
                self.json_string_literal(&owner.declaration_label)?
            ));
        }
        for (name, value) in extra_fields {
            fields.push(format!("{name}: {value}"));
        }
        Ok(format!("{{ {} }}", fields.join(", ")))
    }

    fn wrap_declared_function_trace(
        &self,
        func_name: &str,
        span_id: Option<&str>,
        param_names_expr: &str,
        param_type_ids_expr: &str,
        args_expr: &str,
        body_expr: &str,
    ) -> Result<String, CodegenError> {
        if !self.trace_enabled {
            return Ok(body_expr.to_string());
        }
        let meta = self.trace_meta_literal(
            span_id,
            &[
                ("functionName", self.json_string_literal(func_name)?),
                (
                    "spanKind",
                    self.span_kind_literal(DebugSpanKind::FunctionDecl)?,
                ),
                (
                    "declarationKind",
                    self.json_string_literal("function_decl")?,
                ),
                ("declarationLabel", self.json_string_literal(func_name)?),
            ],
        )?;
        Ok(format!(
            "__sigil_debug_wrap_call({}, {}, {}, {}, () => {})",
            meta, param_names_expr, param_type_ids_expr, args_expr, body_expr
        ))
    }

    fn wrap_effect_trace(
        &self,
        span_id: Option<&str>,
        family: &str,
        operation: &str,
        args_expr: &str,
        body_expr: &str,
        target: Option<&str>,
    ) -> Result<String, CodegenError> {
        if !self.trace_enabled {
            return Ok(body_expr.to_string());
        }
        let mut extra_fields = vec![
            ("effectFamily", self.json_string_literal(family)?),
            ("operation", self.json_string_literal(operation)?),
        ];
        if let Some(target) = target {
            extra_fields.push(("target", self.json_string_literal(target)?));
        }
        let meta = self.trace_meta_literal(span_id, &extra_fields)?;
        Ok(format!(
            "__sigil_trace_wrap_effect({}, {}, () => {})",
            meta, args_expr, body_expr
        ))
    }

    fn trace_declared_return(
        &self,
        func_name: &str,
        param_names_expr: &str,
        param_type_ids_expr: &str,
        args_expr: &str,
        span_id: Option<&str>,
        body_expr: &str,
    ) -> Result<String, CodegenError> {
        let traced = self.wrap_declared_function_trace(
            func_name,
            span_id,
            param_names_expr,
            param_type_ids_expr,
            args_expr,
            body_expr,
        )?;
        Ok(traced)
    }

    fn wrap_expression_debug(
        &self,
        expr: &TypedExpr,
        kind: DebugSpanKind,
        body_expr: String,
        breakpoint_at_entry: bool,
    ) -> Result<String, CodegenError> {
        if !self.breakpoints_enabled && !self.expression_debug_enabled {
            return Ok(body_expr);
        }
        let span_id = self.span_id_for_expr(kind.clone(), expr.location);
        let Some(span_id) = span_id else {
            return Ok(body_expr);
        };
        let meta = self.trace_meta_literal(
            Some(span_id),
            &[("spanKind", self.span_kind_literal(kind)?)],
        )?;
        let type_id = self.named_type_id_for_inference_type(&expr.typ);
        let type_id_literal = self.json_string_or_null(type_id.as_deref())?;
        let options = if breakpoint_at_entry {
            "null".to_string()
        } else {
            "{ breakpointAtEntry: false }".to_string()
        };
        Ok(format!(
            "__sigil_debug_wrap_expression({}, {}, () => {}, {})",
            meta, type_id_literal, body_expr, options
        ))
    }

    fn pattern_binding_names(&self, pattern: &Pattern) -> Vec<String> {
        let mut names = Vec::new();
        self.collect_pattern_binding_names(pattern, &mut names);
        names
    }

    fn collect_pattern_binding_names(&self, pattern: &Pattern, names: &mut Vec<String>) {
        match pattern {
            Pattern::Identifier(id) => names.push(id.name.clone()),
            Pattern::Constructor(ctor) => {
                for pattern in &ctor.patterns {
                    self.collect_pattern_binding_names(pattern, names);
                }
            }
            Pattern::List(list) => {
                for pattern in &list.patterns {
                    self.collect_pattern_binding_names(pattern, names);
                }
                if let Some(rest) = &list.rest {
                    names.push(rest.clone());
                }
            }
            Pattern::Record(record) => {
                for field in &record.fields {
                    if let Some(pattern) = &field.pattern {
                        self.collect_pattern_binding_names(pattern, names);
                    } else {
                        names.push(field.name.clone());
                    }
                }
            }
            Pattern::Tuple(tuple) => {
                for pattern in &tuple.patterns {
                    self.collect_pattern_binding_names(pattern, names);
                }
            }
            Pattern::Literal(_) | Pattern::Wildcard(_) => {}
        }
    }

    fn pattern_scope_locals_expr(
        &self,
        pattern: &Pattern,
        origin: &str,
        type_id: Option<&str>,
    ) -> Result<String, CodegenError> {
        let locals = self
            .pattern_binding_names(pattern)
            .into_iter()
            .map(|name| {
                let mut fields = vec![
                    format!("name: {}", self.json_string_literal(&name)?),
                    format!("origin: {}", self.json_string_literal(origin)?),
                    format!("value: {}", sanitize_js_identifier(&name)),
                ];
                if matches!(pattern, Pattern::Identifier(_)) {
                    let type_id_literal = self.json_string_or_null(type_id)?;
                    fields.push(format!("typeId: {}", type_id_literal));
                }
                Ok(format!("{{ {} }}", fields.join(", ")))
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        Ok(format!("[{}]", locals.join(", ")))
    }

    fn emit_block(&mut self, block: &str) {
        for line in block.lines() {
            self.emit(line);
        }
    }

    fn js_ready(&self, expr: &str) -> String {
        format!("__sigil_ready({})", expr)
    }

    fn js_all(&self, exprs: &[String]) -> String {
        format!(
            "__sigil_all([{}])",
            exprs
                .iter()
                .map(|expr| format!("() => {}", expr))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    fn emit_runtime_helpers(
        &mut self,
        runtime_modules: &BTreeSet<String>,
        include_world_runtime: bool,
    ) {
        let include_process_runtime = self.requires_process_runtime(runtime_modules);
        let include_fswatch_runtime = self.requires_fswatch_runtime(runtime_modules);
        let include_pty_runtime = self.requires_pty_runtime(runtime_modules);
        let include_sql_runtime = self.requires_sql_runtime(runtime_modules);
        let include_websocket_runtime = self.requires_websocket_runtime(runtime_modules);
        let include_http_server_runtime = self.requires_http_server_runtime(runtime_modules);
        let include_cli_runtime = self.requires_cli_runtime(runtime_modules);
        let include_tcp_server_runtime = self.requires_tcp_server_runtime(runtime_modules);
        self.emit("function __sigil_ready(value) {");
        self.emit("  return Promise.resolve(value);");
        self.emit("}");
        self.emit("function __sigil_all(values) {");
        self.emit("  return values.reduce(async (__sigil_acc_promise, __sigil_thunk) => {");
        self.emit("    const __sigil_acc = await __sigil_acc_promise;");
        self.emit("    __sigil_acc.push(await __sigil_thunk());");
        self.emit("    return __sigil_acc;");
        self.emit("  }, Promise.resolve([]));");
        self.emit("}");
        self.emit("function __sigil_sleep(ms) {");
        self.emit("  return new Promise((resolve) => setTimeout(resolve, Math.max(0, ms)));");
        self.emit("}");
        self.emit("function __sigil_option_value(option) {");
        self.emit("  return option && option.__tag === 'Some' ? option.__fields[0] : null;");
        self.emit("}");
        if self.lazy_extern_namespaces {
            self.emit(
                "function __sigil_runtime_extern_reference_error(namespaceLabel, memberName) {",
            );
            self.emit("  const suffix = memberName == null ? '' : `.${String(memberName)}`;");
            self.emit(
                "  return new ReferenceError(`${String(namespaceLabel)}${suffix} is not defined`);",
            );
            self.emit("}");
            self.emit("function __sigil_runtime_extern_global(modulePath) {");
            self.emit("  if (modulePath.includes('/') || modulePath.includes(':')) {");
            self.emit("    return undefined;");
            self.emit("  }");
            self.emit("  return Object.prototype.hasOwnProperty.call(globalThis, modulePath)");
            self.emit("    ? globalThis[modulePath]");
            self.emit("    : undefined;");
            self.emit("}");
            self.emit("function __sigil_runtime_extern_namespace(namespaceLabel, modulePath) {");
            self.emit("  const globalModule = __sigil_runtime_extern_global(modulePath);");
            self.emit("  if (globalModule !== undefined) {");
            self.emit("    return globalModule;");
            self.emit("  }");
            self.emit("  let loadedModule = undefined;");
            self.emit("  let loadPromise = null;");
            self.emit("  function loadModule() {");
            self.emit("    if (loadedModule !== undefined) {");
            self.emit("      return Promise.resolve(loadedModule);");
            self.emit("    }");
            self.emit("    if (loadPromise) {");
            self.emit("      return loadPromise;");
            self.emit("    }");
            self.emit("    loadPromise = import(modulePath).then(");
            self.emit("      (resolvedModule) => {");
            self.emit("        loadedModule = resolvedModule ?? null;");
            self.emit("        return loadedModule;");
            self.emit("      },");
            self.emit("      () => {");
            self.emit(
                "        throw __sigil_runtime_extern_reference_error(namespaceLabel, null);",
            );
            self.emit("      }");
            self.emit("    );");
            self.emit("    return loadPromise;");
            self.emit("  }");
            self.emit("  return new Proxy(Object.create(null), {");
            self.emit("    get(_target, prop) {");
            self.emit("      if (prop === Symbol.toStringTag) {");
            self.emit("        return 'SigilExternNamespace';");
            self.emit("      }");
            self.emit("      if (prop === 'then') {");
            self.emit("        return undefined;");
            self.emit("      }");
            self.emit("      return function __sigil_runtime_extern_member(...args) {");
            self.emit("        return loadModule().then((resolvedModule) => {");
            self.emit("          const member = resolvedModule?.[prop];");
            self.emit("          if (member === undefined) {");
            self.emit("            throw __sigil_runtime_extern_reference_error(namespaceLabel, String(prop));");
            self.emit("          }");
            self.emit("          if (typeof member !== 'function') {");
            self.emit("            throw new TypeError(`${String(namespaceLabel)}.${String(prop)} is not a function`);");
            self.emit("          }");
            self.emit("          return member.apply(resolvedModule, args);");
            self.emit("        });");
            self.emit("      };");
            self.emit("    }");
            self.emit("  });");
            self.emit("}");
        }
        self.emit_block(COVERAGE_RUNTIME_HELPERS);
        self.emit_block(REPLAY_RUNTIME_HELPERS);
        if include_world_runtime {
            self.emit_block(WORLD_RUNTIME_HELPERS);
        }
        if include_fswatch_runtime {
            match self.fswatch_runtime_import_specifier.as_deref() {
                Some(specifier) => {
                    let specifier = self
                        .json_string_literal(specifier)
                        .unwrap_or_else(|_| "\"\"".to_string());
                    self.emit("globalThis.__sigil_fswatch_runtime = null;");
                    self.emit(&format!("globalThis.__sigil_load_fswatch_runtime = async () => {{ if (!globalThis.__sigil_fswatch_runtime) {{ globalThis.__sigil_fswatch_runtime = await import({specifier}); }} return globalThis.__sigil_fswatch_runtime; }};"));
                }
                None => {
                    self.emit("globalThis.__sigil_load_fswatch_runtime = async () => ({");
                    self.emit("  async watchPath() {");
                    self.emit("    throw new Error('§fsWatch runtime helper is unavailable');");
                    self.emit("  }");
                    self.emit("});");
                }
            }
        }
        if include_pty_runtime {
            match self.pty_runtime_import_specifier.as_deref() {
                Some(specifier) => {
                    let specifier = self
                        .json_string_literal(specifier)
                        .unwrap_or_else(|_| "\"\"".to_string());
                    self.emit("globalThis.__sigil_pty_runtime = null;");
                    self.emit(&format!("globalThis.__sigil_load_pty_runtime = async () => {{ if (!globalThis.__sigil_pty_runtime) {{ globalThis.__sigil_pty_runtime = await import({specifier}); }} return globalThis.__sigil_pty_runtime; }};"));
                }
                None => {
                    self.emit("globalThis.__sigil_load_pty_runtime = async () => ({");
                    self.emit("  async spawnPty() {");
                    self.emit("    throw new Error('§pty runtime helper is unavailable');");
                    self.emit("  }");
                    self.emit("});");
                }
            }
        }
        if include_sql_runtime {
            match self.sql_runtime_import_specifier.as_deref() {
                Some(specifier) => {
                    let specifier = self
                        .json_string_literal(specifier)
                        .unwrap_or_else(|_| "\"\"".to_string());
                    self.emit("globalThis.__sigil_sql_runtime = null;");
                    self.emit(&format!("globalThis.__sigil_load_sql_runtime = async () => {{ if (!globalThis.__sigil_sql_runtime) {{ globalThis.__sigil_sql_runtime = await import({specifier}); }} return globalThis.__sigil_sql_runtime; }};"));
                }
                None => {
                    self.emit("globalThis.__sigil_load_sql_runtime = async () => ({");
                    self.emit("  async connect() {");
                    self.emit("    throw new Error('§sql runtime helper is unavailable');");
                    self.emit("  }");
                    self.emit("});");
                }
            }
        }
        if include_websocket_runtime {
            match self.websocket_runtime_import_specifier.as_deref() {
                Some(specifier) => {
                    let specifier = self
                        .json_string_literal(specifier)
                        .unwrap_or_else(|_| "\"\"".to_string());
                    self.emit("globalThis.__sigil_websocket_runtime = null;");
                    self.emit(&format!("globalThis.__sigil_load_websocket_runtime = async () => {{ if (!globalThis.__sigil_websocket_runtime) {{ globalThis.__sigil_websocket_runtime = await import({specifier}); }} return globalThis.__sigil_websocket_runtime; }};"));
                }
                None => {
                    self.emit("globalThis.__sigil_load_websocket_runtime = async () => ({");
                    self.emit("  async listenServer() {");
                    self.emit("    throw new Error('§websocket runtime helper is unavailable');");
                    self.emit("  }");
                    self.emit("});");
                }
            }
        }
        self.emit("async function __sigil_map_list(items, fn) {");
        self.emit("  const results = [];");
        self.emit("  for (const item of items) {");
        self.emit("    results.push(await fn(item));");
        self.emit("  }");
        self.emit("  return results;");
        self.emit("}");
        self.emit("async function __sigil_filter_list(items, predicate) {");
        self.emit("  const results = [];");
        self.emit("  for (const item of items) {");
        self.emit("    if (await predicate(item)) {");
        self.emit("      results.push(item);");
        self.emit("    }");
        self.emit("  }");
        self.emit("  return results;");
        self.emit("}");
        self.emit("async function __sigil_concurrent_region(name, config, tasks) {");
        self.emit("  const concurrency = Math.max(1, Number(config.concurrency));");
        self.emit("  const jitter = __sigil_option_value(config.jitterMs);");
        self.emit("  const stopOn = config.stopOn;");
        self.emit("  const windowMs = __sigil_option_value(config.windowMs);");
        self.emit("  const outcomes = new Array(tasks.length);");
        self.emit("  const startTimes = [];");
        self.emit("  let nextIndex = 0;");
        self.emit("  let stopRequested = false;");
        self.emit("  function abortedOutcome() { return { __tag: 'Aborted', __fields: [] }; }");
        self.emit("  function failureOutcome(errorValue) { return { __tag: 'Failure', __fields: [errorValue] }; }");
        self.emit(
            "  function successOutcome(value) { return { __tag: 'Success', __fields: [value] }; }",
        );
        self.emit("  async function waitForWindowSlot() {");
        self.emit("    if (windowMs === null) return;");
        self.emit("    while (true) {");
        self.emit("      const now = Date.now();");
        self.emit("      while (startTimes.length > 0 && now - startTimes[0] >= windowMs) {");
        self.emit("        startTimes.shift();");
        self.emit("      }");
        self.emit("      if (startTimes.length < concurrency) return;");
        self.emit("      await __sigil_sleep(startTimes[0] + windowMs - now);");
        self.emit("    }");
        self.emit("  }");
        self.emit("  function jitterDelayMs() {");
        self.emit("    if (jitter === null) return 0;");
        self.emit("    const min = Number(jitter.min);");
        self.emit("    const max = Number(jitter.max);");
        self.emit("    if (!Number.isFinite(min) || !Number.isFinite(max)) return 0;");
        self.emit("    if (max <= min) return Math.max(0, min);");
        self.emit("    return Math.floor(Math.random() * (max - min + 1)) + min;");
        self.emit("  }");
        self.emit("  async function worker() {");
        self.emit("    while (true) {");
        self.emit("      const index = nextIndex;");
        self.emit("      if (index >= tasks.length) return;");
        self.emit("      nextIndex += 1;");
        self.emit("      if (stopRequested) {");
        self.emit("        outcomes[index] = abortedOutcome();");
        self.emit("        continue;");
        self.emit("      }");
        self.emit("      await waitForWindowSlot();");
        self.emit("      if (stopRequested) {");
        self.emit("        outcomes[index] = abortedOutcome();");
        self.emit("        continue;");
        self.emit("      }");
        self.emit("      const delay = jitterDelayMs();");
        self.emit("      if (delay > 0) {");
        self.emit("        await __sigil_sleep(delay);");
        self.emit("      }");
        self.emit("      startTimes.push(Date.now());");
        self.emit("      const result = await tasks[index]();");
        self.emit("      if (result && result.__tag === 'Ok') {");
        self.emit("        outcomes[index] = successOutcome(result.__fields[0]);");
        self.emit("        continue;");
        self.emit("      }");
        self.emit("      if (!result || result.__tag !== 'Err') {");
        self.emit("        throw new Error(`Concurrent region ${name} child returned a non-Result value`);");
        self.emit("      }");
        self.emit("      const errorValue = result.__fields[0];");
        self.emit("      outcomes[index] = failureOutcome(errorValue);");
        self.emit("      if (await stopOn(errorValue)) {");
        self.emit("        stopRequested = true;");
        self.emit("      }");
        self.emit("    }");
        self.emit("  }");
        self.emit("  await Promise.all(Array.from({ length: concurrency }, () => worker()));");
        self.emit("  return outcomes.map((outcome) => outcome ?? abortedOutcome());");
        self.emit("}");
        self.emit("function __sigil_map_empty() {");
        self.emit("  return { __sigil_map: [] };");
        self.emit("}");
        self.emit("function __sigil_map_from_entries(entries) {");
        self.emit("  let current = __sigil_map_empty();");
        self.emit("  for (const [key, value] of entries) { current = __sigil_map_insert(current, key, value); }");
        self.emit("  return current;");
        self.emit("}");
        self.emit("function __sigil_map_get(map, key) {");
        self.emit("  for (const [entryKey, entryValue] of map.__sigil_map) { if (__sigil_deep_equal(entryKey, key)) return { __tag: \"Some\", __fields: [entryValue] }; }");
        self.emit("  return { __tag: \"None\", __fields: [] };");
        self.emit("}");
        self.emit("function __sigil_map_has(map, key) {");
        self.emit("  for (const [entryKey] of map.__sigil_map) { if (__sigil_deep_equal(entryKey, key)) return true; }");
        self.emit("  return false;");
        self.emit("}");
        self.emit("function __sigil_map_insert(map, key, value) {");
        self.emit("  const next = [];");
        self.emit("  let replaced = false;");
        self.emit("  for (const [entryKey, entryValue] of map.__sigil_map) {");
        self.emit("    if (__sigil_deep_equal(entryKey, key)) { if (!replaced) { next.push([key, value]); replaced = true; } } else { next.push([entryKey, entryValue]); }");
        self.emit("  }");
        self.emit("  if (!replaced) next.push([key, value]);");
        self.emit("  return { __sigil_map: next };");
        self.emit("}");
        self.emit("function __sigil_map_remove(map, key) {");
        self.emit("  return { __sigil_map: map.__sigil_map.filter(([entryKey]) => !__sigil_deep_equal(entryKey, key)) };");
        self.emit("}");
        self.emit("function __sigil_map_entries(map) {");
        self.emit("  return map.__sigil_map.slice();");
        self.emit("}");
        self.emit("function __sigil_feature_flag_hash(value) {");
        self.emit("  let hash = 2166136261;");
        self.emit("  const text = String(value ?? '');");
        self.emit("  for (let index = 0; index < text.length; index += 1) {");
        self.emit("    hash ^= text.charCodeAt(index);");
        self.emit("    hash = Math.imul(hash, 16777619);");
        self.emit("  }");
        self.emit("  return hash >>> 0;");
        self.emit("}");
        self.emit("function __sigil_feature_flag_pick_rollout(values, hash) {");
        self.emit("  let total = 0;");
        self.emit("  const normalized = [];");
        self.emit("  for (const item of Array.isArray(values) ? values : []) {");
        self.emit("    const weight = Math.max(0, Math.trunc(Number(item?.weight ?? 0)));");
        self.emit("    if (weight <= 0) continue;");
        self.emit("    normalized.push({ value: item.value, weight });");
        self.emit("    total += weight;");
        self.emit("  }");
        self.emit("  if (total <= 0) return { found: false, value: null };");
        self.emit("  let slot = hash % total;");
        self.emit("  for (const item of normalized) {");
        self.emit("    if (slot < item.weight) return { found: true, value: item.value };");
        self.emit("    slot -= item.weight;");
        self.emit("  }");
        self.emit("  return { found: false, value: null };");
        self.emit("}");
        self.emit("function __sigil_feature_flag_error(message) {");
        self.emit("  const error = new Error(`Invalid feature flag config: ${message}`);");
        self.emit("  error.sigilCode = 'SIGIL-RUNTIME-FEATURE-FLAG';");
        self.emit("  throw error;");
        self.emit("}");
        self.emit("function __sigil_feature_flag_validate_rollout(rollout) {");
        self.emit("  if (!rollout || typeof rollout !== 'object') {");
        self.emit(
            "    __sigil_feature_flag_error('rollout action must carry a rollout plan record');",
        );
        self.emit("  }");
        self.emit("  const percentage = Math.trunc(Number(rollout.percentage ?? NaN));");
        self.emit("  if (!Number.isFinite(percentage) || percentage < 0 || percentage > 100) {");
        self.emit("    __sigil_feature_flag_error('rollout percentage must be an integer between 0 and 100');");
        self.emit("  }");
        self.emit("  const variants = Array.isArray(rollout.variants) ? rollout.variants : [];");
        self.emit("  if (variants.length === 0) {");
        self.emit("    __sigil_feature_flag_error('rollout variants must be non-empty');");
        self.emit("  }");
        self.emit("  let totalWeight = 0;");
        self.emit("  for (const item of variants) {");
        self.emit("    const weight = Math.trunc(Number(item?.weight ?? NaN));");
        self.emit("    if (!Number.isFinite(weight) || weight <= 0) {");
        self.emit("      __sigil_feature_flag_error('rollout variant weights must be positive integers');");
        self.emit("    }");
        self.emit("    totalWeight += weight;");
        self.emit("  }");
        self.emit("  if (totalWeight !== 100) {");
        self.emit("    __sigil_feature_flag_error('rollout variant weights must sum to 100');");
        self.emit("  }");
        self.emit("  return { percentage, variants };");
        self.emit("}");
        self.emit("async function __sigil_feature_flag_get(context, flag, set) {");
        self.emit("  let config = null;");
        self.emit("  for (const entry of Array.isArray(set) ? set : []) {");
        self.emit("    if (entry && entry.__sigil_feature_flag_id === flag?.id) {");
        self.emit("      config = entry.config ?? null;");
        self.emit("      break;");
        self.emit("    }");
        self.emit("  }");
        self.emit("  if (!config || typeof config !== 'object') return flag?.default;");
        self.emit("  const keyFn = __sigil_option_value(config.key);");
        self.emit("  let key = null;");
        self.emit("  if (typeof keyFn === 'function') {");
        self.emit("    const keyOption = await Promise.resolve(keyFn(context));");
        self.emit("    const keyValue = __sigil_option_value(keyOption);");
        self.emit("    if (keyValue !== null && keyValue !== undefined) {");
        self.emit("      key = String(keyValue);");
        self.emit("    }");
        self.emit("  }");
        self.emit("  for (const rule of Array.isArray(config.rules) ? config.rules : []) {");
        self.emit("    if (!await Promise.resolve(rule?.predicate?.(context))) continue;");
        self.emit("    const action = rule?.action ?? null;");
        self.emit("    if (action?.__tag === 'Value') return action.__fields?.[0];");
        self.emit("    if (action?.__tag === 'Rollout') {");
        self.emit("      if (key === null) {");
        self.emit("        __sigil_feature_flag_error(`flag ${String(flag?.id ?? '')} uses a rollout rule but no stable key was resolved`);");
        self.emit("      }");
        self.emit("      const rollout = __sigil_feature_flag_validate_rollout(action.__fields?.[0] ?? null);");
        self.emit("      if (rollout.percentage <= 0) return flag?.default;");
        self.emit("      const rolloutHash = __sigil_feature_flag_hash(`${String(flag?.id ?? '')}:${key}`);");
        self.emit("      if ((rolloutHash % 100) < rollout.percentage) {");
        self.emit("        const picked = __sigil_feature_flag_pick_rollout(rollout.variants, __sigil_feature_flag_hash(`variant:${String(flag?.id ?? '')}:${key}`));");
        self.emit("        if (picked.found) return picked.value;");
        self.emit("        __sigil_feature_flag_error(`flag ${String(flag?.id ?? '')} rollout variants could not pick a value`);");
        self.emit("      }");
        self.emit("      return flag?.default;");
        self.emit("    }");
        self.emit("    __sigil_feature_flag_error(`flag ${String(flag?.id ?? '')} rule action must be Value(...) or Rollout(...)`);");
        self.emit("  }");
        self.emit("  return flag?.default;");
        self.emit("}");
        self.emit("function __sigil_json_from_js(value) {");
        self.emit("  if (value === null) return { __tag: \"JsonNull\", __fields: [] };");
        self.emit("  if (Array.isArray(value)) return { __tag: \"JsonArray\", __fields: [value.map(__sigil_json_from_js)] };");
        self.emit(
            "  if (typeof value === 'boolean') return { __tag: \"JsonBool\", __fields: [value] };",
        );
        self.emit(
            "  if (typeof value === 'number') return { __tag: \"JsonNumber\", __fields: [value] };",
        );
        self.emit(
            "  if (typeof value === 'string') return { __tag: \"JsonString\", __fields: [value] };",
        );
        self.emit("  if (typeof value === 'object') {");
        self.emit("    return { __tag: \"JsonObject\", __fields: [__sigil_map_from_entries(Object.entries(value).map(([k, v]) => [k, __sigil_json_from_js(v)]))] };");
        self.emit("  }");
        self.emit("  return { __tag: \"JsonNull\", __fields: [] };");
        self.emit("}");
        self.emit("function __sigil_json_to_js(value) {");
        self.emit("  if (!value || typeof value !== 'object') return null;");
        self.emit("  switch (value.__tag) {");
        self.emit(
            "    case 'JsonArray': return (value.__fields[0] ?? []).map(__sigil_json_to_js);",
        );
        self.emit("    case 'JsonBool': return !!value.__fields[0];");
        self.emit("    case 'JsonNull': return null;");
        self.emit("    case 'JsonNumber': return Number(value.__fields[0]);");
        self.emit("    case 'JsonObject': {");
        self.emit("      const result = {};");
        self.emit("      for (const [k, v] of __sigil_map_entries(value.__fields[0] ?? __sigil_map_empty())) { result[String(k)] = __sigil_json_to_js(v); }");
        self.emit("      return result;");
        self.emit("    }");
        self.emit("    case 'JsonString': return String(value.__fields[0] ?? '');");
        self.emit("    default: return null;");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_json_parse_result(input) {");
        self.emit("  try {");
        self.emit(
            "    return { __tag: \"Ok\", __fields: [__sigil_json_from_js(JSON.parse(input))] };",
        );
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_json_stringify_value(value) {");
        self.emit("  return JSON.stringify(__sigil_json_to_js(value));");
        self.emit("}");
        self.emit("function __sigil_time_is_iso(input) {");
        self.emit("  return /^\\d{4}-\\d{2}-\\d{2}(?:T\\d{2}:\\d{2}:\\d{2}(?:\\.\\d{3})?(?:Z|[+-]\\d{2}:\\d{2}))?$/.test(input);");
        self.emit("}");
        self.emit("function __sigil_time_parse_iso_result(input) {");
        self.emit("  if (!__sigil_time_is_iso(input)) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: \"invalid ISO-8601 timestamp\" }] };");
        self.emit("  }");
        self.emit("  const millis = Date.parse(input);");
        self.emit("  if (Number.isNaN(millis)) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: \"invalid ISO-8601 timestamp\" }] };");
        self.emit("  }");
        self.emit("  return { __tag: \"Ok\", __fields: [{ epochMillis: millis }] };");
        self.emit("}");
        self.emit("function __sigil_time_format_iso(instant) {");
        self.emit("  return new Date(instant.epochMillis).toISOString();");
        self.emit("}");
        self.emit("function __sigil_time_now_instant() {");
        self.emit("  return { epochMillis: Date.now() };");
        self.emit("}");
        if include_process_runtime {
            self.emit("const __sigil_processes = new Map();");
            self.emit("function __sigil_process_env_to_object(envMap) {");
            self.emit("  const out = {};");
            self.emit(
                "  for (const [key, value] of __sigil_map_entries(envMap ?? __sigil_map_empty())) {",
            );
            self.emit("    out[String(key)] = String(value);");
            self.emit("  }");
            self.emit("  return out;");
            self.emit("}");
            self.emit("function __sigil_process_command_cwd(command) {");
            self.emit("  const cwd = command?.cwd;");
            self.emit("  return cwd && cwd.__tag === 'Some' ? cwd.__fields[0] : undefined;");
            self.emit("}");
            self.emit("function __sigil_process_result(code, stderr, stdout) {");
            self.emit("  return { code, stderr, stdout };");
            self.emit("}");
            self.emit("function __sigil_process_failure(result, extraMessage) {");
            self.emit("  const base = String(result?.stderr ?? '');");
            self.emit("  const suffix = extraMessage ? String(extraMessage) : '';");
            self.emit("  return {");
            self.emit("    code: Number(result?.code ?? -1),");
            self.emit("    stderr: base && suffix ? `${base}\\n${suffix}` : (base || suffix),");
            self.emit("    stdout: String(result?.stdout ?? '')");
            self.emit("  };");
            self.emit("}");
            self.emit("function __sigil_process_checked_result(result) {");
            self.emit("  return Number(result?.code ?? -1) === 0");
            self.emit("    ? { __tag: 'Ok', __fields: [result] }");
            self.emit("    : { __tag: 'Err', __fields: [__sigil_process_failure(result, '')] };");
            self.emit("}");
            self.emit("function __sigil_process_json_result(result) {");
            self.emit("  if (Number(result?.code ?? -1) !== 0) {");
            self.emit(
                "    return { __tag: 'Err', __fields: [__sigil_process_failure(result, '')] };",
            );
            self.emit("  }");
            self.emit("  const parsed = __sigil_json_parse_result(String(result?.stdout ?? ''));");
            self.emit("  if (parsed?.__tag === 'Ok') {");
            self.emit("    return parsed;");
            self.emit("  }");
            self.emit("  const message = String(parsed?.__fields?.[0]?.message ?? 'stdout was not valid JSON');");
            self.emit("  return { __tag: 'Err', __fields: [__sigil_process_failure(result, `stdout JSON parse failed: ${message}`)] };");
            self.emit("}");
            self.emit("async function __sigil_process_spawn(command) {");
            self.emit("  const { spawn } = await import('child_process');");
            self.emit("  const argv = Array.isArray(command?.argv) ? command.argv : [];");
            self.emit("  if (argv.length === 0) { return { pid: -1 }; }");
            self.emit("  const child = spawn(argv[0], argv.slice(1), {");
            self.emit("    cwd: __sigil_process_command_cwd(command),");
            self.emit(
                "    env: { ...process.env, ...__sigil_process_env_to_object(command?.env) },",
            );
            self.emit("    stdio: ['ignore', 'pipe', 'pipe'],");
            self.emit("  });");
            self.emit("  const pid = typeof child.pid === 'number' ? child.pid : Math.floor(Math.random() * 2147483647);");
            self.emit("  const state = { child, stdout: '', stderr: '', done: null };");
            self.emit("  if (child.stdout) { child.stdout.on('data', (chunk) => { state.stdout += String(chunk); }); }");
            self.emit("  if (child.stderr) { child.stderr.on('data', (chunk) => { state.stderr += String(chunk); }); }");
            self.emit("  const stdoutDone = child.stdout ? new Promise((resolve) => {");
            self.emit("    child.stdout.once('end', resolve);");
            self.emit("    child.stdout.once('close', resolve);");
            self.emit("  }) : Promise.resolve();");
            self.emit("  const stderrDone = child.stderr ? new Promise((resolve) => {");
            self.emit("    child.stderr.once('end', resolve);");
            self.emit("    child.stderr.once('close', resolve);");
            self.emit("  }) : Promise.resolve();");
            self.emit("  state.done = new Promise((resolve) => {");
            self.emit("    let settled = false;");
            self.emit("    let exitCode = -1;");
            self.emit("    const cleanup = () => {");
            self.emit("      try { child.stdout?.destroy(); } catch (_) {}");
            self.emit("      try { child.stderr?.destroy(); } catch (_) {}");
            self.emit("    };");
            self.emit("    const finish = (code) => {");
            self.emit("      if (settled) { return; }");
            self.emit("      settled = true;");
            self.emit("      cleanup();");
            self.emit("      resolve(__sigil_process_result(typeof code === 'number' ? code : exitCode, state.stderr, state.stdout));");
            self.emit("    };");
            self.emit("    child.once('error', (error) => {");
            self.emit("      if (settled) { return; }");
            self.emit("      settled = true;");
            self.emit("      cleanup();");
            self.emit("      resolve(__sigil_process_result(-1, state.stderr + String(error?.message ?? error), state.stdout));");
            self.emit("    });");
            self.emit("    child.once('exit', (code) => {");
            self.emit("      exitCode = typeof code === 'number' ? code : -1;");
            self.emit("      Promise.race([");
            self.emit("        Promise.all([stdoutDone, stderrDone]),");
            self.emit("        __sigil_sleep(50)");
            self.emit("      ]).then(() => finish(exitCode));");
            self.emit("    });");
            self.emit("    child.once('close', (code) => {");
            self.emit("      finish(typeof code === 'number' ? code : exitCode);");
            self.emit("    });");
            self.emit("  });");
            self.emit("  __sigil_processes.set(pid, state);");
            self.emit("  return { pid };");
            self.emit("}");
            self.emit("async function __sigil_process_wait(processHandle) {");
            self.emit("  const pid = Number(processHandle?.pid ?? -1);");
            self.emit("  const state = __sigil_processes.get(pid);");
            self.emit("  if (!state) {");
            self.emit("    return __sigil_process_result(-1, 'unknown process', '');");
            self.emit("  }");
            self.emit("  const result = await state.done;");
            self.emit("  __sigil_processes.delete(pid);");
            self.emit("  return result;");
            self.emit("}");
            self.emit("async function __sigil_process_kill(processHandle) {");
            self.emit("  const pid = Number(processHandle?.pid ?? -1);");
            self.emit("  const state = __sigil_processes.get(pid);");
            self.emit("  if (state) {");
            self.emit("    try { state.child.kill(); } catch (_) {}");
            self.emit("  }");
            self.emit("  return null;");
            self.emit("}");
            self.emit("async function __sigil_process_run(command) {");
            self.emit("  const handle = await __sigil_process_spawn(command);");
            self.emit("  return __sigil_process_wait(handle);");
            self.emit("}");
            self.emit("async function __sigil_process_argv() {");
            self.emit("  return process.argv.slice(2);");
            self.emit("}");
            self.emit("async function __sigil_process_exit(code) {");
            self.emit("  process.exit(Number(code));");
            self.emit("  return null;");
            self.emit("}");
        }
        if include_cli_runtime {
            self.emit("function __sigil_cli_none() {");
            self.emit("  return { __tag: 'None', __fields: [] };");
            self.emit("}");
            self.emit("function __sigil_cli_some(value) {");
            self.emit("  return { __tag: 'Some', __fields: [value] };");
            self.emit("}");
            self.emit("function __sigil_cli_arg(kind, fields) {");
            self.emit("  return { kind, ...fields };");
            self.emit("}");
            self.emit("function __sigil_cli_command(kind, name, description, args, build) {");
            self.emit("  return { args, build, description: String(description), kind, name: name == null ? null : String(name) };");
            self.emit("}");
            self.emit("function __sigil_cli_program(name, description, root, subcommands) {");
            self.emit("  return { description: String(description), name: String(name), root: root ?? null, subcommands: Array.isArray(subcommands) ? subcommands : [] };");
            self.emit("}");
            self.emit("function __sigil_cli_eprintln(message) {");
            self.emit("  console.error(String(message ?? ''));");
            self.emit("  return null;");
            self.emit("}");
            self.emit("function __sigil_cli_println(message) {");
            self.emit("  console.log(String(message ?? ''));");
            self.emit("  return null;");
            self.emit("}");
            self.emit("function __sigil_cli_usage_piece(arg) {");
            self.emit("  switch (String(arg?.kind ?? '')) {");
            self.emit("    case 'flag': return `[--${String(arg.long)}]`;");
            self.emit(
                "    case 'option': return `[--${String(arg.long)} ${String(arg.valueName)}]`;",
            );
            self.emit("    case 'requiredOption': return `--${String(arg.long)} ${String(arg.valueName)}`;");
            self.emit("    case 'manyOption': return `[--${String(arg.long)} ${String(arg.valueName)} ...]`;");
            self.emit("    case 'positional': return String(arg.name);");
            self.emit("    case 'optionalPositional': return `[${String(arg.name)}]`;");
            self.emit("    case 'manyPositionals': return `[${String(arg.name)} ...]`;");
            self.emit("    default: return '';");
            self.emit("  }");
            self.emit("}");
            self.emit("function __sigil_cli_usage(program, command) {");
            self.emit("  const base = command?.kind === 'command' ? `${program.name} ${String(command?.name ?? '')}` : program.name;");
            self.emit("  const pieces = (Array.isArray(command?.args) ? command.args : []).map(__sigil_cli_usage_piece).filter((piece) => piece.length > 0);");
            self.emit("  return pieces.length === 0 ? base : `${base} ${pieces.join(' ')}`;");
            self.emit("}");
            self.emit("function __sigil_cli_argument_label(arg) {");
            self.emit("  switch (String(arg?.kind ?? '')) {");
            self.emit("    case 'flag': return arg.short?.__tag === 'Some' ? `--${String(arg.long)}, -${String(arg.short.__fields[0])}` : `--${String(arg.long)}`;");
            self.emit("    case 'option':");
            self.emit("    case 'requiredOption':");
            self.emit("    case 'manyOption': {");
            self.emit("      const valueName = String(arg.valueName ?? 'VALUE');");
            self.emit("      return arg.short?.__tag === 'Some' ? `--${String(arg.long)}, -${String(arg.short.__fields[0])} ${valueName}` : `--${String(arg.long)} ${valueName}`;");
            self.emit("    }");
            self.emit("    case 'positional': return String(arg.name);");
            self.emit("    case 'optionalPositional': return `[${String(arg.name)}]`;");
            self.emit("    case 'manyPositionals': return `[${String(arg.name)} ...]`;");
            self.emit("    default: return String(arg?.name ?? arg?.long ?? '');");
            self.emit("  }");
            self.emit("}");
            self.emit("function __sigil_cli_help(program, command) {");
            self.emit("  const lines = [];");
            self.emit("  const summary = String((command?.kind === 'root' ? program?.description : command?.description) ?? program?.description ?? command?.description ?? '').trim();");
            self.emit("  if (summary.length > 0) {");
            self.emit("    lines.push(summary);");
            self.emit("    lines.push('');");
            self.emit("  }");
            self.emit("  lines.push(`Usage: ${__sigil_cli_usage(program, command)}`);");
            self.emit("  if (Array.isArray(program?.subcommands) && program.subcommands.length > 0 && command?.kind !== 'command') {");
            self.emit("    lines.push(`       ${program.name} <subcommand> ...`);");
            self.emit("  }");
            self.emit("  const args = Array.isArray(command?.args) ? command.args : [];");
            self.emit("  if (Array.isArray(program?.subcommands) && program.subcommands.length > 0 && command?.kind !== 'command') {");
            self.emit("    lines.push('');");
            self.emit("    lines.push('Commands:');");
            self.emit("    for (const subcommand of program.subcommands) {");
            self.emit("      lines.push(`  ${String(subcommand.name)}  ${String(subcommand.description ?? '')}`);");
            self.emit("    }");
            self.emit("  }");
            self.emit("  if (args.length > 0) {");
            self.emit("    lines.push('');");
            self.emit("    lines.push('Arguments:');");
            self.emit("    for (const arg of args) {");
            self.emit("      lines.push(`  ${__sigil_cli_argument_label(arg)}  ${String(arg.description ?? '')}`);");
            self.emit("    }");
            self.emit("  }");
            self.emit("  return lines.join('\\n');");
            self.emit("}");
            self.emit("function __sigil_cli_fail(message, command, program) {");
            self.emit("  return { kind: 'error', message: String(message), text: __sigil_cli_help(program, command) };");
            self.emit("}");
            self.emit("function __sigil_cli_find_command(program, argv) {");
            self.emit("  const commands = Array.isArray(program?.subcommands) ? program.subcommands : [];");
            self.emit(
                "  const first = Array.isArray(argv) && argv.length > 0 ? String(argv[0]) : null;",
            );
            self.emit("  const command = first == null ? null : commands.find((entry) => String(entry?.name ?? '') === first) ?? null;");
            self.emit("  if (command) {");
            self.emit("    return { command, argv: argv.slice(1) };");
            self.emit("  }");
            self.emit("  if (program?.root) {");
            self.emit("    return { command: program.root, argv };");
            self.emit("  }");
            self.emit("  return { command: null, argv };");
            self.emit("}");
            self.emit("function __sigil_cli_parse(command, argv, program) {");
            self.emit("  if (!command) {");
            self.emit("    return __sigil_cli_fail(`unknown command '${String(argv?.[0] ?? '')}'`, program, program);");
            self.emit("  }");
            self.emit("  const args = Array.isArray(command.args) ? command.args : [];");
            self.emit("  const positionals = [];");
            self.emit("  const optionStates = new Map();");
            self.emit("  const longOptions = new Map();");
            self.emit("  const shortOptions = new Map();");
            self.emit("  for (let index = 0; index < args.length; index += 1) {");
            self.emit("    const arg = args[index];");
            self.emit("    switch (String(arg?.kind ?? '')) {");
            self.emit("      case 'flag':");
            self.emit("        optionStates.set(index, false);");
            self.emit("        longOptions.set(String(arg.long), { arg, index });");
            self.emit("        if (arg.short?.__tag === 'Some') shortOptions.set(String(arg.short.__fields[0]), { arg, index });");
            self.emit("        break;");
            self.emit("      case 'option':");
            self.emit("        optionStates.set(index, __sigil_cli_none());");
            self.emit("        longOptions.set(String(arg.long), { arg, index });");
            self.emit("        if (arg.short?.__tag === 'Some') shortOptions.set(String(arg.short.__fields[0]), { arg, index });");
            self.emit("        break;");
            self.emit("      case 'requiredOption':");
            self.emit("        optionStates.set(index, null);");
            self.emit("        longOptions.set(String(arg.long), { arg, index });");
            self.emit("        if (arg.short?.__tag === 'Some') shortOptions.set(String(arg.short.__fields[0]), { arg, index });");
            self.emit("        break;");
            self.emit("      case 'manyOption':");
            self.emit("        optionStates.set(index, []);");
            self.emit("        longOptions.set(String(arg.long), { arg, index });");
            self.emit("        if (arg.short?.__tag === 'Some') shortOptions.set(String(arg.short.__fields[0]), { arg, index });");
            self.emit("        break;");
            self.emit("      case 'positional':");
            self.emit("      case 'optionalPositional':");
            self.emit("      case 'manyPositionals':");
            self.emit("        positionals.push({ arg, index });");
            self.emit("        break;");
            self.emit("      default:");
            self.emit("        break;");
            self.emit("    }");
            self.emit("  }");
            self.emit("  for (let index = 0; index < positionals.length; index += 1) {");
            self.emit("    if (String(positionals[index].arg?.kind ?? '') === 'manyPositionals' && index !== positionals.length - 1) {");
            self.emit("      throw new Error('§cli manyPositionals(...) must be the final positional argument');");
            self.emit("    }");
            self.emit("  }");
            self.emit("  const positionalTokens = [];");
            self.emit("  let optionParsing = true;");
            self.emit("  for (let index = 0; index < argv.length; index += 1) {");
            self.emit("    const token = String(argv[index]);");
            self.emit("    if (optionParsing && (token === '--help' || token === '-h')) {");
            self.emit("      return { kind: 'help', text: __sigil_cli_help(program, command) };");
            self.emit("    }");
            self.emit("    if (optionParsing && token === '--') {");
            self.emit("      optionParsing = false;");
            self.emit("      continue;");
            self.emit("    }");
            self.emit("    if (optionParsing && token.startsWith('--') && token.length > 2) {");
            self.emit("      const eq = token.indexOf('=');");
            self.emit("      const key = eq >= 0 ? token.slice(2, eq) : token.slice(2);");
            self.emit("      const attached = eq >= 0 ? token.slice(eq + 1) : null;");
            self.emit("      const entry = longOptions.get(key) ?? null;");
            self.emit("      if (!entry) {");
            self.emit(
                "        return __sigil_cli_fail(`unknown option '--${key}'`, command, program);",
            );
            self.emit("      }");
            self.emit("      const kind = String(entry.arg?.kind ?? '');");
            self.emit("      if (kind === 'flag') {");
            self.emit("        if (attached !== null) {");
            self.emit("          return __sigil_cli_fail(`flag '--${key}' does not take a value`, command, program);");
            self.emit("        }");
            self.emit("        if (optionStates.get(entry.index) === true) {");
            self.emit("          return __sigil_cli_fail(`option '--${key}' may only appear once`, command, program);");
            self.emit("        }");
            self.emit("        optionStates.set(entry.index, true);");
            self.emit("        continue;");
            self.emit("      }");
            self.emit("      const value = attached !== null ? attached : (index + 1 < argv.length ? String(argv[++index]) : null);");
            self.emit("      if (value === null) {");
            self.emit("        return __sigil_cli_fail(`expected value after '--${key}'`, command, program);");
            self.emit("      }");
            self.emit("      if (kind === 'option') {");
            self.emit("        if (optionStates.get(entry.index)?.__tag === 'Some') {");
            self.emit("          return __sigil_cli_fail(`option '--${key}' may only appear once`, command, program);");
            self.emit("        }");
            self.emit("        optionStates.set(entry.index, __sigil_cli_some(value));");
            self.emit("        continue;");
            self.emit("      }");
            self.emit("      if (kind === 'requiredOption') {");
            self.emit("        if (typeof optionStates.get(entry.index) === 'string') {");
            self.emit("          return __sigil_cli_fail(`option '--${key}' may only appear once`, command, program);");
            self.emit("        }");
            self.emit("        optionStates.set(entry.index, value);");
            self.emit("        continue;");
            self.emit("      }");
            self.emit("      if (kind === 'manyOption') {");
            self.emit("        optionStates.get(entry.index).push(value);");
            self.emit("        continue;");
            self.emit("      }");
            self.emit(
                "      return __sigil_cli_fail(`unexpected option '--${key}'`, command, program);",
            );
            self.emit("    }");
            self.emit("    if (optionParsing && token.startsWith('-') && token.length > 1) {");
            self.emit("      if (token.length !== 2) {");
            self.emit(
                "        return __sigil_cli_fail(`unknown option '${token}'`, command, program);",
            );
            self.emit("      }");
            self.emit("      const key = token.slice(1);");
            self.emit("      const entry = shortOptions.get(key) ?? null;");
            self.emit("      if (!entry) {");
            self.emit(
                "        return __sigil_cli_fail(`unknown option '-${key}'`, command, program);",
            );
            self.emit("      }");
            self.emit("      const longKey = String(entry.arg?.long ?? key);");
            self.emit("      const kind = String(entry.arg?.kind ?? '');");
            self.emit("      if (kind === 'flag') {");
            self.emit("        if (optionStates.get(entry.index) === true) {");
            self.emit("          return __sigil_cli_fail(`option '-${key}' may only appear once`, command, program);");
            self.emit("        }");
            self.emit("        optionStates.set(entry.index, true);");
            self.emit("        continue;");
            self.emit("      }");
            self.emit(
                "      const value = index + 1 < argv.length ? String(argv[++index]) : null;",
            );
            self.emit("      if (value === null) {");
            self.emit("        return __sigil_cli_fail(`expected value after '-${key}'`, command, program);");
            self.emit("      }");
            self.emit("      if (kind === 'option') {");
            self.emit("        if (optionStates.get(entry.index)?.__tag === 'Some') {");
            self.emit("          return __sigil_cli_fail(`option '--${longKey}' may only appear once`, command, program);");
            self.emit("        }");
            self.emit("        optionStates.set(entry.index, __sigil_cli_some(value));");
            self.emit("        continue;");
            self.emit("      }");
            self.emit("      if (kind === 'requiredOption') {");
            self.emit("        if (typeof optionStates.get(entry.index) === 'string') {");
            self.emit("          return __sigil_cli_fail(`option '--${longKey}' may only appear once`, command, program);");
            self.emit("        }");
            self.emit("        optionStates.set(entry.index, value);");
            self.emit("        continue;");
            self.emit("      }");
            self.emit("      if (kind === 'manyOption') {");
            self.emit("        optionStates.get(entry.index).push(value);");
            self.emit("        continue;");
            self.emit("      }");
            self.emit("    }");
            self.emit("    positionalTokens.push(token);");
            self.emit("  }");
            self.emit("  const positionalValues = new Map();");
            self.emit("  let cursor = 0;");
            self.emit("  for (const entry of positionals) {");
            self.emit("    const kind = String(entry.arg?.kind ?? '');");
            self.emit("    if (kind === 'positional') {");
            self.emit("      if (cursor >= positionalTokens.length) {");
            self.emit("        return __sigil_cli_fail(`missing required argument '${String(entry.arg?.name ?? '')}'`, command, program);");
            self.emit("      }");
            self.emit("      positionalValues.set(entry.index, positionalTokens[cursor]);");
            self.emit("      cursor += 1;");
            self.emit("      continue;");
            self.emit("    }");
            self.emit("    if (kind === 'optionalPositional') {");
            self.emit("      if (cursor < positionalTokens.length) {");
            self.emit("        positionalValues.set(entry.index, __sigil_cli_some(positionalTokens[cursor]));");
            self.emit("        cursor += 1;");
            self.emit("      } else {");
            self.emit("        positionalValues.set(entry.index, __sigil_cli_none());");
            self.emit("      }");
            self.emit("      continue;");
            self.emit("    }");
            self.emit("    if (kind === 'manyPositionals') {");
            self.emit("      positionalValues.set(entry.index, positionalTokens.slice(cursor));");
            self.emit("      cursor = positionalTokens.length;");
            self.emit("    }");
            self.emit("  }");
            self.emit("  if (cursor < positionalTokens.length) {");
            self.emit("    return __sigil_cli_fail(`unexpected argument '${String(positionalTokens[cursor])}'`, command, program);");
            self.emit("  }");
            self.emit("  const values = [];");
            self.emit("  for (let index = 0; index < args.length; index += 1) {");
            self.emit("    const arg = args[index];");
            self.emit("    switch (String(arg?.kind ?? '')) {");
            self.emit("      case 'flag':");
            self.emit("      case 'option':");
            self.emit("      case 'manyOption':");
            self.emit("        values.push(optionStates.get(index));");
            self.emit("        break;");
            self.emit("      case 'requiredOption': {");
            self.emit("        const value = optionStates.get(index);");
            self.emit("        if (typeof value !== 'string') {");
            self.emit("          return __sigil_cli_fail(`missing required option '--${String(arg.long)}'`, command, program);");
            self.emit("        }");
            self.emit("        values.push(value);");
            self.emit("        break;");
            self.emit("      }");
            self.emit("      case 'positional':");
            self.emit("      case 'optionalPositional':");
            self.emit("      case 'manyPositionals':");
            self.emit("        values.push(positionalValues.get(index));");
            self.emit("        break;");
            self.emit("      default:");
            self.emit("        values.push(null);");
            self.emit("        break;");
            self.emit("    }");
            self.emit("  }");
            self.emit("  return { kind: 'ok', value: values };");
            self.emit("}");
            self.emit("async function __sigil_cli_run(argv, program) {");
            self.emit("  const selected = __sigil_cli_find_command(program, Array.isArray(argv) ? argv : []);");
            self.emit("  const command = selected.command;");
            self.emit("  if (!command && Array.isArray(program?.subcommands) && program.subcommands.length > 0) {");
            self.emit("    const text = __sigil_cli_help(program, program);");
            self.emit("    await __sigil_cli_eprintln(`error: unknown command '${String(selected.argv?.[0] ?? '')}'`);");
            self.emit("    await __sigil_cli_eprintln(text);");
            self.emit("    await __sigil_world_process_exit(2);");
            self.emit("    return null;");
            self.emit("  }");
            self.emit("  const parsed = __sigil_cli_parse(command, selected.argv, program);");
            self.emit("  if (parsed.kind === 'help') {");
            self.emit("    await __sigil_cli_println(parsed.text);");
            self.emit("    await __sigil_world_process_exit(0);");
            self.emit("    return null;");
            self.emit("  }");
            self.emit("  if (parsed.kind === 'error') {");
            self.emit("    await __sigil_cli_eprintln(`error: ${parsed.message}`);");
            self.emit("    await __sigil_cli_eprintln(parsed.text);");
            self.emit("    await __sigil_world_process_exit(2);");
            self.emit("    return null;");
            self.emit("  }");
            self.emit("  return await command.build(...parsed.value);");
            self.emit("}");
        }
        self.emit("function __sigil_regex_compile_result(flags, pattern) {");
        self.emit("  try {");
        self.emit("    const normalizedFlags = String(flags ?? '');");
        self.emit("    const normalizedPattern = String(pattern ?? '');");
        self.emit("    new RegExp(normalizedPattern, normalizedFlags);");
        self.emit("    return { __tag: \"Ok\", __fields: [{ flags: normalizedFlags, pattern: normalizedPattern }] };");
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_regex_find(regex, input) {");
        self.emit("  try {");
        self.emit("    const compiled = new RegExp(String(regex?.pattern ?? ''), String(regex?.flags ?? ''));");
        self.emit("    const source = String(input ?? '');");
        self.emit("    const match = compiled.exec(source);");
        self.emit("    if (!match) { return { __tag: \"None\", __fields: [] }; }");
        self.emit("    return { __tag: \"Some\", __fields: [{ captures: match.slice(1).map((value) => value ?? ''), end: match.index + match[0].length, full: match[0], start: match.index }] };");
        self.emit("  } catch (_) {");
        self.emit("    return { __tag: \"None\", __fields: [] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_regex_is_match(regex, input) {");
        self.emit("  try {");
        self.emit("    return new RegExp(String(regex?.pattern ?? ''), String(regex?.flags ?? '')).test(String(input ?? ''));");
        self.emit("  } catch (_) {");
        self.emit("    return false;");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_regex_find_all(regex, input) {");
        self.emit("  try {");
        self.emit("    const flags = String(regex?.flags ?? '');");
        self.emit("    const withGlobal = flags.includes('g') ? flags : flags + 'g';");
        self.emit("    const compiled = new RegExp(String(regex?.pattern ?? ''), withGlobal);");
        self.emit("    const source = String(input ?? '');");
        self.emit("    const results = [];");
        self.emit("    let match;");
        self.emit("    const isUnicode = withGlobal.includes('u') || withGlobal.includes('v');");
        self.emit("    while ((match = compiled.exec(source)) !== null) {");
        self.emit("      results.push({ captures: match.slice(1).map((v) => v ?? ''), end: match.index + match[0].length, full: match[0], start: match.index });");
        self.emit("      if (match[0].length === 0) {");
        self.emit(
            "        const cp = isUnicode ? (source.codePointAt(compiled.lastIndex) ?? -1) : -1;",
        );
        self.emit("        compiled.lastIndex += (cp > 0xFFFF) ? 2 : 1;");
        self.emit("      }");
        self.emit("    }");
        self.emit("    return results;");
        self.emit("  } catch (_) {");
        self.emit("    return [];");
        self.emit("  }");
        self.emit("}");
        self.emit("async function __sigil_crypto_sha256(input) {");
        self.emit("  const { createHash } = await import('node:crypto');");
        self.emit("  return createHash('sha256').update(String(input ?? '')).digest('hex');");
        self.emit("}");
        self.emit("async function __sigil_crypto_hmac_sha256(key, message) {");
        self.emit("  const { createHmac } = await import('node:crypto');");
        self.emit("  return createHmac('sha256', String(key ?? '')).update(String(message ?? '')).digest('hex');");
        self.emit("}");
        self.emit("function __sigil_crypto_base64_encode(input) {");
        self.emit("  return Buffer.from(String(input ?? ''), 'utf8').toString('base64');");
        self.emit("}");
        self.emit("function __sigil_crypto_base64_decode(input) {");
        self.emit("  try {");
        self.emit("    const s = String(input ?? '');");
        self.emit("    if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=|[A-Za-z0-9+/]{4}|)$/.test(s)) {");
        self.emit(
            "      return { __tag: \"Err\", __fields: [{ message: \"invalid base64 input\" }] };",
        );
        self.emit("    }");
        self.emit(
            "    return { __tag: \"Ok\", __fields: [Buffer.from(s, 'base64').toString('utf8')] };",
        );
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_crypto_hex_encode(input) {");
        self.emit("  return Buffer.from(String(input ?? ''), 'utf8').toString('hex');");
        self.emit("}");
        self.emit("function __sigil_crypto_hex_decode(input) {");
        self.emit("  try {");
        self.emit("    const s = String(input ?? '');");
        self.emit("    if (s.length % 2 !== 0) { return { __tag: \"Err\", __fields: [{ message: \"invalid hex: odd length\" }] }; }");
        self.emit("    if (s.length > 0 && !/^[0-9A-Fa-f]+$/.test(s)) { return { __tag: \"Err\", __fields: [{ message: \"invalid hex: non-hex digits\" }] }; }");
        self.emit(
            "    return { __tag: \"Ok\", __fields: [Buffer.from(s, 'hex').toString('utf8')] };",
        );
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_url_query_map_from_search(search) {");
        self.emit("  const params = new URLSearchParams(search);");
        self.emit("  return __sigil_map_from_entries(Array.from(params.entries()));");
        self.emit("}");
        self.emit("function __sigil_url_from_absolute(absolute) {");
        self.emit("  const protocol = absolute.protocol.endsWith(':') ? absolute.protocol.slice(0, -1) : absolute.protocol;");
        self.emit("  const port = absolute.port.length > 0 ? { __tag: \"Some\", __fields: [Number(absolute.port)] } : { __tag: \"None\", __fields: [] };");
        self.emit("  return {");
        self.emit("    fragment: absolute.hash || '',");
        self.emit("    host: absolute.hostname || '',");
        self.emit("    path: absolute.pathname || '',");
        self.emit("    port,");
        self.emit("    protocol,");
        self.emit("    query: __sigil_url_query_map_from_search(absolute.search || ''),");
        self.emit("    query_string: absolute.search || ''");
        self.emit("  };");
        self.emit("}");
        self.emit("function __sigil_url_from_relative(input) {");
        self.emit("  const fragmentIndex = input.indexOf('#');");
        self.emit("  const fragment = fragmentIndex >= 0 ? input.slice(fragmentIndex) : '';");
        self.emit(
            "  const withoutFragment = fragmentIndex >= 0 ? input.slice(0, fragmentIndex) : input;",
        );
        self.emit("  const queryIndex = withoutFragment.indexOf('?');");
        self.emit("  const path = queryIndex >= 0 ? withoutFragment.slice(0, queryIndex) : withoutFragment;");
        self.emit(
            "  const queryString = queryIndex >= 0 ? withoutFragment.slice(queryIndex) : '';",
        );
        self.emit("  return {");
        self.emit("    fragment,");
        self.emit("    host: '',");
        self.emit("    path,");
        self.emit("    port: { __tag: \"None\", __fields: [] },");
        self.emit("    protocol: '',");
        self.emit("    query: __sigil_url_query_map_from_search(queryString),");
        self.emit("    query_string: queryString");
        self.emit("  };");
        self.emit("}");
        self.emit("function __sigil_url_parse_result(input) {");
        self.emit("  try {");
        self.emit("    const absolutePattern = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;");
        self.emit("    if (absolutePattern.test(input)) {");
        self.emit("      return { __tag: \"Ok\", __fields: [__sigil_url_from_absolute(new URL(input))] };");
        self.emit("    }");
        self.emit("    return { __tag: \"Ok\", __fields: [__sigil_url_from_relative(input)] };");
        self.emit("  } catch (error) {");
        self.emit("    return { __tag: \"Err\", __fields: [{ message: error instanceof Error ? error.message : String(error) }] };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_http_error(kind, message) {");
        self.emit("  return { kind: { __tag: kind, __fields: [] }, message: String(message) };");
        self.emit("}");
        self.emit("function __sigil_http_headers_from_entries(entries) {");
        self.emit("  return __sigil_map_from_entries(entries.map(([key, value]) => [String(key).toLowerCase(), String(value)]));");
        self.emit("}");
        self.emit("function __sigil_http_header_value(value) {");
        self.emit(
            "  if (Array.isArray(value)) return value.map((item) => String(item)).join(', ');",
        );
        self.emit("  if (value === undefined || value === null) return null;");
        self.emit("  return String(value);");
        self.emit("}");
        self.emit("function __sigil_http_headers_from_node(headers) {");
        self.emit("  return __sigil_http_headers_from_entries(Object.entries(headers ?? {}).flatMap(([key, value]) => {");
        self.emit("    const normalized = __sigil_http_header_value(value);");
        self.emit("    return normalized === null ? [] : [[key, normalized]];");
        self.emit("  }));");
        self.emit("}");
        self.emit("function __sigil_http_headers_from_web(headers) {");
        self.emit("  return __sigil_http_headers_from_entries(Array.from(headers.entries()));");
        self.emit("}");
        self.emit("function __sigil_http_headers_to_js(headers) {");
        self.emit("  const result = {};");
        self.emit("  for (const [key, value] of __sigil_map_entries(headers ?? __sigil_map_empty())) { result[String(key)] = String(value); }");
        self.emit("  return result;");
        self.emit("}");
        self.emit("function __sigil_http_method_to_string(method) {");
        self.emit("  switch (method?.__tag) {");
        self.emit("    case 'Delete': return 'DELETE';");
        self.emit("    case 'Get': return 'GET';");
        self.emit("    case 'Patch': return 'PATCH';");
        self.emit("    case 'Post': return 'POST';");
        self.emit("    case 'Put': return 'PUT';");
        self.emit("    default: return 'GET';");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_http_request_path(url) {");
        self.emit("  try {");
        self.emit("    const parsed = new URL(String(url ?? '/'), 'http://127.0.0.1');");
        self.emit("    return parsed.pathname || '/';");
        self.emit("  } catch (_) {");
        self.emit("    return '/';");
        self.emit("  }");
        self.emit("}");
        if include_http_server_runtime {
            self.emit("function __sigil_http_body_error(message) {");
            self.emit("  return { message: String(message) };");
            self.emit("}");
            self.emit("function __sigil_http_json_body_result(request) {");
            self.emit("  try {");
            self.emit(
                "    return { __tag: 'Ok', __fields: [JSON.parse(String(request?.body ?? ''))] };",
            );
            self.emit("  } catch (error) {");
            self.emit(
                "    const message = error instanceof Error ? error.message : String(error);",
            );
            self.emit("    return { __tag: 'Err', __fields: [__sigil_http_body_error(message)] };");
            self.emit("  }");
            self.emit("}");
            self.emit("function __sigil_http_match(method, pathPattern, request) {");
            self.emit("  const actualMethod = String(request?.method ?? 'GET').toUpperCase();");
            self.emit("  if (String(method ?? '').toUpperCase() !== actualMethod) {");
            self.emit("    return { __tag: 'None', __fields: [] };");
            self.emit("  }");
            self.emit(
                "  const patternSegments = String(pathPattern ?? '/').split('/').filter(Boolean);",
            );
            self.emit("  const requestSegments = String(request?.path ?? '/').split('/').filter(Boolean);");
            self.emit("  if (patternSegments.length !== requestSegments.length) {");
            self.emit("    return { __tag: 'None', __fields: [] };");
            self.emit("  }");
            self.emit("  const params = [];");
            self.emit("  for (let index = 0; index < patternSegments.length; index += 1) {");
            self.emit("    const patternSegment = patternSegments[index];");
            self.emit("    const requestSegment = requestSegments[index];");
            self.emit("    if (patternSegment.startsWith(':')) {");
            self.emit("      params.push([patternSegment.slice(1), requestSegment]);");
            self.emit("      continue;");
            self.emit("    }");
            self.emit("    if (patternSegment !== requestSegment) {");
            self.emit("      return { __tag: 'None', __fields: [] };");
            self.emit("    }");
            self.emit("  }");
            self.emit("  return { __tag: 'Some', __fields: [{ params: __sigil_map_from_entries(params) }] };");
            self.emit("}");
            self.emit("function __sigil_http_listen_result(port) {");
            self.emit("  return { port: Number(port) };");
            self.emit("}");
            self.emit("function __sigil_http_server_state(serverHandle) {");
            self.emit("  const world = __sigil_current_world();");
            self.emit("  const port = Number(serverHandle?.port ?? Number.NaN);");
            self.emit("  const state = world.httpServers?.get(port) ?? null;");
            self.emit("  if (!state) {");
            self.emit(
                "    throw new Error(`unknown http server '${String(serverHandle?.port ?? '')}' in current world`);",
            );
            self.emit("  }");
            self.emit("  return state;");
            self.emit("}");
            self.emit("function __sigil_http_register_server_state(serverState) {");
            self.emit("  const world = __sigil_current_world();");
            self.emit("  world.httpServers.set(Number(serverState.port), serverState);");
            self.emit("  return __sigil_http_listen_result(serverState.port);");
            self.emit("}");
            self.emit("function __sigil_http_websocket_routes(routes) {");
            self.emit("  return __sigil_world_websocket_normalize_routes(routes);");
            self.emit("}");
            self.emit("function __sigil_http_websocket_route_state(serverState, handleName) {");
            self.emit("  const routeStates = serverState?.routeStates ?? Object.create(null);");
            self.emit("  const state = routeStates[String(handleName)] ?? null;");
            self.emit("  if (!state) {");
            self.emit(
                "    throw new Error(`HTTP server does not expose websocket route '${String(handleName)}'`);",
            );
            self.emit("  }");
            self.emit("  return state;");
            self.emit("}");
            self.emit("async function __sigil_http_finish_websocket_routes(serverState) {");
            self.emit("  const routeStates = serverState?.routeStates ?? null;");
            self.emit("  if (routeStates) {");
            self.emit("    for (const routeState of Object.values(routeStates)) {");
            self.emit("      await __sigil_world_stream_close(routeState.source);");
            self.emit("    }");
            self.emit("    const routeHandleNames = new Set(Object.keys(routeStates));");
            self.emit("    const world = __sigil_current_world();");
            self.emit(
                "    for (const state of Array.from(world.websocketClients?.values() ?? [])) {",
            );
            self.emit("      if (routeHandleNames.has(String(state.handleName ?? ''))) {");
            self.emit("        state.closed = true;");
            self.emit("        __sigil_world_stream_finish(state.source);");
            self.emit("      }");
            self.emit("    }");
            self.emit("  }");
            self.emit("  if (typeof serverState?.websocketAttachment?.close === 'function') {");
            self.emit("    await serverState.websocketAttachment.close();");
            self.emit("  }");
            self.emit("  return null;");
            self.emit("}");
            self.emit("async function __sigil_http_attach_websocket_routes(serverState, routes) {");
            self.emit("  const world = __sigil_current_world();");
            self.emit("  const normalizedRoutes = __sigil_http_websocket_routes(routes);");
            self.emit("  const routeStates = Object.create(null);");
            self.emit("  const realRoutes = [];");
            self.emit("  for (const route of normalizedRoutes) {");
            self.emit(
                "    const entry = __sigil_world_websocket_entry_for_handle(world, route.handleName);",
            );
            self.emit("    if (entry.kind === 'deny') {");
            self.emit(
                "      __sigil_world_error(`WebSocketHandle '${route.handleName}' is denied by the current world`);",
            );
            self.emit("    }");
            self.emit("    routeStates[route.handleName] = {");
            self.emit("      handleName: route.handleName,");
            self.emit("      path: route.path,");
            self.emit("      source: __sigil_world_stream_open()");
            self.emit("    };");
            self.emit("    if (entry.kind === 'fixture') {");
            self.emit("      for (const rule of entry.rules ?? []) {");
            self.emit(
                "        const source = await __sigil_world_websocket_fixture_messages(world, route.handleName, rule.messages ?? []);",
            );
            self.emit(
                "        const client = __sigil_world_websocket_register_client(world, route.handleName, source, {",
            );
            self.emit("          closed: false,");
            self.emit("          fixture: true,");
            self.emit("          socket: null");
            self.emit("        });");
            self.emit(
                "        __sigil_world_stream_push(routeStates[route.handleName].source, client);",
            );
            self.emit("      }");
            self.emit("      __sigil_world_stream_finish(routeStates[route.handleName].source);");
            self.emit("    } else {");
            self.emit("      realRoutes.push(route);");
            self.emit("    }");
            self.emit("  }");
            self.emit("  serverState.routeStates = routeStates;");
            self.emit("  if (realRoutes.length === 0) {");
            self.emit("    serverState.websocketAttachment = null;");
            self.emit("    return serverState;");
            self.emit("  }");
            self.emit("  let websocketRuntime = null;");
            self.emit("  try {");
            self.emit("    websocketRuntime =");
            self.emit("      typeof globalThis.__sigil_load_websocket_runtime === 'function'");
            self.emit("        ? await globalThis.__sigil_load_websocket_runtime()");
            self.emit("        : null;");
            self.emit("  } catch (error) {");
            self.emit(
                "    __sigil_world_error(`§httpServer websocket runtime helper is unavailable: ${String(error?.message ?? error ?? '')}`);",
            );
            self.emit("  }");
            self.emit(
                "  if (!websocketRuntime || typeof websocketRuntime.attachServer !== 'function') {",
            );
            self.emit(
                "    __sigil_world_error('§httpServer websocket runtime helper is unavailable');",
            );
            self.emit("  }");
            self.emit("  serverState.websocketAttachment = await websocketRuntime.attachServer(");
            self.emit("    serverState.server,");
            self.emit("    realRoutes,");
            self.emit("    (handleName, socket) => {");
            self.emit(
                "      const routeState = __sigil_http_websocket_route_state(serverState, handleName);",
            );
            self.emit("      const source = __sigil_world_stream_open();");
            self.emit(
                "      const client = __sigil_world_websocket_register_client(world, handleName, source, {",
            );
            self.emit("        closed: false,");
            self.emit("        fixture: false,");
            self.emit("        socket");
            self.emit("      });");
            self.emit("      socket.on?.('message', (value) => {");
            self.emit("        const text = __sigil_world_websocket_message_text(value);");
            self.emit("        __sigil_world_websocket_trace(world, 'received', handleName, {");
            self.emit("          clientId: client.id,");
            self.emit("          text");
            self.emit("        });");
            self.emit("        __sigil_world_stream_push(source, text);");
            self.emit("      });");
            self.emit("      const finish = () => {");
            self.emit("        const state = world.websocketClients?.get(client.id) ?? null;");
            self.emit("        if (state) {");
            self.emit("          state.closed = true;");
            self.emit("        }");
            self.emit("        __sigil_world_stream_finish(source);");
            self.emit("      };");
            self.emit("      socket.once?.('close', finish);");
            self.emit("      socket.once?.('error', finish);");
            self.emit("      __sigil_world_stream_push(routeState.source, client);");
            self.emit("    }");
            self.emit("  );");
            self.emit("  return serverState;");
            self.emit("}");
            self.emit("async function __sigil_http_close(serverHandle) {");
            self.emit("  const world = __sigil_current_world();");
            self.emit("  const port = Number(serverHandle?.port ?? Number.NaN);");
            self.emit("  const serverState = world.httpServers?.get(port) ?? null;");
            self.emit("  if (!serverState || serverState.closed) {");
            self.emit("    return null;");
            self.emit("  }");
            self.emit("  serverState.closed = true;");
            self.emit("  if (serverState.requestSource) {");
            self.emit("    await __sigil_world_stream_close(serverState.requestSource);");
            self.emit("  }");
            self.emit("  await __sigil_http_finish_websocket_routes(serverState);");
            self.emit("  const server = serverState.server;");
            self.emit("  if (!server) {");
            self.emit("    world.httpServers?.delete(port);");
            self.emit("    return null;");
            self.emit("  }");
            self.emit("  await new Promise((resolve) => {");
            self.emit("    try {");
            self.emit("      server.close(() => resolve(undefined));");
            self.emit("    } catch (_) {");
            self.emit("      resolve(undefined);");
            self.emit("    }");
            self.emit("  });");
            self.emit("  world.httpServers?.delete(port);");
            self.emit("  return null;");
            self.emit("}");
            self.emit("async function __sigil_http_listen_requests(port) {");
            self.emit("  const { createServer } = await import('node:http');");
            self.emit("  const { text } = await import('stream/consumers');");
            self.emit("  let assignedPort = Number(port ?? 0);");
            self.emit("  let nextResponderId = 1;");
            self.emit("  const requestSource = __sigil_world_stream_open();");
            self.emit("  const server = createServer(async (req, res) => {");
            self.emit("    try {");
            self.emit("      const request = {");
            self.emit("        body: await text(req),");
            self.emit("        headers: __sigil_http_headers_from_node(req.headers),");
            self.emit("        method: String(req.method ?? 'GET'),");
            self.emit("        path: __sigil_http_request_path(req.url)");
            self.emit("      };");
            self.emit("      let replied = false;");
            self.emit("      const responder = { id: `http-${String(nextResponderId++)}` };");
            self.emit("      responder.__sigil_reply = async (response) => {");
            self.emit("        if (replied) {");
            self.emit("          throw new Error(`HTTP responder '${responder.id}' has already replied`);");
            self.emit("        }");
            self.emit("        replied = true;");
            self.emit("        res.writeHead(Number(response?.status ?? 200), __sigil_http_headers_to_js(response?.headers));");
            self.emit("        res.end(String(response?.body ?? ''));");
            self.emit("        return null;");
            self.emit("      };");
            self.emit("      req.once('aborted', () => { replied = true; });");
            self.emit("      res.once('close', () => { replied = true; });");
            self.emit("      __sigil_world_stream_push(requestSource, { request, responder });");
            self.emit("    } catch (error) {");
            self.emit(
                "      const message = error instanceof Error ? error.message : String(error);",
            );
            self.emit("      res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' });");
            self.emit("      res.end(message);");
            self.emit("    }");
            self.emit("  });");
            self.emit("  const done = new Promise((resolve, reject) => {");
            self.emit("    server.once('close', () => resolve(undefined));");
            self.emit("    server.once('error', reject);");
            self.emit("  });");
            self.emit("  await new Promise((resolve, reject) => {");
            self.emit("    server.once('error', reject);");
            self.emit("    server.listen(port, () => resolve(undefined));");
            self.emit("  });");
            self.emit("  const address = server.address();");
            self.emit("  if (address && typeof address === 'object' && 'port' in address) {");
            self.emit("    assignedPort = Number(address.port ?? assignedPort);");
            self.emit("  }");
            self.emit("  return __sigil_http_register_server_state({");
            self.emit("    closed: false,");
            self.emit("    done,");
            self.emit("    port: assignedPort,");
            self.emit("    requestSource,");
            self.emit("    routeStates: Object.create(null),");
            self.emit("    server,");
            self.emit("    websocketAttachment: null");
            self.emit("  });");
            self.emit("}");
            self.emit(
                "async function __sigil_http_listen_requests_with_websockets(port, routes) {",
            );
            self.emit("  const { createServer } = await import('node:http');");
            self.emit("  const { text } = await import('stream/consumers');");
            self.emit("  let assignedPort = Number(port ?? 0);");
            self.emit("  let nextResponderId = 1;");
            self.emit("  const requestSource = __sigil_world_stream_open();");
            self.emit("  const server = createServer(async (req, res) => {");
            self.emit("    try {");
            self.emit("      const request = {");
            self.emit("        body: await text(req),");
            self.emit("        headers: __sigil_http_headers_from_node(req.headers),");
            self.emit("        method: String(req.method ?? 'GET'),");
            self.emit("        path: __sigil_http_request_path(req.url)");
            self.emit("      };");
            self.emit("      let replied = false;");
            self.emit("      const responder = { id: `http-${String(nextResponderId++)}` };");
            self.emit("      responder.__sigil_reply = async (response) => {");
            self.emit("        if (replied) {");
            self.emit("          throw new Error(`HTTP responder '${responder.id}' has already replied`);");
            self.emit("        }");
            self.emit("        replied = true;");
            self.emit("        res.writeHead(Number(response?.status ?? 200), __sigil_http_headers_to_js(response?.headers));");
            self.emit("        res.end(String(response?.body ?? ''));");
            self.emit("        return null;");
            self.emit("      };");
            self.emit("      req.once('aborted', () => { replied = true; });");
            self.emit("      res.once('close', () => { replied = true; });");
            self.emit("      __sigil_world_stream_push(requestSource, { request, responder });");
            self.emit("    } catch (error) {");
            self.emit(
                "      const message = error instanceof Error ? error.message : String(error);",
            );
            self.emit("      res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' });");
            self.emit("      res.end(message);");
            self.emit("    }");
            self.emit("  });");
            self.emit("  const done = new Promise((resolve, reject) => {");
            self.emit("    server.once('close', () => resolve(undefined));");
            self.emit("    server.once('error', reject);");
            self.emit("  });");
            self.emit("  const serverState = {");
            self.emit("    closed: false,");
            self.emit("    done,");
            self.emit("    port: assignedPort,");
            self.emit("    requestSource,");
            self.emit("    routeStates: Object.create(null),");
            self.emit("    server,");
            self.emit("    websocketAttachment: null");
            self.emit("  };");
            self.emit("  await __sigil_http_attach_websocket_routes(serverState, routes);");
            self.emit("  await new Promise((resolve, reject) => {");
            self.emit("    server.once('error', reject);");
            self.emit("    server.listen(port, () => resolve(undefined));");
            self.emit("  });");
            self.emit("  const address = server.address();");
            self.emit("  if (address && typeof address === 'object' && 'port' in address) {");
            self.emit("    assignedPort = Number(address.port ?? assignedPort);");
            self.emit("  }");
            self.emit("  serverState.port = assignedPort;");
            self.emit("  return __sigil_http_register_server_state(serverState);");
            self.emit("}");
            self.emit("async function __sigil_http_requests(serverHandle) {");
            self.emit(
                "  const requestSource = __sigil_http_server_state(serverHandle).requestSource;",
            );
            self.emit("  if (!requestSource) {");
            self.emit("    throw new Error(`HTTP server '${String(serverHandle?.port ?? '')}' does not expose request streams`);");
            self.emit("  }");
            self.emit("  return requestSource;");
            self.emit("}");
            self.emit("async function __sigil_http_reply(response, responder) {");
            self.emit("  if (typeof responder?.__sigil_reply !== 'function') {");
            self.emit("    throw new Error(`HTTP responder '${String(responder?.id ?? '')}' is unavailable`);");
            self.emit("  }");
            self.emit("  return await responder.__sigil_reply(response);");
            self.emit("}");
            self.emit(
                "async function __sigil_http_websocket_connections(handleName, serverHandle) {",
            );
            self.emit(
                "  return __sigil_http_websocket_route_state(__sigil_http_server_state(serverHandle), handleName).source;",
            );
            self.emit("}");
            self.emit("async function __sigil_http_websocket_messages(client) {");
            self.emit("  return __sigil_world_websocket_messages(client);");
            self.emit("}");
            self.emit("async function __sigil_http_websocket_send(client, text) {");
            self.emit("  return __sigil_world_websocket_send(client, text);");
            self.emit("}");
            self.emit("async function __sigil_http_websocket_close(client) {");
            self.emit("  return __sigil_world_websocket_close(client);");
            self.emit("}");
            self.emit("async function __sigil_http_listen(handler, port) {");
            self.emit("  const { createServer } = await import('node:http');");
            self.emit("  const { text } = await import('stream/consumers');");
            self.emit("  let assignedPort = Number(port ?? 0);");
            self.emit("  const server = createServer(async (req, res) => {");
            self.emit("    try {");
            self.emit("      const request = {");
            self.emit("        body: await text(req),");
            self.emit("        headers: __sigil_http_headers_from_node(req.headers),");
            self.emit("        method: String(req.method ?? 'GET'),");
            self.emit("        path: __sigil_http_request_path(req.url)");
            self.emit("      };");
            self.emit("      const response = await Promise.resolve(handler(request));");
            self.emit(
                "      res.writeHead(response.status, __sigil_http_headers_to_js(response.headers));",
            );
            self.emit("      res.end(String(response.body));");
            self.emit("    } catch (error) {");
            self.emit(
                "      const message = error instanceof Error ? error.message : String(error);",
            );
            self.emit("      res.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' });");
            self.emit("      res.end(message);");
            self.emit("    }");
            self.emit("  });");
            self.emit("  const done = new Promise((resolve, reject) => {");
            self.emit("    server.once('close', () => resolve(undefined));");
            self.emit("    server.once('error', reject);");
            self.emit("  });");
            self.emit("  await new Promise((resolve, reject) => {");
            self.emit("    server.once('error', reject);");
            self.emit("    server.listen(port, () => resolve(undefined));");
            self.emit("  });");
            self.emit("  const address = server.address();");
            self.emit("  if (address && typeof address === 'object' && 'port' in address) {");
            self.emit("    assignedPort = Number(address.port ?? assignedPort);");
            self.emit("  }");
            self.emit("  return __sigil_http_register_server_state({");
            self.emit("    closed: false,");
            self.emit("    done,");
            self.emit("    port: assignedPort,");
            self.emit("    requestSource: null,");
            self.emit("    routeStates: Object.create(null),");
            self.emit("    server,");
            self.emit("    websocketAttachment: null");
            self.emit("  });");
            self.emit("}");
            self.emit("async function __sigil_http_wait(serverHandle) {");
            self.emit("  await Promise.resolve(__sigil_http_server_state(serverHandle).done);");
            self.emit("  return null;");
            self.emit("}");
            self.emit("async function __sigil_http_serve(handler, port) {");
            self.emit("  const server = await __sigil_http_listen(handler, port);");
            self.emit("  return __sigil_http_wait(server);");
            self.emit("}");
        }
        self.emit("function __sigil_tcp_error(kind, message) {");
        self.emit("  return { kind: { __tag: kind, __fields: [] }, message: String(message) };");
        self.emit("}");
        self.emit("function __sigil_tcp_is_valid_host(host) {");
        self.emit("  return typeof host === 'string' && host.length > 0;");
        self.emit("}");
        self.emit("function __sigil_tcp_is_valid_port(port) {");
        self.emit("  return Number.isInteger(port) && port > 0 && port <= 65535;");
        self.emit("}");
        self.emit("function __sigil_tcp_first_line(buffer) {");
        self.emit("  const index = buffer.indexOf('\\n');");
        self.emit("  return index === -1 ? null : buffer.slice(0, index).replace(/\\r$/, '');");
        self.emit("}");
        if include_tcp_server_runtime {
            self.emit("function __sigil_tcp_listen_result(server, port, done) {");
            self.emit(
                "  return { __sigil_done: done, __sigil_server: server, port: Number(port) };",
            );
            self.emit("}");
            self.emit("async function __sigil_tcp_listen(handler, port) {");
            self.emit("  const { createServer } = await import('node:net');");
            self.emit("  let assignedPort = Number(port ?? 0);");
            self.emit("  const server = createServer((socket) => {");
            self.emit("    socket.setEncoding('utf8');");
            self.emit("    let received = '';");
            self.emit("    let handled = false;");
            self.emit("    socket.on('data', async (chunk) => {");
            self.emit("      if (handled) return;");
            self.emit("      received += String(chunk);");
            self.emit("      const line = __sigil_tcp_first_line(received);");
            self.emit("      if (line === null) return;");
            self.emit("      handled = true;");
            self.emit("      try {");
            self.emit("        const request = {");
            self.emit("          host: String(socket.remoteAddress ?? ''),");
            self.emit("          message: line,");
            self.emit("          port: assignedPort");
            self.emit("        };");
            self.emit("        const response = await Promise.resolve(handler(request));");
            self.emit(
                "        socket.write(`${String(response.message)}\\n`, () => socket.end());",
            );
            self.emit("      } catch (error) {");
            self.emit(
                "        const message = error instanceof Error ? error.message : String(error);",
            );
            self.emit("        socket.write(`${message}\\n`, () => socket.end());");
            self.emit("      }");
            self.emit("    });");
            self.emit("    socket.once('end', () => {");
            self.emit("      if (!handled) {");
            self.emit("        socket.write('protocol error: missing newline-delimited request\\n', () => socket.end());");
            self.emit("      }");
            self.emit("    });");
            self.emit("    socket.once('error', () => {");
            self.emit("      socket.destroy();");
            self.emit("    });");
            self.emit("  });");
            self.emit("  const done = new Promise((resolve, reject) => {");
            self.emit("    server.once('close', () => resolve(undefined));");
            self.emit("    server.once('error', reject);");
            self.emit("  });");
            self.emit("  await new Promise((resolve, reject) => {");
            self.emit("    server.once('error', reject);");
            self.emit("    server.listen(port, () => resolve(undefined));");
            self.emit("  });");
            self.emit("  const address = server.address();");
            self.emit("  if (address && typeof address === 'object' && 'port' in address) {");
            self.emit("    assignedPort = Number(address.port ?? assignedPort);");
            self.emit("  }");
            self.emit(
                "  console.log(`TCP server running at tcp://127.0.0.1:${String(assignedPort)}`);",
            );
            self.emit("  return __sigil_tcp_listen_result(server, assignedPort, done);");
            self.emit("}");
            self.emit("async function __sigil_tcp_wait(serverHandle) {");
            self.emit("  await Promise.resolve(serverHandle?.__sigil_done);");
            self.emit("  return null;");
            self.emit("}");
            self.emit("async function __sigil_tcp_serve(handler, port) {");
            self.emit("  const server = await __sigil_tcp_listen(handler, port);");
            self.emit("  return __sigil_tcp_wait(server);");
            self.emit("}");
        }
        self.emit("function __sigil_is_map(value) {");
        self.emit(
            "  return !!value && typeof value === 'object' && Array.isArray(value.__sigil_map);",
        );
        self.emit("}");
        self.emit("function __sigil_deep_equal(a, b) {");
        self.emit("  if (a === b) return true;");
        self.emit("  if (a == null || b == null) return false;");
        self.emit("  if (typeof a !== typeof b) return false;");
        self.emit("  if (__sigil_is_map(a) && __sigil_is_map(b)) {");
        self.emit("    if (a.__sigil_map.length !== b.__sigil_map.length) return false;");
        self.emit("    for (const [aKey, aValue] of a.__sigil_map) {");
        self.emit("      let matched = false;");
        self.emit("      for (const [bKey, bValue] of b.__sigil_map) {");
        self.emit("        if (__sigil_deep_equal(aKey, bKey)) {");
        self.emit("          if (!__sigil_deep_equal(aValue, bValue)) return false;");
        self.emit("          matched = true;");
        self.emit("          break;");
        self.emit("        }");
        self.emit("      }");
        self.emit("      if (!matched) return false;");
        self.emit("    }");
        self.emit("    return true;");
        self.emit("  }");
        self.emit("  if (Array.isArray(a) && Array.isArray(b)) {");
        self.emit("    if (a.length !== b.length) return false;");
        self.emit("    for (let i = 0; i < a.length; i++) {");
        self.emit("      if (!__sigil_deep_equal(a[i], b[i])) return false;");
        self.emit("    }");
        self.emit("    return true;");
        self.emit("  }");
        self.emit("  if (typeof a === 'object' && typeof b === 'object') {");
        self.emit("    const aKeys = Object.keys(a).sort();");
        self.emit("    const bKeys = Object.keys(b).sort();");
        self.emit("    if (aKeys.length !== bKeys.length) return false;");
        self.emit("    for (let i = 0; i < aKeys.length; i++) {");
        self.emit("      if (aKeys[i] !== bKeys[i]) return false;");
        self.emit("      if (!__sigil_deep_equal(a[aKeys[i]], b[bKeys[i]])) return false;");
        self.emit("    }");
        self.emit("    return true;");
        self.emit("  }");
        self.emit("  return false;");
        self.emit("}");
        self.emit("function __sigil_preview(value) {");
        self.emit("  try { return JSON.stringify(value); } catch { return String(value); }");
        self.emit("}");
        self.emit("function __sigil_diff_hint(actual, expected) {");
        self.emit("  if (Array.isArray(actual) && Array.isArray(expected)) {");
        self.emit("    if (actual.length !== expected.length) { return { kind: 'array_length', actualLength: actual.length, expectedLength: expected.length }; }");
        self.emit("    for (let i = 0; i < actual.length; i++) { if (actual[i] !== expected[i]) { return { kind: 'array_first_diff', index: i, actual: __sigil_preview(actual[i]), expected: __sigil_preview(expected[i]) }; } }");
        self.emit("    return null;");
        self.emit("  }");
        self.emit("  if (actual && expected && typeof actual === 'object' && typeof expected === 'object') {");
        self.emit("    const actualKeys = Object.keys(actual).sort();");
        self.emit("    const expectedKeys = Object.keys(expected).sort();");
        self.emit("    if (actualKeys.join('|') !== expectedKeys.join('|')) { return { kind: 'object_keys', actualKeys, expectedKeys }; }");
        self.emit("    for (const k of actualKeys) { if (actual[k] !== expected[k]) { return { kind: 'object_field', field: k, actual: __sigil_preview(actual[k]), expected: __sigil_preview(expected[k]) }; } }");
        self.emit("    return null;");
        self.emit("  }");
        self.emit("  return null;");
        self.emit("}");
        self.emit("async function __sigil_test_bool_result(ok) {");
        self.emit("  const result = await ok;");
        self.emit("  return result === true ? { ok: true } : { ok: false, failure: { kind: 'assert_false', message: 'Test body evaluated to false' } };");
        self.emit("}");
        self.emit("async function __sigil_test_compare_result(op, leftFn, rightFn) {");
        self.emit("  const actual = await leftFn();");
        self.emit("  const expected = await rightFn();");
        self.emit("  let ok = false;");
        self.emit("  switch (op) {");
        self.emit("    case '=': ok = __sigil_deep_equal(actual, expected); break;");
        self.emit("    case '≠': ok = !__sigil_deep_equal(actual, expected); break;");
        self.emit("    case '<': ok = actual < expected; break;");
        self.emit("    case '>': ok = actual > expected; break;");
        self.emit("    case '≤': ok = actual <= expected; break;");
        self.emit("    case '≥': ok = actual >= expected; break;");
        self.emit(
            "    default: throw new Error('Unsupported test comparison operator: ' + String(op));",
        );
        self.emit("  }");
        self.emit("  if (ok) { return { ok: true }; }");
        self.emit("  return { ok: false, failure: { kind: 'comparison_mismatch', message: 'Comparison test failed', operator: op, actual: __sigil_preview(actual), expected: __sigil_preview(expected), diffHint: __sigil_diff_hint(actual, expected) } };");
        self.emit("}");
        self.emit("function __sigil_call(_key, actualFn, args = []) {");
        self.emit("  const __sigil_run = () => Promise.resolve().then(() => {");
        self.emit("    switch (_key) {");
        if include_http_server_runtime {
            self.emit("      case 'extern:stdlib/httpServer.jsonBody':");
            self.emit("        return __sigil_http_json_body_result(args[0]);");
            self.emit("      case 'extern:stdlib/httpServer.listen':");
            self.emit(
                "        return __sigil_http_listen_requests(args[0]).then((__server) => __sigil_owned_wrap(__server, async () => { await __sigil_http_close(__server); return null; }));",
            );
            self.emit("      case 'extern:stdlib/httpServer.listenWithWebSockets':");
            self.emit(
                "        return __sigil_http_listen_requests_with_websockets(args[0], args[1]).then((__server) => __sigil_owned_wrap(__server, async () => { await __sigil_http_close(__server); return null; }));",
            );
            self.emit("      case 'extern:stdlib/httpServer.listenWith':");
            self.emit("        return __sigil_http_listen(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/httpServer.match':");
            self.emit("        return __sigil_http_match(args[0], args[1], args[2]);");
            self.emit("      case 'extern:stdlib/httpServer.port':");
            self.emit("        return Number(args[0]?.port ?? 0);");
            self.emit("      case 'extern:stdlib/httpServer.reply':");
            self.emit("        return __sigil_http_reply(args[1], args[0]);");
            self.emit("      case 'extern:stdlib/httpServer.requests':");
            self.emit(
                "        return __sigil_http_requests(args[0]).then((__source) => __sigil_owned_wrap(__source, async () => { await __sigil_world_stream_close(__source); return null; }));",
            );
            self.emit("      case 'extern:stdlib/httpServer.serve':");
            self.emit("        return __sigil_http_serve(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/httpServer.wait':");
            self.emit("        return __sigil_http_wait(args[0]);");
            self.emit("      case 'extern:stdlib/httpServer.websocketClose':");
            self.emit("        return __sigil_http_websocket_close(args[0]);");
            self.emit("      case 'extern:stdlib/httpServer.websocketConnections':");
            self.emit(
                "        return __sigil_http_websocket_connections(args[0]?.__fields?.[0] ?? '', args[1]).then((__source) => __sigil_owned_wrap(__source, async () => { await __sigil_world_stream_close(__source); return null; }));",
            );
            self.emit("      case 'extern:stdlib/httpServer.websocketMessages':");
            self.emit(
                "        return __sigil_http_websocket_messages(args[0]).then((__source) => __sigil_owned_wrap(__source, async () => { await __sigil_world_stream_close(__source); return null; }));",
            );
            self.emit("      case 'extern:stdlib/httpServer.websocketRoute':");
            self.emit("        return { handle: args[0], path: String(args[1] ?? '') };");
            self.emit("      case 'extern:stdlib/httpServer.websocketSend':");
            self.emit("        return __sigil_http_websocket_send(args[0], args[1]);");
        }
        if include_cli_runtime {
            self.emit("      case 'extern:stdlib/cli.command0':");
            self.emit("        return __sigil_cli_command('command', String(args[1] ?? ''), String(args[0] ?? ''), [], async () => args[2]);");
            self.emit("      case 'extern:stdlib/cli.command1':");
            self.emit("        return __sigil_cli_command('command', String(args[3] ?? ''), String(args[2] ?? ''), [args[0]], args[1]);");
            self.emit("      case 'extern:stdlib/cli.command2':");
            self.emit("        return __sigil_cli_command('command', String(args[4] ?? ''), String(args[3] ?? ''), [args[0], args[1]], args[2]);");
            self.emit("      case 'extern:stdlib/cli.command3':");
            self.emit("        return __sigil_cli_command('command', String(args[5] ?? ''), String(args[4] ?? ''), [args[0], args[1], args[2]], args[3]);");
            self.emit("      case 'extern:stdlib/cli.command4':");
            self.emit("        return __sigil_cli_command('command', String(args[6] ?? ''), String(args[5] ?? ''), [args[0], args[1], args[2], args[3]], args[4]);");
            self.emit("      case 'extern:stdlib/cli.command5':");
            self.emit("        return __sigil_cli_command('command', String(args[7] ?? ''), String(args[6] ?? ''), [args[0], args[1], args[2], args[3], args[4]], args[5]);");
            self.emit("      case 'extern:stdlib/cli.command6':");
            self.emit("        return __sigil_cli_command('command', String(args[8] ?? ''), String(args[7] ?? ''), [args[0], args[1], args[2], args[3], args[4], args[5]], args[6]);");
            self.emit("      case 'extern:stdlib/cli.flag':");
            self.emit("        return __sigil_cli_arg('flag', { description: String(args[0] ?? ''), long: String(args[1] ?? ''), short: args[2] });");
            self.emit("      case 'extern:stdlib/cli.manyOption':");
            self.emit("        return __sigil_cli_arg('manyOption', { description: String(args[0] ?? ''), long: String(args[1] ?? ''), short: args[2], valueName: String(args[3] ?? '') });");
            self.emit("      case 'extern:stdlib/cli.manyPositionals':");
            self.emit("        return __sigil_cli_arg('manyPositionals', { description: String(args[0] ?? ''), name: String(args[1] ?? '') });");
            self.emit("      case 'extern:stdlib/cli.option':");
            self.emit("        return __sigil_cli_arg('option', { description: String(args[0] ?? ''), long: String(args[1] ?? ''), short: args[2], valueName: String(args[3] ?? '') });");
            self.emit("      case 'extern:stdlib/cli.optionalPositional':");
            self.emit("        return __sigil_cli_arg('optionalPositional', { description: String(args[0] ?? ''), name: String(args[1] ?? '') });");
            self.emit("      case 'extern:stdlib/cli.positional':");
            self.emit("        return __sigil_cli_arg('positional', { description: String(args[0] ?? ''), name: String(args[1] ?? '') });");
            self.emit("      case 'extern:stdlib/cli.program':");
            self.emit("        return __sigil_cli_program(String(args[1] ?? ''), String(args[0] ?? ''), args[2]?.__tag === 'Some' ? args[2].__fields?.[0] ?? null : null, Array.isArray(args[3]) ? args[3] : []);");
            self.emit("      case 'extern:stdlib/cli.requiredOption':");
            self.emit("        return __sigil_cli_arg('requiredOption', { description: String(args[0] ?? ''), long: String(args[1] ?? ''), short: args[2], valueName: String(args[3] ?? '') });");
            self.emit("      case 'extern:stdlib/cli.root0':");
            self.emit("        return __sigil_cli_command('root', null, String(args[0] ?? ''), [], async () => args[1]);");
            self.emit("      case 'extern:stdlib/cli.root1':");
            self.emit("        return __sigil_cli_command('root', null, String(args[2] ?? ''), [args[0]], args[1]);");
            self.emit("      case 'extern:stdlib/cli.root2':");
            self.emit("        return __sigil_cli_command('root', null, String(args[3] ?? ''), [args[0], args[1]], args[2]);");
            self.emit("      case 'extern:stdlib/cli.root3':");
            self.emit("        return __sigil_cli_command('root', null, String(args[4] ?? ''), [args[0], args[1], args[2]], args[3]);");
            self.emit("      case 'extern:stdlib/cli.root4':");
            self.emit("        return __sigil_cli_command('root', null, String(args[5] ?? ''), [args[0], args[1], args[2], args[3]], args[4]);");
            self.emit("      case 'extern:stdlib/cli.root5':");
            self.emit("        return __sigil_cli_command('root', null, String(args[6] ?? ''), [args[0], args[1], args[2], args[3], args[4]], args[5]);");
            self.emit("      case 'extern:stdlib/cli.root6':");
            self.emit("        return __sigil_cli_command('root', null, String(args[7] ?? ''), [args[0], args[1], args[2], args[3], args[4], args[5]], args[6]);");
            self.emit("      case 'extern:stdlib/cli.run':");
            self.emit("        return __sigil_cli_run(args[0], args[1]);");
        }
        if include_sql_runtime {
            self.emit("      case 'extern:stdlib/sql.all':");
            self.emit(
                "        return __sigil_world_sql_all(args[0]?.__fields?.[0] ?? '', args[1]);",
            );
            self.emit("      case 'extern:stdlib/sql.allIn':");
            self.emit("        return __sigil_world_sql_all_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.and':");
            self.emit("        return { kind: 'and', left: args[0], right: args[1] };");
            self.emit("      case 'extern:stdlib/sql.begin':");
            self.emit("        return __sigil_world_sql_begin(args[0]?.__fields?.[0] ?? '').then(__sigil_sql_begin_wrap);");
            self.emit("      case 'extern:stdlib/sql.boolColumn':");
            self.emit("        return __sigil_sql_column(args[0], args[1], 'bool');");
            self.emit("      case 'extern:stdlib/sql.bytes':");
            self.emit("        return { base64: String(args[0] ?? '') };");
            self.emit("      case 'extern:stdlib/sql.bytesColumn':");
            self.emit("        return __sigil_sql_column(args[0], args[1], 'bytes');");
            self.emit("      case 'extern:stdlib/sql.commit':");
            self.emit("        return __sigil_world_sql_commit(args[0]);");
            self.emit("      case 'extern:stdlib/sql.delete':");
            self.emit("        return { predicate: null, table: args[0] };");
            self.emit("      case 'extern:stdlib/sql.deleteWhere':");
            self.emit("        return { ...args[1], predicate: args[0] };");
            self.emit("      case 'extern:stdlib/sql.eq':");
            self.emit("        return { kind: 'eq', column: args[0], value: args[1] };");
            self.emit("      case 'extern:stdlib/sql.execDelete':");
            self.emit("        return __sigil_world_sql_exec_delete(args[0]?.__fields?.[0] ?? '', args[1]);");
            self.emit("      case 'extern:stdlib/sql.execDeleteIn':");
            self.emit("        return __sigil_world_sql_exec_delete_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.execInsert':");
            self.emit("        return __sigil_world_sql_exec_insert(args[0]?.__fields?.[0] ?? '', args[1]);");
            self.emit("      case 'extern:stdlib/sql.execInsertIn':");
            self.emit("        return __sigil_world_sql_exec_insert_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.execUpdate':");
            self.emit("        return __sigil_world_sql_exec_update(args[0]?.__fields?.[0] ?? '', args[1]);");
            self.emit("      case 'extern:stdlib/sql.execUpdateIn':");
            self.emit("        return __sigil_world_sql_exec_update_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.floatColumn':");
            self.emit("        return __sigil_sql_column(args[0], args[1], 'float');");
            self.emit("      case 'extern:stdlib/sql.gt':");
            self.emit("        return { kind: 'gt', column: args[0], value: args[1] };");
            self.emit("      case 'extern:stdlib/sql.gte':");
            self.emit("        return { kind: 'gte', column: args[0], value: args[1] };");
            self.emit("      case 'extern:stdlib/sql.insert':");
            self.emit("        return { row: args[0], table: args[1] };");
            self.emit("      case 'extern:stdlib/sql.intColumn':");
            self.emit("        return __sigil_sql_column(args[0], args[1], 'int');");
            self.emit("      case 'extern:stdlib/sql.limit':");
            self.emit("        return { ...args[1], limit: Number(args[0] ?? 0) };");
            self.emit("      case 'extern:stdlib/sql.lt':");
            self.emit("        return { kind: 'lt', column: args[0], value: args[1] };");
            self.emit("      case 'extern:stdlib/sql.lte':");
            self.emit("        return { kind: 'lte', column: args[0], value: args[1] };");
            self.emit("      case 'extern:stdlib/sql.neq':");
            self.emit("        return { kind: 'neq', column: args[0], value: args[1] };");
            self.emit("      case 'extern:stdlib/sql.not':");
            self.emit("        return { kind: 'not', predicate: args[0] };");
            self.emit("      case 'extern:stdlib/sql.nullable':");
            self.emit("        return __sigil_sql_nullable(args[0]);");
            self.emit("      case 'extern:stdlib/sql.one':");
            self.emit(
                "        return __sigil_world_sql_one(args[0]?.__fields?.[0] ?? '', args[1]);",
            );
            self.emit("      case 'extern:stdlib/sql.oneIn':");
            self.emit("        return __sigil_world_sql_one_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.or':");
            self.emit("        return { kind: 'or', left: args[0], right: args[1] };");
            self.emit("      case 'extern:stdlib/sql.orderBy':");
            self.emit("        return { ...args[2], order: { column: args[0], direction: args[1]?.__tag === 'Desc' ? 'Desc' : 'Asc' } };");
            self.emit("      case 'extern:stdlib/sql.raw':");
            self.emit("        return { params: args[0] ?? {}, sql: String(args[1] ?? '') };");
            self.emit("      case 'extern:stdlib/sql.rawExec':");
            self.emit(
                "        return __sigil_world_sql_raw_exec(args[0]?.__fields?.[0] ?? '', args[1]);",
            );
            self.emit("      case 'extern:stdlib/sql.rawExecIn':");
            self.emit("        return __sigil_world_sql_raw_exec_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.rawQuery':");
            self.emit("        return __sigil_world_sql_raw_query(args[0]?.__fields?.[0] ?? '', args[1]);");
            self.emit("      case 'extern:stdlib/sql.rawQueryIn':");
            self.emit("        return __sigil_world_sql_raw_query_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.rawQueryOne':");
            self.emit("        return __sigil_world_sql_raw_query_one(args[0]?.__fields?.[0] ?? '', args[1]);");
            self.emit("      case 'extern:stdlib/sql.rawQueryOneIn':");
            self.emit("        return __sigil_world_sql_raw_query_one_in(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/sql.rollback':");
            self.emit("        return __sigil_world_sql_rollback(args[0]);");
            self.emit("      case 'extern:stdlib/sql.select':");
            self.emit(
                "        return { limit: null, order: null, predicate: null, table: args[0] };",
            );
            self.emit("      case 'extern:stdlib/sql.set':");
            self.emit("        return { ...args[1], assignments: [...(Array.isArray(args[1]?.assignments) ? args[1].assignments : []), { column: args[0], value: args[2] }] };");
            self.emit("      case 'extern:stdlib/sql.table1':");
            self.emit("        return __sigil_sql_table(args[1], [args[0]]);");
            self.emit("      case 'extern:stdlib/sql.table2':");
            self.emit("        return __sigil_sql_table(args[2], [args[0], args[1]]);");
            self.emit("      case 'extern:stdlib/sql.table3':");
            self.emit("        return __sigil_sql_table(args[3], [args[0], args[1], args[2]]);");
            self.emit("      case 'extern:stdlib/sql.table4':");
            self.emit(
                "        return __sigil_sql_table(args[4], [args[0], args[1], args[2], args[3]]);",
            );
            self.emit("      case 'extern:stdlib/sql.table5':");
            self.emit("        return __sigil_sql_table(args[5], [args[0], args[1], args[2], args[3], args[4]]);");
            self.emit("      case 'extern:stdlib/sql.table6':");
            self.emit("        return __sigil_sql_table(args[6], [args[0], args[1], args[2], args[3], args[4], args[5]]);");
            self.emit("      case 'extern:stdlib/sql.table7':");
            self.emit("        return __sigil_sql_table(args[7], [args[0], args[1], args[2], args[3], args[4], args[5], args[6]]);");
            self.emit("      case 'extern:stdlib/sql.table8':");
            self.emit("        return __sigil_sql_table(args[8], [args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7]]);");
            self.emit("      case 'extern:stdlib/sql.textColumn':");
            self.emit("        return __sigil_sql_column(args[0], args[1], 'text');");
            self.emit("      case 'extern:stdlib/sql.update':");
            self.emit("        return { assignments: [], predicate: null, table: args[0] };");
            self.emit("      case 'extern:stdlib/sql.updateWhere':");
            self.emit("        return { ...args[1], predicate: args[0] };");
            self.emit("      case 'extern:stdlib/sql.where':");
            self.emit("        return { ...args[1], predicate: args[0] };");
        }
        if include_tcp_server_runtime {
            self.emit("      case 'extern:stdlib/tcpServer.listen':");
            self.emit("        return __sigil_tcp_listen(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/tcpServer.port':");
            self.emit("        return Number(args[0]?.port ?? 0);");
            self.emit("      case 'extern:stdlib/tcpServer.serve':");
            self.emit("        return __sigil_tcp_serve(args[0], args[1]);");
            self.emit("      case 'extern:stdlib/tcpServer.wait':");
            self.emit("        return __sigil_tcp_wait(args[0]);");
        }
        self.emit("      case 'extern:stdlib/task.cancel':");
        self.emit("        return __sigil_world_task_cancel(args[0]);");
        self.emit("      case 'extern:stdlib/task.spawn':");
        self.emit("        return __sigil_world_task_spawn(args[0]);");
        self.emit("      case 'extern:stdlib/task.wait':");
        self.emit("        return __sigil_world_task_wait(args[0]);");
        self.emit("      default:");
        self.emit("        return actualFn(...args);");
        self.emit("    }");
        self.emit("  });");
        self.emit("  return __sigil_run();");
        self.emit("}");
        if self.trace_enabled {
            self.emit_trace_helpers();
        }
        if self.breakpoints_enabled {
            self.emit_breakpoint_helpers();
        }
        if self.expression_debug_enabled || self.breakpoints_enabled {
            self.emit_expression_helpers();
        }
    }

    fn emit_trace_helpers(&mut self) {
        self.emit("function __sigil_trace_enabled() {");
        self.emit("  return !!globalThis.__sigil_trace_config?.enabled;");
        self.emit("}");
        self.emit("function __sigil_trace_expression_enabled() {");
        self.emit("  return !!globalThis.__sigil_trace_config?.enabled && !!globalThis.__sigil_trace_config?.expressions;");
        self.emit("}");
        self.emit("function __sigil_trace_init_state() {");
        self.emit("  const maxEvents = Math.max(1, Number(globalThis.__sigil_trace_config?.maxEvents ?? 256));");
        self.emit("  return { enabled: true, truncated: false, totalEvents: 0, droppedEvents: 0, maxEvents, nextSeq: 1, depth: 0, events: [] };");
        self.emit("}");
        self.emit("function __sigil_trace_state() {");
        self.emit("  if (!__sigil_trace_enabled()) {");
        self.emit("    return null;");
        self.emit("  }");
        self.emit("  if (!globalThis.__sigil_trace_current || typeof globalThis.__sigil_trace_current !== 'object') {");
        self.emit("    globalThis.__sigil_trace_current = __sigil_trace_init_state();");
        self.emit("  }");
        self.emit("  return globalThis.__sigil_trace_current;");
        self.emit("}");
        self.emit("function __sigil_trace_summary(value, depth = 0) {");
        self.emit("  if (value === null || value === undefined) return { kind: 'unit' };");
        self.emit("  const valueType = typeof value;");
        self.emit("  if (valueType === 'boolean') return { kind: 'bool', value: !!value };");
        self.emit("  if (valueType === 'number') return Number.isInteger(value) ? { kind: 'int', value } : { kind: 'float', value };");
        self.emit("  if (valueType === 'string') {");
        self.emit("    const truncated = value.length > 80;");
        self.emit("    return { kind: 'string', value: truncated ? `${value.slice(0, 80)}…` : value, truncated };");
        self.emit("  }");
        self.emit("  if (valueType === 'function') return { kind: 'function' };");
        self.emit("  if (Array.isArray(value)) return { kind: 'list', size: value.length };");
        self.emit("  if (value && valueType === 'object' && Array.isArray(value.__sigil_map)) return { kind: 'map', size: value.__sigil_map.length };");
        self.emit("  if (value && valueType === 'object' && typeof value.__tag === 'string') return { kind: 'sum', tag: value.__tag, arity: Array.isArray(value.__fields) ? value.__fields.length : 0 };");
        self.emit("  if (value && valueType === 'object') {");
        self.emit("    const keys = Object.keys(value).sort();");
        self.emit("    return { kind: depth > 0 ? 'object' : 'record', size: keys.length, fields: keys.slice(0, 6) };");
        self.emit("  }");
        self.emit("  return { kind: valueType };");
        self.emit("}");
        self.emit("function __sigil_trace_summary_typed(value, depth = 0, typeId = null) {");
        self.emit("  const summary = __sigil_trace_summary(value, depth);");
        self.emit(
            "  if (typeId != null && String(typeId) !== '') summary.typeId = String(typeId);",
        );
        self.emit("  return summary;");
        self.emit("}");
        self.emit("function __sigil_trace_error_summary(error) {");
        self.emit(
            "  const name = error instanceof Error && error.name ? String(error.name) : 'Error';",
        );
        self.emit("  const message = error instanceof Error ? String(error.message ?? '') : String(error);");
        self.emit("  const summary = { kind: 'error', name, message };");
        self.emit("  if (error && typeof error === 'object' && 'sigilCode' in error && error.sigilCode != null) {");
        self.emit("    summary.sigilCode = String(error.sigilCode);");
        self.emit("  }");
        self.emit("  return summary;");
        self.emit("}");
        self.emit("function __sigil_trace_error_summary_typed(error, typeId = null) {");
        self.emit("  const summary = __sigil_trace_error_summary(error);");
        self.emit(
            "  if (typeId != null && String(typeId) !== '') summary.typeId = String(typeId);",
        );
        self.emit("  return summary;");
        self.emit("}");
        self.emit("function __sigil_trace_push(event) {");
        self.emit("  const state = __sigil_trace_state();");
        self.emit("  if (!state) return;");
        self.emit("  const normalized = { seq: state.nextSeq, ...event };");
        self.emit("  state.nextSeq += 1;");
        self.emit("  state.totalEvents += 1;");
        self.emit("  if (state.events.length >= state.maxEvents) {");
        self.emit("    state.events.shift();");
        self.emit("    state.droppedEvents += 1;");
        self.emit("    state.truncated = true;");
        self.emit("  }");
        self.emit("  state.events.push(normalized);");
        self.emit("}");
        self.emit("function __sigil_trace_snapshot() {");
        self.emit("  const state = __sigil_trace_state();");
        self.emit("  if (!state) {");
        self.emit("    return { enabled: false, truncated: false, totalEvents: 0, returnedEvents: 0, droppedEvents: 0, events: [] };");
        self.emit("  }");
        self.emit("  return { enabled: true, truncated: !!state.truncated, totalEvents: state.totalEvents, returnedEvents: state.events.length, droppedEvents: state.droppedEvents, events: state.events.slice() };");
        self.emit("}");
        self.emit("globalThis.__sigil_trace_summary = __sigil_trace_summary;");
        self.emit("globalThis.__sigil_trace_summary_typed = __sigil_trace_summary_typed;");
        self.emit("globalThis.__sigil_trace_error_summary = __sigil_trace_error_summary;");
        self.emit(
            "globalThis.__sigil_trace_error_summary_typed = __sigil_trace_error_summary_typed;",
        );
        self.emit(
            "function __sigil_debug_wrap_call(meta, paramNames, paramTypeIds, args, thunk) {",
        );
        self.emit("  const traceEnabled = __sigil_trace_enabled();");
        self.emit("  const breakpointEnabled = typeof __sigil_breakpoint_enabled === 'function' && __sigil_breakpoint_enabled();");
        self.emit("  if (!traceEnabled && !breakpointEnabled) return thunk();");
        self.emit("  const state = __sigil_trace_state();");
        self.emit("  const depth = state ? state.depth : 0;");
        self.emit("  const functionName = String(meta.functionName ?? '');");
        self.emit("  if (traceEnabled) {");
        self.emit("    __sigil_trace_push({ kind: 'call', depth, ...meta, functionName, args: Array.isArray(args) ? args.map((value) => __sigil_trace_summary(value, 1)) : [] });");
        self.emit("  }");
        self.emit("  if (breakpointEnabled) {");
        self.emit("    __sigil_breakpoint_push_frame(meta, functionName, paramNames, paramTypeIds, args);");
        self.emit("  }");
        self.emit("  if (state) state.depth = depth + 1;");
        self.emit("  if (typeof __sigil_debug_step_emit === 'function') {");
        self.emit("    __sigil_debug_step_emit({ kind: 'function_enter', depth, ...meta, functionName });");
        self.emit("  }");
        self.emit("  if (breakpointEnabled) {");
        self.emit("    __sigil_breakpoint_maybe_hit(meta);");
        self.emit("  }");
        self.emit("  const finish = (value) => {");
        self.emit("    if (typeof __sigil_debug_step_emit === 'function') {");
        self.emit("      __sigil_debug_step_emit({ kind: 'function_return', depth, ...meta, functionName, value: __sigil_trace_summary(value, 1) });");
        self.emit("    }");
        self.emit("    if (state) state.depth = depth;");
        self.emit("    if (breakpointEnabled) {");
        self.emit("      __sigil_breakpoint_pop_frame();");
        self.emit("    }");
        self.emit("    if (traceEnabled) {");
        self.emit("      __sigil_trace_push({ kind: 'return', depth, ...meta, functionName, result: __sigil_trace_summary(value, 1) });");
        self.emit("    }");
        self.emit("    return value;");
        self.emit("  };");
        self.emit("  try {");
        self.emit("    const result = thunk();");
        self.emit("    if (result && typeof result.then === 'function') {");
        self.emit("      return result.then((value) => finish(value), (error) => { if (state) state.depth = depth; if (breakpointEnabled) { __sigil_breakpoint_pop_frame(); } throw error; });");
        self.emit("    }");
        self.emit("    return finish(result);");
        self.emit("  } catch (error) {");
        self.emit("    if (state) state.depth = depth;");
        self.emit("    if (breakpointEnabled) {");
        self.emit("      __sigil_breakpoint_pop_frame();");
        self.emit("    }");
        self.emit("    throw error;");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_trace_branch_if(meta, condition, taken) {");
        self.emit("  if (!__sigil_trace_enabled()) return;");
        self.emit("  const state = __sigil_trace_state();");
        self.emit("  __sigil_trace_push({ kind: 'branch_if', depth: state ? state.depth : 0, ...meta, taken: String(taken), condition: __sigil_trace_summary(condition, 1) });");
        self.emit("}");
        self.emit("function __sigil_trace_branch_match(meta, armSpanId, armIndex, hasGuard) {");
        self.emit("  if (!__sigil_trace_enabled()) return;");
        self.emit("  const state = __sigil_trace_state();");
        self.emit("  __sigil_trace_push({ kind: 'branch_match', depth: state ? state.depth : 0, ...meta, armSpanId: String(armSpanId ?? ''), armIndex: Number(armIndex), hasGuard: !!hasGuard });");
        self.emit("}");
        self.emit("function __sigil_trace_wrap_effect(meta, args, thunk) {");
        self.emit("  if (!__sigil_trace_enabled()) return thunk();");
        self.emit("  const state = __sigil_trace_state();");
        self.emit("  const depth = state ? state.depth : 0;");
        self.emit("  __sigil_trace_push({ kind: 'effect_call', depth, ...meta, effectFamily: String(meta.effectFamily ?? ''), operation: String(meta.operation ?? ''), args: Array.isArray(args) ? args.map((value) => __sigil_trace_summary(value, 1)) : [] });");
        self.emit("  const finish = (value) => {");
        self.emit("    __sigil_trace_push({ kind: 'effect_result', depth, ...meta, effectFamily: String(meta.effectFamily ?? ''), operation: String(meta.operation ?? ''), result: __sigil_trace_summary(value, 1) });");
        self.emit("    return value;");
        self.emit("  };");
        self.emit("  const result = thunk();");
        self.emit("  if (result && typeof result.then === 'function') {");
        self.emit("    return result.then((value) => finish(value));");
        self.emit("  }");
        self.emit("  return finish(result);");
        self.emit("}");
        self.emit("globalThis.__sigil_trace_snapshot = __sigil_trace_snapshot;");
    }

    fn emit_breakpoint_helpers(&mut self) {
        self.emit("function __sigil_breakpoint_enabled() {");
        self.emit("  return !!globalThis.__sigil_breakpoint_config?.enabled;");
        self.emit("}");
        self.emit("function __sigil_breakpoint_init_state() {");
        self.emit("  const maxHits = Math.max(1, Number(globalThis.__sigil_breakpoint_config?.maxHits ?? 32));");
        self.emit("  return { enabled: true, mode: String(globalThis.__sigil_breakpoint_config?.mode ?? 'stop'), stopped: false, truncated: false, totalHits: 0, droppedHits: 0, maxHits, hits: [], stack: [] };");
        self.emit("}");
        self.emit("function __sigil_breakpoint_state() {");
        self.emit("  if (!__sigil_breakpoint_enabled()) return null;");
        self.emit("  if (!globalThis.__sigil_breakpoint_current || typeof globalThis.__sigil_breakpoint_current !== 'object') {");
        self.emit("    globalThis.__sigil_breakpoint_current = __sigil_breakpoint_init_state();");
        self.emit("  }");
        self.emit("  return globalThis.__sigil_breakpoint_current;");
        self.emit("}");
        self.emit("function __sigil_breakpoint_stop_signal() {");
        self.emit("  return { __sigilBreakpointStop: true };");
        self.emit("}");
        self.emit("function __sigil_breakpoint_is_stop_signal(error) {");
        self.emit("  return !!(error && typeof error === 'object' && error.__sigilBreakpointStop === true);");
        self.emit("}");
        self.emit("function __sigil_breakpoint_selectors(spanId) {");
        self.emit("  const selectors = globalThis.__sigil_breakpoint_config?.spans?.[String(spanId ?? '')];");
        self.emit("  return Array.isArray(selectors) ? selectors.slice() : [];");
        self.emit("}");
        self.emit("function __sigil_breakpoint_matches(spanId) {");
        self.emit("  return __sigil_breakpoint_selectors(spanId).length > 0;");
        self.emit("}");
        self.emit("function __sigil_breakpoint_push_frame(meta, functionName, paramNames, paramTypeIds, args) {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  if (!state) return;");
        self.emit("  const names = Array.isArray(paramNames) ? paramNames : [];");
        self.emit("  const typeIds = Array.isArray(paramTypeIds) ? paramTypeIds : [];");
        self.emit("  const values = Array.isArray(args) ? args : [];");
        self.emit("  const params = names.map((name, index) => {");
        self.emit("    const typeId = typeIds[index] == null ? null : String(typeIds[index]);");
        self.emit("    return { name: String(name), origin: 'param', raw: values[index], typeId, value: __sigil_trace_summary_typed(values[index], 1, typeId) };");
        self.emit("  });");
        self.emit("  state.stack.push({ moduleId: String(meta.moduleId ?? ''), sourceFile: String(meta.sourceFile ?? ''), spanId: String(meta.spanId ?? ''), declarationKind: meta.declarationKind ?? null, declarationLabel: meta.declarationLabel ?? null, functionName: functionName ? String(functionName) : null, params, scopes: [] });");
        self.emit("}");
        self.emit("function __sigil_breakpoint_pop_frame() {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  if (!state || state.stack.length === 0) return;");
        self.emit("  state.stack.pop();");
        self.emit("}");
        self.emit("function __sigil_breakpoint_push_scope(locals) {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  if (!state || state.stack.length === 0) return;");
        self.emit("  const frame = state.stack[state.stack.length - 1];");
        self.emit("  frame.scopes.push(Array.isArray(locals) ? locals.map((local) => {");
        self.emit("    const typeId = local?.typeId == null ? null : String(local.typeId);");
        self.emit("    return { name: String(local.name ?? ''), origin: String(local.origin ?? 'let'), raw: local.value, typeId, value: __sigil_trace_summary_typed(local.value, 1, typeId) };");
        self.emit("  }) : []);");
        self.emit("}");
        self.emit("function __sigil_breakpoint_pop_scope() {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  if (!state || state.stack.length === 0) return;");
        self.emit("  const frame = state.stack[state.stack.length - 1];");
        self.emit("  if (frame.scopes.length > 0) frame.scopes.pop();");
        self.emit("}");
        self.emit("function __sigil_breakpoint_current_locals(state) {");
        self.emit("  if (!state || state.stack.length === 0) return [];");
        self.emit("  const frame = state.stack[state.stack.length - 1];");
        self.emit("  const locals = frame.params.map((local) => ({ name: local.name, origin: local.origin, typeId: local.typeId ?? null, value: local.value }));");
        self.emit("  for (const scope of frame.scopes) {");
        self.emit("    locals.push(...scope.map((local) => ({ name: local.name, origin: local.origin, typeId: local.typeId ?? null, value: local.value })));");
        self.emit("  }");
        self.emit("  return locals;");
        self.emit("}");
        self.emit("function __sigil_breakpoint_current_locals_raw(state) {");
        self.emit("  if (!state || state.stack.length === 0) return [];");
        self.emit("  const frame = state.stack[state.stack.length - 1];");
        self.emit("  const locals = frame.params.map((local) => ({ name: local.name, origin: local.origin, raw: local.raw, typeId: local.typeId ?? null }));");
        self.emit("  for (const scope of frame.scopes) {");
        self.emit("    locals.push(...scope.map((local) => ({ name: local.name, origin: local.origin, raw: local.raw, typeId: local.typeId ?? null })));");
        self.emit("  }");
        self.emit("  return locals;");
        self.emit("}");
        self.emit("function __sigil_breakpoint_current_locals_raw_snapshot() {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  return state ? __sigil_breakpoint_current_locals_raw(state) : [];");
        self.emit("}");
        self.emit("function __sigil_breakpoint_stack_snapshot(state) {");
        self.emit("  if (!state) return [];");
        self.emit("  return state.stack.slice().reverse().map((frame) => ({ moduleId: frame.moduleId, sourceFile: frame.sourceFile, spanId: frame.spanId, declarationKind: frame.declarationKind, declarationLabel: frame.declarationLabel, functionName: frame.functionName }));");
        self.emit("}");
        self.emit("function __sigil_breakpoint_recent_trace() {");
        self.emit("  if (typeof globalThis.__sigil_trace_snapshot !== 'function') return [];");
        self.emit("  try {");
        self.emit("    const limit = Math.max(1, Number(globalThis.__sigil_breakpoint_config?.recentTraceLimit ?? 32));");
        self.emit("    const snapshot = globalThis.__sigil_trace_snapshot();");
        self.emit("    const events = Array.isArray(snapshot?.events) ? snapshot.events : [];");
        self.emit("    return events.slice(-limit);");
        self.emit("  } catch (_breakpointTraceError) {");
        self.emit("    return [];");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_debug_step_emit(event) {");
        self.emit("  if (typeof globalThis.__sigil_debug_step_event !== 'function') return;");
        self.emit("  const state = typeof __sigil_breakpoint_state === 'function' ? __sigil_breakpoint_state() : null;");
        self.emit("  const expressionDepth = typeof __sigil_expression_state === 'function' ? (Array.isArray(__sigil_expression_state()?.stack) ? __sigil_expression_state().stack.length : 0) : 0;");
        self.emit("  globalThis.__sigil_debug_step_event({");
        self.emit("    ...event,");
        self.emit("    locals: 'locals' in event ? event.locals : (state ? __sigil_breakpoint_current_locals(state) : []),");
        self.emit("    stack: 'stack' in event ? event.stack : (state ? __sigil_breakpoint_stack_snapshot(state) : []),");
        self.emit("    recentTrace: 'recentTrace' in event ? event.recentTrace : __sigil_breakpoint_recent_trace(),");
        self.emit("    frameDepth: 'frameDepth' in event ? Number(event.frameDepth ?? 0) : (state?.stack?.length ?? 0),");
        self.emit("    expressionDepth: 'expressionDepth' in event ? Number(event.expressionDepth ?? 0) : expressionDepth");
        self.emit("  });");
        self.emit("}");
        self.emit("function __sigil_breakpoint_record_hit(meta) {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  if (!state) return;");
        self.emit("  const selectors = __sigil_breakpoint_selectors(meta.spanId);");
        self.emit("  if (selectors.length === 0) return;");
        self.emit("  state.totalHits += 1;");
        self.emit("  const hit = { matched: selectors, moduleId: String(meta.moduleId ?? ''), sourceFile: String(meta.sourceFile ?? ''), spanId: String(meta.spanId ?? ''), spanKind: meta.spanKind ?? null, declarationKind: meta.declarationKind ?? null, declarationLabel: meta.declarationLabel ?? null, locals: __sigil_breakpoint_current_locals(state), stack: __sigil_breakpoint_stack_snapshot(state), recentTrace: __sigil_breakpoint_recent_trace() };");
        self.emit("  if (typeof __sigil_debug_step_emit === 'function') {");
        self.emit("    __sigil_debug_step_emit({ kind: 'breakpoint', ...hit, frameDepth: state.stack.length });");
        self.emit("  }");
        self.emit("  if (state.hits.length >= state.maxHits) {");
        self.emit("    state.hits.shift();");
        self.emit("    state.droppedHits += 1;");
        self.emit("    state.truncated = true;");
        self.emit("  }");
        self.emit("  state.hits.push(hit);");
        self.emit("  if (state.mode === 'stop') {");
        self.emit("    state.stopped = true;");
        self.emit("    throw __sigil_breakpoint_stop_signal();");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_breakpoint_maybe_hit(meta) {");
        self.emit("  if (!__sigil_breakpoint_matches(meta.spanId)) return;");
        self.emit("  __sigil_breakpoint_record_hit(meta);");
        self.emit("}");
        self.emit("function __sigil_breakpoint_probe(meta, thunk) {");
        self.emit("  if (!__sigil_breakpoint_enabled()) return thunk();");
        self.emit("  __sigil_breakpoint_maybe_hit(meta);");
        self.emit("  return thunk();");
        self.emit("}");
        self.emit("function __sigil_breakpoint_snapshot() {");
        self.emit("  const state = __sigil_breakpoint_state();");
        self.emit("  if (!state) {");
        self.emit("    return { enabled: false, mode: String(globalThis.__sigil_breakpoint_config?.mode ?? 'stop'), stopped: false, truncated: false, totalHits: 0, returnedHits: 0, droppedHits: 0, maxHits: Math.max(1, Number(globalThis.__sigil_breakpoint_config?.maxHits ?? 32)), hits: [] };");
        self.emit("  }");
        self.emit("  return { enabled: true, mode: String(state.mode), stopped: !!state.stopped, truncated: !!state.truncated, totalHits: Number(state.totalHits), returnedHits: state.hits.length, droppedHits: Number(state.droppedHits), maxHits: Number(state.maxHits), hits: state.hits.slice() };");
        self.emit("}");
        self.emit("globalThis.__sigil_breakpoint_snapshot = __sigil_breakpoint_snapshot;");
        self.emit("globalThis.__sigil_breakpoint_recent_trace = __sigil_breakpoint_recent_trace;");
        self.emit("globalThis.__sigil_breakpoint_current_locals_raw = __sigil_breakpoint_current_locals_raw_snapshot;");
        self.emit(
            "globalThis.__sigil_breakpoint_is_stop_signal = __sigil_breakpoint_is_stop_signal;",
        );
    }

    fn emit_expression_helpers(&mut self) {
        self.emit("function __sigil_expression_init_state() {");
        self.emit("  return { stack: [], failure: null };");
        self.emit("}");
        self.emit("function __sigil_expression_state() {");
        self.emit("  if (!globalThis.__sigil_expression_current || typeof globalThis.__sigil_expression_current !== 'object') {");
        self.emit("    globalThis.__sigil_expression_current = __sigil_expression_init_state();");
        self.emit("  }");
        self.emit("  return globalThis.__sigil_expression_current;");
        self.emit("}");
        self.emit("function __sigil_expression_should_ignore_error(error) {");
        self.emit("  return typeof __sigil_breakpoint_is_stop_signal === 'function' ? !!__sigil_breakpoint_is_stop_signal(error) : false;");
        self.emit("}");
        self.emit("function __sigil_expression_locals_snapshot() {");
        self.emit("  if (typeof __sigil_breakpoint_state !== 'function' || typeof __sigil_breakpoint_current_locals !== 'function') return [];");
        self.emit("  try {");
        self.emit("    const state = __sigil_breakpoint_state();");
        self.emit("    return state ? __sigil_breakpoint_current_locals(state) : [];");
        self.emit("  } catch (_expressionLocalsError) {");
        self.emit("    return [];");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_expression_stack_snapshot() {");
        self.emit("  if (typeof __sigil_breakpoint_state !== 'function' || typeof __sigil_breakpoint_stack_snapshot !== 'function') return [];");
        self.emit("  try {");
        self.emit("    const state = __sigil_breakpoint_state();");
        self.emit("    return state ? __sigil_breakpoint_stack_snapshot(state) : [];");
        self.emit("  } catch (_expressionStackError) {");
        self.emit("    return [];");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_expression_snapshot_from_meta(meta, typeId, extras = {}) {");
        self.emit("  const snapshot = { moduleId: String(meta.moduleId ?? ''), sourceFile: String(meta.sourceFile ?? ''), spanId: String(meta.spanId ?? ''), spanKind: meta.spanKind ?? null, declarationKind: meta.declarationKind ?? null, declarationLabel: meta.declarationLabel ?? null, locals: __sigil_expression_locals_snapshot(), stack: __sigil_expression_stack_snapshot() };");
        self.emit("  if (extras && typeof extras === 'object') {");
        self.emit("    if ('value' in extras) snapshot.value = extras.value;");
        self.emit("    if ('error' in extras) snapshot.error = extras.error;");
        self.emit("  }");
        self.emit("  return snapshot;");
        self.emit("}");
        self.emit("function __sigil_expression_record_throw(meta, typeId, error, depth) {");
        self.emit("  const state = __sigil_expression_state();");
        self.emit("  const errorSummary = __sigil_trace_error_summary_typed(error, typeId);");
        self.emit("  if (__sigil_trace_expression_enabled()) {");
        self.emit("    const traceState = __sigil_trace_state();");
        self.emit("    __sigil_trace_push({ kind: 'expr_throw', depth: traceState ? traceState.depth : 0, ...meta, spanKind: String(meta.spanKind ?? ''), error: errorSummary });");
        self.emit("  }");
        self.emit("  if (typeof __sigil_debug_step_emit === 'function') {");
        self.emit("    __sigil_debug_step_emit({ kind: 'expr_throw', ...meta, spanKind: String(meta.spanKind ?? ''), error: errorSummary, expressionDepth: depth });");
        self.emit("  }");
        self.emit("  if (!state.failure || Number(depth) >= Number(state.failure.depth ?? 0)) {");
        self.emit("    state.failure = { depth: Number(depth), snapshot: __sigil_expression_snapshot_from_meta(meta, typeId, { error: errorSummary }) };");
        self.emit("  }");
        self.emit("}");
        self.emit("function __sigil_expression_exception_payload() {");
        self.emit("  const state = __sigil_expression_state();");
        self.emit("  if (state.failure && state.failure.snapshot) return state.failure.snapshot;");
        self.emit("  const current = state.stack[state.stack.length - 1];");
        self.emit("  return current ? __sigil_expression_snapshot_from_meta(current.meta, current.typeId) : null;");
        self.emit("}");
        self.emit("function __sigil_debug_wrap_expression(meta, typeId, thunk, options) {");
        self.emit("  const state = __sigil_expression_state();");
        self.emit("  const breakpointAtEntry = !options || options.breakpointAtEntry !== false;");
        self.emit("  state.stack.push({ meta, typeId });");
        self.emit("  if (__sigil_trace_expression_enabled()) {");
        self.emit("    const traceState = __sigil_trace_state();");
        self.emit("    __sigil_trace_push({ kind: 'expr_enter', depth: traceState ? traceState.depth : 0, ...meta, spanKind: String(meta.spanKind ?? '') });");
        self.emit("  }");
        self.emit("  if (typeof __sigil_debug_step_emit === 'function') {");
        self.emit("    __sigil_debug_step_emit({ kind: 'expr_enter', ...meta, spanKind: String(meta.spanKind ?? ''), expressionDepth: state.stack.length });");
        self.emit("  }");
        self.emit("  const fail = (error) => {");
        self.emit("    const depth = state.stack.length;");
        self.emit("    if (!__sigil_expression_should_ignore_error(error)) {");
        self.emit("      __sigil_expression_record_throw(meta, typeId, error, depth);");
        self.emit("    }");
        self.emit("    if (state.stack.length > 0) state.stack.pop();");
        self.emit("    throw error;");
        self.emit("  };");
        self.emit("  try {");
        self.emit(
            "    if (breakpointAtEntry && typeof __sigil_breakpoint_maybe_hit === 'function') {",
        );
        self.emit("      __sigil_breakpoint_maybe_hit(meta);");
        self.emit("    }");
        self.emit("    const result = thunk();");
        self.emit("    const finish = (value) => {");
        self.emit("      if (__sigil_trace_expression_enabled()) {");
        self.emit("        const traceState = __sigil_trace_state();");
        self.emit("        __sigil_trace_push({ kind: 'expr_return', depth: traceState ? traceState.depth : 0, ...meta, spanKind: String(meta.spanKind ?? ''), value: __sigil_trace_summary(value, 1) });");
        self.emit("      }");
        self.emit("      if (typeof __sigil_debug_step_emit === 'function') {");
        self.emit("        __sigil_debug_step_emit({ kind: 'expr_return', ...meta, spanKind: String(meta.spanKind ?? ''), value: __sigil_trace_summary_typed(value, 1, typeId), expressionDepth: state.stack.length });");
        self.emit("      }");
        self.emit("      if (state.stack.length > 0) state.stack.pop();");
        self.emit("      return value;");
        self.emit("    };");
        self.emit("    if (result && typeof result.then === 'function') {");
        self.emit("      return result.then((value) => finish(value), (error) => fail(error));");
        self.emit("    }");
        self.emit("    return finish(result);");
        self.emit("  } catch (error) {");
        self.emit("    return fail(error);");
        self.emit("  }");
        self.emit("}");
        self.emit("globalThis.__sigil_expression_exception_payload = __sigil_expression_exception_payload;");
    }

    fn generate_declaration(&mut self, decl: &TypedDeclaration) -> Result<(), CodegenError> {
        match decl {
            TypedDeclaration::Function(func) => self.generate_function(func),
            TypedDeclaration::Type(type_decl) => self.generate_type_decl(&type_decl.ast),
            TypedDeclaration::Const(const_decl) => self.generate_const(const_decl),
            TypedDeclaration::Extern(extern_decl) => self.generate_extern(&extern_decl.ast),
            TypedDeclaration::Test(test) => self.generate_test(test),
        }
    }

    fn generate_function(&mut self, func: &TypedFunctionDecl) -> Result<(), CodegenError> {
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/core/map.lib.sigil"))
        {
            if self.generate_core_map_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/string.lib.sigil"))
        {
            if self.generate_stdlib_string_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/httpClient.lib.sigil"))
        {
            if self.generate_stdlib_http_client_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/httpServer.lib.sigil"))
        {
            if self.generate_stdlib_http_server_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/tcpClient.lib.sigil"))
        {
            if self.generate_stdlib_tcp_client_function(func)? {
                return Ok(());
            }
        }
        if self
            .source_file
            .as_deref()
            .is_some_and(|path| path.ends_with("language/stdlib/tcpServer.lib.sigil"))
        {
            if self.generate_stdlib_tcp_server_function(func)? {
                return Ok(());
            }
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let func_name = sanitize_js_identifier(&func.name);

        // Export logic:
        // - .lib.sigil files: export all functions
        // - .sigil files: export main() only (for executables)
        let should_export = if self.should_export_from_lib() {
            true
        } else {
            func.name == "main"
        };

        let fn_keyword = if should_export {
            "export function"
        } else {
            "function"
        };
        let coverage_module_id = self
            .module_id
            .as_deref()
            .unwrap_or("<unknown>")
            .replace('\"', "\\\"");
        let coverage_function_name = func.name.replace('\"', "\\\"");
        let function_span_id = self
            .span_id_for_expr(DebugSpanKind::FunctionDecl, func.location)
            .map(str::to_string);

        self.emit(&format!("{} {}({}) {{", fn_keyword, func_name, params_str));
        self.indent += 1;

        self.emit(&format!(
            "__sigil_record_coverage_call(\"{}\", \"{}\");",
            coverage_module_id, coverage_function_name
        ));
        let body_code = self.with_trace_owner("function_decl", func.name.clone(), |generator| {
            generator.generate_expression(&func.body)
        })?;
        let traced_body = self.trace_declared_return(
            &func.name,
            &format!(
                "[{}]",
                func.params
                    .iter()
                    .map(|param| self.json_string_literal(&param.name))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
            &format!(
                "[{}]",
                func.params
                    .iter()
                    .map(|param| {
                        let type_id = param
                            .type_annotation
                            .as_ref()
                            .and_then(|typ| self.named_type_id_for_surface_type(typ));
                        self.json_string_or_null(type_id.as_deref())
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
            &format!("[{}]", params_str),
            function_span_id.as_deref(),
            &format!(
                "__sigil_record_coverage_result(\"{}\", \"{}\", {})",
                coverage_module_id, coverage_function_name, body_code
            ),
        )?;
        self.emit(&format!("return {};", traced_body));

        self.indent -= 1;
        self.emit("}");

        Ok(())
    }

    fn generate_core_map_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        let body = match (func.name.as_str(), params.as_slice()) {
            ("empty", []) => Some("__sigil_ready(__sigil_map_empty())".to_string()),
            ("entries", [map]) => Some(format!(
                "{}.then((__map) => __sigil_map_entries(__map).map(([__key, __value]) => ({{ key: __key, value: __value }})))",
                self.js_ready(map)
            )),
            ("filter", [map, pred]) => Some(format!(
                "{}.then(async ([__map, __fn]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ if (await Promise.resolve(__fn(__key, __value))) {{ __current = __sigil_map_insert(__current, __key, __value); }} }} return __current; }})",
                self.js_all(&[self.js_ready(map), self.js_ready(pred)])
            )),
            ("fold", [fn_name, init, map]) => Some(format!(
                "{}.then(async ([__fn, __acc, __map]) => {{ let __current = __acc; for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = await Promise.resolve(__fn(__current, __key, __value)); }} return __current; }})",
                self.js_all(&[self.js_ready(fn_name), self.js_ready(init), self.js_ready(map)])
            )),
            ("fromList", [entries]) => Some(format!(
                "{}.then((__entries) => __sigil_map_from_entries(__entries.map((__entry) => [__entry.key, __entry.value])))",
                self.js_ready(entries)
            )),
            ("get", [key, map]) => Some(format!(
                "{}.then(([__key, __map]) => __sigil_map_get(__map, __key))",
                self.js_all(&[self.js_ready(key), self.js_ready(map)])
            )),
            ("has", [key, map]) => Some(format!(
                "{}.then(([__key, __map]) => __sigil_map_has(__map, __key))",
                self.js_all(&[self.js_ready(key), self.js_ready(map)])
            )),
            ("insert", [key, map, value]) => Some(format!(
                "{}.then(([__key, __map, __value]) => __sigil_map_insert(__map, __key, __value))",
                self.js_all(&[
                    self.js_ready(key),
                    self.js_ready(map),
                    self.js_ready(value),
                ])
            )),
            ("keys", [map]) => Some(format!(
                "{}.then((__map) => __sigil_map_entries(__map).map(([__key]) => __key))",
                self.js_ready(map)
            )),
            ("mapValues", [fn_name, map]) => Some(format!(
                "{}.then(async ([__fn, __map]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = __sigil_map_insert(__current, __key, await Promise.resolve(__fn(__value))); }} return __current; }})",
                self.js_all(&[self.js_ready(fn_name), self.js_ready(map)])
            )),
            ("merge", [left, right]) => Some(format!(
                "{}.then(([__left, __right]) => {{ let __current = __sigil_map_from_entries(__sigil_map_entries(__left)); for (const [__key, __value] of __sigil_map_entries(__right)) {{ __current.__sigil_map = __sigil_map_insert(__current, __key, __value).__sigil_map; }} return __current; }})",
                self.js_all(&[self.js_ready(left), self.js_ready(right)])
            )),
            ("remove", [key, map]) => Some(format!(
                "{}.then(([__key, __map]) => __sigil_map_remove(__map, __key))",
                self.js_all(&[self.js_ready(key), self.js_ready(map)])
            )),
            ("singleton", [key, value]) => Some(format!(
                "{}.then(([__key, __value]) => __sigil_map_insert(__sigil_map_empty(), __key, __value))",
                self.js_all(&[self.js_ready(key), self.js_ready(value)])
            )),
            ("size", [map]) => Some(format!(
                "{}.then((__map) => __map.__sigil_map.length)",
                self.js_ready(map)
            )),
            ("values", [map]) => Some(format!(
                "{}.then((__map) => __sigil_map_entries(__map).map(([_, __value]) => __value))",
                self.js_ready(map)
            )),
            _ => None,
        };

        let Some(body) = body else {
            return Ok(false);
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!("return {};", body));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_string_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let fn_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        let body = match (func.name.as_str(), params.as_slice()) {
            ("charAt", [idx, s]) => Some(format!(
                "{}.then(([__index, __string]) => __sigil_ready(__string.charAt(__index)))",
                self.js_all(&[self.js_ready(idx), self.js_ready(s)])
            )),
            ("contains", [s, search]) => Some(format!(
                "{}.then(([__string, __needle]) => __string.includes(__needle))",
                self.js_all(&[self.js_ready(s), self.js_ready(search)])
            )),
            ("endsWith", [s, suffix]) => Some(format!(
                "{}.then(([__string, __suffix]) => __string.endsWith(__suffix))",
                self.js_all(&[self.js_ready(s), self.js_ready(suffix)])
            )),
            ("indexOf", [s, search]) => Some(format!(
                "{}.then(([__string, __needle]) => __string.indexOf(__needle))",
                self.js_all(&[self.js_ready(s), self.js_ready(search)])
            )),
            ("intToString", [n]) => Some(format!("{}.then((__value) => String(__value))", self.js_ready(n))),
            ("isDigit", [s]) => Some(format!(
                "{}.then((__value) => /^[0-9]$/.test(__value))",
                self.js_ready(s)
            )),
            ("join", [separator, strings]) => Some(format!(
                "{}.then(([__separator, __items]) => __items.join(__separator))",
                self.js_all(&[self.js_ready(separator), self.js_ready(strings)])
            )),
            ("replaceAll", [pattern, replacement, s]) => Some(format!(
                "{}.then(([__search, __replacement, __string]) => __string.replaceAll(__search, __replacement))",
                self.js_all(&[
                    self.js_ready(pattern),
                    self.js_ready(replacement),
                    self.js_ready(s),
                ])
            )),
            ("reverse", [s]) => Some(format!(
                "{}.then((__value) => __sigil_ready(__value.split(\"\").reverse().join(\"\")))",
                self.js_ready(s)
            )),
            ("split", [delimiter, s]) => Some(format!(
                "{}.then(([__separator, __string]) => __string.split(__separator))",
                self.js_all(&[self.js_ready(delimiter), self.js_ready(s)])
            )),
            ("startsWith", [prefix, s]) => Some(format!(
                "{}.then(([__prefix, __string]) => __string.startsWith(__prefix))",
                self.js_all(&[self.js_ready(prefix), self.js_ready(s)])
            )),
            ("substring", [end, s, start]) => Some(format!(
                "{}.then(([__end, __string, __start]) => __sigil_ready(__string.substring(__start, __end)))",
                self.js_all(&[self.js_ready(end), self.js_ready(s), self.js_ready(start)])
            )),
            ("toLower", [s]) => Some(format!("{}.then((__value) => __value.toLowerCase())", self.js_ready(s))),
            ("toUpper", [s]) => Some(format!("{}.then((__value) => __value.toUpperCase())", self.js_ready(s))),
            ("trimEndChars", [chars, s]) => Some(
                self.generate_string_trim_chars_js(&self.js_ready(chars), &self.js_ready(s), false),
            ),
            ("trimStartChars", [chars, s]) => Some(
                self.generate_string_trim_chars_js(&self.js_ready(chars), &self.js_ready(s), true),
            ),
            ("trim", [s]) => Some(format!("{}.then((__value) => __value.trim())", self.js_ready(s))),
            _ => None,
        };

        let Some(body) = body else {
            return Ok(false);
        };

        let coverage_module_id = self
            .module_id
            .as_deref()
            .unwrap_or("<unknown>")
            .replace('\"', "\\\"");
        let coverage_function_name = func.name.replace('\"', "\\\"");
        let function_span_id = self
            .span_id_for_expr(DebugSpanKind::FunctionDecl, func.location)
            .map(str::to_string);

        self.emit(&format!(
            "{} {}({}) {{",
            fn_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "__sigil_record_coverage_call(\"{}\", \"{}\");",
            coverage_module_id, coverage_function_name
        ));
        let traced_body = self.trace_declared_return(
            &func.name,
            &format!(
                "[{}]",
                func.params
                    .iter()
                    .map(|param| self.json_string_literal(&param.name))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
            &format!(
                "[{}]",
                func.params
                    .iter()
                    .map(|param| {
                        let type_id = param
                            .type_annotation
                            .as_ref()
                            .and_then(|typ| self.named_type_id_for_surface_type(typ));
                        self.json_string_or_null(type_id.as_deref())
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ")
            ),
            &format!("[{}]", params_str),
            function_span_id.as_deref(),
            &format!(
                "__sigil_record_coverage_result(\"{}\", \"{}\", {})",
                coverage_module_id, coverage_function_name, body
            ),
        )?;
        self.emit(&format!("return {};", traced_body));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_string_trim_chars_js(
        &self,
        chars_expr: &str,
        string_expr: &str,
        trim_start: bool,
    ) -> String {
        let loop_body = if trim_start {
            "while (__start < __end && __chars.includes(__string.charAt(__start))) {
        __start += 1;
      }"
        } else {
            "while (__end > __start && __chars.includes(__string.charAt(__end - 1))) {
        __end -= 1;
      }"
        };

        format!(
            "{}.then(([__chars, __string]) => {{
      if (__chars.length === 0 || __string.length === 0) {{
        return __sigil_ready(__string);
      }}
      let __start = 0;
      let __end = __string.length;
      {}
      return __sigil_ready(__string.substring(__start, __end));
    }})",
            self.js_all(&[chars_expr.to_string(), string_expr.to_string()]),
            loop_body
        )
    }

    fn generate_stdlib_http_client_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "request" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then((__request) => __sigil_world_http_request(__request));",
            self.js_ready(&params[0])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_http_server_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "serve" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then(([__handler, __port]) => __sigil_http_serve(__handler, __port));",
            self.js_all(&[self.js_ready(&params[0]), self.js_ready(&params[1])])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_tcp_client_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "request" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then((__request) => __sigil_world_tcp_request(__request));",
            self.js_ready(&params[0])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_stdlib_tcp_server_function(
        &mut self,
        func: &TypedFunctionDecl,
    ) -> Result<bool, CodegenError> {
        if func.name != "serve" {
            return Ok(false);
        }

        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let export_keyword = if self.should_export_from_lib() {
            "export function"
        } else {
            "function"
        };

        self.emit(&format!(
            "{} {}({}) {{",
            export_keyword,
            sanitize_js_identifier(&func.name),
            params_str
        ));
        self.indent += 1;
        self.emit(&format!(
            "return {}.then(([__handler, __port]) => __sigil_tcp_serve(__handler, __port));",
            self.js_all(&[self.js_ready(&params[0]), self.js_ready(&params[1])])
        ));
        self.indent -= 1;
        self.emit("}");
        Ok(true)
    }

    fn generate_type_decl(&mut self, type_decl: &TypeDecl) -> Result<(), CodegenError> {
        // Generate constructor functions for sum types
        if let TypeDef::Sum(sum_type) = &type_decl.definition {
            let type_params = if type_decl.type_params.is_empty() {
                String::new()
            } else {
                format!("[{}]", type_decl.type_params.join(","))
            };
            self.emit(&format!("// type {}{}", type_decl.name, type_params));

            for variant in &sum_type.variants {
                // Generate constructor function
                // Example: Some(x) => { __tag: "Some", __fields: [x] }
                let param_names: Vec<String> = (0..variant.types.len())
                    .map(|i| format!("_{}", i))
                    .collect();
                let params = param_names.join(", ");

                // Export constructors from .lib.sigil files
                let ctor_keyword = if self.should_export_from_lib() {
                    "export function"
                } else {
                    "function"
                };

                self.emit(&format!(
                    "{} {}({}) {{",
                    ctor_keyword,
                    sanitize_js_identifier(&variant.name),
                    params
                ));
                self.indent += 1;
                if param_names.is_empty() {
                    self.emit(&format!(
                        "return __sigil_ready({{ __tag: \"{}\", __fields: [] }});",
                        variant.name
                    ));
                } else {
                    self.emit(&format!(
                        "return {}.then((__fields) => ({{ __tag: \"{}\", __fields }}));",
                        self.js_all(&param_names),
                        variant.name
                    ));
                }
                self.indent -= 1;
                self.emit("}");
            }
        } else {
            // Product types and type aliases are erased for now
            self.emit(&format!("// type {} (erased)", type_decl.name));
        }

        Ok(())
    }

    fn generate_const(&mut self, const_decl: &TypedConstDecl) -> Result<(), CodegenError> {
        let value = self.with_trace_owner("const_decl", const_decl.name.clone(), |generator| {
            generator.generate_expression(&const_decl.value)
        })?;
        let should_export = self.should_export_from_lib()
            || const_decl.name == "world"
            || matches!(
                &const_decl.typ,
                InferenceType::Constructor(tcons)
                    if tcons.name == "World"
                        || tcons.name.ends_with(".Environment")
                        || tcons.name.ends_with(".FsRoot")
                        || tcons.name.ends_with(".HttpServiceDependency")
                        || tcons.name.ends_with(".LogSink")
                        || tcons.name.ends_with(".ProcessHandle")
                        || tcons.name.ends_with(".TcpServiceDependency")
            );
        self.emit(&format!(
            "{}const {} = {};",
            if should_export { "export " } else { "" },
            sanitize_js_identifier(&const_decl.name),
            value
        ));
        Ok(())
    }

    fn emit_module_import(&mut self, module_id: &str) -> Result<(), CodegenError> {
        let module_path = module_id.split("::").collect::<Vec<_>>();
        let namespace = sanitize_js_identifier(&module_path.join("_"));
        let target_module_id =
            remap_package_local_runtime_module_id(self.module_id.as_deref(), module_id)
                .unwrap_or_else(|| module_id.to_string());
        let import_path = if let Some(ref output_file) = self.output_file {
            let output_path = Path::new(output_file);
            if let Some(local_root) = find_output_root(output_path) {
                let target_abs = local_root
                    .join(target_module_id.replace("::", "/"))
                    .with_extension(&self.import_extension);
                relative_import_path(
                    output_path.parent().unwrap_or_else(|| Path::new(".")),
                    &target_abs,
                )
            } else {
                format!(
                    "./{}.{}",
                    target_module_id.replace("::", "/"),
                    self.import_extension
                )
            }
        } else {
            format!(
                "./{}.{}",
                target_module_id.replace("::", "/"),
                self.import_extension
            )
        };

        self.emit(&format!(
            "import * as {} from '{}';",
            namespace, import_path
        ));
        self.output.push("\n".to_string());
        Ok(())
    }

    fn generate_extern(&mut self, extern_decl: &ExternDecl) -> Result<(), CodegenError> {
        let namespace = extern_decl.module_path.join("::");
        // Only runtime-backed internal stdlib wrappers suppress imports here.
        // Keep this list aligned with the alias-style normalization in
        // `generate_extern_call`. Other intrinsic namespaces such as
        // `stdlib::featureFlags`, `stdlib::crypto`, `stdlib::float`,
        // `stdlib::regex`, and `stdlib::url` intentionally keep their
        // canonical module ids because they are not suppressed alias wrappers.
        if matches!(
            namespace.as_str(),
            "stdlib::cli"
                | "stdlib::file"
                | "stdlib::fsWatch"
                | "stdlib::httpClient"
                | "stdlib::httpServer"
                | "stdlib::io"
                | "stdlib::log"
                | "stdlib::process"
                | "stdlib::pty"
                | "stdlib::random"
                | "stdlib::sql"
                | "stdlib::stream"
                | "stdlib::task"
                | "stdlib::tcpClient"
                | "stdlib::tcpServer"
                | "stdlib::terminal"
                | "stdlib::time"
                | "stdlib::timer"
                | "stdlib::websocket"
                | "stdlibCli"
                | "stdlibFile"
                | "stdlibFsWatch"
                | "stdlibHttpClient"
                | "stdlibHttpServer"
                | "stdlibIo"
                | "stdlibLog"
                | "stdlibProcess"
                | "stdlibPty"
                | "stdlibRandom"
                | "stdlibSql"
                | "stdlibStream"
                | "stdlibTask"
                | "stdlibTcpClient"
                | "stdlibTcpServer"
                | "stdlibTerminal"
                | "stdlibTime"
                | "stdlibTimer"
                | "stdlibWebSocket"
        ) {
            return Ok(());
        }

        let import_path = self.extern_import_path(extern_decl)?;
        if self.lazy_extern_namespaces {
            let namespace = sanitize_js_identifier(&extern_decl.module_path.join("_"));
            let namespace_label_json =
                serde_json::to_string(&extern_decl.module_path.join("::")).unwrap();
            let module_path_json = serde_json::to_string(&import_path).unwrap();
            self.emit(&format!(
                "const {} = __sigil_runtime_extern_namespace({}, {});",
                namespace, namespace_label_json, module_path_json
            ));
            return Ok(());
        }

        // Extern declarations become ES module imports
        if extern_decl
            .members
            .as_ref()
            .map(|members: &Vec<_>| !members.is_empty())
            .unwrap_or(false)
        {
            // Typed extern with declared members: import only those members
            if let Some(members) = &extern_decl.members {
                let imports = members
                    .iter()
                    .map(|member| member.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.emit(&format!("import {{ {} }} from '{}';", imports, import_path));
            }
        } else {
            // Untyped extern or no declared members: namespace import
            let namespace = sanitize_js_identifier(&extern_decl.module_path.join("_"));
            self.emit(&format!(
                "import * as {} from '{}';",
                namespace, import_path
            ));
        }

        Ok(())
    }

    fn extern_import_path(&self, extern_decl: &ExternDecl) -> Result<String, CodegenError> {
        let module_path = extern_decl.module_path.join("/");
        if !extern_decl
            .module_path
            .first()
            .is_some_and(|segment| segment == "bridge")
        {
            return Ok(module_path);
        }

        let project_root = self
            .source_file
            .as_deref()
            .and_then(find_project_root_for_path)
            .or_else(|| {
                self.output_file
                    .as_deref()
                    .and_then(find_project_root_for_path)
            })
            .ok_or_else(|| {
                CodegenError::General(
                    "bridge:: externs require a Sigil project root with sigil.json".to_string(),
                )
            })?;
        let output_file = self.output_file.as_deref().ok_or_else(|| {
            CodegenError::General(
                "bridge:: externs require an output file to compute a relative import path"
                    .to_string(),
            )
        })?;
        let bridge_segments = extern_decl.module_path.iter().skip(1).collect::<Vec<_>>();
        if bridge_segments.is_empty() {
            return Err(CodegenError::General(
                "bridge:: externs must reference at least one bridge module segment".to_string(),
            ));
        }

        let bridge_target = bridge_segments
            .iter()
            .fold(project_root.join("bridges"), |acc, segment| {
                acc.join(segment)
            });
        let target_abs = bridge_target.with_extension(&self.import_extension);
        let output_path = Path::new(output_file);
        Ok(relative_import_path(
            output_path.parent().unwrap_or_else(|| Path::new(".")),
            &target_abs,
        ))
    }

    fn generate_test(&mut self, test: &TypedTestDecl) -> Result<(), CodegenError> {
        // Generate a unique test name from the description
        let test_name = test
            .description
            .chars()
            .filter(|c: &char| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .to_lowercase();
        let test_name = if test_name.is_empty() {
            format!("test_{}", self.test_meta_entries.len())
        } else {
            test_name
        };

        // Generate test function
        self.emit(&format!("async function __test_{}() {{", test_name));
        self.indent += 1;
        let mut world_binding_names = Vec::new();
        for binding in &test.world_bindings {
            let binding_name = sanitize_js_identifier(&binding.name);
            let binding_value =
                self.with_trace_owner("test_decl", test.description.clone(), |generator| {
                    generator.generate_expression(&binding.value)
                })?;
            self.emit(&format!(
                "const {} = await {};",
                binding_name, binding_value
            ));
            world_binding_names.push(binding_name);
        }
        let body = self.with_trace_owner("test_decl", test.description.clone(), |generator| {
            generator.generate_expression(&test.body)
        })?;
        self.emit(&format!(
            "return await __sigil_run_test_world([{}], async () => {});",
            world_binding_names.join(", "),
            body
        ));
        self.indent -= 1;
        self.emit("}");

        // Add to test metadata
        let source_file = self.source_file.as_deref().unwrap_or("<unknown>");
        let module_id = self.module_id.as_deref().unwrap_or("<unknown>");
        let test_span_id = self
            .span_id_for_expr(DebugSpanKind::TestDecl, test.location)
            .unwrap_or("");
        let source_file_json = serde_json::to_string(source_file).map_err(|e| {
            CodegenError::General(format!("Failed to JSON-encode source file: {}", e))
        })?;
        let module_id_json = serde_json::to_string(module_id).map_err(|e| {
            CodegenError::General(format!("Failed to JSON-encode module id: {}", e))
        })?;
        let span_id_json = serde_json::to_string(test_span_id).map_err(|e| {
            CodegenError::General(format!("Failed to JSON-encode test span id: {}", e))
        })?;
        let span_kind_json = self.span_kind_literal(DebugSpanKind::TestDecl)?;
        let test_id = format!("{}::{}", source_file, test.description);
        let test_id_json = serde_json::to_string(&test_id)
            .map_err(|e| CodegenError::General(format!("Failed to JSON-encode test id: {}", e)))?;
        let description_json = serde_json::to_string(&test.description).map_err(|e| {
            CodegenError::General(format!("Failed to JSON-encode test description: {}", e))
        })?;
        self.test_meta_entries.push(format!(
            "{{ id: {}, name: {}, description: {}, moduleId: {}, sourceFile: {}, spanId: {}, spanKind: {}, location: {{ start: {{ line: {}, column: {} }} }}, fn: __test_{} }}",
            &test_id_json,
            &description_json,
            &description_json,
            &module_id_json,
            &source_file_json,
            &span_id_json,
            &span_kind_json,
            test.location.start.line,
            test.location.start.column,
            test_name
        ));

        Ok(())
    }

    fn generate_expression(&mut self, expr: &TypedExpr) -> Result<String, CodegenError> {
        let generated = match &expr.kind {
            TypedExprKind::Literal(lit) => self.generate_literal(lit),
            TypedExprKind::Identifier(id) => Ok(self.js_ready(&sanitize_js_identifier(&id.name))),
            TypedExprKind::NamespaceMember { namespace, member } => Ok(self.js_ready(&format!(
                "{}.{}",
                sanitize_js_identifier(&namespace.join("_")),
                sanitize_js_identifier(member)
            ))),
            TypedExprKind::Lambda(lambda) => self.generate_lambda(lambda),
            TypedExprKind::Call(call) => self.generate_call(expr, call),
            TypedExprKind::ConstructorCall(call) => self.generate_constructor_call(call),
            TypedExprKind::ExternCall(call) => self.generate_extern_call(expr, call),
            TypedExprKind::MethodCall(call) => self.generate_method_call(call),
            TypedExprKind::Binary(bin) => self.generate_binary(bin),
            TypedExprKind::Unary(un) => self.generate_unary(un),
            TypedExprKind::Match(match_expr) => self.generate_match(expr, match_expr),
            TypedExprKind::Let(let_expr) => self.generate_let(expr, let_expr),
            TypedExprKind::Using(using_expr) => self.generate_using(expr, using_expr),
            TypedExprKind::If(if_expr) => self.generate_if(expr, if_expr),
            TypedExprKind::List(list) => self.generate_list(list),
            TypedExprKind::Tuple(tuple) => self.generate_tuple(tuple),
            TypedExprKind::Record(record) => self.generate_record(record),
            TypedExprKind::MapLiteral(map) => self.generate_map_literal(map),
            TypedExprKind::FieldAccess(field_access) => self.generate_field_access(field_access),
            TypedExprKind::Index(index) => self.generate_index(index),
            TypedExprKind::Map(map) => self.generate_map(map),
            TypedExprKind::Filter(filter) => self.generate_filter(filter),
            TypedExprKind::Fold(fold) => self.generate_fold(fold),
            TypedExprKind::Concurrent(concurrent) => self.generate_concurrent(concurrent),
            TypedExprKind::Pipeline(pipeline) => self.generate_pipeline(pipeline),
        }?;

        if !self.breakpoints_enabled && !self.expression_debug_enabled {
            return Ok(generated);
        }

        let wrapped = match &expr.kind {
            TypedExprKind::Literal(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprLiteral, generated, true)
            }
            TypedExprKind::Identifier(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprIdentifier, generated, true)
            }
            TypedExprKind::NamespaceMember { .. } => self.wrap_expression_debug(
                expr,
                DebugSpanKind::ExprNamespaceMember,
                generated,
                true,
            ),
            TypedExprKind::Lambda(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprLambda, generated, true)
            }
            TypedExprKind::Call(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprCall, generated, true)
            }
            TypedExprKind::ConstructorCall(_) => self.wrap_expression_debug(
                expr,
                DebugSpanKind::ExprConstructorCall,
                generated,
                true,
            ),
            TypedExprKind::ExternCall(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprExternCall, generated, true)
            }
            TypedExprKind::MethodCall(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprMethodCall, generated, true)
            }
            TypedExprKind::Binary(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprBinary, generated, true)
            }
            TypedExprKind::Unary(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprUnary, generated, true)
            }
            TypedExprKind::Match(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprMatch, generated, true)
            }
            TypedExprKind::Let(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprLet, generated, false)
            }
            TypedExprKind::Using(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprLet, generated, false)
            }
            TypedExprKind::If(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprIf, generated, true)
            }
            TypedExprKind::List(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprList, generated, true)
            }
            TypedExprKind::Tuple(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprTuple, generated, true)
            }
            TypedExprKind::Record(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprRecord, generated, true)
            }
            TypedExprKind::MapLiteral(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprMapLiteral, generated, true)
            }
            TypedExprKind::FieldAccess(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprFieldAccess, generated, true)
            }
            TypedExprKind::Index(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprIndex, generated, true)
            }
            TypedExprKind::Map(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprMap, generated, true)
            }
            TypedExprKind::Filter(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprFilter, generated, true)
            }
            TypedExprKind::Fold(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprFold, generated, true)
            }
            TypedExprKind::Concurrent(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprConcurrent, generated, true)
            }
            TypedExprKind::Pipeline(_) => {
                self.wrap_expression_debug(expr, DebugSpanKind::ExprPipeline, generated, true)
            }
        }?;
        Ok(wrapped)
    }

    fn generate_literal(&mut self, lit: &LiteralExpr) -> Result<String, CodegenError> {
        let value = match &lit.value {
            LiteralValue::Int(n) => n.to_string(),
            LiteralValue::Float(f) => f.to_string(),
            LiteralValue::String(s) => serde_json::to_string(s).unwrap(),
            LiteralValue::Char(c) => serde_json::to_string(&c.to_string()).unwrap(),
            LiteralValue::Bool(b) => b.to_string(),
            LiteralValue::Unit => "null".to_string(),
        };
        Ok(self.js_ready(&value))
    }

    fn generate_lambda(&mut self, lambda: &TypedLambdaExpr) -> Result<String, CodegenError> {
        let params: Vec<String> = lambda
            .params
            .iter()
            .map(|p| sanitize_js_identifier(&p.name))
            .collect();
        let params_str = params.join(", ");
        let body = self.generate_expression(&lambda.body)?;
        Ok(format!("(({}) => {})", params_str, body))
    }

    fn generate_call(
        &mut self,
        expr: &TypedExpr,
        call: &TypedCallExpr,
    ) -> Result<String, CodegenError> {
        if let Some(intrinsic) = self.try_generate_typed_intrinsic(expr, &call.func, &call.args)? {
            return Ok(intrinsic);
        }

        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;
        match &call.func.kind {
            TypedExprKind::Identifier(id) => Ok(format!(
                "{}.then((__sigil_args) => __sigil_call(\"{}\", {}, __sigil_args))",
                self.js_all(&args),
                id.name,
                sanitize_js_identifier(&id.name)
            )),
            TypedExprKind::NamespaceMember { namespace, member } => {
                let func_ref = format!(
                    "{}.{}",
                    sanitize_js_identifier(&namespace.join("_")),
                    sanitize_js_identifier(member)
                );
                let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, expr.location);
                let namespace_name = namespace.join("::");
                Ok(format!(
                    "{}.then((__sigil_args) => {})",
                    self.js_all(&args),
                    self.wrap_effect_trace(
                        span_id,
                        if namespace_name.is_empty() {
                            "extern"
                        } else {
                            &namespace_name
                        },
                        member,
                        "__sigil_args",
                        &format!(
                            "__sigil_call(\"extern:{}.{}\", {}, __sigil_args)",
                            namespace.join("/"),
                            member,
                            func_ref
                        ),
                        None,
                    )?
                ))
            }
            _ => {
                let func = self.generate_expression(&call.func)?;
                let mut values = vec![func];
                values.extend(args);
                Ok(format!(
                    "{}.then(([__sigil_fn, ...__sigil_args]) => __sigil_fn(...__sigil_args))",
                    self.js_all(&values)
                ))
            }
        }
    }

    fn try_generate_typed_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        func: &TypedExpr,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        match &func.kind {
            TypedExprKind::NamespaceMember { namespace, member } => {
                let module = namespace.join("/");
                if module == "stdlib/string" {
                    return self.generate_string_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/json" {
                    return self.generate_json_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/file" {
                    return self.generate_file_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/fsWatch" {
                    return self.generate_fswatch_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/log" {
                    return self.generate_log_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/httpClient" {
                    return self.generate_http_client_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/httpServer" {
                    return self.generate_http_server_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/cli" {
                    return self.generate_cli_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/tcpClient" {
                    return self.generate_tcp_client_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/tcpServer" {
                    return self.generate_tcp_server_intrinsic(call_expr, member, args);
                }
                if module.starts_with("test/observe/") {
                    return self.generate_test_observe_intrinsic(call_expr, &module, member, args);
                }
                if module == "stdlib/time" {
                    return self.generate_time_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/timer" {
                    return self.generate_timer_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/io" {
                    return self.generate_io_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/terminal" {
                    return self.generate_terminal_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/sql" {
                    return self.generate_sql_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/process" {
                    return self.generate_process_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/pty" {
                    return self.generate_pty_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/websocket" {
                    return self.generate_websocket_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/random" {
                    return self.generate_random_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/stream" {
                    return self.generate_stream_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/task" {
                    return self.generate_task_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/featureFlags" {
                    return self.generate_feature_flags_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/crypto" {
                    return self.generate_crypto_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/float" {
                    return self.generate_float_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/regex" {
                    return self.generate_regex_intrinsic(call_expr, member, args);
                }
                if module == "stdlib/url" {
                    return self.generate_url_intrinsic(call_expr, member, args);
                }
                if module == "core/map" {
                    return self.generate_map_intrinsic(call_expr, member, args);
                }
                Ok(None)
            }
            TypedExprKind::Identifier(name) => {
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/string.lib.sigil"))
                {
                    return self.generate_string_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/json.lib.sigil"))
                {
                    return self.generate_json_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/file.lib.sigil"))
                {
                    return self.generate_file_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/fsWatch.lib.sigil"))
                {
                    return self.generate_fswatch_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/cli.lib.sigil"))
                {
                    return self.generate_cli_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/httpClient.lib.sigil"))
                {
                    return self.generate_http_client_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/httpServer.lib.sigil"))
                {
                    return self.generate_http_server_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/tcpClient.lib.sigil"))
                {
                    return self.generate_tcp_client_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/tcpServer.lib.sigil"))
                {
                    return self.generate_tcp_server_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/time.lib.sigil"))
                {
                    return self.generate_time_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/timer.lib.sigil"))
                {
                    return self.generate_timer_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/io.lib.sigil"))
                {
                    return self.generate_io_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/log.lib.sigil"))
                {
                    return self.generate_log_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/terminal.lib.sigil"))
                {
                    return self.generate_terminal_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/sql.lib.sigil"))
                {
                    return self.generate_sql_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/process.lib.sigil"))
                {
                    return self.generate_process_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/pty.lib.sigil"))
                {
                    return self.generate_pty_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/websocket.lib.sigil"))
                {
                    return self.generate_websocket_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/random.lib.sigil"))
                {
                    return self.generate_random_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/stream.lib.sigil"))
                {
                    return self.generate_stream_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/task.lib.sigil"))
                {
                    return self.generate_task_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/featureFlags.lib.sigil"))
                {
                    return self.generate_feature_flags_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/crypto.lib.sigil"))
                {
                    return self.generate_crypto_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/float.lib.sigil"))
                {
                    return self.generate_float_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.contains("/language/test/observe/"))
                {
                    let module = self
                        .source_file
                        .as_deref()
                        .unwrap()
                        .split("language/")
                        .nth(1)
                        .unwrap()
                        .trim_end_matches(".lib.sigil")
                        .replace(".sigil", "")
                        .replace('/', "::");
                    return self.generate_test_observe_intrinsic(
                        call_expr,
                        &module.replace("::", "/"),
                        &name.name,
                        args,
                    );
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/regex.lib.sigil"))
                {
                    return self.generate_regex_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/stdlib/url.lib.sigil"))
                {
                    return self.generate_url_intrinsic(call_expr, &name.name, args);
                }
                if self
                    .source_file
                    .as_deref()
                    .is_some_and(|path| path.ends_with("language/core/map.lib.sigil"))
                {
                    return self.generate_map_intrinsic(call_expr, &name.name, args);
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn generate_string_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args: Result<Vec<String>, CodegenError> = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect();
        let generated_args = generated_args?;

        match member {
            "charAt" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__index, __string]) => __sigil_ready(__string.charAt(__index)))", self.js_all(&generated_args))))
            }
            "contains" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __needle]) => __string.includes(__needle))", self.js_all(&generated_args))))
            }
            "substring" if generated_args.len() == 3 => {
                Ok(Some(format!("{}.then(([__end, __string, __start]) => __sigil_ready(__string.substring(__start, __end)))", self.js_all(&generated_args))))
            }
            "toUpper" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.toUpperCase())", generated_args[0])))
            }
            "toLower" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.toLowerCase())", generated_args[0])))
            }
            "trimStartChars" if generated_args.len() == 2 => Ok(Some(
                self.generate_string_trim_chars_js(&generated_args[0], &generated_args[1], true),
            )),
            "trimEndChars" if generated_args.len() == 2 => Ok(Some(
                self.generate_string_trim_chars_js(&generated_args[0], &generated_args[1], false),
            )),
            "trim" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __value.trim())", generated_args[0])))
            }
            "indexOf" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __needle]) => __string.indexOf(__needle))", self.js_all(&generated_args))))
            }
            "split" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__separator, __string]) => __string.split(__separator))", self.js_all(&generated_args))))
            }
            "reverse" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => __sigil_ready(__value.split(\"\").reverse().join(\"\")))", generated_args[0])))
            }
            "replaceAll" if generated_args.len() == 3 => {
                Ok(Some(format!("{}.then(([__search, __replacement, __string]) => __string.replaceAll(__search, __replacement))", self.js_all(&generated_args))))
            }
            "intToString" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => String(__value))", generated_args[0])))
            }
            "join" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__separator, __items]) => __items.join(__separator))", self.js_all(&generated_args))))
            }
            "take" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__count, __string]) => __string.substring(0, __count))", self.js_all(&generated_args))))
            }
            "drop" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__count, __string]) => __string.substring(__count))", self.js_all(&generated_args))))
            }
            "startsWith" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__prefix, __string]) => __string.startsWith(__prefix))", self.js_all(&generated_args))))
            }
            "endsWith" if generated_args.len() == 2 => {
                Ok(Some(format!("{}.then(([__string, __suffix]) => __string.endsWith(__suffix))", self.js_all(&generated_args))))
            }
            "isDigit" if generated_args.len() == 1 => {
                Ok(Some(format!("{}.then((__value) => /^[0-9]$/.test(__value))", generated_args[0])))
            }
            _ => Ok(None),
        }
    }

    fn generate_map_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "empty" if generated_args.is_empty() => {
                Ok(Some("__sigil_ready(__sigil_map_empty())".to_string()))
            }
            "entries" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __sigil_map_entries(__map).map(([__key, __value]) => ({{ key: __key, value: __value }})))",
                    generated_args[0]
                )))
            }
            "filter" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(async ([__map, __fn]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ if (await Promise.resolve(__fn(__key, __value))) {{ __current = __sigil_map_insert(__current, __key, __value); }} }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "fold" if generated_args.len() == 3 => {
                Ok(Some(format!(
                    "{}.then(async ([__fn, __acc, __map]) => {{ let __current = __acc; for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = await Promise.resolve(__fn(__current, __key, __value)); }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "fromList" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__entries) => __sigil_map_from_entries(__entries.map((__entry) => [__entry.key, __entry.value])))",
                    generated_args[0]
                )))
            }
            "get" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map]) => __sigil_map_get(__map, __key))",
                    self.js_all(&generated_args)
                )))
            }
            "has" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map]) => __sigil_map_has(__map, __key))",
                    self.js_all(&generated_args)
                )))
            }
            "insert" if generated_args.len() == 3 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map, __value]) => __sigil_map_insert(__map, __key, __value))",
                    self.js_all(&generated_args)
                )))
            }
            "keys" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __sigil_map_entries(__map).map(([__key]) => __key))",
                    generated_args[0]
                )))
            }
            "mapValues" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(async ([__fn, __map]) => {{ let __current = __sigil_map_empty(); for (const [__key, __value] of __sigil_map_entries(__map)) {{ __current = __sigil_map_insert(__current, __key, await Promise.resolve(__fn(__value))); }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "merge" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__left, __right]) => {{ let __current = __left; for (const [__key, __value] of __sigil_map_entries(__right)) {{ __current = __sigil_map_insert(__current, __key, __value); }} return __current; }})",
                    self.js_all(&generated_args)
                )))
            }
            "remove" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __map]) => __sigil_map_remove(__map, __key))",
                    self.js_all(&generated_args)
                )))
            }
            "singleton" if generated_args.len() == 2 => {
                Ok(Some(format!(
                    "{}.then(([__key, __value]) => __sigil_map_insert(__sigil_map_empty(), __key, __value))",
                    self.js_all(&generated_args)
                )))
            }
            "size" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __map.__sigil_map.length)",
                    generated_args[0]
                )))
            }
            "values" if generated_args.len() == 1 => {
                Ok(Some(format!(
                    "{}.then((__map) => __sigil_map_entries(__map).map(([_, __value]) => __value))",
                    generated_args[0]
                )))
            }
            _ => Ok(None),
        }
    }

    fn generate_json_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "asArray" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonArray' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asBool" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonBool' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asNumber" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonNumber' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asObject" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonObject' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "asString" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonString' ? {{ __tag: \"Some\", __fields: [__value.__fields[0]] }} : {{ __tag: \"None\", __fields: [] }})",
                generated_args[0]
            ))),
            "getField" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __obj]) => __sigil_map_get(__obj, __key))",
                self.js_all(&generated_args)
            ))),
            "getIndex" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__idx, __arr]) => (__idx >= 0 && __idx < __arr.length) ? {{ __tag: \"Some\", __fields: [__arr[__idx]] }} : {{ __tag: \"None\", __fields: [] }})",
                self.js_all(&generated_args)
            ))),
            "isNull" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __value?.__tag === 'JsonNull')",
                generated_args[0]
            ))),
            "parse" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_json_parse_result(__input))",
                generated_args[0]
            ))),
            "stringify" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__value) => __sigil_json_stringify_value(__value))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_feature_flags_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "entry" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__config, __flag]) => ({{ __sigil_feature_flag_id: String(__flag?.id ?? ''), config: __config }}))",
                self.js_all(&generated_args)
            ))),
            "get" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__context, __flag, __set]) => __sigil_feature_flag_get(__context, __flag, __set))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_file_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "appendText" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__content, __path]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "appendText",
                    "[__content, __path]",
                    "__sigil_world_file_appendText(__content, __path)",
                    None,
                )?
            ))),
            "appendTextAt" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__content, __path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "appendTextAt",
                    "[__content, __path, __root]",
                    "__sigil_world_file_appendTextAt(__root?.__fields?.[0] ?? '', __content, __path)",
                    None,
                )?
            ))),
            "exists" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "exists",
                    "[__path]",
                    "__sigil_world_file_exists(__path)",
                    None,
                )?
            ))),
            "existsAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "existsAt",
                    "[__path, __root]",
                    "__sigil_world_file_existsAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "listDir" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "listDir",
                    "[__path]",
                    "__sigil_world_file_listDir(__path)",
                    None,
                )?
            ))),
            "listDirAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "listDirAt",
                    "[__path, __root]",
                    "__sigil_world_file_listDirAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "makeDir" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "makeDir",
                    "[__path]",
                    "__sigil_world_file_makeDir(__path)",
                    None,
                )?
            ))),
            "makeDirAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "makeDirAt",
                    "[__path, __root]",
                    "__sigil_world_file_makeDirAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "makeDirs" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "makeDirs",
                    "[__path]",
                    "__sigil_world_file_makeDirs(__path)",
                    None,
                )?
            ))),
            "makeDirsAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "makeDirsAt",
                    "[__path, __root]",
                    "__sigil_world_file_makeDirsAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "makeTempDir" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__prefix) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "makeTempDir",
                    "[__prefix]",
                    "__sigil_world_file_makeTempDir(__prefix)",
                    None,
                )?
            ))),
            "makeTempDirAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__prefix, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "makeTempDirAt",
                    "[__prefix, __root]",
                    "__sigil_world_file_makeTempDirAt(__root?.__fields?.[0] ?? '', __prefix)",
                    None,
                )?
            ))),
            "readText" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "readText",
                    "[__path]",
                    "__sigil_world_file_readText(__path)",
                    None,
                )?
            ))),
            "readTextAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "readTextAt",
                    "[__path, __root]",
                    "__sigil_world_file_readTextAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "remove" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "remove",
                    "[__path]",
                    "__sigil_world_file_remove(__path)",
                    None,
                )?
            ))),
            "removeAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "removeAt",
                    "[__path, __root]",
                    "__sigil_world_file_removeAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "removeTree" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "removeTree",
                    "[__path]",
                    "__sigil_world_file_removeTree(__path)",
                    None,
                )?
            ))),
            "removeTreeAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "removeTreeAt",
                    "[__path, __root]",
                    "__sigil_world_file_removeTreeAt(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            "writeText" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__content, __path]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "writeText",
                    "[__content, __path]",
                    "__sigil_world_file_writeText(__content, __path)",
                    None,
                )?
            ))),
            "writeTextAt" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__content, __path, __root]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "file",
                    "writeTextAt",
                    "[__content, __path, __root]",
                    "__sigil_world_file_writeTextAt(__root?.__fields?.[0] ?? '', __content, __path)",
                    None,
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_fswatch_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "close" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__watch) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "fsWatch",
                    "close",
                    "[__watch]",
                    "__sigil_world_fswatch_close(__watch)",
                    None,
                )?
            ))),
            "events" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__watch) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "fsWatch",
                    "events",
                    "[__watch]",
                    "__sigil_world_fswatch_events(__watch)",
                    None,
                )?
            ))),
            "watch" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__path) => {}).then((__watch) => __sigil_owned_wrap(__watch, async () => {{ await __sigil_world_fswatch_close(__watch); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "fsWatch",
                    "watch",
                    "[__path]",
                    "__sigil_world_fswatch_watch(__path)",
                    None,
                )?
            ))),
            "watchAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__path, __root]) => {}).then((__watch) => __sigil_owned_wrap(__watch, async () => {{ await __sigil_world_fswatch_close(__watch); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "fsWatch",
                    "watchAt",
                    "[__path, __root]",
                    "__sigil_world_fswatch_watch_at(__root?.__fields?.[0] ?? '', __path)",
                    None,
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_log_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "write" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__message, __sink]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "log",
                    "write",
                    "[__message, __sink]",
                    "__sigil_world_log_write_to(__sink?.__fields?.[0] ?? '', __message)",
                    None,
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_io_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "debug" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__message) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "log",
                    "debug",
                    "[__message]",
                    "__sigil_world_log_debug(__message)",
                    None
                )?
            ))),
            "eprintln" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__message) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "log",
                    "eprintln",
                    "[__message]",
                    "__sigil_world_log_eprintln(__message)",
                    None
                )?
            ))),
            "print" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__message) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "log",
                    "print",
                    "[__message]",
                    "__sigil_world_log_print(__message)",
                    None
                )?
            ))),
            "println" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__message) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "log",
                    "println",
                    "[__message]",
                    "__sigil_world_log_println(__message)",
                    None
                )?
            ))),
            "warn" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__message) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "log",
                    "warn",
                    "[__message]",
                    "__sigil_world_log_warn(__message)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_test_observe_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        module: &str,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match (module, member, generated_args.len()) {
            ("test/observe/file", "exists", 1) => Ok(Some(format!(
                "{}.then((__path) => __sigil_test_file_exists(__path))",
                generated_args[0]
            ))),
            ("test/observe/file", "existsAt", 2) => Ok(Some(format!(
                "Promise.all([{}, {}]).then(([__path, __root]) => __sigil_test_file_exists_at(__root?.__fields?.[0] ?? '', __path))",
                generated_args[0], generated_args[1]
            ))),
            ("test/observe/file", "listDir", 1) => Ok(Some(format!(
                "{}.then((__path) => __sigil_test_file_list_dir(__path))",
                generated_args[0]
            ))),
            ("test/observe/file", "listDirAt", 2) => Ok(Some(format!(
                "Promise.all([{}, {}]).then(([__path, __root]) => __sigil_test_file_list_dir_at(__root?.__fields?.[0] ?? '', __path))",
                generated_args[0], generated_args[1]
            ))),
            ("test/observe/file", "readText", 1) => Ok(Some(format!(
                "{}.then((__path) => __sigil_test_file_read_text(__path))",
                generated_args[0]
            ))),
            ("test/observe/file", "readTextAt", 2) => Ok(Some(format!(
                "Promise.all([{}, {}]).then(([__path, __root]) => __sigil_test_file_read_text_at(__root?.__fields?.[0] ?? '', __path))",
                generated_args[0], generated_args[1]
            ))),
            ("test/observe/fsWatch", "closeCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_fswatch_close_count())".to_string(),
            )),
            ("test/observe/fsWatch", "closeCountAt", 1) => Ok(Some(format!(
                "{}.then((__root) => __sigil_test_fswatch_close_count_at(__root?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/fsWatch", "events", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_fswatch_events())".to_string(),
            )),
            ("test/observe/fsWatch", "eventsAt", 1) => Ok(Some(format!(
                "{}.then((__root) => __sigil_test_fswatch_events_at(__root?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/fsWatch", "watches", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_fswatch_watches())".to_string(),
            )),
            ("test/observe/fsWatch", "watchesAt", 1) => Ok(Some(format!(
                "{}.then((__root) => __sigil_test_fswatch_watches_at(__root?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/http", "callCount", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_http_requests(__entry).length)",
                generated_args[0]
            ))),
            ("test/observe/http", "lastPath", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_http_last_path(__entry))",
                generated_args[0]
            ))),
            ("test/observe/http", "lastRequest", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_http_last_request(__entry))",
                generated_args[0]
            ))),
            ("test/observe/http", "requests", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_http_requests(__entry))",
                generated_args[0]
            ))),
            ("test/observe/log", "entries", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_log_entries())".to_string(),
            )),
            ("test/observe/log", "entriesAt", 1) => Ok(Some(format!(
                "{}.then((__sink) => __sigil_test_log_entries_at(__sink?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/pty", "closeCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_pty_close_count())".to_string(),
            )),
            ("test/observe/pty", "closeCountAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_pty_close_count_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/pty", "resizes", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_pty_resizes())".to_string(),
            )),
            ("test/observe/pty", "resizesAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_pty_resizes_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/pty", "spawnCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_pty_spawn_count())".to_string(),
            )),
            ("test/observe/pty", "spawnCountAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_pty_spawn_count_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/pty", "spawns", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_pty_spawns())".to_string(),
            )),
            ("test/observe/pty", "spawnsAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_pty_spawns_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/pty", "writes", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_pty_writes())".to_string(),
            )),
            ("test/observe/pty", "writesAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_pty_writes_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/websocket", "closeCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_websocket_close_count())".to_string(),
            )),
            ("test/observe/websocket", "closeCountAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_websocket_close_count_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/websocket", "connectionCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_websocket_connection_count())".to_string(),
            )),
            ("test/observe/websocket", "connectionCountAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_websocket_connection_count_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/websocket", "received", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_websocket_received())".to_string(),
            )),
            ("test/observe/websocket", "receivedAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_websocket_received_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/websocket", "sent", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_websocket_sent())".to_string(),
            )),
            ("test/observe/websocket", "sentAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_websocket_sent_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/process", "callCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_process_call_count())".to_string(),
            )),
            ("test/observe/process", "callCountAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_process_call_count_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/process", "commands", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_process_commands())".to_string(),
            )),
            ("test/observe/process", "commandsAt", 1) => Ok(Some(format!(
                "{}.then((__handle) => __sigil_test_process_commands_at(__handle?.__fields?.[0] ?? ''))",
                generated_args[0]
            ))),
            ("test/observe/tcp", "callCount", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_tcp_requests(__entry).length)",
                generated_args[0]
            ))),
            ("test/observe/tcp", "lastRequest", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_tcp_last_request(__entry))",
                generated_args[0]
            ))),
            ("test/observe/tcp", "requests", 1) => Ok(Some(format!(
                "{}.then((__entry) => __sigil_test_tcp_requests(__entry))",
                generated_args[0]
            ))),
            ("test/observe/time", "currentIso", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_current_iso())".to_string(),
            )),
            ("test/observe/timer", "lastSleepMs", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_timer_last_sleep_ms())".to_string(),
            )),
            ("test/observe/timer", "sleepCount", 0) => Ok(Some(
                "__sigil_ready(__sigil_test_timer_sleep_count())".to_string(),
            )),
            _ => Ok(None),
        }
    }

    fn generate_time_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "compare" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => (__left.epochMillis < __right.epochMillis ? -1 : (__left.epochMillis > __right.epochMillis ? 1 : 0)))",
                self.js_all(&generated_args)
            ))),
            "formatIso" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__instant) => __sigil_time_format_iso(__instant))",
                generated_args[0]
            ))),
            "fromEpochMillis" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__millis) => ({{ epochMillis: __millis }}))",
                generated_args[0]
            ))),
            "isAfter" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => __left.epochMillis > __right.epochMillis)",
                self.js_all(&generated_args)
            ))),
            "isBefore" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => __left.epochMillis < __right.epochMillis)",
                self.js_all(&generated_args)
            ))),
            "now" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "time",
                "now",
                "[]",
                "__sigil_ready(__sigil_world_time_now_instant())",
                None,
            )?)),
            "parseIso" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_time_parse_iso_result(__input))",
                generated_args[0]
            ))),
            "sleepMs" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__ms) => {})",
                generated_args[0],
                self.wrap_effect_trace(span_id, "timer", "sleepMs", "[__ms]", "__sigil_world_timer_sleep(__ms)", None)?
            ))),
            "toEpochMillis" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__instant) => __instant.epochMillis)",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_timer_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "afterMs" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__ms) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "timer",
                    "afterMs",
                    "[__ms]",
                    "__sigil_world_timer_after(__ms)",
                    None
                )?
            ))),
            "everyMs" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__ms) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "timer",
                    "everyMs",
                    "[__ms]",
                    "__sigil_world_timer_every(__ms)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_sql_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "all" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __select]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "all",
                    "[__handle, __select]",
                    "__sigil_world_sql_all(__handle?.__fields?.[0] ?? '', __select)",
                    None
                )?
            ))),
            "allIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__select, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "allIn",
                    "[__select, __transaction]",
                    "__sigil_world_sql_all_in(__select, __transaction)",
                    None
                )?
            ))),
            "and" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => ({{ kind: 'and', left: __left, right: __right }}))",
                self.js_all(&generated_args)
            ))),
            "begin" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__handle) => {}).then(__sigil_sql_begin_wrap)",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "begin",
                    "[__handle]",
                    "__sigil_world_sql_begin(__handle?.__fields?.[0] ?? '')",
                    None
                )?
            ))),
            "boolColumn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__field, __name]) => __sigil_sql_column(__field, __name, 'bool'))",
                self.js_all(&generated_args)
            ))),
            "bytes" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__base64) => ({{ base64: String(__base64 ?? '') }}))",
                generated_args[0]
            ))),
            "bytesColumn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__field, __name]) => __sigil_sql_column(__field, __name, 'bytes'))",
                self.js_all(&generated_args)
            ))),
            "commit" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__transaction) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "commit",
                    "[__transaction]",
                    "__sigil_world_sql_commit(__transaction)",
                    None
                )?
            ))),
            "delete" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__table) => ({{ predicate: null, table: __table }}))",
                generated_args[0]
            ))),
            "deleteWhere" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__predicate, __statement]) => ({{ ...__statement, predicate: __predicate }}))",
                self.js_all(&generated_args)
            ))),
            "eq" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column, __value]) => ({{ kind: 'eq', column: __column, value: __value }}))",
                self.js_all(&generated_args)
            ))),
            "execDelete" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __statement]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "execDelete",
                    "[__handle, __statement]",
                    "__sigil_world_sql_exec_delete(__handle?.__fields?.[0] ?? '', __statement)",
                    None
                )?
            ))),
            "execDeleteIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__statement, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "execDeleteIn",
                    "[__statement, __transaction]",
                    "__sigil_world_sql_exec_delete_in(__statement, __transaction)",
                    None
                )?
            ))),
            "execInsert" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __statement]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "execInsert",
                    "[__handle, __statement]",
                    "__sigil_world_sql_exec_insert(__handle?.__fields?.[0] ?? '', __statement)",
                    None
                )?
            ))),
            "execInsertIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__statement, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "execInsertIn",
                    "[__statement, __transaction]",
                    "__sigil_world_sql_exec_insert_in(__statement, __transaction)",
                    None
                )?
            ))),
            "execUpdate" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __statement]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "execUpdate",
                    "[__handle, __statement]",
                    "__sigil_world_sql_exec_update(__handle?.__fields?.[0] ?? '', __statement)",
                    None
                )?
            ))),
            "execUpdateIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__statement, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "execUpdateIn",
                    "[__statement, __transaction]",
                    "__sigil_world_sql_exec_update_in(__statement, __transaction)",
                    None
                )?
            ))),
            "floatColumn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__field, __name]) => __sigil_sql_column(__field, __name, 'float'))",
                self.js_all(&generated_args)
            ))),
            "gt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column, __value]) => ({{ kind: 'gt', column: __column, value: __value }}))",
                self.js_all(&generated_args)
            ))),
            "gte" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column, __value]) => ({{ kind: 'gte', column: __column, value: __value }}))",
                self.js_all(&generated_args)
            ))),
            "insert" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__row, __table]) => ({{ row: __row, table: __table }}))",
                self.js_all(&generated_args)
            ))),
            "intColumn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__field, __name]) => __sigil_sql_column(__field, __name, 'int'))",
                self.js_all(&generated_args)
            ))),
            "limit" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__count, __select]) => ({{ ...__select, limit: Number(__count ?? 0) }}))",
                self.js_all(&generated_args)
            ))),
            "lt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column, __value]) => ({{ kind: 'lt', column: __column, value: __value }}))",
                self.js_all(&generated_args)
            ))),
            "lte" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column, __value]) => ({{ kind: 'lte', column: __column, value: __value }}))",
                self.js_all(&generated_args)
            ))),
            "neq" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column, __value]) => ({{ kind: 'neq', column: __column, value: __value }}))",
                self.js_all(&generated_args)
            ))),
            "not" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__predicate) => ({{ kind: 'not', predicate: __predicate }}))",
                generated_args[0]
            ))),
            "nullable" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__column) => __sigil_sql_nullable(__column))",
                generated_args[0]
            ))),
            "one" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __select]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "one",
                    "[__handle, __select]",
                    "__sigil_world_sql_one(__handle?.__fields?.[0] ?? '', __select)",
                    None
                )?
            ))),
            "oneIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__select, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "oneIn",
                    "[__select, __transaction]",
                    "__sigil_world_sql_one_in(__select, __transaction)",
                    None
                )?
            ))),
            "or" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__left, __right]) => ({{ kind: 'or', left: __left, right: __right }}))",
                self.js_all(&generated_args)
            ))),
            "orderBy" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__column, __direction, __select]) => ({{ ...__select, order: {{ column: __column, direction: __direction?.__tag === 'Desc' ? 'Desc' : 'Asc' }} }}))",
                self.js_all(&generated_args)
            ))),
            "raw" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__params, __sql]) => ({{ params: __params ?? {{}}, sql: String(__sql ?? '') }}))",
                self.js_all(&generated_args)
            ))),
            "rawExec" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __statement]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rawExec",
                    "[__handle, __statement]",
                    "__sigil_world_sql_raw_exec(__handle?.__fields?.[0] ?? '', __statement)",
                    None
                )?
            ))),
            "rawExecIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__statement, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rawExecIn",
                    "[__statement, __transaction]",
                    "__sigil_world_sql_raw_exec_in(__statement, __transaction)",
                    None
                )?
            ))),
            "rawQuery" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __statement]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rawQuery",
                    "[__handle, __statement]",
                    "__sigil_world_sql_raw_query(__handle?.__fields?.[0] ?? '', __statement)",
                    None
                )?
            ))),
            "rawQueryIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__statement, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rawQueryIn",
                    "[__statement, __transaction]",
                    "__sigil_world_sql_raw_query_in(__statement, __transaction)",
                    None
                )?
            ))),
            "rawQueryOne" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __statement]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rawQueryOne",
                    "[__handle, __statement]",
                    "__sigil_world_sql_raw_query_one(__handle?.__fields?.[0] ?? '', __statement)",
                    None
                )?
            ))),
            "rawQueryOneIn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__statement, __transaction]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rawQueryOneIn",
                    "[__statement, __transaction]",
                    "__sigil_world_sql_raw_query_one_in(__statement, __transaction)",
                    None
                )?
            ))),
            "rollback" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__transaction) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "sql",
                    "rollback",
                    "[__transaction]",
                    "__sigil_world_sql_rollback(__transaction)",
                    None
                )?
            ))),
            "select" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__table) => ({{ limit: null, order: null, predicate: null, table: __table }}))",
                generated_args[0]
            ))),
            "set" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__column, __statement, __value]) => ({{ ...__statement, assignments: [...(Array.isArray(__statement?.assignments) ? __statement.assignments : []), {{ column: __column, value: __value }}] }}))",
                self.js_all(&generated_args)
            ))),
            "table1" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__column1, __name]) => __sigil_sql_table(__name, [__column1]))",
                self.js_all(&generated_args)
            ))),
            "table2" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __name]) => __sigil_sql_table(__name, [__column1, __column2]))",
                self.js_all(&generated_args)
            ))),
            "table3" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __column3, __name]) => __sigil_sql_table(__name, [__column1, __column2, __column3]))",
                self.js_all(&generated_args)
            ))),
            "table4" if generated_args.len() == 5 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __column3, __column4, __name]) => __sigil_sql_table(__name, [__column1, __column2, __column3, __column4]))",
                self.js_all(&generated_args)
            ))),
            "table5" if generated_args.len() == 6 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __column3, __column4, __column5, __name]) => __sigil_sql_table(__name, [__column1, __column2, __column3, __column4, __column5]))",
                self.js_all(&generated_args)
            ))),
            "table6" if generated_args.len() == 7 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __column3, __column4, __column5, __column6, __name]) => __sigil_sql_table(__name, [__column1, __column2, __column3, __column4, __column5, __column6]))",
                self.js_all(&generated_args)
            ))),
            "table7" if generated_args.len() == 8 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __column3, __column4, __column5, __column6, __column7, __name]) => __sigil_sql_table(__name, [__column1, __column2, __column3, __column4, __column5, __column6, __column7]))",
                self.js_all(&generated_args)
            ))),
            "table8" if generated_args.len() == 9 => Ok(Some(format!(
                "{}.then(([__column1, __column2, __column3, __column4, __column5, __column6, __column7, __column8, __name]) => __sigil_sql_table(__name, [__column1, __column2, __column3, __column4, __column5, __column6, __column7, __column8]))",
                self.js_all(&generated_args)
            ))),
            "textColumn" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__field, __name]) => __sigil_sql_column(__field, __name, 'text'))",
                self.js_all(&generated_args)
            ))),
            "update" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__table) => ({{ assignments: [], predicate: null, table: __table }}))",
                generated_args[0]
            ))),
            "updateWhere" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__predicate, __statement]) => ({{ ...__statement, predicate: __predicate }}))",
                self.js_all(&generated_args)
            ))),
            "where" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__predicate, __select]) => ({{ ...__select, predicate: __predicate }}))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_task_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "cancel" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__task) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "task",
                    "cancel",
                    "[__task]",
                    "__sigil_world_task_cancel(__task)",
                    None
                )?
            ))),
            "spawn" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__work) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "task",
                    "spawn",
                    "[__work]",
                    "__sigil_world_task_spawn(__work)",
                    None
                )?
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__task) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "task",
                    "wait",
                    "[__task]",
                    "__sigil_world_task_wait(__task)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_cli_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "program" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__description, __name, __root, __subcommands]) => __sigil_cli_program(__name, __description, __root?.__tag === 'Some' ? __root.__fields[0] : null, __subcommands))",
                self.js_all(&generated_args)
            ))),
            "run" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__argv, __program]) => __sigil_cli_run(__argv, __program))",
                self.js_all(&generated_args)
            ))),
            "root0" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__description, __result]) => __sigil_cli_command('root', null, __description, [], async () => __result))",
                self.js_all(&generated_args)
            ))),
            "root1" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__arg1, __build, __description]) => __sigil_cli_command('root', null, __description, [__arg1], __build))",
                self.js_all(&generated_args)
            ))),
            "root2" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __build, __description]) => __sigil_cli_command('root', null, __description, [__arg1, __arg2], __build))",
                self.js_all(&generated_args)
            ))),
            "root3" if generated_args.len() == 5 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __build, __description]) => __sigil_cli_command('root', null, __description, [__arg1, __arg2, __arg3], __build))",
                self.js_all(&generated_args)
            ))),
            "root4" if generated_args.len() == 6 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __arg4, __build, __description]) => __sigil_cli_command('root', null, __description, [__arg1, __arg2, __arg3, __arg4], __build))",
                self.js_all(&generated_args)
            ))),
            "root5" if generated_args.len() == 7 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __arg4, __arg5, __build, __description]) => __sigil_cli_command('root', null, __description, [__arg1, __arg2, __arg3, __arg4, __arg5], __build))",
                self.js_all(&generated_args)
            ))),
            "root6" if generated_args.len() == 8 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __arg4, __arg5, __arg6, __build, __description]) => __sigil_cli_command('root', null, __description, [__arg1, __arg2, __arg3, __arg4, __arg5, __arg6], __build))",
                self.js_all(&generated_args)
            ))),
            "command0" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__description, __name, __result]) => __sigil_cli_command('command', __name, __description, [], async () => __result))",
                self.js_all(&generated_args)
            ))),
            "command1" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__arg1, __build, __description, __name]) => __sigil_cli_command('command', __name, __description, [__arg1], __build))",
                self.js_all(&generated_args)
            ))),
            "command2" if generated_args.len() == 5 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __build, __description, __name]) => __sigil_cli_command('command', __name, __description, [__arg1, __arg2], __build))",
                self.js_all(&generated_args)
            ))),
            "command3" if generated_args.len() == 6 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __build, __description, __name]) => __sigil_cli_command('command', __name, __description, [__arg1, __arg2, __arg3], __build))",
                self.js_all(&generated_args)
            ))),
            "command4" if generated_args.len() == 7 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __arg4, __build, __description, __name]) => __sigil_cli_command('command', __name, __description, [__arg1, __arg2, __arg3, __arg4], __build))",
                self.js_all(&generated_args)
            ))),
            "command5" if generated_args.len() == 8 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __arg4, __arg5, __build, __description, __name]) => __sigil_cli_command('command', __name, __description, [__arg1, __arg2, __arg3, __arg4, __arg5], __build))",
                self.js_all(&generated_args)
            ))),
            "command6" if generated_args.len() == 9 => Ok(Some(format!(
                "{}.then(([__arg1, __arg2, __arg3, __arg4, __arg5, __arg6, __build, __description, __name]) => __sigil_cli_command('command', __name, __description, [__arg1, __arg2, __arg3, __arg4, __arg5, __arg6], __build))",
                self.js_all(&generated_args)
            ))),
            "flag" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__description, __long, __short]) => __sigil_cli_arg('flag', {{ description: String(__description), long: String(__long), short: __short }}))",
                self.js_all(&generated_args)
            ))),
            "option" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__description, __long, __short, __valueName]) => __sigil_cli_arg('option', {{ description: String(__description), long: String(__long), short: __short, valueName: String(__valueName) }}))",
                self.js_all(&generated_args)
            ))),
            "requiredOption" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__description, __long, __short, __valueName]) => __sigil_cli_arg('requiredOption', {{ description: String(__description), long: String(__long), short: __short, valueName: String(__valueName) }}))",
                self.js_all(&generated_args)
            ))),
            "manyOption" if generated_args.len() == 4 => Ok(Some(format!(
                "{}.then(([__description, __long, __short, __valueName]) => __sigil_cli_arg('manyOption', {{ description: String(__description), long: String(__long), short: __short, valueName: String(__valueName) }}))",
                self.js_all(&generated_args)
            ))),
            "positional" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__description, __name]) => __sigil_cli_arg('positional', {{ description: String(__description), name: String(__name) }}))",
                self.js_all(&generated_args)
            ))),
            "optionalPositional" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__description, __name]) => __sigil_cli_arg('optionalPositional', {{ description: String(__description), name: String(__name) }}))",
                self.js_all(&generated_args)
            ))),
            "manyPositionals" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__description, __name]) => __sigil_cli_arg('manyPositionals', {{ description: String(__description), name: String(__name) }}))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_terminal_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "clearScreen" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "terminal",
                "clearScreen",
                "[]",
                "__sigil_world_terminal_clear_screen()",
                None,
            )?)),
            "disableRawMode" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "terminal",
                "disableRawMode",
                "[]",
                "__sigil_world_terminal_disable_raw_mode()",
                None,
            )?)),
            "enableRawMode" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "terminal",
                "enableRawMode",
                "[]",
                "__sigil_world_terminal_enable_raw_mode()",
                None,
            )?)),
            "hideCursor" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "terminal",
                "hideCursor",
                "[]",
                "__sigil_world_terminal_hide_cursor()",
                None,
            )?)),
            "readKey" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "terminal",
                "readKey",
                "[]",
                "__sigil_world_terminal_read_key()",
                None,
            )?)),
            "showCursor" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "terminal",
                "showCursor",
                "[]",
                "__sigil_world_terminal_show_cursor()",
                None,
            )?)),
            "write" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__text) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "terminal",
                    "write",
                    "[__text]",
                    "__sigil_world_terminal_write(__text)",
                    None,
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_process_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "argv" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "process",
                "argv",
                "[]",
                "__sigil_world_process_argv()",
                None,
            )?)),
            "kill" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__process) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "kill",
                    "[__process]",
                    "__sigil_world_process_kill(__process)",
                    None
                )?
            ))),
            "run" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__command) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "run",
                    "[__command]",
                    "__sigil_world_process_run(__command)",
                    None
                )?
            ))),
            "exit" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__code) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "exit",
                    "[__code]",
                    "__sigil_world_process_exit(__code)",
                    None
                )?
            ))),
            "runAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__command, __handle]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "runAt",
                    "[__command, __handle]",
                    "__sigil_world_process_run_at(__handle?.__fields?.[0] ?? '', __command)",
                    None
                )?
            ))),
            "runChecked" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__command) => {}).then((__result) => __sigil_process_checked_result(__result))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "runChecked",
                    "[__command]",
                    "__sigil_world_process_run(__command)",
                    None
                )?
            ))),
            "runJson" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__command) => {}).then((__result) => __sigil_process_json_result(__result))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "runJson",
                    "[__command]",
                    "__sigil_world_process_run(__command)",
                    None
                )?
            ))),
            "start" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__command) => {}).then((__handle) => __sigil_owned_wrap(__handle, async () => {{ await __sigil_world_process_kill(__handle); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "start",
                    "[__command]",
                    "__sigil_world_process_spawn(__command)",
                    None
                )?
            ))),
            "startAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__command, __handle]) => {}).then((__started) => __sigil_owned_wrap(__started, async () => {{ await __sigil_world_process_kill(__started); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "startAt",
                    "[__command, __handle]",
                    "__sigil_world_process_spawn_at(__handle?.__fields?.[0] ?? '', __command)",
                    None
                )?
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__process) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "process",
                    "wait",
                    "[__process]",
                    "__sigil_world_process_wait(__process)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_pty_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "close" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__session) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "close",
                    "[__session]",
                    "__sigil_world_pty_close(__session)",
                    None
                )?
            ))),
            "closeManaged" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__sessionRef) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "closeManaged",
                    "[__sessionRef]",
                    "__sigil_world_pty_close_managed(__sessionRef)",
                    None
                )?
            ))),
            "events" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__session) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "events",
                    "[__session]",
                    "__sigil_world_pty_events(__session)",
                    None
                )?
            ))),
            "eventsManaged" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__sessionRef) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "eventsManaged",
                    "[__sessionRef]",
                    "__sigil_world_pty_events_managed(__sessionRef)",
                    None
                )?
            ))),
            "resize" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__cols, __rows, __session]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "resize",
                    "[__cols, __rows, __session]",
                    "__sigil_world_pty_resize(__session, __cols, __rows)",
                    None
                )?
            ))),
            "resizeManaged" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__cols, __rows, __sessionRef]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "resizeManaged",
                    "[__cols, __rows, __sessionRef]",
                    "__sigil_world_pty_resize_managed(__sessionRef, __cols, __rows)",
                    None
                )?
            ))),
            "spawn" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__spawn) => {}).then((__session) => __sigil_owned_wrap(__session, async () => {{ await __sigil_world_pty_close(__session); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "spawn",
                    "[__spawn]",
                    "__sigil_world_pty_spawn(__spawn)",
                    None
                )?
            ))),
            "spawnManaged" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__spawn) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "spawnManaged",
                    "[__spawn]",
                    "__sigil_world_pty_spawn_managed(__spawn)",
                    None
                )?
            ))),
            "spawnAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __spawn]) => {}).then((__session) => __sigil_owned_wrap(__session, async () => {{ await __sigil_world_pty_close(__session); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "spawnAt",
                    "[__handle, __spawn]",
                    "__sigil_world_pty_spawn_at(__handle?.__fields?.[0] ?? '', __spawn)",
                    None
                )?
            ))),
            "spawnManagedAt" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __spawn]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "spawnManagedAt",
                    "[__handle, __spawn]",
                    "__sigil_world_pty_spawn_managed_at(__handle?.__fields?.[0] ?? '', __spawn)",
                    None
                )?
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__session) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "wait",
                    "[__session]",
                    "__sigil_world_pty_wait(__session)",
                    None
                )?
            ))),
            "waitManaged" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__sessionRef) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "waitManaged",
                    "[__sessionRef]",
                    "__sigil_world_pty_wait_managed(__sessionRef)",
                    None
                )?
            ))),
            "write" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __session]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "write",
                    "[__input, __session]",
                    "__sigil_world_pty_write(__session, __input)",
                    None
                )?
            ))),
            "writeManaged" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __sessionRef]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "pty",
                    "writeManaged",
                    "[__input, __sessionRef]",
                    "__sigil_world_pty_write_managed(__sessionRef, __input)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_websocket_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "close" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__client) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "websocket",
                    "close",
                    "[__client]",
                    "__sigil_world_websocket_close(__client)",
                    None
                )?
            ))),
            "connections" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __server]) => {}).then((__source) => __sigil_owned_wrap(__source, async () => {{ await __sigil_world_stream_close(__source); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "websocket",
                    "connections",
                    "[__handle, __server]",
                    "__sigil_world_websocket_connections(__handle?.__fields?.[0] ?? '', __server)",
                    None
                )?
            ))),
            "listen" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__port, __routes]) => {}).then((__server) => __sigil_owned_wrap(__server, async () => {{ await __sigil_world_websocket_close_server(__server); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "websocket",
                    "listen",
                    "[__port, __routes]",
                    "__sigil_world_websocket_listen(__port, __routes)",
                    None
                )?
            ))),
            "messages" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__client) => {}).then((__source) => __sigil_owned_wrap(__source, async () => {{ await __sigil_world_stream_close(__source); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "websocket",
                    "messages",
                    "[__client]",
                    "__sigil_world_websocket_messages(__client)",
                    None
                )?
            ))),
            "port" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => Number(__server?.port ?? 0))",
                generated_args[0]
            ))),
            "route" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __path]) => ({{ handle: __handle, path: String(__path ?? '') }}))",
                self.js_all(&generated_args)
            ))),
            "send" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__client, __text]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "websocket",
                    "send",
                    "[__client, __text]",
                    "__sigil_world_websocket_send(__client, __text)",
                    None
                )?
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "websocket",
                    "wait",
                    "[__server]",
                    "__sigil_world_websocket_wait(__server)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_random_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "intBetween" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__max, __min]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "random",
                    "intBetween",
                    "[__max, __min]",
                    "__sigil_world_random_int_between(__max, __min)",
                    None
                )?
            ))),
            "pick" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__items) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "random",
                    "pick",
                    "[__items]",
                    "__sigil_world_random_pick(__items)",
                    None
                )?
            ))),
            "shuffle" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__items) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "random",
                    "shuffle",
                    "[__items]",
                    "__sigil_world_random_shuffle(__items)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_stream_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "close" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__source) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "stream",
                    "close",
                    "[__source]",
                    "__sigil_world_stream_close(__source)",
                    None
                )?
            ))),
            "hub" if generated_args.is_empty() => Ok(Some(self.wrap_effect_trace(
                span_id,
                "stream",
                "hub",
                "[]",
                "__sigil_ready(__sigil_world_stream_open_hub())",
                None,
            )?)),
            "next" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__source) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "stream",
                    "next",
                    "[__source]",
                    "__sigil_world_stream_next(__source)",
                    None
                )?
            ))),
            "publish" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__hub, __value]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "stream",
                    "publish",
                    "[__value, __hub]",
                    "__sigil_world_stream_publish(__hub, __value)",
                    None
                )?
            ))),
            "subscribe" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__hub) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "stream",
                    "subscribe",
                    "[__hub]",
                    "__sigil_world_stream_subscribe(__hub)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_http_client_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "request" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__request) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "http",
                    "request",
                    "[__request]",
                    "__sigil_world_http_request(__request)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_regex_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "compile" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__flags, __pattern]) => __sigil_regex_compile_result(__flags, __pattern))",
                self.js_all(&generated_args)
            ))),
            "find" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __regex]) => __sigil_regex_find(__regex, __input))",
                self.js_all(&generated_args)
            ))),
            "findAll" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __regex]) => __sigil_regex_find_all(__regex, __input))",
                self.js_all(&generated_args)
            ))),
            "isMatch" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__input, __regex]) => __sigil_regex_is_match(__regex, __input))",
                self.js_all(&generated_args)
            ))),
            _ => Ok(None),
        }
    }

    fn generate_float_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "abs" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.abs(__x))",
                generated_args[0]
            ))),
            "ceil" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.ceil(__x))",
                generated_args[0]
            ))),
            "cos" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.cos(__x))",
                generated_args[0]
            ))),
            "exp" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.exp(__x))",
                generated_args[0]
            ))),
            "floor" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.floor(__x))",
                generated_args[0]
            ))),
            "isFinite" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Number.isFinite(__x))",
                generated_args[0]
            ))),
            "isNaN" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Number.isNaN(__x))",
                generated_args[0]
            ))),
            "log" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.log(__x))",
                generated_args[0]
            ))),
            "max" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__a, __b]) => Math.max(__a, __b))",
                self.js_all(&generated_args)
            ))),
            "min" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__a, __b]) => Math.min(__a, __b))",
                self.js_all(&generated_args)
            ))),
            "pow" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__base, __exp]) => Math.pow(__base, __exp))",
                self.js_all(&generated_args)
            ))),
            "round" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.round(__x))",
                generated_args[0]
            ))),
            "sin" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.sin(__x))",
                generated_args[0]
            ))),
            "sqrt" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.sqrt(__x))",
                generated_args[0]
            ))),
            "tan" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.tan(__x))",
                generated_args[0]
            ))),
            "toFloat" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Number(__x))",
                generated_args[0]
            ))),
            "toInt" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__x) => Math.trunc(__x))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_crypto_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "base64Decode" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_crypto_base64_decode(__input))",
                generated_args[0]
            ))),
            "base64Encode" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_crypto_base64_encode(__input))",
                generated_args[0]
            ))),
            "hexDecode" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_crypto_hex_decode(__input))",
                generated_args[0]
            ))),
            "hexEncode" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_crypto_hex_encode(__input))",
                generated_args[0]
            ))),
            "hmacSha256" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __message]) => __sigil_crypto_hmac_sha256(__key, __message))",
                self.js_all(&generated_args)
            ))),
            "sha256" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_crypto_sha256(__input))",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_http_server_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "jsonBody" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__request) => __sigil_http_json_body_result(__request))",
                generated_args[0]
            ))),
            "listen" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__port) => {}).then((__server) => __sigil_owned_wrap(__server, async () => {{ await __sigil_http_close(__server); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "listen",
                    "[__port]",
                    "__sigil_http_listen_requests(__port)",
                    None
                )?
            ))),
            "listenWithWebSockets" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__port, __routes]) => {}).then((__server) => __sigil_owned_wrap(__server, async () => {{ await __sigil_http_close(__server); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "listenWithWebSockets",
                    "[__port, __routes]",
                    "__sigil_http_listen_requests_with_websockets(__port, __routes)",
                    None
                )?
            ))),
            "listenWith" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handler, __port]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "listenWith",
                    "[__handler, __port]",
                    "__sigil_http_listen(__handler, __port)",
                    None
                )?
            ))),
            "match" if generated_args.len() == 3 => Ok(Some(format!(
                "{}.then(([__method, __pattern, __request]) => __sigil_http_match(__method, __pattern, __request))",
                self.js_all(&generated_args)
            ))),
            "port" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => Number(__server?.port ?? 0))",
                generated_args[0]
            ))),
            "reply" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__responder, __response]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "reply",
                    "[__responder, __response]",
                    "__sigil_http_reply(__response, __responder)",
                    None
                )?
            ))),
            "requests" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => {}).then((__source) => __sigil_owned_wrap(__source, async () => {{ await __sigil_world_stream_close(__source); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "requests",
                    "[__server]",
                    "__sigil_http_requests(__server)",
                    None
                )?
            ))),
            "serve" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handler, __port]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "serve",
                    "[__handler, __port]",
                    "__sigil_http_serve(__handler, __port)",
                    None
                )?
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "wait",
                    "[__server]",
                    "__sigil_http_wait(__server)",
                    None
                )?
            ))),
            "websocketClose" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__client) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "websocketClose",
                    "[__client]",
                    "__sigil_http_websocket_close(__client)",
                    None
                )?
            ))),
            "websocketConnections" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __server]) => {}).then((__source) => __sigil_owned_wrap(__source, async () => {{ await __sigil_world_stream_close(__source); return null; }}))",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "websocketConnections",
                    "[__handle, __server]",
                    "__sigil_http_websocket_connections(__handle?.__fields?.[0] ?? '', __server)",
                    None
                )?
            ))),
            "websocketMessages" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__client) => {}).then((__source) => __sigil_owned_wrap(__source, async () => {{ await __sigil_world_stream_close(__source); return null; }}))",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "websocketMessages",
                    "[__client]",
                    "__sigil_http_websocket_messages(__client)",
                    None
                )?
            ))),
            "websocketRoute" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handle, __path]) => ({{ handle: __handle, path: String(__path ?? '') }}))",
                self.js_all(&generated_args)
            ))),
            "websocketSend" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__client, __text]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "httpServer",
                    "websocketSend",
                    "[__client, __text]",
                    "__sigil_http_websocket_send(__client, __text)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_tcp_client_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "request" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__request) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "tcp",
                    "request",
                    "[__request]",
                    "__sigil_world_tcp_request(__request)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_tcp_server_intrinsic(
        &mut self,
        call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprCall, call_expr.location);

        match member {
            "listen" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handler, __port]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "tcpServer",
                    "listen",
                    "[__handler, __port]",
                    "__sigil_tcp_listen(__handler, __port)",
                    None
                )?
            ))),
            "port" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => Number(__server?.port ?? 0))",
                generated_args[0]
            ))),
            "serve" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__handler, __port]) => {})",
                self.js_all(&generated_args),
                self.wrap_effect_trace(
                    span_id,
                    "tcpServer",
                    "serve",
                    "[__handler, __port]",
                    "__sigil_tcp_serve(__handler, __port)",
                    None
                )?
            ))),
            "wait" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__server) => {})",
                generated_args[0],
                self.wrap_effect_trace(
                    span_id,
                    "tcpServer",
                    "wait",
                    "[__server]",
                    "__sigil_tcp_wait(__server)",
                    None
                )?
            ))),
            _ => Ok(None),
        }
    }

    fn generate_url_intrinsic(
        &mut self,
        _call_expr: &TypedExpr,
        member: &str,
        args: &[TypedExpr],
    ) -> Result<Option<String>, CodegenError> {
        let generated_args = args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<Vec<_>, CodegenError>>()?;

        match member {
            "get_query" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __url]) => __sigil_map_get(__url.query, __key))",
                self.js_all(&generated_args)
            ))),
            "has_query" if generated_args.len() == 2 => Ok(Some(format!(
                "{}.then(([__key, __url]) => __sigil_map_has(__url.query, __key))",
                self.js_all(&generated_args)
            ))),
            "is_absolute" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__url) => __url.protocol.length > 0)",
                generated_args[0]
            ))),
            "is_anchor" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__url) => __url.path.length === 0 && __url.fragment.length > 0)",
                generated_args[0]
            ))),
            "parse" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__input) => __sigil_url_parse_result(__input))",
                generated_args[0]
            ))),
            "suffix" if generated_args.len() == 1 => Ok(Some(format!(
                "{}.then((__url) => __url.query_string + __url.fragment)",
                generated_args[0]
            ))),
            _ => Ok(None),
        }
    }

    fn generate_constructor_call(
        &mut self,
        call: &TypedConstructorCallExpr,
    ) -> Result<String, CodegenError> {
        let func = match &call.module_path {
            Some(module_path) => {
                let namespace = module_path
                    .iter()
                    .cloned()
                    .collect::<Vec<String>>()
                    .join("_");
                format!(
                    "{}.{}",
                    sanitize_js_identifier(&namespace),
                    sanitize_js_identifier(&call.constructor)
                )
            }
            None => sanitize_js_identifier(&call.constructor),
        };
        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;
        let mut values = vec![self.js_ready(&func)];
        values.extend(args);
        Ok(format!(
            "{}.then(([__sigil_fn, ...__sigil_args]) => __sigil_fn(...__sigil_args))",
            self.js_all(&values)
        ))
    }

    fn generate_extern_call(
        &mut self,
        expr: &TypedExpr,
        call: &TypedExternCallExpr,
    ) -> Result<String, CodegenError> {
        let joined_namespace = call.namespace.join("/");
        // Normalize only the alias-style internal stdlib wrappers whose imports
        // are suppressed in `generate_extern`. Non-suppressed intrinsic
        // namespaces such as `stdlib/featureFlags`, `stdlib/crypto`,
        // `stdlib/float`, `stdlib/regex`, and `stdlib/url` intentionally do not
        // participate in alias-style normalization.
        let call_namespace = match joined_namespace.as_str() {
            "stdlibCli" => "stdlib/cli",
            "stdlibFile" => "stdlib/file",
            "stdlibFsWatch" => "stdlib/fsWatch",
            "stdlibHttpClient" => "stdlib/httpClient",
            "stdlibHttpServer" => "stdlib/httpServer",
            "stdlibIo" => "stdlib/io",
            "stdlibLog" => "stdlib/log",
            "stdlibProcess" => "stdlib/process",
            "stdlibPty" => "stdlib/pty",
            "stdlibRandom" => "stdlib/random",
            "stdlibSql" => "stdlib/sql",
            "stdlibStream" => "stdlib/stream",
            "stdlibTask" => "stdlib/task",
            "stdlibTcpClient" => "stdlib/tcpClient",
            "stdlibTcpServer" => "stdlib/tcpServer",
            "stdlibTerminal" => "stdlib/terminal",
            "stdlibTime" => "stdlib/time",
            "stdlibTimer" => "stdlib/timer",
            "stdlibWebSocket" => "stdlib/websocket",
            _ => joined_namespace.as_str(),
        };
        if call_namespace == "stdlib/string" {
            if let Some(intrinsic) =
                self.generate_string_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/json" {
            if let Some(intrinsic) = self.generate_json_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/file" {
            if let Some(intrinsic) = self.generate_file_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/fsWatch" {
            if let Some(intrinsic) =
                self.generate_fswatch_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/log" {
            if let Some(intrinsic) = self.generate_log_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/io" {
            if let Some(intrinsic) = self.generate_io_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/httpClient" {
            if let Some(intrinsic) =
                self.generate_http_client_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/httpServer" {
            if let Some(intrinsic) =
                self.generate_http_server_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/cli" {
            if let Some(intrinsic) = self.generate_cli_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/tcpClient" {
            if let Some(intrinsic) =
                self.generate_tcp_client_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/tcpServer" {
            if let Some(intrinsic) =
                self.generate_tcp_server_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/time" {
            if let Some(intrinsic) = self.generate_time_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/timer" {
            if let Some(intrinsic) =
                self.generate_timer_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/terminal" {
            if let Some(intrinsic) =
                self.generate_terminal_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/sql" {
            if let Some(intrinsic) = self.generate_sql_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/process" {
            if let Some(intrinsic) =
                self.generate_process_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/pty" {
            if let Some(intrinsic) = self.generate_pty_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/websocket" {
            if let Some(intrinsic) =
                self.generate_websocket_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/random" {
            if let Some(intrinsic) =
                self.generate_random_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/stream" {
            if let Some(intrinsic) =
                self.generate_stream_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call_namespace == "stdlib/task" {
            if let Some(intrinsic) = self.generate_task_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/featureFlags" {
            if let Some(intrinsic) =
                self.generate_feature_flags_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/crypto" {
            if let Some(intrinsic) =
                self.generate_crypto_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/float" {
            if let Some(intrinsic) =
                self.generate_float_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/regex" {
            if let Some(intrinsic) =
                self.generate_regex_intrinsic(expr, &call.member, &call.args)?
            {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/") == "stdlib/url" {
            if let Some(intrinsic) = self.generate_url_intrinsic(expr, &call.member, &call.args)? {
                return Ok(intrinsic);
            }
        }
        if call.namespace.join("/").starts_with("test/observe/") {
            if let Some(intrinsic) = self.generate_test_observe_intrinsic(
                expr,
                &call.namespace.join("/"),
                &call.member,
                &call.args,
            )? {
                return Ok(intrinsic);
            }
        }
        if matches!(
            call_namespace,
            "stdlib/cli"
                | "stdlib/file"
                | "stdlib/fsWatch"
                | "stdlib/httpClient"
                | "stdlib/httpServer"
                | "stdlib/io"
                | "stdlib/log"
                | "stdlib/process"
                | "stdlib/pty"
                | "stdlib/random"
                | "stdlib/sql"
                | "stdlib/stream"
                | "stdlib/task"
                | "stdlib/tcpClient"
                | "stdlib/tcpServer"
                | "stdlib/terminal"
                | "stdlib/time"
                | "stdlib/timer"
                | "stdlib/websocket"
        ) {
            return Err(CodegenError::General(format!(
                "Internal stdlib namespace '{}' does not support member '{}' in codegen; add an intrinsic lowering before suppressing its import",
                call_namespace, call.member
            )));
        }

        let func_ref = format!(
            "{}.{}",
            sanitize_js_identifier(&call.namespace.join("_")),
            sanitize_js_identifier(&call.member)
        );
        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;
        let span_id = self.span_id_for_expr(DebugSpanKind::ExprExternCall, expr.location);
        let namespace_name = call.namespace.join("::");
        let invocation = if call.subscription {
            format!(
                "__sigil_extern_subscribe(\"{}\", {}, __sigil_args)",
                call.mock_key, func_ref
            )
        } else {
            format!(
                "__sigil_call(\"{}\", {}, __sigil_args)",
                call.mock_key, func_ref
            )
        };
        Ok(format!(
            "{}.then((__sigil_args) => {})",
            self.js_all(&args),
            self.wrap_effect_trace(
                span_id,
                if namespace_name.is_empty() {
                    "extern"
                } else {
                    &namespace_name
                },
                &call.member,
                "__sigil_args",
                &invocation,
                None,
            )?
        ))
    }

    fn generate_method_call(&mut self, call: &TypedMethodCallExpr) -> Result<String, CodegenError> {
        let receiver = self.generate_expression(&call.receiver)?;
        let args: Vec<String> = call
            .args
            .iter()
            .map(|arg| self.generate_expression(arg))
            .collect::<Result<_, _>>()?;

        match &call.selector {
            MethodSelector::Field(field) => {
                let mut values = vec![receiver];
                values.extend(args);
                Ok(format!(
                    "{}.then(([__sigil_object, ...__sigil_args]) => __sigil_object.{}.call(__sigil_object, ...__sigil_args))",
                    self.js_all(&values),
                    field
                ))
            }
            MethodSelector::Index(index) => {
                let index = self.generate_expression(index)?;
                let mut values = vec![receiver, index];
                values.extend(args);
                Ok(format!(
                    "{}.then(([__sigil_object, __sigil_index, ...__sigil_args]) => __sigil_object[__sigil_index].call(__sigil_object, ...__sigil_args))",
                    self.js_all(&values)
                ))
            }
        }
    }

    fn generate_binary(&mut self, bin: &TypedBinaryExpr) -> Result<String, CodegenError> {
        let left = self.generate_expression(&bin.left)?;
        let right = self.generate_expression(&bin.right)?;

        let op = match bin.operator {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Power => "**",
            BinaryOperator::Equal => "",
            BinaryOperator::NotEqual => "",
            BinaryOperator::Less => "<",
            BinaryOperator::Greater => ">",
            BinaryOperator::LessEq => "<=",
            BinaryOperator::GreaterEq => ">=",
            BinaryOperator::And => "&&",
            BinaryOperator::Or => "||",
            BinaryOperator::Append => "+", // String concatenation
            BinaryOperator::ListAppend => ".concat", // Will need special handling
            BinaryOperator::Pipe => {
                // Pipeline operator - right(left)
                return Ok(format!(
                    "{}.then(([__sigil_fn, __sigil_value]) => __sigil_fn(__sigil_value))",
                    self.js_all(&[right, left])
                ));
            }
            BinaryOperator::ComposeFwd | BinaryOperator::ComposeBwd => {
                // Function composition - defer to helper
                return Err(CodegenError::General(
                    "Function composition not yet implemented".to_string(),
                ));
            }
        };

        match bin.operator {
            BinaryOperator::Equal => Ok(format!(
                "{}.then(([__left, __right]) => __sigil_deep_equal(__left, __right))",
                self.js_all(&[left, right])
            )),
            BinaryOperator::NotEqual => Ok(format!(
                "{}.then(([__left, __right]) => !__sigil_deep_equal(__left, __right))",
                self.js_all(&[left, right])
            )),
            BinaryOperator::ListAppend => Ok(format!(
                "{}.then(([__left, __right]) => __left.concat(__right))",
                self.js_all(&[left, right])
            )),
            BinaryOperator::And => Ok(format!(
                "{}.then((__left) => __left ? {}.then((__right) => (__left && __right)) : false)",
                left, right
            )),
            BinaryOperator::Or => Ok(format!(
                "{}.then((__left) => __left ? true : {}.then((__right) => (__left || __right)))",
                left, right
            )),
            _ => Ok(format!(
                "{}.then(([__left, __right]) => (__left {} __right))",
                self.js_all(&[left, right]),
                op
            )),
        }
    }

    fn generate_unary(&mut self, un: &TypedUnaryExpr) -> Result<String, CodegenError> {
        let operand = self.generate_expression(&un.operand)?;

        match un.operator {
            UnaryOperator::Negate => Ok(format!("{}.then((__value) => (-__value))", operand)),
            UnaryOperator::Not => Ok(format!("{}.then((__value) => (!__value))", operand)),
            UnaryOperator::Length => Ok(format!("{}.then((__value) => (__sigil_is_map(__value) ? __value.__sigil_map.length : __value.length))", operand)),
        }
    }

    fn generate_if(
        &mut self,
        expr: &TypedExpr,
        if_expr: &TypedIfExpr,
    ) -> Result<String, CodegenError> {
        let condition = self.generate_expression(&if_expr.condition)?;
        let then_branch = self.generate_expression(&if_expr.then_branch)?;
        let trace_span_id = self.span_id_for_expr(DebugSpanKind::ExprIf, expr.location);
        let trace_meta = if self.trace_enabled {
            Some(self.trace_meta_literal(trace_span_id, &[])?)
        } else {
            None
        };

        if let Some(ref else_branch) = if_expr.else_branch {
            let else_code = self.generate_expression(else_branch)?;
            Ok(format!(
                "{}.then((__cond) => {{ {} return __cond ? {} : {}; }})",
                condition,
                trace_meta
                    .map(|meta| format!(
                        "__sigil_trace_branch_if({}, __cond, __cond ? \"then\" : \"else\"); ",
                        meta
                    ))
                    .unwrap_or_default(),
                then_branch,
                else_code
            ))
        } else {
            // No else branch - return null for the false case
            Ok(format!(
                "{}.then((__cond) => {{ {} return __cond ? {} : __sigil_ready(null); }})",
                condition,
                trace_meta
                    .map(|meta| format!(
                        "__sigil_trace_branch_if({}, __cond, __cond ? \"then\" : \"else\"); ",
                        meta
                    ))
                    .unwrap_or_default(),
                then_branch
            ))
        }
    }

    fn generate_list(&mut self, list: &TypedListExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = list
            .elements
            .iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        let elements = elements?;
        Ok(format!(
            "{}.then((__items) => __items)",
            self.js_all(&elements)
        ))
    }

    fn generate_tuple(&mut self, tuple: &TypedTupleExpr) -> Result<String, CodegenError> {
        let elements: Result<Vec<String>, CodegenError> = tuple
            .elements
            .iter()
            .map(|elem| self.generate_expression(elem))
            .collect();
        let elements = elements?;
        Ok(format!(
            "{}.then((__items) => __items)",
            self.js_all(&elements)
        ))
    }

    fn generate_record(&mut self, record: &TypedRecordExpr) -> Result<String, CodegenError> {
        let field_names: Vec<String> = record
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect();
        let values: Vec<String> = record
            .fields
            .iter()
            .map(|field| self.generate_expression(&field.value))
            .collect::<Result<_, _>>()?;

        let assignments: Result<Vec<String>, CodegenError> = field_names
            .iter()
            .enumerate()
            .map(|(index, field_name)| {
                let quoted_name = serde_json::to_string(field_name).map_err(|e| {
                    CodegenError::General(format!("Failed to JSON-encode field name: {}", e))
                })?;
                Ok(format!("{}: __values[{}]", quoted_name, index))
            })
            .collect();

        Ok(format!(
            "{}.then((__values) => ({{ {} }}))",
            self.js_all(&values),
            assignments?.join(", ")
        ))
    }

    fn generate_map_literal(&mut self, map: &TypedMapLiteralExpr) -> Result<String, CodegenError> {
        let entries = map
            .entries
            .iter()
            .map(|entry| {
                let key = self.generate_expression(&entry.key)?;
                let value = self.generate_expression(&entry.value)?;
                Ok(format!(
                    "{}.then(([__sigil_key, __sigil_value]) => [__sigil_key, __sigil_value])",
                    self.js_all(&[key, value])
                ))
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;

        Ok(format!(
            "{}.then((__entries) => __sigil_map_from_entries(__entries))",
            self.js_all(&entries)
        ))
    }

    fn generate_field_access(
        &mut self,
        field_access: &TypedFieldAccessExpr,
    ) -> Result<String, CodegenError> {
        let object = self.generate_expression(&field_access.object)?;
        Ok(format!(
            "{}.then((__value) => __value.{} )",
            object, field_access.field
        ))
    }

    fn generate_index(&mut self, index: &TypedIndexExpr) -> Result<String, CodegenError> {
        let object = self.generate_expression(&index.object)?;
        let idx = self.generate_expression(&index.index)?;
        Ok(format!(
            "{}.then(([__value, __index]) => __value[__index])",
            self.js_all(&[object, idx])
        ))
    }

    fn generate_let(
        &mut self,
        expr: &TypedExpr,
        let_expr: &TypedLetExpr,
    ) -> Result<String, CodegenError> {
        // Generate async IIFE for let binding
        let value = self.generate_expression(&let_expr.value)?;
        let body = self.generate_expression(&let_expr.body)?;
        let bindings = self.generate_pattern_bindings(&let_expr.pattern, "__let_value")?;
        let let_type_id = self.named_type_id_for_inference_type(&let_expr.value.typ);
        let scope_locals =
            self.pattern_scope_locals_expr(&let_expr.pattern, "let", let_type_id.as_deref())?;
        let breakpoint_meta = if self.breakpoints_enabled {
            Some(self.trace_meta_literal(
                self.span_id_for_expr(DebugSpanKind::ExprLet, expr.location),
                &[],
            )?)
        } else {
            None
        };

        let mut lines = Vec::new();
        lines.push("(async () => {".to_string());
        lines.push(format!("  const __let_value = await {};", value));
        if let Some(binding) = bindings {
            lines.push(format!("  {}", binding));
        }
        if breakpoint_meta.is_some() {
            lines.push(format!(
                "  __sigil_breakpoint_push_scope({});",
                scope_locals
            ));
            lines.push("  try {".to_string());
            lines.push(format!(
                "    __sigil_breakpoint_maybe_hit({});",
                breakpoint_meta.unwrap()
            ));
            lines.push(format!("    return await {};", body));
            lines.push("  } finally {".to_string());
            lines.push("    __sigil_breakpoint_pop_scope();".to_string());
            lines.push("  }".to_string());
        } else {
            lines.push(format!("  return {};", body));
        }
        lines.push("})()".to_string());

        Ok(lines.join("\n"))
    }

    fn generate_using(
        &mut self,
        expr: &TypedExpr,
        using_expr: &TypedUsingExpr,
    ) -> Result<String, CodegenError> {
        let owned_value = self.generate_expression(&using_expr.value)?;
        let body = self.generate_expression(&using_expr.body)?;
        let binding_name = sanitize_js_identifier(&using_expr.name);
        let scope_locals = format!(
            "[{{ name: {}, origin: \"using\", value: {}, typeId: null }}]",
            self.json_string_literal(&using_expr.name)?,
            binding_name
        );
        let breakpoint_meta = if self.breakpoints_enabled {
            Some(self.trace_meta_literal(
                self.span_id_for_expr(DebugSpanKind::ExprLet, expr.location),
                &[],
            )?)
        } else {
            None
        };

        let mut lines = Vec::new();
        lines.push("(async () => {".to_string());
        lines.push(format!("  const __sigil_owned = await {};", owned_value));
        lines.push(format!(
            "  const {} = __sigil_owned_take(__sigil_owned);",
            binding_name
        ));
        if breakpoint_meta.is_some() {
            lines.push(format!(
                "  __sigil_breakpoint_push_scope({});",
                scope_locals
            ));
            lines.push("  try {".to_string());
            lines.push(format!(
                "    __sigil_breakpoint_maybe_hit({});",
                breakpoint_meta.unwrap()
            ));
            lines.push(format!("    return await {};", body));
            lines.push("  } finally {".to_string());
            lines.push("    __sigil_breakpoint_pop_scope();".to_string());
            lines.push("    await __sigil_owned_dispose(__sigil_owned);".to_string());
            lines.push("  }".to_string());
        } else {
            lines.push("  try {".to_string());
            lines.push(format!("    return await {};", body));
            lines.push("  } finally {".to_string());
            lines.push("    await __sigil_owned_dispose(__sigil_owned);".to_string());
            lines.push("  }".to_string());
        }
        lines.push("})()".to_string());

        Ok(lines.join("\n"))
    }

    fn generate_match(
        &mut self,
        expr: &TypedExpr,
        match_expr: &TypedMatchExpr,
    ) -> Result<String, CodegenError> {
        // Generate an async IIFE that implements pattern matching
        let scrutinee = self.generate_expression(&match_expr.scrutinee)?;
        let trace_span_id = self.span_id_for_expr(DebugSpanKind::ExprMatch, expr.location);
        let trace_meta = if self.trace_enabled {
            Some(self.trace_meta_literal(trace_span_id, &[])?)
        } else {
            None
        };

        let mut lines = Vec::new();
        lines.push("(async () => {".to_string());
        lines.push(format!("  const __match = await {};", scrutinee));

        for (arm_index, arm) in match_expr.arms.iter().enumerate() {
            let condition = self.generate_pattern_condition(&arm.pattern, "__match")?;
            let body = self.generate_expression(&arm.body)?;
            let bindings = self.generate_pattern_bindings(&arm.pattern, "__match")?;
            let arm_span_id = self
                .span_id_for_match_arm(arm.location)
                .unwrap_or("")
                .to_string();
            let trace_line = trace_meta.as_ref().map(|meta| {
                format!(
                    "      __sigil_trace_branch_match({}, {}, {}, {});",
                    meta,
                    serde_json::to_string(&arm_span_id).unwrap(),
                    arm_index,
                    arm.guard.is_some()
                )
            });

            lines.push(format!("  if ({}) {{", condition));

            if let Some(binding) = bindings {
                lines.push(format!("    {}", binding));
            }

            // Add guard check if present
            if let Some(ref guard) = arm.guard {
                let guard_expr = self.generate_expression(guard)?;
                lines.push(format!("    if (await {}) {{", guard_expr));
                if self.breakpoints_enabled {
                    let scope_locals =
                        self.pattern_scope_locals_expr(&arm.pattern, "pattern", None)?;
                    let arm_meta = self.trace_meta_literal(Some(arm_span_id.as_str()), &[])?;
                    lines.push(format!(
                        "      __sigil_breakpoint_push_scope({});",
                        scope_locals
                    ));
                    lines.push("      try {".to_string());
                    lines.push(format!(
                        "        __sigil_breakpoint_maybe_hit({});",
                        arm_meta
                    ));
                    if let Some(trace_line) = &trace_line {
                        lines.push(trace_line.clone());
                    }
                    lines.push(format!("        return await {};", body));
                    lines.push("      } finally {".to_string());
                    lines.push("        __sigil_breakpoint_pop_scope();".to_string());
                    lines.push("      }".to_string());
                    lines.push("    }".to_string());
                    lines.push("  }".to_string());
                    continue;
                }
                if let Some(trace_line) = &trace_line {
                    lines.push(trace_line.clone());
                }
                lines.push(format!("      return {};", body));
                lines.push("    }".to_string());
            } else {
                if self.breakpoints_enabled {
                    let scope_locals =
                        self.pattern_scope_locals_expr(&arm.pattern, "pattern", None)?;
                    let arm_meta = self.trace_meta_literal(Some(arm_span_id.as_str()), &[])?;
                    lines.push(format!(
                        "    __sigil_breakpoint_push_scope({});",
                        scope_locals
                    ));
                    lines.push("    try {".to_string());
                    lines.push(format!("      __sigil_breakpoint_maybe_hit({});", arm_meta));
                    if let Some(trace_line) = &trace_line {
                        lines.push(trace_line.clone());
                    }
                    lines.push(format!("      return await {};", body));
                    lines.push("    } finally {".to_string());
                    lines.push("      __sigil_breakpoint_pop_scope();".to_string());
                    lines.push("    }".to_string());
                    lines.push("  }".to_string());
                    continue;
                }
                if let Some(trace_line) = &trace_line {
                    lines.push(trace_line.clone());
                }
                lines.push(format!("    return {};", body));
            }

            lines.push("  }".to_string());
        }

        lines.push("  throw new Error('Match failed: no pattern matched');".to_string());
        lines.push("})()".to_string());

        Ok(lines.join("\n"))
    }

    fn generate_pattern_condition(
        &mut self,
        pattern: &Pattern,
        scrutinee: &str,
    ) -> Result<String, CodegenError> {
        match pattern {
            Pattern::Literal(lit) => {
                let value = match &lit.value {
                    PatternLiteralValue::Int(n) => n.to_string(),
                    PatternLiteralValue::Float(f) => f.to_string(),
                    PatternLiteralValue::String(s) => {
                        // Use JSON encoding to properly escape all special characters
                        serde_json::to_string(s).unwrap()
                    }
                    PatternLiteralValue::Char(c) => {
                        // Chars are also strings in JavaScript, use JSON encoding
                        serde_json::to_string(&c.to_string()).unwrap()
                    }
                    PatternLiteralValue::Bool(b) => b.to_string(),
                    PatternLiteralValue::Unit => "null".to_string(),
                };
                Ok(format!("{} === {}", scrutinee, value))
            }
            Pattern::Identifier(_) => Ok("true".to_string()),
            Pattern::Wildcard(_) => Ok("true".to_string()),
            Pattern::Constructor(ctor) => {
                let mut conditions = vec![format!("{}?.__tag === \"{}\"", scrutinee, ctor.name)];
                if ctor.patterns.is_empty() {
                    return Ok(conditions.join(" && "));
                }

                conditions.push(format!("Array.isArray({}?.__fields)", scrutinee));
                conditions.push(format!(
                    "{}.__fields.length === {}",
                    scrutinee,
                    ctor.patterns.len()
                ));

                for (i, pattern) in ctor.patterns.iter().enumerate() {
                    conditions.push(self.generate_pattern_condition(
                        pattern,
                        &format!("{}.__fields[{}]", scrutinee, i),
                    )?);
                }

                Ok(conditions.join(" && "))
            }
            Pattern::List(list) => {
                let mut conditions = vec![format!("Array.isArray({})", scrutinee)];
                let length_check = if list.rest.is_some() {
                    format!("{}.length >= {}", scrutinee, list.patterns.len())
                } else {
                    format!("{}.length === {}", scrutinee, list.patterns.len())
                };
                conditions.push(length_check);

                for (i, pattern) in list.patterns.iter().enumerate() {
                    conditions.push(
                        self.generate_pattern_condition(pattern, &format!("{}[{}]", scrutinee, i))?,
                    );
                }

                Ok(conditions.join(" && "))
            }
            Pattern::Tuple(tuple) => {
                let mut conditions = vec![format!(
                    "Array.isArray({}) && {}.length === {}",
                    scrutinee,
                    scrutinee,
                    tuple.patterns.len()
                )];

                for (i, pattern) in tuple.patterns.iter().enumerate() {
                    conditions.push(
                        self.generate_pattern_condition(pattern, &format!("{}[{}]", scrutinee, i))?,
                    );
                }

                Ok(conditions.join(" && "))
            }
            Pattern::Record(_) => Ok("true".to_string()),
        }
    }

    fn generate_pattern_bindings(
        &mut self,
        pattern: &Pattern,
        scrutinee: &str,
    ) -> Result<Option<String>, CodegenError> {
        match pattern {
            Pattern::Identifier(id) => Ok(Some(format!(
                "const {} = {};",
                sanitize_js_identifier(&id.name),
                scrutinee
            ))),
            Pattern::Constructor(ctor) => {
                if ctor.patterns.is_empty() {
                    return Ok(None);
                }

                let mut bindings = Vec::new();
                for (i, p) in ctor.patterns.iter().enumerate() {
                    if let Some(b) = self
                        .generate_pattern_bindings(p, &format!("{}.__fields[{}]", scrutinee, i))?
                    {
                        bindings.push(b);
                    }
                }

                if bindings.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bindings.join(" ")))
                }
            }
            Pattern::List(list) => {
                let mut bindings = Vec::new();

                for (i, p) in list.patterns.iter().enumerate() {
                    if let Some(b) =
                        self.generate_pattern_bindings(p, &format!("{}[{}]", scrutinee, i))?
                    {
                        bindings.push(b);
                    }
                }

                if let Some(ref rest) = list.rest {
                    bindings.push(format!(
                        "const {} = {}.slice({});",
                        sanitize_js_identifier(rest),
                        scrutinee,
                        list.patterns.len()
                    ));
                }

                if bindings.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bindings.join(" ")))
                }
            }
            Pattern::Tuple(tuple) => {
                let mut bindings = Vec::new();
                for (i, p) in tuple.patterns.iter().enumerate() {
                    if let Some(b) =
                        self.generate_pattern_bindings(p, &format!("{}[{}]", scrutinee, i))?
                    {
                        bindings.push(b);
                    }
                }

                if bindings.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(bindings.join(" ")))
                }
            }
            _ => Ok(None),
        }
    }

    fn generate_map(&mut self, map: &TypedMapExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&map.list)?;
        let func = self.generate_expression(&map.func)?;
        Ok(format!(
            "{}.then(([__items, __fn]) => __sigil_map_list(__items, __fn))",
            self.js_all(&[list, func])
        ))
    }

    fn generate_filter(&mut self, filter: &TypedFilterExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&filter.list)?;
        let predicate = self.generate_expression(&filter.predicate)?;
        Ok(format!(
            "{}.then(([__items, __predicate]) => __sigil_filter_list(__items, __predicate))",
            self.js_all(&[list, predicate])
        ))
    }

    fn generate_fold(&mut self, fold: &TypedFoldExpr) -> Result<String, CodegenError> {
        let list = self.generate_expression(&fold.list)?;
        let func = self.generate_expression(&fold.func)?;
        let init = self.generate_expression(&fold.init)?;
        // Inline fold expansion keeps generated output deterministic
        Ok(format!(
            "{}.then(([__items, __fn, __init]) => __items.reduce((__acc, x) => __acc.then((acc) => __fn(acc, x)), Promise.resolve(__init)))",
            self.js_all(&[list, func, init])
        ))
    }

    fn generate_concurrent(
        &mut self,
        concurrent: &TypedConcurrentExpr,
    ) -> Result<String, CodegenError> {
        let width = self.generate_expression(&concurrent.config.width)?;
        let jitter_ms = concurrent
            .config
            .jitter_ms
            .as_ref()
            .map(|expr| self.generate_expression(expr))
            .transpose()?
            .unwrap_or_else(|| self.js_ready("{ __tag: \"None\", __fields: [] }"));
        let stop_on = concurrent
            .config
            .stop_on
            .as_ref()
            .map(|expr| self.generate_expression(expr))
            .transpose()?
            .unwrap_or_else(|| self.js_ready("(__sigil_error) => false"));
        let window_ms = concurrent
            .config
            .window_ms
            .as_ref()
            .map(|expr| self.generate_expression(expr))
            .transpose()?
            .unwrap_or_else(|| self.js_ready("{ __tag: \"None\", __fields: [] }"));

        let mut body_lines = Vec::new();
        body_lines.push("const __sigil_tasks = [];".to_string());

        for (index, step) in concurrent.steps.iter().enumerate() {
            match step {
                TypedConcurrentStep::Spawn(spawn) => {
                    let expr = self.generate_expression(&spawn.expr)?;
                    body_lines.push(format!("__sigil_tasks.push(() => {});", expr));
                }
                TypedConcurrentStep::SpawnEach(spawn_each) => {
                    let list = self.generate_expression(&spawn_each.list)?;
                    let func = self.generate_expression(&spawn_each.func)?;
                    body_lines.push(format!(
                        "const [__sigil_items_{index}, __sigil_fn_{index}] = await {};",
                        self.js_all(&[list, func])
                    ));
                    body_lines.push(format!(
                        "for (const __sigil_item_{index} of __sigil_items_{index}) {{"
                    ));
                    body_lines.push(format!(
                        "  __sigil_tasks.push(() => __sigil_fn_{index}(__sigil_item_{index}));"
                    ));
                    body_lines.push("}".to_string());
                }
            }
        }

        let body = body_lines
            .into_iter()
            .map(|line| format!("    {}", line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(format!(
            "(async () => {{\n  const [__sigil_concurrency, __sigil_jitterMs, __sigil_stopOn, __sigil_windowMs] = await {};\n{}\n  return __sigil_concurrent_region({}, {{ concurrency: __sigil_concurrency, jitterMs: __sigil_jitterMs, stopOn: __sigil_stopOn, windowMs: __sigil_windowMs }}, __sigil_tasks);\n}})()",
            self.js_all(&[width, jitter_ms, stop_on, window_ms]),
            body,
            serde_json::to_string(&concurrent.name).unwrap()
        ))
    }

    fn generate_pipeline(&mut self, pipeline: &TypedPipelineExpr) -> Result<String, CodegenError> {
        let left = self.generate_expression(&pipeline.left)?;
        let right = self.generate_expression(&pipeline.right)?;

        match pipeline.operator {
            PipelineOperator::Pipe => {
                // a |> f becomes f(a) without eager await
                Ok(format!(
                    "{}.then(([__sigil_value, __sigil_fn]) => __sigil_fn(__sigil_value))",
                    self.js_all(&[left, right])
                ))
            }
            PipelineOperator::ComposeFwd | PipelineOperator::ComposeBwd => Err(
                CodegenError::General("Function composition not yet implemented".to_string()),
            ),
        }
    }
}

fn debug_span_matches_location(span: &DebugSpanRecord, location: SourceLocation) -> bool {
    span.location.start.line == location.start.line
        && span.location.start.column == location.start.column
        && span.location.start.offset == location.start.offset
        && span.location.end.line == location.end.line
        && span.location.end.column == location.end.column
        && span.location.end.offset == location.end.offset
}

fn sanitize_js_identifier(raw: &str) -> String {
    let mut sanitized = String::with_capacity(raw.len());

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    let sanitized = if sanitized.is_empty() {
        "_".to_string()
    } else if sanitized.chars().next().unwrap().is_ascii_digit() {
        format!("_{}", sanitized)
    } else {
        sanitized
    };

    if is_reserved_js_identifier(&sanitized) {
        format!("_{}", sanitized)
    } else {
        sanitized
    }
}

fn is_reserved_js_identifier(name: &str) -> bool {
    matches!(
        name,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn find_output_root(output_path: &Path) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    let mut last_local_root = None;

    for component in output_path.components() {
        root.push(component.as_os_str());
        if matches!(component, Component::Normal(name) if name == ".local") {
            last_local_root = Some(root.clone());
        }
    }

    last_local_root
}

fn find_project_root_for_path(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    let start = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };

    for ancestor in start.ancestors() {
        if ancestor.join("sigil.json").is_file() {
            return Some(ancestor.to_path_buf());
        }
    }

    if let Some(local_root) = find_output_root(path) {
        return local_root.parent().map(Path::to_path_buf);
    }

    None
}

fn relative_import_path(from_dir: &Path, target_file: &Path) -> String {
    let from_components: Vec<_> = from_dir.components().collect();
    let target_components: Vec<_> = target_file.components().collect();
    let common_len = from_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();

    for _ in common_len..from_components.len() {
        relative.push("..");
    }

    for component in &target_components[common_len..] {
        relative.push(component.as_os_str());
    }

    let relative_str = relative.to_string_lossy().replace('\\', "/");
    if relative_str.starts_with("../") {
        relative_str
    } else {
        format!("./{}", relative_str)
    }
}

fn remap_package_local_runtime_module_id(
    current_module_id: Option<&str>,
    imported_module_id: &str,
) -> Option<String> {
    let current_module_id = current_module_id?;
    let current_parts = current_module_id.split("::").collect::<Vec<_>>();
    if current_parts.len() < 3 {
        return None;
    }

    let package_name = current_parts[1];
    let package_version = current_parts[2];

    if imported_module_id == "config" {
        return Some(format!("packageConfig::{package_name}::{package_version}"));
    }

    if let Some(rest) = imported_module_id.strip_prefix("config::") {
        return Some(format!(
            "packageConfig::{package_name}::{package_version}::{rest}"
        ));
    }

    let Some(rest) = imported_module_id.strip_prefix("src::") else {
        return None;
    };

    if rest == "package" {
        return Some(format!("package::{package_name}::{package_version}"));
    }

    if let Some(package_rest) = rest.strip_prefix("package::") {
        if package_rest.is_empty() {
            return Some(format!("package::{package_name}::{package_version}"));
        }
        return Some(format!(
            "package::{package_name}::{package_version}::{}",
            package_rest
        ));
    }

    Some(format!(
        "package::{package_name}::{package_version}::{rest}"
    ))
}

fn collect_runtime_module_ids(program: &TypedProgram) -> BTreeSet<String> {
    let mut modules = BTreeSet::new();
    for declaration in &program.declarations {
        collect_runtime_modules_in_declaration(declaration, &mut modules);
    }
    modules.retain(|module_id| is_sigil_runtime_module(module_id));
    modules
}

fn collect_runtime_modules_in_declaration(
    declaration: &TypedDeclaration,
    modules: &mut BTreeSet<String>,
) {
    match declaration {
        TypedDeclaration::Function(function) => {
            collect_runtime_modules_in_expr(&function.body, modules)
        }
        TypedDeclaration::Const(const_decl) => {
            collect_runtime_modules_in_expr(&const_decl.value, modules)
        }
        TypedDeclaration::Test(test_decl) => {
            for binding in &test_decl.world_bindings {
                collect_runtime_modules_in_expr(&binding.value, modules);
            }
            collect_runtime_modules_in_expr(&test_decl.body, modules);
        }
        TypedDeclaration::Type(_) | TypedDeclaration::Extern(_) => {}
    }
}

fn collect_runtime_modules_in_expr(expr: &TypedExpr, modules: &mut BTreeSet<String>) {
    match &expr.kind {
        TypedExprKind::Literal(_) | TypedExprKind::Identifier(_) => {}
        TypedExprKind::NamespaceMember { namespace, .. } => {
            modules.insert(namespace.join("::"));
        }
        TypedExprKind::Lambda(lambda) => collect_runtime_modules_in_expr(&lambda.body, modules),
        TypedExprKind::Call(call) => {
            collect_runtime_modules_in_expr(&call.func, modules);
            for arg in &call.args {
                collect_runtime_modules_in_expr(arg, modules);
            }
        }
        TypedExprKind::ConstructorCall(call) => {
            if let Some(module_path) = &call.module_path {
                modules.insert(module_path.join("::"));
            }
            for arg in &call.args {
                collect_runtime_modules_in_expr(arg, modules);
            }
        }
        TypedExprKind::ExternCall(call) => {
            modules.insert(call.namespace.join("::"));
            for arg in &call.args {
                collect_runtime_modules_in_expr(arg, modules);
            }
        }
        TypedExprKind::MethodCall(call) => {
            collect_runtime_modules_in_expr(&call.receiver, modules);
            if let MethodSelector::Index(index) = &call.selector {
                collect_runtime_modules_in_expr(index, modules);
            }
            for arg in &call.args {
                collect_runtime_modules_in_expr(arg, modules);
            }
        }
        TypedExprKind::Binary(binary) => {
            collect_runtime_modules_in_expr(&binary.left, modules);
            collect_runtime_modules_in_expr(&binary.right, modules);
        }
        TypedExprKind::Unary(unary) => collect_runtime_modules_in_expr(&unary.operand, modules),
        TypedExprKind::Match(match_expr) => {
            collect_runtime_modules_in_expr(&match_expr.scrutinee, modules);
            for arm in &match_expr.arms {
                if let Some(guard) = &arm.guard {
                    collect_runtime_modules_in_expr(guard, modules);
                }
                collect_runtime_modules_in_expr(&arm.body, modules);
            }
        }
        TypedExprKind::Let(let_expr) => {
            collect_runtime_modules_in_expr(&let_expr.value, modules);
            collect_runtime_modules_in_expr(&let_expr.body, modules);
        }
        TypedExprKind::Using(using_expr) => {
            collect_runtime_modules_in_expr(&using_expr.value, modules);
            collect_runtime_modules_in_expr(&using_expr.body, modules);
        }
        TypedExprKind::If(if_expr) => {
            collect_runtime_modules_in_expr(&if_expr.condition, modules);
            collect_runtime_modules_in_expr(&if_expr.then_branch, modules);
            if let Some(else_branch) = &if_expr.else_branch {
                collect_runtime_modules_in_expr(else_branch, modules);
            }
        }
        TypedExprKind::List(list) => {
            for element in &list.elements {
                collect_runtime_modules_in_expr(element, modules);
            }
        }
        TypedExprKind::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_runtime_modules_in_expr(element, modules);
            }
        }
        TypedExprKind::Record(record) => {
            for field in &record.fields {
                collect_runtime_modules_in_expr(&field.value, modules);
            }
        }
        TypedExprKind::MapLiteral(map) => {
            for entry in &map.entries {
                collect_runtime_modules_in_expr(&entry.key, modules);
                collect_runtime_modules_in_expr(&entry.value, modules);
            }
        }
        TypedExprKind::FieldAccess(field_access) => {
            collect_runtime_modules_in_expr(&field_access.object, modules);
        }
        TypedExprKind::Index(index) => {
            collect_runtime_modules_in_expr(&index.object, modules);
            collect_runtime_modules_in_expr(&index.index, modules);
        }
        TypedExprKind::Map(map) => {
            collect_runtime_modules_in_expr(&map.list, modules);
            collect_runtime_modules_in_expr(&map.func, modules);
        }
        TypedExprKind::Filter(filter) => {
            collect_runtime_modules_in_expr(&filter.list, modules);
            collect_runtime_modules_in_expr(&filter.predicate, modules);
        }
        TypedExprKind::Fold(fold) => {
            collect_runtime_modules_in_expr(&fold.list, modules);
            collect_runtime_modules_in_expr(&fold.func, modules);
            collect_runtime_modules_in_expr(&fold.init, modules);
        }
        TypedExprKind::Concurrent(concurrent) => {
            collect_runtime_modules_in_expr(&concurrent.config.width, modules);
            if let Some(jitter_ms) = &concurrent.config.jitter_ms {
                collect_runtime_modules_in_expr(jitter_ms, modules);
            }
            if let Some(stop_on) = &concurrent.config.stop_on {
                collect_runtime_modules_in_expr(stop_on, modules);
            }
            if let Some(window_ms) = &concurrent.config.window_ms {
                collect_runtime_modules_in_expr(window_ms, modules);
            }
            for step in &concurrent.steps {
                match step {
                    TypedConcurrentStep::Spawn(spawn) => {
                        collect_runtime_modules_in_expr(&spawn.expr, modules)
                    }
                    TypedConcurrentStep::SpawnEach(spawn_each) => {
                        collect_runtime_modules_in_expr(&spawn_each.list, modules);
                        collect_runtime_modules_in_expr(&spawn_each.func, modules);
                    }
                }
            }
        }
        TypedExprKind::Pipeline(pipeline) => {
            collect_runtime_modules_in_expr(&pipeline.left, modules);
            collect_runtime_modules_in_expr(&pipeline.right, modules);
        }
    }
}

fn is_sigil_runtime_module(module_id: &str) -> bool {
    module_id.starts_with("core::")
        || module_id.starts_with("stdlib::")
        || module_id.starts_with("world::")
        || module_id.starts_with("test::")
        || module_id.starts_with("src::")
        || module_id == "config"
        || module_id.starts_with("config::")
        || module_id.starts_with("package::")
        || module_id.starts_with("packageConfig::")
}

fn requires_world_runtime(program: &TypedProgram, runtime_modules: &BTreeSet<String>) -> bool {
    program
        .declarations
        .iter()
        .any(|declaration| matches!(declaration, TypedDeclaration::Test(_)))
        || runtime_modules
            .iter()
            .any(|module_id| requires_world_runtime_module(module_id))
}

fn requires_world_runtime_module(module_id: &str) -> bool {
    module_id == "config"
        || module_id.starts_with("config::")
        || module_id.starts_with("packageConfig::")
        || module_id.starts_with("world::")
        || module_id.starts_with("test::")
        || matches!(
            module_id,
            "stdlib::cli"
                | "stdlib::file"
                | "stdlib::fsWatch"
                | "stdlib::httpClient"
                | "stdlib::httpServer"
                | "stdlib::io"
                | "stdlib::log"
                | "stdlib::process"
                | "stdlib::pty"
                | "stdlib::random"
                | "stdlib::sql"
                | "stdlib::stream"
                | "stdlib::task"
                | "stdlib::tcpClient"
                | "stdlib::tcpServer"
                | "stdlib::terminal"
                | "stdlib::timer"
                | "stdlib::time"
                | "stdlib::websocket"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::{tokenize, Position, SourceLocation};
    use sigil_parser::parse;
    use sigil_typechecker::type_check;
    use sigil_typechecker::typed_ir::{PurityClass, StrictnessClass};
    use sigil_typechecker::types::{InferenceType, TList};
    use std::collections::HashSet;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn typed_program_for(source: &str, path: &str) -> TypedProgram {
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, path).unwrap();
        type_check(&program, source, None).unwrap().typed_program
    }

    fn test_location() -> SourceLocation {
        SourceLocation::single(Position::new(1, 1, 0))
    }

    fn temp_node_script_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sigil-codegen-{}-{}-{}.mjs",
            prefix,
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn test_empty_program() {
        let program = TypedProgram {
            declarations: vec![],
        };

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_function() {
        let source = "λadd(x:Int,y:Int)=>Int=x+y";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        // Should contain a plain function that returns promise-shaped values
        assert!(result.contains("function add"));
        assert!(!result.contains("async function add"));
        // Should contain return statement
        assert!(result.contains("return"));
        // Should contain parameters
        assert!(result.contains("x, y"));
    }

    #[test]
    fn test_sum_type_constructors() {
        let source = "t Color=Red|Green|Blue";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        // Should contain constructor functions without eager async wrappers
        assert!(result.contains("function Red"));
        assert!(result.contains("function Green"));
        assert!(result.contains("function Blue"));
        assert!(!result.contains("async function Red"));
        // Should use __tag pattern
        assert!(result.contains("__tag"));
    }

    #[test]
    fn test_core_prelude_result_helper_codegen() {
        let source = "t Result[T,E]=Ok(T)|Err(E)\nλnormalize[T,E](res:Result[T,E])=>Result[T,E] match res{Ok(value)=>Ok(value)|Err(error)=>Err(error)}";
        let program = typed_program_for(source, "test.lib.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function normalize"));
        assert!(result.contains("__tag"));
    }

    #[test]
    fn test_regular_function_calls_route_through_call_runtime() {
        let source = "λping()=>String=\"real\"\nλmain()=>String=ping()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_call(_key, actualFn, args = [])"));
        assert!(result.contains("__sigil_call(\"ping\", ping, __sigil_args)"));
    }

    #[test]
    fn test_generate_import_sanitizes_alias_and_uses_relative_path() {
        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: None,
            source_file: Some("projects/algorithms/tests/rot13Encoder.sigil".to_string()),
            output_file: Some("/tmp/projects/algorithms/.local/tests/rot13Encoder.ts".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        gen.emit_module_import("src::rot13Encoder").unwrap();
        let result = gen.output.join("");

        assert!(result.contains("import * as src_rot13Encoder from '../src/rot13Encoder.js';"));
    }

    #[test]
    fn test_generate_import_uses_local_root_for_stdlib_test_outputs() {
        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: None,
            source_file: Some("language/stdlib-tests/tests/numericPredicates.sigil".to_string()),
            output_file: Some(
                "/tmp/language/stdlib-tests/.local/tests/numericPredicates.ts".to_string(),
            ),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        gen.emit_module_import("stdlib::numeric").unwrap();
        let result = gen.output.join("");

        assert!(result.contains("import * as stdlib_numeric from '../stdlib/numeric.js';"));
    }

    #[test]
    fn test_generate_import_prefers_deepest_local_root() {
        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: None,
            source_file: Some("/repo/.local/temp-project/src/topology.lib.sigil".to_string()),
            output_file: Some("/repo/.local/temp-project/.local/src/topology.ts".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        gen.emit_module_import("stdlib::topology").unwrap();
        let result = gen.output.join("");

        assert!(result.contains("import * as stdlib_topology from '../stdlib/topology.js';"));
    }

    #[test]
    fn test_generate_import_remaps_package_local_src_module_paths() {
        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("package::featureFlagStorefrontFlags::v20260412_140000::flags".to_string()),
            source_file: Some(
                "/repo/projects/app/.sigil/packages/featureFlagStorefrontFlags/2026-04-12T14-00-00Z/src/flags.lib.sigil"
                    .to_string(),
            ),
            output_file: Some(
                "/repo/projects/app/.local/package/featureFlagStorefrontFlags/v20260412_140000/flags.ts"
                    .to_string(),
            ),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        gen.emit_module_import("src::types").unwrap();
        let result = gen.output.join("");

        assert!(result.contains("import * as src_types from './types.js';"));
    }

    #[test]
    fn test_generate_extern_namespace_uses_full_sanitized_alias() {
        let source = "e fs::promises\nλmain()=>Unit=()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import * as fs_promises from 'fs/promises';"));
    }

    #[test]
    fn test_generate_project_bridge_typed_extern_uses_relative_import() {
        let source =
            "e bridge::ptyAdapter:{open:λ()=>String}\nλmain()=>String=bridge::ptyAdapter.open()";
        let program = typed_program_for(source, "projects/syntarch/src/runtime.lib.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::runtime".to_string()),
            source_file: Some("/tmp/workspace/projects/syntarch/src/runtime.lib.sigil".to_string()),
            output_file: Some(
                "/tmp/workspace/projects/syntarch/.local/generated/runtime.ts".to_string(),
            ),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("import { open } from '../../bridges/ptyAdapter.js';"));
    }

    #[test]
    fn test_generate_project_bridge_lazy_extern_uses_relative_runtime_import() {
        let source = "e bridge::ptyAdapter\nλmain()=>Unit=()";
        let program = typed_program_for(source, "projects/syntarch/src/runtime.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::runtime".to_string()),
            source_file: Some("/tmp/workspace/projects/syntarch/src/runtime.sigil".to_string()),
            output_file: Some(
                "/tmp/workspace/projects/syntarch/.local/generated/runtime.mjs".to_string(),
            ),
            import_extension: "mjs".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: true,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(
            "const bridge_ptyAdapter = __sigil_runtime_extern_namespace(\"bridge::ptyAdapter\", \"../../bridges/ptyAdapter.mjs\");"
        ));
    }

    #[test]
    fn test_generate_match_with_guard_falls_through_to_later_arms() {
        let source =
            "λclassify(x:Int)=>String match x{n when n>1=>\"big\"|0=>\"zero\"|_=>\"other\"}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("if (__match === 0)"));
        assert!(!result.contains("else if (__match === 0)"));
    }

    #[test]
    fn test_generate_list_preserves_nested_lists() {
        let source = "λwrap(xs:[Int])=>[[Int]]=[xs]";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(".then((__items) => __items)"));
        assert!(!result.contains("[].concat(xs)"));
    }

    #[test]
    fn test_generate_list_append_parenthesizes_awaited_left_side() {
        let source = "λleft()=>[Int]=[1]\nλright()=>[Int]=[2]\nλmain()=>[Int]=left()⧺right()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains(".concat("));
        assert!(!result.contains("await left().concat("));
    }

    #[test]
    fn test_generate_qualified_constructor_call_without_mock_wrapper() {
        let program = TypedProgram {
            declarations: vec![TypedDeclaration::Function(TypedFunctionDecl {
                name: "main".to_string(),
                type_params: vec![],
                params: vec![],
                return_type: InferenceType::Any,
                effects: None,
                requires: None,
                decreases: None,
                ensures: None,
                body: TypedExpr {
                    kind: TypedExprKind::ConstructorCall(TypedConstructorCallExpr {
                        module_path: Some(vec!["src".to_string(), "graphTypes".to_string()]),
                        constructor: "Ordering".to_string(),
                        args: vec![TypedExpr {
                            kind: TypedExprKind::List(TypedListExpr { elements: vec![] }),
                            typ: InferenceType::List(Box::new(TList {
                                element_type: InferenceType::Any,
                            })),
                            effects: HashSet::new(),
                            purity: PurityClass::Pure,
                            strictness: StrictnessClass::Deferred,
                            location: test_location(),
                        }],
                    }),
                    typ: InferenceType::Any,
                    effects: HashSet::new(),
                    purity: PurityClass::Pure,
                    strictness: StrictnessClass::Deferred,
                    location: test_location(),
                },
                location: test_location(),
            })],
        };

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: None,
            source_file: Some("projects/algorithms/src/topologicalSort.sigil".to_string()),
            output_file: Some("/tmp/projects/algorithms/.local/src/topologicalSort.ts".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("src_graphTypes.Ordering"));
        assert!(!result.contains("__sigil_call(\"extern:src/graphTypes.Ordering\""));
    }

    #[test]
    fn test_generate_test_metadata_includes_id_and_location() {
        let source = "λmain()=>Unit=()\n\ntest \"smoke\" { true }";
        let program = typed_program_for(source, "tests/smoke.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: None,
            source_file: Some("tests/smoke.sigil".to_string()),
            output_file: Some("/tmp/tests/smoke.ts".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("id: \"tests/smoke.sigil::smoke\""));
        assert!(result.contains("name: \"smoke\""));
        assert!(result.contains("description: \"smoke\""));
        assert!(result.contains("location: { start: { line: 3, column: 1 } }"));
    }

    #[test]
    fn test_generate_span_map_includes_function_and_nested_expression_spans() {
        let source = "λmain()=>Int=1+2";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        gen.generate(&program).unwrap();

        let span_map = gen.generated_span_map().unwrap();
        assert_eq!(span_map.module_id, "src::main");
        assert_eq!(span_map.source_file, "test.sigil");
        assert_eq!(span_map.output_file, "/tmp/test.js");
        assert!(span_map.spans.iter().any(|span| {
            span.kind == DebugSpanKind::FunctionDecl
                && span.label.as_deref() == Some("main")
                && span.generated_range.is_some()
        }));
        assert!(span_map
            .spans
            .iter()
            .any(|span| span.kind == DebugSpanKind::ExprBinary && span.generated_range.is_none()));
    }

    #[test]
    fn test_generate_span_map_includes_match_arm_hierarchy() {
        let source = "λmain(x:Bool)=>Int match x{true=>1|false=>0}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        gen.generate(&program).unwrap();

        let span_map = gen.generated_span_map().unwrap();
        let match_span_id = span_map
            .spans
            .iter()
            .find(|span| span.kind == DebugSpanKind::ExprMatch)
            .map(|span| span.span_id.clone())
            .unwrap();
        let arm_spans = span_map
            .spans
            .iter()
            .filter(|span| span.kind == DebugSpanKind::MatchArm)
            .collect::<Vec<_>>();
        assert_eq!(arm_spans.len(), 2);
        assert!(arm_spans
            .iter()
            .all(|span| span.parent_span_id.as_deref() == Some(match_span_id.as_str())));
    }

    #[test]
    fn test_generate_trace_enabled_instruments_declared_calls_and_match_selection() {
        let source =
            "λhelper(flag:Bool)=>Int match flag{true=>1|false=>0}\nλmain()=>Int=helper(true)";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: true,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_debug_wrap_call("));
        assert!(result.contains("__sigil_trace_branch_match("));
        assert!(result.contains("__sigil_debug_wrap_call({ moduleId: \"src::main\""));
    }

    #[test]
    fn test_generate_trace_enabled_instruments_effectful_extern_calls() {
        let source =
            "e process:{argv:λ()=>!Process [String]}\nλmain()=>!Process [String]=process.argv()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: true,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_trace_wrap_effect("));
        assert!(result.contains("__sigil_call("));
    }

    #[test]
    fn test_generate_expression_debug_wraps_nested_expressions() {
        let source = "e boom:{explode:λ()=>Int}\nλmain()=>Int=1+boom.explode()";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: true,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_debug_wrap_expression("));
        assert!(result.contains("globalThis.__sigil_expression_exception_payload"));
        assert!(result.contains("kind: 'expr_throw'"));
        assert!(result.contains("spanKind: \"expr_binary\""));
    }

    #[test]
    fn test_generate_breakpoints_emit_function_and_let_scope_helpers() {
        let source = "λmain(x:Int)=>Int={l y=(x+1:Int);y}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: true,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_breakpoint_snapshot()"));
        assert!(result.contains("__sigil_breakpoint_push_frame("));
        assert!(result.contains(
            "__sigil_breakpoint_push_scope([{ name: \"y\", origin: \"let\", value: y, typeId: null }])"
        ));
        assert!(result.contains("__sigil_breakpoint_maybe_hit("));
    }

    #[test]
    fn test_generate_breakpoints_emit_named_type_ids_for_param_and_let_locals() {
        let source =
            "t UserId=Int where value≥0\nλmain(userId:UserId)=>UserId={l current=(userId:UserId);current}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: true,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("[\"src::main.UserId\"]"));
        assert!(result.contains(
            "__sigil_breakpoint_push_scope([{ name: \"current\", origin: \"let\", value: current, typeId: \"src::main.UserId\" }])"
        ));
    }

    #[test]
    fn test_generate_breakpoints_emit_pattern_scope_for_match_arms() {
        let source = "λmain(value:Bool)=>Int match value{true=>{l n=(1:Int);n}|false=>0}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: true,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_breakpoint_push_scope([]);"));
        assert!(result.contains("__sigil_breakpoint_maybe_hit("));
        assert!(result.contains("__sigil_debug_wrap_expression("));
    }

    #[test]
    fn test_generate_map_uses_ordered_helper_not_promise_all_map() {
        let source = "λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_map_list"));
        assert!(!result.contains("Promise.all(__items.map"));
    }

    #[test]
    fn test_pure_lib_codegen_omits_world_runtime_helpers() {
        let source = "λdouble(xs:[Int])=>[Int]=xs map (λ(x:Int)=>Int=x*2)";
        let program = typed_program_for(source, "test.lib.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::double".to_string()),
            source_file: Some("test.lib.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_map_list"));
        assert!(result.contains("function __sigil_record_coverage_call(moduleId, functionName)"));
        assert!(!result.contains("function __sigil_world_error(message)"));
        assert!(!result.contains("node:fs/promises"));
    }

    #[test]
    fn test_world_backed_stdlib_codegen_keeps_world_runtime_helpers() {
        let program = TypedProgram {
            declarations: vec![TypedDeclaration::Function(TypedFunctionDecl {
                name: "main".to_string(),
                type_params: vec![],
                params: vec![],
                return_type: InferenceType::Any,
                effects: None,
                requires: None,
                decreases: None,
                ensures: None,
                body: TypedExpr {
                    kind: TypedExprKind::Call(TypedCallExpr {
                        func: Box::new(TypedExpr {
                            kind: TypedExprKind::NamespaceMember {
                                namespace: vec!["stdlib".to_string(), "process".to_string()],
                                member: "argv".to_string(),
                            },
                            typ: InferenceType::Any,
                            effects: HashSet::new(),
                            purity: PurityClass::Effectful,
                            strictness: StrictnessClass::Deferred,
                            location: test_location(),
                        }),
                        args: vec![],
                    }),
                    typ: InferenceType::Any,
                    effects: HashSet::new(),
                    purity: PurityClass::Effectful,
                    strictness: StrictnessClass::Deferred,
                    location: test_location(),
                },
                location: test_location(),
            })],
        };

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_world_error(message)"));
        assert!(result.contains("__sigil_world_process_argv()"));
    }

    #[test]
    fn test_stream_stdlib_codegen_keeps_world_runtime_helpers() {
        let program = TypedProgram {
            declarations: vec![TypedDeclaration::Function(TypedFunctionDecl {
                name: "main".to_string(),
                type_params: vec![],
                params: vec![],
                return_type: InferenceType::Any,
                effects: None,
                requires: None,
                decreases: None,
                ensures: None,
                body: TypedExpr {
                    kind: TypedExprKind::Call(TypedCallExpr {
                        func: Box::new(TypedExpr {
                            kind: TypedExprKind::NamespaceMember {
                                namespace: vec!["stdlib".to_string(), "stream".to_string()],
                                member: "next".to_string(),
                            },
                            typ: InferenceType::Any,
                            effects: HashSet::new(),
                            purity: PurityClass::Effectful,
                            strictness: StrictnessClass::Deferred,
                            location: test_location(),
                        }),
                        args: vec![TypedExpr {
                            kind: TypedExprKind::Identifier(sigil_ast::IdentifierExpr {
                                name: "source".to_string(),
                                location: test_location(),
                            }),
                            typ: InferenceType::Any,
                            effects: HashSet::new(),
                            purity: PurityClass::Pure,
                            strictness: StrictnessClass::Deferred,
                            location: test_location(),
                        }],
                    }),
                    typ: InferenceType::Any,
                    effects: HashSet::new(),
                    purity: PurityClass::Effectful,
                    strictness: StrictnessClass::Deferred,
                    location: test_location(),
                },
                location: test_location(),
            })],
        };

        let mut gen = TypeScriptGenerator::new(CodegenOptions {
            module_id: Some("src::main".to_string()),
            source_file: Some("test.sigil".to_string()),
            output_file: Some("/tmp/test.js".to_string()),
            import_extension: "js".to_string(),
            fswatch_runtime_import_specifier: None,
            pty_runtime_import_specifier: None,
            websocket_runtime_import_specifier: None,
            sql_runtime_import_specifier: None,
            lazy_extern_namespaces: false,
            trace: false,
            breakpoints: false,
            expression_debug: false,
        });
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("function __sigil_world_error(message)"));
        assert!(result.contains("__sigil_world_stream_next(__source)"));
    }

    #[test]
    fn test_stream_runtime_helpers_yield_items_then_done_and_close_is_terminal() {
        let script = format!(
            r#"{}
const __sigil_world = __sigil_world_fresh(__sigil_world_host_template());
const __sigil_result = await __sigil_with_world(__sigil_world, async () => {{
  const source = __sigil_world_stream_test_source([1, 2]);
  const first = await __sigil_world_stream_next(source);
  const second = await __sigil_world_stream_next(source);
  const third = await __sigil_world_stream_next(source);
  const closable = __sigil_world_stream_test_source([3, 4]);
  await __sigil_world_stream_close(closable);
  const closed = await __sigil_world_stream_next(closable);
  return {{ first, second, third, closed }};
}});
console.log(JSON.stringify(__sigil_result));
"#,
            world_runtime_helpers_source()
        );

        let script_path = temp_node_script_path("stream-runtime");
        fs::write(&script_path, script).unwrap();
        let output = Command::new("node").arg(&script_path).output().unwrap();
        let _ = fs::remove_file(&script_path);

        assert!(output.status.success(), "{:?}", output);
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json["first"]["__tag"], "Item");
        assert_eq!(json["first"]["__fields"][0], 1);
        assert_eq!(json["second"]["__tag"], "Item");
        assert_eq!(json["second"]["__fields"][0], 2);
        assert_eq!(json["third"]["__tag"], "Done");
        assert_eq!(json["closed"]["__tag"], "Done");
    }

    #[test]
    fn test_generate_concurrent_region_uses_scheduler_helper() {
        let source = "e clock:{tick:λ()=>!Timer Unit}\nt ConcurrentOutcome[T,E]=Aborted()|Failure(E)|Success(T)\nt Option[T]=Some(T)|None()\nt Result[T,E]=Ok(T)|Err(E)\nλmain()=>!Timer [ConcurrentOutcome[Int,String]]=concurrent urlAudit@2{spawnEach [1,2] process}\nλprocess(value:Int)=>!Timer Result[Int,String]={l _=(clock.tick():Unit);Ok(value)}";
        let program = typed_program_for(source, "test.sigil");

        let mut gen = TypeScriptGenerator::new(CodegenOptions::default());
        let result = gen.generate(&program).unwrap();

        assert!(result.contains("__sigil_concurrent_region(\"urlAudit\""));
        assert!(result.contains("__sigil_tasks.push(() => __sigil_fn_0(__sigil_item_0));"));
    }
}
