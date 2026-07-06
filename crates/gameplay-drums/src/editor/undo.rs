//! Stub — filled by the undo task.
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct UndoStack;

impl UndoStack {
    /// No-op stub; the undo task replaces this with real history.
    pub fn push(
        &mut self,
        _layouts: &crate::widget_layout::WidgetLayouts,
        _lanes: &crate::lanes::Lanes,
    ) {
    }
}

pub fn plugin(_app: &mut App) {}
