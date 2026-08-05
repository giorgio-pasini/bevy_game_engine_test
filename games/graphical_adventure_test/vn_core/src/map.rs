//! Colour-keyed map hotspots: clicking a *region of a picture* rather than a
//! button.
//!
//! A place may name a **lookup image** alongside its background. The lookup is
//! never drawn — each clickable area is a flat colour in it, and an option
//! naming that colour is what the player selects by clicking there. Nothing
//! authors coordinates, so a region can be any shape at all, and moving a
//! mountain in the art means repainting the same shape in the lookup rather
//! than editing numbers in a file.
//!
//! Two properties make this safe rather than fragile, and both are enforced by
//! `build.rs` rather than trusted:
//!
//! * the lookup is **losslessly encoded** — a lossy format resamples the flat
//!   colours and the keys stop matching exactly;
//! * the lookup contains **only declared colours** — anti-aliased edges produce
//!   thousands of in-between shades, each of which is a pixel that silently
//!   does nothing.
//!
//! This module is deliberately split from `present.rs`: here is geometry and
//! input, there is what the player sees.

use bevy::prelude::*;

use crate::place::*;
use crate::present::{DESIGN_H, DESIGN_W};
use crate::story::{SceneKey, StoryState};

/// The lookup image for the place currently being explored.
///
/// Keyed by place id rather than by a "dirty" flag: the same question the
/// exploration view answers by looking at what is on screen. Asking what place
/// we are in cannot go stale; remembering that we already loaded something can.
#[derive(Resource, Default)]
pub struct MapLookup {
    pub place: Option<String>,
    pub handle: Option<Handle<Image>>,
}

/// The option under the cursor, if the cursor is over a region whose condition
/// currently holds. Recomputed every frame from the image — never accumulated.
#[derive(Resource, Default)]
pub struct HoveredHotspot(pub Option<usize>);

/// Reads the colour of the lookup pixel under a design-space point.
///
/// The design area is a fixed 1920x1080 rectangle centred on the origin, and
/// the lookup is sampled in **normalised** coordinates — so it may be any
/// resolution, and only its aspect ratio has to match. That is what makes a
/// low-resolution placeholder and finished art interchangeable without touching
/// a line of data.
///
/// Pure on purpose: this is the half of hotspot handling worth testing, and it
/// needs no window, no GPU and no asset server to run.
pub fn sample_design_point(image: &Image, x: f32, y: f32) -> Option<Rgb> {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return None;
    }
    // y is up in the world and down in an image, hence the subtraction.
    let u = (x + DESIGN_W * 0.5) / DESIGN_W;
    let v = (DESIGN_H * 0.5 - y) / DESIGN_H;
    if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
        return None;
    }
    let px = ((u * w as f32) as u32).min(w - 1);
    let py = ((v * h as f32) as u32).min(h - 1);
    let c = image.get_color_at(px, py).ok()?.to_srgba();
    // Back to bytes: the file is 8-bit sRGB, so this round-trips exactly, and
    // exact equality is what a colour key needs.
    Some(Rgb(
        (c.red * 255.0).round() as u8,
        (c.green * 255.0).round() as u8,
        (c.blue * 255.0).round() as u8,
    ))
}

/// Loads (or drops) the lookup image when the place changes.
pub fn sync_lookup<S: SceneKey>(
    state: Res<StoryState<S>>,
    places: Res<Places>,
    assets: Res<AssetServer>,
    mut lookup: ResMut<MapLookup>,
    mut hovered: ResMut<HoveredHotspot>,
) {
    let current = state.current_place().map(str::to_string);
    if lookup.place == current {
        return;
    }
    hovered.0 = None;
    lookup.handle = current
        .as_deref()
        .and_then(|id| places.get(id))
        .and_then(|place| place.lookup.as_ref())
        .map(|path| assets.load(path.clone()));
    lookup.place = current;
}

/// Resolves what the cursor is pointing at. The only writer of
/// [`HoveredHotspot`].
pub fn hover_hotspot<S: SceneKey>(
    state: Res<StoryState<S>>,
    places: Res<Places>,
    lookup: Res<MapLookup>,
    images: Res<Assets<Image>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    buttons: Query<&Interaction>,
    mut hovered: ResMut<HoveredHotspot>,
) {
    // Cleared first, so every early return below leaves "nothing hovered"
    // rather than whatever was true last frame.
    hovered.0 = None;

    let Some(place_id) = state.current_place() else {
        return;
    };
    let Some(handle) = lookup.handle.as_ref() else {
        return;
    };
    let Some(image) = images.get(handle) else {
        return; // still loading
    };
    // A place may show buttons over its map. The button wins: without this, one
    // click would both press it and fire whatever region sits underneath.
    if buttons.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = windows.single() else {
        return; // headless
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    // Goes through the camera rather than through window size arithmetic of our
    // own, so the letterbox bars are accounted for by the same code that draws
    // them.
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };
    let Some(colour) = sample_design_point(image, world.x, world.y) else {
        return;
    };
    hovered.0 = places.option_for_region(place_id, colour, &state.flags, &state.items);
}

/// Turns a click on a region into a picked option — the same currency a button
/// press produces, so both end up in one place that applies the effect.
pub fn click_hotspot(
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredHotspot>,
    mut picked: ResMut<PickedOption>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if let Some(index) = hovered.0 {
        picked.0 = Some(index);
    }
}
