//! dtx-layout — persisted user layout (lanes now; HUD widgets in plan 2).
//!
//! Pure data crate: serde types, presets, `layout.toml` I/O. No bevy.
//! Sibling of `dtx-config` (same XDG dir, separate file).

use std::path::{Path, PathBuf};

use thiserror::Error;

pub mod file;
pub mod lanes;
pub mod presets;

pub use file::{parse_with_migrations, LanesSection, LayoutFile, LATEST_VERSION};
pub use lanes::{
    channel_from_short, channel_short_name, default_lane_width, DisplayLane, LaneArrangement,
    DRUM_CHANNELS, MAX_LANE_WIDTH, MIN_LANE_WIDTH,
};
pub use presets::{arrangement_for, classic, nx_type_b, nx_type_d, LanePreset};

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// `$XDG_CONFIG_HOME/dtxmaniars/layout.toml` → `$HOME/.config/dtxmaniars/layout.toml`
/// → `layout.toml` (cwd fallback). Same directory as dtx-config's config.toml.
pub fn default_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let mut p = PathBuf::from(xdg);
        p.push("dtxmaniars");
        p.push("layout.toml");
        return p;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".config");
        p.push("dtxmaniars");
        p.push("layout.toml");
        return p;
    }
    PathBuf::from("layout.toml")
}

/// Load layout. Missing/corrupt file → defaults; never panics, never writes.
pub fn load(path: &Path) -> LayoutFile {
    match std::fs::read_to_string(path) {
        Ok(s) => parse_with_migrations(&s),
        Err(_) => LayoutFile::default(),
    }
}

/// Save layout, creating parent dirs.
pub fn save(path: &Path, file: &LayoutFile) -> Result<(), LayoutError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, toml::to_string_pretty(file)?)?;
    Ok(())
}
