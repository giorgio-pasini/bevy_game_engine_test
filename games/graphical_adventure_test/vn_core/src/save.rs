//! Save and load.
//!
//! The file holds a **snapshot of story state and nothing else**: the control
//! stack, the flags, the inventory. No sprite positions, no reveal counts, no
//! entity ids. The screen is rebuilt by replaying the stack, which is only
//! sound because presentation is a pure function of state — this is where that
//! discipline is finally cashed in.
//!
//! Two rules govern what may go in here, and both are easy to break later:
//!
//! * **Nothing derived.** `StoryState::current` is a cached scene, so it is not
//!   saved; it is recomputed from the top story frame on load. Saving it would
//!   let a file disagree with itself.
//! * **Nothing recomputed.** Replay reconstructs the *stage* only. If it ever
//!   re-ran `Set` or `Add`, a counter would advance again on every load and the
//!   corruption would compound each time the player reloaded.
//!
//! Scenes are stored by **name**, not as the generated enum. A save naming
//! `"the_offer"` survives scenes being added, removed or reordered, reads
//! plainly in a text editor, and fails loudly rather than silently landing on
//! the wrong scene if that name ever disappears.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::place::SceneLookup;
use crate::story::*;

/// Bumped when the format changes incompatibly. There is deliberately **no
/// migration machinery**: it would be speculation until real players have real
/// saves. This field is what makes adding it possible later — and what makes an
/// old file fail with a sentence rather than a parse error.
pub const SAVE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SavedFrame {
    Story { scene: String, step: usize },
    Explore { place: String },
}

/// Everything the game needs to be put back exactly where it was.
///
/// `BTreeMap`/`BTreeSet` rather than the hash variants so serialisation order is
/// deterministic — which is what lets a save→load→save round-trip be compared
/// byte for byte.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct SaveState {
    pub stack: Vec<SavedFrame>,
    pub flags: BTreeMap<String, i32>,
    pub items: BTreeSet<String>,
    /// The playing track.
    ///
    /// The one piece of `Stage` that belongs in a save, and it follows directly
    /// from music being **ambient rather than re-established**: no replay of the
    /// stack can work out which earlier scene started it. The general rule is
    /// sharper than "never save presentation" — it is *save exactly what cannot
    /// be derived*, and for this engine that is one field.
    pub music: Option<String>,
}

/// What lands on disk: the state, plus metadata for a slot list.
///
/// Metadata is kept in its own field precisely so a round-trip test can compare
/// `state` alone. A timestamp legitimately differs between two saves; the game
/// state must not.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SaveFile {
    pub version: u32,
    /// Seconds since the Unix epoch. Enough to sort slots by; not a date format.
    pub saved_at: u64,
    /// Where the player was, for a slot list to show. Derived, never read back.
    pub location: String,
    pub state: SaveState,
}

/// Where slot files live. A resource, so the **game** decides and the library
/// does not — and so a test can point it at a temp directory instead of writing
/// into the player's real save folder.
#[derive(Resource, Clone, Debug)]
pub struct SaveSlots {
    pub root: PathBuf,
}

impl Default for SaveSlots {
    /// A generic per-user folder. Games should override this with
    /// [`SaveSlots::for_game`] so their saves are not pooled with everyone
    /// else's.
    fn default() -> Self {
        Self::for_game("vn_core")
    }
}

impl SaveSlots {
    /// The conventional per-user save directory for `game`.
    ///
    /// **Not** beside the executable, which is the tempting answer and wrong in
    /// a release: the install directory is not user-writable on Windows, a
    /// machine may have several users, and cloud sync expects a stable path
    /// outside the install. Not the working directory either — that is wherever
    /// the player happened to launch from.
    ///
    /// | Platform | Location |
    /// | --- | --- |
    /// | Windows | `%APPDATA%\<game>\saves` |
    /// | macOS | `~/Library/Application Support/<game>/saves` |
    /// | Linux | `$XDG_DATA_HOME/<game>/saves`, else `~/.local/share/<game>/saves` |
    ///
    /// Resolved from environment variables rather than by adding the
    /// `directories` crate: that crate pulls `windows-sys`, which Bevy already
    /// uses through winit, and a normal dependency sharing a crate with the
    /// engine is how a feature-unification cascade — and a twenty-minute
    /// rebuild — starts. This is fifteen lines and touches nothing.
    pub fn for_game(game: &str) -> Self {
        Self {
            root: Self::data_dir().join(game).join("saves"),
        }
    }

    #[cfg(target_os = "windows")]
    fn data_dir() -> PathBuf {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[cfg(target_os = "macos")]
    fn data_dir() -> PathBuf {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support"))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn data_dir() -> PathBuf {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn path(&self, slot: u8) -> PathBuf {
        self.root.join(format!("save_{slot}.ron"))
    }

    pub fn exists(&self, slot: u8) -> bool {
        self.path(slot).is_file()
    }
}

/// A load the player has asked for: slot number in, file contents out.
///
/// Two hops rather than one, and deliberately: both the menu button and the
/// keyboard write a *request*, one system turns a request into state, and one
/// system installs it. The same shape as [`crate::place::PickedOption`] — inputs
/// name what they want, one place does it.
#[derive(Resource, Default)]
pub struct LoadRequest(pub Option<u8>);

/// A save file read from disk and waiting to be installed.
#[derive(Resource, Default)]
pub struct PendingLoad(pub Option<SaveState>);

/// A short message to the player: "saved", or why not.
///
/// Without feedback there is no way to tell a keypress worked, and a save that
/// silently failed is worse than one that never happened.
#[derive(Resource, Default)]
pub struct Toast {
    pub text: String,
    pub seconds_left: f32,
}

impl Toast {
    pub fn show(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.seconds_left = 2.0;
    }
}

/// Story state to snapshot.
pub fn capture<S: SceneKey>(
    state: &StoryState<S>,
    music: Option<String>,
    name_of: fn(S) -> &'static str,
) -> SaveState {
    SaveState {
        music,
        stack: state
            .stack
            .iter()
            .map(|frame| match frame {
                Frame::Story { scene, step } => SavedFrame::Story {
                    scene: name_of(*scene).to_string(),
                    step: *step,
                },
                Frame::Explore { place } => SavedFrame::Explore {
                    place: place.clone(),
                },
            })
            .collect(),
        flags: state.flags.clone(),
        items: state.items.clone(),
    }
}

/// Snapshot back to story state.
///
/// Fails rather than guesses. A name that no longer exists means the content
/// changed under the save, and quietly dropping the frame would resume the
/// player somewhere they have never been.
pub fn restore<S: SceneKey>(
    saved: &SaveState,
    script: &Script<S>,
    of_name: fn(&str) -> Option<S>,
) -> Result<StoryState<S>, String> {
    let mut stack = Vec::with_capacity(saved.stack.len());
    for frame in &saved.stack {
        stack.push(match frame {
            SavedFrame::Story { scene, step } => {
                let id = of_name(scene)
                    .ok_or_else(|| format!("this save names scene `{scene}`, which no longer exists"))?;
                Frame::Story {
                    scene: id,
                    step: *step,
                }
            }
            SavedFrame::Explore { place } => Frame::Explore {
                place: place.clone(),
            },
        });
    }

    // `current` is derived, which is exactly why it is not in the file: the
    // topmost story frame determines it, and a save can never disagree with
    // itself about what scene is running.
    let top_story = stack.iter().rev().find_map(|f| match f {
        Frame::Story { scene, .. } => Some(*scene),
        Frame::Explore { .. } => None,
    });
    let Some(scene) = top_story else {
        return Err("this save has no story frame at all — it is corrupt".to_string());
    };

    Ok(StoryState {
        stack,
        current: (script.scene_of)(scene),
        flags: saved.flags.clone(),
        items: saved.items.clone(),
    })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn write_slot(slots: &SaveSlots, slot: u8, state: SaveState, location: String) -> Result<(), String> {
    let file = SaveFile {
        version: SAVE_VERSION,
        saved_at: now_secs(),
        location,
        state,
    };
    let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("cannot serialise save: {e}"))?;
    std::fs::create_dir_all(&slots.root)
        .map_err(|e| format!("cannot create {}: {e}", slots.root.display()))?;

    // Write beside the slot, then rename over it. Writing in place means a
    // crash, a full disk or a lost battery mid-write leaves a truncated file —
    // and the file it truncates is the player's only copy. Rename is atomic on
    // both Windows and POSIX, so the slot is either the old save or the new
    // one, never half of either.
    let path = slots.path(slot);
    let staging = path.with_extension("ron.tmp");
    std::fs::write(&staging, text)
        .map_err(|e| format!("cannot write {}: {e}", staging.display()))?;
    std::fs::rename(&staging, &path).map_err(|e| {
        // Leave no litter behind if the rename is what failed.
        let _ = std::fs::remove_file(&staging);
        format!("cannot replace {}: {e}", path.display())
    })
}

pub fn read_slot(slots: &SaveSlots, slot: u8) -> Result<SaveFile, String> {
    let path = slots.path(slot);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file: SaveFile =
        ron::from_str(&text).map_err(|e| format!("{} is not a valid save: {e}", path.display()))?;
    if file.version != SAVE_VERSION {
        return Err(format!(
            "save is version {} and this build reads version {SAVE_VERSION}",
            file.version
        ));
    }
    Ok(file)
}

/// F5 saves to slot 1.
#[allow(clippy::too_many_arguments)]
pub fn save_on_key<S: SceneKey>(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppState>>,
    state: Res<StoryState<S>>,
    stage: Res<Stage>,
    lookup: Res<SceneLookup<S>>,
    slots: Res<SaveSlots>,
    mut toast: ResMut<Toast>,
) {
    if !keys.just_pressed(KeyCode::F5) {
        return;
    }
    // Saving from the menu would capture a run that has not started.
    if *app_state.get() == AppState::MainMenu {
        return;
    }
    let location = match state.stack.last() {
        Some(Frame::Explore { place }) => place.clone(),
        Some(Frame::Story { scene, .. }) => (lookup.name_of)(*scene).to_string(),
        None => "nowhere".to_string(),
    };
    let snapshot = capture(&state, stage.music.clone(), lookup.name_of);
    match write_slot(&slots, 1, snapshot, location) {
        Ok(()) => toast.show("saved"),
        Err(e) => {
            error!("{e}");
            toast.show(format!("save failed: {e}"));
        }
    }
}

/// F9 asks for slot 1. Reading it is somebody else's job.
pub fn request_load_on_key(keys: Res<ButtonInput<KeyCode>>, mut request: ResMut<LoadRequest>) {
    if keys.just_pressed(KeyCode::F9) {
        request.0 = Some(1);
    }
}

/// Turns a request into state, or into a message saying why not.
pub fn read_requested_save(
    mut request: ResMut<LoadRequest>,
    slots: Res<SaveSlots>,
    mut pending: ResMut<PendingLoad>,
    mut toast: ResMut<Toast>,
) {
    let Some(slot) = request.0.take() else {
        return;
    };
    match read_slot(&slots, slot) {
        Ok(file) => pending.0 = Some(file.state),
        Err(e) => {
            error!("{e}");
            toast.show(format!("load failed: {e}"));
        }
    }
}

pub fn tick_toast(time: Res<Time>, mut toast: ResMut<Toast>) {
    if toast.seconds_left > 0.0 {
        toast.seconds_left -= time.delta_secs();
        if toast.seconds_left <= 0.0 {
            toast.text.clear();
        }
    }
}
