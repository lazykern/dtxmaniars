//! Chrome dimension constants — SINGLE SOURCE for both the editor chrome
//! spawns (ui.rs rail, panel.rs left panel + inspector) and the stage preset
//! math (stage.rs `preset_rect`). They must agree or the shrunk miniature
//! misaligns with the chrome around it.

/// Tabs-only rail width, docked at the window's left edge.
pub const RAIL_WIDTH: f32 = 132.0;
/// Left content panel width, docked flush right of the rail.
pub const LEFT_PANEL_WIDTH: f32 = 348.0;
/// Right inspector panel width, docked at the window's right edge.
pub const INSPECTOR_WIDTH: f32 = 240.0;
