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

#[derive(Clone, Deserialize, Debug)]
pub struct PlaceOption {
    pub label: String,
    pub effect: Effect,
    /// `None` means always available.
    #[serde(default)]
    pub condition: Option<Condition>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Place {
    pub background: String,
    #[serde(default)]
    pub music: Option<String>,
    /// Shown above the options; the room's name or a line of description.
    #[serde(default)]
    pub caption: Option<String>,
    pub options: Vec<PlaceOption>,
}

/// Place id -> place. Loaded once from the embedded `places/*.ron`.
#[derive(Resource, Clone, Default)]
pub struct Places(pub BTreeMap<String, Place>);

/// Resolves a scene *name* (as written in a place file) to the game's scene id.
/// Supplied by the game, since only it knows the generated enum.
#[derive(Resource)]
pub struct SceneLookup<S>(pub fn(&str) -> Option<S>);

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
}
