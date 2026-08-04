//! Headless tests that run the **real** Bevy app.
//!
//! The other layers cannot see engine bugs. `build.rs` validates content;
//! `no_reachable_path_dead_ends` walks the story model. Both passed happily
//! while the game flickered, dead-ended, fell silent and rendered blank — every
//! one of those was found by a human playing.
//!
//! These tests close that gap by building the actual app with the actual
//! plugins, driving it, and asserting what the player would see: *are there
//! buttons on screen?*
//!
//! No window and no GPU: `MinimalPlugins` plus the asset types the systems ask
//! for. Systems that need a window (the letterbox) find none and return early,
//! which is exactly what they should do.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use vn_core::prelude::*;
// Not in the prelude: it is an internal presentation detail, but asserting on it
// is precisely how these tests check "would the player see buttons?".
use vn_core::present::PlaceOptionButton;

use crate::scenes;

fn harness() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(StatesPlugin)
        // `MinimalPlugins` carries no input, so `ButtonInput<KeyCode>` would not
        // exist and every system reading it fails parameter validation.
        .add_plugins(bevy::input::InputPlugin)
        // Registered by hand because the render and audio plugins are not
        // present: the systems only need the asset *types* to exist.
        .init_asset::<Image>()
        .init_asset::<bevy::audio::AudioSource>()
        .add_plugins(VnPlugin {
            script: Script {
                scene_of: scenes::scene,
                start: scenes::START,
            },
            locale: Locale {
                resolve: scenes::text_en,
            },
            cast: scenes::cast(),
            tuning: scenes::presentation(),
            places: scenes::places(),
            scene_of_name: scenes::scene_of_name,
        });
    app
}

/// Runs enough updates for a state transition to apply and its systems to run.
fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn go(app: &mut App, state: AppState) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(state);
    settle(app);
}

fn option_buttons(app: &mut App) -> usize {
    app.world_mut()
        .query::<&PlaceOptionButton>()
        .iter(app.world())
        .count()
}

#[test]
fn app_builds_and_runs_headless() {
    let mut app = harness();
    settle(&mut app);
    assert_eq!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::MainMenu,
        "the app should start at the menu"
    );
}

#[test]
fn starting_the_story_shows_a_line() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    let line = app.world().resource::<CurrentLine>();
    assert!(
        !line.full.is_empty(),
        "entering Playing should leave a line on screen, got an empty one"
    );

    let stage = app.world().resource::<Stage>();
    assert!(
        stage.background.is_some(),
        "the opening scene should have set a background"
    );
}

/// The exploration view must actually render options.
#[test]
fn entering_a_place_draws_its_options() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    // Put the player in a place directly: reaching it through the story needs
    // simulated clicks, and what is under test here is the *view*, not the
    // route to it.
    app.world_mut()
        .resource_mut::<StoryState<scenes::SceneId>>()
        .push_place("ridge".to_string());
    go(&mut app, AppState::Exploring);

    assert!(
        option_buttons(&mut app) > 0,
        "the ridge offers four options but none were drawn — the player would be stranded"
    );
}

/// Reproduces the dead end found by hand: a room option calls a scene, the
/// exploration UI is despawned on the way out, and on return the view must be
/// rebuilt. The first implementation cached "what is shown" and drew nothing.
#[test]
fn returning_from_a_called_scene_redraws_the_place() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    app.world_mut()
        .resource_mut::<StoryState<scenes::SceneId>>()
        .push_place("ridge".to_string());
    go(&mut app, AppState::Exploring);
    let before = option_buttons(&mut app);
    assert!(before > 0, "precondition: the place should be drawn");

    // Call a scene from the room, as clicking "Ask the golem about the pass"
    // would, then come back.
    {
        let world = app.world_mut();
        let script = world.resource::<Script<scenes::SceneId>>().scene_of;
        let mut state = world.resource_mut::<StoryState<scenes::SceneId>>();
        state.call_scene(scenes::SceneId::PassTalk, &Script { scene_of: script, start: scenes::START });
    }
    go(&mut app, AppState::Playing);
    assert_eq!(
        option_buttons(&mut app),
        0,
        "leaving exploration should tear the option buttons down"
    );

    // Return: pop the scene frame, back to the place.
    {
        let world = app.world_mut();
        let script = world.resource::<Script<scenes::SceneId>>().scene_of;
        let mut state = world.resource_mut::<StoryState<scenes::SceneId>>();
        state.pop(&Script { scene_of: script, start: scenes::START });
    }
    go(&mut app, AppState::Exploring);

    assert!(
        option_buttons(&mut app) > 0,
        "returning from a called scene left the place with no options — this is \
         the dead end that shipped, and it passed every other test"
    );
}

/// Music is persistent state, not derived presentation: moving between phases
/// must not silence the soundtrack.
#[test]
fn music_survives_entering_and_leaving_a_place() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    let track = app.world().resource::<Stage>().music.clone();
    assert!(track.is_some(), "the opening scene should start music");

    app.world_mut()
        .resource_mut::<StoryState<scenes::SceneId>>()
        .push_place("ridge".to_string());
    go(&mut app, AppState::Exploring);
    assert_eq!(
        app.world().resource::<Stage>().music,
        track,
        "the soundtrack stopped on entering a place"
    );

    go(&mut app, AppState::Playing);
    assert_eq!(
        app.world().resource::<Stage>().music,
        track,
        "the soundtrack stopped on returning from a place"
    );
}
