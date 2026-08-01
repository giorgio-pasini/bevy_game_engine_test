//! Presentation: sprites, dialogue box, choice menu, music.
//!
//! Every system here is a *reader* of story state and a writer of exactly one
//! visual concern. Nothing here decides what happens next.

use bevy::prelude::*;
use std::collections::BTreeMap;

use crate::story::*;

/// Design-space size. The camera shows exactly this, letterboxed.
pub const DESIGN_W: f32 = 1920.0;
pub const DESIGN_H: f32 = 1080.0;

const BOX_HEIGHT: f32 = 260.0;

// z-order: background behind, characters over it, UI is a separate pass.
const Z_BACKGROUND: f32 = 0.0;
const Z_CHARACTER: f32 = 1.0;

#[derive(Component)]
pub struct BackgroundSprite;

#[derive(Component)]
pub struct CharacterSprite(pub Slot);

#[derive(Component)]
pub struct SpeakerText;

#[derive(Component)]
pub struct BodyText;

#[derive(Component)]
pub struct ContinueHint;

#[derive(Component)]
pub struct ChoiceRoot;

#[derive(Component)]
pub struct ChoiceButton(pub usize);

#[derive(Component)]
pub struct MusicPlayer;

/// A non-story button. Story choices use [`ChoiceButton`] instead, because they
/// carry an index into the current branch rather than a fixed action.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum UiButton {
    StartStory,
    ReturnToMenu,
    Quit,
}

/// Shared styling so the menu, the end screen and story choices read as one
/// interface rather than three.
fn menu_button(action: UiButton, label: &str) -> impl Bundle + use<> {
    (
        Button,
        Node {
            width: Val::Px(420.0),
            padding: UiRect::all(Val::Px(16.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.10, 0.12, 0.18, 0.92)),
        BorderColor::all(Color::srgb(0.98, 0.82, 0.36)),
        action,
        children![(
            Text::new(label.to_string()),
            TextFont {
                font_size: FontSize::Px(24.0),
                ..default()
            },
            TextColor(Color::srgb(0.94, 0.95, 0.97)),
        )],
    )
}

pub fn spawn_menu(mut commands: Commands, locale: Res<Locale>) {
    commands.spawn((
        DespawnOnExit(AppState::MainMenu),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(28.0),
            ..default()
        },
        children![
            (
                Text::new((locale.resolve)(TextKey("menu.title")).to_string()),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.82, 0.36)),
            ),
            menu_button(UiButton::StartStory, (locale.resolve)(TextKey("menu.begin"))),
            menu_button(UiButton::Quit, (locale.resolve)(TextKey("menu.quit"))),
        ],
    ));
}

/// Shown when the story reaches `Step::End`.
pub fn spawn_end_screen(mut commands: Commands, locale: Res<Locale>) {
    commands.spawn((
        DespawnOnExit(PlayMode::Finished),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(300.0),
            left: Val::Px(0.0),
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },
        children![menu_button(
            UiButton::ReturnToMenu,
            (locale.resolve)(TextKey("menu.return"))
        )],
    ));
}

/// Tracks what the presentation has already applied, so each reconcile system
/// can tell what actually changed rather than rebuilding every frame.
#[derive(Resource, Default)]
pub struct AppliedStage {
    pub background: Option<&'static str>,
    pub characters: BTreeMap<Slot, &'static str>,
    pub music: Option<&'static str>,
}

/// Spawned on entering `Playing` and torn down automatically on leaving it.
pub fn spawn_ui(mut commands: Commands) {
    // Dialogue box, pinned to the bottom of the design area.
    commands.spawn((
        DespawnOnExit(AppState::Playing),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            width: percent(100),
            height: Val::Px(BOX_HEIGHT),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(28.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.82)),
        children![
            (
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(30.0),
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.82, 0.36)),
                SpeakerText,
            ),
            (
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.93, 0.95)),
                BodyText,
            ),
        ],
    ));

    // Continue indicator, bottom-right.
    commands.spawn((
        DespawnOnExit(AppState::Playing),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(16.0),
            right: Val::Px(28.0),
            ..default()
        },
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.65, 0.68, 0.74)),
        ContinueHint,
    ));
}

/// Background: one sprite, replaced when the story asks for a different image.
///
/// `FillCenter` is what implements the "square art in a 16:9 frame" decision —
/// it scales to cover and centre-crops rather than distorting.
pub fn sync_background(
    mut commands: Commands,
    stage: Res<Stage>,
    mut applied: ResMut<AppliedStage>,
    assets: Res<AssetServer>,
    existing: Query<Entity, With<BackgroundSprite>>,
) {
    if applied.background == stage.background {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if let Some(path) = stage.background {
        commands.spawn((
            Sprite {
                image: assets.load(path),
                custom_size: Some(Vec2::new(DESIGN_W, DESIGN_H)),
                image_mode: SpriteImageMode::Scale(SpriteScalingMode::FillCenter),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, Z_BACKGROUND),
            BackgroundSprite,
        ));
    }
    applied.background = stage.background;
}

pub fn sync_characters(
    mut commands: Commands,
    stage: Res<Stage>,
    mut applied: ResMut<AppliedStage>,
    assets: Res<AssetServer>,
    existing: Query<(Entity, &CharacterSprite)>,
) {
    if applied.characters == stage.characters {
        return;
    }
    for (e, _) in &existing {
        commands.entity(e).despawn();
    }
    for (slot, path) in &stage.characters {
        let x = match slot {
            Slot::Left => -DESIGN_W * 0.27,
            Slot::Centre => 0.0,
            Slot::Right => DESIGN_W * 0.27,
        };
        // Sits above the dialogue box rather than behind it.
        let size = DESIGN_H * 0.86;
        commands.spawn((
            Sprite {
                image: assets.load(*path),
                custom_size: Some(Vec2::splat(size)),
                ..default()
            },
            Transform::from_xyz(x, BOX_HEIGHT * 0.5, Z_CHARACTER),
            CharacterSprite(*slot),
        ));
    }
    applied.characters = stage.characters.clone();
}

pub fn sync_music(
    mut commands: Commands,
    stage: Res<Stage>,
    mut applied: ResMut<AppliedStage>,
    assets: Res<AssetServer>,
    existing: Query<Entity, With<MusicPlayer>>,
) {
    if applied.music == stage.music {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if let Some(path) = stage.music {
        commands.spawn((
            AudioPlayer::new(assets.load(path)),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: bevy::audio::Volume::Linear(0.5),
                ..default()
            },
            MusicPlayer,
        ));
    }
    applied.music = stage.music;
}

/// Typewriter. Reveals by *count*, never by mutating the source string — so
/// "skip to full line" and the backlog stay trivial.
pub fn reveal_text(
    time: Res<Time>,
    mut line: ResMut<CurrentLine>,
    mode: Res<State<PlayMode>>,
    mut next: ResMut<NextState<PlayMode>>,
) {
    if *mode.get() != PlayMode::Revealing {
        return;
    }
    line.timer += time.delta_secs() * TEXT_SPEED;
    let target = line.timer as usize;
    if target > line.revealed {
        line.revealed = target;
    }
    if line.is_complete() {
        line.complete();
        next.set(PlayMode::AwaitingInput);
    }
}

pub fn draw_dialogue(
    line: Res<CurrentLine>,
    mode: Res<State<PlayMode>>,
    mut speaker: Query<&mut Text, (With<SpeakerText>, Without<BodyText>, Without<ContinueHint>)>,
    mut body: Query<&mut Text, (With<BodyText>, Without<SpeakerText>, Without<ContinueHint>)>,
    mut hint: Query<&mut Text, (With<ContinueHint>, Without<SpeakerText>, Without<BodyText>)>,
) {
    if let Ok(mut t) = speaker.single_mut() {
        t.0 = line.speaker.unwrap_or("").to_string();
    }
    if let Ok(mut t) = body.single_mut() {
        t.0 = line.visible();
    }
    if let Ok(mut t) = hint.single_mut() {
        t.0 = match *mode.get() {
            PlayMode::AwaitingInput => "click / space  ▸".to_string(),
            PlayMode::Finished => "the end".to_string(),
            _ => String::new(),
        };
    }
}

pub fn spawn_choices(mut commands: Commands, choices: Res<PendingChoices>, locale: Res<Locale>) {
    if choices.items.is_empty() {
        return;
    }
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(BOX_HEIGHT + 40.0),
                left: Val::Px(0.0),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            ChoiceRoot,
        ))
        .with_children(|parent| {
            for (key, index) in &choices.items {
                parent
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(760.0),
                            padding: UiRect::all(Val::Px(16.0)),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.12, 0.18, 0.92)),
                        BorderColor::all(Color::srgb(0.98, 0.82, 0.36)),
                        ChoiceButton(*index),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new((locale.resolve)(*key)),
                            TextFont {
                                font_size: FontSize::Px(24.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.94, 0.95, 0.97)),
                        ));
                    });
            }
        });
}

pub fn highlight_choices(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut colour) in &mut buttons {
        *colour = match interaction {
            Interaction::Hovered | Interaction::Pressed => {
                BackgroundColor(Color::srgba(0.22, 0.26, 0.36, 0.96))
            }
            Interaction::None => BackgroundColor(Color::srgba(0.10, 0.12, 0.18, 0.92)),
        };
    }
}

pub fn despawn_choices(mut commands: Commands, roots: Query<Entity, With<ChoiceRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}
