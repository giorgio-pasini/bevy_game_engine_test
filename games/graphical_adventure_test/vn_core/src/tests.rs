//! The engine's own tests, against a **synthetic** game.
//!
//! Everything else in this project tests `vn_core` through `demo_game`, which
//! has two problems: a regression the demo's content does not happen to
//! exercise is invisible, and editing the story breaks tests of the engine. A
//! library validated only through one consumer is not validated.
//!
//! So the fixture below is three scenes and two places that exist purely to be
//! awkward — a branch that is never taken, an option that is always refused, a
//! line immediately before the cursor. None of it is playable and none of it
//! needs to be.

// Only `Image`, deliberately not the prelude: `bevy::prelude::Scene` collides
// with `story::Scene`, and glob-importing both makes every mention ambiguous.
use bevy::prelude::Image;
use std::collections::BTreeMap;

use crate::map::*;
use crate::place::*;
use crate::save::*;
use crate::story::*;

// --------------------------------------------------------------- fixture ---

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Test {
    Opening,
    Branch,
    Ending,
}

impl GameScript for Test {
    fn scene(self) -> Scene<Test> {
        content(self)
    }

    fn name(self) -> &'static str {
        match self {
            Test::Opening => "opening",
            Test::Branch => "branch",
            Test::Ending => "ending",
        }
    }

    fn from_name(name: &str) -> Option<Test> {
        match name {
            "opening" => Some(Test::Opening),
            "branch" => Some(Test::Branch),
            "ending" => Some(Test::Ending),
            _ => None,
        }
    }
}

fn bg() -> Step<Test> {
    Step::Background {
        image: "bg.png".into(),
    }
}

fn content(id: Test) -> Scene<Test> {
    let steps = match id {
        // Establishes a whole stage before it blocks, which is what `advance`
        // batching is for.
        Test::Opening => vec![
            bg(),
            Step::Music {
                track: Some("m.ogg".into()),
            },
            Step::Show {
                character: "mira".into(),
                slot: Slot::Left,
            },
            Step::Say {
                speaker: "mira".into(),
                text: "hello".into(),
            },
            Step::Narrate {
                text: "a beat".into(),
            },
            Step::End,
        ],
        // The guarded option is deliberately FIRST. With it last, the surviving
        // option keeps index 0 and a bug that renumbered options after
        // filtering would be invisible — the test would pass by coincidence.
        Test::Branch => vec![
            bg(),
            Step::Choices(vec![
                Choice {
                    text: "never".into(),
                    target: Test::Opening,
                    condition: Some(Condition::IsSet("impossible".into())),
                },
                Choice {
                    text: "always".into(),
                    target: Test::Ending,
                    condition: None,
                },
            ]),
        ],
        // Carries a `Show` *and* a `Hide` so the replay-agreement test below has
        // every stage effect to compare. Without the `Hide`, a replay that
        // ignored it would leave a character on screen and nothing would notice.
        Test::Ending => vec![
            bg(),
            Step::Show {
                character: "golem".into(),
                slot: Slot::Right,
            },
            Step::Hide {
                slot: Slot::Right,
            },
            Step::Add {
                flag: "counter".into(),
                delta: 1,
            },
            Step::GiveItem {
                item: "token".into(),
            },
            Step::End,
        ],
    };
    Scene { id, steps }
}

fn state_at(id: Test, step: usize) -> StoryState<Test> {
    StoryState::from_frames(
        vec![Frame::Story { scene: id, step }],
        BTreeMap::new(),
        Default::default(),
    )
    .expect("a story frame is present")
}

/// Two rooms: one with an exit and a conditional action, one that only leaves.
fn places() -> Places {
    let mut map = BTreeMap::new();
    map.insert(
        "hall".to_string(),
        Place {
            background: "bg.png".into(),
            music: None,
            caption: None,
            lookup: Some("lookup.png".into()),
            options: vec![
                PlaceOption {
                    label: "to the cellar".into(),
                    effect: Effect::GoTo("cellar".into()),
                    condition: None,
                    region: Some(Rgb(10, 20, 30)),
                },
                PlaceOption {
                    label: "call a scene".into(),
                    effect: Effect::Call("ending".into()),
                    condition: None,
                    region: None,
                },
                PlaceOption {
                    label: "locked".into(),
                    effect: Effect::Leave,
                    condition: Some(Condition::HasItem("key".into())),
                    region: Some(Rgb(40, 50, 60)),
                },
                // Deliberately mis-authored: black is reserved for "no hotspot
                // here". `build.rs` rejects this in a real game, but the engine
                // must refuse it too — and without an option claiming black,
                // the assertion that black selects nothing could never fail.
                PlaceOption {
                    label: "claims black".into(),
                    effect: Effect::Leave,
                    condition: None,
                    region: Some(Rgb::EMPTY),
                },
            ],
        },
    );
    map.insert(
        "cellar".to_string(),
        Place {
            background: "bg.png".into(),
            music: None,
            caption: None,
            lookup: None,
            options: vec![PlaceOption {
                label: "leave".into(),
                effect: Effect::Leave,
                condition: None,
                region: None,
            }],
        },
    );
    Places(map)
}

/// Runs the engine once, discarding the presentation it produces.
fn advance_once(state: &mut StoryState<Test>) -> (Flow, Stage, CurrentLine, PendingChoices) {
    let (mut stage, mut line, mut choices) = (
        Stage::default(),
        CurrentLine::default(),
        PendingChoices::default(),
    );
    let flow = advance(state, &mut stage, &mut line, &mut choices);
    (flow, stage, line, choices)
}

// ----------------------------------------------------------------- story ---

#[test]
fn advance_batches_stage_steps_until_something_blocks() {
    let mut state = state_at(Test::Opening, 0);
    let (flow, stage, line, _) = advance_once(&mut state);

    assert_eq!(flow, Flow::Playing(PlayMode::Revealing));
    assert_eq!(line.full, "hello", "should stop at the first line");
    assert_eq!(stage.background.as_deref(), Some("bg.png"));
    assert_eq!(stage.music.as_deref(), Some("m.ogg"));
    assert_eq!(
        stage.characters.get(&Slot::Left).map(String::as_str),
        Some("mira"),
        "the cast established before the line should be on stage"
    );
    assert_eq!(
        state.story_step(),
        Some(4),
        "the cursor should sit one past the line it just displayed"
    );
}

#[test]
fn end_pops_the_stack_and_an_empty_stack_ends_the_game() {
    let mut state = state_at(Test::Ending, 0);
    let (flow, _, _, _) = advance_once(&mut state);

    assert_eq!(flow, Flow::Ended);
    assert!(state.is_over());
    assert_eq!(
        state.flags.get("counter"),
        Some(&1),
        "steps before End should still have applied"
    );
    assert!(state.items.contains("token"));
}

#[test]
fn a_branch_offers_only_options_whose_condition_holds() {
    let mut state = state_at(Test::Branch, 0);
    let before = state.story_step();
    let (flow, _, _, choices) = advance_once(&mut state);

    assert_eq!(flow, Flow::Playing(PlayMode::AwaitingChoice));
    assert_eq!(choices.items.len(), 1, "the guarded option must be hidden");
    assert_eq!(choices.items[0].0, "always");
    assert_eq!(
        choices.items[0].1, 1,
        "the ORIGINAL index must survive filtering, or a click resolves the wrong branch"
    );
    assert_eq!(
        state.story_step(),
        before.map(|_| 1),
        "a branch must not move the cursor — the player's answer resolves it"
    );
}

#[test]
fn taking_a_choice_replaces_the_frame_rather_than_pushing() {
    let mut state = state_at(Test::Branch, 1);
    let (_, _, _, _) = advance_once(&mut state);
    assert!(take_choice(1, &mut state));

    assert_eq!(state.depth(), 1, "choosing is not calling");
    assert!(matches!(
        state.top(),
        Some(Frame::Story {
            scene: Test::Ending,
            step: 0
        })
    ));
}

#[test]
fn a_refused_choice_leaves_the_state_alone() {
    let mut state = state_at(Test::Branch, 1);
    advance_once(&mut state);
    assert!(
        !take_choice(0, &mut state),
        "the guarded option must be refused even if something clicks it"
    );
    assert!(matches!(
        state.top(),
        Some(Frame::Story {
            scene: Test::Branch,
            ..
        })
    ));
}

#[test]
fn rewind_steps_back_over_a_line_but_never_over_a_branch() {
    // After a line: the cursor sits past it and must come back.
    let mut state = state_at(Test::Opening, 4);
    state.rewind_to_current_line();
    assert_eq!(state.story_step(), Some(3), "should land on the `Say`");

    // At a branch: the cursor already points at the choice.
    let mut state = state_at(Test::Branch, 1);
    state.rewind_to_current_line();
    assert_eq!(state.story_step(), Some(1), "a branch needs no correction");

    // Nothing has been shown yet.
    let mut state = state_at(Test::Opening, 0);
    state.rewind_to_current_line();
    assert_eq!(state.story_step(), Some(0));

    // Exploring: there is no line to come back to.
    let mut state = state_at(Test::Opening, 4);
    state.push_place("hall".into());
    state.rewind_to_current_line();
    assert_eq!(state.current_place(), Some("hall"));
}

/// The engine and the replay must agree about what a scene puts on screen.
///
/// There are two interpreters of `Step`: `advance`, which runs the story, and
/// `replay_story_frame`, which rebuilds the stage from a cursor position after a
/// phase change or a load. Nothing but care has kept them in step, and when they
/// disagree the screen silently contradicts the story — which is how scenes
/// shipped blank and how the music once stopped.
///
/// The property: running the engine to a stopping point, and replaying to the
/// cursor it stopped at, must produce the *same* `Stage`.
///
/// This can only see a divergence in a step the fixture actually uses, so the
/// fixture deliberately exercises all four stage effects — `Background`,
/// `Music`, `Show` and `Hide`. A new stage-affecting variant must be added
/// there too, or this test will not be watching it.
#[test]
fn replay_reproduces_exactly_the_stage_the_engine_built() {
    for id in [Test::Opening, Test::Branch, Test::Ending] {
        let mut state = state_at(id, 0);
        let (mut engine, mut line, mut choices) = (
            Stage::default(),
            CurrentLine::default(),
            PendingChoices::default(),
        );
        advance(&mut state, &mut engine, &mut line, &mut choices);

        // Where the engine stopped. `End` pops the frame, so a scene that ran to
        // completion has no cursor left — that is the whole scene replayed.
        let cursor = state.story_step().unwrap_or_else(|| id.scene().steps.len());

        let mut replayed = Stage::default();
        crate::replay_story_frame(&id.scene(), cursor, &mut replayed);

        assert_eq!(
            replayed, engine,
            "scene `{}` at step {cursor}: the replay built a different stage than \
             the engine did — the two interpreters of `Step` have diverged",
            id.name()
        );
    }
}

// ----------------------------------------------------------------- place ---

#[test]
fn moving_between_rooms_replaces_the_frame_and_calling_pushes_one() {
    let mut state = state_at(Test::Opening, 4);
    state.push_place("hall".into());
    let depth = state.depth();

    // GoTo: same depth, different room. Pushing here would make one `Leave`
    // unwind a single room instead of leaving exploration.
    let flow = take_option(0, &mut state, &places());
    assert_eq!(flow, Flow::Exploring);
    assert_eq!(state.depth(), depth);
    assert_eq!(state.current_place(), Some("cellar"));

    // Call: one frame deeper, and the room is still underneath.
    let mut state = state_at(Test::Opening, 4);
    state.push_place("hall".into());
    let flow = take_option(1, &mut state, &places());
    assert_eq!(flow, Flow::Playing(PlayMode::Revealing));
    assert_eq!(state.depth(), depth + 1);
    assert_eq!(state.scene().id, Test::Ending);

    // Leave: back to the story frame beneath.
    let mut state = state_at(Test::Opening, 4);
    state.push_place("cellar".into());
    let flow = take_option(0, &mut state, &places());
    assert_eq!(flow, Flow::Playing(PlayMode::Revealing));
    assert_eq!(state.current_place(), None);
    assert_eq!(state.story_step(), Some(4), "the story resumes where it was");
}

#[test]
fn an_option_whose_condition_fails_is_refused_even_when_clicked() {
    let mut state = state_at(Test::Opening, 4);
    state.push_place("hall".into());

    // Index 2 requires an item the player does not carry. A button can outlive
    // the state that offered it, so `take_option` re-checks rather than trusting.
    let flow = take_option(2, &mut state, &places());
    assert_eq!(flow, Flow::Exploring);
    assert_eq!(state.current_place(), Some("hall"), "nothing should have happened");

    let places = places();
    let available = places.available("hall", &state.flags, &state.items);
    assert_eq!(available.len(), 3, "only the guarded option should be withheld");
    assert!(
        !available.iter().any(|(i, _)| *i == 2),
        "and it must be the guarded one that is missing"
    );
}

// ------------------------------------------------------------------- map ---

/// A 2x2 lookup: top-left and bottom-right carry keys, the rest is empty.
fn lookup_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let px = |r, g, b| [r, g, b, 255u8];
    let mut data = Vec::new();
    data.extend_from_slice(&px(10, 20, 30)); // top-left
    data.extend_from_slice(&px(0, 0, 0)); // top-right
    data.extend_from_slice(&px(0, 0, 0)); // bottom-left
    data.extend_from_slice(&px(40, 50, 60)); // bottom-right

    Image::new(
        Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    )
}

#[test]
fn sampling_maps_design_space_onto_the_lookup_and_rejects_the_letterbox() {
    let image = lookup_image();
    let (w, h) = (crate::present::DESIGN_W, crate::present::DESIGN_H);

    // y is up in the world and down in the image; getting this backwards
    // mirrors every map vertically, which is easy to miss on a symmetric one.
    assert_eq!(
        sample_design_point(&image, -w * 0.25, h * 0.25),
        Some(Rgb(10, 20, 30)),
        "upper-left quadrant"
    );
    assert_eq!(
        sample_design_point(&image, w * 0.25, -h * 0.25),
        Some(Rgb(40, 50, 60)),
        "lower-right quadrant"
    );
    assert_eq!(
        sample_design_point(&image, w * 0.25, h * 0.25),
        Some(Rgb::EMPTY),
        "upper-right is empty, not missing"
    );

    // Outside the design area there is nothing to click — the black bars.
    assert_eq!(sample_design_point(&image, w, 0.0), None);
    assert_eq!(sample_design_point(&image, 0.0, -h), None);
    assert_eq!(sample_design_point(&image, -w * 0.5 - 1.0, 0.0), None);
}

#[test]
fn a_region_selects_the_option_that_declared_it() {
    let places = places();
    let flags = BTreeMap::new();
    let items = Default::default();

    assert_eq!(
        places.option_for_region("hall", Rgb(10, 20, 30), &flags, &items),
        Some(0)
    );
    assert_eq!(
        places.option_for_region("hall", Rgb::EMPTY, &flags, &items),
        None,
        "black means `no hotspot here` and must never select anything"
    );
    assert_eq!(
        places.option_for_region("hall", Rgb(99, 99, 99), &flags, &items),
        None,
        "an undeclared colour selects nothing"
    );
    assert_eq!(
        places.option_for_region("hall", Rgb(40, 50, 60), &flags, &items),
        None,
        "a region whose condition fails must not be clickable either"
    );

    let mut carrying = std::collections::BTreeSet::new();
    carrying.insert("key".to_string());
    assert_eq!(
        places.option_for_region("hall", Rgb(40, 50, 60), &flags, &carrying),
        Some(2),
        "and must become clickable once it holds"
    );
}

// ------------------------------------------------------------------ save ---

#[test]
fn a_snapshot_round_trips_through_capture_and_restore() {
    let mut state = state_at(Test::Opening, 4);
    state.push_place("hall".into());
    state.flags.insert("counter".into(), 3);
    state.items.insert("token".into());

    let snapshot = capture(&state, Some("m.ogg".into()));
    let restored = restore::<Test>(&snapshot).expect("should restore");

    assert_eq!(capture(&restored, Some("m.ogg".into())), snapshot);
    assert_eq!(restored.current_place(), Some("hall"));
    assert_eq!(restored.flags.get("counter"), Some(&3));
    assert!(restored.items.contains("token"));
}

#[test]
fn restoring_rebuilds_the_cached_scene_from_the_top_story_frame() {
    // The cache is deliberately not in the file: a save that stored it could
    // disagree with its own stack.
    //
    // **Two** story frames, with a place between them, and they name different
    // scenes. With only one, "topmost story frame" and "any story frame" give
    // the same answer and this test cannot tell them apart — which it could not,
    // until a deliberate break of the derivation left it green.
    let mut state = state_at(Test::Opening, 4);
    state.push_place("hall".into());
    state.call_scene(Test::Ending);
    assert_eq!(state.depth(), 3, "story -> place -> called scene");

    let snapshot = capture(&state, None);
    let restored = restore::<Test>(&snapshot).unwrap();

    assert_eq!(
        restored.scene().id,
        Test::Ending,
        "the innermost story frame is what is running — not the outermost"
    );
}

#[test]
fn slots_resolve_to_a_per_user_directory_named_for_the_game() {
    let slots = SaveSlots::for_game("SomeGame");

    assert!(
        slots.root.is_absolute(),
        "a relative save path follows the working directory, which is exactly \
         the bug this replaced — got {}",
        slots.root.display()
    );
    assert!(
        slots.root.ends_with("SomeGame/saves") || slots.root.ends_with("SomeGame\\saves"),
        "saves should live under the game's own folder, got {}",
        slots.root.display()
    );
    assert_eq!(
        slots.path(2).file_name().unwrap(),
        std::ffi::OsStr::new("save_2.ron")
    );
}

#[test]
fn writing_a_slot_replaces_it_atomically_and_leaves_no_staging_file() {
    // Plain digits: a `Debug`-formatted `SystemTime` contains braces and a
    // colon, which Windows rejects as a directory name.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!("vn_core_atomic_{}_{unique}", std::process::id()));
    let slots = SaveSlots { root: root.clone() };
    // Cleans up on the way out even if an assertion below panics — which is
    // exactly the run that used to leak, since a trailing `remove_dir_all` is
    // never reached when a test fails.
    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = TempDir(root.clone());

    let first = SaveState {
        flags: BTreeMap::from([("counter".to_string(), 1)]),
        ..Default::default()
    };
    let second = SaveState {
        flags: BTreeMap::from([("counter".to_string(), 2)]),
        ..Default::default()
    };

    write_slot(&slots, 1, first, "here".into()).expect("first write");
    write_slot(&slots, 1, second.clone(), "here".into()).expect("overwrite");

    assert_eq!(
        read_slot(&slots, 1).expect("should read back").state,
        second,
        "the second write should have replaced the first"
    );
    assert!(
        !slots.path(1).with_extension("ron.tmp").exists(),
        "the staging file must not survive a successful write"
    );

}

#[test]
fn restoring_refuses_a_save_naming_content_that_no_longer_exists() {
    let snapshot = SaveState {
        stack: vec![SavedFrame::Story {
            scene: "a_scene_that_was_deleted".into(),
            step: 0,
        }],
        ..Default::default()
    };
    // `.err()` rather than `expect_err`: the success type is `StoryState`, which
    // is deliberately not `Debug` — a scene's steps include whole strings of
    // prose, and printing them on a failure would bury the message.
    let error = restore::<Test>(&snapshot)
        .err()
        .expect("should refuse a scene name that no longer exists");
    assert!(
        error.contains("a_scene_that_was_deleted"),
        "the error must name the missing scene, got: {error}"
    );

    // A save with no story frame cannot rebuild the scene cache either.
    let snapshot = SaveState {
        stack: vec![SavedFrame::Explore {
            place: "hall".into(),
        }],
        ..Default::default()
    };
    assert!(restore::<Test>(&snapshot).is_err());
}
