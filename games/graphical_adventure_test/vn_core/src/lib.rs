//! `vn_core` — a visual-novel engine as Bevy plugins.
//!
//! See `../../DESIGN.md` for the architecture. The rule that governs the whole
//! crate: **presentation is a pure function of story state.** The story module
//! updates [`story::Stage`] and [`story::CurrentLine`]; the presentation systems
//! reconcile the screen toward them and decide nothing.

use bevy::prelude::*;

mod letterbox;
pub mod map;
pub mod place;
pub mod present;
pub mod profile;
pub mod save;
pub mod story;

#[cfg(test)]
mod tests;

pub mod prelude {
    pub use crate::map::{HoveredHotspot, MapLookup, sample_design_point};
    pub use crate::place::{Effect, Place, PlaceOption, Places, Rgb};
    pub use crate::present::{DESIGN_H, DESIGN_W};
    pub use crate::profile::{
        PROFILE_VERSION, PlayerProfile, SeenChoice, SeenLine, SeenPlaceOption, profile_path,
        read_profile, write_profile,
    };
    pub use crate::save::{
        LoadRequest, SAVE_VERSION, SaveFile, SaveSlots, SaveState, SavedFrame, capture,
        read_slot, restore, write_slot,
    };
    pub use crate::story::{
        AppState, Cast, CastMember, Choice, Condition, CurrentLine, Flow, Frame, GameScript,
        Locale, PendingChoices, PlayMode, Presentation, RawChoice, RawScene, RawStep,
        Scene as VnScene, Slot, Stage, Step, StoryState, TextKey, advance, take_choice,
        take_option,
    };
    pub use crate::{VnPlugin, VnSet};
}

use place::*;
use present::*;
use profile::*;
use save::*;
use story::*;

/// The shape of a frame, stated once.
///
/// Before this existed the engine registered three separate `Update` chains —
/// presentation, story, exploration — each internally ordered and with **no
/// order between them**. Bevy's ambiguity detector reported 42 pairs of our own
/// systems with indeterminate execution order, and one consequence was
/// architectural rather than cosmetic: the presentation chain was registered
/// first, so the reconcilers read `Stage` *before* the logic wrote it. Every
/// frame drew the previous frame's state.
///
/// Reading these five names in order is now the answer to "what happens in a
/// frame", and it restores the rule the whole crate is built on: state first,
/// screen after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VnSet {
    /// The session rather than the story: menu buttons, saving, loading.
    Session,
    /// The narrative phase. Runs only in [`AppState::Playing`], which is also
    /// what makes `PlayMode` — a sub-state — safe to read here.
    Story,
    /// The exploration phase. Runs only in [`AppState::Exploring`].
    Explore,
    /// Snapshotting, after the frame's decisions are made — so a save captures
    /// what the player just did, not what they had done a frame ago.
    Persist,
    /// Reconciling the screen toward whatever the sets above decided.
    Present,
}

/// Add this, supplying the game's [`Script`] and [`Locale`].
pub struct VnPlugin<S: GameScript> {
    /// Where a new run begins.
    pub start: S,
    pub locale: Locale,
    /// Character id -> display name and image. `build.rs` guarantees every id a
    /// scene references is present.
    pub cast: Cast,
    /// Visual tuning from `presentation.ron`.
    pub tuning: Presentation,
    /// Explorable places. `build.rs` guarantees every referenced id exists.
    pub places: Places,
}

impl<S: GameScript> Plugin for VnPlugin<S> {
    fn build(&self, app: &mut App) {
        let start = self.start;

        app.insert_resource(StartScene::<S>(start))
            .insert_resource(Locale {
            resolve: self.locale.resolve,
        })
        .insert_resource(self.cast.clone())
        .insert_resource(self.tuning.clone())
        .insert_resource(self.places.clone())
        .insert_resource(StoryState::<S>::new(start))
        .init_resource::<Stage>()
        .init_resource::<CurrentLine>()
        .init_resource::<PendingChoices>()
        .init_resource::<PickedOption>()
        .init_resource::<map::MapLookup>()
        .init_resource::<map::HoveredHotspot>()
        .init_resource::<SaveSlots>()
        .init_resource::<PlayerProfile>()
        .init_resource::<ProfileDirty>()
        .init_resource::<LoadRequest>()
        .init_resource::<PendingLoad>()
        .init_resource::<Toast>()
        .init_state::<AppState>()
        .add_sub_state::<PlayMode>()
        .add_systems(
            Startup,
            (letterbox::spawn_camera, spawn_toast, load_profile),
        )
        .add_systems(OnEnter(AppState::MainMenu), spawn_menu)
        // Starting a NEW game happens on leaving the menu — see `restart`.
        .add_systems(OnExit(AppState::MainMenu), restart::<S>)
        // Entering `Playing` also happens when returning from exploration, so
        // this only rebuilds the screen; it never touches story state.
        .add_systems(
            OnEnter(AppState::Playing),
            (rebuild_stage::<S>, spawn_ui).chain(),
        )
        // Clearing the stage on the way out makes the reconcilers below despawn
        // the background, cast and music. No bespoke teardown.
        .add_systems(OnExit(AppState::Playing), clear_visuals)
        .add_systems(OnEnter(AppState::MainMenu), stop_everything)
        .add_systems(OnExit(AppState::Exploring), clear_visuals)
        .add_systems(OnEnter(PlayMode::AwaitingChoice), spawn_choices::<S>)
        .add_systems(OnExit(PlayMode::AwaitingChoice), despawn_choices)
        .add_systems(OnEnter(PlayMode::Finished), spawn_end_screen)
        // One total order for the frame. `Story` and `Explore` are mutually
        // exclusive by run condition, so their relative order never matters —
        // but stating it costs nothing and leaves no pair undefined.
        //
        // The parallelism given up is nominal: nearly every system here touches
        // `Stage` or `StoryState`, so Bevy could not have run them concurrently
        // anyway. What is bought is a frame that reads in one direction.
        .configure_sets(
            Update,
            (
                VnSet::Session,
                VnSet::Story.run_if(in_state(AppState::Playing)),
                VnSet::Explore.run_if(in_state(AppState::Exploring)),
                VnSet::Persist,
                VnSet::Present,
            )
                .chain(),
        )
        // Session: menu buttons, then load ordered request -> read -> install.
        // `apply_pending_load` stands down in the menu so `restart` cannot
        // reset a run that is being loaded — see its own comment.
        .add_systems(
            Update,
            (
                handle_ui_buttons,
                request_load_on_key,
                read_requested_save,
                apply_pending_load::<S>.run_if(not(in_state(AppState::MainMenu))),
            )
                .chain()
                .in_set(VnSet::Session),
        )
        // Story. `reveal_text` and `draw_dialogue` read `PlayMode`, a sub-state
        // that does not exist outside `Playing` — the set's run condition is
        // what keeps reading it legal.
        .add_systems(
            Update,
            (
                advance_on_input::<S>,
                reveal_text,
                choose_on_click::<S>,
                record_displayed_line::<S>,
                draw_dialogue,
            )
                .chain()
                .in_set(VnSet::Story),
        )
        // Exploration, ordered read → decide → apply: the lookup image follows
        // the place, the cursor is resolved against it, the two input systems
        // say what was picked, and one system applies it. Running the applier
        // last is what lets a click be handled in the frame it happens.
        //
        // The view is rebuilt by `refresh_place_view` whenever the place
        // changes, since moving between rooms does not change `AppState`.
        .add_systems(
            Update,
            (
                refresh_place_view::<S>,
                map::sync_lookup::<S>,
                map::hover_hotspot::<S>,
                draw_hotspot_label::<S>,
                pick_place_button,
                map::click_hotspot,
                apply_picked_option::<S>,
            )
                .chain()
                .in_set(VnSet::Explore),
        )
        .add_systems(
            Update,
            (persist_profile, save_on_key::<S>)
                .chain()
                .in_set(VnSet::Persist),
        )
        // Presentation runs in every state, so leaving `Playing` actually tears
        // the screen down — and now runs *after* the logic, so what is drawn is
        // this frame's state rather than last frame's.
        .add_systems(
            Update,
            (
                letterbox::fit_letterbox,
                sync_background,
                sync_characters,
                // Must follow sync_characters: it sizes what that just spawned,
                // and speaker_cue then animates the sized sprites.
                fit_characters,
                speaker_cue,
                sync_music,
                highlight_choices,
                tick_toast,
                draw_toast,
            )
                .chain()
                .in_set(VnSet::Present),
        );
    }
}

/// Clears what is *seen* on a phase change, but deliberately leaves the music
/// playing.
///
/// A soundtrack is ambient: it belongs to the moment, not to the screen.
/// Clearing it too meant the track stopped dead every time the player opened a
/// room and started again on the way out. A place that wants different music
/// says so; otherwise the current track continues.
fn clear_visuals(mut stage: ResMut<Stage>) {
    stage.background = None;
    stage.characters.clear();
}

/// Returning to the menu *does* stop the music — that is a genuine reset.
fn stop_everything(mut stage: ResMut<Stage>) {
    *stage = Stage::default();
}

/// Routes the result of advancing into the state machine.
///
/// One place decides what `Flow` means for `AppState`/`PlayMode`, so the two
/// phases cannot drift apart as more call sites appear.
/// `current` matters: Bevy runs **identity transitions**, so setting the state
/// you are already in still fires `OnExit`/`OnEnter`. Setting `Playing` on every
/// advance therefore re-ran the enter systems on every click — which reset the
/// story to its first line and made the text flicker and vanish.
fn apply_flow(
    flow: Flow,
    current: &AppState,
    next_app: &mut NextState<AppState>,
    next_mode: &mut NextState<PlayMode>,
) {
    let go = |target: AppState, next_app: &mut NextState<AppState>| {
        if *current != target {
            next_app.set(target);
        }
    };
    match flow {
        Flow::Playing(mode) => {
            go(AppState::Playing, next_app);
            next_mode.set(mode);
        }
        Flow::Exploring => go(AppState::Exploring, next_app),
        Flow::Ended => {
            go(AppState::Playing, next_app);
            next_mode.set(PlayMode::Finished);
        }
    }
}

/// Rewinds to the opening and plays the first batch of steps.
///
/// Registered on `OnExit(MainMenu)`, **not** `OnEnter(Playing)`: `Playing` is
/// also entered when returning from exploration, and restarting there would
/// throw away the player's progress.
fn restart<S: GameScript>(
    pending: Res<PendingLoad>,
    mut state: ResMut<StoryState<S>>,
    start: Res<StartScene<S>>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_mode: ResMut<NextState<PlayMode>>,
) {
    // "Continue" leaves the menu exactly as "Begin" does, so this would wipe the
    // save on its way past. The load is installed a frame later, by
    // `apply_pending_load`, which cannot run until we are out of the menu.
    if pending.0.is_some() {
        return;
    }
    reset(&mut state, start.0, &mut stage, &mut line, &mut choices);
    let flow = advance(&mut state, &mut stage, &mut line, &mut choices);
    // We are already on our way to `Playing`, so treat that as current: only a
    // flow wanting `Exploring` should redirect.
    apply_flow(flow, &AppState::Playing, &mut next_app, &mut next_mode);
}

/// Rebuilds the visible stage by replaying the current scene's steps up to the
/// cursor, applying only their stage effects.
///
/// Needed because leaving `Playing` clears the stage, so returning from
/// exploration would otherwise resume against a blank screen. This is the
/// "silent replay" the design reserved for save/load — the control stack simply
/// needed it sooner.
fn rebuild_stage<S: GameScript>(
    state: Res<StoryState<S>>,
    places: Res<Places>,
    mut stage: ResMut<Stage>,
) {
    replay_stack(&state, &places, &mut stage);
}

/// The replay itself, as a plain function.
///
/// Extracted from the system above because loading needs the identical rebuild
/// at a moment when no state transition is firing. Two copies of this would be
/// two chances for the loaded screen and the resumed screen to differ.
fn replay_stack<S: GameScript>(
    state: &StoryState<S>,
    places: &Places,
    stage: &mut Stage,
) {
    // Music is persistent, not derived: it belongs to the moment rather than to
    // the screen, and no replay of the current stack can know which earlier
    // scene started it. Carried across the rebuild unless a replayed step
    // changes it.
    let music = stage.music.clone();
    *stage = Stage::default();
    stage.music = music;

    // Replay every frame from the BOTTOM up. Replaying only the top frame left
    // a scene *called* from a place with nothing to replay — it sits at step 0
    // — so the screen came back blank.
    let frames: Vec<Frame<S>> = state.frames().to_vec();
    for frame in &frames {
        match frame {
            Frame::Explore { place } => {
                if let Some(p) = places.get(place) {
                    stage.background = Some(p.background.clone());
                    stage.characters.clear();
                    if let Some(track) = &p.music {
                        stage.music = Some(track.clone());
                    }
                }
            }
            Frame::Story { scene, step } => {
                replay_story_frame(&scene.scene(), *step, stage);
            }
        }
    }
}

/// Applies only the stage effects of a scene's first `upto` steps.
///
/// `story::Scene` is qualified because `bevy::prelude` exports a `Scene` too.
fn replay_story_frame<S>(scene: &story::Scene<S>, upto: usize, stage: &mut Stage) {
    for step in scene.steps.iter().take(upto) {
        // Variants listed explicitly: a new step with a stage effect must not
        // be silently skipped here.
        match step {
            Step::Background { image } => stage.background = Some(image.clone()),
            Step::Show { character, slot } => {
                stage.characters.insert(*slot, character.clone());
            }
            Step::Hide { slot } => {
                stage.characters.remove(slot);
            }
            Step::Music { track } => stage.music = track.clone(),
            Step::Say { .. }
            | Step::Narrate { .. }
            | Step::Choices(_)
            | Step::JumpTo(_)
            | Step::JumpIf { .. }
            | Step::Set { .. }
            | Step::Clear { .. }
            | Step::Add { .. }
            | Step::GiveItem { .. }
            | Step::TakeItem { .. }
            | Step::Explore { .. }
            | Step::End => {}
        }
    }
}

fn handle_ui_buttons(
    buttons: Query<(&Interaction, &UiButton), Changed<Interaction>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut load: ResMut<LoadRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiButton::StartStory => next_app.set(AppState::Playing),
            // Leaves the menu exactly as "Begin" does; the request is what makes
            // the difference, and `restart` stands aside when it sees one.
            UiButton::Continue => {
                load.0 = Some(1);
                next_app.set(AppState::Playing);
            }
            UiButton::ReturnToMenu => next_app.set(AppState::MainMenu),
            UiButton::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Installs a save that has been read from disk.
///
/// Runs everywhere **except** the menu, and that is load-bearing rather than
/// tidiness. Leaving the menu runs `restart`, which resets the story; this
/// system must not have consumed the pending load before that happens, or the
/// reset would win. Excluding it from the menu orders the two without either
/// having to know about the other's schedule position.
#[allow(clippy::too_many_arguments)]
fn apply_pending_load<S: GameScript>(
    mut pending: ResMut<PendingLoad>,
    mut state: ResMut<StoryState<S>>,
    places: Res<Places>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    mut toast: ResMut<Toast>,
    app_state: Res<State<AppState>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_mode: ResMut<NextState<PlayMode>>,
) {
    let Some(saved) = pending.0.take() else {
        return;
    };
    let restored = match save::restore(&saved) {
        Ok(state) => state,
        Err(e) => {
            error!("{e}");
            toast.show(format!("load failed: {e}"));
            return;
        }
    };

    *state = restored;
    // The saved cursor sits one past the line the player was reading, because
    // `advance` steps past a line before handing control back.
    state.rewind_to_current_line();
    *line = CurrentLine::default();
    choices.items.clear();

    // Ambient state comes from the file; everything else is rebuilt from the
    // stack. Set before the replay, which carries the current track across.
    stage.music = saved.music.clone();
    replay_stack(&state, &places, &mut stage);

    // Then run the engine normally, which re-displays the line that was on
    // screen and puts us in the right mode — the same path a fresh start takes.
    let flow = advance(&mut state, &mut stage, &mut line, &mut choices);
    apply_flow(flow, app_state.get(), &mut next_app, &mut next_mode);
    toast.show("loaded");
}

/// Space, enter or left-click advances. First press completes the line, second
/// moves on — the convention every visual novel uses.
fn advance_on_input<S: GameScript>(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mode: Res<State<PlayMode>>,
    mut state: ResMut<StoryState<S>>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    app_state: Res<State<AppState>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_mode: ResMut<NextState<PlayMode>>,
) {
    let pressed = keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || mouse.just_pressed(MouseButton::Left);
    if !pressed {
        return;
    }

    match *mode.get() {
        PlayMode::Revealing => {
            line.complete();
            next_mode.set(PlayMode::AwaitingInput);
        }
        PlayMode::AwaitingInput => {
            let flow = advance(&mut state, &mut stage, &mut line, &mut choices);
            apply_flow(flow, app_state.get(), &mut next_app, &mut next_mode);
        }
        // Clicking a choice is handled by `choose_on_click`; a stray click
        // elsewhere must not advance past the branch.
        PlayMode::AwaitingChoice | PlayMode::Finished => {}
    }
}

fn choose_on_click<S: GameScript>(
    buttons: Query<(&Interaction, &ChoiceButton), Changed<Interaction>>,
    mut state: ResMut<StoryState<S>>,
    mut profile: ResMut<PlayerProfile>,
    mut profile_dirty: ResMut<ProfileDirty>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    app_state: Res<State<AppState>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_mode: ResMut<NextState<PlayMode>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let key = state.frames().last().and_then(|frame| match frame {
            Frame::Story { scene, step } => Some(SeenChoice {
                scene: scene.name().to_string(),
                step: *step,
                option: button.0,
            }),
            Frame::Explore { .. } => None,
        });
        if take_choice(button.0, &mut state) {
            if key.is_some_and(|key| profile.seen_choices.insert(key)) {
                profile_dirty.0 = true;
            }
            choices.items.clear();
            let flow = advance(&mut state, &mut stage, &mut line, &mut choices);
            apply_flow(flow, app_state.get(), &mut next_app, &mut next_mode);
        }
        break;
    }
}

/// Applies whichever option the player picked: move, call a scene, or leave.
///
/// The single consumer of [`PickedOption`], and therefore the only place an
/// option's effect happens. A button press and a click on a map region are two
/// ways of naming an option; what follows must not depend on which one it was.
#[allow(clippy::too_many_arguments)]
fn apply_picked_option<S: GameScript>(
    mut picked: ResMut<PickedOption>,
    mut state: ResMut<StoryState<S>>,
    mut profile: ResMut<PlayerProfile>,
    mut profile_dirty: ResMut<ProfileDirty>,
    places: Res<Places>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    app_state: Res<State<AppState>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_mode: ResMut<NextState<PlayMode>>,
) {
    let Some(index) = picked.0.take() else {
        return;
    };
    let key = state.current_place().and_then(|place| {
        places
            .get(place)
            .and_then(|place_data| place_data.options.get(index))
            .filter(|option| allowed(&option.condition, &state.flags, &state.items))
            .map(|_| SeenPlaceOption {
                place: place.to_string(),
                option: index,
            })
    });
    let flow = take_option(index, &mut state, &places);
    if key.is_some_and(|key| profile.seen_place_options.insert(key)) {
        profile_dirty.0 = true;
    }
    // Calling a scene or leaving lands us mid-frame, so run the engine to
    // reach the next thing that actually waits for the player.
    let flow = match flow {
        Flow::Playing(_) => advance(&mut state, &mut stage, &mut line, &mut choices),
        other => other,
    };
    apply_flow(flow, app_state.get(), &mut next_app, &mut next_mode);
}
