//! Chrome dimension constants — SINGLE SOURCE for both the editor chrome
//! spawns (ui.rs tab bar, panel.rs left panel + inspector) and the stage preset
//! math (stage.rs `preset_rect`). They must agree or the shrunk miniature
//! misaligns with the chrome around it.

use bevy::prelude::Color;

/// Horizontal SETTINGS/KIT tab bar height, docked at the top of the left column.
pub const TAB_BAR_HEIGHT: f32 = 64.0;
/// Left column width (tab bar on top, content panel below), docked at the
/// window's left edge.
pub const LEFT_PANEL_WIDTH: f32 = 480.0;
/// Right inspector panel width, docked at the window's right edge.
pub const INSPECTOR_WIDTH: f32 = 240.0;

// Quiet-instrument palette (spec 2026-07-11-controls-lanes-redesign).
pub const PANEL_BG: Color = Color::srgb(0.070, 0.078, 0.102); // #12141a
pub const CARD_BG: Color = Color::srgb(0.090, 0.102, 0.133); // #171a22
pub const CARD_BORDER: Color = Color::srgb(0.149, 0.165, 0.208); // #262a35
pub const ACCENT: Color = Color::srgb(0.357, 0.549, 1.0); // #5b8cff
pub const ROW_SELECTED_BG: Color = Color::srgb(0.114, 0.149, 0.208);
pub const CHIP_BG: Color = Color::srgb(0.122, 0.141, 0.188);
pub const CHIP_BORDER: Color = Color::srgb(0.20, 0.227, 0.29);
pub const TEXT_MUTED: Color = Color::srgb(0.365, 0.396, 0.47);
pub const DIRTY: Color = Color::srgb(0.847, 0.627, 0.184); // amber
pub const OK: Color = Color::srgb(0.306, 0.788, 0.541); // green
pub const ERR: Color = Color::srgb(0.86, 0.34, 0.34); // red
pub const WARN_TINT: Color = Color::srgb(0.19, 0.12, 0.10); // unbound row bg
