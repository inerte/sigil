# Topology

<h2 id="topology-is-runtime-truth">Topology Is Runtime Truth</h2>

Sigil topology is the canonical, compiler-visible declaration of a project's
external runtime dependencies.

Topology is not config.

Topology answers:
- what external things this project depends on
- what those logical dependencies are called
- which environment names exist

Config answers:
- how one named environment binds those dependencies

## Why Sigil Splits Topology from Config

Without this split, runtime truth gets blurred together:
- architecture and credentials live in one file
- app code falls back to `process.env`
- tools reconstruct the system from strings

Sigil prefers one explicit model:
- `src/topology.lib.sigil` declares dependency handles and environment names
- `config/<env>.lib.sigil` binds every declared dependency for the selected environment
- application code uses typed handles from `src::topology`
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

`src/topology.lib.sigil` declares only dependency handles and environment names:

```sigil module projects/topology-http/src/topology.lib.sigil
i stdlib::topology

c local=(stdlib::topology.environment("local"):stdlib::topology.Environment)

c mailerApi=(stdlib::topology.httpService("mailerApi"):stdlib::topology.HttpServiceDependency)

c prod=(stdlib::topology.environment("prod"):stdlib::topology.Environment)

c test=(stdlib::topology.environment("test"):stdlib::topology.Environment)
```

No URLs.
No ports.
No usernames.
No passwords.
No env-var names.

Those belong in config.

## Canonical Config Modules

Each declared environment gets one config module:

```sigil module projects/topology-http/config/test.lib.sigil
i src::topology

i stdlib::config

c bindings=(stdlib::config.bindings([stdlib::config.bindHttp("http://127.0.0.1:45110",src::topology.mailerApi)],[]):stdlib::config.Bindings)
```

Production-style config can read env vars, but only there:

```sigil module projects/topology-http/config/prod.lib.sigil
e process

i src::topology

i stdlib::config

c bindings=(stdlib::config.bindings([stdlib::config.bindHttpEnv(src::topology.mailerApi,"MAILER_API_URL")],[]):stdlib::config.Bindings)
```

## Application Code Uses Handles, Not Endpoints

Canonical HTTP usage:

```sigil program projects/topology-http/src/getClient.sigil
i src::topology

i stdlib::httpClient

λmain()=>!Http String match stdlib::httpClient.get(src::topology.mailerApi,stdlib::httpClient.emptyHeaders(),"/health"){
  Ok(response)=>response.body|
  Err(error)=>error.message
}
```

Canonical TCP usage:

```sigil program projects/topology-tcp/src/pingClient.sigil
i src::topology

i stdlib::tcpClient

λmain()=>!Tcp String match stdlib::tcpClient.send(src::topology.eventStream,"ping"){
  Ok(response)=>response.message|
  Err(error)=>error.message
}
```

Forbidden patterns:

```text
stdlib::httpClient.get("http://127.0.0.1:45110",headers,"/health")
stdlib::tcpClient.send("127.0.0.1","ping",45120)
process.env.MAILER_API_URL
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
- config binding constructors only in `config/*.lib.sigil`
- topology-aware HTTP/TCP APIs require dependency handles
- raw endpoint usage is rejected
- `process.env` is only allowed in `config/*.lib.sigil`

Validate-time:
- the selected environment must be declared in topology
- `config/<env>.lib.sigil` must exist
- the config module must export `bindings`
- every declared dependency must be bound exactly once
- no extra bindings are allowed
- binding kinds must match dependency kinds

## Tests Are Environments

Tests are just another environment:
- same logical dependency identity
- different concrete bindings

That keeps one runtime model for:
- app code
- local development
- integration tests
- production
