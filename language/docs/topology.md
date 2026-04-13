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
- `config/<env>.lib.sigil` exports the selected environment's `world` plus any
  env-selected declarations such as `flags`
- application code uses typed handles from `•topology`
- application code may also read selected env declarations through
  `•config.<name>`, for example `•config.flags`
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
e process

c world=(†runtime.world(
  †clock.systemClock(),
  †fs.real(),
  [†http.proxy(
    mailerApiBaseUrl(),
    •topology.mailerApi
  )],
  †log.capture(),
  †process.real(),
  †random.seeded(1337),
  [],
  †timer.virtual()
):†runtime.World)

λmailerApiBaseUrl()=>String=mailerApiBaseUrlFromProperty(process.env.hasOwnProperty("sigilHttpTestBaseUrl"))

λmailerApiBaseUrlFromProperty(hasValue:Bool)=>String match hasValue{
  true=>(process.env.sigilHttpTestBaseUrl:String)|
  false=>"http://127.0.0.1:45110"
}
```

Production-style config can read env vars, but only there:

```sigil module projects/topology-http/config/prod.lib.sigil
e process

c world=(†runtime.world(
  †clock.systemClock(),
  †fs.real(),
  [†http.proxy(
    (process.env.mailerApiUrl:String),
    •topology.mailerApi
  )],
  †log.stdout(),
  †process.real(),
  †random.real(),
  [],
  †timer.real()
):†runtime.World)
```

Config modules may also export selected env declarations for ordinary
application code. These are reached through `•config.<name>` rather than
through `•topology`.

Canonical example:

```sigil expr
•config.flags
```

## Application Code Uses Handles, Not Endpoints

Canonical HTTP usage:

```sigil program projects/topology-http/src/getClient.sigil
λmain()=>!Http String match §httpClient.get(
  •topology.mailerApi,
  §httpClient.emptyHeaders(),
  "/health"
){
  Ok(response)=>response.body|
  Err(error)=>error.message
}
```

Canonical TCP usage:

```sigil program projects/topology-tcp/src/pingClient.sigil
λmain()=>!Tcp String match §tcpClient.send(
  •topology.eventStream,
  "ping"
){
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

```sigil module projects/labelled-boundaries/src/app.lib.sigil
λrunExample()=>!Fs!Log!Process Unit={
  l _=(§file.makeDirsAt(
    "",
    •topology.exportsDir
  ):Unit);
  l _=(§file.writeTextAt(
    ("12345678901":µCpf),
    "cpf.txt",
    •topology.exportsDir
  ):Unit);
  l _=(§log.write(
    •policies.redactSsn(("123456789":µSsn)),
    •topology.auditLog
  ):Unit);
  l _=(§process.runAt(
    •policies.govBrCommand(("gov-br-token":µGovBrToken)),
    •topology.govBrCli
  ):§process.ProcessResult);
  ()
}
```

## `--env` Is Required

Sigil does not guess a default environment for topology-aware or selected-config
work.

Use:

```bash
sigil validate projects/topology-http --env test
sigil run projects/topology-http/src/getClient.sigil --env test
sigil run projects/featureFlagStorefront/src/main.sigil --env test
sigil test projects/topology-http/tests --env test
```

If topology is present, or if code reads `•config.<name>`, Sigil rejects the
command when `--env` is missing.

## What Sigil Enforces

Compile-time:
- topology constructors only in `src/topology.lib.sigil`
- world named-boundary entry constructors only in `config/*.lib.sigil` and test-local `world { ... }`
- topology-aware HTTP/TCP APIs require dependency handles
- label-aware filesystem, log, and process crossings use named `FsRoot`, `LogSink`, and `ProcessHandle` handles
- raw endpoint usage is rejected
- `process.env` is only allowed in `config/*.lib.sigil`
- `•config.<name>` requires `--env <name>`

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

For labelled boundaries, tests should assert the exact named-boundary outcome
instead of relying on ambient global state. The canonical helpers are:

- `※check::file.existsAt(path,•topology.exportsDir)`
- `※check::log.containsAt(message,•topology.auditLog)`
- `※observe::process.commandsAt(•topology.govBrCli)`

The canonical example shape is:

```sigil program projects/labelled-boundaries/tests/boundaries.sigil
λmain()=>Unit=()

test "audit sink receives redacted ssn" =>!Fs!Log!Process world {
  c exports=(†fs.sandboxRoot(
  ".local/labelled-boundaries-tests/audit",
  •topology.exportsDir
):†fs.FsRootEntry)
} {
  l _=(•app.runExample():Unit);
  ※check::log.containsAt(
    "***-**-6789",
    •topology.auditLog
  )
}
```
