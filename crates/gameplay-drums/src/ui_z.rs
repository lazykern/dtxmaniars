//! Global z-index registry for gameplay-drums UI stacking. `GlobalZIndex`
//! creates one global stacking order across the whole UI; every layer that
//! participates is named here so collisions are greppable instead of folklore.

/// Practice HUD chip + mini strip.
pub const PRACTICE: i32 = 900;
/// Practice full HUD root.
pub const PRACTICE_FULL_HUD: i32 = 1000;
/// Pause overlay.
pub const PAUSE: i32 = 1000;
/// Stage-end results overlay.
pub const STAGE_END: i32 = 1100;
/// Practice toasts (same layer as stage-end).
pub const TOAST: i32 = 1100;
/// Customize: full-window dim scrim (above all HUD, below editor layers).
pub const PREVIEW_SCRIM: i32 = 1500;
/// Customize: miniature bounds outline.
pub const STAGE_OUTLINE: i32 = 1900;
/// Customize: bindings selected-lane overlay.
pub const BIND_OVERLAY: i32 = 1910;
/// Customize: chrome (rail, panels, inspector, footer).
pub const EDITOR_CHROME: i32 = 2000;
/// Customize: snap guide lines.
pub const SNAP_GUIDES: i32 = 2050;
/// Customize: hover outline.
pub const HOVER_OUTLINE: i32 = 2100;
/// Customize: anchor line/dots.
pub const ANCHOR_VIZ: i32 = 2150;
/// Customize: selection box.
pub const SELECTION_BOX: i32 = 2200;
/// Customize: modal dialogs above all editor visuals.
pub const EDITOR_MODAL: i32 = 2300;
