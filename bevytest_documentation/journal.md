# Journal

Dated record of what happened. Newest first. Append-only — corrections are made
by adding a later entry, never by editing an earlier one.

The [guide](ai_gamedev_guide_for_bevy.md) and
[antipatterns](ai_gamedev_antipatterns_for_bevy.md) are current-state documents
and get rewritten freely; this file is what preserves the history behind them.

## Decision index

| Date | Decision | Entry |
| --- | --- | --- |
| 2026-08-14 | Two consumers define a curated prelude; workspace metadata defines the shared development feature contract | [2026-08-14](#2026-08-14) |
| 2026-08-14 | A tiny second consumer proves the reusable boundary before public-API cleanup | [2026-08-14](#2026-08-14) |
| 2026-08-13 | Presentation asks spawned entities what exists; the retained scene cache is private behind one invariant | [2026-08-13](#2026-08-13) |
| 2026-08-06 | `GameScript` trait replaces three function pointers — argued on readability, not on churn | [2026-08-06](#2026-08-06) |
| 2026-08-05 | The frame has one stated order — five system sets, not three unordered chains | [2026-08-05](#2026-08-05) |
| 2026-08-05 | Saves live in a per-user OS directory, written atomically; the library takes it as a resource | [2026-08-05](#2026-08-05) |
| 2026-08-05 | Saves store what cannot be derived — including the playing track, excluding cached fields | [2026-08-05](#2026-08-05) |
| 2026-08-05 | Hotspots are options with a colour, audited against the actual pixels at build time | [2026-08-05](#2026-08-05) |
| 2026-08-05 | The cast is re-established per scene, like the background — **not** persistent like music | [2026-08-05](#2026-08-05) |
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

## 2026-08-14

### The engine has a second consumer

Added `games/graphical_adventure_test/reuse_probe`: three Rust-authored scenes,
one choice, two endings, no build script, no exploration and no new engine
feature. It owns one known-decodable asset of each presentation kind — WebP
background, PNG character and Ogg music — so it asks whether a normal Bevy
binary can supply `GameScript`, menu text, cast, empty places and presentation
tuning, then run the existing plugin without depending on demo content.

Four tests cover the consumer rather than the library internals: scene names
round-trip for saves; all owned assets exist; both choices reach distinct
endings and the river route applies `Hide`; and the real headless plugin enters
this game's opening scene with a spawned background, character and music
player. The workspace now passes 49 tests: 29 demo, 16 `vn_core`, and 4
reuse-probe tests. The asset-complete binary was also built and stayed alive
during a bounded native launch with no asset decode or runtime schedule errors;
visual composition, audible music and clicked ending routes still need human
observation.

### The probe paid on its first compile

Both Bevy and `vn_core` export `Scene` through their preludes. Importing the two
natural preludes made `Scene` ambiguous, which the original demo hid through its
module layout. The consumer now names `vn_core::story::Scene` as `VnScene`.

That is evidence for the upcoming public-API audit, not a reason to redesign the
library in the same change. `ENGINEERING_REVIEW.md` finding B4 had deliberately
waited for a second consumer; its trigger is now satisfied. The next pass should
list what both games import, then narrow or rename only from that evidence.

### Deferred features now have one home

Created `games/graphical_adventure_test/BACKLOG.md`. It separates base
solidification from player-facing work and gives each feature dependencies and
a done condition: persistent profile/seen decisions, history, auto/skip,
settings, transitions, audio completion, localisation, gallery/achievements,
save UX/migrations and measured asset preloading. `DESIGN.md` remains the
historical reasoning; the backlog is authoritative for what is still wanted.

### The second consumer now defines the public boundary

Audited imports and app-level queries in both `demo_game` and `reuse_probe`.
`vn_core::prelude` now names its exports explicitly instead of glob-exporting
five modules, and exposes the narrative type as `VnScene`; the direct
`vn_core::story::Scene` path remains public. This removes the collision with
Bevy's prelude without renaming the model itself. Systems registered only by
`VnPlugin`, transient load/toast resources, UI-only markers and the letterbox
module became crate-private. Public model and save types, `VnSet`, map sampling
helpers, and components queried by consumer tests stayed public because the two
games supplied evidence for them.

The workspace manifest now also carries one `dev` feature contract. The demo's
existing build script enumerates all workspace members, rejects any different
set of `bevy/...` development flags, and requires crates depending on `vn_core`
to forward `vn_core/dev`. This deliberately accepts the documented layering
oddity in exchange for making a costly feature drift fail the normal workspace
build. No Bevy feature changed. Focused serial runs passed all 49 tests after an
initial parallel attempt exhausted memory while native games were also running.

---

## 2026-08-13

### Finished the two substantive hardening findings

`AppliedStage` is gone. Background and music entities now carry the path they
actually display, while character entities already carried their slot and cast
id. The three reconcilers compare `Stage` against the real entities instead of
against a second resource describing them. Four headless tests assert that the
requested background, cast and track are truly spawned, and that an unchanged
stage does not replace the background or restart the music. The fifth deletes
both entities behind the reconcilers' backs and proves the unchanged
authoritative stage recreates them on the next frame — the exact failure the
old cache could not survive.

`StoryState::current` remains because resolving it on every step clones a full
scene including prose. The unsafe part was not the cache itself but the public
stack and five independent sync points. Stack and cache are now private;
construction, restore and mutation use methods that maintain the invariant, and
`from_frames` is the only place that derives the cached scene. The restore test
uses two different story frames with a place between them so an incorrect
"first story frame" implementation genuinely fails.

### Verification

- `cargo test --workspace --features dev`: 45 tests passed — 29 demo/app/content
  and documentation tests, 16 `vn_core` tests.
- `git diff --check`: clean.
- Five build-script dead-field warnings remain; they predate this slice and are
  confined to the validator's deserialisation mirror.
- The design document's stale "no code exists" status and overstated 3D feature
  saving were corrected after the code was verified.

### Next sensible probe

After runtime play validation, build a tiny second game consuming `vn_core`
before doing more internal API cleanup. One consumer can only prove that the
engine serves itself; a deliberately small second consumer will expose the real
library boundary and provide evidence for the currently deferred visibility
work.

---

## 2026-08-06

### What happened

A critical review of the codebase — asked for in three categories: antipatterns
we implemented ourselves, good patterns available and not applied, and
unnecessary engineering. Ten findings, written up in
`games/graphical_adventure_test/ENGINEERING_REVIEW.md` with each one's evidence,
correction and cost, so the decisions could be made with the reasoning in view
rather than from a summary.

Then the first of them was implemented.

### The review found two of our own antipatterns

`AppliedStage` and `StoryState::current` are both caches of things the world
already knows — the exact rule in the antipatterns document, committed twice
after writing it down. Neither is currently wrong; both rest on every future
teardown path remembering they exist, which is how the `Local` cache shipped a
dead end.

The measurable one: **194 leaked temp directories** from test runs. 193 from a
harness that never cleans up, and one from the test that *does* — which leaked
from the run where its guarantee was deliberately broken, because its cleanup is
a trailing statement and the assertion panicked first. **Cleanup that is not a
`Drop` guard runs only when the test passes**, which is the case where it matters
least.

### Decision: `GameScript`, and a correction about how it was argued

Three function pointers described the game's scenes — `Script::scene_of`, and
`SceneLookup`'s `of_name` and `name_of` — spread over two resources and two
plugin fields. The review first filed this as *unnecessary engineering to leave
alone*: a trait with one implementor is over-engineering, the churn buys nothing.

Asked directly which version a human would find easier to read and maintain, that
answer did not survive. It had answered **"is it worth the churn"** — a cost
question — while presenting itself as a quality judgement. On the question
actually asked, the function pointers lose:

- **they defeat IDE navigation** — "go to definition" on `(script.scene_of)(id)`
  lands on the field, not the function that runs, because a function pointer is
  a value and values cannot be followed;
- **they threaded a parameter through 7 systems and 8 functions**, almost
  entirely so something could eventually call one of them;
- `(script.scene_of)(*scene)` appears seven times and reads like nothing else in
  the codebase.

One trait replaced all of it. `Script` and `SceneLookup` are gone, `VnPlugin`
went from seven fields to five, and `&script` disappeared from every signature
that only carried it.

**Measured: 175 lines added, 226 removed — 51 fewer, including the new trait and
its documentation.** All 39 tests passed unchanged, which is the entire
verification a behaviour-preserving refactor is entitled to.

### The general lesson, which is about arguing rather than about Rust

Two different questions were being conflated:

| Question | Answer for this refactor |
| --- | --- |
| Is it worth the churn? | Marginal — no behaviour changes, no new tests |
| Which is easier to read and maintain? | The trait, clearly |

A cost answer dressed as a quality answer is hard to spot precisely because it
is not wrong — it is just answering something else. Asking *which is better for a
human* separately from *is it worth doing now* is what separated them.

There was also a scheduling insight worth keeping: **deferring a refactor whose
cost compounds is a decision to pay more later.** Narrowing visibility (finding
B4) costs the same whenever it is done and genuinely needs a second consumer, so
deferring it is free. This one grew with every function that threaded
`&Script<S>`, and the exploration work had added three.

### Numbers

| | |
| --- | --- |
| Findings | 10 — 3 self-inflicted antipatterns, 4 unapplied patterns, 4 unnecessary engineering, 2 examined and cleared |
| Implemented | 1 (C3) |
| Tests | 39, unchanged and all passing |
| Ambiguous system pairs | still 1, still Bevy's own |

### Then stage 2: the cheap findings, led by the valuable one

**The replay/advance agreement test** — the highest-value gap the review found.
Two interpreters of `Step` have to agree about what a scene puts on screen, and
until now only care kept them together. When they diverge the screen silently
contradicts the story, which is how scenes shipped blank and how the music once
stopped.

The property is simple: run the engine to a stopping point, replay to the cursor
it stopped at, and the two `Stage`s must be equal.

Writing it exposed a fixture gap immediately. The synthetic scenes used
`Background`, `Music` and `Show` but never `Hide` — so a replay that ignored
`Hide` would have left a character on screen with nothing to notice. Adding one
made the test able to fail, confirmed by making the replay drop `Hide`: that test
went red and nothing else did. **Third time this week that a fixture, not the
assertion, was what limited a test.**

**The temp-directory leak, measured rather than argued.** A `TempSaves` guard is
now held *as a resource*, so it dies with the `App` — on failure as well as
success, and with no cleanup line in any test. Without it a run leaves 5
directories behind; with it the count does not move. The 224 that had piled up
were deleted.

Also: one `allowed()` helper replacing four identical guard checks, verified by
making it always permit (the exhaustive branch walk caught it), and an expiry
note on `tools/mapgen` saying to delete the crate when real art lands.

### Numbers

| | |
| --- | --- |
| Findings | 10 — 3 self-inflicted antipatterns, 4 unapplied patterns, 4 unnecessary engineering, 2 examined and cleared |
| Implemented | 5 — C3, B2, B1, A3, C2 |
| Tests | 40 — 16 `vn_core` unit, 22 `demo_game` app/content, 2 documentation alignment |
| Verified by deletion | 4 more, each taking down exactly the expected test |
| Temp directories leaked per run | 5 → 0 |

### Changed in the other documents

- New: `games/graphical_adventure_test/ENGINEERING_REVIEW.md`, added to the
  documentation-alignment test's list so its citations are checked. Five of its
  ten findings now carry outcome banners.
- Guide: *cleanup belongs in `Drop`, not at the end of a test* — see below.
- Antipatterns: the fixture-strength entry gained a third instance.

---

## 2026-08-05

### What happened

Stage 4 of the exploration work: **map hotspots**. A place may name a lookup
image whose flat colours identify clickable regions, so navigation can be
"click the cairn on the map" rather than "press the third button".

Mid-stage, the user reported that characters were missing from almost every
scene. That turned out to be the more important finding of the day.

### Decision: a hotspot is an option with a colour

The alternative was a separate hotspot system with its own targets and its own
conditions. Instead, `PlaceOption` gained one optional field:

```ron
(label: "The cairn on the shoulder", effect: GoTo("cairn"), region: Some((220, 60, 60)))
```

An option with a `region` is clicked on the picture; one without is drawn as a
button; a place may hold both. Everything downstream — the condition re-check,
the control stack, the flow routing — is untouched and shared. Buttons and
hotspots now both write "the player picked option *n*" into one resource that a
single system consumes.

Sampling is in **normalised** coordinates, so a lookup image may be any
resolution and only its aspect ratio must match. A 960x540 placeholder and
finished 4K art are interchangeable with no data change.

### Decision: audit the pixels at build time

A colour-keyed map has failure modes that produce **silence**, never an error:
the click just does nothing. `build.rs` now decodes the lookup and checks that
every declared colour occurs, covers at least 0.1% of the image, that the
encoding is lossless PNG, that the aspect is 16:9, that no two options share a
colour, and — the one that matters most — that **no undeclared colour occurs at
all**.

That last check is the anti-aliasing guard. A soft-edged region is surrounded by
a halo of blended colours, each of which is a dead pixel, around the outline of
every hotspot. Verified by generating a deliberately soft lookup:

```
`maps/valley_lookup.png` contains 3 colour(s) matching no region:
(30, 70, 110) x656, (110, 30, 30) x596, (120, 100, 40) x682.
The usual cause is ANTI-ALIASING ...
```

All six new guards were verified by breaking them and reverting.

### The bug the user found: the background rule's twin

**21 of 22 scenes never staged a character.** Only the opening scene did; every
other scene inherited whoever was already on screen.

This is *exactly* the bug fixed the previous day for backgrounds, in the field
next to it in the same struct. It survived because the audit stopped at the case
that had been reported. Within a straight narrative run it is invisible — a jump
leaves the stage alone, so the cast persists — and it breaks the instant a phase
change clears the stage, which is the feature that had just been built.

`Stage` holds three things, and each needed a decision that was only ever made
for two of them:

| | Category | Rule |
| --- | --- | --- |
| `background` | re-established | every scene sets its own |
| `characters` | re-established | every scene stages its own — **new today** |
| `music` | ambient | persists across phases, restored from state |

The general form is now in the guide as *classify every piece of presentation
state: re-established or ambient*. The test for which is: **can the code that
rebuilds the screen work it out from where we are now?** A background can; a
soundtrack cannot.

Fixed in all 22 scenes, plus a guard: a `Say` whose speaker was never `Show`n in
that same scene fails the build. Nine lines, and it caught all 21 immediately.
The message says what to do about a voice that is *meant* to be off-stage —
because "deliberately absent" and "forgotten" must not look identical in the
data.

### Tooling notes worth keeping

Two dependency decisions that would normally be a full engine rebuild, and were
not:

- **`image` as a build-dependency** of the game crate, to decode lookup images.
  Cargo's resolver 2/3 resolves build-dependencies separately from normal ones,
  so the copy under Bevy is untouched. Measured: `image`, `png` and four small
  leaves compiled; no `bevy_*` crate rebuilt; incremental build stayed at 38s.
- **`tools/mapgen` is not a workspace member.** An empty `[workspace]` table in
  its manifest detaches it, so its `image` dependency cannot enter the set cargo
  unifies features over. It still shares the build cache through
  `games/.cargo/config.toml`.

The generator emits the map picture and its lookup from **one** set of shape
predicates at two resolutions, so the two cannot disagree. When real art
arrives, that guarantee moves to the build-time audit.

### Numbers

| | |
| --- | --- |
| Scenes / places / endings | 22 / 4 / 4 |
| Build-time guards | the hotspot audit alone adds **11** distinct failure conditions, plus the speaker-staging guard; **6** of them verified by deliberate breakage |
| Tests | 16 — 7 content, 7 headless app, 2 documentation alignment |
| Incremental rebuild | ~38s |
| Placeholder art | 44 KB picture + 12 KB lookup, generated |

### Changed in the other documents

- Guide: *classify presentation state*; *validate the contents of binary
  assets*; *generate paired assets from one definition*; *build-dependencies are
  feature-isolated*; *keep tools out of the workspace*; *fix the class, then
  find the siblings*; one-writer extended to input.
- Antipatterns: *fixing the instance and declaring the class fixed*.
- `DESIGN.md`: hotspots documented; stage 4 out of the backlog.

### Where to pick up

**Stage 5 — save/load across the control stack.** The design is already written
(`DESIGN.md` Part 3): a snapshot of stack, flags and items, with the screen
rebuilt by replaying **every frame from the bottom up**. `rebuild_stage` already
does exactly that replay, so most of the work is serialisation and a
save→load→save round-trip test.

Also outstanding: greying out already-taken options (Part 8b — it belongs in the
profile, not the save).

**State:** all tests green — 7 content, 7 headless app, 2 documentation
alignment. Nothing in flight. Run `cargo wt` for the current count rather than
trusting this line.

Stage 4 and the cast fix were **confirmed by playing**, not only by the checks:
the map regions click where they look like they should, the hover label reads,
and the characters are present again. Worth stating separately, because every
engine bug in this project so far passed the automated layers and was found at
this one.

### Later the same day: stage 5, save/load

The last stage of the exploration roadmap. F5 saves, F9 loads, and the menu
offers **Continue** when a slot exists.

**The file holds what cannot be derived, and nothing else.** The control stack,
flags, inventory — plus one field of apparent presentation state, the playing
track, because music is *ambient*: the scene that started it may be several
jumps in the past, so no replay of the current stack can reach it. Scenes are
stored by **name**, not as the generated enum, so a save survives scenes being
added or reordered and reads plainly in a text editor.

`StoryState::current` is deliberately absent: it is a cached scene, recomputed
from the topmost story frame on load. A save that stores a cache can contradict
itself.

**The cursor needed a correction nobody predicted.** `advance` steps *past* a
line before handing control back, so a save points at the line *after* the one
being read. Restoring verbatim silently skips a line. `rewind_to_current_line`
steps back onto it — and the reason this was found at all is the round-trip
test the design specified two years of sessions ago: save → load → save must be
byte-identical, and without the rewind the cursor drifted one step forward every
cycle.

**A test that could not fail.** The soundtrack test passed with the restoring
line deleted, because the track it saved was the one a fresh run starts anyway.
Fixed by choosing a value only the save file can produce. Written up in the
antipatterns — the break-it-and-confirm habit caught a bug in the *test*, which
is the clearest argument for the habit yet.

**One ordering hazard, expressed as a run condition.** Leaving the menu runs
`restart`, which resets the story; "Continue" leaves the menu too. Rather than
rely on schedule position, the installer runs everywhere *except* the menu, and
`restart` stands aside when a load is pending. Two independent guards, neither
depending on the other's position in the chain.

### Numbers, end of day

| | |
| --- | --- |
| Tests | 20 — 7 content, 11 headless app, 2 documentation alignment |
| New guarantees | 4, each verified by deletion: 2 tests died for the cursor rewind, 1 each for the ambient field and the restored flags |
| Save file | ~20 lines of RON, readable and diffable |

### What is left

The exploration roadmap is **complete** — stages 1 to 5 are all in. Remaining in
`DESIGN.md` Part 8b: greying out already-taken options, which belongs in the
profile rather than the save. Beyond that the milestone table's outstanding
entries are backlog, auto-advance and skip (9), settings (10), transitions (11),
and gallery/achievements/voice (12).

### Later still: hardening, after the stages

Five weaknesses a working game was hiding. Nothing was broken; this is what
"better structure, identify critical issues" turned out to mean.

#### Bevy's ambiguity detector found 43 pairs — and a bug in the architecture

`ScheduleBuildSettings { ambiguity_detection: LogLevel::Warn }` on the `Update`
schedule reports systems that touch the same data with **no order between
them**. Off by default, so it has to be asked for.

It reported **43 pairs, 42 of them ours** — because the engine registered three
separate `Update` chains (presentation, story, exploration), each internally
ordered and mutually unordered.

The interesting part was not the indeterminacy. It was that the presentation
chain had been registered **first**, so the reconcilers read `Stage` *before* the
logic wrote it: every frame drew the previous frame's state. Invisible at 60fps,
and the exact opposite of the rule the crate is built on.

Fixed structurally rather than with ordering patches — five named sets, chained:

```
Session -> Story -> Explore -> Persist -> Present
```

Reading those five names is now the answer to "what happens in a frame". **43
pairs became 1**, and the survivor is `handle_ui_buttons` against Bevy's own
`TerminalCtrlCHandlerPlugin::exit_on_flag`, both writing `AppExit` messages —
harmless, engine-owned, and deliberately left rather than papered over by
coupling our plugin to an engine internal.

The parallelism given up is nominal: nearly every one of these systems touches
`Stage` or `StoryState`, so they could never have run concurrently anyway.

#### `vn_core` had no tests of its own

All 20 tests lived in `demo_game` and ran against one game's scenes. A library
validated only through its demo cannot catch a regression that demo does not
happen to exercise, and its tests break whenever the story is edited.

Now 15 tests inside `vn_core`, against a **synthetic** three-scene game that
exists purely to be awkward. Two findings from writing them:

- The fixture's guarded branch option was originally *last*, so the surviving
  option kept index 0 — and a bug that renumbered options after filtering would
  have been invisible. Moving the guarded option first made the test able to
  fail. **Fixture order is part of a test's strength.**
- An assertion that black never selects a hotspot could not fail, because
  nothing in the fixture claimed black. Adding a deliberately mis-authored
  black region — the thing `build.rs` forbids in real content — turned a
  decorative assertion into a real one.

#### The input layer is now tested, including the cursor

This is where **every engine bug in this project has lived**, and it had no
tests at all. Four now, all driving the app as a player does:

- **Keyboard** via `KeyboardInput` *messages*. Writing to `ButtonInput` directly
  does not work — `keyboard_input_system` clears it in `PreUpdate`, so a press
  written from outside the schedule is wiped before any system sees it.
- **Buttons** by inserting `Interaction::Pressed`, which counts as a change and
  so satisfies `Changed<Interaction>`.
- **The full hotspot path** — cursor position → camera → design space → lookup
  pixel → option → click. This was the one expected to fail: it needs a camera,
  and cameras are normally filled in by the render app. It worked, because
  `Camera.computed`, `RenderTargetInfo` and `Window::set_cursor_position` are
  all public. Two things are fabricated (the camera's computed values, and the
  lookup image, since there is no PNG loader headlessly), and the cursor
  position is derived by asking *the same camera* where a known world point
  lands — so the test depends on the projection being self-consistent rather
  than on having re-derived Bevy's matrix correctly.

#### Save location, decided for production rather than for dev

Saves were landing in whatever directory the process was launched from. The
question that reframed it: *how would this be managed in a release?*

Answer: a **per-user OS data directory** — `%APPDATA%\<game>\saves` and its
equivalents — because the install directory is not user-writable on Windows, a
machine has several users, and cloud sync needs a stable path outside the
install.

Resolved from environment variables rather than by adding the `directories`
crate: that crate pulls `windows-sys`, which Bevy already uses through winit,
and a *normal* dependency sharing a crate with the engine is how a
feature-unification cascade starts. Fifteen lines instead.

Saves are now written to a staging file and **renamed over** the slot. Writing in
place means a crash mid-write truncates the player's only copy; rename is atomic
on Windows and POSIX both.

#### The decodable-format list now reads the manifest

`build.rs` kept a hand-copied list of decodable extensions with a comment asking
the author to keep it in step with `games/Cargo.toml`. That guard exists
*because* a `.webp` slipped through once, so a guard that can silently describe
the wrong engine had to go. It now parses the workspace manifest — `toml` as a
build-dependency, feature-isolated like `image`, no engine rebuild.

One trap on the way: deriving purely from the explicit feature list would have
**dropped `.ogg`**, which is every piece of music in the game. Codecs also arrive
from *profiles* — `common_api` brings `png` and `hdr`, and
`audio = ["bevy_audio", "vorbis"]` brings Ogg. Verified against `bevy/Cargo.toml`
rather than assumed, and the profile-implied set is now a named constant with
that citation.

Verified by pointing the derivation at a fixture manifest with `webp` removed:

```
asset `backgrounds/mountains.webp` has extension `.webp`, which is not among
the formats this build of bevy can decode. ... Currently decodable:
{"bmp", "gif", "hdr", "jpeg", "jpg", "mp3", "ogg", "png", "qoi", "tga", "wav"}
```

A fixture rather than a real feature change on purpose: removing `webp` from the
manifest and putting it back costs two full engine rebuilds, ~40 minutes, for
the same evidence. What this does not prove is that cargo would actually disable
the feature — but that is cargo's job, not this guard's.

#### Numbers

| | |
| --- | --- |
| Tests | 39 — 15 `vn_core` unit, 22 `demo_game` app/content, 2 documentation alignment |
| Ambiguous system pairs | 43 → 1 (the survivor is Bevy's own, and benign) |
| New guarantees verified by deletion | 10, each taking down exactly the expected test |
| Incremental rebuild | ~40s; `toml` added as a build-dep with no engine rebuild |

#### Changed in the other documents

- Guide: *state the frame's order as sets*; *a library's tests must not depend
  on one consumer's content*; *drive input as messages*; *resolve user paths from
  the environment*; *write saves atomically*; *make fixtures able to fail*.
- `DESIGN.md`: Part 3 gains the real save location and the atomic write; Part 8b
  loses the completed items; a new Part 2.5 on the frame's order.

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
