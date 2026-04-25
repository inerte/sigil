# Topology TCP (Sigil Project)

Small TCP topology examples for Sigil clients and servers.

Commands (from repo root):

```bash
cargo run -q -p sigil-cli --no-default-features -- run projects/topology-tcp/src/main.sigil
cargo run -q -p sigil-cli --no-default-features -- run projects/topology-tcp/src/echoClient.sigil --env local
```

`src/main.sigil` is the default project entrypoint and lists the available
TCP example entrypoints. The runnable TCP examples expect an explicit
`--env <name>` such as `local`, `prod`, or `test`.
