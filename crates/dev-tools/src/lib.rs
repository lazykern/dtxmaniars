//! Developer tools (inspector, FPS, log viewer). Dev-only.
//!
//! Lands in M3+. Gated behind `#[cfg(debug_assertions)]` at the call site.

#![allow(dead_code)] // scaffold stub

/// Root plugin; currently a no-op.
pub fn plugin() {}
