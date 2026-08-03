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

#[derive(serde::Deserialize)]
enum Condition {
    IsSet(String),
    IsNotSet(String),
    AtLeast(String, i32),
    All(Vec<Condition>),
    Any(Vec<Condition>),
}

impl Condition {
    fn flags_read(&self, out: &mut Vec<String>) {
        match self {
            Condition::IsSet(k) | Condition::IsNotSet(k) | Condition::AtLeast(k, _) => {
                out.push(k.clone())
            }
            Condition::All(cs) | Condition::Any(cs) => cs.iter().for_each(|c| c.flags_read(out)),
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

#[derive(serde::Deserialize)]
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

    // ---- validation -------------------------------------------------------

    let known: BTreeSet<&String> = scenes.keys().collect();

    // Flags written anywhere vs flags read anywhere. Comparing the two is what
    // catches a mistyped flag name — the failure mode a string-keyed system
    // invites, and one no compiler can see.
    let mut flags_written: BTreeSet<String> = BTreeSet::new();
    let mut flags_read: Vec<(String, String)> = Vec::new();

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
                RawStep::JumpIf { condition, .. } => {
                    let mut v = Vec::new();
                    condition.flags_read(&mut v);
                    flags_read.extend(v.into_iter().map(|f| (f, where_.clone())));
                }
                RawStep::Choices(items) => {
                    for c in items {
                        if let Some(cond) = &c.condition {
                            let mut v = Vec::new();
                            cond.flags_read(&mut v);
                            flags_read.extend(v.into_iter().map(|f| (f, where_.clone())));
                        }
                    }
                }
                _ => {}
            }

            let mut check_asset = |rel: &str| check_asset_at(&assets, rel, &where_);

            let mut check_target = |target: &String| {
                if !known.contains(target) {
                    fail(&format!(
                        "{where_}: jumps to `{target}`, but there is no `{target}.ron`. \
                         Known scenes: {:?}",
                        known.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                    ));
                }
            };

            let mut check_cast = |id: &String, what: &str| {
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
                | RawStep::End => {}
            }
        }
        let _ = stem;
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

    // Reachability: content nobody can ever see is almost always a mistake.
    let mut seen = BTreeSet::new();
    let mut queue = vec![START_STEM.to_string()];
    while let Some(stem) = queue.pop() {
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

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("scenes_generated.rs");
    std::fs::write(&dest, out).unwrap_or_else(|e| fail(&format!("cannot write generated file: {e}")));

    let _: &Path = &assets;
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
