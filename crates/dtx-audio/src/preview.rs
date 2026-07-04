//! Preview audio: handle cache, playback state, and swap events.
//!
//! ADR-0015. Two pieces live here:
//!
//! - [`AudioHandleCache`] — path-keyed cache of loaded
//!   `Handle<KiraAudioSource>` for preview BGM (Phase 1).
//! - [`PreviewPlayer`] + [`PreviewState`] + [`PreviewSwapEvent`] — the
//!   crossfade state machine and the event `dtx-ui` widgets subscribe
//!   to for parallax / album-art animation (Phase 2).
//!
//! Layer: Engine (bevy + bevy_kira_audio). No Pure or Game deps.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::asset::Handle;
use bevy::prelude::*;
use bevy_kira_audio::AudioInstance;
use bevy_kira_audio::AudioSource as KiraAudioSource;
use bevy_kira_audio::prelude::*;

use crate::crossfade::{
    PREVIEW_FADE_DELAY_MS, PREVIEW_FADE_IN_MS, PREVIEW_FADE_OUT_MS, mute, start_fade_in_with_delay,
    start_fade_out,
};

// =====================================================================
// Phase 1: AudioHandleCache
// =====================================================================

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

// =====================================================================
// Phase 2: PreviewPlayer + PreviewState + PreviewSwapEvent
// =====================================================================

/// Direction of a preview swap. Used by `dtx-ui` widgets to drive
/// parallax / album-art animation that matches the user's scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewSwapDirection {
    #[default]
    None,
    Next,
    Prev,
}

/// Event published when a preview swap starts. Consumers: `dtx-ui`
/// widgets (album art tween, info wedge parallax).
///
/// Not emitted for the first play (no swap), only for changes.
#[derive(Message, Debug, Clone)]
pub struct PreviewSwapEvent {
    pub old_path: Option<PathBuf>,
    pub new_path: PathBuf,
    pub direction: PreviewSwapDirection,
}

/// The crossfade state machine.
///
/// - `Idle` — no preview playing.
/// - `Playing` — a preview is currently playing; `current` is the
///   audio instance handle. A new swap will crossfade from this.
/// - `Crossfading` — a crossfade is in progress. `old` is fading out,
///   `new` is fading in. `fade_in_started` flips once the pre-roll
///   delay elapses and the fade-in tween is kicked off.
#[derive(Resource, Debug, Default, Clone)]
pub struct PreviewPlayer {
    pub state: PreviewState,
    /// Whether the preview loops. Per-screen: `true` on song select,
    /// `false` on title (autoplay-through). Mutated by callers before
    /// the next `play_preview` call.
    pub looping: bool,
    /// The most recently accepted selection index. Used by callers to
    /// compute `PreviewSwapDirection`. `None` until the first accepted
    /// play.
    pub previous_index: Option<usize>,
}

#[derive(Debug, Default, Clone)]
pub enum PreviewState {
    #[default]
    Idle,
    Playing {
        current: Handle<AudioInstance>,
    },
    Crossfading {
        old: Handle<AudioInstance>,
        new: Handle<AudioInstance>,
        elapsed_ms: u32,
        fade_in_started: bool,
    },
}

impl PreviewState {
    /// True when a crossfade is in flight. New swap requests are
    /// rejected while busy. This also acts as the rapid-mash debounce:
    /// each swap takes ~400ms, so requests arriving faster than that
    /// are dropped, mirroring osu-lazer's carousel-snap behavior.
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Crossfading { .. })
    }

    /// The currently-audible preview handle, if any.
    pub fn current(&self) -> Option<&Handle<AudioInstance>> {
        match self {
            Self::Idle => None,
            Self::Playing { current } => Some(current),
            Self::Crossfading { new, .. } => Some(new),
        }
    }
}

/// System: drive the crossfade state machine. Runs every frame in
/// `Update`. No-op unless `state == Crossfading`.
///
/// On reaching `delay_ms`, kicks off the fade-in tween. On reaching
/// `delay_ms + fade_in_ms`, transitions to `Playing { new }` and drops
/// the old handle (kira's fade-out tween continues to play out).
pub fn preview_tick_system(
    time: Res<Time>,
    mut player: ResMut<PreviewPlayer>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let delta_ms = (time.delta_secs() * 1000.0) as u32;

    let new_state = match &mut player.state {
        PreviewState::Crossfading {
            old: _,
            new,
            elapsed_ms,
            fade_in_started,
        } => {
            *elapsed_ms = elapsed_ms.saturating_add(delta_ms);

            if !*fade_in_started && *elapsed_ms >= PREVIEW_FADE_DELAY_MS {
                start_fade_in_with_delay(
                    &mut instances,
                    new,
                    PREVIEW_FADE_IN_MS,
                    /* delay = */ 0,
                );
                *fade_in_started = true;
            }

            if *elapsed_ms >= PREVIEW_FADE_DELAY_MS + PREVIEW_FADE_IN_MS {
                Some(PreviewState::Playing {
                    current: new.clone(),
                })
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(next) = new_state {
        player.state = next;
    }
}

impl PreviewPlayer {
    /// Set whether subsequent `play()` calls loop the preview.
    /// `true` on song select (loop excerpt), `false` on title
    /// (autoplay-through). Has no effect on a currently-playing
    /// preview — call `stop()` first or wait for the next swap.
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }
    /// Start playing a new preview. If another preview is currently
    /// playing or crossfading, the request is rejected and the original
    /// continues (busy-gate). Returns `true` if the swap was accepted.
    ///
    /// On first play (state == Idle), starts immediately with no fade.
    /// On a crossfade-eligible swap, schedules old fade-out + new
    /// fade-in (with 30ms pre-roll) and publishes a `PreviewSwapEvent`
    /// via `events`.
    pub fn play(
        &mut self,
        audio: &Audio,
        source: Handle<KiraAudioSource>,
        path: PathBuf,
        events: &mut MessageWriter<PreviewSwapEvent>,
        direction: PreviewSwapDirection,
        instances: &mut Assets<AudioInstance>,
    ) -> bool {
        if self.state.is_busy() {
            return false;
        }

        let new_handle = if self.looping {
            audio.play(source).looped().handle()
        } else {
            audio.play(source).handle()
        };

        // Mute the new instance immediately so the fade-in drives its
        // audible onset (matches osu's `queuedTrack.Volume.Value = 0`).
        mute(instances, &new_handle);

        let old_handle = match &self.state {
            PreviewState::Idle => None,
            PreviewState::Playing { current } => Some(current.clone()),
            PreviewState::Crossfading { .. } => unreachable!("busy-gated above"),
        };

        if let Some(old) = old_handle {
            start_fade_out(instances, &old, PREVIEW_FADE_OUT_MS);
            self.state = PreviewState::Crossfading {
                old,
                new: new_handle,
                elapsed_ms: 0,
                fade_in_started: false,
            };
            events.write(PreviewSwapEvent {
                old_path: Some(path.clone()),
                new_path: path,
                direction,
            });
        } else {
            self.state = PreviewState::Playing {
                current: new_handle,
            };
        }

        true
    }

    /// Stop the current preview with a fade-out. No-op if idle.
    /// `fade_out_ms=0` uses the crossfade module default.
    pub fn stop(
        &mut self,
        instances: &mut Assets<AudioInstance>,
        fade_out_ms: u32,
    ) {
        let handle = match &self.state {
            PreviewState::Playing { current } => Some(current.clone()),
            PreviewState::Crossfading { new, .. } => Some(new.clone()),
            PreviewState::Idle => None,
        };
        if let Some(handle) = handle {
            let ms = if fade_out_ms == 0 {
                PREVIEW_FADE_OUT_MS
            } else {
                fade_out_ms
            };
            start_fade_out(instances, &handle, ms);
        }
        self.state = PreviewState::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::Handle;

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
        let mut cache = AudioHandleCache::default();
        let path = PathBuf::from("/songs/a/preview.ogg");
        let handle = Handle::<KiraAudioSource>::default();
        cache.put(path.clone(), handle.clone());
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        let asset_server = app.world().resource::<AssetServer>().clone();
        let returned = get_or_load(&mut cache, &asset_server, &path);
        assert_eq!(returned, handle);
    }

    // PreviewState tests — exercise the state machine without audio.

    #[test]
    fn preview_state_starts_idle() {
        let player = PreviewPlayer::default();
        assert!(matches!(player.state, PreviewState::Idle));
        assert!(!player.state.is_busy());
        assert!(player.state.current().is_none());
    }

    #[test]
    fn playing_state_is_not_busy() {
        let state = PreviewState::Playing {
            current: Handle::<AudioInstance>::default(),
        };
        assert!(!state.is_busy());
        assert!(state.current().is_some());
    }

    #[test]
    fn crossfading_state_is_busy() {
        let state = PreviewState::Crossfading {
            old: Handle::<AudioInstance>::default(),
            new: Handle::<AudioInstance>::default(),
            elapsed_ms: 0,
            fade_in_started: false,
        };
        assert!(state.is_busy());
        // The "new" handle is reported as current even mid-crossfade.
        assert!(state.current().is_some());
    }

    #[test]
    fn preview_swap_direction_default_is_none() {
        assert_eq!(PreviewSwapDirection::default(), PreviewSwapDirection::None);
    }
}
