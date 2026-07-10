//! Chrome dimension constants — SINGLE SOURCE for both the editor chrome
//! spawns (ui.rs tab bar, panel.rs left panel + inspector) and the stage preset
//! math (stage.rs `preset_rect`). They must agree or the shrunk miniature
//! misaligns with the chrome around it.

/// Horizontal SETTINGS/KIT tab bar height, docked at the top of the left column.
pub const TAB_BAR_HEIGHT: f32 = 64.0;
/// Left column width (tab bar on top, content panel below), docked at the
/// window's left edge.
pub const LEFT_PANEL_WIDTH: f32 = 480.0;
/// Right inspector panel width, docked at the window's right edge.
pub const INSPECTOR_WIDTH: f32 = 240.0;
