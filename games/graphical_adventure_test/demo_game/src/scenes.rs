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

/// Menu chrome. Not story content, so it stays in Rust rather than becoming a
/// scene file.
pub fn text_en(key: TextKey) -> &'static str {
    match key.0 {
        "menu.title" => "The Golden Golem",
        "menu.begin" => "Begin",
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
    /// character art from the cast (steps now name a cast id, not a path).
    fn walk() -> BTreeSet<String> {
        let mut assets = BTreeSet::new();
        for member in cast().0.values() {
            assets.insert(member.image.clone());
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
        for k in ["menu.title", "menu.begin", "menu.quit", "menu.return"] {
            assert_ne!(text_en(TextKey(k)), MISSING, "missing menu string `{k}`");
        }
    }

    #[test]
    fn every_scene_is_reachable_from_start() {
        let mut seen = BTreeSet::new();
        let mut queue = vec![START];
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
                    Step::Background { .. }
                    | Step::Show { .. }
                    | Step::Hide { .. }
                    | Step::Music { .. }
                    | Step::Say { .. }
                    | Step::Narrate { .. }
                    | Step::Set { .. }
                    | Step::Clear { .. }
                    | Step::Add { .. }
                    | Step::End => {}
                }
            }
        }
        let unreachable: Vec<_> = SceneId::ALL.iter().filter(|id| !seen.contains(id)).collect();
        assert!(unreachable.is_empty(), "unreachable scenes: {unreachable:?}");
    }
}
