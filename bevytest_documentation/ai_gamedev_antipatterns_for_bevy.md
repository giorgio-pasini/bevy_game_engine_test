# AI-assisted game development with Bevy — antipatterns

What went wrong, what it cost, and why the obvious thing was wrong. Written as
**evidence**, not as rules — the rules are in
[ai_gamedev_guide_for_bevy.md](ai_gamedev_guide_for_bevy.md).

Everything here actually happened in this repository.

---

## Writing Bevy APIs from memory

**What it looks like:** the assistant confidently writes idiomatic-looking Bevy
code. It is idiomatic — for a version you are not using.

**Why it bites:** Bevy's API moves fast, and almost all training data and online
tutorials target older releases. The code *looks* right, which is worse than
looking wrong.

**Evidence** — Bevy 0.19.0, all four would have been written incorrectly from
recall:

| Written from memory | Actually 0.19 |
| --- | --- |
| `font_size: 26.0` | `font_size: FontSize::Px(26.0)` |
| `WindowResolution::new(1280.0, 800.0)` | integers, not floats |
| `Trigger<E>` in observers | `On<E>` |
| `EventWriter<AppExit>` + `.send()` | `MessageWriter<AppExit>` + `.write()` |
| `bevy::render::camera::Viewport` | `bevy::camera::Viewport` (the other is private) |

**Cost:** the last one was the single compile error in an otherwise clean
340-line engine — cheap only because everything else had been checked against
the local clone first.

**Instead:** grep the version-matched checkout's `examples/` before writing.

---

## Treating a clean `cargo check` as validation

**What it looks like:** it compiles with zero warnings, so it works.

**Why it bites:** Bevy validates ECS query conflicts at **runtime**, when the
schedule is built. No amount of type-checking sees them.

**Evidence:** a Snake game type-checked with zero warnings and panicked
instantly:

```
error[B0001]: <Enable the debug feature to see the name> in system
<Enable the debug feature to see the name> accesses component(s)
<Enable the debug feature to see the name> in a way that conflicts with a
previous system parameter.
```

The cause was `Query<&mut Pos>` alongside `Query<(Entity, &Pos), With<Food>>` —
food also has a `Pos`, so the sets overlapped. Fixed with `Without<Food>`.

Note the diagnostic named nothing, because `bevy/debug` was off.

**Cost:** a 21-minute build that produced a binary which died in under a second.

**Instead:** always launch. Enable `bevy/dev` so the error names the system.

---

## Changing engine features casually

**What it looks like:** adding one small feature — an image codec, an audio
format — as an afterthought.

**Why it bites:** any feature change invalidates the entire engine graph.

**Evidence:** cold build 21m42s across 532 crates. A later feature change
rebuilt only **30** crates and still took **20m10s** — the crates a feature
change touches are precisely the expensive ones.

**The trap inside the trap:** crate count is not build time. A "we removed
hundreds of crates" claim can be nearly worthless.

**Instead:** batch every pending feature change into one build. Enable
capabilities you know you will need later *now* — carrying an unused feature
costs one build, adding it later costs two.

---

## Assuming what is in the default feature set

**What it looks like:** loading a `.jpg` because `.png` works.

**Why it bites:** Bevy 0.19 groups features into profiles. `png` is pulled in via
`common_api`; **`jpeg` is not in any default profile** and appears only as a
`required-features` entry on two examples.

**Cost:** would have been a runtime asset failure with a confusing message,
"fixed" by a 20-minute rebuild. Caught before building only because the feature
chain was traced rather than assumed.

**Also worth knowing:** `default_font` *is* in the defaults, which is why text
renders with no font asset — the opposite assumption error.

**Instead:** trace the profile chain in the engine's own `Cargo.toml`.

---

## Mixing cargo invocations in a workspace

**What it looks like:** `cargo build --workspace` to build, `cargo run -p thing`
to run. Both reasonable. Both correct in isolation.

**Why it bites:** cargo resolves features over **the set of packages being
built**. Different selections produce different feature sets for shared crates,
so switching between them rebuilds the engine each way.

**Evidence:** the two invocations' dependency trees differed by 51 lines around
`rand_core`, `getrandom`, `arrayref` and `cpufeatures` — because one member
pulled `rand`/`chacha20` and the others did not. It cascades into the engine
because Bevy's `uuid` also uses `getrandom`.

**Cost:** three separate 20-minute rebuilds started by accident in one session —
including once *immediately after* documenting the rule, and once more while
trying to demonstrate a test.

**The real lesson is not about cargo.** A rule that its own author violates three
times in one session is not a documentation problem. The structural fix is to
make every member's dependency list uniform, so no invocation can diverge.
Aliases help you type less; they do not prevent typing something else valid.

---

## Building an engine template on experimental APIs

**What it looks like:** using the engine's newest widget toolkit for menus and
buttons, because that is clearly the intended path.

**Why it bites:** the API is not stable, and the template outlives the release.

**Evidence:** Bevy 0.19's own `bevy_ui_widgets` example says the patterns "are
likely to change substantially as the `bevy_ui_widgets` crate matures, so please
exercise caution if you are using this as a reference for your own code, and note
that there are still 'user experience' issues with this API."

**Instead:** read what examples say about themselves. Plain `Node` + `Button` +
observers is stable and sufficient.

---

## Serialising the world to make save files

**What it looks like:** the engine ships world serialisation
(`bevy_world_serialization`, `DynamicWorld`), so use it for saves.

**Why it bites:** it couples the save format to ECS layout. Rename a component
or split a bundle and every existing save breaks. It also stores enormous
amounts of derived data — sprite entities, transforms, UI nodes — none of which
is meaningful.

**Instead:** snapshot a small, explicit, serde-serialisable struct of the
*authoritative* state and rebuild presentation by replaying. This is only
possible if presentation is a pure function of state, which is one of the
concrete reasons that rule pays.

**Related trap:** deciding what to include. An early draft of a replay routine
re-applied variable assignments, which are already restored from the snapshot.
Harmless for idempotent assignment, silently corrupting the moment anyone adds
an increment — a counter would advance on every load. The rule that survives:
*replay reconstructs derived state only; anything in the snapshot is restored by
deserialisation and never recomputed.*

---

## Letting the build use every core

**What it looks like:** the default. Cargo runs one compiler per logical CPU.

**Why it bites:** with dependencies compiled at `opt-level = 3`, LLVM
optimisation is memory-hungry, and heavy crates each hold multiple GB.

**Evidence:** 12 parallel compilers on a 6-core / 32 GB laptop exhausted RAM,
sent the machine into swap, and froze it completely.

**Cost:** an unusable machine for the duration of a 20-minute build.

**Instead:** `jobs = 4` in `.cargo/config.toml`. It is not part of the artifact
fingerprint, so setting it never triggers a rebuild.

---

## Running the binary directly without the DLL path

**What it looks like:** the build succeeded, so run the `.exe`.

**Why it bites:** with `dynamic_linking`, the executable imports
`bevy_dylib-<hash>.dll` and `std-<hash>.dll`. `cargo run` puts both on `PATH`;
launching the binary yourself does not.

**Evidence:** exit code `0xC0000135` (`STATUS_DLL_NOT_FOUND`) with **no error
message at all**.

Two details that make it worse: the hashed dylib lives in `deps/`, not beside the
executable — the unhashed `bevy_dylib.dll` sitting next to the binary is *not*
the one it imports. And the asset root resolves relative to the executable, so
`BEVY_ASSET_ROOT` must also be set.

---

## Round-tripping UTF-8 files through PowerShell

**What it looks like:** `Get-Content -Raw`, edit the string, `Set-Content`.

**Why it bites:** Windows PowerShell 5.1 reads UTF-8 as ANSI, so every non-ASCII
character is mangled — em-dashes become `â€"`.

**Evidence:** happened twice in one session, the second time in the very file
documenting the first.

**Instead:** use proper editing tools. If PowerShell is unavoidable, use
`[System.IO.File]::ReadAllText($p, [System.Text.Encoding]::UTF8)` and the
matching write.

---

## Caching what the world already knows

**What it looks like:** a `Local`, a field, or an assumption that records
something about the game's state so it need not be recomputed. Ordinary,
sensible-looking code.

**Why it bites:** in an ECS the world changes without notifying anything. The
cache is right until it silently isn't, and the failure appears far from the
cause.

**Evidence — five bugs in one session, all the same shape:**

| Cached | What moved | Symptom |
| --- | --- | --- |
| "which place is displayed" in a `Local` | `DespawnOnExit` removed the UI | Room actions became dead ends: background, no buttons |
| "these are all the variants" via `_ => {}` | a `JumpIf` step was added | Reachable content reported unreachable, twice |
| "setting my current state is a no-op" | Bevy runs identity transitions | Every click re-ran `OnEnter` and restarted the story |
| "the top stack frame is the whole story" | frames beneath held the background | Called scenes rendered blank |
| "a background is already on screen" | another entry path had cleared it | 20 scenes blank down some routes only |

**Cost:** four of the five were invisible to a clean build, a clean test run and
a zero exit code. Every one was found by a human playing the game.

**Instead:** query the world for what exists rather than tracking what you
spawned; rebuild derived state from authoritative state rather than patching it;
list enum variants explicitly; and compare against the current state before
setting it.

The general form: **if a variable's job is to describe something the world
already knows, delete it and ask the world.**

---

## Writing a test that passes for the wrong reason

**What it looks like:** a test asserting the right thing about the right code
path, green on the first run. Nothing about it looks wrong, because nothing
about it *is* wrong — it simply cannot fail.

**Why it bites:** a green test is treated as evidence. This one is evidence of
nothing, and it is worse than no test, because it stops anyone writing the real
one.

**Evidence.** Save files carry the playing track, because music is ambient state
that no replay can reconstruct. The test: start the game, save, load in a fresh
app, assert the track came back.

It passed with the restoring line **deleted**. The saved track was the one the
opening scene starts, and loading runs a fresh start first — so the correct
answer arrived by coincidence down a completely different path.

The fix was to choose a discriminating value: a track that **no frame on the
restored stack sets** and that differs from what a fresh run starts. Then the
save file is the only thing that can produce it.

**Cost:** none, because the break-it-and-confirm habit caught it within a minute
of writing it. That is the entire argument for the habit — the bug it found was
in the test, not in the code, and no amount of reading would have shown it.

**Instead:** after a test goes green, break the thing it watches and confirm it
goes red. When it stays green, ask *what else could produce this value* — a
default, a fresh start, a coincidence of content — and pick an expected value
that only the code under test can produce.

**The fixture is part of the test's strength**, which is the subtler version of
this and bit twice more the same week:

| Fixture as first written | Why nothing could fail | Fix |
| --- | --- | --- |
| A branch whose *guarded* option came last | The surviving option kept index 0, so a bug renumbering options after filtering produced the same answer | Put the guarded option first |
| No option claiming the reserved "empty" colour | The assertion that the reserved colour selects nothing held whether or not the guard existed | Add a deliberately mis-authored option claiming it |

Both were found by breaking the code and watching the test stay green. A fixture
chosen for readability tends to be the one where indices, defaults and
coincidences line up — which is exactly where a test stops discriminating.

---

## Fixing the instance and declaring the class fixed

**What it looks like:** a bug is diagnosed properly, the underlying rule is
articulated, the content is corrected everywhere it applies, a build-time guard
is added so it cannot return, and the finding is written into the guide, the
antipatterns and the journal. Textbook. And the sibling case, one field away in
the same struct, is never looked at.

**Why it bites:** understanding a bug feels like finishing it. The guard that
gets written covers the instance that was reported, and its existence is then
taken as evidence that the whole category is handled — by the same person who
would otherwise have gone looking.

**Evidence.** `Stage` in this project holds three things:

| | Rule it needed | When it got one |
| --- | --- | --- |
| `background` | re-established by every scene | 2026-08-04, after 20 scenes rendered blank |
| `music` | ambient, persists across phases | 2026-08-04, same session |
| `characters` | re-established by every scene | 2026-08-05, after the user reported it |

Twenty-one of twenty-two scenes never staged a character; only the opening scene
did. Within a straight narrative run this is invisible, because a jump leaves the
stage alone — the cast simply persists. It breaks the moment a phase change
clears the stage, which is exactly the feature that had just been built.

The build-time guard that had been written the day before validated
`Background`. Adding the `Say`-speaker-must-be-on-stage guard alongside it was
nine lines and caught every one of the twenty-one scenes immediately.

**Cost:** a full session of playtesting on a game whose characters were absent
for most of its running time, and a second bug report for a failure that had
already been fully understood and documented.

**Instead:** when a rule emerges, enumerate what else it governs before writing
the guard — the enumeration is usually the fields of one struct or the variants
of one enum, and it takes a minute. Then write the guard to cover the category
rather than the case.

---

## Using `_ => {}` in code that must handle every variant

**What it looks like:** a `match` over a content enum that only cares about a few
variants, so the rest are swept up in a wildcard. Perfectly idiomatic Rust.

**Why it bites:** a wildcard arm **opts out of exhaustiveness checking** — the
one guarantee that makes enum-based content safe. Add a variant later and the
code still compiles, silently ignoring it.

**Evidence.** A `JumpIf` step was added to a visual novel. Two separate graph
walkers — the reachability check in `build.rs`, and the equivalent test — both
matched `JumpTo` and `Choices` and ended in `_ => {}`. Neither followed the new
variant, so three scenes reachable only through `JumpIf` were reported
unreachable.

The build script found the first one. The second surfaced only when its test
failed *after* the first was fixed — the identical bug, in a file whose entire
job is catching mistakes.

**Cost:** two false failures and a misleading diagnosis: the tool blamed the
content when the tool itself was wrong.

**Instead:** in any code that must observe every variant — validators, walkers,
serialisers, dispatchers — list the variants explicitly, grouping the no-ops:

```rust
Step::Background { .. } | Step::Show { .. } | Step::End => {}
```

Verbose, and that verbosity is the point: the next variant will not compile
until someone decides what it means here. Keep wildcards for code that genuinely
does not care.

---

## Checking that an asset exists but not that it loads

**What it looks like:** a validator confirms every referenced asset is present on
disk, and is described as covering asset errors.

**Why it bites:** existence is not loadability. An engine only decodes the
formats it was compiled with, so a file in an unsupported format passes the
check, ships, and fails at runtime with a decode error.

**Evidence:** a `mountains.webp` background was added while Bevy's `webp` feature
was off. The build script — written the previous day and described as thorough —
passed it, because the file was unquestionably there. Only tracing the enabled
features by hand caught it before it ran.

**Instead:** validate the *extension against the formats actually enabled*.
Because a build script cannot inspect another crate's feature flags, the list
must be maintained by hand next to the manifest — worth the duplication, since
the failure it prevents is otherwise invisible until runtime:

```
asset `backgrounds/test.tiff` has extension `.tiff`, which is not among the
formats enabled for bevy in games/Cargo.toml. Enabled: ["png", "jpg", ...]
```

**The general lesson:** when you write a validator, ask what it *does not* check.
This one was believed complete for a day.

---

## Confusing compile-time with test-time validation

**What it looks like:** "we'll move content into data files, and add a test that
walks it — so broken references are caught at compilation time."

**Why it bites:** a test does not run at compile time. It runs at `cargo test`,
which is earlier runtime. `cargo build` succeeds, `cargo run` succeeds, and a
broken reference only surfaces if someone remembers to run the tests.

The distinction matters because moving content out of Rust *removes* real
compile-time guarantees — enum variants and exhaustive matches — and it is easy
to believe a test replaces them when it does not.

**What actually gives compile-time validation:** a build script. `build.rs` runs
before the crate compiles, so anything it rejects fails the build outright.

> Verified by deliberate breakage. A typo'd scene target produced:
> ``offer.ron (step 3): jumps to `acccept`, but there is no `acccept.ron`.
> Known scenes: ["accept", "awakening", "offer", "refuse"]`` — and `cargo build`
> exited non-zero.

Keep the tests as well: the build script validates against the files present at
build time, and tests catch an asset deleted afterwards, without needing a GPU.

---

## Reaching for YAML in Rust without checking the ecosystem

**What it looks like:** YAML is the obvious choice for human-authored content —
block scalars, minimal punctuation, ideal for prose.

**Why it bites:** the serde YAML ecosystem is currently unsettled.

> Checked 2026-08-01: `serde_yaml` publishes as **`0.9.34+deprecated`** (last
> released March 2024, deliberately marked by its author). `serde_yml`, the
> usual replacement, is **also** deprecated and is now a shim forwarding to
> `noyalib`.

Adopting YAML means picking a dependency mid-migration, in a project where
dependency changes cost real build time.

**Instead:** RON is already in Bevy's tree, has a documented custom-asset-loader
example, and supports `r#"..."#` raw strings spanning multiple lines — which was
YAML's only substantive advantage for prose. Check crate health before adopting
a format, not after.

---

## Fixing the syntax when the problem is the indirection

**What it looks like:** authoring content is painful, so change the file format.

**Why it bites:** the format may not be what hurts. In this project, scenes were
verbose Rust *and* prose lived behind `TextKey` in a separate table — so adding
one line of dialogue meant editing two files. Moving to a new format while
keeping the keys would have preserved the actual friction and delivered a
disappointing improvement.

**Instead:** identify which part of the workflow costs the most edits before
choosing a fix. Here the win came from writing prose inline next to the line
that speaks it; the format change was secondary.

Localisation was the original reason for the indirection, and it is not lost —
it becomes an *extraction* step later, which is how real localisation tooling
works anyway.

---

## Optimising a metric instead of the goal

**What it looks like:** "we cut the build from 532 crates to 75."

**Why it bites:** the number moved and the goal barely did. Dropping Bevy's `3d`
profile was supposed to remove the heavy 3D crates — but `bevy/dev` pulls in
`bevy_dev_tools`, which depends on `bevy_pbr`, so `bevy_pbr` and `bevy_light`
compiled anyway. Only `bevy_gltf`, `bevy_anti_alias` and `bevy_mikktspace`
actually went away.

**Cost:** a confidently reported saving that was substantially smaller than
claimed, discovered only when a dependency tree was inspected for an unrelated
reason.

**Instead:** verify that the thing you removed is actually absent from the build
log, not merely absent from the feature list.
