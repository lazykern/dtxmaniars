//! Undo/redo for editor edits: snapshots of (WidgetLayouts, Lanes).

use bevy::prelude::*;

use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Clone)]
pub struct Snapshot {
    pub layouts: WidgetLayouts,
    pub lanes: Lanes,
}

/// Bounded undo/redo history.
#[derive(Resource, Default)]
pub struct UndoStack {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
}

const MAX_HISTORY: usize = 64;

impl UndoStack {
    /// Record the current state as an undo point (clears redo history).
    pub fn push(&mut self, layouts: &WidgetLayouts, lanes: &Lanes) {
        self.push_snapshot(Snapshot {
            layouts: layouts.clone(),
            lanes: lanes.clone(),
        });
    }

    /// Push a pre-built snapshot (callers that must snapshot before a
    /// conditional mutation).
    pub fn push_snapshot(&mut self, snap: Snapshot) {
        self.past.push(snap);
        if self.past.len() > MAX_HISTORY {
            self.past.remove(0);
        }
        self.future.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Undo: restore the last snapshot, pushing the current state to redo.
    pub fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let prev = self.past.pop()?;
        self.future.push(current);
        Some(prev)
    }

    /// Redo: reapply the last undone snapshot.
    pub fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.future.pop()?;
        self.past.push(current);
        Some(next)
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        undo_redo_hotkeys
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

/// Ctrl+Z = undo, Ctrl+Y or Ctrl+Shift+Z = redo.
fn undo_redo_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<UndoStack>,
    mut layouts: ResMut<WidgetLayouts>,
    mut lanes: ResMut<Lanes>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !ctrl {
        return;
    }
    let current = Snapshot {
        layouts: layouts.clone(),
        lanes: lanes.clone(),
    };
    if keys.just_pressed(KeyCode::KeyZ) && !shift {
        if let Some(s) = stack.undo(current) {
            *layouts = s.layouts;
            *lanes = s.lanes;
        }
    } else if keys.just_pressed(KeyCode::KeyY) || (shift && keys.just_pressed(KeyCode::KeyZ)) {
        if let Some(s) = stack.redo(current) {
            *layouts = s.layouts;
            *lanes = s.lanes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_layout::WidgetKind;

    fn snap(offx: f32) -> (WidgetLayouts, Lanes) {
        let mut l = WidgetLayouts::default();
        l.0.get_mut(&WidgetKind::Combo).unwrap().offset = (offx, 0.0);
        (l, Lanes::default())
    }

    #[test]
    fn undo_then_redo_round_trips() {
        let mut stack = UndoStack::default();
        let (l0, n0) = snap(0.0);
        stack.push(&l0, &n0);
        let (l1, n1) = snap(50.0);
        let cur_b = Snapshot { layouts: l1, lanes: n1 };
        let restored_a = stack.undo(cur_b.clone()).unwrap();
        assert_eq!(restored_a.layouts.get(WidgetKind::Combo).offset, (0.0, 0.0));
        let back_b = stack.redo(restored_a).unwrap();
        assert_eq!(back_b.layouts.get(WidgetKind::Combo).offset, (50.0, 0.0));
    }

    #[test]
    fn undo_empty_is_none() {
        let mut stack = UndoStack::default();
        let (l, n) = snap(1.0);
        assert!(stack.undo(Snapshot { layouts: l, lanes: n }).is_none());
    }

    #[test]
    fn push_clears_redo() {
        let mut stack = UndoStack::default();
        let (l, n) = snap(0.0);
        stack.push(&l, &n);
        let _ = stack.undo(Snapshot { layouts: l.clone(), lanes: n.clone() });
        assert!(stack.can_redo());
        stack.push(&l, &n);
        assert!(!stack.can_redo());
    }
}
