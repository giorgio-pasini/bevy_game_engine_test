//! A simple grid-based Snake game, written to exercise Bevy 0.19.
//!
//! The snake is a `Vec<Entity>` (head first) plus a `Pos` grid coordinate on
//! each segment. Movement is a discrete tick: the head steps one cell and every
//! other segment inherits the cell the segment ahead of it just left. Rendering
//! is a separate concern — `sync_transforms` is the only system that touches
//! `Transform`, deriving it from `Pos`.

use bevy::prelude::*;
use bevy::window::WindowResolution;

use chacha20::ChaCha8Rng;
use rand::RngExt;

// ---------------------------------------------------------------- tuning ----

const GRID_W: i32 = 20;
const GRID_H: i32 = 20;
/// World units per grid cell. With the default 2D camera these are pixels.
const CELL: f32 = 28.0;
/// Shrinks each sprite slightly so cells read as separate blocks.
const CELL_INSET: f32 = 3.0;
/// Seconds between snake steps. Lower is harder.
const TICK: f32 = 0.15;

const BG_COLOR: Color = Color::srgb(0.08, 0.09, 0.11);
const ARENA_COLOR: Color = Color::srgb(0.13, 0.14, 0.17);
const HEAD_COLOR: Color = Color::srgb(0.45, 0.87, 0.45);
const BODY_COLOR: Color = Color::srgb(0.24, 0.58, 0.31);
const FOOD_COLOR: Color = Color::srgb(0.92, 0.36, 0.42);
const TEXT_COLOR: Color = Color::srgb(0.86, 0.88, 0.91);

// ------------------------------------------------------------ state/types ----

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, States)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

/// A cell on the board. `(0, 0)` is bottom-left.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
struct Pos {
    x: i32,
    y: i32,
}

#[derive(Component)]
struct Food;

#[derive(Component)]
struct ScoreUi;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, 1),
            Dir::Down => (0, -1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }

    fn is_opposite(self, other: Dir) -> bool {
        matches!(
            (self, other),
            (Dir::Up, Dir::Down)
                | (Dir::Down, Dir::Up)
                | (Dir::Left, Dir::Right)
                | (Dir::Right, Dir::Left)
        )
    }
}

#[derive(Resource)]
struct Snake {
    /// Segment entities, head first.
    body: Vec<Entity>,
    dir: Dir,
    /// Buffered input, applied on the next tick. Buffering means a keypress
    /// between ticks is never dropped, and prevents turning twice within one
    /// tick (which would let the snake reverse into itself).
    next_dir: Dir,
}

#[derive(Resource, Deref, DerefMut)]
struct GameRng(ChaCha8Rng);

#[derive(Resource, Deref, DerefMut)]
struct Tick(Timer);

#[derive(Resource, Default)]
struct Score(u32);

// ------------------------------------------------------------------- app ----

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Snake".into(),
                // NOTE: 0.19 takes integers here, not floats.
                resolution: WindowResolution::new(
                    GRID_W as u32 * CELL as u32 + 80,
                    GRID_H as u32 * CELL as u32 + 80,
                ),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(BG_COLOR))
        .insert_resource(Score::default())
        .insert_resource(GameRng(rand::make_rng()))
        .insert_resource(Tick(Timer::from_seconds(TICK, TimerMode::Repeating)))
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Playing), spawn_snake)
        .add_systems(
            Update,
            (read_input, snake_tick, sync_transforms)
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(Update, update_score_text)
        .add_systems(OnEnter(GameState::GameOver), show_game_over)
        .add_systems(
            Update,
            restart_on_space.run_if(in_state(GameState::GameOver)),
        )
        .run();
}

/// Grid coordinate -> world translation, centring the board on the origin.
fn world_pos(p: Pos, z: f32) -> Vec3 {
    let ox = (GRID_W as f32 - 1.0) * 0.5;
    let oy = (GRID_H as f32 - 1.0) * 0.5;
    Vec3::new((p.x as f32 - ox) * CELL, (p.y as f32 - oy) * CELL, z)
}

/// Camera, arena backdrop and score readout. These outlive a game over, so
/// they are spawned once at startup rather than per round.
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Arena backdrop, so the playfield edges are visible.
    commands.spawn((
        Sprite::from_color(
            ARENA_COLOR,
            Vec2::new(GRID_W as f32 * CELL, GRID_H as f32 * CELL),
        ),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    commands.spawn((
        Text::new("Score: "),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextColor(TEXT_COLOR),
        ScoreUi,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(14.0),
            ..default()
        },
        children![(
            TextSpan::default(),
            TextFont {
                font_size: FontSize::Px(26.0),
                ..default()
            },
            TextColor(TEXT_COLOR),
        )],
    ));
}

fn spawn_segment(commands: &mut Commands, p: Pos, is_head: bool) -> Entity {
    commands
        .spawn((
            p,
            // Tie lifetime to the round: leaving `Playing` clears the board.
            DespawnOnExit(GameState::Playing),
            Sprite::from_color(
                if is_head { HEAD_COLOR } else { BODY_COLOR },
                Vec2::splat(CELL - CELL_INSET),
            ),
            Transform::from_translation(world_pos(p, if is_head { 2.0 } else { 1.5 })),
        ))
        .id()
}

/// Places food on a uniformly random *free* cell. Rejection sampling would
/// degrade as the snake fills the board, so we enumerate the free cells instead.
fn spawn_food(commands: &mut Commands, rng: &mut ChaCha8Rng, occupied: &[Pos]) {
    let free: Vec<Pos> = (0..GRID_W)
        .flat_map(|x| (0..GRID_H).map(move |y| Pos { x, y }))
        .filter(|p| !occupied.contains(p))
        .collect();

    // Board full — the player has effectively won; nothing left to place.
    if free.is_empty() {
        return;
    }

    let p = free[rng.random_range(0..free.len())];
    commands.spawn((
        Food,
        p,
        DespawnOnExit(GameState::Playing),
        Sprite::from_color(FOOD_COLOR, Vec2::splat(CELL - CELL_INSET)),
        Transform::from_translation(world_pos(p, 1.0)),
    ));
}

fn spawn_snake(mut commands: Commands, mut score: ResMut<Score>, mut rng: ResMut<GameRng>, mut tick: ResMut<Tick>) {
    score.0 = 0;
    tick.reset();

    let start = Pos {
        x: GRID_W / 2,
        y: GRID_H / 2,
    };
    let body = vec![
        spawn_segment(&mut commands, start, true),
        spawn_segment(&mut commands, Pos { x: start.x - 1, y: start.y }, false),
        spawn_segment(&mut commands, Pos { x: start.x - 2, y: start.y }, false),
    ];

    let occupied = [
        start,
        Pos { x: start.x - 1, y: start.y },
        Pos { x: start.x - 2, y: start.y },
    ];
    spawn_food(&mut commands, &mut rng.0, &occupied);

    commands.insert_resource(Snake {
        body,
        dir: Dir::Right,
        next_dir: Dir::Right,
    });
}

fn read_input(keys: Res<ButtonInput<KeyCode>>, mut snake: ResMut<Snake>) {
    let requested = if keys.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
        Some(Dir::Up)
    } else if keys.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
        Some(Dir::Down)
    } else if keys.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
        Some(Dir::Left)
    } else if keys.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]) {
        Some(Dir::Right)
    } else {
        None
    };

    // Compare against `dir`, not `next_dir`: the snake may only reverse
    // relative to the direction it has actually travelled.
    if let Some(d) = requested {
        if !d.is_opposite(snake.dir) {
            snake.next_dir = d;
        }
    }
}

fn snake_tick(
    time: Res<Time>,
    mut tick: ResMut<Tick>,
    mut commands: Commands,
    mut snake: ResMut<Snake>,
    mut score: ResMut<Score>,
    mut rng: ResMut<GameRng>,
    mut next_state: ResMut<NextState<GameState>>,
    // `Without<Food>` is load-bearing: food carries `Pos` too, so without it
    // this mutable query overlaps the `food` query below and Bevy panics with
    // B0001 at runtime. `cargo check` cannot catch that — only running can.
    mut positions: Query<&mut Pos, Without<Food>>,
    food: Query<(Entity, &Pos), With<Food>>,
) {
    if !tick.tick(time.delta()).is_finished() {
        return;
    }

    snake.dir = snake.next_dir;
    let (dx, dy) = snake.dir.delta();

    // Snapshot every segment's cell before anything moves.
    let old: Vec<Pos> = snake
        .body
        .iter()
        .filter_map(|e| positions.get(*e).ok().copied())
        .collect();

    // A despawn mid-round would desync body and positions; bail rather than panic.
    if old.len() != snake.body.len() {
        return;
    }

    let head = old[0];
    let new_head = Pos {
        x: head.x + dx,
        y: head.y + dy,
    };

    if new_head.x < 0 || new_head.x >= GRID_W || new_head.y < 0 || new_head.y >= GRID_H {
        next_state.set(GameState::GameOver);
        return;
    }

    // Self collision. The final segment is excluded because it vacates its cell
    // on this same tick — moving into it is legal.
    if old[..old.len() - 1].contains(&new_head) {
        next_state.set(GameState::GameOver);
        return;
    }

    // Each segment inherits the cell of the one ahead of it.
    for (i, e) in snake.body.iter().enumerate() {
        let target = if i == 0 { new_head } else { old[i - 1] };
        if let Ok(mut p) = positions.get_mut(*e) {
            *p = target;
        }
    }

    let eaten = food.iter().find(|(_, p)| **p == new_head);

    if let Some((food_entity, _)) = eaten {
        commands.entity(food_entity).despawn();
        score.0 += 1;

        // Grow into the cell the tail just left, so length increases without
        // displacing the body.
        let tail = *old.last().unwrap();
        let grown = spawn_segment(&mut commands, tail, false);
        snake.body.push(grown);

        let mut occupied = vec![new_head];
        occupied.extend_from_slice(&old[..old.len() - 1]);
        occupied.push(tail);
        spawn_food(&mut commands, &mut rng.0, &occupied);
    }
}

/// The single place `Transform` is derived from grid state.
fn sync_transforms(mut q: Query<(&Pos, &mut Transform), Changed<Pos>>) {
    for (p, mut transform) in &mut q {
        let z = transform.translation.z;
        transform.translation = world_pos(*p, z);
    }
}

fn update_score_text(
    score: Res<Score>,
    root: Single<Entity, (With<ScoreUi>, With<Text>)>,
    mut writer: TextUiWriter,
) {
    *writer.text(*root, 1) = score.0.to_string();
}

fn show_game_over(mut commands: Commands, score: Res<Score>) {
    commands.spawn((
        DespawnOnExit(GameState::GameOver),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            ..default()
        },
        children![
            (
                Text::new(format!("Game Over — score {}", score.0)),
                TextFont {
                    font_size: FontSize::Px(44.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
            ),
            (
                Text::new("press SPACE to play again"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(TEXT_COLOR),
            ),
        ],
    ));
}

fn restart_on_space(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Playing);
    }
}
