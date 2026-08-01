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
    Show { image: String, slot: Slot },
    Hide { slot: Slot },
    Music(Option<String>),
    Say { speaker: String, text: String },
    Narrate(String),
    Choices(Vec<RawChoice>),
    JumpTo(String),
    End,
}

#[derive(serde::Deserialize)]
struct RawChoice {
    #[allow(dead_code)]
    text: String,
    target: String,
}

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

    // ---- validation -------------------------------------------------------

    let known: BTreeSet<&String> = scenes.keys().collect();

    for (stem, (path, _, scene)) in &scenes {
        for (i, step) in scene.steps.iter().enumerate() {
            let where_ = format!("{} (step {i})", path.display());

            let mut check_asset = |rel: &str| {
                if !assets.join(rel).is_file() {
                    fail(&format!(
                        "{where_}: asset `{rel}` does not exist under {}",
                        assets.display()
                    ));
                }
            };

            let mut check_target = |target: &String| {
                if !known.contains(target) {
                    fail(&format!(
                        "{where_}: jumps to `{target}`, but there is no `{target}.ron`. \
                         Known scenes: {:?}",
                        known.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                    ));
                }
            };

            match step {
                RawStep::Background(image) => check_asset(image),
                RawStep::Show { image, .. } => check_asset(image),
                RawStep::Music(Some(track)) => check_asset(track),
                RawStep::JumpTo(target) => check_target(target),
                RawStep::Choices(items) => {
                    if items.is_empty() {
                        fail(&format!("{where_}: a Choices step must offer at least one option"));
                    }
                    for c in items {
                        check_target(&c.target);
                    }
                }
                RawStep::Music(None)
                | RawStep::Hide { .. }
                | RawStep::Say { .. }
                | RawStep::Narrate(_)
                | RawStep::End => {}
            }
        }
        let _ = stem;
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
                match step {
                    RawStep::JumpTo(t) => queue.push(t.clone()),
                    RawStep::Choices(items) => {
                        queue.extend(items.iter().map(|c| c.target.clone()))
                    }
                    _ => {}
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
