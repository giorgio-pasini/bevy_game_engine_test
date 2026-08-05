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

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
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
            scene_name_of: scenes::scene_name_of,
        });
    // Saves must never land in the player's game folder from a test run, and two
    // tests must never share a slot file.
    let root = std::env::temp_dir().join(format!(
        "vn_core_test_saves_{}_{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    app.insert_resource(SaveSlots { root });
    app
}

fn slots(app: &App) -> SaveSlots {
    app.world().resource::<SaveSlots>().clone()
}

/// Saves slot 1 straight from the live app, exactly as F5 does.
fn save_now(app: &mut App) {
    let world = app.world();
    let state = world.resource::<StoryState<scenes::SceneId>>();
    let stage = world.resource::<Stage>();
    let lookup = world.resource::<SceneLookup<scenes::SceneId>>();
    let snapshot = capture(state, stage.music.clone(), lookup.name_of);
    write_slot(&slots(app), 1, snapshot, "test".to_string()).expect("save should succeed");
}

fn load_now(app: &mut App) {
    app.world_mut().resource_mut::<LoadRequest>().0 = Some(1);
    settle(app);
}

fn capture_now(app: &App) -> SaveState {
    let world = app.world();
    capture(
        world.resource::<StoryState<scenes::SceneId>>(),
        world.resource::<Stage>().music.clone(),
        world.resource::<SceneLookup<scenes::SceneId>>().name_of,
    )
}

/// Runs the story engine one step, the way a click would.
///
/// Deliberately not simulated keyboard input: `ButtonInput` is cleared in
/// `PreUpdate`, so a press written from outside the schedule is wiped before any
/// system sees it. Running the engine directly tests the same function the input
/// system calls, without pretending to test input handling as well.
fn advance_once(app: &mut App) {
    app.world_mut()
        .run_system_once(step_story)
        .expect("the story should advance");
}

fn step_story(
    mut state: ResMut<StoryState<scenes::SceneId>>,
    script: Res<Script<scenes::SceneId>>,
    mut stage: ResMut<Stage>,
    mut line: ResMut<CurrentLine>,
    mut choices: ResMut<PendingChoices>,
) {
    advance(&mut state, &script, &mut stage, &mut line, &mut choices);
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

/// A map place lists nothing — its options are on the picture — but it must
/// still tell the player what is under the cursor.
#[test]
fn a_map_place_draws_a_hotspot_label_and_no_buttons() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    app.world_mut()
        .resource_mut::<StoryState<scenes::SceneId>>()
        .push_place("valley".to_string());
    go(&mut app, AppState::Exploring);

    assert_eq!(
        option_buttons(&mut app),
        0,
        "every option on the valley map has a region, so none should be listed"
    );
    assert_eq!(
        app.world_mut()
            .query::<&vn_core::present::HotspotLabel>()
            .iter(app.world())
            .count(),
        1,
        "a map place must name the region under the cursor — the hotspots are \
         invisible, so without the label the player is hunting pixels"
    );
}

/// Sampling a colour must select the option that declared it, and must respect
/// that option's condition exactly as a button does.
///
/// The real lookup PNG cannot be loaded here — no render plugins means no image
/// loader — and `build.rs` already audits its pixels. What needs proving in the
/// engine is the step after that: colour in, option out.
#[test]
fn a_sampled_region_selects_the_option_that_declared_it() {
    let places = scenes::places();
    let place = places.get("valley").expect("the demo has a map place");
    assert!(place.lookup.is_some(), "`valley` should be a map place");

    let regions: Vec<Rgb> = place.options.iter().filter_map(|o| o.region).collect();
    assert_eq!(regions.len(), 3, "the valley map declares three regions");

    // A lookup one pixel tall and one column per region, so column i covers
    // design-space band i — the geometry the real image has, in miniature.
    let mut data = Vec::new();
    for r in &regions {
        data.extend_from_slice(&[r.0, r.1, r.2, 255]);
    }
    let image = Image::new(
        Extent3d {
            width: regions.len() as u32,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );

    let flags = Default::default();
    let items = Default::default();
    for (column, expected) in regions.iter().enumerate() {
        let x = DESIGN_W * ((column as f32 + 0.5) / regions.len() as f32 - 0.5);
        let sampled = sample_design_point(&image, x, 0.0);
        assert_eq!(
            sampled,
            Some(*expected),
            "sampling the middle of column {column} read the wrong colour"
        );
        let picked = places.option_for_region("valley", sampled.unwrap(), &flags, &items);
        assert_eq!(
            picked.and_then(|i| place.options.get(i)).map(|o| &o.label),
            place
                .options
                .iter()
                .find(|o| o.region == Some(*expected))
                .map(|o| &o.label),
            "region {expected:?} selected the wrong option"
        );
    }

    // Outside the design area there is nothing to click — the letterbox bars.
    assert_eq!(sample_design_point(&image, DESIGN_W, 0.0), None);
    // Black is `no hotspot here`, never an option.
    assert_eq!(
        places.option_for_region("valley", Rgb::EMPTY, &flags, &items),
        None
    );
}

/// The design's own test of the architecture: save, load, save again, and the
/// state must come back **exactly**.
///
/// It catches the whole class of restore bugs in one assertion. A replay that
/// re-ran `Set` or `Add` would show up as a drifted counter; a cursor restored
/// without the rewind would show up as a step index one further along every
/// round trip.
///
/// Metadata is excluded on purpose — a timestamp legitimately differs between
/// two saves. The game state must not.
#[test]
fn save_load_save_round_trips_the_state_exactly() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);
    // A few lines in, so the cursor is somewhere non-trivial.
    for _ in 0..3 {
        advance_once(&mut app);
    }

    save_now(&mut app);
    let first = read_slot(&slots(&app), 1).expect("save should read back");

    load_now(&mut app);
    save_now(&mut app);
    let second = read_slot(&slots(&app), 1).expect("save should read back");

    assert_eq!(
        ron::ser::to_string_pretty(&first.state, default()).unwrap(),
        ron::ser::to_string_pretty(&second.state, default()).unwrap(),
        "loading and re-saving changed the state"
    );
}

/// Saving inside a scene *called from a place* is the case the control stack
/// exists for: two frames deep, with the room waiting underneath.
#[test]
fn a_save_two_frames_deep_restores_the_whole_stack() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    app.world_mut()
        .resource_mut::<StoryState<scenes::SceneId>>()
        .push_place("ridge".to_string());
    go(&mut app, AppState::Exploring);
    {
        let world = app.world_mut();
        let scene_of = world.resource::<Script<scenes::SceneId>>().scene_of;
        let mut state = world.resource_mut::<StoryState<scenes::SceneId>>();
        state.call_scene(
            scenes::SceneId::PassTalk,
            &Script {
                scene_of,
                start: scenes::START,
            },
        );
    }
    go(&mut app, AppState::Playing);
    advance_once(&mut app);

    let before = capture_now(&app);
    assert_eq!(before.stack.len(), 3, "story -> place -> called scene");
    save_now(&mut app);

    // Wander off somewhere else entirely, then load.
    app.world_mut()
        .resource_mut::<StoryState<scenes::SceneId>>()
        .stack
        .truncate(1);
    load_now(&mut app);

    let after = capture_now(&app);
    assert_eq!(after.stack, before.stack, "the stack did not come back");
    assert_eq!(after.flags, before.flags);
    assert_eq!(after.items, before.items);

    // And the screen was rebuilt, not left blank — the reason replay walks every
    // frame from the bottom rather than just the top one.
    let stage = app.world().resource::<Stage>();
    assert!(
        stage.background.is_some(),
        "loading left the stage with no background"
    );
    assert!(
        !app.world().resource::<CurrentLine>().full.is_empty(),
        "loading left no line on screen"
    );
}

/// Loading must not re-run the steps that *changed* state — only the ones that
/// dress the stage. A counter that advanced again on every load would corrupt a
/// save a little more each time it was reloaded.
#[test]
fn loading_does_not_replay_state_changing_steps() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    // `ledge_find` grants an item and adds to a counter, then we save past it.
    {
        let world = app.world_mut();
        let scene_of = world.resource::<Script<scenes::SceneId>>().scene_of;
        let mut state = world.resource_mut::<StoryState<scenes::SceneId>>();
        state.call_scene(
            scenes::SceneId::LedgeFind,
            &Script {
                scene_of,
                start: scenes::START,
            },
        );
    }
    for _ in 0..3 {
        advance_once(&mut app);
    }
    let before = capture_now(&app);
    assert_eq!(
        before.flags.get("trust"),
        Some(&1),
        "precondition: the scene should have added to `trust` exactly once"
    );
    assert!(before.items.contains("brass_gear"));

    save_now(&mut app);
    load_now(&mut app);
    load_now(&mut app);

    let after = capture_now(&app);
    assert_eq!(
        after.flags, before.flags,
        "two loads changed the flags — the replay is re-running Set/Add steps"
    );
    assert_eq!(after.items, before.items);
}

/// Music cannot be derived from the stack, so it is the one piece of stage state
/// the file carries.
///
/// The discriminator matters: the track has to be one that **no frame on the
/// restored stack sets**, and different from what a fresh run would start. Any
/// other choice passes whether or not the save carries music at all — replay
/// would reproduce it, or a fresh start would coincidentally match. This is the
/// scenario the save field exists for: a track started by a scene the story has
/// since jumped away from.
#[test]
fn a_load_restores_the_soundtrack() {
    let mut app = harness();
    settle(&mut app);
    go(&mut app, AppState::Playing);

    // Leave the opening scene, so the frame carrying `Music(...)` is gone.
    for _ in 0..30 {
        if !matches!(
            capture_now(&app).stack.first(),
            Some(SavedFrame::Story { scene, .. }) if scene == "awakening"
        ) {
            break;
        }
        advance_once(&mut app);
    }
    assert!(
        !matches!(
            capture_now(&app).stack.first(),
            Some(SavedFrame::Story { scene, .. }) if scene == "awakening"
        ),
        "precondition: the story should have moved off the opening scene"
    );

    // A track an earlier scene started and nothing on the stack can reproduce.
    let track = Some("music/calm.ogg".to_string());
    app.world_mut().resource_mut::<Stage>().music = track.clone();
    save_now(&mut app);

    let expected = capture_now(&app);

    // A second app, as if the process had been restarted, taking the "Continue"
    // path out of the menu: leaving it runs `restart`, which must not reset the
    // run out from under the load.
    let mut fresh = harness();
    fresh.insert_resource(slots(&app));
    settle(&mut fresh);
    fresh.world_mut().resource_mut::<LoadRequest>().0 = Some(1);
    fresh
        .world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    settle(&mut fresh);

    assert_eq!(
        fresh.world().resource::<Stage>().music,
        track,
        "the loaded game is playing what a fresh run starts, not what was saved"
    );
    assert_eq!(
        capture_now(&fresh).stack,
        expected.stack,
        "starting a run over the top of a pending load threw the save away"
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
