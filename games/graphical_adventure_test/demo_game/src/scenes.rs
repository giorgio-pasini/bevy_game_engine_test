//! Scene loading.
//!
//! The story itself lives in `assets/scenes/*.ron`, one file per scene, with the
//! prose written inline next to the line that speaks it.
//!
//! `build.rs` validates those files and generates `SceneId` from the filenames,
//! so a jump to a scene that does not exist, or an image path with a typo, is a
//! **build error** — not something you discover mid-playtest.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use vn_core::prelude::*;

// SceneId, SceneId::ALL / stem / from_stem / source — all derived from the
// contents of assets/scenes/.
include!(concat!(env!("OUT_DIR"), "/scenes_generated.rs"));

/// Must match `START_STEM` in `build.rs`, which verifies the file exists.
pub const START: SceneId = SceneId::Awakening;

/// Embedded so it is exactly the file `build.rs` validated.
const CAST_SOURCE: &str = include_str!("../assets/cast.ron");

pub fn cast() -> Cast {
    Cast(ron::from_str(CAST_SOURCE).unwrap_or_else(|e| panic!("cast.ron failed to parse: {e}")))
}

/// Visual tuning — character size, slot spacing, the speaker cue, text speed.
/// Edit `assets/presentation.ron` and rebuild; no Rust changes needed.
const PRESENTATION_SOURCE: &str = include_str!("../assets/presentation.ron");

pub fn presentation() -> Presentation {
    ron::from_str(PRESENTATION_SOURCE)
        .unwrap_or_else(|e| panic!("presentation.ron failed to parse: {e}"))
}

/// Explorable places, from `assets/places/*.ron`.
pub fn places() -> Places {
    let mut map = BTreeMap::new();
    for (id, source) in PLACE_SOURCES {
        let place: Place = ron::from_str(source)
            .unwrap_or_else(|e| panic!("place `{id}` failed to parse: {e}"));
        map.insert((*id).to_string(), place);
    }
    Places(map)
}

/// Resolves a scene name written in a place file to its generated id.
/// `build.rs` has already proven every such name exists.
pub fn scene_of_name(name: &str) -> Option<SceneId> {
    SceneId::from_stem(name)
}

/// The reverse, used to write save files in terms of scene *names* rather than
/// enum ordinals — so a save survives scenes being added or reordered.
pub fn scene_name_of(id: SceneId) -> &'static str {
    id.stem()
}

/// Menu chrome. Not story content, so it stays in Rust rather than becoming a
/// scene file.
pub fn text_en(key: TextKey) -> &'static str {
    match key.0 {
        "menu.title" => "The Golden Golem",
        "menu.begin" => "Begin",
        "menu.continue" => "Continue",
        "menu.quit" => "Quit",
        "menu.return" => "Return to the menu",
        _ => MISSING,
    }
}

pub const MISSING: &str = "<missing text>";

/// Parsed once on first use. The scene files are a few KB, so this needs no
/// loading state and no async.
fn cache() -> &'static BTreeMap<SceneId, Scene<SceneId>> {
    static CACHE: OnceLock<BTreeMap<SceneId, Scene<SceneId>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut map = BTreeMap::new();
        for id in SceneId::ALL {
            let raw: RawScene = ron::from_str(id.source()).unwrap_or_else(|e| {
                // Unreachable in a build that compiled: build.rs parsed this
                // same text. Only a corrupted build could reach here.
                panic!("scene `{}` failed to parse: {e}", id.stem())
            });
            let scene = raw
                .resolve(*id, SceneId::from_stem)
                .unwrap_or_else(|e| panic!("scene `{}`: {e}", id.stem()));
            map.insert(*id, scene);
        }
        map
    })
}

pub fn scene(id: SceneId) -> Scene<SceneId> {
    cache()
        .get(&id)
        .unwrap_or_else(|| panic!("scene `{}` missing from cache", id.stem()))
        .clone()
}

// --- validation -----------------------------------------------------------
//
// build.rs already performs these checks and fails the build. They are repeated
// here because the build script validates at *build* time against the assets
// present then — these catch an asset deleted afterwards, and they run in CI
// without a GPU.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn asset_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    /// Every asset the game can reach: backgrounds and music from steps,
    /// character art from the cast (steps now name a cast id, not a path), and
    /// each place's background, music and hotspot lookup.
    fn walk() -> BTreeSet<String> {
        let mut assets = BTreeSet::new();
        for member in cast().0.values() {
            assets.insert(member.image.clone());
        }
        for place in places().0.values() {
            assets.insert(place.background.clone());
            assets.extend(place.music.clone());
            assets.extend(place.lookup.clone());
        }
        for id in SceneId::ALL {
            for step in scene(*id).steps {
                match step {
                    Step::Background { image } => {
                        assets.insert(image);
                    }
                    Step::Music { track: Some(t) } => {
                        assets.insert(t);
                    }
                    _ => {}
                }
            }
        }
        assets
    }

    /// Every character a scene references must exist in the cast. `build.rs`
    /// enforces this too; this catches a cast entry deleted afterwards.
    #[test]
    fn every_character_is_in_the_cast() {
        let cast = cast();
        let mut missing = BTreeSet::new();
        for id in SceneId::ALL {
            for step in scene(*id).steps {
                match step {
                    Step::Show { character, .. } => {
                        if cast.image_of(&character).is_none() {
                            missing.insert(character);
                        }
                    }
                    Step::Say { speaker, .. } => {
                        if cast.image_of(&speaker).is_none() {
                            missing.insert(speaker);
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(missing.is_empty(), "characters not in cast.ron: {missing:?}");
    }

    #[test]
    fn every_scene_parses_and_resolves() {
        // `scene()` panics on a bad file, so simply walking them all is the test.
        for id in SceneId::ALL {
            assert!(!scene(*id).steps.is_empty(), "scene `{}` is empty", id.stem());
        }
    }

    #[test]
    fn every_referenced_asset_exists() {
        let root = asset_root().join("assets");
        let missing: Vec<_> = walk()
            .into_iter()
            .filter(|p| !root.join(p).is_file())
            .collect();
        assert!(
            missing.is_empty(),
            "assets referenced by scenes but not on disk under {}: {missing:?}",
            root.display()
        );
    }

    #[test]
    fn menu_chrome_resolves() {
        for k in [
            "menu.title",
            "menu.begin",
            "menu.continue",
            "menu.quit",
            "menu.return",
        ] {
            assert_ne!(text_en(TextKey(k)), MISSING, "missing menu string `{k}`");
        }
    }

    /// Exhaustively plays **every branch** of the game and asserts no path
    /// dead-ends.
    ///
    /// This is the answer to "must I replay every branch by hand?" for content:
    /// a depth-first walk over the real story, taking every choice and every
    /// place option, running the actual engine functions rather than a model of
    /// them. It reaches states no hand-playthrough would bother with — the
    /// third option after refusing, having read the cairn but not the ledge.
    ///
    /// It cannot see *engine* bugs. When the option buttons failed to redraw on
    /// returning from a called scene, the content was perfectly valid and this
    /// walk would have passed. Catching that needs a headless run of the real
    /// app, which is a separate piece of work.
    #[test]
    fn no_reachable_path_dead_ends() {
        let script = Script {
            scene_of: scene,
            start: START,
        };
        let places = places();

        // Identity of a situation: where we are, plus what we know and carry.
        // Deduplicating on PUSH keeps the queue bounded — doing it on pop lets
        // duplicates accumulate faster than they are discarded, which is how the
        // first version of this test ran away.
        fn key<S: std::fmt::Debug>(s: &StoryState<SceneId>) -> String
        where
            S: Sized,
        {
            format!("{:?}|{:?}|{:?}", s.stack, s.flags, s.items)
        }

        let start_state = StoryState {
            stack: vec![Frame::Story {
                scene: START,
                step: 0,
            }],
            current: scene(START),
            flags: Default::default(),
            items: Default::default(),
        };

        let mut visited: BTreeSet<String> = BTreeSet::new();
        visited.insert(key::<SceneId>(&start_state));
        let mut queue = vec![start_state];

        let mut endings = 0usize;
        let mut steps = 0usize;

        // Pushes only genuinely new situations.
        macro_rules! push {
            ($visited:expr, $queue:expr, $s:expr) => {{
                let k = key::<SceneId>(&$s);
                if $visited.insert(k) {
                    $queue.push($s);
                }
            }};
        }

        while let Some(mut state) = queue.pop() {
            steps += 1;
            assert!(steps < 50_000, "walk did not terminate — a cycle with no exit?");

            let mut stage = Stage::default();
            let mut line = CurrentLine::default();
            let mut choices = PendingChoices::default();

            let flow = advance(&mut state, &script, &mut stage, &mut line, &mut choices);

            match flow {
                Flow::Ended => endings += 1,
                Flow::Playing(PlayMode::AwaitingChoice) => {
                    assert!(
                        !choices.items.is_empty(),
                        "a Choices step offered nothing with flags {:?} — dead end",
                        state.flags
                    );
                    for (_, index) in &choices.items {
                        let mut branch = state.clone();
                        if take_choice(*index, &mut branch, &script) {
                            push!(visited, queue, branch);
                        }
                    }
                }
                Flow::Playing(_) => {
                    // A line to read: advancing continues from here.
                    push!(visited, queue, state);
                }
                Flow::Exploring => {
                    let id = state.current_place().unwrap().to_string();
                    let options = places.available(&id, &state.flags, &state.items);
                    assert!(
                        !options.is_empty(),
                        "place `{id}` offers nothing with flags {:?} — the player is stranded",
                        state.flags
                    );
                    for (index, _) in options {
                        let mut branch = state.clone();
                        take_option(index, &mut branch, &script, &places, SceneId::from_stem);
                        push!(visited, queue, branch);
                    }
                }
            }
        }

        assert!(endings > 0, "no path ever reaches an ending");
    }

    /// Every scene and place a place option points at must exist.
    #[test]
    fn every_place_target_resolves() {
        let places = places();
        let mut bad = Vec::new();
        for (id, place) in &places.0 {
            for option in &place.options {
                match &option.effect {
                    Effect::GoTo(t) if !places.0.contains_key(t) => {
                        bad.push(format!("{id}: GoTo unknown place `{t}`"))
                    }
                    Effect::Call(s) if SceneId::from_stem(s).is_none() => {
                        bad.push(format!("{id}: Call unknown scene `{s}`"))
                    }
                    _ => {}
                }
            }
        }
        assert!(bad.is_empty(), "{bad:?}");
    }

    /// Walks scenes AND places together. Scenes reached only through a place's
    /// `Call` would look unreachable to a scenes-only walk.
    #[test]
    fn every_scene_is_reachable_from_start() {
        let places = places();
        let mut seen_places: BTreeSet<String> = BTreeSet::new();
        let mut place_queue: Vec<String> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut queue = vec![START];

        // Seed with every place any scene explores, then follow places into the
        // scenes they call.
        for id in SceneId::ALL {
            for step in scene(*id).steps {
                if let Step::Explore { place } = step {
                    place_queue.push(place);
                }
            }
        }
        while let Some(p) = place_queue.pop() {
            if !seen_places.insert(p.clone()) {
                continue;
            }
            if let Some(place) = places.get(&p) {
                for option in &place.options {
                    match &option.effect {
                        Effect::GoTo(t) => place_queue.push(t.clone()),
                        Effect::Call(s) => {
                            if let Some(id) = SceneId::from_stem(s) {
                                queue.push(id);
                            }
                        }
                        Effect::Leave => {}
                    }
                }
            }
        }

        while let Some(id) = queue.pop() {
            if !seen.insert(id) {
                continue;
            }
            for step in scene(id).steps {
                // Variants listed explicitly rather than `_ => {}`. The exact
                // same wildcard in build.rs silently skipped `JumpIf` when it
                // was added, so scenes reachable only through it were reported
                // unreachable. A wildcard opts out of exhaustiveness checking —
                // in a walker that must see every variant, that is the one
                // guarantee worth keeping.
                match step {
                    Step::JumpTo(t) => queue.push(t),
                    Step::JumpIf { target, .. } => queue.push(target),
                    Step::Choices(items) => queue.extend(items.iter().map(|c| c.target)),
                    // `Explore` reaches places, not scenes. Scenes reachable
                    // only through a place's `Call` are covered by
                    // `every_place_target_resolves` below and, definitively, by
                    // build.rs's combined-graph walk.
                    Step::Explore { .. }
                    | Step::Background { .. }
                    | Step::Show { .. }
                    | Step::Hide { .. }
                    | Step::Music { .. }
                    | Step::Say { .. }
                    | Step::Narrate { .. }
                    | Step::Set { .. }
                    | Step::Clear { .. }
                    | Step::Add { .. }
                    | Step::GiveItem { .. }
                    | Step::TakeItem { .. }
                    | Step::End => {}
                }
            }
        }
        let unreachable: Vec<_> = SceneId::ALL.iter().filter(|id| !seen.contains(id)).collect();
        assert!(unreachable.is_empty(), "unreachable scenes: {unreachable:?}");
    }
}
