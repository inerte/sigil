# Topology

<h2 id="topology-is-runtime-truth">Topology Is Runtime Truth</h2>

Sigil topology is the canonical, compiler-visible declaration of a project's
named runtime boundaries.

Topology is not config.

Topology answers:
- what named boundaries exist outside ordinary local computation
- what those boundaries are called in application code
- which environment names exist

Config answers:
- how one named environment constructs the runtime world for those boundaries

## Why Sigil Splits Topology from Config

Without this split, runtime truth gets blurred together:
- architecture and credentials live in one file
- app code falls back to `process.env`
- tools reconstruct the system from strings

Sigil prefers one explicit model:
- `src/topology.lib.sigil` declares boundary handles and environment names
- `src/policies.lib.sigil` declares boundary rules and trusted transforms for labelled data
- `config/<env>.lib.sigil` exports the selected environment's `world`
- application code uses typed handles from `•topology`
- only config modules may read `process.env`

## Canonical Project Shape

Topology-aware projects define:

```text
src/topology.lib.sigil
config/local.lib.sigil
config/test.lib.sigil
config/production.lib.sigil
```

Environment names are flexible, but the file path is canonical:
- if Sigil is run with `--env test`, the project needs `config/test.lib.sigil`
- if Sigil is run with `--env production`, the project needs `config/production.lib.sigil`

## Canonical Topology Module

`src/topology.lib.sigil` declares only boundary handles and environment names:

```sigil module projects/topology-http/src/topology.lib.sigil
c auditLog=(§topology.logSink("auditLog"):§topology.LogSink)

c exportsDir=(§topology.fsRoot("exportsDir"):§topology.FsRoot)

c local=(§topology.environment("local"):§topology.Environment)

c mailerApi=(§topology.httpService("mailerApi"):§topology.HttpServiceDependency)

c mailerCli=(§topology.processHandle("mailerCli"):§topology.ProcessHandle)

c prod=(§topology.environment("prod"):§topology.Environment)

c test=(§topology.environment("test"):§topology.Environment)
```

No URLs.
No ports.
No usernames.
No passwords.
No env-var names.

Those belong in config.

## Canonical Config Modules

Each declared environment gets one config module exporting `world`:

```sigil module projects/topology-http/config/test.lib.sigil
c world=(†runtime.withProcessHandles([†process.fixtureHandle(•topology.mailerCli,[†process.rule(["mailer"],None(),{code:0,stderr:"",stdout:"ok"})])],†runtime.withLogSinks([†log.captureSink(•topology.auditLog)],†runtime.withFsRoots([†fs.sandboxRoot(".local/topology-http",•topology.exportsDir)],†runtime.world(†clock.systemClock(),†fs.real(),[†http.proxy("http://127.0.0.1:45110",•topology.mailerApi)],†log.capture(),†process.deny(),†random.seeded(1337),[],†timer.virtual())))):†runtime.World)
```

Production-style config can read env vars, but only there:

```sigil module projects/topology-http/config/prod.lib.sigil
e process

c world=(†runtime.world(†clock.systemClock(),†fs.real(),[†http.proxy((process.env.mailerApiUrl:String),•topology.mailerApi)],†log.stdout(),†process.real(),†random.real(),[],†timer.real()):†runtime.World)
```

## Application Code Uses Handles, Not Endpoints

Canonical HTTP usage:

```sigil program projects/topology-http/src/getClient.sigil
λmain()=>!Http String match §httpClient.get(•topology.mailerApi,§httpClient.emptyHeaders(),"/health"){
  Ok(response)=>response.body|
  Err(error)=>error.message
}
```

Canonical TCP usage:

```sigil program projects/topology-tcp/src/pingClient.sigil
λmain()=>!Tcp String match §tcpClient.send(•topology.eventStream,"ping"){
  Ok(response)=>response.message|
  Err(error)=>error.message
}
```

Forbidden patterns:

```text
§httpClient.get("http://127.0.0.1:45110",headers,"/health")
§tcpClient.send("127.0.0.1","ping",45120)
process.env.mailerApiUrl
§file.writeText("raw","/tmp/out.txt")
§process.run(§process.command(["mailer"]))
```

For labelled boundary handling, projects use the handle-based `§file.*At`,
`§log.write`, and `§process.runAt` / `§process.startAt` surfaces so policy
rules can target exact `•topology...` boundaries.

Example:

```sigil program projects/labelled-boundaries/src/main.sigil
λmain()=>!Fs!Log!Process Unit={
  l cpf=("12345678901":µCpf);
  l ssn=("123456789":µSsn);
  l token=("gov-br-token":µGovBrToken);
  l _=(§file.writeTextAt(cpf,"cpf.txt",•topology.exportsDir):Unit);
  l _=(§log.write(•policies.redactSsn(ssn),•topology.auditLog):Unit);
  l _=(§process.runAt(•policies.govBrCommand(token),•topology.govBrCli):§process.ProcessResult);
  ()
}
```

## `--env` Is Required

Sigil does not guess a default environment for topology-aware work.

Use:

```bash
sigil validate projects/topology-http --env test
sigil run projects/topology-http/src/getClient.sigil --env test
sigil test projects/topology-http/tests --env test
```

If topology is present and `--env` is missing, Sigil rejects the command.

## What Sigil Enforces

Compile-time:
- topology constructors only in `src/topology.lib.sigil`
- world named-boundary entry constructors only in `config/*.lib.sigil` and test-local `world { ... }`
- topology-aware HTTP/TCP APIs require dependency handles
- label-aware filesystem, log, and process crossings use named `FsRoot`, `LogSink`, and `ProcessHandle` handles
- raw endpoint usage is rejected
- `process.env` is only allowed in `config/*.lib.sigil`

Validate-time:
- the selected environment must be declared in topology
- `config/<env>.lib.sigil` must exist
- the config module must export `world`
- `world` must include every primitive effect entry
- every declared named boundary must appear in the matching `world` entry collection
- no undeclared boundary handles are allowed in `world`

## Tests Are Environments

Tests are just another environment:
- same logical dependency identity
- different baseline world
- optional per-test `world { ... }` derivation

That keeps one runtime model for:
- app code
- local development
- integration tests
- production
