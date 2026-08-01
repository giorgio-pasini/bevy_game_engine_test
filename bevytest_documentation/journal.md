# Journal

Dated record of what happened. Newest first. Append-only — corrections are made
by adding a later entry, never by editing an earlier one.

The [guide](ai_gamedev_guide_for_bevy.md) and
[antipatterns](ai_gamedev_antipatterns_for_bevy.md) are current-state documents
and get rewritten freely; this file is what preserves the history behind them.

## Decision index

| Date | Decision | Entry |
| --- | --- | --- |
| 2026-07-31 | Keep Bevy's `3d` profile despite building only 2D games | [2026-07-31](#2026-07-31) |
| 2026-07-31 | Uniform member dependency lists over cargo aliases | [2026-07-31](#2026-07-31) |
| 2026-07-30 | Scene structure in Rust, prose in external localised data | [2026-07-30](#2026-07-30) |
| 2026-07-30 | Snapshot saves, not `bevy_world_serialization` | [2026-07-30](#2026-07-30) |
| 2026-07-30 | Cargo workspace over independent projects sharing a target dir | [2026-07-30](#2026-07-30) |
| 2026-07-30 | Rust toolchain and build cache on D:, not the system drive | [2026-07-30](#2026-07-30) |

---

## 2026-07-31

### What happened

Built the visual-novel vertical slice: `vn_core` (engine as Bevy plugins) plus
`demo_game`, running with letterboxed 16:9 presentation, background, transparent
character sprite, typewriter dialogue, a branching choice with two endings, and
music. Then added a main menu and end-of-story return, closing the loop.

Added five validation tests and proved they fail on a deliberately broken asset
path. Created this documentation set.

### What we learned

**Crate count is not build time.** A feature change rebuilt 30 crates and took
20m10s — versus 21m42s for the original 532-crate cold build. The crates a
feature change touches are the expensive ones.

**The `3d`-drop saving was overstated.** Dropping Bevy's `3d` profile was
reported as 532 → 75 crates. Inspecting the build log later showed `bevy_pbr`
and `bevy_light` were built anyway, dragged in by `bevy_dev_tools` via
`bevy/dev`. Only `bevy_gltf`, `bevy_anti_alias` and `bevy_mikktspace` actually
left. Reported as a saving before it was verified — the mistake was checking the
feature list rather than the build log.

**Architecture paid off measurably.** Returning the game to its menu required
one line — clearing the derived stage — because presentation is a pure function
of story state. The same systems that built the screen tore it down. No teardown
path was written.

**Two more 0.19 API surprises**, both caught by checking rather than recall:
`MessageWriter<AppExit>` with `.write()` (not `EventWriter`/`.send()`), and
`bevy::camera::Viewport` (`bevy::render::camera::Viewport` is private). The
latter was the only compile error in the whole engine.

**The invocation trap bit twice more.** `cargo run -p demo_game` after
`cargo build --workspace` started a full engine rebuild; then, one message after
documenting the rule, `cargo test -p demo_game` did it again. Investigating
produced the actual mechanism: the two selections' dependency trees differ by 51
lines around `rand_core`/`getrandom`/`arrayref`/`cpufeatures`, cascading into the
engine via `uuid`.

### Decisions

**Keep the `3d` profile.** A 3D experiment is planned. Since feature changes cost
a full rebuild, carrying 3D now costs one build while dropping and re-adding it
would cost two. Rebuild: 20m10s.

**Fix the invocation trap structurally, not with documentation.** `vn_core` now
declares `rand`/`chacha20` so every member's dependency list is uniform and all
invocations resolve identically. Cargo aliases (`cargo vn`, `cargo wb`) were
added too, but as convenience — an alias would not have prevented any of the
three failures, because each was a valid command, just the wrong one.

### Numbers

| | |
| --- | --- |
| Rebuild with `3d` restored | 20m10s, 30 crates |
| Incremental rebuild, both crates | 46s |
| Incremental, one file (snake, earlier) | 14.79s |
| Shared build cache | 20.5 GB |
| Validation tests | 5, all passing, failure verified |

### Changed in the other documents

- Guide: added *batch feature changes* (with the crate-count caveat), *uniform
  dependency lists*, *validation tests*, *presentation as a pure function of
  state*, *enums + exhaustive match*, *documentation must be in the load path*,
  *prefer structural guards over written rules*.
- Antipatterns: added *optimising a metric instead of the goal*, *mixing cargo
  invocations*, *serialising the world for saves*, *running the binary without
  the DLL path*.

---

## 2026-07-30

### What happened

Wrote `DESIGN.md` for a visual-novel template — component inventory,
architecture, patterns, and an explicit over-engineering boundary — to be used by
a future AI session as the implementation brief. Converted `games/` into a cargo
workspace. Produced a 49-slide Rust course taught entirely through the Snake
source.

### What we learned

**A shared target directory gives only partial reuse.** A second independent
project reused ~489 leaf third-party crates but still rebuilt 31 `bevy_*`
crates — the expensive ones. Versions were identical; feature resolution had
diverged because one project depended on `rand` directly, changing features on
`glam`, which all of Bevy sits on.

**`bevy_ui_widgets` is experimental** and says so in its own example. Recorded as
an antipattern for anything meant to last.

**Build cache is enormous relative to the game.** 8.5 GB of cache for 12 KB of
source: 4.0 GB `.rlib`, 2.6 GB `.pdb`, 1.1 GB `.rmeta`. The shipped artefact was
0.7 MB plus a 126 MB engine DLL.

### Decisions

**Scene structure in Rust, prose in external localised data.** The initial
recommendation was fully data-driven (Ren'Py-style RON). That was challenged and
changed: Ren'Py's design optimises for non-programmer writers, which does not
apply here. With AI-generated content the dominant failure is a plausible
reference to something that does not exist, and Rust turns that into a build
error. Prose was still split out because localisation genuinely needs runtime
string swapping.

*This decision came from the user pushing back on the assistant's
recommendation, and the pushback was correct.*

**Snapshot saves, not `bevy_world_serialization`.** Coupling the save format to
ECS layout means refactors break players' saves.

**Cargo workspace over independent projects.** One `Cargo.lock` guarantees
matching versions. Caveat recorded: it does *not* guarantee matching features.

### Changed in the other documents

Seeded most of the guide's build/tooling section and the antipatterns on
experimental APIs and world serialisation.

---

## 2026-07-29

### What happened

First session. Installed the toolchain from nothing and ran an official Bevy
example (`alien_cake_addict`), then built Snake as the first original game.

### What we learned

**The environment was mostly ready and unverified assumptions were the risk.**
MSVC, Windows SDK, GPU and long-path support were all already present; only Rust
was missing. Checking first avoided a wasted install.

**The first Bevy build is brutal.** 21m42s, 532 crates, 17.5 GB target directory
— and it froze the machine until parallelism was capped.

**`cargo check` is not validation.** Snake type-checked with zero warnings and
panicked immediately with `error[B0001]`, a runtime query conflict. This became
the single most-repeated lesson in the project.

**A "broken" game was working correctly.** The 3D example appeared to render
nothing but text. It was the game-over screen: every 3D entity carried
`DespawnOnExit(GameState::Playing)`, and the untouched game had ended ~15 seconds
in. Diagnosed by reading the source rather than assuming a rendering fault.

### Decisions

**Rust toolchain and cargo home on D:.** The system drive had 15.9 GB free; the
build cache alone reached 17.5 GB.

**`dynamic_linking` behind an opt-in `dev` feature.** Measured 21m42s cold
versus 14.79s after a one-file edit.

### Changed in the other documents

Established the core practices: verify APIs against the local checkout, launch
the game rather than trusting a clean check, cap build parallelism.
