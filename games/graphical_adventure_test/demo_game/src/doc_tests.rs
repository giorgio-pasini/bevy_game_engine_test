//! Checks that the documentation still describes the code.
//!
//! Two audits in one session found seven places where the docs and the project
//! had drifted — a stale roadmap, files that had moved, a data-model sketch
//! describing an API that no longer existed. Drift is constant in a research
//! repo, so the discipline needs a machine behind it.
//!
//! These are cheap structural checks, not comprehension: they cannot tell you a
//! document is *wrong about why*, only that it names something which no longer
//! exists. That is still most of the drift.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // demo_game -> graphical_adventure_test -> games -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repo root")
        .to_path_buf()
}

/// Every document that is expected to describe reality.
fn documents() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = vec![
        root.join("CLAUDE.md"),
        root.join("games/snake/CLAUDE.md"),
        root.join("games/graphical_adventure_test/DESIGN.md"),
        root.join("games/graphical_adventure_test/BACKLOG.md"),
        // A review is worth checking like any other document: it cites more
        // paths than most, and a finding that names a file which has since moved
        // is a finding nobody can act on.
        root.join("games/graphical_adventure_test/ENGINEERING_REVIEW.md"),
    ];
    if let Ok(entries) = std::fs::read_dir(root.join("bevytest_documentation")) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
    out.retain(|p| p.is_file());
    assert!(out.len() >= 4, "expected to find the documentation set, got {out:?}");
    out
}

/// Text inside single backticks. Good enough: fenced blocks contain no lone
/// backticks, so their contents are simply not sampled.
fn backticked(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// --------------------------------------------------------------- paths ----

/// Paths a document may legitimately mention without them existing: runtime
/// artefacts, generated output, and the reference clone that is not in git.
const PATH_EXCEPTIONS: &[&str] = &[
    "saves/",
    "settings.ron",
    "profile.ron",
    "save_<n>.ron",
    "imported_assets/",
    "target/",
    "target-shared/",
    ".cargo/config.toml",
    "Cargo.lock",
    "assets/inn.webp",
    "backgrounds/inn.webp",
    "backgrounds/forest.png",
    "backgrounds/village.png",
    "scenes_generated.rs",
    "tests/design_sketches.rs",
];

fn looks_like_path(s: &str) -> bool {
    if s.contains('*') || s.contains(' ') || s.contains('<') {
        return false;
    }
    if !s.contains('/') && !s.contains('\\') {
        return false;
    }
    [".rs", ".ron", ".md", ".toml", ".png", ".jpg", ".webp", ".ogg", ".html"]
        .iter()
        .any(|ext| s.ends_with(ext))
}

#[test]
fn every_path_mentioned_in_docs_exists() {
    let root = repo_root();
    let mut missing: Vec<String> = Vec::new();

    for doc in documents() {
        let text = std::fs::read_to_string(&doc).unwrap();
        let dir = doc.parent().unwrap();

        for candidate in backticked(&text) {
            let cleaned = candidate.trim_start_matches("./").replace('\\', "/");
            if !looks_like_path(&cleaned) {
                continue;
            }
            if PATH_EXCEPTIONS.iter().any(|e| cleaned.ends_with(e)) {
                continue;
            }
            // Documents write paths relative to whichever base is obvious in
            // context — the document itself, the repo, the Bevy examples the
            // component map points at, or the game crate whose `assets/` they
            // describe. The check is "does this file exist", not "is it spelled
            // relative to the repo root".
            let bases = [
                dir.to_path_buf(),
                root.clone(),
                root.join("bevy/examples"),
                root.join("games"),
                root.join("games/graphical_adventure_test/demo_game"),
                root.join("games/graphical_adventure_test"),
            ];
            if !bases.iter().any(|b| b.join(&cleaned).exists()) {
                missing.push(format!(
                    "{}: `{cleaned}`",
                    doc.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "documentation mentions paths that do not exist:\n  {}",
        missing.join("\n  ")
    );
}

// ----------------------------------------------------------- identifiers --

/// Collects `Enum::Variant` pairs declared in a Rust source file.
fn declared_variants(src: &str) -> Vec<(String, BTreeSet<String>)> {
    let mut out = Vec::new();
    let mut current: Option<(String, BTreeSet<String>)> = None;
    let mut depth = 0usize;

    for line in src.lines() {
        let trimmed = line.trim();
        if current.is_none() {
            if let Some(rest) = trimmed
                .strip_prefix("pub enum ")
                .or_else(|| trimmed.strip_prefix("enum "))
            {
                let name = rest
                    .split(|c: char| c == '<' || c == '{' || c == ' ')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !name.is_empty() {
                    current = Some((name, BTreeSet::new()));
                    depth = 1;
                }
            }
            continue;
        }
        // Inside an enum: first identifier on a line is a variant.
        depth += trimmed.matches('{').count();
        depth -= trimmed.matches('}').count().min(depth);
        if depth == 0 {
            out.push(current.take().unwrap());
            continue;
        }
        if let Some((_, variants)) = current.as_mut() {
            let ident: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !ident.is_empty() && ident.chars().next().unwrap().is_ascii_uppercase() {
                variants.insert(ident);
            }
        }
    }
    out
}

#[test]
fn every_enum_variant_mentioned_in_docs_exists() {
    let root = repo_root();
    let mut declared: Vec<(String, BTreeSet<String>)> = Vec::new();
    for rel in [
        "games/graphical_adventure_test/vn_core/src/story.rs",
        "games/graphical_adventure_test/vn_core/src/place.rs",
    ] {
        let src = std::fs::read_to_string(root.join(rel)).unwrap();
        declared.extend(declared_variants(&src));
    }
    assert!(
        declared.iter().any(|(n, _)| n == "Step"),
        "failed to parse enums out of vn_core — the check would silently pass"
    );

    let mut wrong: Vec<String> = Vec::new();
    for doc in documents() {
        let text = std::fs::read_to_string(&doc).unwrap();
        for item in backticked(&text) {
            let Some((ty, variant)) = item.split_once("::") else {
                continue;
            };
            // Only our own enums; anything else is Bevy or illustrative.
            let Some((_, variants)) = declared.iter().find(|(n, _)| n == ty) else {
                continue;
            };
            let variant: String = variant
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if variant.is_empty() {
                continue;
            }
            if !variants.contains(&variant) {
                wrong.push(format!(
                    "{}: `{ty}::{variant}` — {ty} has {:?}",
                    doc.file_name().unwrap().to_string_lossy(),
                    variants.iter().collect::<Vec<_>>()
                ));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "documentation names enum variants that no longer exist:\n  {}",
        wrong.join("\n  ")
    );
}
