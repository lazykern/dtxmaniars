//! Keyboard nav for Customize settings tabs: up/down move the focused row,
//! left/right adjust it. Settings tabs only (no clash with kit arrow-nudge).

use bevy::prelude::*;

/// Which settings row is focused for keyboard nav. Reset to 0 on tab change.
#[derive(Resource, Default)]
pub struct FocusedRow(pub usize);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FocusedRow>().add_systems(
        Update,
        settings_keyboard_nav
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(super::editor_open),
    );
}

fn settings_keyboard_nav(
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<super::tabs::ActiveTab>,
    mut focused: ResMut<FocusedRow>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl {
        if keys.just_pressed(KeyCode::PageDown) {
            active.0 = active.0.next();
            return;
        } else if keys.just_pressed(KeyCode::PageUp) {
            active.0 = active.0.prev();
            return;
        }
    }
    if !active.0.is_settings() {
        return;
    }
    // Switching tabs resets focus to the first row before reading keys.
    if active.is_changed() {
        focused.0 = 0;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    if items.is_empty() {
        return;
    }
    // Don't hijack when Ctrl held (perf hotkeys / save).
    if ctrl {
        return;
    }
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let reps = if coarse { 10 } else { 1 };
    if keys.just_pressed(KeyCode::ArrowDown) {
        focused.0 = (focused.0 + 1).min(items.len() - 1);
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        focused.0 = focused.0.saturating_sub(1);
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        if let Some(item) = items.get(focused.0) {
            for _ in 0..reps {
                (item.adjust)(&mut draft.0, 1);
            }
        }
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        if let Some(item) = items.get(focused.0) {
            for _ in 0..reps {
                (item.adjust)(&mut draft.0, -1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn focus_clamps_to_row_range() {
        // pure clamp logic sanity — replicate the min/saturating_sub used above
        let len = 3usize;
        let mut f = 0usize;
        f = (f + 1).min(len - 1);
        assert_eq!(f, 1);
        f = 5;
        f = f.min(len - 1);
        assert_eq!(f, 2);
        f = 0;
        f = f.saturating_sub(1);
        assert_eq!(f, 0);
    }
}
