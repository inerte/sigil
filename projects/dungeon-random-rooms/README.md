# Dungeon Random Rooms (Sigil Project)

Pure-Sigil ASCII dungeon generator (deterministic first pass).

Current stage:
- fixed rooms and corridors
- recursive rendering (`tileAt(x,y)` model)
- no randomness yet

Run from repo root:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run projects/dungeon-random-rooms/src/main.sigil
```
