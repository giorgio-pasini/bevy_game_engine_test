//! Places: the exploration phase.
//!
//! A place is a background plus a list of options. Movement, actions and
//! leaving differ only by their [`Effect`] — keeping them uniform means the
//! existing choice-button UI renders all three without special cases.
//!
//! Place ids are `String` here and resolved through [`Places`]; `build.rs`
//! guarantees every referenced id exists, exactly as it does for scenes.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::story::Condition;

/// What choosing an option does.
#[derive(Clone, Deserialize, Debug)]
pub enum Effect {
    /// Move to another place, replacing the current one. Does not grow the
    /// stack — walking from room to room is not recursion.
    GoTo(String),
    /// Run a scene, then come back to this place. Pushes a frame.
    Call(String),
    /// Pop back to whatever handed control to this exploration.
    Leave,
}

/// A colour key in a hotspot lookup image, as written in RON: `(220, 60, 60)`.
///
/// Kept as raw bytes rather than a [`Color`]: these are looked up by exact
/// equality, and a float colour invites a near-miss that is invisible in the
/// file and unclickable in the game.
#[derive(Copy, Clone, PartialEq, Eq, Deserialize, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Reserved for "no hotspot here", so a lookup image needs no alpha channel
    /// and no separate mask.
    pub const EMPTY: Rgb = Rgb(0, 0, 0);
}

#[derive(Clone, Deserialize, Debug)]
pub struct PlaceOption {
    pub label: String,
    pub effect: Effect,
    /// `None` means always available.
    #[serde(default)]
    pub condition: Option<Condition>,
    /// Colour identifying this option's region in the place's lookup image.
    ///
    /// An option with a region is clicked **on the map** and is not listed as a
    /// button; one without is a button, as before. A place may mix the two — a
    /// map with a "Back" button underneath is just an option with no region.
    #[serde(default)]
    pub region: Option<Rgb>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Place {
    pub background: String,
    #[serde(default)]
    pub music: Option<String>,
    /// Shown above the options; the room's name or a line of description.
    #[serde(default)]
    pub caption: Option<String>,
    /// Colour-keyed hotspot image for this place, if it is a map.
    ///
    /// Never drawn — it is sampled. Same 16:9 framing as the background, so a
    /// point on screen is the same point in both; `build.rs` enforces that, the
    /// lossless format, and that every declared colour is actually present.
    #[serde(default)]
    pub lookup: Option<String>,
    pub options: Vec<PlaceOption>,
}

/// Place id -> place. Loaded once from the embedded `places/*.ron`.
#[derive(Resource, Clone, Default)]
pub struct Places(pub BTreeMap<String, Place>);

/// The option the player picked this frame, whichever input noticed it.
///
/// Buttons and map hotspots are two ways of saying the same thing, so they both
/// write here and exactly one system applies the effect. Without this the
/// re-checked condition, the stack manipulation and the flow routing would exist
/// once per input device, and the second copy would be the one that drifts.
#[derive(Resource, Default)]
pub struct PickedOption(pub Option<usize>);

impl Places {
    pub fn get(&self, id: &str) -> Option<&Place> {
        self.0.get(id)
    }

    /// Options whose condition currently holds, with their original indices so
    /// a click maps back to the unfiltered list.
    pub fn available<'a>(
        &'a self,
        id: &str,
        flags: &BTreeMap<String, i32>,
        items: &std::collections::BTreeSet<String>,
    ) -> Vec<(usize, &'a PlaceOption)> {
        self.get(id)
            .map(|p| {
                p.options
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| {
                        o.condition
                            .as_ref()
                            .is_none_or(|c| c.holds(flags, items))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The option a hotspot colour selects, if any.
    ///
    /// Built on [`Places::available`] so a hotspot obeys exactly the same guard
    /// as a button: an option whose condition fails is not clickable on the map
    /// either, without that rule being written down twice.
    pub fn option_for_region(
        &self,
        id: &str,
        colour: Rgb,
        flags: &BTreeMap<String, i32>,
        items: &std::collections::BTreeSet<String>,
    ) -> Option<usize> {
        if colour == Rgb::EMPTY {
            return None;
        }
        self.available(id, flags, items)
            .into_iter()
            .find(|(_, o)| o.region == Some(colour))
            .map(|(i, _)| i)
    }
}
