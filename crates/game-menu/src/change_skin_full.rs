#![allow(dead_code)]
#![allow(clippy::type_complexity)]
//! `CStageChangeSkin` — port of `Stage/09.ChangeSkin/CStageChangeSkin.cs` (95 LOC).
//!
//! Strict-port-first. The change-skin stage is intentionally bare — no
//! sprites render. It triggers a skin reload and returns to caller.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/09.ChangeSkin/CStageChangeSkin.cs:1-95`

use bevy::prelude::{App, Resource};

/// Stage ID (CStageChangeSkin.cs:11) — `EStage.ChangeSkin_9`.
pub const CHANGE_SKIN_STAGE_ID: u32 = 9;

/// Skin state — current subfolder + reload tracking.
#[derive(Resource, Debug, Default, Clone)]
pub struct ChangeSkinState {
    /// Current skin subfolder name (e.g. "Default").
    pub current_skin: String,
    /// True if a reload is currently in progress.
    pub reloading: bool,
    /// Number of reloads performed (for diagnostic logging).
    pub reload_count: u32,
}

impl ChangeSkinState {
    pub fn new() -> Self {
        Self {
            current_skin: "Default".into(),
            reloading: false,
            reload_count: 0,
        }
    }

    /// Trigger skin reload (CStageChangeSkin.cs:78-86 — tChangeSkinMain).
    pub fn reload(&mut self, new_skin: &str) {
        self.reloading = true;
        self.current_skin = new_skin.to_string();
        self.reload_count += 1;
        self.reloading = false;
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ChangeSkinState>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_id_matches_e_stage_change_skin_9() {
        // CStageChangeSkin.cs:11
        assert_eq!(CHANGE_SKIN_STAGE_ID, 9);
    }

    #[test]
    fn default_skin_is_default() {
        let s = ChangeSkinState::new();
        assert_eq!(s.current_skin, "Default");
        assert!(!s.reloading);
    }

    #[test]
    fn default_reload_count_zero() {
        let s = ChangeSkinState::new();
        assert_eq!(s.reload_count, 0);
    }
    fn reload_updates_skin_name() {
        let mut s = ChangeSkinState::new();
        s.reload("MySkin");
        assert_eq!(s.current_skin, "MySkin");
        assert_eq!(s.reload_count, 1);
    }

    #[test]
    fn reload_increments_count() {
        let mut s = ChangeSkinState::new();
        s.reload("Skin1");
        s.reload("Skin2");
        s.reload("Skin3");
        assert_eq!(s.reload_count, 3);
        assert_eq!(s.current_skin, "Skin3");
    }

    #[test]
    fn reload_resets_reloading_flag() {
        let mut s = ChangeSkinState::new();
        s.reload("X");
        assert!(!s.reloading);
    }
}
