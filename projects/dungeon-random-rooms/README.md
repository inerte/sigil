# Dungeon Random Rooms (Sigil Project)

Pure-Sigil ASCII dungeon generator using `§random`.

Current stage:
- random room generation with canonical `!Random` effects
- fixed corridor layout between generated rooms
- recursive rendering (`tileAt(x,y)` model)

Run from repo root:

```bash
cargo run -q -p sigil-cli --no-default-features -- run projects/dungeon-random-rooms/src/main.sigil
```
