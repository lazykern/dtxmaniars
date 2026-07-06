//! dtx-layout — persisted user layout (lanes now; HUD widgets in plan 2).
//!
//! Pure data crate: serde types, presets, `layout.toml` I/O. No bevy.
//! Sibling of `dtx-config` (same XDG dir, separate file).

pub mod lanes;
pub mod presets;

pub use lanes::{
    channel_from_short, channel_short_name, default_lane_width, DisplayLane, LaneArrangement,
    DRUM_CHANNELS, MAX_LANE_WIDTH, MIN_LANE_WIDTH,
};
pub use presets::LanePreset;
