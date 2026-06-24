//! DTX chart parser (pure Rust, no bevy).
//!
//! Parses DTX files into [`Chart`] / [`Chip`] / [`EChannel`] data.
//! See `docs/decisions/0005-flat-workspace-layout.md` for layering rules.

pub mod bga;
pub mod channel;
pub mod chart;
pub mod error;
pub mod fdk_sub_acts;
pub mod cdtx_model;
pub mod cscore_ini;
pub mod c_box_set_def;
pub mod parser;
pub mod score_song;

pub use channel::EChannel;
pub use chart::{Chart, Chip, Metadata};
pub use error::DtxError;
pub use parser::parse;

/// Parse a DTX file from a string slice.
///
/// Convenience for tests and embedded use cases.
pub fn parse_str(input: &str) -> Result<Chart, DtxError> {
    parser::parse(input.as_bytes())
}
