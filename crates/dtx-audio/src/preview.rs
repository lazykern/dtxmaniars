//! Preview audio handle cache.
//!
//! ADR-0015 Phase 1: dedupe audio source loads by resolved file path.
//! Switching to a chart whose `bgm_path` is already cached avoids the
//! `AssetServer` load + decode overhead.
//!
//! Bevy's `AssetServer` already dedupes `Handle<T>` by path, so the
//! primary value of this cache is the explicit lookup hook that
//! `PreviewPlayer` uses. Future multi-diff (M6+) and decode-pool (M14+)
//! work builds on the same shape.
//!
//! Layer: Engine (bevy + bevy_kira_audio). No `Pure` or `Game` deps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::asset::Handle;
use bevy::prelude::*;
use bevy_kira_audio::AudioSource as KiraAudioSource;

/// Cache of loaded Kira audio source handles keyed by resolved file path.
///
/// Use [`get_or_load`] to look up a path; cache hits return the existing
/// handle, misses load via `AssetServer` and insert.
#[derive(Resource, Default, Debug)]
pub struct AudioHandleCache {
    by_path: HashMap<PathBuf, Handle<KiraAudioSource>>,
}

impl AudioHandleCache {
    /// Look up a cached handle by path.
    pub fn get(&self, path: &Path) -> Option<&Handle<KiraAudioSource>> {
        self.by_path.get(path)
    }

    /// Insert a handle into the cache, replacing any existing entry.
    pub fn put(&mut self, path: PathBuf, handle: Handle<KiraAudioSource>) {
        self.by_path.insert(path, handle);
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.by_path.len()
    }

    /// True when no entries are cached.
    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    /// Remove all entries. Use when the song library is rescanned or the
    /// cache needs to be reset (e.g. audio device change).
    pub fn clear(&mut self) {
        self.by_path.clear();
    }
}

/// Look up an audio source for `path` in the cache, loading on miss.
///
/// Returns the cached handle on a hit, or loads via `AssetServer` and
/// inserts into the cache on a miss. The path is stored exactly as given
/// (caller is responsible for canonicalisation if desired).
pub fn get_or_load(
    cache: &mut AudioHandleCache,
    asset_server: &AssetServer,
    path: &Path,
) -> Handle<KiraAudioSource> {
    if let Some(handle) = cache.get(path) {
        return handle.clone();
    }
    let path_str = path.to_string_lossy().into_owned();
    let handle = asset_server
        .load_builder()
        .override_unapproved()
        .load(path_str);
    cache.put(path.to_path_buf(), handle.clone());
    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_starts_empty() {
        let cache = AudioHandleCache::default();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn cache_put_then_get() {
        let mut cache = AudioHandleCache::default();
        let handle = Handle::<KiraAudioSource>::default();
        let path = PathBuf::from("/songs/a/preview.ogg");
        cache.put(path.clone(), handle.clone());
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
        assert_eq!(cache.get(&path).unwrap(), &handle);
    }

    #[test]
    fn cache_get_missing_returns_none() {
        let cache = AudioHandleCache::default();
        assert!(cache.get(Path::new("/missing.ogg")).is_none());
    }

    #[test]
    fn cache_put_replaces_existing() {
        let mut cache = AudioHandleCache::default();
        let path = PathBuf::from("/songs/a/preview.ogg");
        let first = Handle::<KiraAudioSource>::default();
        let second = Handle::<KiraAudioSource>::default();
        cache.put(path.clone(), first);
        cache.put(path.clone(), second.clone());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&path).unwrap(), &second);
    }

    #[test]
    fn cache_clear_empties_all_entries() {
        let mut cache = AudioHandleCache::default();
        cache.put(PathBuf::from("/a.ogg"), Handle::<KiraAudioSource>::default());
        cache.put(PathBuf::from("/b.ogg"), Handle::<KiraAudioSource>::default());
        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn get_or_load_cache_hit_does_not_invoke_asset_server() {
        // Cache hit path is pure: no AssetServer, no asset event side
        // effects. We pre-populate and confirm the same handle comes
        // back. The asset_server parameter is required by signature; it
        // would only be used on a cache miss.
        let mut cache = AudioHandleCache::default();
        let path = PathBuf::from("/songs/a/preview.ogg");
        let handle = Handle::<KiraAudioSource>::default();
        cache.put(path.clone(), handle.clone());
        // Constructing AssetServer requires a real App; the cache-hit
        // branch never touches it. Build a real App so the function can
        // be called. The test asserts the cached handle is returned
        // unchanged.
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        let asset_server = app.world().resource::<AssetServer>().clone();
        let returned = get_or_load(&mut cache, &asset_server, &path);
        assert_eq!(returned, handle);
    }
}
