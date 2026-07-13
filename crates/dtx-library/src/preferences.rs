//! Durable, user-owned library preferences.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::user_data_dir;

/// Serialized schema version for [`LibraryPreferences`].
pub const LIBRARY_PREFERENCES_VERSION: u32 = 1;

/// Preferences that belong to the library rather than score history.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct LibraryPreferences {
    #[serde(default = "preferences_version")]
    pub version: u32,
    #[serde(default)]
    favorites: BTreeSet<String>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Default for LibraryPreferences {
    fn default() -> Self {
        Self {
            version: LIBRARY_PREFERENCES_VERSION,
            favorites: BTreeSet::new(),
            path: Some(default_path()),
        }
    }
}

/// Errors while loading or saving preferences. Callers treat these as nonfatal.
#[derive(Debug, Error)]
pub enum LibraryPreferencesError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported library preferences version {0}")]
    UnsupportedVersion(u32),
}

impl LibraryPreferences {
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            ..Self::default()
        }
    }

    pub fn is_favorite(&self, chart: &Path) -> bool {
        self.favorites.contains(&chart_key(chart))
    }

    /// Toggles the chart favorite and returns its new state.
    pub fn toggle_favorite(&mut self, chart: &Path) -> bool {
        let key = chart_key(chart);
        if !self.favorites.insert(key.clone()) {
            self.favorites.remove(&key);
            false
        } else {
            true
        }
    }

    pub fn load(&mut self) -> Result<(), LibraryPreferencesError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !path.exists() {
            return Ok(());
        }
        let bytes = std::fs::read(path)?;
        if bytes.is_empty() {
            return Ok(());
        }
        let parsed: Self = serde_json::from_slice(&bytes)?;
        if parsed.version > LIBRARY_PREFERENCES_VERSION {
            return Err(LibraryPreferencesError::UnsupportedVersion(parsed.version));
        }
        self.version = parsed.version;
        self.favorites = parsed.favorites;
        Ok(())
    }

    pub fn save(&self) -> Result<(), LibraryPreferencesError> {
        if self.version > LIBRARY_PREFERENCES_VERSION {
            return Err(LibraryPreferencesError::UnsupportedVersion(self.version));
        }
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

pub fn load_preferences_system(mut preferences: ResMut<LibraryPreferences>) {
    if let Err(error) = preferences.load() {
        warn!("dtx-library: preferences unavailable; using empty favorites: {error}");
        *preferences = LibraryPreferences::default();
    }
}

fn preferences_version() -> u32 {
    LIBRARY_PREFERENCES_VERSION
}

fn default_path() -> PathBuf {
    user_data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("library-preferences.json")
}

fn chart_key(chart: &Path) -> String {
    std::fs::canonicalize(chart)
        .unwrap_or_else(|_| chart.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
