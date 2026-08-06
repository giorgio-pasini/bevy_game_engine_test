//! Generates placeholder map art and its hotspot lookup image.
//!
//! Run from this directory:
//!
//! ```text
//! cargo run
//! ```
//!
//! Writes `demo_game/assets/maps/valley.png` (what the player sees) and
//! `valley_lookup.png` (what the engine samples).
//!
//! **The point of this tool is that both images come out of one geometry
//! definition.** A map and its lookup are two renderings of the same shapes, and
//! hand-maintaining them in an image editor means they drift — a region nudged
//! in the art but not in the lookup produces a game where clicking the mountain
//! does nothing, with no error anywhere. Here that is impossible by
//! construction.
//!
//! **Delete this crate when real map art lands.** Its whole justification is
//! that placeholder art and its lookup must agree, and it guarantees that by
//! generating both. Once an artist supplies the images, that guarantee moves to
//! `build.rs`'s pixel audit and this becomes a crate nobody runs, with a
//! dependency nobody audits.
//!
//! When real art replaces this, the guarantee moves to `demo_game/build.rs`,
//! which checks the lookup for lossy encoding, stray colours (anti-aliased
//! edges), missing regions and unclickably small ones.

use std::path::PathBuf;

use image::{Rgb, RgbImage};

/// The design area is 16:9; both images must be, or a point on screen is not
/// the same point in the lookup. Their *resolutions* are free — sampling is
/// normalised — so the lookup is deliberately smaller than the art.
const ART: (u32, u32) = (1920, 1080);
const LOOKUP: (u32, u32) = (960, 540);

/// Key colours, matching `assets/places/valley.ron`. Never black: black is
/// reserved for "no hotspot here".
const KEY_CAIRN: Rgb<u8> = Rgb([220, 60, 60]);
const KEY_LEDGE: Rgb<u8> = Rgb([60, 140, 220]);
const KEY_RIDGE: Rgb<u8> = Rgb([240, 200, 80]);

const EMPTY: Rgb<u8> = Rgb([0, 0, 0]);

/// Geometry in design units (1920x1080, origin top-left), so the shapes read the
/// same way whatever resolution each image is rendered at.
struct Region {
    key: Rgb<u8>,
    fill: Rgb<u8>,
    edge: Rgb<u8>,
    inside: fn(f32, f32) -> bool,
}

fn cairn(x: f32, y: f32) -> bool {
    (x - 700.0).powi(2) + (y - 330.0).powi(2) < 150.0_f32.powi(2)
}

fn ledge(x: f32, y: f32) -> bool {
    // Rounded rectangle: the corner radius is what makes it obvious at a glance
    // that regions are shapes, not the bounding boxes a coordinate-based system
    // would have forced.
    let (x0, y0, x1, y1) = (1150.0, 560.0, 1580.0, 790.0);
    let r = 48.0;
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    x >= x0 && x <= x1 && y >= y0 && y <= y1 && ((x - cx).powi(2) + (y - cy).powi(2) <= r * r)
}

fn ridge(x: f32, y: f32) -> bool {
    // Triangle (250,900) (640,900) (445,600), by half-plane tests.
    let side = |ax: f32, ay: f32, bx: f32, by: f32| (bx - ax) * (y - ay) - (by - ay) * (x - ax);
    let a = side(250.0, 900.0, 640.0, 900.0);
    let b = side(640.0, 900.0, 445.0, 600.0);
    let c = side(445.0, 600.0, 250.0, 900.0);
    (a <= 0.0 && b <= 0.0 && c <= 0.0) || (a >= 0.0 && b >= 0.0 && c >= 0.0)
}

fn regions() -> Vec<Region> {
    vec![
        Region {
            key: KEY_CAIRN,
            fill: Rgb([150, 70, 72]),
            edge: Rgb([214, 128, 124]),
            inside: cairn,
        },
        Region {
            key: KEY_LEDGE,
            fill: Rgb([58, 92, 138]),
            edge: Rgb([124, 168, 216]),
            inside: ledge,
        },
        Region {
            key: KEY_RIDGE,
            fill: Rgb([150, 128, 72]),
            edge: Rgb([228, 200, 130]),
            inside: ridge,
        },
    ]
}

/// True when the point is inside the shape but within `width` of its boundary.
/// Generic over the shape, so adding a region needs no matching outline code.
fn on_edge(inside: fn(f32, f32) -> bool, x: f32, y: f32, width: f32) -> bool {
    if !inside(x, y) {
        return false;
    }
    [
        (width, 0.0),
        (-width, 0.0),
        (0.0, width),
        (0.0, -width),
        (width, width),
        (width, -width),
        (-width, width),
        (-width, -width),
    ]
    .iter()
    .any(|(dx, dy)| !inside(x + dx, y + dy))
}

/// Renders at `size`, calling `shade` with design-space coordinates. Sampling at
/// pixel *centres* keeps the two resolutions aligned.
fn render(size: (u32, u32), shade: impl Fn(f32, f32) -> Rgb<u8>) -> RgbImage {
    let (w, h) = size;
    RgbImage::from_fn(w, h, |px, py| {
        let x = (px as f32 + 0.5) / w as f32 * ART.0 as f32;
        let y = (py as f32 + 0.5) / h as f32 * ART.1 as f32;
        shade(x, y)
    })
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../demo_game/assets/maps")
        .canonicalize()
        .unwrap_or_else(|e| panic!("assets/maps must exist next to demo_game: {e}"));

    let shapes = regions();

    // The art: a graded slate field, each region filled and outlined. No text —
    // the game names the region under the cursor, which is the affordance that
    // has to work with real art too.
    let art = render(ART, |x, y| {
        for region in &shapes {
            if on_edge(region.inside, x, y, 5.0) {
                return region.edge;
            }
            if (region.inside)(x, y) {
                return region.fill;
            }
        }
        let t = y / ART.1 as f32;
        Rgb([
            (18.0 + 26.0 * t) as u8,
            (22.0 + 30.0 * t) as u8,
            (34.0 + 34.0 * t) as u8,
        ])
    });

    // The lookup: the same shapes as flat keys, hard-edged. Anti-aliasing here
    // would be actively harmful — every blended pixel is a colour matching no
    // region, so the edges of every hotspot would go dead.
    let lookup = render(LOOKUP, |x, y| {
        for region in &shapes {
            if (region.inside)(x, y) {
                return region.key;
            }
        }
        EMPTY
    });

    for (name, image) in [("valley.png", &art), ("valley_lookup.png", &lookup)] {
        let path = out.join(name);
        image
            .save(&path)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
        println!("wrote {} ({}x{})", path.display(), image.width(), image.height());
    }
}
