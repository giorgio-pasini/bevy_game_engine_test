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

/// Reports systems whose relative order is undefined — they touch the same data
/// and nothing sequences them.
///
/// Bevy runs conflicting systems serially, so an ambiguity is not a data race.
/// It is worse in one respect: the order is *arbitrary but stable enough to look
/// deliberate*, and it can flip the day someone adds a system. Off by default
/// (`LogLevel::Ignore`), so it has to be asked for.
///
/// Dev-only: the report names Bevy's own systems too, which is noise in a
/// shipped log.
#[cfg(feature = "dev")]
fn detect_ambiguities(app: &mut App) {
    use bevy::ecs::schedule::{LogLevel, ScheduleBuildSettings};

    app.edit_schedule(Update, |schedule| {
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Warn,
            ..default()
        });
    });
}

fn main() {
    let mut app = App::new();

    #[cfg(feature = "dev")]
    detect_ambiguities(&mut app);

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
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
    // The game names its own save folder; the library only supplies the
    // convention. On Windows this is %APPDATA%\GoldenGolem\saves — a per-user
    // path that does not move when the player launches the game differently.
    .insert_resource(SaveSlots::for_game("GoldenGolem"))
    .add_plugins(VnPlugin {
        start: scenes::START,
        locale: Locale {
            resolve: scenes::text_en,
        },
        cast: scenes::cast(),
        tuning: scenes::presentation(),
        places: scenes::places(),
    })
    .run();
}
