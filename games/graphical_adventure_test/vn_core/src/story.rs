//! The story model and its runtime.
//!
//! This module owns *what is true* about the story. It never touches sprites,
//! text nodes or audio players — it updates [`Stage`] and [`CurrentLine`], and
//! the presentation systems reconcile the screen toward those. See `DESIGN.md`
//! Part 2.1.

use bevy::prelude::*;
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

/// Indirection between a line and its localised prose. Deliberately *not*
/// verified at compile time — a validation test covers it instead.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct TextKey(pub &'static str);

/// Where a character stands. Named positions, never raw pixels.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Slot {
    Left,
    Centre,
    Right,
}

#[derive(Clone)]
pub struct Choice<S> {
    pub text: TextKey,
    pub target: S,
}

#[derive(Clone)]
pub enum Step<S> {
    /// Replace the background image.
    Background { image: &'static str },
    /// Place a character sprite at a slot.
    Show { image: &'static str, slot: Slot },
    /// Clear a slot.
    Hide { slot: Slot },
    /// `None` stops the music.
    Music { track: Option<&'static str> },
    /// A character speaks. Blocking.
    Say { speaker: &'static str, text: TextKey },
    /// Unattributed prose. Blocking.
    Narrate { text: TextKey },
    /// Offer a branch. Blocking.
    Choices(Vec<Choice<S>>),
    JumpTo(S),
    End,
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
#[derive(Resource, Default)]
pub struct Stage {
    pub background: Option<&'static str>,
    pub characters: BTreeMap<Slot, &'static str>,
    pub music: Option<&'static str>,
}

/// The line currently on screen, plus how much of it has been revealed.
#[derive(Resource, Default)]
pub struct CurrentLine {
    pub speaker: Option<&'static str>,
    pub full: &'static str,
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
    pub items: Vec<(TextKey, usize)>,
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
    locale: &Locale,
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
                    full: (locale.resolve)(text),
                    revealed: 0,
                    timer: 0.0,
                };
                state.step += 1;
                return PlayMode::Revealing;
            }
            Step::Narrate { text } => {
                *line = CurrentLine {
                    speaker: None,
                    full: (locale.resolve)(text),
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
                    .map(|(i, c)| (c.text, i))
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
