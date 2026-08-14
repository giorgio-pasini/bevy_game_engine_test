# AI-assisted game development with Bevy — practices that work

Current best understanding. **Prescriptive by design**: rules, not stories. The
reasoning and the failures behind them live in
[ai_gamedev_antipatterns_for_bevy.md](ai_gamedev_antipatterns_for_bevy.md) and
[journal.md](journal.md).

Every entry is tagged by scope and marked with evidence strength — see
[README.md](README.md). Bevy claims carry the version they were verified against.

---

## Working with the engine

### Keep a version-matched engine checkout and read it before writing code
`[bevy]` `[ai-workflow]` — **confirmed**

Clone Bevy at the exact tag you depend on. Before using an unfamiliar API, grep
its `examples/` rather than recalling it.

Bevy's API churns hard between releases, and any given model's training data is
usually a version or two behind. Recalled APIs compile in your head and fail in
the file.

> Verified Bevy 0.19.0, 2026-07-31. A 340-line game written this way
> type-checked on the first attempt with zero warnings; four of its APIs would
> have been wrong from recall.

### Treat the engine's own examples as the specification
`[bevy]` — **confirmed**

When examples disagree with documentation or memory, the examples win — they are
compiled by CI against the version you have.

Also read what they say about themselves. Bevy 0.19's `bevy_ui_widgets` example
warns its patterns "are likely to change substantially"; that is a decision
input, not boilerplate.

### Launch the game. Compiling is not validating
`[bevy]` `[ai-workflow]` — **confirmed**

Bevy resolves ECS query conflicts at **runtime**. `cargo check` and `cargo build`
cannot see them. Every milestone must actually run.

Use `cargo check` as the fast inner loop for type errors, but never conclude
"it works" from it.

### Enable the `dev` feature bundle while developing
`[bevy]` — **confirmed**, Bevy 0.19.0

`bevy/dev` turns on `debug` (names systems and components in ECS errors),
`file_watcher` (asset hot-reload) and `bevy_dev_tools`. Without `debug`, a query
conflict reports `<Enable the debug feature to see the name>` and tells you
nothing.

Gate it behind your own `dev` feature so release builds are unaffected.

### Use `DespawnOnExit(State)` for anything screen-scoped
`[bevy]` — **confirmed**, Bevy 0.19.0

Tie entity lifetime to the state that owns it instead of writing teardown. It
removes a whole class of leak.

---

## Architecture

### Make presentation a pure function of state
`[bevy]` `[architecture]` — **confirmed**

Keep authoritative state in resources or components; let systems *reconcile* the
screen toward it. Never mutate the screen as a side effect of a state change.

The payoff is concrete: save/load, restart, skip and rollback stop being
features and become consequences. Returning a visual novel to its menu needed
exactly one line — clearing the derived stage — because the same systems that
built the screen tore it down.

### Derive from state; do not cache what you can ask for
`[bevy]` `[architecture]` — **confirmed**

This is the same rule as *presentation is a pure function of state*, applied to
the engine's own bookkeeping rather than to the screen — and it is the one that
gets forgotten, because caching feels like an optimisation rather than a design
decision.

Five separate bugs in a single session were all one shape: something remembered
a fact, and reality moved underneath it.

- A `Local` remembered which screen had been drawn; a state exit despawned it,
  and the system then refused to redraw because it "already had".
- A `_ => {}` arm remembered which enum variants existed; a new one appeared.
- Code assumed setting the current state was a no-op; the engine re-ran the
  transition.
- A replay assumed the top of a stack was the whole story; frames beneath it
  held the rest.
- Content assumed a background someone else had set; another entry path had not.

Each fix was the same move: **stop remembering, start asking.** Query the world
for what exists instead of tracking what you spawned. Rebuild from state instead
of patching it incrementally.

The tell is a variable whose job is to describe something the world already
knows. Whenever you find one, ask what happens when the world changes without
telling it — and in an ECS, the world changes without telling anything.

When a cache is retained because deriving it is materially expensive, make the
source and cache private behind one invariant-preserving API. Construction,
restoration and mutation must all pass through that API, with exactly one place
deriving the cached value. This turns "remember to keep these fields aligned"
from a comment into something callers cannot violate.

### Save exactly what cannot be derived — no less, and no more
`[architecture]` — **measured**

"Never put presentation state in a save" is the well-known half of the rule, and
on its own it produces a game that loads back silent or mis-framed. The sharper
form falls out of the classification below:

- **derived** — rebuilt by replaying the state you did save. Never stored.
- **ambient** — cannot be rebuilt, because nothing in the current state records
  where it came from. **Must** be stored, even though it looks like
  presentation.

In a visual novel that is one field. Backgrounds and characters are re-derived
by replaying the stack; the playing track is not, because the scene that started
it may be three jumps in the past. So the save carries one `Option<String>` of
apparent presentation state, and it is correct precisely because it is the piece
replay cannot reach.

Two consequences worth taking:

- **Do not save a cached field.** A cursor's cached scene, a resolved handle, a
  memoised lookup — all recomputed on load. A save that stores a cache can
  contradict itself; one that omits it cannot.
- **Do not replay anything that mutates saved state.** Replay reconstructs the
  screen only. A replay that re-ran `Set` or `Add` would corrupt a save a little
  more on every reload — and the corruption compounds, so it surfaces long after
  the cause.

> Verified by deleting each guarantee in turn and confirming exactly one test
> went red: the ambient field, the restored flags, and the cursor correction
> below.

### Put test cleanup in `Drop`, never at the end of the test
`[rust]` `[testing]` — **measured**

A `remove_dir_all` as a test's last statement runs only when the test **passes**.
The run that leaves the mess is the one that failed — and the run you most want
repeatable is the one you are debugging.

```rust
struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
```

`Drop` runs while the stack unwinds, so a panicking assertion cleans up too. In
a Bevy test, holding the guard **as a resource** is neater still: it dies with
the `App`, so no test needs a cleanup line at all.

> Measured: a harness with no cleanup had left **194 directories**. The one test
> that did tidy up had leaked exactly once — from the run where its guarantee was
> deliberately broken to prove the test worked. After the guard: a run leaves 5
> behind without it, and none with it.

### When two pieces of code must agree, test that they agree
`[architecture]` `[testing]` — **confirmed**

Two interpreters of the same data — an engine and a replay, a writer and a
reader, a validator and a runtime — drift silently, because each is
self-consistent. Nothing type-checks "these two agree".

Assert the agreement directly rather than testing each half:

> A visual novel had `advance` (runs the story) and `replay` (rebuilds the screen
> from a cursor after a load or a phase change). Their disagreement had already
> shipped twice — blank scenes, and music that stopped. The test is one property:
> run the engine to a stopping point, replay to the cursor it stopped at, and the
> two screens must be equal.

Such a test only sees the cases its fixture exercises, so the fixture must use
**every** construct the two interpreters handle — and say so in a comment, since
the next person adding a construct has to add it there too.

### Assert idempotence, not just success
`[testing]` — **measured**

For anything that round-trips — save/load, serialise/deserialise, import/export,
format/parse — the strong test is not "it worked". It is **do it, undo it, do it
again, and compare**.

One assertion then covers a whole class of defect that individual assertions
miss, because any drift at all shows up as a difference:

> Measured: `save → load → save` caught the cursor being one step further along
> each cycle. A save is written *after* the engine has stepped past the line on
> screen, so restoring it verbatim skipped a line — and the round-trip made that
> visible as an inequality rather than as a player complaint months later.

Compare state, not the whole file: a timestamp legitimately differs between two
saves, so keep metadata in its own field and compare the payload.

### Classify every piece of presentation state: re-established or ambient
`[bevy]` `[architecture]` — **confirmed**

Derived presentation splits in two, and the split has to be made deliberately
for each piece:

- **Re-established** — must match the content currently running, so every unit of
  content sets it itself and never inherits it. Backgrounds, characters on
  stage, camera framing.
- **Ambient** — belongs to the moment rather than to the screen, so it persists
  across transitions and is restored from saved state, not from replay. Music,
  volume, weather.

Getting this wrong is invisible along the path you tested. A scene reachable by
three routes that inherits its background renders correctly down one of them.
Music re-derived per scene stops dead every time the player opens a menu.

The test for which category something belongs to: *can the code that rebuilds
the screen work it out from where we are now?* A background can (this scene says
so); a soundtrack cannot (some earlier scene started it).

> Both halves of this were learned by getting them wrong, a day apart. First 20
> of 24 scenes inherited a background, then — the same bug, unnoticed because
> only backgrounds had been audited — 21 of 22 scenes inherited their cast, so
> characters vanished for most of the game the moment any phase change cleared
> the stage.

Then enforce it: a build-time check that each unit of content establishes its own
re-established state is a few lines, and it is the difference between a rule and
a habit.

### State the frame's order once, as system sets
`[bevy]` `[architecture]` — **measured**, Bevy 0.19.0

Registering several `Update` chains — one per feature, each internally ordered —
leaves every *cross*-chain pair undefined. Bevy runs conflicting systems
serially, so this is not a data race; it is worse in one way, because the
arbitrary order looks deliberate and can flip the day someone adds a system.

Name the phases of a frame as `SystemSet`s and chain them once:

```rust
app.configure_sets(Update, (
    Session,                                        // menu, save, load
    Story.run_if(in_state(AppState::Playing)),      // narrative phase
    Explore.run_if(in_state(AppState::Exploring)),  // exploration phase
    Persist,                                        // snapshot after decisions
    Present,                                        // reconcile the screen
).chain());
```

Two payoffs beyond determinism. Reading the set names *is* the answer to "what
happens in a frame". And it forces the question of where presentation belongs —
which, in this project, exposed a real defect.

> Measured: Bevy's ambiguity detector reported **43 pairs, 42 of them ours**. It
> also revealed that the presentation chain had been registered first, so the
> reconcilers read state *before* the logic wrote it — every frame drew the
> previous frame's state, invisibly. After the sets: **1 pair**, engine-owned and
> benign.

The parallelism given up is usually nominal: systems that conflict could not run
concurrently anyway, and these all touched the same two resources.

### Turn on ambiguity detection and read what it says
`[bevy]` `[tooling]` — **measured**, Bevy 0.19.0

It is off by default (`LogLevel::Ignore`), costs one call, and is pure
diagnosis:

```rust
app.edit_schedule(Update, |schedule| {
    schedule.set_build_settings(ScheduleBuildSettings {
        ambiguity_detection: LogLevel::Warn,
        ..default()
    });
});
```

Gate it behind your `dev` feature: the report names the engine's own systems
too. Expect to ignore a few engine-owned pairs — and write down *which*, so
"ignored" stays a decision rather than a habit. Do not silence one by declaring
`ambiguous_with` against an engine internal; that trades a warning for a
coupling.

### One writer per piece of data
`[bevy]` `[architecture]` — **confirmed**

Exactly one system writes each visual concern. Everything else emits events or
updates state. This is also what keeps Bevy's scheduler able to parallelise.

The same rule applied to *input* is what keeps a second input device from
becoming a second implementation. When a menu could be driven both by clicking
buttons and by clicking regions of a map, each input system was reduced to
writing "the player picked option 3" into one resource, and a single system
applied it. The re-checked guard conditions, the stack manipulation and the state
routing exist once. Two copies of that logic would not have stayed identical.

### Encode identity as enums, and produce content from an exhaustive `match`
`[rust]` `[ai-workflow]` — **confirmed**

For scenes, levels, characters, assets: use enums, and resolve them through a
`match` that the compiler forces to be exhaustive.

This matters more with AI-generated content than with hand-written content. The
characteristic AI failure is a *plausible reference to something that does not
exist*. Enums turn that into a build error; strings turn it into a crash during
playtesting. Adding a variant then makes the compiler hand you the to-do list.

### Centralise asset paths in one registry
`[bevy]` `[architecture]` — **confirmed**

Asset paths are strings resolved at runtime; no compiler checks them. Put every
path in a single `match` so one test can validate the whole game.

### Validate external content in `build.rs`, not only in tests
`[rust]` `[ai-workflow]` — **measured**

When content lives in data files, a build script can parse it, validate it, and
generate Rust from it. Anything it rejects fails `cargo build` — which is a
genuinely stronger guarantee than a test, because it cannot be skipped.

This is what makes data-driven content safe for AI-generated work: you keep the
authoring ergonomics of external files *and* the compile-time rejection of
invented references.

Worth validating: every cross-reference between files resolves, every asset path
exists on disk, nothing is unreachable. Generate an enum from the filenames so
code referencing that content stays compile-checked too, and emit
`cargo::rerun-if-changed` so edits re-validate.

> Measured: a typo'd scene link failed the build with the file, the step index,
> the bad name and the list of valid names. Prose edits still rebuild in 9.3s.

It is not a DSL — `serde` does the parsing — so it does not cross the
over-engineering line that hand-writing a parser would.

### Derive a guard from the source of truth, never copy it
`[rust]` `[ai-workflow]` — **confirmed**

A validator that restates a fact — enabled features, supported formats, valid
names — is a second copy of that fact, and copies drift. The failure is
particularly nasty because it is silent *and* inverted: the guard keeps passing
while describing a world that no longer exists, and its existence is what stops
anyone checking.

The tell is a comment asking the reader to keep two things in step. That comment
is a bug report about the design.

> A build script listed the image and audio formats the engine could decode, with
> a note to keep it matching the manifest. It now parses the manifest. A
> build-dependency on `toml` made that five lines, and build-dependency features
> are resolved separately, so it cost no engine rebuild.

Two cautions when deriving:

- **Check what the source of truth actually says.** Deriving only from the
  explicit list would have dropped `.ogg` — every piece of music in the game —
  because Bevy's codecs also arrive through feature *profiles*. What comes from
  a profile is now a named constant citing the engine manifest it was read from.
- **Guard the guard.** If the parse silently matches nothing, the derived list is
  empty and the check either rejects everything or — if anyone "fixes" that by
  falling back — accepts everything. Assert that the derivation found something
  it must always find.

### Validate that assets *load*, not merely that they exist
`[bevy]` `[tooling]` — **confirmed**, Bevy 0.19.0

An engine decodes only the formats it was compiled with. A present-but-
unsupported file passes an existence check and fails at runtime with a decode
error, which is among the least obvious failures to diagnose.

Check the extension against the formats actually enabled, and keep that list
beside the manifest that enables them.

> Bevy 0.19 groups codecs into opt-in features; only `png` and `hdr` arrive via
> the default profiles. A `.webp` asset passed an existence-only validator that
> had been described as thorough the previous day.

### Validate the *contents* of binary assets, not only their paths
`[tooling]` `[ai-workflow]` — **measured**

Once an image or an audio file carries meaning that code depends on — a
colour-keyed hotspot map, a tile atlas, a palette — checking that the file exists
and decodes is no longer enough. The data inside it has to agree with the data
file that describes it, and nothing at runtime will ever complain: the click
simply does nothing.

A build script can decode the image and audit it. Worth checking, in the case of
a hotspot lookup:

- every declared key colour actually **occurs** in the image;
- it covers enough area to be clickable, not one stray pixel;
- **no undeclared colour occurs** — the anti-aliasing guard, and the one that
  matters most, because a soft edge surrounds every region with a halo of
  near-miss colours that match nothing;
- the encoding is **lossless** — a lossy codec shifts flat colours by a shade,
  which is invisible to the eye and fatal to exact-match keys;
- the aspect ratio matches what the game samples it in.

> Measured: decoding a 960x540 lookup and counting colours costs nothing
> noticeable in a build script. Each of the five checks above was verified by
> deliberately breaking it; the anti-aliasing one reported
> `contains 3 colour(s) matching no region: (30, 70, 110) x656, ...`

Adding an image decoder to a build script is safe even in a Bevy project — see
the feature-isolation note under *Build and tooling*.

### Generate paired assets from one definition
`[tooling]` — **confirmed**

When two files must agree pixel-for-pixel — art and its hotspot mask, a tileset
and its collision map — do not maintain them side by side in an editor. Generate
both from one source, so agreement is structural rather than a discipline.

For placeholder art this is nearly free and pays immediately: a ~150-line
generator emitted a map picture and its lookup image from one set of shape
predicates, at different resolutions, and they cannot drift.

When real art arrives the guarantee has to move somewhere, and that somewhere is
the build-time audit above.

### Spell out enum variants in code that must handle all of them
`[rust]` `[ai-workflow]` — **confirmed**

In validators, graph walkers, serialisers and dispatchers, list every variant and
group the no-ops rather than writing `_ => {}`. A wildcard silently absorbs
variants added later — discarding the exhaustiveness guarantee that is the main
reason to model content as enums at all.

> Two independent graph walkers in the same project both used a wildcard and
> both silently skipped a newly added variant, producing false "unreachable
> content" reports that blamed the content rather than the tool.

### Walk the whole content graph in a test, not by hand
`[ai-workflow]` `[testing]` — **measured**

Branching content cannot be verified by playing it. A test that runs the *real*
engine functions over the *real* content, exploring every choice and every
option with a visited-set to bound it, checks every path in about a millisecond.

Assert the things a player would notice: no decision point offers nothing, no
location strands you, at least one ending is reachable, and the walk terminates.

> Found a genuine exploit on its first run: an action returned the player to the
> room that offered it, so a counter could be farmed indefinitely. That state
> space is infinite, which is *why* the walk failed to terminate — the
> non-termination was the bug report. No hand-playthrough finds this, because a
> person does the action once and moves on.

Deduplicate on **push**, not on pop: popping-and-discarding lets duplicates
accumulate faster than they are consumed.

Be honest about the boundary: this validates content, not the engine. A UI that
fails to redraw passes it happily.

### Test the real app headlessly — it is the only layer that sees engine bugs
`[bevy]` `[testing]` — **measured**, Bevy 0.19.0

Content validation and model walks cannot see a view that fails to draw, a state
machine that loops, or audio that stops. Build the **actual** `App` with the
actual plugins, drive it, and assert what a player would notice.

```rust
App::new()
    .add_plugins(MinimalPlugins)
    .add_plugins(AssetPlugin::default())
    .add_plugins(StatesPlugin)
    .add_plugins(bevy::input::InputPlugin)
    .init_asset::<Image>()
    .init_asset::<AudioSource>()
    .add_plugins(YourGamePlugin { .. });
```

No window and no GPU. Systems needing a window find none and return early.
Run `app.update()` a few times so state transitions apply, then query the world:
*are there option buttons?* is a better assertion than any amount of internal
state.

> The hard part is knowing which plugin supplies which resource. `MinimalPlugins`
> carries no input, so every system reading the keyboard failed with
> `Res<ButtonInput<KeyCode>> failed validation: Resource does not exist`.
> Register asset types by hand with `init_asset` rather than pulling in render
> plugins.
>
> Measured: eleven such tests run in 0.17s. They cover two bugs that had already
> shipped and been found by hand, plus every save/load guarantee — each one
> verified by deleting it and watching exactly the expected test go red.

Verify them the same way as any other guard: break the thing they watch, confirm
they fail, revert.

Two things that bite when driving a headless app:

- **Do not write to `ButtonInput` from outside the schedule.** It is cleared in
  `PreUpdate`, so a press written before `app.update()` is wiped before any
  system sees it. Call the function the input system calls, and leave input
  handling to be tested by playing.
- **When a system must not run before a state transition, say so with a run
  condition**, not by where it sits in the schedule. `run_if(not(in_state(..)))`
  survives someone reordering the chain; a position does not.

### A library's tests must not depend on one consumer's content
`[testing]` `[architecture]` — **confirmed**

When an engine and a game live in one repository, it is natural to test the
engine by driving the game. It is also a trap: a regression the game does not
happen to exercise is invisible, and every content edit threatens tests that are
supposed to be about the engine.

Give the library a **synthetic** fixture — a handful of scenes, rooms or levels
that exist only to be awkward: a branch never taken, an option always refused, a
cursor one step past a line. None of it needs to be playable.

Then split by subject, not by convenience: the library's tests assert *engine*
behaviour, and the game's assert *its own content*.

> In this project, 20 tests all lived in the demo and ran against one story.
> Moving the pure logic into the library produced 15 unit tests that need no
> assets, no app and no window, and run in hundredths of a second.

### Prove a reusable API with a deliberately tiny second consumer
`[architecture]` `[testing]` `[ai-workflow]` — **measured**

One consumer proves that a library can serve the application it grew inside.
It does not prove the boundary is reusable. Before polishing or narrowing that
API, add the smallest independent consumer that supplies its own types and
content while asking for no new feature.

Small is what makes the evidence readable: every workaround belongs to the API
rather than to content complexity. A three-scene consumer of `vn_core` exposed
an ambiguous `Scene` name exported by both Bevy's and the library's preludes on
its first compile. The original game hid it through its module structure. One
explicit alias was enough; the important output was knowing where the public
contract rubs against a normal Bevy application.

Do not refactor the API in the same step that creates the evidence. Land the
probe, list what both consumers use, then narrow from facts.

### Drive input as messages, not by writing to `ButtonInput`
`[bevy]` `[testing]` — **confirmed**, Bevy 0.19.0

`keyboard_input_system` **clears** `ButtonInput` in `PreUpdate` and rebuilds it
from `KeyboardInput` messages. A press written from outside the schedule is
therefore wiped before any system sees it — the test does nothing and passes for
the wrong reason.

Write the message instead; it is the same path a window uses:

```rust
world.write_message(KeyboardInput {
    key_code: KeyCode::F5, logical_key: Key::F5,
    state: ButtonState::Pressed,
    text: None, repeat: false, window: Entity::PLACEHOLDER,
});
```

For UI, inserting `Interaction::Pressed` counts as a change and so satisfies
`Changed<Interaction>`.

**Cursor-driven systems are testable too**, which is worth knowing because it
looks impossible without a renderer. `Window::set_cursor_position`,
`Camera.computed` and `RenderTargetInfo` are all public, so a headless test can
fabricate a window and camera and drive a full pointer path. Derive the cursor
position by asking the same camera where a known world point lands — then the
test depends on the projection being self-consistent rather than on your having
re-derived the engine's matrix.

### Write files that matter atomically
`[rust]` — **confirmed**

Serialise to a staging file beside the target, then rename over it. `rename` is
atomic on both Windows and POSIX, so the file is either the old version or the
new one, never half of either.

Writing in place risks a crash, a full disk or a lost battery truncating the
only copy — and for a save file, the only copy is the player's progress. It is
three lines, and it is the difference between "lost the last five minutes" and
"lost the save".

Delete the staging file if the rename itself fails, or a failure leaves litter
that the next attempt has to reason about.

### Resolve user-data paths from the environment, not from a crate
`[rust]` `[tooling]` — **confirmed**

Saves, settings and profiles belong in a **per-user OS directory**
(`%APPDATA%\<game>` and its equivalents), not beside the executable and not in
the working directory. The install directory is not user-writable on Windows, a
machine may have several users, and cloud sync expects a stable path.

The obvious answer is the `directories` crate. In an engine project, check what
it pulls in first: it depends on `windows-sys`, which Bevy already uses through
winit, and **a normal dependency sharing a crate with the engine is how a
feature-unification cascade starts**. Fifteen lines of `env::var_os` behind
`#[cfg(target_os)]` costs nothing and cannot touch the engine's feature graph.

Have the *library* take the path as a resource and the *game* supply it. That
also gives tests somewhere harmless to write.

### Write validation tests for what the compiler cannot check
`[rust]` `[ai-workflow]` — **measured**

Compile-time safety over structure leaves two runtime holes: asset paths and
localisation keys. Close them with tests that **walk the real content graph**
rather than a hand-maintained list, so they cannot go stale.

Worth writing: assets exist on disk, text keys resolve, the enum list matches the
enum, no content is unreachable.

> Then break something on purpose and confirm the test fails. A test that has
> never failed has not been shown to work.

---

## Build and tooling

### Batch feature changes — they are the expensive operation
`[rust]` `[tooling]` — **measured**

Any change to an engine feature set rebuilds the whole engine graph. Before
making one, ask what else is pending and do it all at once.

> Measured, Bevy 0.19.0: cold build 21m42s / 532 crates. A later feature change
> rebuilt only 30 crates and still took 20m10s — **crate count is a poor proxy
> for build time**, because the crates a feature change touches are the
> expensive ones.

Corollary: if you know you will need a capability later (3D, an audio format,
an image codec), enable it *now*. Carrying an unused feature costs one build;
adding it later costs two.

### Keep every workspace member's dependency list uniform
`[rust]` `[tooling]` — **measured**

Cargo resolves features over **the set of packages being built**. If members
differ, `cargo build --workspace` and `cargo build -p member` produce different
feature sets and rebuild the engine when you switch.

> Measured: one member pulling `rand`/`chacha20` made the two invocations'
> dependency trees differ by 51 lines around `rand_core`, `getrandom`,
> `arrayref` and `cpufeatures` — which cascades into the engine because Bevy's
> `uuid` also uses `getrandom`.

Declare shared dependencies in `[workspace.dependencies]` and have every member
take them, even where currently unused.

### Build-dependencies are feature-isolated — use them freely
`[rust]` `[tooling]` — **measured**

The rule that a new dependency can trigger a full engine rebuild applies to
*normal* dependencies. Cargo's resolver 2 and 3 resolve **build-dependencies**
separately, so a build script may depend on a crate the engine also uses without
changing the features of the engine's copy.

This matters because it is what lets a build script do real work — decode images,
parse binary formats, hash assets — instead of being limited to text.

> Measured, resolver 3: adding `image` (with `png` + `webp`) to a Bevy crate's
> `[build-dependencies]` rebuilt `image`, `png` and four small leaves as build
> artifacts. No `bevy_*` crate recompiled; the incremental build stayed at 38s.

Pin such a dependency to the version already in `Cargo.lock` so it resolves from
the registry cache rather than fetching a second source copy.

### Keep tools out of the workspace, not just out of the build
`[rust]` `[tooling]` — **confirmed**

An asset generator, a one-off converter or a benchmark harness usually wants
dependencies the game does not have. Adding it as a workspace member puts those
dependencies into the set cargo unifies features over — the exact mechanism that
rebuilds an engine.

Give the tool its own directory with an empty `[workspace]` table in its
manifest. That detaches it: it resolves independently, builds only when run, and
cannot influence the game's feature graph. It still picks up a shared
`.cargo/config.toml` above it, so the build cache is reused anyway.

### Use a workspace with a shared build cache
`[rust]` `[tooling]` — **measured**

A shared `target/` alone gives only *partial* reuse. A workspace additionally
guarantees one `Cargo.lock`, so versions match.

> Measured: two independent projects sharing only a target directory reused the
> ~489 leaf third-party crates but still rebuilt 31 `bevy_*` crates — the
> expensive ones — because feature resolution diverged.

Costs: members cannot build concurrently (cargo locks the target dir), and
`cargo clean` is all-or-nothing.

### Enable `dynamic_linking` behind an opt-in feature
`[bevy]` `[tooling]` — **measured**

Links the engine as a shared library so edits to your own code skip relinking it.

> Measured: 21m42s cold, **14.79s** to rebuild after a one-file edit.

Keep it opt-in — a dynamically linked binary needs `bevy_dylib` shipped beside
it. Note the executable then imports `bevy_dylib-<hash>.dll` from `deps/` and
`std-<hash>.dll` from the toolchain; `cargo run` puts those on `PATH`, running
the binary yourself does not.

### Cap build parallelism on memory-limited machines
`[tooling]` — **measured**

Cargo defaults to one compiler per logical CPU. With optimised dependencies each
can hold multiple GB during LLVM optimisation.

> Measured: 12 parallel compilers on a 6-core / 32 GB laptop exhausted RAM and
> froze the machine outright. `jobs = 4` in `.cargo/config.toml` fixed it.

`jobs` is not part of the artifact fingerprint, so tuning it never forces a
rebuild.

### Check what is actually in the default feature set
`[bevy]` `[tooling]` — **confirmed**, Bevy 0.19.0

Do not assume. 0.19 groups features into profiles (`2d`, `3d`, `ui`, `audio`).
`png` is included via `common_api`; **`jpeg` is not** and must be requested
explicitly or `.jpg` loads fail at runtime. `default_font` *is* included, so text
needs no font asset.

Trace the profile chain in the engine's `Cargo.toml` before assuming a format,
codec or subsystem is available.

---

## Working with the AI

### Test the documentation against the code
`[ai-workflow]` `[testing]` — **measured**

Documentation drifts faster than anyone expects, and an assistant trusts it
completely — so stale docs actively cause wrong work rather than merely failing
to help. Two cheap tests catch most of it:

- **every path a document mentions must resolve on disk** (try the bases your
  documents actually use: their own directory, the repo root, a reference
  checkout);
- **every `Enum::Variant` a document names must exist**, parsed out of the real
  source rather than hardcoded.

> Measured: 0.08s, and they immediately caught a moved file and a renamed
> variant when drift was introduced deliberately to check they fired.

Guard the guard: assert the parse actually found your enums. A parser that
silently matches nothing passes forever.

They cannot catch a document that is *wrong about why* — only one that names
something gone. In practice that is most of the drift, and the rest needs the
discipline below rather than a test.

### When you find a bug, fix its class — then go looking for the siblings
`[ai-workflow]` — **confirmed**

A bug is nearly always an instance of a rule that was never stated. Once the rule
is clear, the work is not finished: something else in the project is almost
certainly breaking the same rule, and it will be found by a human playing rather
than by the guard you just wrote.

The mechanical version of this: after adding a guard, write down the *category*
it protects, then enumerate the other members of that category and check each
one.

> A rule that content must establish its own background was discovered, fixed
> across 20 scenes, given a build-time guard, and written into three documents —
> while the identical problem with characters sat one field away in the same
> struct, untouched, and shipped. It cost a second bug report from the user for
> what was, by then, a completely understood failure.

### Numbers rot fastest — write the command beside them
`[ai-workflow]` — **confirmed**

*Report measurements, not impressions* has a sharp edge: a number in a document
is correct on the day it is written and silently wrong afterwards. In one audit,
**five of eleven** misalignments were stale counts and timings — test counts,
guard counts, a suite duration — in documents whose prose was still accurate.

Nothing checks them, and they are the most quotable part of a document, so they
are what a future session repeats with confidence.

Two habits help:

- For anything that changes as work continues, name the command that prints the
  current value: *"7 content and 11 headless tests — run `cargo test` for the
  live count"*. The number is then a snapshot with an expiry date attached.
- For a number that *is* the finding — a build time, a measured regression —
  stamp it with the date and the conditions, and let it stand as history.

The distinction is whether the number is evidence for a claim or a description
of the current state. Evidence keeps; state rots.

### Write documentation after the work lands, not during
`[ai-workflow]` — **confirmed**

Three times in one session a document described work still in progress, and once
a journal declared a feature outstanding *after it had been built* — which would
have led the next session to build it twice.

Once code exists, **the code is authoritative**. When an implementation diverges
from a design sketch, mark the sketch superseded, state exactly how it differs,
and point at the file holding the truth. Keep the original reasoning visible:
"we considered that, here is why we chose otherwise" is the most valuable thing
a project's documentation carries.

### Documentation outside the AI's load path does not exist
`[ai-workflow]` — **confirmed**

An assistant does not remember previous sessions; the documents are the memory.
But nothing loads them automatically. Put a `CLAUDE.md` at the repository root
pointing at the guide, and one in each project directory, so proximity does the
loading.

> A rule written into a design document was violated by its own author one
> message later, three times in one session. Writing it down is necessary and
> not sufficient — make the correct action the easy one.

### Prefer structural guards over written rules
`[ai-workflow]` — **confirmed**

When a rule is repeatedly broken, change the structure rather than restating it.
Uniform dependency lists beat "remember to use the same cargo command". An
exhaustive `match` beats "remember to update the list".

### Report measurements, not impressions
`[ai-workflow]` — **confirmed**

"21m42s cold, 46s warm" can be checked and regressed against. "Builds are slow"
cannot. This applies to the assistant's own claims most of all.

### Verify claims against the machine, not against expectation
`[ai-workflow]` — **confirmed**

Before recommending, check: does the file exist, does the feature resolve, does
the format decode. Several recommendations in this project were wrong on first
statement and corrected only because they were tested.

### State the residual risk
`[ai-workflow]` — **confirmed**

When something has not been verified — visual appearance, print output, anything
requiring human eyes — say so explicitly rather than implying completeness.
