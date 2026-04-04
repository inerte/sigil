# Roguelike (Sigil Project)

Tiny seeded ASCII terminal roguelike demo with branching dungeon generation, enemy roles, ranged combat, consumables, treasure scoring, and raw-key turn-based play.

Glyphs:
- `@` player
- `g` goblin
- `a` archer
- `B` brute
- `s` shaman
- `!` potion
- `*` bomb
- `?` blink
- `/` spear
- `$` treasure
- `>` exit

The player starts with a sword, bow, arrows, and one potion. Movement is on `w/a/s/d`, ranged attacks use `f` plus a direction, bombs use `b` plus a direction, blink uses `t` plus a direction, `p` drinks a potion, `.` waits, and `q` quits.

Run from repo root:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run projects/roguelike/src/main.sigil
```
