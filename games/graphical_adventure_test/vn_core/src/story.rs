//! The story model and its runtime.
//!
//! This module owns *what is true* about the story. It never touches sprites,
//! text nodes or audio players — it updates [`Stage`] and [`CurrentLine`], and
//! the presentation systems reconcile the screen toward those. See `DESIGN.md`
//! Part 2.1.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Blanket trait for a game's scene enum. Being an enum is what makes jumps
/// compile-checked: `Step::JumpTo(SceneId::Forest)` cannot name a scene that
/// does not exist.
pub trait SceneKey:
    Copy + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static
{
}

impl<T> SceneKey for T where
    T: Copy + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static
{
}

/// Indirection between a line and its localised prose.
///
/// Retained for menu chrome and for the eventual localisation pipeline. Scene
/// prose is authored inline instead — see [`Step::Say`] — because splitting it
/// out meant editing two files to add one line of dialogue.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct TextKey(pub &'static str);

/// Where a character stands. Named positions, never raw pixels.
///
/// Stays a Rust enum so RON still type-checks it at parse time: `Centre` is
/// validated, `Centrre` is a parse error. External data does not have to mean
/// stringly-typed.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize)]
pub enum Slot {
    Left,
    Centre,
    Right,
}

#[derive(Clone)]
pub struct Choice<S> {
    pub text: String,
    pub target: S,
}

/// A choice as written in a scene file, before its target is resolved to a
/// scene id.
#[derive(Clone, Deserialize)]
pub struct RawChoice {
    pub text: String,
    pub target: String,
}

#[derive(Clone)]
pub enum Step<S> {
    /// Replace the background image.
    Background { image: String },
    /// Place a character sprite at a slot.
    Show { image: String, slot: Slot },
    /// Clear a slot.
    Hide { slot: Slot },
    /// `None` stops the music.
    Music { track: Option<String> },
    /// A character speaks. Blocking.
    Say { speaker: String, text: String },
    /// Unattributed prose. Blocking.
    Narrate { text: String },
    /// Offer a branch. Blocking.
    Choices(Vec<Choice<S>>),
    JumpTo(S),
    End,
}

/// A step exactly as written in a `.ron` scene file.
///
/// Targets are still strings here. [`Scene::resolve`] turns them into scene ids,
/// and `build.rs` has already proven every one of them resolves — so the
/// conversion cannot fail in a shipped build.
#[derive(Clone, Deserialize)]
pub enum RawStep {
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

/// A scene file's contents, before target resolution.
#[derive(Clone, Deserialize)]
pub struct RawScene {
    pub steps: Vec<RawStep>,
}

impl RawScene {
    /// Converts to a runnable scene, mapping target names through `lookup`.
    ///
    /// `lookup` returns `None` for an unknown name. That is unreachable in a
    /// build that compiled, because `build.rs` validates every target — the
    /// error path exists only so a hand-edited file fails loudly instead of
    /// silently misbehaving.
    pub fn resolve<S: SceneKey>(
        self,
        id: S,
        lookup: impl Fn(&str) -> Option<S>,
    ) -> Result<Scene<S>, String> {
        let mut steps = Vec::with_capacity(self.steps.len());
        for raw in self.steps {
            steps.push(match raw {
                RawStep::Background(image) => Step::Background { image },
                RawStep::Show { image, slot } => Step::Show { image, slot },
                RawStep::Hide { slot } => Step::Hide { slot },
                RawStep::Music(track) => Step::Music { track },
                RawStep::Say { speaker, text } => Step::Say { speaker, text },
                RawStep::Narrate(text) => Step::Narrate { text },
                RawStep::JumpTo(name) => Step::JumpTo(
                    lookup(&name).ok_or_else(|| format!("unknown scene `{name}`"))?,
                ),
                RawStep::Choices(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for c in items {
                        out.push(Choice {
                            text: c.text,
                            target: lookup(&c.target)
                                .ok_or_else(|| format!("unknown scene `{}`", c.target))?,
                        });
                    }
                    Step::Choices(out)
                }
                RawStep::End => Step::End,
            });
        }
        Ok(Scene { id, steps })
    }
}

// Steps split into two kinds: `Background`, `Show`, `Hide`, `Music` and
// `JumpTo` are applied immediately, while `Say`, `Narrate`, `Choices` and `End`
// stop and wait for the player. `advance` below batches the former until it
// reaches one of the latter, which is what lets a scene establish its
// background, cast and music before the first line appears.

#[derive(Clone)]
pub struct Scene<S> {
    pub id: S,
    pub steps: Vec<Step<S>>,
}

/// The game supplies its scene lookup and starting point.
#[derive(Resource)]
pub struct Script<S: SceneKey> {
    /// Implemented in the game as an exhaustive `match`, so adding a scene
    /// variant without writing its content fails to compile.
    pub scene_of: fn(S) -> Scene<S>,
    pub start: S,
}

/// Resolves a text key for the active language.
#[derive(Resource)]
pub struct Locale {
    pub resolve: fn(TextKey) -> &'static str,
}

/// Everything that would go into a save file. Presentation is absent on
/// purpose — it is derived, never stored.
#[derive(Resource)]
pub struct StoryState<S: SceneKey> {
    pub scene: S,
    pub step: usize,
    pub current: Scene<S>,
}

/// Derived presentation state. Rebuilt from the story, never authored directly.
#[derive(Resource, Default, PartialEq, Eq, Clone)]
pub struct Stage {
    pub background: Option<String>,
    pub characters: BTreeMap<Slot, String>,
    pub music: Option<String>,
}

/// The line currently on screen, plus how much of it has been revealed.
#[derive(Resource, Default)]
pub struct CurrentLine {
    pub speaker: Option<String>,
    pub full: String,
    pub revealed: usize,
    pub timer: f32,
}

impl CurrentLine {
    pub fn is_complete(&self) -> bool {
        self.revealed >= self.full.chars().count()
    }

    pub fn complete(&mut self) {
        self.revealed = self.full.chars().count();
    }

    /// The portion to draw. Counts by `char`, not by byte — indexing a `String`
    /// by byte would panic mid-multibyte-character.
    pub fn visible(&self) -> String {
        self.full.chars().take(self.revealed).collect()
    }
}

/// Choices awaiting an answer. Empty when not in a branch.
#[derive(Resource, Default)]
pub struct PendingChoices {
    pub items: Vec<(String, usize)>,
}

/// Top-level screen. The story only runs inside `Playing`.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    MainMenu,
    Playing,
}

/// In-story mode. A *sub*-state of `Playing`, so it simply does not exist while
/// the menu is up — which makes it impossible for story systems to run there.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::Playing)]
pub enum PlayMode {
    /// Typewriter still running.
    #[default]
    Revealing,
    /// Line fully shown; waiting for the player to advance.
    AwaitingInput,
    AwaitingChoice,
    Finished,
}

/// Returns the story to its opening state.
///
/// Only story state is reset — `Stage` is cleared so the reconcilers tear the
/// screen down on their own. That is the purity rule paying off: there is no
/// "unload the background, stop the music, despawn the cast" teardown to write
/// or to forget.
pub fn reset<S: SceneKey>(
    state: &mut StoryState<S>,
    script: &Script<S>,
    stage: &mut Stage,
    line: &mut CurrentLine,
    choices: &mut PendingChoices,
) {
    state.scene = script.start;
    state.current = (script.scene_of)(script.start);
    state.step = 0;
    *stage = Stage::default();
    *line = CurrentLine::default();
    choices.items.clear();
}

/// Characters revealed per second.
pub const TEXT_SPEED: f32 = 45.0;

/// Applies steps from the current position until a blocking one is reached.
///
/// This is the whole story engine. Everything else in the crate reacts to the
/// `Stage` and `CurrentLine` it leaves behind.
pub fn advance<S: SceneKey>(
    state: &mut StoryState<S>,
    script: &Script<S>,
    stage: &mut Stage,
    line: &mut CurrentLine,
    choices: &mut PendingChoices,
) -> PlayMode {
    loop {
        let Some(step) = state.current.steps.get(state.step).cloned() else {
            return PlayMode::Finished;
        };

        match step {
            Step::Background { image } => {
                stage.background = Some(image);
                state.step += 1;
            }
            Step::Show { image, slot } => {
                stage.characters.insert(slot, image);
                state.step += 1;
            }
            Step::Hide { slot } => {
                stage.characters.remove(&slot);
                state.step += 1;
            }
            Step::Music { track } => {
                stage.music = track;
                state.step += 1;
            }
            Step::JumpTo(target) => {
                state.scene = target;
                state.current = (script.scene_of)(target);
                state.step = 0;
            }
            Step::Say { speaker, text } => {
                *line = CurrentLine {
                    speaker: Some(speaker),
                    full: text,
                    revealed: 0,
                    timer: 0.0,
                };
                state.step += 1;
                return PlayMode::Revealing;
            }
            Step::Narrate { text } => {
                *line = CurrentLine {
                    speaker: None,
                    full: text,
                    revealed: 0,
                    timer: 0.0,
                };
                state.step += 1;
                return PlayMode::Revealing;
            }
            Step::Choices(items) => {
                choices.items = items
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c.text.clone(), i))
                    .collect();
                // Deliberately does NOT advance `step`: the branch is resolved
                // by the player's answer, which replaces the scene outright.
                return PlayMode::AwaitingChoice;
            }
            Step::End => return PlayMode::Finished,
        }
    }
}

/// Applies the player's answer to a branch.
pub fn take_choice<S: SceneKey>(
    index: usize,
    state: &mut StoryState<S>,
    script: &Script<S>,
) -> bool {
    let Some(Step::Choices(items)) = state.current.steps.get(state.step).cloned() else {
        return false;
    };
    let Some(choice) = items.get(index) else {
        return false;
    };
    state.scene = choice.target;
    state.current = (script.scene_of)(choice.target);
    state.step = 0;
    true
}
