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

### One writer per piece of data
`[bevy]` `[architecture]` — **confirmed**

Exactly one system writes each visual concern. Everything else emits events or
updates state. This is also what keeps Bevy's scheduler able to parallelise.

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

### Spell out enum variants in code that must handle all of them
`[rust]` `[ai-workflow]` — **confirmed**

In validators, graph walkers, serialisers and dispatchers, list every variant and
group the no-ops rather than writing `_ => {}`. A wildcard silently absorbs
variants added later — discarding the exhaustiveness guarantee that is the main
reason to model content as enums at all.

> Two independent graph walkers in the same project both used a wildcard and
> both silently skipped a newly added variant, producing false "unreachable
> content" reports that blamed the content rather than the tool.

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
