# Graphical adventure backlog

Deferred work for `vn_core` and its games. This is the authoritative list of
features we intend to revisit after the base engine is proven solid. `DESIGN.md`
keeps the reasoning and historical sketches; this file answers **what is still
wanted, what it depends on, and what would make it done**.

## Gate: solidify the base first

Do not start the player-facing backlog until the reusable base has evidence
beyond its original demo.

- **Second consumer probe — done 2026-08-14.** `reuse_probe` has its own
  three-scene enum, two endings, WebP background, PNG character and Ogg music,
  with no new engine features. Its real plugin test is independent of
  `demo_game`, both branches terminate in the model, and the native binary
  starts with all three asset types.
- **Public API audit — done 2026-08-14.** The two consumers now define a curated
  prelude; plugin-only systems and resources are crate-private. The story type
  is exported as `VnScene`, avoiding Bevy's own `Scene` prelude name while the
  direct `vn_core::story::Scene` path remains available.
- **Feature-set drift guard — done 2026-08-14.** The workspace manifest owns one
  Bevy `dev` contract. `demo_game/build.rs` checks every member against it and
  requires consumers to forward `vn_core/dev`.
- **Manual regression route — awaiting observation.** In `demo_game`, start a
  new game, confirm background/cast/music, advance to **Look around the ridge
  first**, enter **Look out over the valley**, hover and click a labelled map
  hotspot, return to the ridge, save with F5, move elsewhere, load with F9 and
  confirm the saved place returns, then leave via **Go on over the peaks**. In
  `reuse_probe`, confirm its own background/guide/music, then play both
  **Follow the lantern** and **Follow the river**; the river route must hide the
  guide before its final narration. Automation remains the first line; a real
  launch remains the final one.

Splitting the large `demo_game/build.rs`, simplifying the two-hop load request,
and centralising presentation colours are engineering cleanup, not gates. Take
them only when they obstruct feature work.

## Player-facing backlog

### 1. Persistent player profile and seen decisions

The foundation for several later features.

- Store seen lines and taken scene/place options outside save slots.
- Grey out choices and exploration actions the player has already taken.
- Persist atomically in the same per-user game directory as saves.
- Reloading an old save must not erase profile knowledge.

Done when a decision remains visibly seen after restarting the process and
loading a save made before that decision.

### 2. History / backlog screen

- Record displayed narration and dialogue with speaker names.
- Add a scrollable, re-readable log screen.
- Keep history for the current run; decide separately whether any part belongs
  in a save after the interaction is proven.

Done when the player can open the log mid-scene, read earlier lines, close it,
and resume at the exact same story position.

### 3. Auto-advance and skip

- Auto-advance after a delay derived from visible line length and text speed.
- First input during reveal completes the line; it must not also advance it.
- Skip may be restricted to lines recorded as seen in the profile.
- Both modes must stop at choices and exploration transitions.

Done when headless tests cover stopping boundaries and a played run shows no
double-advance or skipped unread choice.

### 4. Settings and persistence

- Text speed and auto delay.
- Master, music, sound-effect and voice volumes.
- Window mode and language selection.
- A settings screen reachable from both menu and play.
- Store settings separately from saves and the player profile.

Done when settings survive restart and changing one does not mutate story state.

### 5. Transitions and screen effects

- Background/scene fades first.
- Later, a small closed set: flash and shake only if content needs them.
- Transitions must remain presentation derived from authoritative state, not
  imperative story side effects.

Done when rapid advance, save/load and phase changes cannot leave a transition
half-applied.

### 6. Audio completion

- Sound-effect one-shots with despawn on completion.
- Music crossfade rather than hard replacement.
- Optional voice clip per line, cancelled correctly on advance or skip.
- Route all audio through the settings volume buses.

Done when audio state survives menus and load according to its ambient/derived
classification, with tests for cancellation and persistence.

### 7. Localisation pipeline

- Keep prose inline while authoring.
- Extract stable keys and source strings into translator-facing files.
- Resolve translated text at runtime with a visible missing-key fallback.
- Do not choose plural/gender machinery until a real target language requires
  it.

Done with one complete second-language route and validation that every extracted
key resolves in both languages.

### 8. Gallery and achievements

- Persist unlocks in the player profile, never in an individual save.
- Gallery for unlocked backgrounds/CGs.
- Named achievements emitted from explicit story events or conditions.

Done when loading an older save cannot revoke an unlock and a fresh profile
starts locked.

### 9. Save UX and compatibility

- Multiple visible slots with timestamp and location metadata.
- Screenshot thumbnails only after the basic slot UI proves useful.
- Add an explicit save-format version and migrations before the first external
  playtester receives a build whose saves should survive updates.

Done when an older fixture save loads through a tested migration and a failed
write leaves the previous slot intact.

### 10. Loading and asset preloading

Current content is embedded or small enough that no loading state is needed.
Add preloading and a loading screen only when measured asset latency causes a
visible hitch in a real game.

## Delivery rule

Take one vertical slice at a time. Each slice ends with build-time guards where
appropriate, headless coverage of the player-visible result, a real launch, and
updates to the guide, antipatterns and journal. Features may move in order when
a real game creates evidence; convenience alone is not evidence.
