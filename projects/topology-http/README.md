# Topology HTTP (Sigil Project)

Small HTTP topology examples for Sigil clients and servers.

Commands (from repo root):

```bash
cargo run -q -p sigil-cli --no-default-features -- run projects/topology-http/src/main.sigil
cargo run -q -p sigil-cli --no-default-features -- run projects/topology-http/src/getClient.sigil --env local
```

`src/main.sigil` is the default project entrypoint and lists the available
HTTP example entrypoints. The runnable HTTP examples expect an explicit
`--env <name>` such as `local`, `prod`, or `test`.
