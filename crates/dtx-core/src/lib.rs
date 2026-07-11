//! DTX chart parser (pure Rust, no bevy).
//!
//! Parses DTX files into [`Chart`] / [`Chip`] / [`EChannel`] data.
//! See `docs/decisions/0005-flat-workspace-layout.md` for layering rules.

pub mod assets;
pub mod base36;
pub mod beat_lines;
pub mod bga;
pub mod c_avi;
pub mod c_box_set_def;
pub mod c_chart_data;
pub mod c_chip;
pub mod c_song_list_node;
pub mod cdtx_config;
pub mod cdtx_model;
pub mod cdtx_nested;
pub mod channel;
pub mod chart;
pub mod chip_classify;
pub mod chip_transform;
pub mod constants;
pub mod cscore_ini;
pub mod enum_converter;
pub mod error;
pub mod fdk_sub_acts;
pub mod parser;
pub mod random_mode;
pub mod score_song;
pub mod timing;
pub mod trigger_pipeline;

pub use assets::resolve_bgm_path;
pub use beat_lines::{expand_timing_lines, TimingLine, TimingLineKind};
pub use channel::EChannel;
pub use chart::{display_dlevel, Chart, Chip, EmptyHitEvent, Metadata};
pub use chip_transform::{apply_mirror, apply_random, RandomMode};
pub use error::DtxError;
pub use parser::parse;

/// Parse a DTX file from a string slice.
///
/// Convenience for tests and embedded use cases.
pub fn parse_str(input: &str) -> Result<Chart, DtxError> {
    parser::parse(input.as_bytes())
}
