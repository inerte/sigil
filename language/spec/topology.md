# Topology Specification

## Purpose

Sigil topology defines the canonical representation of external runtime
dependencies for topology-aware projects.

Topology is declaration only.
Concrete environment bindings live in config modules.

## Canonical Files

A topology-aware project uses:

```text
src/topology.lib.sigil
config/<env>.lib.sigil
```

`src/topology.lib.sigil` is the canonical source of truth for:
- declared dependency handles
- declared environment names

`config/<env>.lib.sigil` is the canonical source of truth for:
- concrete bindings for one selected environment

## Topology Surface

`stdlib::topology` defines:

```sigil decl stdlib::topology
t Environment=Environment(String)
t HttpServiceDependency=HttpServiceDependency(String)
t TcpServiceDependency=TcpServiceDependency(String)

λenvironment(name:String)=>Environment
λhttpService(name:String)=>HttpServiceDependency
λtcpService(name:String)=>TcpServiceDependency
```

`stdlib::config` defines:

```sigil decl stdlib::config
t BindingValue=EnvVar(String)|Literal(String)
t Bindings={httpBindings:[HttpBinding],tcpBindings:[TcpBinding]}
t HttpBinding={baseUrl:BindingValue,dependencyName:String}
t PortBindingValue=EnvVarPort(String)|LiteralPort(Int)
t TcpBinding={dependencyName:String,host:BindingValue,port:PortBindingValue}

λbindHttp(baseUrl:String,dependency:stdlib::topology.HttpServiceDependency)=>HttpBinding
λbindHttpEnv(dependency:stdlib::topology.HttpServiceDependency,envVar:String)=>HttpBinding
λbindTcp(dependency:stdlib::topology.TcpServiceDependency,host:String,port:Int)=>TcpBinding
λbindTcpEnv(dependency:stdlib::topology.TcpServiceDependency,hostEnvVar:String,portEnvVar:String)=>TcpBinding
λbindings(httpBindings:[HttpBinding],tcpBindings:[TcpBinding])=>Bindings
```

## Compile-Time Rules

### Topology declaration location

Calls to these constructors are only valid in `src/topology.lib.sigil`:
- `stdlib::topology.httpService`
- `stdlib::topology.tcpService`
- `stdlib::topology.environment`

### Config binding location

Calls to these constructors are only valid in `config/*.lib.sigil`:
- `stdlib::config.bindHttp`
- `stdlib::config.bindHttpEnv`
- `stdlib::config.bindTcp`
- `stdlib::config.bindTcpEnv`
- `stdlib::config.bindings`

### Ambient env access

`process.env` access is only valid in `config/*.lib.sigil`.

It is invalid in:
- `src/topology.lib.sigil`
- ordinary application modules
- tests
- any other project source file

### Dependency-aware API usage

Topology-aware HTTP/TCP APIs require dependency handles:
- `stdlib::httpClient.*` requires `HttpServiceDependency`
- `stdlib::tcpClient.*` requires `TcpServiceDependency`

The compiler rejects:
- raw URLs passed to topology-aware HTTP client APIs
- raw host/port values passed to topology-aware TCP client APIs
- dependency kind mismatches

## Validate-Time Rules

Validation is environment-specific.

For selected environment `<env>`:
- `src/topology.lib.sigil` must exist
- `<env>` must be declared in topology
- `config/<env>.lib.sigil` must exist
- `config/<env>.lib.sigil` must export `bindings`
- every declared dependency must be bound exactly once
- no undeclared dependencies may be bound
- binding kinds must match dependency kinds
- dependency names must be unique in topology

## Execution Model

Topology-aware commands require an explicit environment:

```bash
sigil validate <project> --env <name>
sigil run <file> --env <name>
sigil test <path> --env <name>
```

Sigil does not provide an implicit default environment for topology-aware
projects.

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
