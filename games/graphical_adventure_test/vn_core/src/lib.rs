//! `vn_core` — a visual-novel engine as Bevy plugins.
//!
//! See `../../DESIGN.md` for the architecture. The rule that governs the whole
//! crate: **presentation is a pure function of story state.** The story module
//! updates [`story::Stage`] and [`story::CurrentLine`]; the presentation systems
//! reconcile the screen toward them and decide nothing.

use bevy::prelude::*;

pub mod letterbox;
pub mod present;
pub mod story;

pub mod prelude {
    pub use crate::letterbox::*;
    pub use crate::present::{DESIGN_H, DESIGN_W};
    pub use crate::story::*;
    pub use crate::VnPlugin;
}

use present::*;
use story::*;

/// Add this, supplying the game's [`Script`] and [`Locale`].
pub struct VnPlugin<S: SceneKey> {
    pub script: Script<S>,
    pub locale: Locale,
    /// Character id -> display name and image. `build.rs` guarantees every id a
    /// scene references is present.
    pub cast: Cast,
}

impl<S: SceneKey> Plugin for VnPlugin<S> {
    fn build(&self, app: &mut App) {
        let start = self.script.start;
        let current = (self.script.scene_of)(start);

        app.insert_resource(Script::<S> {
            scene_of: self.script.scene_of,
            start,
        })
        .insert_resource(Locale {
            resolve: self.locale.resolve,
        })
        .insert_resource(self.cast.clone())
        .insert_resource(StoryState::<S> {
            scene: start,
            step: 0,
            current,
            flags: Default::default(),
        })
        .init_resource::<Stage>()
        .init_resource::<AppliedStage>()
        .init_resource::<CurrentLine>()
        .init_resource::<PendingChoices>()
        .init_state::<AppState>()
        .add_sub_state::<PlayMode>()
        .add_systems(Startup, letterbox::spawn_camera)
        .add_systems(OnEnter(AppState::MainMenu), spawn_menu)
        // Reset before spawning UI so the first line is on screen immediately.
        .add_systems(OnEnter(AppState::Playing), (restart::<S>, spawn_ui).chain())
        // Clearing the stage on the way out makes the reconcilers below despawn
        // the background, cast and music. No bespoke teardown.
        .add_systems(OnExit(AppState::Playing), clear_stage)
        .add_systems(OnEnter(PlayMode::AwaitingChoice), spawn_choices)
        .add_systems(OnExit(PlayMode::AwaitingChoice), despawn_choices)
        .add_systems(OnEnter(PlayMode::Finished), spawn_end_screen)
        // Reconcilers and the letterbox run in every state, so leaving `Playing`
        // actually tears the screen down.
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
                handle_ui_buttons,
            )
                .chain(),
        )
        // Story systems exist only while playing.
        .add_systems(
            Update,
            (
                advance_on_input::<S>,
                reveal_text,
                choose_on_click::<S>,
                draw_dialogue,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );
    }
}

fn clear_stage(mut stage: ResMut<Stage>) {
    *stage = Stage::default();
}

/// Rewinds to the opening and plays the first batch of steps.
fn restart<S: SceneKey>(
    mut state: ResMut<StoryState<S>>,
    script: Res<Script<S>>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    mut next: ResMut<NextState<PlayMode>>,
) {
    reset(&mut state, &script, &mut stage, &mut line, &mut choices);
    let mode = advance(&mut state, &script, &mut stage, &mut line, &mut choices);
    next.set(mode);
}

fn handle_ui_buttons(
    buttons: Query<(&Interaction, &UiButton), Changed<Interaction>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            UiButton::StartStory => next_app.set(AppState::Playing),
            UiButton::ReturnToMenu => next_app.set(AppState::MainMenu),
            UiButton::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Space, enter or left-click advances. First press completes the line, second
/// moves on — the convention every visual novel uses.
fn advance_on_input<S: SceneKey>(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mode: Res<State<PlayMode>>,
    mut state: ResMut<StoryState<S>>,
    script: Res<Script<S>>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    mut next: ResMut<NextState<PlayMode>>,
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
            next.set(PlayMode::AwaitingInput);
        }
        PlayMode::AwaitingInput => {
            let m = advance(&mut state, &script, &mut stage, &mut line, &mut choices);
            next.set(m);
        }
        // Clicking a choice is handled by `choose_on_click`; a stray click
        // elsewhere must not advance past the branch.
        PlayMode::AwaitingChoice | PlayMode::Finished => {}
    }
}

fn choose_on_click<S: SceneKey>(
    buttons: Query<(&Interaction, &ChoiceButton), Changed<Interaction>>,
    mut state: ResMut<StoryState<S>>,
    script: Res<Script<S>>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
    mut next: ResMut<NextState<PlayMode>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if take_choice(button.0, &mut state, &script) {
            choices.items.clear();
            let m = advance(&mut state, &script, &mut stage, &mut line, &mut choices);
            next.set(m);
        }
        break;
    }
}
