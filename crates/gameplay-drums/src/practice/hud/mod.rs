//! Two-tier practice HUD: quick tier (mini strip, chip, toasts) during
//! play, full HUD (timeline + right rail) while paused. Fixed overlay —
//! deliberately NOT a dtx-layout widget (no editor-pillar dependency).

pub mod timeline_ui;

use bevy::prelude::*;

/// Format chart ms as `m:ss.d`.
pub fn format_chart_time(ms: i64) -> String {
    let ms = ms.max(0);
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let d = (ms % 1000) / 100;
    format!("{m}:{s:02}.{d}")
}

pub(super) fn plugin(_app: &mut App) {}
