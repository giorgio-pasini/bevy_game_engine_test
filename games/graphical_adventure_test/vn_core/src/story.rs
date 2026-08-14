//! The story model and its runtime.
//!
//! This module owns *what is true* about the story. It never touches sprites,
//! text nodes or audio players — it updates [`Stage`] and [`CurrentLine`], and
//! the presentation systems reconcile the screen toward those. See `DESIGN.md`
//! Part 2.1.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::place::{Effect, Places};

/// A game's scene enum, and everything the engine needs to ask about it.
///
/// Being an enum is what makes jumps compile-checked: `Step::JumpTo(
/// SceneId::Forest)` cannot name a scene that does not exist.
///
/// This began as three function pointers — `scene_of` on a `Script` resource,
/// `of_name` and `name_of` on a `SceneLookup` resource — and became a trait
/// because the pointers cost more to read than they saved. A function pointer
/// is a *value*, so "go to definition" lands on the field rather than on the
/// function that runs; and `Script` had to be threaded through seven systems
/// and eight functions purely so that something could eventually call it. See
/// `../../ENGINEERING_REVIEW.md`, finding C3.
pub trait GameScript:
    Copy + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static
{
    /// The scene's content. Implemented in the game as an exhaustive `match`,
    /// so adding a variant without writing its content fails to compile.
    fn scene(self) -> Scene<Self>;

    /// The name this scene is known by in data files and save files.
    ///
    /// Paired with [`GameScript::from_name`], and they must agree: saves are
    /// written with one and read with the other.
    fn name(self) -> &'static str;

    /// Resolves a name written in a place file or a save. `None` for a name
    /// that no longer exists, which callers must treat as an error rather than
    /// silently skip.
    fn from_name(name: &str) -> Option<Self>;
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
    /// `None` means always offered.
    pub condition: Option<Condition>,
}

/// A choice as written in a scene file, before its target is resolved to a
/// scene id.
#[derive(Clone, Deserialize)]
pub struct RawChoice {
    pub text: String,
    pub target: String,
    #[serde(default)]
    pub condition: Option<Condition>,
}

/// A member of the cast: who a character id refers to.
#[derive(Clone, Deserialize)]
pub struct CastMember {
    /// Shown in the dialogue box.
    pub name: String,
    pub image: String,
    /// Multiplies the global character height, so a giant and a person can
    /// share one setting. Defaults to 1.0.
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

/// Visual tuning, loaded from `presentation.ron`.
///
/// These are the numbers you actually want to fiddle with, kept out of Rust so
/// adjusting them is an edit and a rebuild rather than a code change. All
/// lengths are in design units (a 1920x1080 stage).
#[derive(Resource, Clone, Deserialize)]
pub struct Presentation {
    /// Character sprite height as a fraction of stage height.
    pub character_height: f32,
    /// Left/Right slot distance from centre, as a fraction of stage width.
    pub slot_offset: f32,
    /// How far above the stage bottom characters stand.
    pub character_baseline: f32,
    pub listener_dim: f32,
    pub speaker_rise: f32,
    pub speaker_scale: f32,
    pub cue_speed: f32,
    /// Characters revealed per second.
    pub text_speed: f32,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            character_height: 0.60,
            slot_offset: 0.24,
            character_baseline: 130.0,
            listener_dim: 0.45,
            speaker_rise: 26.0,
            speaker_scale: 1.04,
            cue_speed: 7.0,
            text_speed: 45.0,
        }
    }
}

/// Character id -> who they are. Loaded from `cast.ron`; `build.rs` guarantees
/// every id referenced by a scene is present here.
#[derive(Resource, Clone, Default, Deserialize)]
pub struct Cast(pub BTreeMap<String, CastMember>);

impl Cast {
    /// Falls back to the raw id when unknown — which `build.rs` prevents, but
    /// showing `golem` beats showing nothing if it ever happens.
    ///
    /// The explicit lifetime is required: the result borrows from either `self`
    /// or `id`, and elision would tie it to `self` alone.
    pub fn name_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.0.get(id).map(|m| m.name.as_str()).unwrap_or(id)
    }

    pub fn image_of(&self, id: &str) -> Option<&str> {
        self.0.get(id).map(|m| m.image.as_str())
    }
}

/// Guards on choices and jumps.
///
/// Deliberately a small closed set — flags and counters cover essentially every
/// visual novel, and an expression language is the classic over-engineering
/// trap. Extending it means adding a variant, which the compiler then demands
/// be handled everywhere.
#[derive(Clone, Deserialize, Debug)]
pub enum Condition {
    IsSet(String),
    IsNotSet(String),
    AtLeast(String, i32),
    /// Inventory. Items are deliberately modelled as richer flags rather than
    /// as a verb matrix — an action can require one, and that covers most
    /// adventure puzzles without per-item-per-target authoring.
    HasItem(String),
    LacksItem(String),
    All(Vec<Condition>),
    Any(Vec<Condition>),
}

impl Condition {
    pub fn holds(&self, flags: &BTreeMap<String, i32>, items: &BTreeSet<String>) -> bool {
        let get = |k: &String| flags.get(k).copied().unwrap_or(0);
        match self {
            Condition::IsSet(k) => get(k) != 0,
            Condition::IsNotSet(k) => get(k) == 0,
            Condition::AtLeast(k, n) => get(k) >= *n,
            Condition::HasItem(i) => items.contains(i),
            Condition::LacksItem(i) => !items.contains(i),
            Condition::All(cs) => cs.iter().all(|c| c.holds(flags, items)),
            Condition::Any(cs) => cs.iter().any(|c| c.holds(flags, items)),
        }
    }

    /// Every flag this condition reads. Used by `build.rs` to catch typos.
    pub fn flags_read(&self, out: &mut Vec<String>) {
        match self {
            Condition::IsSet(k) | Condition::IsNotSet(k) | Condition::AtLeast(k, _) => {
                out.push(k.clone())
            }
            Condition::HasItem(_) | Condition::LacksItem(_) => {}
            Condition::All(cs) | Condition::Any(cs) => {
                for c in cs {
                    c.flags_read(out);
                }
            }
        }
    }

    /// Every item this condition requires. Checked against items granted
    /// anywhere, the same way flags are.
    pub fn items_read(&self, out: &mut Vec<String>) {
        match self {
            Condition::HasItem(i) | Condition::LacksItem(i) => out.push(i.clone()),
            Condition::IsSet(_) | Condition::IsNotSet(_) | Condition::AtLeast(_, _) => {}
            Condition::All(cs) | Condition::Any(cs) => {
                for c in cs {
                    c.items_read(out);
                }
            }
        }
    }
}

/// Whether an optional guard permits something. `None` always permits.
///
/// One function rather than four copies of `is_none_or(|c| c.holds(..))`. The
/// copies were identical, which is exactly why they would not have stayed that
/// way: a filter that offers an option and a re-check that accepts the click
/// must agree, and agreement by inspection is not agreement.
pub(crate) fn allowed(
    condition: &Option<Condition>,
    flags: &BTreeMap<String, i32>,
    items: &BTreeSet<String>,
) -> bool {
    condition.as_ref().is_none_or(|c| c.holds(flags, items))
}

#[derive(Clone)]
pub enum Step<S> {
    /// Replace the background image.
    Background { image: String },
    /// Place a character on stage. `character` is a cast id, not a path.
    Show { character: String, slot: Slot },
    /// Clear a slot.
    Hide { slot: Slot },
    /// `None` stops the music.
    Music { track: Option<String> },
    /// A character speaks. Blocking. `speaker` is a cast id.
    Say { speaker: String, text: String },
    /// Unattributed prose. Blocking.
    Narrate { text: String },
    /// Offer a branch. Blocking.
    Choices(Vec<Choice<S>>),
    JumpTo(S),
    /// Jump only if the condition holds; otherwise fall through to the next step.
    JumpIf { condition: Condition, target: S },
    /// Set a flag to 1.
    Set { flag: String },
    /// Set a flag to 0.
    Clear { flag: String },
    /// Add to a counter.
    Add { flag: String, delta: i32 },
    GiveItem { item: String },
    TakeItem { item: String },
    /// Hand control to the exploration phase. Returns here when the player
    /// leaves that place.
    Explore { place: String },
    /// Finish this frame and return to whatever called it. With an empty stack
    /// beneath, the game ends.
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
    pub fn resolve<S: GameScript>(
        self,
        id: S,
        lookup: impl Fn(&str) -> Option<S>,
    ) -> Result<Scene<S>, String> {
        let mut steps = Vec::with_capacity(self.steps.len());
        for raw in self.steps {
            steps.push(match raw {
                RawStep::Background(image) => Step::Background { image },
                RawStep::Show { character, slot } => Step::Show { character, slot },
                RawStep::Hide { slot } => Step::Hide { slot },
                RawStep::Music(track) => Step::Music { track },
                RawStep::Say { speaker, text } => Step::Say { speaker, text },
                RawStep::Narrate(text) => Step::Narrate { text },
                RawStep::Set(flag) => Step::Set { flag },
                RawStep::Clear(flag) => Step::Clear { flag },
                RawStep::Add { flag, delta } => Step::Add { flag, delta },
                RawStep::GiveItem(item) => Step::GiveItem { item },
                RawStep::TakeItem(item) => Step::TakeItem { item },
                RawStep::Explore(place) => Step::Explore { place },
                RawStep::JumpTo(name) => Step::JumpTo(
                    lookup(&name).ok_or_else(|| format!("unknown scene `{name}`"))?,
                ),
                RawStep::JumpIf { condition, target } => Step::JumpIf {
                    condition,
                    target: lookup(&target)
                        .ok_or_else(|| format!("unknown scene `{target}`"))?,
                },
                RawStep::Choices(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for c in items {
                        out.push(Choice {
                            text: c.text,
                            target: lookup(&c.target)
                                .ok_or_else(|| format!("unknown scene `{}`", c.target))?,
                            condition: c.condition,
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

/// Where a new run begins.
///
/// Deliberately data rather than an associated constant on [`GameScript`]: a
/// game may want to choose it — chapter select, a debug entry point — and only
/// `reset` reads it, so it costs one system a parameter rather than seven.
#[derive(Resource)]
pub(crate) struct StartScene<S: GameScript>(pub S);

/// Resolves a text key for the active language.
#[derive(Resource)]
pub struct Locale {
    pub resolve: fn(TextKey) -> &'static str,
}

/// One level of the control stack.
///
/// Story and exploration call each other freely — Ren'Py's `call`/`return`
/// model — so "where we are" is a stack, not a cursor.
#[derive(Clone, Debug)]
pub enum Frame<S> {
    Story { scene: S, step: usize },
    Explore { place: String },
}

/// Everything that would go into a save file. Presentation is absent on
/// purpose — it is derived, never stored.
#[derive(Resource, Clone)]
pub struct StoryState<S: GameScript> {
    /// Top of the stack is the current context. Empty means the game is over.
    ///
    /// Private, together with `current`, because the two have an invariant: the
    /// cached scene must be the one the topmost story frame names. Every method
    /// below maintains it; a public field would let any caller break it
    /// silently, and the tests used to do exactly that.
    stack: Vec<Frame<S>>,
    /// Scene cached for the topmost `Story` frame, so steps are not re-resolved
    /// every tick. Derived from `stack` — see [`StoryState::from_frames`], which
    /// is the one place that computes it.
    current: Scene<S>,
    /// Booleans are stored as 0/1 so flags and counters share one map.
    pub flags: BTreeMap<String, i32>,
    /// Inventory.
    pub items: BTreeSet<String>,
}

impl<S: GameScript> StoryState<S> {
    /// A fresh run beginning at `start`.
    pub fn new(start: S) -> Self {
        Self {
            stack: vec![Frame::Story {
                scene: start,
                step: 0,
            }],
            current: start.scene(),
            flags: BTreeMap::new(),
            items: BTreeSet::new(),
        }
    }

    /// Rebuilds a state from frames — loading a save, or a test setting up a
    /// situation.
    ///
    /// **The only place the cached scene is derived**, which is why it is worth
    /// having: `restore` used to re-implement this, and a second implementation
    /// of an invariant is how invariants stop holding. Fails when no story frame
    /// is present, since there would be no scene to run.
    pub fn from_frames(
        stack: Vec<Frame<S>>,
        flags: BTreeMap<String, i32>,
        items: BTreeSet<String>,
    ) -> Option<Self> {
        let scene = stack.iter().rev().find_map(|f| match f {
            Frame::Story { scene, .. } => Some(*scene),
            Frame::Explore { .. } => None,
        })?;
        Some(Self {
            stack,
            current: scene.scene(),
            flags,
            items,
        })
    }

    /// The control stack, innermost last. Read-only: mutating it is what the
    /// methods below are for.
    pub fn frames(&self) -> &[Frame<S>] {
        &self.stack
    }

    /// What is running now, or `None` when the game is over.
    pub fn top(&self) -> Option<&Frame<S>> {
        self.stack.last()
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn is_over(&self) -> bool {
        self.stack.is_empty()
    }

    /// The scene the topmost story frame is running.
    pub fn scene(&self) -> &Scene<S> {
        &self.current
    }

    /// The step index of the topmost `Story` frame, if that is what we are in.
    pub fn story_step(&self) -> Option<usize> {
        match self.stack.last() {
            Some(Frame::Story { step, .. }) => Some(*step),
            _ => None,
        }
    }

    pub fn current_place(&self) -> Option<&str> {
        match self.stack.last() {
            Some(Frame::Explore { place }) => Some(place.as_str()),
            _ => None,
        }
    }

    fn set_story_step(&mut self, n: usize) {
        if let Some(Frame::Story { step, .. }) = self.stack.last_mut() {
            *step = n;
        }
    }

    /// Replaces the top `Story` frame's scene and rewinds it.
    fn enter_scene(&mut self, id: S) {
        self.current = id.scene();
        match self.stack.last_mut() {
            Some(Frame::Story { scene, step }) => {
                *scene = id;
                *step = 0;
            }
            _ => self.stack.push(Frame::Story {
                scene: id,
                step: 0,
            }),
        }
    }

    /// Pushes a scene, to be returned from later.
    pub fn call_scene(&mut self, id: S) {
        self.stack.push(Frame::Story {
            scene: id,
            step: 0,
        });
        self.current = id.scene();
    }

    pub fn push_place(&mut self, place: String) {
        self.stack.push(Frame::Explore { place });
    }

    /// Abandons every nested frame, leaving the run at its outermost story
    /// frame — and re-derives the cached scene, which is the whole reason this
    /// exists rather than callers truncating the stack themselves.
    ///
    /// A "quit to the outer story" would use it; the tests use it to simulate
    /// wandering off before loading a save.
    pub fn unwind_to_root(&mut self) {
        self.stack.truncate(1);
        if let Some(Frame::Story { scene, .. }) = self.stack.first() {
            self.current = scene.scene();
        }
    }

    /// Puts the cursor back onto the line that was on screen.
    ///
    /// [`advance`] steps *past* a `Say` or `Narrate` before returning, so a
    /// saved cursor points at the line *after* the one the player was reading.
    /// Restoring without this correction silently skips a line — and it is the
    /// reason a save→load→save round-trip comes back byte-identical rather than
    /// drifting one step forward every time.
    ///
    /// A `Choices` step deliberately does not advance the cursor: the player's
    /// answer resolves it. A save made at a branch therefore already points at
    /// the branch and needs no adjustment.
    pub fn rewind_to_current_line(&mut self) {
        let Some(here) = self.story_step() else {
            return; // exploring: nothing was being read
        };
        if here == 0 {
            return;
        }
        if matches!(
            self.current.steps.get(here - 1),
            Some(Step::Say { .. }) | Some(Step::Narrate { .. })
        ) {
            self.set_story_step(here - 1);
        }
    }

    /// Pops the current frame. Restores the cached scene if the frame beneath is
    /// a story frame. Returns false when the stack empties — the game is over.
    pub fn pop(&mut self) -> bool {
        self.stack.pop();
        match self.stack.last() {
            Some(Frame::Story { scene, .. }) => {
                self.current = scene.scene();
                true
            }
            Some(Frame::Explore { .. }) => true,
            None => false,
        }
    }
}

/// Derived presentation state. Rebuilt from the story, never authored directly.
#[derive(Resource, Default, PartialEq, Eq, Clone, Debug)]
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
    /// Narrative phase.
    Playing,
    /// Exploration phase. Story and exploration call each other freely; which
    /// one is active is simply whatever is on top of the control stack.
    Exploring,
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
pub(crate) fn reset<S: GameScript>(
    state: &mut StoryState<S>,
    start: S,
    stage: &mut Stage,
    line: &mut CurrentLine,
    choices: &mut PendingChoices,
) {
    // A single frame at the opening scene — the stack unwinds completely, so a
    // restart cannot inherit a half-finished exploration from the last run.
    *state = StoryState::new(start);
    *stage = Stage::default();
    *line = CurrentLine::default();
    choices.items.clear();
}

// Text speed now lives in `Presentation`, loaded from presentation.ron.

/// Applies steps from the current frame until control has to stop — a line to
/// read, a choice to make, a place to explore, or an empty stack.
///
/// This is the whole story engine. Everything else in the crate reacts to the
/// `Stage` and `CurrentLine` it leaves behind.
///
/// Where control ended up after advancing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow {
    /// Still in a scene, in the given mode.
    Playing(PlayMode),
    /// Top of the stack is now a place.
    Exploring,
    /// The stack emptied.
    Ended,
}

pub fn advance<S: GameScript>(
    state: &mut StoryState<S>,
    stage: &mut Stage,
    line: &mut CurrentLine,
    choices: &mut PendingChoices,
) -> Flow {
    loop {
        // Only story frames step. Anything else means control has moved.
        let Some(here) = state.story_step() else {
            return match state.stack.last() {
                Some(Frame::Explore { .. }) => Flow::Exploring,
                Some(Frame::Story { .. }) => unreachable!(),
                None => Flow::Ended,
            };
        };

        let Some(step) = state.current.steps.get(here).cloned() else {
            // Ran off the end of a scene: treat as `End`.
            if state.pop() {
                continue;
            }
            return Flow::Ended;
        };

        match step {
            Step::Background { image } => {
                stage.background = Some(image);
                state.set_story_step(here + 1);
            }
            Step::Show { character, slot } => {
                stage.characters.insert(slot, character);
                state.set_story_step(here + 1);
            }
            Step::Hide { slot } => {
                stage.characters.remove(&slot);
                state.set_story_step(here + 1);
            }
            Step::Music { track } => {
                stage.music = track;
                state.set_story_step(here + 1);
            }
            Step::Set { flag } => {
                state.flags.insert(flag, 1);
                state.set_story_step(here + 1);
            }
            Step::Clear { flag } => {
                state.flags.insert(flag, 0);
                state.set_story_step(here + 1);
            }
            Step::Add { flag, delta } => {
                *state.flags.entry(flag).or_insert(0) += delta;
                state.set_story_step(here + 1);
            }
            Step::GiveItem { item } => {
                state.items.insert(item);
                state.set_story_step(here + 1);
            }
            Step::TakeItem { item } => {
                state.items.remove(&item);
                state.set_story_step(here + 1);
            }
            Step::JumpIf { condition, target } => {
                if condition.holds(&state.flags, &state.items) {
                    state.enter_scene(target);
                } else {
                    state.set_story_step(here + 1);
                }
            }
            Step::JumpTo(target) => state.enter_scene(target),
            Step::Explore { place } => {
                // Step past the Explore before pushing, so returning here
                // resumes *after* it rather than looping straight back in.
                state.set_story_step(here + 1);
                state.push_place(place);
                return Flow::Exploring;
            }
            Step::Say { speaker, text } => {
                *line = CurrentLine {
                    speaker: Some(speaker),
                    full: text,
                    revealed: 0,
                    timer: 0.0,
                };
                state.set_story_step(here + 1);
                return Flow::Playing(PlayMode::Revealing);
            }
            Step::Narrate { text } => {
                *line = CurrentLine {
                    speaker: None,
                    full: text,
                    revealed: 0,
                    timer: 0.0,
                };
                state.set_story_step(here + 1);
                return Flow::Playing(PlayMode::Revealing);
            }
            Step::Choices(items) => {
                // Keep each option's ORIGINAL index: `take_choice` indexes into
                // the unfiltered list.
                choices.items = items
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| allowed(&c.condition, &state.flags, &state.items))
                    .map(|(i, c)| (c.text.clone(), i))
                    .collect();
                // Deliberately does not step: the branch is resolved by the
                // player's answer.
                return Flow::Playing(PlayMode::AwaitingChoice);
            }
            Step::End => {
                if !state.pop() {
                    return Flow::Ended;
                }
            }
        }
    }
}

/// Applies a place option: move, call a scene, or leave.
pub fn take_option<S: GameScript>(
    index: usize,
    state: &mut StoryState<S>,
    places: &Places,
) -> Flow {
    let Some(place_id) = state.current_place().map(str::to_string) else {
        return Flow::Exploring;
    };
    let Some(option) = places
        .get(&place_id)
        .and_then(|p| p.options.get(index))
        .cloned()
    else {
        return Flow::Exploring;
    };

    // Re-check the guard: a button can outlive the state that offered it.
    if !allowed(&option.condition, &state.flags, &state.items) {
        return Flow::Exploring;
    }

    match option.effect {
        Effect::GoTo(target) => {
            // Replaces the frame rather than pushing: walking between rooms is
            // not recursion, and pushing would make `Leave` unwind one room at
            // a time.
            if let Some(Frame::Explore { place }) = state.stack.last_mut() {
                *place = target;
            }
            Flow::Exploring
        }
        // `build.rs` has already proven every name here resolves, so the `None`
        // arm is unreachable in a built game — it exists so a hand-edited file
        // does nothing rather than something wrong.
        Effect::Call(scene_name) => match S::from_name(&scene_name) {
            Some(id) => {
                state.call_scene(id);
                Flow::Playing(PlayMode::Revealing)
            }
            None => Flow::Exploring,
        },
        Effect::Leave => {
            if state.pop() {
                match state.stack.last() {
                    Some(Frame::Explore { .. }) => Flow::Exploring,
                    _ => Flow::Playing(PlayMode::Revealing),
                }
            } else {
                Flow::Ended
            }
        }
    }
}

/// Applies the player's answer to a branch.
pub fn take_choice<S: GameScript>(index: usize, state: &mut StoryState<S>) -> bool {
    let Some(here) = state.story_step() else {
        return false;
    };
    let Some(Step::Choices(items)) = state.current.steps.get(here).cloned() else {
        return false;
    };
    let Some(choice) = items.get(index) else {
        return false;
    };
    // Re-check the guard: a button could otherwise outlive the state that made
    // its option available.
    if !allowed(&choice.condition, &state.flags, &state.items) {
        return false;
    }
    // A branch replaces the current scene rather than pushing: choosing is not
    // calling, so there is nothing to return to.
    state.enter_scene(choice.target);
    true
}
