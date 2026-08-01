//! The story. Structure is Rust, prose lives behind text keys.
//!
//! `scene()` is an exhaustive `match`: add a `SceneId` variant without writing
//! its scene and the compiler names exactly what is missing.

use vn_core::prelude::*;

// `Ord` is here so scenes can go in a `BTreeSet` during validation, which also
// keeps any diagnostic output in a stable order.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SceneId {
    Awakening,
    Offer,
    Accept,
    Refuse,
}

pub const START: SceneId = SceneId::Awakening;

/// Every scene, for validation walks.
///
/// This list can drift when a variant is added, so `all_lists_every_scene`
/// below guards it with an exhaustive `match` — adding a variant breaks that
/// test's compilation, which is the reminder to update this.
pub const ALL_SCENES: &[SceneId] = &[
    SceneId::Awakening,
    SceneId::Offer,
    SceneId::Accept,
    SceneId::Refuse,
];

// --- asset registry -------------------------------------------------------
// The single place paths are written. Everything else refers to these, so one
// validation test can cover the whole game.

const BG_STORM: &str = "backgrounds/golem_01.jpg";
const BG_PEAKS: &str = "backgrounds/golem_05.jpg";
const BG_FORGE: &str = "backgrounds/golem_09.jpg";
const GOLEM: &str = "characters/golem.png";
const MUSIC_MYSTERY: &str = "music/mystery.ogg";
const MUSIC_CALM: &str = "music/calm.ogg";

const GOLEM_NAME: &str = "The Golden Golem";

pub fn scene(id: SceneId) -> Scene<SceneId> {
    match id {
        SceneId::Awakening => awakening(),
        SceneId::Offer => offer(),
        SceneId::Accept => accept(),
        SceneId::Refuse => refuse(),
    }
}

fn awakening() -> Scene<SceneId> {
    Scene {
        id: SceneId::Awakening,
        steps: vec![
            Step::Background { image: BG_STORM },
            Step::Music {
                track: Some(MUSIC_MYSTERY),
            },
            Step::Narrate {
                text: TextKey("awaken.1"),
            },
            Step::Narrate {
                text: TextKey("awaken.2"),
            },
            Step::Show {
                image: GOLEM,
                slot: Slot::Centre,
            },
            Step::Say {
                speaker: GOLEM_NAME,
                text: TextKey("awaken.3"),
            },
            Step::JumpTo(SceneId::Offer),
        ],
    }
}

fn offer() -> Scene<SceneId> {
    Scene {
        id: SceneId::Offer,
        steps: vec![
            Step::Background { image: BG_PEAKS },
            Step::Say {
                speaker: GOLEM_NAME,
                text: TextKey("offer.1"),
            },
            Step::Say {
                speaker: GOLEM_NAME,
                text: TextKey("offer.2"),
            },
            Step::Choices(vec![
                Choice {
                    text: TextKey("choice.accept"),
                    target: SceneId::Accept,
                },
                Choice {
                    text: TextKey("choice.refuse"),
                    target: SceneId::Refuse,
                },
            ]),
        ],
    }
}

fn accept() -> Scene<SceneId> {
    Scene {
        id: SceneId::Accept,
        steps: vec![
            Step::Background { image: BG_FORGE },
            Step::Music {
                track: Some(MUSIC_CALM),
            },
            Step::Say {
                speaker: GOLEM_NAME,
                text: TextKey("accept.1"),
            },
            Step::Narrate {
                text: TextKey("accept.2"),
            },
            Step::Hide {
                slot: Slot::Centre,
            },
            Step::Narrate {
                text: TextKey("accept.3"),
            },
            Step::End,
        ],
    }
}

fn refuse() -> Scene<SceneId> {
    Scene {
        id: SceneId::Refuse,
        steps: vec![
            Step::Music { track: None },
            Step::Say {
                speaker: GOLEM_NAME,
                text: TextKey("refuse.1"),
            },
            Step::Hide {
                slot: Slot::Centre,
            },
            Step::Narrate {
                text: TextKey("refuse.2"),
            },
            Step::End,
        ],
    }
}

// --- localisation ---------------------------------------------------------
// Milestone 5 replaces this with RON tables loaded per language. The TextKey
// indirection is here from the start so that swap is a loader change, not a
// rewrite of every scene.

pub fn text_en(key: TextKey) -> &'static str {
    match key.0 {
        // Menu chrome. Lives in the same table as the prose so translating the
        // game means translating one thing, not two.
        "menu.title" => "The Golden Golem",
        "menu.begin" => "Begin",
        "menu.quit" => "Quit",
        "menu.return" => "Return to the menu",
        "awaken.1" => "Thunder folds the sky into itself. You have been walking for three days, and the mountain has not once looked back at you.",
        "awaken.2" => "Then the storm parts — and something vast turns its head.",
        "awaken.3" => "You are small, and you are late. I have been counting the winters since anyone last climbed this far.",
        "offer.1" => "There is work in me that has gone unfinished for six hundred years. Gears that no longer turn. A name I can no longer pronounce.",
        "offer.2" => "Mend me, and I will carry you over the peaks by morning. Refuse, and I will let you pass — but you will walk, and the walking is long.",
        "choice.accept" => "Reach for the seized gear in his chest.",
        "choice.refuse" => "Step back, and keep your hands to yourself.",
        "accept.1" => "Ah. Warm hands. I had forgotten the shape of that.",
        "accept.2" => "The mechanism gives, complains, and finally turns. Somewhere deep inside the gold, a second heartbeat starts keeping time with yours.",
        "accept.3" => "He carries you over the peaks by morning, exactly as promised. Neither of you says anything about the winters.",
        "refuse.1" => "Wise. Cowardly, but wise. Not every hand that opens me means to close me again.",
        "refuse.2" => "The storm swallows him as you pass. You walk. The walking is long, and you are alone with the sound of your own gears.",
        _ => MISSING,
    }
}

pub const MISSING: &str = "<missing text>";

/// Menu strings live outside any scene, so the walk below cannot find them.
const CHROME_KEYS: &[&str] = &["menu.title", "menu.begin", "menu.quit", "menu.return"];

// --- validation -----------------------------------------------------------
//
// `DESIGN.md` Part 0 argues that putting scene *structure* in Rust makes broken
// jumps a compile error. It is explicit that two things remain runtime lookups:
// asset paths and text keys. These tests are what close that gap — without
// them, the safety argument is only two-thirds true.
//
// Run with: cargo test -p demo_game

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    /// Walks every scene and collects what each step references.
    fn walk() -> (BTreeSet<&'static str>, BTreeSet<&'static str>) {
        let mut assets = BTreeSet::new();
        let mut keys = BTreeSet::new();

        for id in ALL_SCENES {
            for step in scene(*id).steps {
                match step {
                    Step::Background { image } => {
                        assets.insert(image);
                    }
                    Step::Show { image, .. } => {
                        assets.insert(image);
                    }
                    Step::Music { track: Some(t) } => {
                        assets.insert(t);
                    }
                    Step::Say { text, .. } | Step::Narrate { text } => {
                        keys.insert(text.0);
                    }
                    Step::Choices(items) => {
                        for c in items {
                            keys.insert(c.text.0);
                        }
                    }
                    Step::Music { track: None }
                    | Step::Hide { .. }
                    | Step::JumpTo(_)
                    | Step::End => {}
                }
            }
        }
        (assets, keys)
    }

    #[test]
    fn every_referenced_asset_exists() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let (assets, _) = walk();
        assert!(!assets.is_empty(), "walk found no assets — is ALL_SCENES stale?");

        let missing: Vec<_> = assets
            .iter()
            .filter(|p| !root.join(p).is_file())
            .collect();

        assert!(
            missing.is_empty(),
            "assets referenced by scenes but not on disk under {}: {missing:?}",
            root.display()
        );
    }

    #[test]
    fn every_text_key_resolves() {
        let (_, keys) = walk();
        let unresolved: Vec<_> = keys
            .iter()
            .filter(|k| text_en(TextKey(k)) == MISSING)
            .collect();

        assert!(
            unresolved.is_empty(),
            "text keys used by scenes with no English string: {unresolved:?}"
        );
    }

    #[test]
    fn menu_chrome_resolves() {
        let unresolved: Vec<_> = CHROME_KEYS
            .iter()
            .filter(|k| text_en(TextKey(k)) == MISSING)
            .collect();
        assert!(unresolved.is_empty(), "missing menu strings: {unresolved:?}");
    }

    /// Guards `ALL_SCENES` against drift. Adding a `SceneId` variant makes this
    /// match non-exhaustive, so the test stops compiling — which is the
    /// reminder to extend the list.
    #[test]
    fn all_lists_every_scene() {
        for id in ALL_SCENES {
            match id {
                SceneId::Awakening | SceneId::Offer | SceneId::Accept | SceneId::Refuse => {}
            }
        }
        assert_eq!(
            ALL_SCENES.len(),
            4,
            "ALL_SCENES is out of step with the SceneId enum"
        );
    }

    /// Catches orphaned content: a scene nobody can ever reach.
    #[test]
    fn every_scene_is_reachable_from_start() {
        let mut seen = BTreeSet::new();
        let mut queue = vec![START];

        while let Some(id) = queue.pop() {
            if !seen.insert(id) {
                continue;
            }
            for step in scene(id).steps {
                match step {
                    Step::JumpTo(t) => queue.push(t),
                    Step::Choices(items) => queue.extend(items.iter().map(|c| c.target)),
                    _ => {}
                }
            }
        }

        let unreachable: Vec<_> = ALL_SCENES.iter().filter(|id| !seen.contains(id)).collect();
        assert!(unreachable.is_empty(), "unreachable scenes: {unreachable:?}");
    }
}
