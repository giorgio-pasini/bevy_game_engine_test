# BevyTest

An experiment in **AI-assisted game development with Bevy**. The games are
probes; the research subject is the process. Findings are documented, not just
applied.

## Read before working here

1. **[bevytest_documentation/ai_gamedev_guide_for_bevy.md](bevytest_documentation/ai_gamedev_guide_for_bevy.md)**
   — practices that work. Read this first.
2. **[bevytest_documentation/ai_gamedev_antipatterns_for_bevy.md](bevytest_documentation/ai_gamedev_antipatterns_for_bevy.md)**
   — mistakes already made here, with what they cost. Read before repeating one.
3. [bevytest_documentation/journal.md](bevytest_documentation/journal.md) —
   history and the decision record. Consult before re-litigating a settled
   decision.

These exist because an assistant does not remember previous sessions. They are
the memory, and keeping them current is part of the work, not an afterthought.

## Layout

```
bevy/                          Bevy 0.19.0 reference clone — READ-ONLY, NOT in git
bevytest_documentation/        General findings (this is the research output)
games/                         Cargo workspace
  snake/                       2D game — see its CLAUDE.md
  graphical_adventure_test/    Visual novel + point-and-click — see its DESIGN.md
    vn_core/                   engine as plugins (story, places, presentation)
    demo_game/                 demo consuming it
      assets/scenes/*.ron        narrative, prose inline
      assets/places/*.ron        explorable locations
      assets/cast.ron            character id -> name + image
      assets/presentation.ron    visual tuning; edit and rebuild, no Rust
      build.rs                   validates all of the above at BUILD time
      src/app_tests.rs           headless tests of the real Bevy app
  target-shared/               shared build cache (~20 GB)
test_images/                   source art
```

## First-time setup after cloning

The `bevy/` reference checkout is **not** in git — it is ~18 GB and its own
repository. It is also not optional: the guide's central practice is verifying
APIs against it rather than recalling them. Recreate it from the repo root:

```powershell
git clone --depth 1 --branch v0.19.0 https://github.com/bevyengine/bevy.git
```

Keep the tag matched to the `bevy` version in `games/Cargo.toml`. If they drift,
you will be checking your code against the wrong API — which is worse than
having no reference at all, because it looks authoritative.

## The three rules that cost the most time when broken

**Verify APIs against `bevy/examples/`, never from memory.** 0.19 postdates most
training data; five APIs have already been wrong from recall.

**Launch the game — compiling is not validating.** Bevy resolves query conflicts
at runtime. A clean `cargo check` has already produced a binary that panicked in
under a second.

**Batch feature changes.** Any change to Bevy's feature set rebuilds the engine:
20 minutes, and it has frozen this machine.

## Building

```powershell
# Environment: Rust lives on D:, not %USERPROFILE%
$env:RUSTUP_HOME='D:\RustDev\.rustup'; $env:CARGO_HOME='D:\RustDev\.cargo'
$env:PATH="D:\RustDev\.cargo\bin;$env:PATH"

cd games
cargo wb      # build --workspace --features dev
cargo wt      # test --workspace --features dev
cargo vn      # run the visual novel
cargo snake   # run snake
```

Always pass `--features dev`: it enables `dynamic_linking` (14.79s rebuilds
instead of 21 minutes) and `bevy/dev` (names systems in ECS errors).

A missing `C:\Users\giorg\.cargo` is expected — do not "fix" it by reinstalling.

## Updating the documentation

At the end of a session, update all three documents. A finding belongs in
`bevytest_documentation/` only if it follows from Bevy, Rust or the AI workflow
rather than from one game's design, and would still hold for a different genre.
Everything else belongs in that project's own docs.
