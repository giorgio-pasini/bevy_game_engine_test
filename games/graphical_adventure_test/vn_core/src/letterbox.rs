//! Fixed 16:9 presentation, letterboxed on any other window aspect.
//!
//! Two pieces working together:
//!   * `ScalingMode::FixedVertical` keeps the world exactly `DESIGN_H` tall, so
//!     coordinates in scenes mean the same thing at every window size.
//!   * `Camera::viewport` restricts rendering to a centred 16:9 rectangle, which
//!     is what actually produces the black bars.

// Both live in `bevy::camera` in 0.19 — `bevy::render::camera::Viewport` is
// private and will not compile.
use bevy::camera::{ScalingMode, Viewport};
use bevy::prelude::*;

use crate::present::{DESIGN_H, DESIGN_W};

const ASPECT: f32 = DESIGN_W / DESIGN_H;

pub(crate) fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: DESIGN_H,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

/// Recomputes the letterbox rectangle. Cheap enough to run every frame, which
/// avoids having to catch every resize event.
pub(crate) fn fit_letterbox(windows: Query<&Window>, mut cameras: Query<&mut Camera>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = window.resolution.physical_size();
    if size.x == 0 || size.y == 0 {
        return;
    }

    let win_aspect = size.x as f32 / size.y as f32;
    let (w, h) = if win_aspect > ASPECT {
        // Window is wider than 16:9 — bars on the left and right.
        (((size.y as f32) * ASPECT) as u32, size.y)
    } else {
        // Taller than 16:9 — bars on top and bottom.
        (size.x, ((size.x as f32) / ASPECT) as u32)
    };

    // A zero-sized or oversized viewport makes wgpu unhappy, so clamp.
    let w = w.clamp(1, size.x);
    let h = h.clamp(1, size.y);
    let pos = UVec2::new((size.x - w) / 2, (size.y - h) / 2);

    for mut camera in &mut cameras {
        let wanted = Viewport {
            physical_position: pos,
            physical_size: UVec2::new(w, h),
            ..default()
        };
        let changed = camera
            .viewport
            .as_ref()
            .is_none_or(|v| v.physical_position != wanted.physical_position
                || v.physical_size != wanted.physical_size);
        if changed {
            camera.viewport = Some(wanted);
        }
    }
}
