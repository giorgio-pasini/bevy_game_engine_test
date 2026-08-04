//! Demo game — a vertical slice proving `vn_core` end to end.
//!
//! Run with: `cargo run -p demo_game --features dev`

mod scenes;
#[cfg(test)]
mod app_tests;
#[cfg(test)]
mod doc_tests;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use vn_core::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Golden Golem — vn_core vertical slice".into(),
                // Deliberately not 16:9, so the letterbox is visible from the
                // first frame rather than only after a manual resize.
                resolution: WindowResolution::new(1280, 800),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.03)))
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
        })
        .run();
}
