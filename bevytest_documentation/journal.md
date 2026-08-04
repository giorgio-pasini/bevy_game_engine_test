# Journal

Dated record of what happened. Newest first. Append-only — corrections are made
by adding a later entry, never by editing an earlier one.

The [guide](ai_gamedev_guide_for_bevy.md) and
[antipatterns](ai_gamedev_antipatterns_for_bevy.md) are current-state documents
and get rewritten freely; this file is what preserves the history behind them.

## Decision index

| Date | Decision | Entry |
| --- | --- | --- |
| 2026-08-04 | **Scope change**: pure VN → VN + point-and-click; control stack; every scene must set its own background | [2026-08-04](#2026-08-04) |
| 2026-08-02 | Enable image codecs generously; cast as the speaker/sprite link; flags and conditions | [2026-08-02](#2026-08-02) |
| 2026-08-01 | Scenes as RON data validated by `build.rs` — **supersedes** the 2026-07-30 authoring decision | [2026-08-01](#2026-08-01) |
| 2026-07-31 | Keep Bevy's `3d` profile despite building only 2D games | [2026-07-31](#2026-07-31) |
| 2026-07-31 | Uniform member dependency lists over cargo aliases | [2026-07-31](#2026-07-31) |
| 2026-07-30 | Scene structure in Rust, prose in external localised data | [2026-07-30](#2026-07-30) |
| 2026-07-30 | Snapshot saves, not `bevy_world_serialization` | [2026-07-30](#2026-07-30) |
| 2026-07-30 | Cargo workspace over independent projects sharing a target dir | [2026-07-30](#2026-07-30) |
| 2026-07-30 | Rust toolchain and build cache on D:, not the system drive | [2026-07-30](#2026-07-30) |

---

## 2026-08-04

### What happened

Added an exploration phase: places with exits and actions, a control stack so
story and exploration call each other freely, and inventory conditions. Stage 1
of five.

### Decision: the project is no longer a pure visual novel

`DESIGN.md` was written as "pure VN, designed for later extension", with
inventory explicitly deferred. It is now a visual novel **and** point-and-click
adventure engine: freely mixed control, room option lists, map hotspots
(colour-keyed, later), and inventory. The over-engineering boundary moved with
it — several entries listed as "do not do yet" are now in scope on purpose.

Delivery is staged, each stage playable, precisely because the scope grew.

### Decision: every scene must set its own background

Content that relies on state established elsewhere renders correctly down the
path you tested and blank down the others. **20 of 24 scenes** were inheriting
whatever happened to be on screen; a phase change clears the stage, so calling
one of them from a place produced an empty screen. `build.rs` now rejects a
scene that speaks before setting a background, naming it.

Music went the other way: it is now **persistent state, not derived
presentation**. No replay of the current stack can know which earlier scene
started the track, and silence-by-default is wrong for a soundtrack.

### What we learned — five bugs, one shape

Every engine bug this session was something cached diverging from reality: a
`Local` holding "what is displayed", a `_ => {}` arm holding "these are all the
variants", an assumption that setting the current state is a no-op, a replay
assuming the top frame was the whole story, and content assuming a background
someone else had set.

The architecture's founding rule — *presentation is a pure function of state* —
was the fix for all five. It had been applied to the screen but not to the
engine's own bookkeeping.

**And every one was found by the user playing, not by any check.** A clean
build, seven passing tests and exit code 0 accompanied a game that flickered,
dead-ended, fell silent and rendered blank. Process exit codes are not
validation any more than type-checking is.

### What the automated checks *did* catch

The exhaustive branch walk, added in response to "must I replay every branch by
hand?", found a genuine exploit on its first run: an action returned the player
to the room offering it, so `trust` could be farmed indefinitely. Infinite state
space — which is why the walk failed to terminate. The non-termination *was* the
bug report.

`build.rs` caught the 20 backgroundless scenes, an unknown place target, and a
place with no unconditional exit.

### Numbers

| | |
| --- | --- |
| Scenes / places / endings | 21 / 3 / 4 |
| Build-time guards | ~15 distinct checks; the new ones verified by deliberate breakage |
| Tests | 14 — 7 content, 5 headless app, 2 documentation-alignment |
| Incremental rebuild | ~25–40s |

### Changed in the other documents

- Guide: *derive from state, do not cache what you can ask for*; *walk the whole
  content graph in a test*.
- Antipatterns: *caching what the world already knows*, with the five-bug table.
- `DESIGN.md`: rescoped; Part 6's seams are now implemented architecture.

### Later the same day: the third layer was built

Five **headless app tests** now run the real Bevy `App` — `MinimalPlugins` plus
`AssetPlugin`, `StatesPlugin`, `InputPlugin` and hand-registered `Image` and
`AudioSource` — in 0.07s, with no window and no GPU.

They cover the two engine bugs that had already shipped and been found by hand:
returning from a called scene must redraw the place, and music must survive a
phase change. Verified by breaking the view and confirming both fail with *"the
ridge offers four options but none were drawn"*, then reverting.

The harness failed on its first run with
`Res<ButtonInput<KeyCode>> failed validation: Resource does not exist` —
`MinimalPlugins` carries no input. Knowing which plugin supplies which resource
is the whole difficulty of headless Bevy testing, and `bevy/dev` naming the
system and parameter is what made it a one-line fix.

**Coverage now stands at four layers:** build-time guards, a content-graph walk,
headless app tests, and a human playing. Every engine bug this session escaped
the first two; the third now closes that gap.

### And then: the documentation itself was found drifting

Two audits, prompted by the user asking whether the docs matched the project,
found **seven** misalignments between them — a roadmap listing finished
milestones as pending, a data-model sketch describing an API that no longer
existed, paths to moved files, wrong counts, and the journal declaring a built
feature outstanding.

The last of those is the worst kind: a future session would have rebuilt
something that already existed.

Two rules went into `README.md` — write documentation *after* the work lands,
and treat code as authoritative once it exists — and two tests now enforce the
mechanical half:

- every path mentioned in a document must resolve on disk;
- every `Enum::Variant` a document names must exist in `vn_core`.

Both verified by introducing drift deliberately. They cannot catch a document
that is *wrong about why* — only one that names something gone — but that was
most of what the audits found.

Worth noting the shape: this is the same failure as every engine bug this
session. A record of the world diverging from the world. The fix was the same
too — stop trusting the record, ask the source.

### Stage 3 — inventory, in play

A brass gear is found on the ledge (`GiveItem`), gates a new option on the ridge
(`HasItem`), and is consumed when given to the golem (`TakeItem`).

Cheap because of a decision taken when the exploration phase was scoped: items
are **richer flags**, not a use-item-on-target matrix. Stage 3 was therefore
content plus one build guard, not an engine feature.

Worth noting there is no "already given" flag on that option. `TakeItem` removes
the item, the condition stops holding, the option disappears. **The inventory is
the state** — the same derive-don't-cache rule, applied to content instead of code.

New guard, verified by breaking it: an item required by a condition must be
granted somewhere, or the content is silently unreachable.

```
ridge.ron (place `ridge`) option 3: condition requires item `brass_gearr`,
which no scene ever grants. Items granted somewhere: ["brass_gear"]
```

### Where to pick up

**Next:** stage 4 (map hotspots, colour-keyed lookup images — needs the
placeholder generator described in the plan) and stage 5 (save/load across the
control stack, the real test of the architecture). Also greying out options
already taken — `DESIGN.md` Part 8b, which notes it belongs in the profile
rather than the save.

**State:** all tests green — 7 content (including the exhaustive branch walk),
5 headless app, 2 documentation alignment. Nothing in flight. Build ~25–40s;
scene edits ~9s.

*(Counts are stated by layer rather than as a total on purpose: totals drift
every time a test is added, and nothing checks them. Run `cargo wt` for the
number.)*

---

## 2026-08-02

### What happened

Grew the visual novel into a two-character piece: 16 scenes, four endings, a
second character on stage, a speaker cue, and a working flag/condition system.

### What we learned

**An existence check is not a loadability check.** A `mountains.webp` background
was added while Bevy's `webp` feature was off. The build script — written the day
before and described as thorough — passed it, because the file was undeniably
present. It would have failed at runtime with a decode error. Validators now
check the extension against the enabled formats.

The wider lesson: *when you write a validator, ask what it does not check.* This
one was believed complete for a day.

**A wildcard `match` arm silently discards exhaustiveness.** Adding a `JumpIf`
step broke two independent graph walkers — the reachability check in `build.rs`
and its equivalent test — because both ended in `_ => {}`. Three scenes reachable
only via `JumpIf` were reported unreachable, so the tools blamed the content
while the tools were wrong.

Notable that the second copy surfaced only after the first was fixed. Both files
exist to catch mistakes, and both had opted out of the guarantee that makes
enum-modelled content safe in the first place.

**The architecture absorbed a real feature cleanly.** Flags, conditions, a cast,
aspect-aware sizing and an animated speaker cue were added without touching the
story/presentation split. `speaker_cue` is the only system that writes character
tint or scale, matching the one-writer rule.

### Decisions

**Enable image codecs generously** — `webp`, `bmp`, `tga`, `qoi`, `gif` in one
change. There is no image equivalent of `audio-all-formats`, and this was the
third time an unsupported format interrupted work.

**A cast file as the sprite/speaker link.** `Show(character: "mira")` and
`Say(speaker: "mira")` share an id resolved through `cast.ron`; previously `Show`
took a path and `Say` a display name, so nothing could tell which sprite was
talking. Ids stay strings validated by `build.rs` rather than becoming a second
generic parameter — one type parameter is enough complexity.

**Flags and conditions**, deliberately small: `Set`/`Clear`/`Add`, `JumpIf`, and
optional per-choice conditions. No nested blocks, no expression language.
`build.rs` cross-checks flags written against flags read, which catches the
mistyped-name failure a string-keyed system invites, and requires at least one
unconditional option per branch so a choice can never dead-end.

### Numbers

| | |
| --- | --- |
| Engine rebuild for the new codecs | ~20 min |
| Incremental rebuild afterwards | 39s |
| Scenes / endings | 16 / 4 |
| Build-time guards, each verified by deliberate breakage | 4 |
| Tests | 5, passing |

### Changed in the other documents

- Guide: *validate that assets load, not merely exist*; *spell out enum variants
  in code that must handle all of them*.
- Antipatterns: the two entries above, both with the evidence.

---

## 2026-08-01

### What happened

Moved the visual novel's scenes out of Rust and into `.ron` files — one per
scene, prose written inline — and added a `build.rs` that validates them and
generates the `SceneId` enum from the filenames.

Also set up the repository properly: `.gitignore`, and this documentation set.

### The problem being solved

Authoring scenes in Rust was genuinely unpleasant. `Step` literals are verbose,
but the real cost was the `TextKey` indirection: adding one line of dialogue
meant editing two places, the step and the text table. Diagnosing that
correctly mattered — a format change alone would not have fixed it, because
keeping keys in YAML would have kept the two-file problem.

### Decision: RON scenes validated at build time — supersedes 2026-07-30

The earlier decision put scene structure in Rust *specifically* so that
AI-invented scene names became compile errors. That reasoning was never wrong,
so this does not discard it — it keeps compile-time validation and moves only
the authoring surface.

`build.rs` runs before the crate compiles, parses every scene file, and fails
the build on a bad jump target, a missing asset, or an unreachable scene. It
then generates a `SceneId` enum from the filenames, so Rust code referencing
scenes stays compile-checked exactly as before.

**RON over YAML, on evidence.** `serde_yaml` publishes as
`0.9.34+deprecated` (dtolnay, March 2024) and `serde_yml` is *also* deprecated,
now forwarding to `noyalib`. YAML would mean adopting a dependency mid-churn.
RON was already in the lock, is Bevy-native, and its grammar supports `r#"..."#`
raw strings spanning lines — which was YAML's only real advantage for prose.

### What we learned

**"Compile-time" and "test-time" are different guarantees**, and the distinction
is worth defending. The original proposal was a validation *test*; tests run at
`cargo test`, which is earlier runtime, not compile time. A build script is what
actually makes the build fail.

**Both guards were verified by breaking them.** A typo'd jump target produced:

```
offer.ron (step 3): jumps to `acccept`, but there is no `acccept.ron`.
Known scenes: ["accept", "awakening", "offer", "refuse"]
```

and a bad image path failed the same way. Neither was assumed to work.

**The authoring loop is now 9.3 seconds** for a prose edit — the build script
re-validates and the crate recompiles. Fast enough that hot-reload is a
convenience rather than a necessity, which is why scene text is embedded at
build time instead of loaded from disk.

### Numbers

| | |
| --- | --- |
| Rebuild after a one-word prose edit | **9.3s** |
| Rebuild after reverting a scene file | 20.6s |
| Full build with the new pipeline | 2m41s |
| Tests | 4, passing; both build-time guards verified by deliberate breakage |

### Changed in the other documents

- Guide: added *validate generated content in a build script*.
- Antipatterns: added *confusing compile-time with test-time validation* and the
  YAML-ecosystem finding.
- `DESIGN.md`: authoring section rewritten; it described the superseded approach.

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
