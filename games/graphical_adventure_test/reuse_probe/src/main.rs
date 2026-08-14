//! Minimal independent consumer of `vn_core`.
//!
//! This is an architectural probe, not a second content showcase. It owns one
//! background, character and music track but has no data pipeline or new engine
//! features: three Rust-authored scenes are enough to prove another game can
//! supply the full presentation contract and run.

use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::collections::BTreeMap;
use vn_core::prelude::*;
// Both preludes export `Scene`: Bevy's trait and vn_core's story struct. A real
// second consumer exposed the collision, so name the story type explicitly.
use vn_core::story::Scene as VnScene;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SceneId {
    Crossroads,
    Lantern,
    River,
}

impl GameScript for SceneId {
    fn scene(self) -> VnScene<Self> {
        match self {
            SceneId::Crossroads => VnScene {
                id: self,
                steps: vec![
                    Step::Background {
                        image: "backgrounds/crossroads.webp".into(),
                    },
                    Step::Music {
                        track: Some("music/road.ogg".into()),
                    },
                    Step::Show {
                        character: "guide".into(),
                        slot: Slot::Centre,
                    },
                    Step::Say {
                        speaker: "guide".into(),
                        text: "At the crossroads, two quiet paths remain.".into(),
                    },
                    Step::Choices(vec![
                        Choice {
                            text: "Follow the lantern".into(),
                            target: SceneId::Lantern,
                            condition: None,
                        },
                        Choice {
                            text: "Follow the river".into(),
                            target: SceneId::River,
                            condition: None,
                        },
                    ]),
                ],
            },
            SceneId::Lantern => VnScene {
                id: self,
                steps: vec![
                    Step::Background {
                        image: "backgrounds/crossroads.webp".into(),
                    },
                    Step::Show {
                        character: "guide".into(),
                        slot: Slot::Centre,
                    },
                    Step::Say {
                        speaker: "guide".into(),
                        text: "The lantern leads home. The engine belongs to more than one game."
                            .into(),
                    },
                    Step::End,
                ],
            },
            SceneId::River => VnScene {
                id: self,
                steps: vec![
                    Step::Background {
                        image: "backgrounds/crossroads.webp".into(),
                    },
                    Step::Show {
                        character: "guide".into(),
                        slot: Slot::Centre,
                    },
                    Step::Say {
                        speaker: "guide".into(),
                        text: "The river reaches dawn. I leave the last steps to you.".into(),
                    },
                    Step::Hide { slot: Slot::Centre },
                    Step::Narrate {
                        text: "The guide disappears; the road remains.".into(),
                    },
                    Step::End,
                ],
            },
        }
    }

    fn name(self) -> &'static str {
        match self {
            SceneId::Crossroads => "crossroads",
            SceneId::Lantern => "lantern",
            SceneId::River => "river",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "crossroads" => Some(SceneId::Crossroads),
            "lantern" => Some(SceneId::Lantern),
            "river" => Some(SceneId::River),
            _ => None,
        }
    }
}

fn text_en(key: TextKey) -> &'static str {
    match key.0 {
        "menu.title" => "Two Roads",
        "menu.begin" => "Begin",
        "menu.continue" => "Continue",
        "menu.quit" => "Quit",
        "menu.return" => "Return to the menu",
        _ => "<missing text>",
    }
}

fn plugin() -> VnPlugin<SceneId> {
    VnPlugin {
        start: SceneId::Crossroads,
        locale: Locale { resolve: text_en },
        cast: cast(),
        tuning: Presentation::default(),
        places: Places::default(),
    }
}

fn cast() -> Cast {
    Cast(BTreeMap::from([(
        "guide".into(),
        CastMember {
            name: "The Guide".into(),
            image: "characters/guide.png".into(),
            scale: 0.78,
        },
    )]))
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Two Roads — vn_core reuse probe".into(),
                resolution: WindowResolution::new(960, 600),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.025, 0.03, 0.045)))
        .insert_resource(SaveSlots::for_game("VnCoreReuseProbe"))
        .add_plugins(plugin())
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use std::path::Path;
    use vn_core::present::{BackgroundSprite, CharacterSprite, MusicPlayer};

    #[test]
    fn names_round_trip_and_every_scene_has_content() {
        for id in [SceneId::Crossroads, SceneId::Lantern, SceneId::River] {
            assert_eq!(SceneId::from_name(id.name()), Some(id));
            assert!(!id.scene().steps.is_empty());
        }
    }

    #[test]
    fn every_probe_asset_exists() {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for path in [
            "backgrounds/crossroads.webp",
            "characters/guide.png",
            "music/road.ogg",
        ] {
            assert!(assets.join(path).is_file(), "missing probe asset `{path}`");
        }
    }

    #[test]
    fn both_choices_reach_an_ending_and_hide_changes_the_stage() {
        fn at_crossroads() -> (StoryState<SceneId>, Stage, CurrentLine, PendingChoices) {
            (
                StoryState::new(SceneId::Crossroads),
                Stage::default(),
                CurrentLine::default(),
                PendingChoices::default(),
            )
        }

        let (mut state, mut stage, mut line, mut choices) = at_crossroads();
        assert!(matches!(
            advance(&mut state, &mut stage, &mut line, &mut choices),
            Flow::Playing(PlayMode::Revealing)
        ));
        assert!(matches!(
            advance(&mut state, &mut stage, &mut line, &mut choices),
            Flow::Playing(PlayMode::AwaitingChoice)
        ));
        assert!(take_choice(0, &mut state));
        assert_eq!(state.scene().id, SceneId::Lantern);
        advance(&mut state, &mut stage, &mut line, &mut choices);
        assert!(matches!(
            advance(&mut state, &mut stage, &mut line, &mut choices),
            Flow::Ended
        ));

        let (mut state, mut stage, mut line, mut choices) = at_crossroads();
        advance(&mut state, &mut stage, &mut line, &mut choices);
        advance(&mut state, &mut stage, &mut line, &mut choices);
        assert!(take_choice(1, &mut state));
        assert_eq!(state.scene().id, SceneId::River);
        advance(&mut state, &mut stage, &mut line, &mut choices);
        assert!(stage.characters.contains_key(&Slot::Centre));
        advance(&mut state, &mut stage, &mut line, &mut choices);
        assert!(
            stage.characters.is_empty(),
            "the river ending's Hide step must remove the guide"
        );
        assert!(matches!(
            advance(&mut state, &mut stage, &mut line, &mut choices),
            Flow::Ended
        ));
    }

    #[test]
    fn the_real_plugin_runs_and_starts_this_games_story() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .init_asset::<Image>()
            .init_asset::<bevy::audio::AudioSource>()
            .insert_resource(SaveSlots {
                root: std::env::temp_dir().join("vn_core_reuse_probe_test"),
            })
            .add_plugins(plugin());

        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<CurrentLine>().full,
            "At the crossroads, two quiet paths remain."
        );
        assert_eq!(
            app.world().resource::<StoryState<SceneId>>().scene().id,
            SceneId::Crossroads
        );

        let stage = app.world().resource::<Stage>();
        assert_eq!(
            stage.background.as_deref(),
            Some("backgrounds/crossroads.webp")
        );
        assert_eq!(stage.music.as_deref(), Some("music/road.ogg"));
        assert_eq!(
            stage.characters.get(&Slot::Centre).map(String::as_str),
            Some("guide")
        );

        assert_eq!(
            app.world_mut()
                .query::<&BackgroundSprite>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&CharacterSprite>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&MusicPlayer>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
