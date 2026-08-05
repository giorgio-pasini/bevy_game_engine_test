//! Validates the RON scene files and generates Rust from them.
//!
//! This runs **before** the crate compiles, so anything it rejects fails
//! `cargo build` rather than surfacing when the player reaches that scene.
//! It is what makes authoring scenes as external data safe: the structure lives
//! in files, but broken references are still a build error.
//!
//! Checks performed:
//!   * every scene file parses;
//!   * every jump and choice target names an existing scene file;
//!   * every asset path referenced by a step exists on disk;
//!   * every scene is reachable from the start scene.
//!
//! Generates `$OUT_DIR/scenes_generated.rs`, containing a `SceneId` enum built
//! from the filenames plus the embedded source of each file — so Rust code
//! referencing a scene stays compile-checked exactly as before.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The scene the story starts from. Must match `START` in `src/scenes.rs`.
const START_STEM: &str = "awakening";

// Mirrors of vn_core's raw types. Deliberately duplicated rather than depending
// on vn_core: a build script cannot depend on a crate that depends on Bevy
// without dragging the entire engine into the build-dependency graph.
#[derive(serde::Deserialize)]
struct RawScene {
    steps: Vec<RawStep>,
}

#[derive(serde::Deserialize)]
enum RawStep {
    Background(String),
    Show { character: String, slot: Slot },
    Hide { slot: Slot },
    Music(Option<String>),
    Say { speaker: String, text: String },
    Narrate(String),
    Choices(Vec<RawChoice>),
    JumpTo(String),
    JumpIf { condition: Condition, target: String },
    Set(String),
    Clear(String),
    Add { flag: String, delta: i32 },
    GiveItem(String),
    TakeItem(String),
    Explore(String),
    End,
}

#[derive(serde::Deserialize)]
struct RawChoice {
    #[allow(dead_code)]
    text: String,
    target: String,
    #[serde(default)]
    condition: Option<Condition>,
}

// --- places ---------------------------------------------------------------

#[derive(serde::Deserialize)]
struct RawPlace {
    background: String,
    #[serde(default)]
    music: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    caption: Option<String>,
    #[serde(default)]
    lookup: Option<String>,
    options: Vec<RawPlaceOption>,
}

#[derive(serde::Deserialize)]
struct RawPlaceOption {
    #[allow(dead_code)]
    label: String,
    effect: RawEffect,
    #[serde(default)]
    condition: Option<Condition>,
    #[serde(default)]
    region: Option<Rgb>,
}

/// Mirror of `vn_core::place::Rgb`.
#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Rgb(u8, u8, u8);

impl std::fmt::Display for Rgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.0, self.1, self.2)
    }
}

#[derive(serde::Deserialize)]
enum RawEffect {
    GoTo(String),
    Call(String),
    Leave,
}

#[derive(serde::Deserialize)]
enum Condition {
    IsSet(String),
    IsNotSet(String),
    AtLeast(String, i32),
    HasItem(String),
    LacksItem(String),
    All(Vec<Condition>),
    Any(Vec<Condition>),
}

impl Condition {
    fn flags_read(&self, out: &mut Vec<String>) {
        match self {
            Condition::IsSet(k) | Condition::IsNotSet(k) | Condition::AtLeast(k, _) => {
                out.push(k.clone())
            }
            Condition::HasItem(_) | Condition::LacksItem(_) => {}
            Condition::All(cs) | Condition::Any(cs) => cs.iter().for_each(|c| c.flags_read(out)),
        }
    }

    fn items_read(&self, out: &mut Vec<String>) {
        match self {
            Condition::HasItem(i) | Condition::LacksItem(i) => out.push(i.clone()),
            Condition::IsSet(_) | Condition::IsNotSet(_) | Condition::AtLeast(_, _) => {}
            Condition::All(cs) | Condition::Any(cs) => cs.iter().for_each(|c| c.items_read(out)),
        }
    }
}

#[derive(serde::Deserialize)]
struct CastMember {
    #[allow(dead_code)]
    name: String,
    image: String,
    /// Multiplies the global character height. Optional in the file.
    #[serde(default = "one")]
    scale: f32,
}

fn one() -> f32 {
    1.0
}

/// Mirror of `vn_core::story::Presentation`, for build-time validation only.
/// Every field is required, so a missing key is a serde error naming it.
#[derive(serde::Deserialize)]
struct PresentationCheck {
    character_height: f32,
    #[allow(dead_code)]
    slot_offset: f32,
    #[allow(dead_code)]
    character_baseline: f32,
    listener_dim: f32,
    #[allow(dead_code)]
    speaker_rise: f32,
    #[allow(dead_code)]
    speaker_scale: f32,
    cue_speed: f32,
    text_speed: f32,
}

/// Image and audio extensions Bevy can actually decode, given the features
/// enabled for `bevy` in `games/Cargo.toml`.
///
/// A build script cannot inspect another crate's feature flags, so this list is
/// maintained by hand — **keep it in step with the manifest**. It exists because
/// checking only that a file is present let a `.webp` through while the `webp`
/// feature was off, which fails at runtime with a decode error rather than at
/// build time.
const DECODABLE: &[&str] = &[
    // images
    "png", "jpg", "jpeg", "webp", "bmp", "tga", "qoi", "gif", "hdr",
    // audio
    "ogg", "mp3", "wav",
];

#[derive(serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
enum Slot {
    Left,
    Centre,
    Right,
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets = manifest.join("assets");
    let scenes_dir = assets.join("scenes");

    // Re-run whenever content changes, so edits are re-validated.
    println!("cargo::rerun-if-changed=assets");
    println!("cargo::rerun-if-changed=build.rs");

    let mut scenes: BTreeMap<String, (PathBuf, String, RawScene)> = BTreeMap::new();

    let entries = std::fs::read_dir(&scenes_dir)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", scenes_dir.display())));

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| fail(&format!("unusable filename: {}", path.display())))
            .to_string();

        if !stem.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()) {
            fail(&format!(
                "scene filename `{stem}` must be lowercase ascii, digits or underscore \
                 (it becomes a Rust enum variant)"
            ));
        }

        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", path.display())));

        let parsed: RawScene = ron::from_str(&text).unwrap_or_else(|e| {
            fail(&format!("{}: RON parse error: {e}", path.display()))
        });

        scenes.insert(stem, (path, text, parsed));
    }

    if scenes.is_empty() {
        fail(&format!("no .ron scenes found in {}", scenes_dir.display()));
    }
    if !scenes.contains_key(START_STEM) {
        fail(&format!(
            "start scene `{START_STEM}.ron` not found in {}",
            scenes_dir.display()
        ));
    }

    // ---- cast -------------------------------------------------------------

    let cast_path = assets.join("cast.ron");
    let cast_text = std::fs::read_to_string(&cast_path)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", cast_path.display())));
    let cast: BTreeMap<String, CastMember> = ron::from_str(&cast_text)
        .unwrap_or_else(|e| fail(&format!("{}: RON parse error: {e}", cast_path.display())));

    for (id, member) in &cast {
        check_asset_at(&assets, &member.image, &format!("cast `{id}`"));
        if member.scale <= 0.0 {
            fail(&format!("cast `{id}`: scale must be positive, got {}", member.scale));
        }
    }

    // Tuning file: parsed here so a typo or a missing key fails the build rather
    // than panicking at launch. Every field is required, so serde reports an
    // omission for us — no hand-written key list to fall out of date.
    let pres_path = assets.join("presentation.ron");
    let pres_text = std::fs::read_to_string(&pres_path)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", pres_path.display())));
    let pres: PresentationCheck = ron::from_str(&pres_text).unwrap_or_else(|e| {
        fail(&format!("{}: {e}", pres_path.display()))
    });
    if pres.character_height <= 0.0 {
        fail(&format!("{}: character_height must be positive", pres_path.display()));
    }
    if !(0.0..=1.0).contains(&pres.listener_dim) {
        fail(&format!(
            "{}: listener_dim must be between 0.0 and 1.0, got {}",
            pres_path.display(),
            pres.listener_dim
        ));
    }
    if pres.cue_speed <= 0.0 || pres.text_speed <= 0.0 {
        fail(&format!(
            "{}: cue_speed and text_speed must be positive",
            pres_path.display()
        ));
    }

    // ---- places -----------------------------------------------------------

    let places_dir = assets.join("places");
    let mut places: BTreeMap<String, (PathBuf, String, RawPlace)> = BTreeMap::new();
    if places_dir.is_dir() {
        for entry in std::fs::read_dir(&places_dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", path.display())));
            let parsed: RawPlace = ron::from_str(&text)
                .unwrap_or_else(|e| fail(&format!("{}: RON parse error: {e}", path.display())));
            places.insert(stem, (path, text, parsed));
        }
    }

    // ---- validation -------------------------------------------------------

    let known: BTreeSet<&String> = scenes.keys().collect();
    let known_places: BTreeSet<&String> = places.keys().collect();

    // Flags written anywhere vs flags read anywhere. Comparing the two is what
    // catches a mistyped flag name — the failure mode a string-keyed system
    // invites, and one no compiler can see.
    let mut flags_written: BTreeSet<String> = BTreeSet::new();
    let mut flags_read: Vec<(String, String)> = Vec::new();
    // Same treatment for inventory: an item required somewhere must be granted
    // somewhere, or the condition can never hold and the content is unreachable.
    let mut items_given: BTreeSet<String> = BTreeSet::new();
    let mut items_read: Vec<(String, String)> = Vec::new();

    for (stem, (path, _, scene)) in &scenes {
        for (i, step) in scene.steps.iter().enumerate() {
            let where_ = format!("{} (step {i})", path.display());

            match step {
                RawStep::Set(f) | RawStep::Clear(f) => {
                    flags_written.insert(f.clone());
                }
                RawStep::Add { flag, .. } => {
                    flags_written.insert(flag.clone());
                }
                RawStep::GiveItem(i) => {
                    items_given.insert(i.clone());
                }
                RawStep::JumpIf { condition, .. } => {
                    let mut v = Vec::new();
                    condition.flags_read(&mut v);
                    flags_read.extend(v.into_iter().map(|f| (f, where_.clone())));
                    let mut w = Vec::new();
                    condition.items_read(&mut w);
                    items_read.extend(w.into_iter().map(|i| (i, where_.clone())));
                }
                RawStep::Choices(items) => {
                    for c in items {
                        if let Some(cond) = &c.condition {
                            let mut v = Vec::new();
                            cond.flags_read(&mut v);
                            flags_read.extend(v.into_iter().map(|f| (f, where_.clone())));
                            let mut w = Vec::new();
                            cond.items_read(&mut w);
                            items_read.extend(w.into_iter().map(|i| (i, where_.clone())));
                        }
                    }
                }
                _ => {}
            }

            let check_asset = |rel: &str| check_asset_at(&assets, rel, &where_);

            let check_target = |target: &String| {
                if !known.contains(target) {
                    fail(&format!(
                        "{where_}: jumps to `{target}`, but there is no `{target}.ron`. \
                         Known scenes: {:?}",
                        known.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                    ));
                }
            };

            let check_cast = |id: &String, what: &str| {
                if !cast.contains_key(id) {
                    fail(&format!(
                        "{where_}: {what} `{id}` is not in cast.ron. Known: {:?}",
                        cast.keys().collect::<Vec<_>>()
                    ));
                }
            };

            match step {
                RawStep::Background(image) => check_asset(image),
                RawStep::Music(Some(track)) => check_asset(track),
                RawStep::Show { character, .. } => check_cast(character, "character"),
                RawStep::Say { speaker, .. } => check_cast(speaker, "speaker"),
                RawStep::JumpTo(target) => check_target(target),
                RawStep::JumpIf { target, .. } => check_target(target),
                RawStep::Explore(place) => {
                    if !known_places.contains(place) {
                        fail(&format!(
                            "{where_}: explores `{place}`, but there is no places/{place}.ron. \
                             Known places: {:?}",
                            known_places.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                        ));
                    }
                }
                RawStep::Choices(items) => {
                    if items.is_empty() {
                        fail(&format!("{where_}: a Choices step must offer at least one option"));
                    }
                    // At least one option must always be available, or a branch
                    // could leave the player with nothing to click.
                    if items.iter().all(|c| c.condition.is_some()) {
                        fail(&format!(
                            "{where_}: every option is conditional, so this branch can dead-end. \
                             At least one must be unconditional."
                        ));
                    }
                    for c in items {
                        check_target(&c.target);
                    }
                }
                RawStep::Music(None)
                | RawStep::Hide { .. }
                | RawStep::Narrate(_)
                | RawStep::Set(_)
                | RawStep::Clear(_)
                | RawStep::Add { .. }
                | RawStep::GiveItem(_)
                | RawStep::TakeItem(_)
                | RawStep::End => {}
            }
        }
        let _ = stem;
    }

    // Places: assets, targets, and conditions.
    for (stem, (path, _, place)) in &places {
        let where_ = format!("{} (place `{stem}`)", path.display());
        check_asset_at(&assets, &place.background, &where_);
        if let Some(track) = &place.music {
            check_asset_at(&assets, track, &where_);
        }
        if place.options.is_empty() {
            fail(&format!("{where_}: a place must offer at least one option"));
        }
        for (i, option) in place.options.iter().enumerate() {
            let w = format!("{where_} option {i}");
            match &option.effect {
                RawEffect::GoTo(target) => {
                    if !known_places.contains(target) {
                        fail(&format!(
                            "{w}: goes to `{target}`, but there is no places/{target}.ron. \
                             Known places: {:?}",
                            known_places.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                        ));
                    }
                }
                RawEffect::Call(scene) => {
                    if !known.contains(scene) {
                        fail(&format!(
                            "{w}: calls scene `{scene}`, but there is no scenes/{scene}.ron. \
                             Known scenes: {:?}",
                            known.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                        ));
                    }
                }
                RawEffect::Leave => {}
            }
            if let Some(cond) = &option.condition {
                let mut v = Vec::new();
                cond.flags_read(&mut v);
                flags_read.extend(v.into_iter().map(|f| (f, w.clone())));
                let mut items = Vec::new();
                cond.items_read(&mut items);
                items_read.extend(items.into_iter().map(|i| (i, w.clone())));
            }
        }
        check_hotspots(&assets, &where_, place);

        // A place with no way out would trap the player.
        if !place
            .options
            .iter()
            .any(|o| matches!(o.effect, RawEffect::GoTo(_) | RawEffect::Leave) && o.condition.is_none())
        {
            fail(&format!(
                "{where_}: has no unconditional way out. Add a GoTo or Leave option \
                 without a condition, or the player can be stranded here."
            ));
        }
    }

    // Every scene must establish its own background before it says anything.
    //
    // A scene can be entered by several routes — jumped to, or called from a
    // place — and a phase change clears the visible stage. A scene that relies
    // on inheriting whatever was on screen therefore renders blank down some
    // paths and fine down others, which is exactly the kind of bug that only
    // shows up in one branch.
    for (stem, (path, _, scene)) in &scenes {
        let mut background_at = None;
        let mut first_blocking = None;
        for (i, step) in scene.steps.iter().enumerate() {
            match step {
                RawStep::Background(_) if background_at.is_none() => background_at = Some(i),
                RawStep::Say { .. } | RawStep::Narrate(_) | RawStep::Choices(_)
                    if first_blocking.is_none() =>
                {
                    first_blocking = Some(i)
                }
                _ => {}
            }
        }
        match (background_at, first_blocking) {
            (None, Some(_)) => fail(&format!(
                "{}: scene `{stem}` never sets a Background, so it renders blank when \
                 entered after a phase change. Add `Background(\"...\")` as its first step.",
                path.display()
            )),
            (Some(b), Some(f)) if b > f => fail(&format!(
                "{}: scene `{stem}` sets its Background at step {b}, after it starts \
                 speaking at step {f}. Move it before the first line.",
                path.display()
            )),
            _ => {}
        }
    }

    // A speaker must be standing on the stage its own scene built.
    //
    // The twin of the background rule above, and it was missed the first time:
    // only ONE scene of twenty-two ever showed a character, and every other one
    // inherited whoever happened to be on screen. That reads correctly while the
    // story runs straight through and empties the moment a phase change clears
    // the stage — so the golem talked from nowhere for most of the game.
    //
    // A linear scan is exact: branching happens *between* scenes, never inside
    // one, so the steps before a given line always run in this order.
    for (stem, (path, _, scene)) in &scenes {
        let mut on_stage: BTreeMap<&Slot, &String> = BTreeMap::new();
        for (i, step) in scene.steps.iter().enumerate() {
            match step {
                RawStep::Show { character, slot } => {
                    on_stage.insert(slot, character);
                }
                RawStep::Hide { slot } => {
                    on_stage.remove(slot);
                }
                RawStep::Say { speaker, .. } => {
                    if !on_stage.values().any(|c| *c == speaker) {
                        fail(&format!(
                            "{} (step {i}): scene `{stem}` has `{speaker}` speak, but no `Show` \
                             in this scene put them on stage — so they are invisible whenever \
                             this scene is entered after a phase change, and the speaker cue \
                             has no sprite to light. Add `Show(character: \"{speaker}\", \
                             slot: ...)` before the line. If the voice is meant to come from \
                             off-stage, that has to be said explicitly rather than by omission.",
                            path.display()
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    for (item, where_) in &items_read {
        if !items_given.contains(item) {
            fail(&format!(
                "{where_}: condition requires item `{item}`, which no scene ever grants. \
                 Items granted somewhere: {:?}",
                items_given.iter().collect::<Vec<_>>()
            ));
        }
    }

    for (flag, where_) in &flags_read {
        if !flags_written.contains(flag) {
            fail(&format!(
                "{where_}: condition reads flag `{flag}`, which is never set by any scene. \
                 Flags set somewhere: {:?}",
                flags_written.iter().collect::<Vec<_>>()
            ));
        }
    }

    // Reachability across the COMBINED graph: scenes reach places via `Explore`,
    // places reach scenes via `Call` and each other via `GoTo`. Walking them
    // separately would miss content only reachable by crossing between them.
    let mut seen = BTreeSet::new();
    let mut seen_places: BTreeSet<String> = BTreeSet::new();
    let mut queue = vec![START_STEM.to_string()];
    let mut place_queue: Vec<String> = Vec::new();

    loop {
        while let Some(place) = place_queue.pop() {
            if !seen_places.insert(place.clone()) {
                continue;
            }
            if let Some((_, _, p)) = places.get(&place) {
                for option in &p.options {
                    match &option.effect {
                        RawEffect::GoTo(t) => place_queue.push(t.clone()),
                        RawEffect::Call(s) => queue.push(s.clone()),
                        RawEffect::Leave => {}
                    }
                }
            }
        }
        let Some(stem) = queue.pop() else { break };
        if !seen.insert(stem.clone()) {
            continue;
        }
        if let Some((_, _, scene)) = scenes.get(&stem) {
            for step in &scene.steps {
                // Every variant is listed rather than using `_ => {}`. A
                // wildcard here silently skipped `JumpIf` when it was added,
                // and scenes reachable only through it were wrongly reported
                // unreachable. Spelling them out makes the compiler demand an
                // arm for the next new step type.
                match step {
                    RawStep::JumpTo(t) => queue.push(t.clone()),
                    RawStep::JumpIf { target, .. } => queue.push(target.clone()),
                    RawStep::Explore(p) => place_queue.push(p.clone()),
                    RawStep::Choices(items) => {
                        queue.extend(items.iter().map(|c| c.target.clone()))
                    }
                    RawStep::Background(_)
                    | RawStep::Show { .. }
                    | RawStep::Hide { .. }
                    | RawStep::Music(_)
                    | RawStep::Say { .. }
                    | RawStep::Narrate(_)
                    | RawStep::Set(_)
                    | RawStep::Clear(_)
                    | RawStep::Add { .. }
                    | RawStep::GiveItem(_)
                    | RawStep::TakeItem(_)
                    | RawStep::End => {}
                }
            }
        }
    }
    let unreachable: Vec<&str> = scenes
        .keys()
        .filter(|s| !seen.contains(*s))
        .map(|s| s.as_str())
        .collect();
    if !unreachable.is_empty() {
        fail(&format!(
            "these scenes are unreachable from `{START_STEM}`: {unreachable:?}"
        ));
    }
    let orphan_places: Vec<&str> = places
        .keys()
        .filter(|p| !seen_places.contains(*p))
        .map(|s| s.as_str())
        .collect();
    if !orphan_places.is_empty() {
        fail(&format!(
            "these places are unreachable from `{START_STEM}`: {orphan_places:?}"
        ));
    }

    // ---- codegen ----------------------------------------------------------

    let mut out = String::new();
    out.push_str("// @generated by build.rs — do not edit.\n\n");
    out.push_str(
        "#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n\
         pub enum SceneId {\n",
    );
    for stem in scenes.keys() {
        out.push_str(&format!("    {},\n", to_variant(stem)));
    }
    out.push_str("}\n\n");

    out.push_str("impl SceneId {\n");

    out.push_str("    pub const ALL: &'static [SceneId] = &[");
    for stem in scenes.keys() {
        out.push_str(&format!("SceneId::{}, ", to_variant(stem)));
    }
    out.push_str("];\n\n");

    out.push_str("    pub fn stem(self) -> &'static str {\n        match self {\n");
    for stem in scenes.keys() {
        out.push_str(&format!(
            "            SceneId::{} => \"{stem}\",\n",
            to_variant(stem)
        ));
    }
    out.push_str("        }\n    }\n\n");

    out.push_str("    pub fn from_stem(s: &str) -> Option<SceneId> {\n        match s {\n");
    for stem in scenes.keys() {
        out.push_str(&format!(
            "            \"{stem}\" => Some(SceneId::{}),\n",
            to_variant(stem)
        ));
    }
    out.push_str("            _ => None,\n        }\n    }\n\n");

    // Embedding the source guarantees what runs is exactly what was validated.
    out.push_str("    pub fn source(self) -> &'static str {\n        match self {\n");
    for (stem, (path, _, _)) in &scenes {
        let p = path.to_string_lossy().replace('\\', "\\\\");
        out.push_str(&format!(
            "            SceneId::{} => include_str!(\"{p}\"),\n",
            to_variant(stem)
        ));
    }
    out.push_str("        }\n    }\n}\n");

    // Places are keyed by string rather than an enum: unlike scenes, nothing in
    // Rust references a place by name, so an enum would buy no checking that
    // build.rs does not already provide.
    out.push_str("\npub const PLACE_SOURCES: &[(&str, &str)] = &[\n");
    for (stem, (path, _, _)) in &places {
        let p = path.to_string_lossy().replace('\\', "\\\\");
        out.push_str(&format!("    (\"{stem}\", include_str!(\"{p}\")),\n"));
    }
    out.push_str("];\n");

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("scenes_generated.rs");
    std::fs::write(&dest, out).unwrap_or_else(|e| fail(&format!("cannot write generated file: {e}")));

    let _: &Path = &assets;
}

/// The design area's aspect ratio. A hotspot image is sampled in normalised
/// coordinates, so its resolution is free but its shape is not: at any other
/// aspect the background is centre-cropped to fit the screen and the lookup is
/// not, and every hotspot drifts away from the art it was painted over.
const DESIGN_ASPECT: f32 = 1920.0 / 1080.0;

/// The smallest region worth having, as a fraction of the lookup image. Below
/// this a hotspot exists but is a pixel-hunt — the failure that makes
/// point-and-click games infuriating.
const MIN_REGION: f64 = 0.001;

/// Validates a place's colour-keyed hotspots against its actual lookup image.
///
/// Everything here is a failure that is **invisible until played**: a region
/// colour that appears nowhere in the image, one too small to hit, a lossy
/// re-encode that shifted every key by a shade, or anti-aliased edges that fill
/// the boundary of every region with colours matching nothing. None of them are
/// type errors, and none of them produce a message at runtime — the click just
/// does nothing.
fn check_hotspots(assets: &Path, where_: &str, place: &RawPlace) {
    let declared: Vec<(usize, Rgb)> = place
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| o.region.map(|r| (i, r)))
        .collect();

    let lookup = match (&place.lookup, declared.is_empty()) {
        (None, true) => return, // an ordinary place, nothing to check
        (None, false) => fail(&format!(
            "{where_}: option {} declares a region {} but the place has no `lookup:` image, \
             so nothing can ever match it.",
            declared[0].0, declared[0].1
        )),
        (Some(path), true) => fail(&format!(
            "{where_}: declares `lookup: Some(\"{path}\")` but no option has a `region:`, \
             so the map has nothing clickable on it."
        )),
        (Some(path), false) => path,
    };

    check_asset_at(assets, lookup, where_);

    // Lossy formats resample flat colour into gradients. The keys would still
    // *look* right and would no longer be equal to anything.
    let ext = Path::new(lookup)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "png" {
        fail(&format!(
            "{where_}: lookup `{lookup}` is `.{ext}`; hotspot lookups must be PNG. \
             The colours are compared for exact equality, and a lossy encoder shifts them \
             by a shade or two — which is invisible in the image and fatal to every hotspot."
        ));
    }

    for (i, colour) in &declared {
        if *colour == Rgb(0, 0, 0) {
            fail(&format!(
                "{where_} option {i}: black is reserved for `no hotspot here`. \
                 Give this region another colour."
            ));
        }
        if let Some((j, _)) = declared
            .iter()
            .find(|(j, other)| j > i && other == colour)
        {
            fail(&format!(
                "{where_}: options {i} and {j} both claim region {colour}. \
                 One colour can only select one option."
            ));
        }
    }

    let aspect_of = |rel: &str| -> (u32, u32) {
        image::ImageReader::open(assets.join(rel))
            .and_then(|r| r.into_dimensions().map_err(std::io::Error::other))
            .unwrap_or_else(|e| {
                fail(&format!(
                    "{where_}: cannot measure `{rel}`: {e}. A hotspot place's images must be \
                     PNG or WebP — those are the formats this build script can decode."
                ))
            })
    };

    for (what, rel) in [("background", place.background.as_str()), ("lookup", lookup)] {
        let (w, h) = aspect_of(rel);
        let aspect = w as f32 / h as f32;
        if (aspect - DESIGN_ASPECT).abs() > 0.01 {
            fail(&format!(
                "{where_}: {what} `{rel}` is {w}x{h} ({aspect:.3}:1), but hotspot places are \
                 framed 16:9 ({DESIGN_ASPECT:.3}:1). Any other shape is centre-cropped on \
                 screen while the lookup is not, so the regions no longer line up with the art."
            ));
        }
    }

    // The census. Cheap, and it is what turns "I painted the map" into a fact
    // rather than a hope.
    let image = image::ImageReader::open(assets.join(lookup))
        .and_then(|r| r.decode().map_err(std::io::Error::other))
        .unwrap_or_else(|e| fail(&format!("{where_}: cannot decode `{lookup}`: {e}")))
        .to_rgb8();

    let total = (image.width() as u64 * image.height() as u64) as f64;
    let mut census: BTreeMap<Rgb, u64> = BTreeMap::new();
    for px in image.pixels() {
        *census.entry(Rgb(px.0[0], px.0[1], px.0[2])).or_default() += 1;
    }

    for (i, colour) in &declared {
        let count = census.get(colour).copied().unwrap_or(0);
        if count == 0 {
            fail(&format!(
                "{where_} option {i}: region {colour} does not occur anywhere in `{lookup}`, \
                 so this option is unreachable. Colours actually present: {}",
                summarise(&census)
            ));
        }
        if count as f64 / total < MIN_REGION {
            fail(&format!(
                "{where_} option {i}: region {colour} covers {count} of {total:.0} pixels \
                 ({:.4}%), under the {:.1}% minimum — the player would have to hunt for it.",
                count as f64 / total * 100.0,
                MIN_REGION * 100.0
            ));
        }
    }

    let stray: Vec<String> = census
        .iter()
        .filter(|(c, _)| **c != Rgb(0, 0, 0) && !declared.iter().any(|(_, d)| d == *c))
        .map(|(c, n)| format!("{c} x{n}"))
        .collect();
    if !stray.is_empty() {
        fail(&format!(
            "{where_}: `{lookup}` contains {} colour(s) matching no region: {}. \
             The usual cause is ANTI-ALIASING — a soft edge blends the key with its \
             neighbour, and every blended pixel is a dead spot around the whole outline of \
             every hotspot. Export the lookup with anti-aliasing off; black means `nothing \
             here`.",
            stray.len(),
            stray.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
}

/// The colours present, most common first — the useful half of a census when
/// reporting that a declared one is missing.
fn summarise(census: &BTreeMap<Rgb, u64>) -> String {
    let mut by_count: Vec<_> = census.iter().collect();
    by_count.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    by_count
        .iter()
        .take(6)
        .map(|(c, n)| format!("{c} x{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Checks an asset both exists **and** is in a format Bevy was built to decode.
///
/// Existence alone is not enough: a file in an unsupported format loads fine as
/// bytes and then fails at runtime with a decode error, which is exactly the
/// class of failure this build script exists to prevent.
fn check_asset_at(assets: &Path, rel: &str, where_: &str) {
    let path = assets.join(rel);
    if !path.is_file() {
        fail(&format!(
            "{where_}: asset `{rel}` does not exist under {}",
            assets.display()
        ));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !DECODABLE.contains(&ext.as_str()) {
        fail(&format!(
            "{where_}: asset `{rel}` has extension `.{ext}`, which is not among the formats \
             enabled for bevy in games/Cargo.toml. Enable the matching feature (and add the \
             extension to DECODABLE in build.rs), or convert the file. Enabled: {DECODABLE:?}"
        ));
    }
}

/// `golden_gate` -> `GoldenGate`
fn to_variant(stem: &str) -> String {
    stem.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Build scripts communicate failure by panicking; `cargo::error` gives cargo a
/// clean single-line message instead of a backtrace.
fn fail(msg: &str) -> ! {
    println!("cargo::error={msg}");
    panic!("scene validation failed: {msg}");
}
