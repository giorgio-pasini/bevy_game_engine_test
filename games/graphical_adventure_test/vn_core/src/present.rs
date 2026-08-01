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

/// A character on stage. Carries the cast id so the speaker cue can tell which
/// sprite is talking — the link that did not exist when `Show` took a path.
#[derive(Component)]
pub struct CharacterSprite {
    pub slot: Slot,
    pub id: String,
}

/// Height a character occupies, in design units. Width follows from the image's
/// real aspect ratio, so portrait and square art both render undistorted.
const CHARACTER_HEIGHT: f32 = DESIGN_H * 0.82;

/// How strongly listeners are dimmed, and how far the speaker rises.
const DIM: f32 = 0.45;
const SPEAKER_RISE: f32 = 26.0;
const SPEAKER_SCALE: f32 = 1.04;
/// Units per second for the eased transition.
const CUE_SPEED: f32 = 7.0;

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
    pub background: Option<String>,
    pub characters: BTreeMap<Slot, String>,
    pub music: Option<String>,
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
    if let Some(path) = stage.background.as_ref() {
        commands.spawn((
            Sprite {
                image: assets.load(path.clone()),
                custom_size: Some(Vec2::new(DESIGN_W, DESIGN_H)),
                image_mode: SpriteImageMode::Scale(SpriteScalingMode::FillCenter),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, Z_BACKGROUND),
            BackgroundSprite,
        ));
    }
    applied.background = stage.background.clone();
}

pub fn sync_characters(
    mut commands: Commands,
    stage: Res<Stage>,
    cast: Res<Cast>,
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
    for (slot, id) in &stage.characters {
        let Some(path) = cast.image_of(id) else {
            // build.rs guarantees this cannot happen in a built game.
            warn!("character `{id}` is not in the cast");
            continue;
        };
        let x = match slot {
            Slot::Left => -DESIGN_W * 0.26,
            Slot::Centre => 0.0,
            Slot::Right => DESIGN_W * 0.26,
        };
        commands.spawn((
            Sprite {
                image: assets.load(path.to_string()),
                // Left unset here: `fit_characters` sets it once the texture's
                // real dimensions are known, so art of any aspect ratio renders
                // undistorted.
                ..default()
            },
            Transform::from_xyz(x, BOX_HEIGHT * 0.5, Z_CHARACTER),
            CharacterSprite {
                slot: *slot,
                id: id.clone(),
            },
        ));
    }
    applied.characters = stage.characters.clone();
}

/// Sizes each character to a fixed height, preserving the image's aspect ratio.
///
/// Deriving this from the loaded texture rather than hand-authoring it means
/// replacing art of a different shape needs no code or data change. Runs until
/// the texture is available, then leaves the sprite alone.
pub fn fit_characters(
    images: Res<Assets<Image>>,
    mut sprites: Query<&mut Sprite, With<CharacterSprite>>,
) {
    for mut sprite in &mut sprites {
        if sprite.custom_size.is_some() {
            continue;
        }
        let Some(image) = images.get(&sprite.image) else {
            continue; // still loading
        };
        let size = image.size();
        if size.y == 0 {
            continue;
        }
        let aspect = size.x as f32 / size.y as f32;
        sprite.custom_size = Some(Vec2::new(CHARACTER_HEIGHT * aspect, CHARACTER_HEIGHT));
    }
}

/// Dims listeners; brightens, raises and slightly enlarges the speaker.
///
/// Eased rather than snapped, so it reads as attention shifting rather than a
/// flicker. This is the only system that writes character tint or scale.
pub fn speaker_cue(
    time: Res<Time>,
    line: Res<CurrentLine>,
    mut sprites: Query<(&CharacterSprite, &mut Sprite, &mut Transform)>,
) {
    let t = (time.delta_secs() * CUE_SPEED).clamp(0.0, 1.0);

    for (character, mut sprite, mut transform) in &mut sprites {
        let speaking = line.speaker.as_deref() == Some(character.id.as_str());

        // With nobody speaking (narration), everyone sits at full brightness so
        // the scene does not look uniformly greyed out.
        let anybody_speaking = line.speaker.is_some();
        let target_tint = if speaking || !anybody_speaking { 1.0 } else { DIM };
        let target_rise = if speaking { SPEAKER_RISE } else { 0.0 };
        let target_scale = if speaking { SPEAKER_SCALE } else { 1.0 };

        let current = sprite.color.to_linear().red;
        let tint = current + (target_tint - current) * t;
        sprite.color = Color::linear_rgb(tint, tint, tint);

        let base_y = BOX_HEIGHT * 0.5;
        let y = transform.translation.y;
        transform.translation.y = y + ((base_y + target_rise) - y) * t;

        let s = transform.scale.x;
        let ns = s + (target_scale - s) * t;
        transform.scale = Vec3::new(ns, ns, 1.0);
    }
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
    if let Some(path) = stage.music.as_ref() {
        commands.spawn((
            AudioPlayer::new(assets.load(path.clone())),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Loop,
                volume: bevy::audio::Volume::Linear(0.5),
                ..default()
            },
            MusicPlayer,
        ));
    }
    applied.music = stage.music.clone();
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
    cast: Res<Cast>,
    mode: Res<State<PlayMode>>,
    mut speaker: Query<&mut Text, (With<SpeakerText>, Without<BodyText>, Without<ContinueHint>)>,
    mut body: Query<&mut Text, (With<BodyText>, Without<SpeakerText>, Without<ContinueHint>)>,
    mut hint: Query<&mut Text, (With<ContinueHint>, Without<SpeakerText>, Without<BodyText>)>,
) {
    if let Ok(mut t) = speaker.single_mut() {
        // `line.speaker` is a cast id; the box shows the display name.
        t.0 = line
            .speaker
            .as_deref()
            .map(|id| cast.name_of(id).to_string())
            .unwrap_or_default();
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

pub fn spawn_choices(mut commands: Commands, choices: Res<PendingChoices>) {
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
            for (label, index) in &choices.items {
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
                            Text::new(label.clone()),
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
