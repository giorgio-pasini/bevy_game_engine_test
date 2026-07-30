# Snake — Bevy 0.19

A small grid-based Snake game. Its real purpose is to be a testbed for an
AI-assisted Bevy workflow, so the conventions below matter more than the game.

## Running

```powershell
cargo run --features dev     # fast iteration — Bevy linked as a shared library
cargo run --release          # static, optimized
```

Always pass `--features dev` while iterating. Without it, every edit to
`main.rs` relinks the entire engine.

## Toolchain: cargo lives on D:

Rust is **not** in `%USERPROFILE%`. If `cargo` isn't found, or it starts
re-downloading the whole registry, export these first:

```powershell
$env:RUSTUP_HOME='D:\RustDev\.rustup'; $env:CARGO_HOME='D:\RustDev\.cargo'
$env:PATH="D:\RustDev\.cargo\bin;$env:PATH"
```

A missing `C:\Users\giorg\.cargo` is expected. Do not "fix" it by reinstalling.

## Full rebuilds are heavy — don't trigger them casually

Dependencies build at `opt-level = 3`, so a *full* rebuild compiles 532 crates
optimized. On this 6-core / 32 GB laptop the first one took 21m42s and froze the
machine outright until parallelism was capped to `jobs = 4` in
`.cargo/config.toml`. Treat a full rebuild as a deliberate, announced action.

What forces one: changing the Bevy version, changing Bevy's feature set
(including adding `bevy/debug`), or editing any `[profile.*]` section. Editing
`src/main.rs` does **not** — that's a single rustc invocation plus a link, and
with `dynamic_linking` the engine isn't even relinked.

`jobs` is not part of the artifact fingerprint, so tuning it is always free.

## Verify API against the local Bevy source, not from memory

**This is the most important rule here.** Bevy's API churns hard between
releases, and 0.19 is recent enough that a model's recalled API is often a
version or two stale. A full Bevy 0.19 checkout sits at `..\bevy\`:

- `..\bevy\examples\` — the official example catalogue, at exactly our version
- `..\bevy\crates\` — engine source, for when examples don't cover it

Before writing an unfamiliar API, grep the examples for it. Every non-obvious
call in `src/main.rs` was confirmed this way. Note 0.19 moved the game examples
from `examples/games/` to `examples/showcase/`.

Prefer `cargo check` over `cargo run` when validating that code compiles — same
errors, much less work than codegen.

## 0.19 specifics that differ from older tutorials

Most Bevy material online targets 0.13–0.16 and will mislead you:

| Thing | 0.19 form |
| --- | --- |
| Font size | `FontSize::Px(26.0)` — **not** a bare `f32` |
| Window size | `WindowResolution::new(600, 600)` — **integers**, not floats |
| Observer system arg | `On<MyEvent>` — not `Trigger<MyEvent>` |
| Timer completion | `timer.tick(delta).is_finished()` — not `.finished()` |
| Round-scoped entities | `DespawnOnExit(GameState::Playing)` as a component |
| Exactly-one queries | `Single<&mut T, With<Marker>>` instead of `query.single()` |
| Child entities | the `children![(...), (...)]` macro |
| Nested text spans | `TextSpan` children + `TextUiWriter` |
| UI percentages | `percent(100)` helper, not `Val::Percent(100.0)` |
| RNG | `rand` 0.10: trait is `RngExt`, constructor is `rand::make_rng()` |

## Code conventions

- `Pos { x: i32, y: i32 }` is the source of truth for board state. Only
  `sync_transforms` derives `Transform` from it — no system writes both.
- Game logic reads and writes grid coordinates, never pixels. `world_pos()` is
  the single conversion point.
- Input is buffered into `Snake::next_dir` and applied on tick, so a keypress
  between ticks is never dropped and the snake can't reverse into itself.
- Round lifetime is handled by `DespawnOnExit(GameState::Playing)` rather than
  manual teardown. Entities that must survive a game over (camera, arena
  backdrop, score readout) are spawned in `Startup` instead.
- Prefer bailing out over `panic!`/`unwrap` on ECS desync — see the
  `old.len() != snake.body.len()` guard in `snake_tick`.

## Tuning

Constants at the top of `src/main.rs`: `GRID_W`, `GRID_H`, `CELL`, `TICK`
(seconds per step — lower is harder), and the colour palette.
