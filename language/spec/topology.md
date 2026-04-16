# Topology Specification

## Purpose

Sigil topology defines the canonical representation of named runtime
boundaries for topology-aware projects.

Topology is declaration only.
Concrete environment worlds live in config modules.

Outside projects, Sigil uses the same topology and world constructors directly
in a single file with ordinary local names and a local top-level `c world`.

## Canonical Files

A topology-aware project uses:

```text
src/topology.lib.sigil
config/<env>.lib.sigil
```

`src/topology.lib.sigil` is the canonical source of truth for:
- declared boundary handles
- declared environment names

`config/<env>.lib.sigil` is the canonical source of truth for:
- one selected environment's runtime world
- selected env declarations exposed to application code through `•config.<name>`

`src/policies.lib.sigil` is the canonical source of truth for:
- boundary rules over labelled data
- trusted transforms referenced by `Through(...)`

## Topology Surface

`§topology` defines:

```sigil decl §topology
t Environment=Environment(String)
t FsRoot=FsRoot(String)
t HttpServiceDependency=HttpServiceDependency(String)
t LogSink=LogSink(String)
t ProcessHandle=ProcessHandle(String)
t PtyHandle=PtyHandle(String)
t TcpServiceDependency=TcpServiceDependency(String)
t WebSocketHandle=WebSocketHandle(String)

λenvironment(name:String)=>Environment
λfsRoot(name:String)=>FsRoot
λhttpService(name:String)=>HttpServiceDependency
λlogSink(name:String)=>LogSink
λprocessHandle(name:String)=>ProcessHandle
λptyHandle(name:String)=>PtyHandle
λtcpService(name:String)=>TcpServiceDependency
λwebsocketHandle(name:String)=>WebSocketHandle
```

`†runtime` and world entry roots define the canonical env surface:

```sigil decl †runtime
 t World={clock:†clock.ClockEntry,fs:†fs.FsEntry,fsRoots:[†fs.FsRootEntry],fsWatch:†fsWatch.FsWatchEntry,fsWatchRoots:[†fsWatch.FsWatchRootEntry],http:[†http.HttpEntry],log:†log.LogEntry,logSinks:[†log.LogSinkEntry],process:†process.ProcessEntry,processHandles:[†process.ProcessHandleEntry],pty:†pty.PtyEntry,ptyHandles:[†pty.PtyHandleEntry],random:†random.RandomEntry,stream:†stream.StreamEntry,tcp:[†tcp.TcpEntry],timer:†timer.TimerEntry,websocket:†websocket.WebSocketEntry,websocketHandles:[†websocket.WebSocketHandleEntry]}

λworld(clock:†clock.ClockEntry,fs:†fs.FsEntry,fsWatch:†fsWatch.FsWatchEntry,http:[†http.HttpEntry],log:†log.LogEntry,process:†process.ProcessEntry,pty:†pty.PtyEntry,random:†random.RandomEntry,stream:†stream.StreamEntry,tcp:[†tcp.TcpEntry],timer:†timer.TimerEntry,websocket:†websocket.WebSocketEntry)=>World
λwithFsRoots(fsRoots:[†fs.FsRootEntry],world:World)=>World
λwithFsWatchRoots(fsWatchRoots:[†fsWatch.FsWatchRootEntry],world:World)=>World
λwithLogSinks(logSinks:[†log.LogSinkEntry],world:World)=>World
λwithProcessHandles(processHandles:[†process.ProcessHandleEntry],world:World)=>World
λwithPtyHandles(ptyHandles:[†pty.PtyHandleEntry],world:World)=>World
λwithWebSocketHandles(websocketHandles:[†websocket.WebSocketHandleEntry],world:World)=>World
```

## Compile-Time Rules

### Topology declaration location

In project mode, calls to these constructors are only valid in
`src/topology.lib.sigil`:
- `§topology.fsRoot`
- `§topology.httpService`
- `§topology.logSink`
- `§topology.processHandle`
- `§topology.ptyHandle`
- `§topology.tcpService`
- `§topology.websocketHandle`
- `§topology.environment`

### World entry location

In project mode, calls to `†http.*`, `†fs.*Root`, `†fsWatch.*`, `†fsWatch.*Root`, `†log.*Sink`, `†process.*`, `†pty.*`, and `†websocket.*` world-entry constructors are only valid in:

- `config/*.lib.sigil`
- test-local `world { ... }` clauses

### Ambient env access

In project mode, `process.env` access is only valid in `config/*.lib.sigil`.

In standalone mode, `process.env` may be read directly because there is no
separate config module.

It is invalid in:
- `src/topology.lib.sigil`
- ordinary application modules
- tests
- any other project source file

### Dependency-aware API usage

Topology-aware HTTP/TCP APIs require dependency handles:
- `§httpClient.*` requires `HttpServiceDependency`
- `§tcpClient.*` requires `TcpServiceDependency`

The compiler rejects:
- raw URLs passed to topology-aware HTTP client APIs
- raw host/port values passed to topology-aware TCP client APIs
- dependency kind mismatches

Label-aware boundary rules operate on exact named boundaries:
- `§file.*At` requires `FsRoot`
- `§fsWatch.watchAt` requires `FsRoot`
- `§log.write` requires `LogSink`
- `§process.runAt` / `§process.startAt` require `ProcessHandle`
- `§pty.spawnAt` requires `PtyHandle`
- `§websocket.route` / `§websocket.connections` require `WebSocketHandle`

## Validate-Time Rules

Validation is environment-specific.

For selected environment `<env>`:
- `src/topology.lib.sigil` must exist
- `<env>` must be declared in topology
- `config/<env>.lib.sigil` must exist
- `config/<env>.lib.sigil` must export `world`
- `world` must provide all primitive effect entries
- every declared `FsRoot` must appear in both `fsRoots` and `fsWatchRoots`
- every other declared named boundary must appear in the matching `world` entry collection
- no undeclared boundaries may appear in `world`
- boundary names must be unique in topology
- non-`world` top-level declarations in that config module are exposed through
  the selected config root `•config.<name>`

## Execution Model

Topology-aware commands require an explicit environment:

```bash
sigil validate <project> --env <name>
sigil run <file> --env <name>
sigil test <path> --env <name>
```

Sigil does not provide an implicit default environment for topology-aware
projects or for code that reads `•config.<name>`.

Standalone files with a local top-level `c world` do not require `--env`.

## Test-World Observation

Topology-aware tests assert exact named-boundary outcomes through the active
test world. Canonical examples include:

- `※check::file.existsAt(path,•topology.exportsDir)`
- `※check::fsWatch.watchingAt(path,•topology.exportsDir)`
- `※check::log.containsAt(message,•topology.auditLog)`
- `※observe::fsWatch.watchesAt(•topology.exportsDir)`
- `※observe::process.commandsAt(•topology.govBrCli)`
- `※observe::pty.writesAt(•topology.assistantShell)`
- `※check::pty.spawnedOnceAt(•topology.assistantShell)`
- `※observe::websocket.receivedAt(•topology.liveUpdates)`
- `※check::websocket.connectedOnceAt(•topology.liveUpdates)`

## Diagnostics

Topology diagnostics use `SIGIL-TOPO-*`.

Current codes include:
- `SIGIL-TOPO-MISSING-MODULE`
- `SIGIL-TOPO-MISSING-CONFIG-MODULE`
- `SIGIL-TOPO-INVALID-CONFIG-MODULE`
- `SIGIL-TOPO-ENV-REQUIRED`
- `SIGIL-TOPO-ENV-NOT-FOUND`
- `SIGIL-TOPO-ENV-ACCESS-LOCATION`
- `SIGIL-TOPO-CONSTRUCTOR-LOCATION`
- `SIGIL-TOPO-RAW-ENDPOINT-FORBIDDEN`
- `SIGIL-TOPO-DEPENDENCY-KIND-MISMATCH`
- `SIGIL-TOPO-INVALID-HANDLE`
- `SIGIL-TOPO-DUPLICATE-DEPENDENCY`
- `SIGIL-TOPO-DUPLICATE-BINDING`
- `SIGIL-TOPO-MISSING-BINDING`
- `SIGIL-TOPO-BINDING-KIND-MISMATCH`
