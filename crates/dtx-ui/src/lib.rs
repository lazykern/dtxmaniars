//! Reusable UI widgets, screen fade constants, animation helpers.
//!
//! Implementation lands in M3+.

#![allow(dead_code)] // scaffold stub

/// DTXManiaNX-derived fluidity constants. See ADR-0010 + `docs/BEVY_PATTERNS.md`.
/// IMPORTANT: these are the DTXManiaNX baseline values, NOT osu-lazer aspirational
/// ones. Do not "modernize" without an ADR override.
pub const SCREEN_FADE_MS: u32 = 1500; // StageManager.cs:29 FadeDurationMs = 1500f
pub const LOAD_HOLD_MS: u32 = 0; // DTXManiaNX has no load hold (no min wait)
pub const INPUT_LATENCY_MS: u32 = 16; // bevy_framepace target

/// Root plugin; currently a no-op until widgets land.
pub fn plugin() {}
