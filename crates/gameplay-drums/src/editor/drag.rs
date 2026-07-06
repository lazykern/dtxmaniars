//! Stub — filled by the drag/selection task.
use bevy::prelude::*;

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Selection(pub Option<dtx_layout::WidgetKind>);

pub fn plugin(_app: &mut App) {}
