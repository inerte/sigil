# Topology Specification

## Purpose

Sigil topology defines the canonical representation of named runtime
boundaries for topology-aware projects.

Topology is declaration only.
Concrete environment worlds live in config modules.

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
t TcpServiceDependency=TcpServiceDependency(String)

λenvironment(name:String)=>Environment
λfsRoot(name:String)=>FsRoot
λhttpService(name:String)=>HttpServiceDependency
λlogSink(name:String)=>LogSink
λprocessHandle(name:String)=>ProcessHandle
λtcpService(name:String)=>TcpServiceDependency
```

`†runtime` and world entry roots define the canonical env surface:

```sigil decl †runtime
t World={clock:†clock.ClockEntry,fs:†fs.FsEntry,fsRoots:[†fs.FsRootEntry],http:[†http.HttpEntry],log:†log.LogEntry,logSinks:[†log.LogSinkEntry],process:†process.ProcessEntry,processHandles:[†process.ProcessHandleEntry],random:†random.RandomEntry,tcp:[†tcp.TcpEntry],timer:†timer.TimerEntry}

λworld(clock:†clock.ClockEntry,fs:†fs.FsEntry,http:[†http.HttpEntry],log:†log.LogEntry,process:†process.ProcessEntry,random:†random.RandomEntry,tcp:[†tcp.TcpEntry],timer:†timer.TimerEntry)=>World
λwithFsRoots(fsRoots:[†fs.FsRootEntry],world:World)=>World
λwithLogSinks(logSinks:[†log.LogSinkEntry],world:World)=>World
λwithProcessHandles(processHandles:[†process.ProcessHandleEntry],world:World)=>World
```

## Compile-Time Rules

### Topology declaration location

Calls to these constructors are only valid in `src/topology.lib.sigil`:
- `§topology.fsRoot`
- `§topology.httpService`
- `§topology.logSink`
- `§topology.processHandle`
- `§topology.tcpService`
- `§topology.environment`

### World entry location

Calls to `†http.*`, `†fs.*Root`, `†log.*Sink`, and `†process.*Handle` entry constructors are only valid in:

- `config/*.lib.sigil`
- test-local `world { ... }` clauses

### Ambient env access

`process.env` access is only valid in `config/*.lib.sigil`.

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
- `§log.write` requires `LogSink`
- `§process.runAt` / `§process.startAt` require `ProcessHandle`

## Validate-Time Rules

Validation is environment-specific.

For selected environment `<env>`:
- `src/topology.lib.sigil` must exist
- `<env>` must be declared in topology
- `config/<env>.lib.sigil` must exist
- `config/<env>.lib.sigil` must export `world`
- `world` must provide all primitive effect entries
- every declared named boundary must appear in the matching `world` entry collection
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

## Test-World Observation

Topology-aware tests assert exact named-boundary outcomes through the active
test world. Canonical examples include:

- `※check::file.existsAt(path,•topology.exportsDir)`
- `※check::log.containsAt(message,•topology.auditLog)`
- `※observe::process.commandsAt(•topology.govBrCli)`

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
