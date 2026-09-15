//! Persistent knowledge about what the player has already experienced.
//!
//! This is deliberately separate from save slots. Loading an older snapshot
//! may rewind the story, but it must not make the player forget lines and
//! decisions they have genuinely seen.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::save::{SaveSlots, Toast, write_atomic};
use crate::story::{Frame, GameScript, StoryState};

pub const PROFILE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeenLine {
    pub scene: String,
    pub step: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeenChoice {
    pub scene: String,
    pub step: usize,
    pub option: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeenPlaceOption {
    pub place: String,
    pub option: usize,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerProfile {
    pub seen_lines: BTreeSet<SeenLine>,
    pub seen_choices: BTreeSet<SeenChoice>,
    pub seen_place_options: BTreeSet<SeenPlaceOption>,
}

impl PlayerProfile {
    pub fn has_choice(&self, scene: &str, step: usize, option: usize) -> bool {
        self.seen_choices.contains(&SeenChoice {
            scene: scene.to_string(),
            step,
            option,
        })
    }

    pub fn has_place_option(&self, place: &str, option: usize) -> bool {
        self.seen_place_options.contains(&SeenPlaceOption {
            place: place.to_string(),
            option,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ProfileFile {
    version: u32,
    profile: PlayerProfile,
}

#[derive(Resource, Default)]
pub(crate) struct ProfileDirty(pub bool);

pub fn profile_path(slots: &SaveSlots) -> PathBuf {
    slots.root.join("profile.ron")
}

pub fn write_profile(slots: &SaveSlots, profile: &PlayerProfile) -> Result<(), String> {
    let file = ProfileFile {
        version: PROFILE_VERSION,
        profile: profile.clone(),
    };
    let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("cannot serialise player profile: {e}"))?;
    write_atomic(&profile_path(slots), &text)
}

pub fn read_profile(slots: &SaveSlots) -> Result<PlayerProfile, String> {
    let path = profile_path(slots);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file: ProfileFile = ron::from_str(&text)
        .map_err(|e| format!("{} is not a valid player profile: {e}", path.display()))?;
    if file.version != PROFILE_VERSION {
        return Err(format!(
            "player profile is version {} and this build reads version {PROFILE_VERSION}",
            file.version
        ));
    }
    Ok(file.profile)
}

pub(crate) fn load_profile(slots: Res<SaveSlots>, mut profile: ResMut<PlayerProfile>) {
    if !profile_path(&slots).is_file() {
        return;
    }
    match read_profile(&slots) {
        Ok(loaded) => *profile = loaded,
        Err(e) => error!("{e}"),
    }
}

pub(crate) fn persist_profile(
    slots: Res<SaveSlots>,
    profile: Res<PlayerProfile>,
    mut dirty: ResMut<ProfileDirty>,
    mut toast: ResMut<Toast>,
) {
    if !dirty.0 {
        return;
    }
    match write_profile(&slots, &profile) {
        Ok(()) => dirty.0 = false,
        Err(e) => {
            // Keep it dirty: a transient full disk or permission failure must
            // not make this process forget that the profile still needs saving.
            error!("{e}");
            toast.show(format!("profile save failed: {e}"));
        }
    }
}

/// Records the line installed by the story engine this frame. At that point
/// the story cursor has advanced one step, so the stable id is `step - 1`.
pub(crate) fn record_displayed_line<S: GameScript>(
    state: Res<StoryState<S>>,
    line: Res<crate::story::CurrentLine>,
    mut profile: ResMut<PlayerProfile>,
    mut dirty: ResMut<ProfileDirty>,
) {
    if !line.is_changed() || line.full.is_empty() {
        return;
    }
    let Some(Frame::Story { scene, step }) = state.frames().last() else {
        return;
    };
    let Some(step) = step.checked_sub(1) else {
        return;
    };
    if profile.seen_lines.insert(SeenLine {
        scene: scene.name().to_string(),
        step,
    }) {
        dirty.0 = true;
    }
}
