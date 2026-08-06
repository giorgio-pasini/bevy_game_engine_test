# Engineering review — 2026-08-05

A critical read of `vn_core` and `demo_game` after the five exploration stages
and the hardening pass, looking for three things specifically:

1. **antipatterns we implemented ourselves** — including ones this repository
   documents and then committed anyway;
2. **good patterns that were available and not applied**;
3. **unnecessary engineering**, where a simpler and equally good option existed.

**Nothing in here is a live bug.** The game works, 39 tests pass, and every
finding below is a hazard, a duplication or an inconsistency rather than a
defect a player could hit today. That is the point of writing it down: these are
the things that become defects later, and they are cheapest to remove now.

Findings are lettered by category and ordered within it by value. Each gives the
evidence, the correction, and what the correction costs — in lines, in risk, and
in what would verify it.

This document exists to make the decisions before the edits, so each finding is
written as a proposal. **One has since been carried out** — C3, on 2026-08-06 —
and carries an outcome banner; the rest stand as written. The status table at the
end is the quick answer to "what is left".

---

## A — antipatterns we implemented ourselves

### A1. `AppliedStage` caches what the world already knows

**What.** `vn_core/src/present.rs` defines a resource holding the background
path, the cast on stage and the music track that the presentation "has already
applied". Each of the three reconcilers compares against it and then writes back
to it — six sites in one file.

**Why it matters.** This is, verbatim, the antipattern in
`bevytest_documentation/ai_gamedev_antipatterns_for_bevy.md`: *"if a variable's
job is to describe something the world already knows, delete it and ask the
world."* It is the same shape as the `Local` cache that shipped a dead end in
this project — a system that refused to redraw because it believed it already
had.

It is **not currently wrong**: `clear_visuals` and `stop_everything` reset
`Stage`, which makes the reconcilers despawn and clear the cache in step. But
that correctness rests on every future teardown path remembering the cache
exists. The bug it invites is exactly the one already paid for: something
despawns a sprite outside a reconciler, the cache still says "applied", and the
screen stays blank.

**Why it was written.** Not gratuitously — it buys **per-field granularity**.
Bevy's own change detection marks the whole resource changed, so a background
swap would also restart the music. That granularity has to come from somewhere.

**How to correct it.** Take it from the world instead: carry the identity on the
entity, which is what `ExploreRoot(String)` already does — a pattern this
codebase adopted *specifically* as the fix for the `Local` bug.

- `BackgroundSprite` becomes `BackgroundSprite(String)` holding its path;
- `MusicPlayer` becomes `MusicPlayer(String)` holding its track;
- `CharacterSprite` already carries `slot` and `id`, so the cast needs nothing.

Each reconciler then compares the wanted state against what is actually on
screen. The resource is deleted.

**Cost.** ~40 lines across three systems in `vn_core/src/present.rs`, plus one
line in `vn_core/src/lib.rs`. **Medium risk** — it touches everything that draws.
Verification is already in place: the headless tests assert a background exists
after entering `Playing`, that music survives a phase change, and that options
redraw on return. Break each reconciler in turn and confirm the matching test
fails.

---

### A2. `StoryState::current` is a cached scene with five sync points

**What.** `StoryState` holds `current: Scene<S>`, a copy of the scene named by
the topmost story frame. Five places must remember to update it —
`enter_scene`, `call_scene` and `pop` in `vn_core/src/story.rs`, `reset` in the
same file, and `restore` in `vn_core/src/save.rs`, which had to re-implement the
derivation from scratch.

**Why it matters.** Same rule as A1, one layer down: it is a cache of something
the stack already determines. Worse, `stack` is a public field, so **anything can
desync it silently** — the tests already do, calling `stack.truncate(1)` to
simulate wandering off. Nothing enforces that the cache and the stack agree, and
a mismatch means the engine runs steps from the wrong scene.

**How to correct it.** Two options, and the obvious one is the worse one:

- *Derive on demand* — delete the field and call `(script.scene_of)(id)` where
  needed. Clean, but `scene_of` clones a whole scene including its prose, and
  `advance` touches it once per step. Wasteful for no gain.
- **Enforce the invariant instead** — make `stack` and `current` private, so the
  only way to move the cursor is through the existing methods, which already
  maintain both. The compiler then guarantees what a comment currently asks for.
  `flags` and `items` stay public: they are plain data with no invariant.

**Cost.** ~30 lines in `vn_core/src/story.rs` plus roughly six call sites in the
tests that mutate the stack directly, which need small helpers. **Low risk** —
the compiler finds every use, and nothing can be missed silently.

---

### A3. The availability check is written four times

**What.** "Does this condition allow this option" appears in four places: once
in `Places::available` in `vn_core/src/place.rs`, and three times in
`vn_core/src/story.rs` — filtering a branch's choices, re-checking in
`take_choice`, and re-checking in `take_option`.

**Why it matters.** Minor today, because all four are one line and identical. It
becomes real the moment the rule grows — a "seen already" flag, a debug override,
an accessibility setting — and one of the four is missed. The re-checks exist
precisely because a button can outlive the state that offered it, so they must
agree with the filter by construction, not by inspection.

**How to correct it.** One helper — `fn allows(condition: &Option<Condition>,
flags, items) -> bool` — used by all four.

**Cost.** ~10 lines. No risk. Existing tests cover all four paths.

---

## B — good patterns available, not applied

### B1. The tests leak temporary directories — 194 of them

**What.** `harness()` in `demo_game/src/app_tests.rs` gives every test its own
save directory under the system temp folder, and never removes it. Measured
today: **193 directories** from that harness, plus **1** from `vn_core`'s
atomic-write test.

**Why it matters.** It is litter that grows with every test run — roughly a
dozen directories per `cargo wt`. It is also a small correctness smell: a test
that cannot clean up after itself usually cannot be run twice in the same
process safely either.

The single leaked directory from the atomic-write test is the more interesting
one. That test *does* clean up — with `remove_dir_all` as its last statement. It
leaked anyway, from the run where atomicity was deliberately broken to prove the
test worked: **the assertion panicked before the cleanup line was reached.**
Cleanup written as a trailing statement runs only when the test passes, which is
the case where it matters least.

**How to correct it.** The pattern the language provides for exactly this: an
RAII guard.

```rust
struct TempSaves(PathBuf);
impl Drop for TempSaves {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
```

`Drop` runs while the stack unwinds, so it cleans up on failure too. Hold one in
each test that writes files.

**Cost.** ~15 lines in `demo_game/src/app_tests.rs`, plus the same treatment in
`vn_core/src/tests.rs`. No risk. The 194 existing directories can be deleted in
one command — worth doing knowingly rather than as a side effect.

---

### B2. Nothing checks that replay agrees with the engine

**What.** There are two interpreters of `Step` in `vn_core/src/lib.rs` and
`vn_core/src/story.rs`: `advance`, which runs the story, and
`replay_story_frame`, which rebuilds the stage from a cursor position. They must
agree about which steps affect the screen.

**Why it matters.** This is the highest-value gap in the review. When they
disagree, the screen silently contradicts the story — and that class has already
produced two shipped bugs here: scenes rendering blank, and music stopping.
Today the only protections are an explicit `match` with every variant spelled
out, which stops a *new* variant being ignored silently, and human diligence
about what each variant should do. Nothing checks that the two agree.

**How to correct it.** A property test over the synthetic fixture in
`vn_core/src/tests.rs`: for every cursor position in a scene, the `Stage`
produced by replaying to *n* must equal the `Stage` produced by advancing to *n*.
`Stage` already derives `PartialEq`, so the assertion is direct.

**Cost.** ~30 lines. No risk, and no new dependencies. Verify it the usual way:
add a stage effect to `advance` alone and confirm the test goes red.

---

### B3. The `dev` feature is copied across three manifests

**What.** `dev = ["bevy/dynamic_linking", "bevy/dev", "bevy/track_location"]`
appears in `snake/Cargo.toml`, `vn_core/Cargo.toml` and `demo_game/Cargo.toml`
(the last also forwarding `vn_core/dev`), with a comment asking that they stay
identical.

**Why it matters.** It is the same class of defect that was just removed from
`build.rs`'s format list: **a fact copied three times, guarded by a comment.**
The consequence of drift is specific and expensive — members with different
feature sets make `cargo build --workspace` and a single-member build resolve
differently, which is the engine-rebuild cascade documented in `DESIGN.md`
Part 8.

**How to correct it.** `build.rs` already takes `toml` as a build-dependency and
already reads the workspace manifest. Have it read the three member manifests
and fail the build if their `dev` feature lists diverge.

**Cost.** ~25 lines. No risk. One wrinkle worth accepting knowingly:
`demo_game`'s build script would be validating `snake`'s manifest, which is a
layering oddity. The alternative — a workspace-level check — has nowhere
natural to live, since only crates with build scripts run code at build time.

---

### B4. Everything in `vn_core` is public

**What.** Every system, component and helper in the library is `pub`, including
things only `VnPlugin` ever registers.

**Why it matters.** Two costs, both currently theoretical. Any refactor is a
breaking change for a consumer, because the whole surface is API. And nothing
stops a game registering `advance_on_input` itself, out of the set order that
was just established — which would reintroduce exactly the frame ordering
problem the sets were added to fix.

**How to correct it.** `pub(crate)` for what the plugin alone registers; keep
the model types, components a game may query, and the plugin itself public.

**Cost.** ~20 lines plus a few test imports — `demo_game/src/app_tests.rs`
reaches into `vn_core::present` deliberately, and would need those items kept
public with a note saying why.

**Recommendation: defer.** With exactly one consumer this is speculative, and
the project's own over-engineering rule says to wait until a second game asks for
it. Recorded so the decision is deliberate rather than forgotten.

---

## C — unnecessary engineering

### C1. Loading uses two resources and three systems

**What.** A load travels through `LoadRequest` (written by F9 and by the
Continue button), then `read_requested_save` turns it into `PendingLoad`, then
`apply_pending_load` installs it.

**Why it is more than needed.** The two hops each had a reason, and only one
survives scrutiny:

- one place doing file I/O rather than two — **genuine**, and worth keeping;
- `restart` needs to see that a load is in flight so it does not reset the run —
  but it can see that from the *request* just as well as from the pending state.

So the second resource buys nothing that the first does not already provide.

**How to correct it.** Keep `LoadRequest`. Merge reading and installing into one
system that keeps `run_if(not(in_state(AppState::MainMenu)))`, and gate `restart`
on the request instead. One resource and one system disappear; behaviour is
identical.

**Counterpoint, stated honestly.** The two-hop shape mirrors `PickedOption`,
where inputs write a request and one system applies it. Collapsing it makes
loading less symmetric with option handling. The symmetry costs a resource, a
system and a hop — but if a second save slot or an autosave arrives, the split
may earn its keep back.

**Cost.** ~20 lines removed from `vn_core/src/lib.rs` and
`vn_core/src/save.rs`. Low risk — the F9 test and the Continue test both cover
it end to end.

---

### C2. `tools/mapgen` is a whole crate for placeholder art

**What.** 175 lines, its own manifest and lockfile, an `image` dependency, and a
deliberate exclusion from the workspace — all to generate a map picture and its
hotspot lookup.

**Why it is justified *for now*.** The two images must agree pixel for pixel, and
generating both from one set of shapes makes that structural rather than a matter
of care. That is a real guarantee while the art is placeholder.

**Why it will stop being justified.** The moment real art replaces it, the
generator is a crate nobody runs, with a dependency nobody audits, guarding a
guarantee that has moved to `build.rs`'s pixel audit.

**How to correct it.** Not now. Add an expiry note to its header and to
`DESIGN.md` Part 6b: *delete this when real map art lands.* Deleting an unused
tool is easy; noticing it is unused is the hard part, so write the trigger down.

**Cost.** Five minutes.

---

### C3. Three function pointers describe "the game's scenes"

> **Reclassified, then done — 2026-08-06.** This was first written up as
> unnecessary engineering to leave alone, on the grounds that a trait with one
> implementor is over-engineering and the churn buys nothing. That answered the
> wrong question — *is it worth the churn* — and the right one is *which is
> easier for a human to read and maintain*. On that question the current shape
> loses, for reasons that are concrete rather than aesthetic.
>
> **Outcome: 175 lines added, 226 removed — 51 fewer, including the new trait
> and its documentation.** All 39 tests passed unchanged, which is the whole
> verification a behaviour-preserving refactor gets. The description below is
> kept in the future tense as it was written, because the reasoning is the part
> worth re-reading.

**What.** `Script::scene_of`, and `SceneLookup`'s `of_name` and `name_of`, spread
across two resources and two `VnPlugin` fields, all answering questions about the
same generated enum.

**Why it matters — three measured costs.**

1. **It defeats IDE navigation.** On `(script.scene_of)(id)`, "go to definition"
   lands on the *field declaration*, not on the function that runs. Finding the
   real implementation means grepping for `scene_of:` and hopping to `main.rs`.
   Function pointers are values, and values cannot be followed the way types can.
2. **It threads a parameter through the whole engine.** `Script<S>` is carried by
   **7 systems and 8 functions**, almost entirely so that something, eventually,
   can call `scene_of`. Two of those systems already carry
   `#[allow(clippy::too_many_arguments)]`.
3. **The call syntax is unidiomatic.** `(script.scene_of)(*scene)` appears seven
   times and reads like nothing else in the codebase. `scene.content()` is
   ordinary Rust.

**How to correct it.** One trait, implemented once by the game:

```rust
pub trait GameScript: Copy + Eq + Hash + Debug + Send + Sync + 'static {
    fn scene(self) -> Scene<Self>;
    fn name(self) -> &'static str;
    fn from_name(name: &str) -> Option<Self>;
}
```

It replaces `SceneKey` outright, so there is one trait rather than a blanket one
plus two resources of function pointers. `Script` and `SceneLookup` are deleted;
`VnPlugin` goes from seven fields to five; `&script` disappears from every
signature that only carried it to resolve a scene.

The starting scene stays **data**, as a `start: S` field on the plugin — a game
may want to choose it (chapter select, a debug entry point), and it is also what
lets `VnPlugin` stay generic over `S` without a `PhantomData`.

**What the current shape gives up.** Two things, honestly:

- `VnPlugin { .. }` is a single struct literal, so "what must a game supply" is
  one checklist the compiler forces you to complete. With a trait it becomes two
  places — the plugin's fields and the impl.
- Test fakes get slightly longer: `Script { scene_of: my_fn, start: X }` inline
  becomes a small `impl` block. Written once per fixture.

Both are real and neither outweighs a parameter removed from eight signatures.

**Cost.** ~123 use sites across 10 files, all mechanical — the compiler finds
every one. No behaviour changes, so the existing 39 tests are the verification:
they must pass unchanged, and the ones that construct a `Script` by hand are the
only tests that need editing.

**Why this one should not wait for a second game, unlike B4.** Their costs
behave differently. Narrowing visibility later costs exactly what it costs now,
and genuinely needs a second consumer to be designed well — so deferring B4 is
free. This refactor gets **more expensive with every function that threads
`&Script<S>`**, and the exploration work added three. A deferral that compounds
is a decision to pay more later.

---

### C4. `build.rs` is one thousand lines

**What.** `demo_game/build.rs` parses five kinds of file, runs roughly
twenty-two distinct validations, decodes images, and generates Rust, in a single
file.

**Why it matters.** Only navigability. It is well-commented and each guard is
independent, but finding the guard that fired means scrolling. Build scripts can
use modules like any other crate.

**How to correct it.** Split into `build/` modules — parsing, scene guards, place
and hotspot guards, codegen — with `build.rs` reduced to the sequence.

**Cost.** ~1 hour, zero functional change, and the whole suite plus a deliberate
break of one guard per module as verification. **Low priority** — this is
comfort, not risk.

---

## D — examined and cleared

Recorded so these are not re-litigated in a later session.

**The three parallel `Step` enums are self-guarding.** `vn_core` has the model
and the raw deserialised form; `build.rs` has a third mirror it cannot avoid,
since depending on `vn_core` would drag Bevy into the build graph. This looks
like a drift hazard needing a guard, and it is not: content using a variant
`build.rs` does not know fails the build with a serde error naming it, and the
reverse fails the tests. Both mirrors currently list the same sixteen variants.

**Stringly-typed flags and items are adequately guarded.** `BTreeMap<String,
i32>` and `BTreeSet<String>` invite typos, but `build.rs` already cross-checks
every flag read against the flags written, and every item required against the
items granted. An enum would add compile-time safety the build guards already
provide at the same moment — build time.

---

## The colour question

**The evidence.** Sixteen colour literals in `vn_core/src/present.rs`. Three
spawn sites use the same panel colour, and `highlight_choices` hardcodes that
same value a fourth time as its "not hovered" state. Change the panel colour in
the three spawn sites and every button will snap to the old colour the first
time the mouse leaves it.

So the defect is **duplication, not inflexibility** — worth being precise about,
because the two candidate fixes solve different problems.

### Option 1 — constants in `present.rs`

```rust
const PANEL: Color = Color::srgba(0.10, 0.12, 0.18, 0.92);
const PANEL_HOVER: Color = Color::srgba(0.22, 0.26, 0.36, 0.96);
const ACCENT: Color = Color::srgb(0.98, 0.82, 0.36);
const TEXT: Color = Color::srgb(0.94, 0.95, 0.97);
```

| | |
| --- | --- |
| **Pros** | Removes the duplication completely, in ~20 lines. A typo is a compile error. No new file format, no new validation, nothing that can disagree with the code. Usable in `const` position. |
| **Cons** | Retuning means editing Rust and rebuilding (~40s). A non-programmer cannot touch it. Sits oddly beside `assets/presentation.ron`, which exists precisely so that "adjusting the look should not mean editing Rust". |

### Option 2 — move them into `assets/presentation.ron`

| | |
| --- | --- |
| **Pros** | Consistent with the mechanism already built and documented for visual tuning. `build.rs` already parses and range-checks that file, so validating four more fields is incremental. Opens the door to hot reload, since `file_watcher` is already on in dev builds. A designer could retune the palette without a compiler. |
| **Cons** | Several times the work: Bevy's `Color` does not deserialise from RON directly, so it needs a small newtype and conversion; four-plus new fields; new build guards for each. Safety moves from the compiler to a guard — a missing field is caught by `build.rs` rather than by `rustc`. Every use site gains an indirection (`tuning.panel` instead of `PANEL`), and the systems that use colours would need the `Presentation` resource passed in where they currently need nothing. |

### Recommendation

**Constants first.** The problem actually observed is duplication, and constants
remove it entirely for a fifth of the effort. Nobody has asked to retune the
palette, so paying for runtime tunability now is buying insurance against an
imagined future — the exact test `DESIGN.md` Part 4 sets for the
over-engineering line.

The move to `presentation.ron` stays open and gets *cheaper* afterwards: once
every colour is named in one place, relocating them is mechanical. Do it the day
someone wants to retune the palette without a rebuild.

---

## Suggested order, if these are taken up

Grouped so each stage is independently valuable and independently abandonable.

| Stage | Findings | Why together |
| --- | --- | --- |
| ~~1~~ | ~~**C3**~~ — **done 2026-08-06** | Done first because its cost compounds: every function added that threads `&Script<S>` makes it bigger. Purely mechanical, and the existing tests were the proof. |
| 2 | B1, B2, A3, C2 | Cheap, no-risk, no design decisions. Removes the litter, closes the highest-value test gap, and writes down the mapgen expiry. |
| 3 | Colours, C1 | Two contained simplifications with a decision each — palette location, and whether the load hop is worth its resource. |
| 4 | A1, A2 | The two real refactors. Both remove a cache this project has already been bitten by. Do them one at a time, each verified by breaking the tests that cover it. |
| 5 | B3, C4 | Structural tidying with no behavioural effect. |
| — | B4 | Deliberately deferred: an API boundary drawn before a second consumer exists is a guess, and deferring it costs nothing. |
