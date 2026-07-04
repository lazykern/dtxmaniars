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
    stop_with_fade,
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

/// Screen-fade phase transition. Published by `game-shell` when
/// `ScreenFade` enters a new phase. Consumed by `dtx-audio` to
/// align preview-audio fade with the visual fade.
///
/// ADR-0015 deferred item (c).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenFadeTransition {
    /// Fade-out just started (transition initiated).
    Out,
    /// Fade-in just started (after fade-out completed, new screen
    /// is being revealed).
    In,
    /// Fade-in completed, back to Idle.
    Done,
}

/// The crossfade state machine.
///
/// - `Idle` — no preview playing.
/// - `Playing` — a preview is currently playing; `current` is the
///   audio instance handle. A new swap will crossfade from this.
/// - `Crossfading` — a crossfade is in progress. `old` is `Some` on a
///   swap from `Playing` and `None` on the initial play (no prior
///   handle to fade out). `new` is fading in (or has just started).
///   `fade_in_started` flips once the pre-roll delay elapses and the
///   fade-in tween is kicked off.
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
    /// Path of the most recently played preview. Used by `play()` to
    /// short-circuit when the caller asks for the same file again
    /// (e.g. switching to a sibling chart / difficulty that shares
    /// the preview BGM, or a redundant arrow press that lands on the
    /// same row).
    pub current_path: Option<PathBuf>,
    /// Most recent path submitted while the state machine was busy.
    /// `drain_pending_preview` re-runs `play()` with it as soon as
    /// state returns to `Playing` — guarantees the user's *latest*
    /// arrow reaches the audio engine even when mashed faster than
    /// the 250 ms crossfade window.
    pub pending_path: Option<PathBuf>,
}

#[derive(Debug, Default, Clone)]
pub enum PreviewState {
    #[default]
    Idle,
    Playing {
        current: Handle<AudioInstance>,
    },
    Crossfading {
        old: Option<Handle<AudioInstance>>,
        new: Handle<AudioInstance>,
        elapsed_ms: u32,
        fade_in_started: bool,
    },
}

impl PreviewState {
    /// True when a crossfade is in flight (including the initial fade-in
    /// from `Idle`). New swap requests are rejected while busy. This
    /// also acts as the rapid-mash debounce: each swap takes ~250ms,
    /// so requests arriving faster than that are dropped, mirroring
    /// osu-lazer's carousel-snap behavior.
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

/// System: respond to `ScreenFadeTransition` events. Aligns the
/// preview audio with the visual screen fade.
///
/// - `Out`: fade current preview to silence over the same duration
///   as the visual fade (300ms, matches `dtx_ui::SCREEN_TRANSITION_MS`).
/// - `In`: no-op (the new screen's BGM, if any, takes over).
/// - `Done`: no-op. `Out`/OnExit already stops previews; resetting here
///   can desync state from still-playing song-select audio.
pub fn screen_fade_responder_system(
    mut events: MessageReader<ScreenFadeTransition>,
    mut player: ResMut<PreviewPlayer>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    for event in events.read() {
        match event {
            ScreenFadeTransition::Out => {
                // Visual fade-out is 300ms (dtx-ui::SCREEN_TRANSITION_MS).
                player.fade_to_silent(&mut instances, 300);
            }
            ScreenFadeTransition::In => {
                // The audio is already fading out; the new screen's
                // BGM (gameplay) will play via song_loading / orchestrator.
            }
            ScreenFadeTransition::Done => {
                info!("Preview: screen fade done; keeping preview state");
            }
        }
    }
}

/// System: drive the crossfade state machine. Runs every frame in
/// `Update`. No-op unless `state == Crossfading`.
///
/// On reaching `delay_ms`, kicks off the fade-in tween. On reaching
/// `delay_ms + fade_in_ms`, transitions to `Playing { new }`. The
/// `old` handle, if any, was scheduled to stop in `play()` with a
/// fade-out tween of `PREVIEW_FADE_OUT_MS` (150ms) — by the time we
/// resolve here (250ms in) that tween has long completed and the
/// kira instance is gone, so no explicit stop is needed.
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

    /// Fade the currently-playing preview to silence over `ms`
    /// milliseconds, then release the kira instance. Used to align
    /// with `ScreenFade`'s fade-out.
    ///
    /// After this call, the state goes to `Idle` so subsequent `play()`
    /// calls don't trip on a stale handle. The underlying kira
    /// instances are scheduled to stop after their respective fade-out
    /// tweens complete.
    pub fn fade_to_silent(
        &mut self,
        instances: &mut Assets<AudioInstance>,
        ms: u32,
    ) {
        let handles: Vec<Handle<AudioInstance>> = match &self.state {
            PreviewState::Playing { current } => vec![current.clone()],
            PreviewState::Crossfading { old, new, .. } => {
                let mut h = Vec::with_capacity(2);
                if let Some(o) = old {
                    h.push(o.clone());
                }
                h.push(new.clone());
                h
            }
            PreviewState::Idle => {
                self.pending_path = None;
                return;
            }
        };
        for h in &handles {
            stop_with_fade(instances, h, ms);
        }
        self.state = PreviewState::Idle;
        self.current_path = None;
        self.pending_path = None;
    }

    /// Reset state to Idle and clear the previous-index tracking.
    /// Called on `ScreenFadeTransition::Done` so the next time
    /// `play()` is called, it starts fresh (no crossfade in).
    pub fn reset(&mut self) {
        self.state = PreviewState::Idle;
        self.previous_index = None;
        self.current_path = None;
        self.pending_path = None;
    }
    /// Start playing a new preview. Same path is ignored so difficulty
    /// changes sharing preview audio do not restart. A different path
    /// stops any in-flight preview before starting the new one.
    ///
    /// Always enters `Crossfading` state (with `old = None` for the
    /// initial play). This guarantees a 30ms pre-roll + 220ms fade-in
    /// so the first selection is audible, not pinned at the -60dB
    /// mute level. The busy-gate debounces rapid mashing the same as
    /// a swap.
    ///
    /// On a swap from `Playing`, the old handle is stopped with a
    /// 150ms fade-out (the kira instance is released when the tween
    /// completes — prevents the "mashed previews leaking into
    /// Performance" bug where every accepted `play()` left a silent
    /// -60dB kira handle alive in the audio engine).
    pub fn play(
        &mut self,
        audio: &Audio,
        source: Handle<KiraAudioSource>,
        path: PathBuf,
        events: &mut MessageWriter<PreviewSwapEvent>,
        direction: PreviewSwapDirection,
        instances: &mut Assets<AudioInstance>,
    ) -> bool {
        if self.current_path.as_deref() == Some(path.as_path()) {
            info!("Preview: unchanged {}; skip", path.display());
            return true;
        }
        let old_path_for_event = self.current_path.clone();
        if let Some(old) = &old_path_for_event {
            info!(
                "Preview: path changed {} -> {}; stopping main track",
                old.display(),
                path.display()
            );
            audio.stop();
        }
        if self.state.is_busy() {
            info!("Preview: force-stopping in-flight preview before {}", path.display());
            self.stop(instances, 0);
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
            PreviewState::Playing { current } => Some(current.clone()),
            PreviewState::Idle => None,
            PreviewState::Crossfading { .. } => unreachable!("busy-gated above"),
        };

        if let Some(old) = &old_handle {
            // Use stop_with_fade (not set_decibels) so the kira
            // instance is released once the fade-out tween completes.
            stop_with_fade(instances, old, PREVIEW_FADE_OUT_MS);
        }

        let old_path_for_event = if old_handle.is_some() {
            old_path_for_event
        } else {
            None
        };
        self.current_path = Some(path.clone());
        info!(
            "Preview: started {} old_handle={} looping={}",
            path.display(),
            old_handle.is_some(),
            self.looping
        );
        self.state = PreviewState::Crossfading {
            old: old_handle,
            new: new_handle,
            elapsed_ms: 0,
            fade_in_started: false,
        };
        events.write(PreviewSwapEvent {
            old_path: old_path_for_event,
            new_path: path,
            direction,
        });

        true
    }

    /// Stop the current preview with a fade-out, releasing the kira
    /// instance(s). No-op if idle. `fade_out_ms=0` uses the crossfade
    /// module default. Both the fading-in `new` and (if present) the
    /// fading-out `old` handles are scheduled to stop.
    pub fn stop(
        &mut self,
        instances: &mut Assets<AudioInstance>,
        fade_out_ms: u32,
    ) {
        let handles: Vec<Handle<AudioInstance>> = match &self.state {
            PreviewState::Playing { current } => vec![current.clone()],
            PreviewState::Crossfading { old, new, .. } => {
                let mut h = Vec::with_capacity(2);
                if let Some(o) = old {
                    h.push(o.clone());
                }
                h.push(new.clone());
                h
            }
            PreviewState::Idle => {
                info!("Preview: stop requested; already idle");
                self.pending_path = None;
                return;
            }
        };
        let ms = if fade_out_ms == 0 {
            PREVIEW_FADE_OUT_MS
        } else {
            fade_out_ms
        };
        info!("Preview: stopping {} handle(s), fade={}ms", handles.len(), ms);
        for h in &handles {
            stop_with_fade(instances, h, ms);
        }
        self.state = PreviewState::Idle;
        self.current_path = None;
        self.pending_path = None;
    }
}

/// Drain a queued preview request. Usually idle because `play()` now
/// stops in-flight previews immediately on path changes, but kept for
/// older callers that may still fill `pending_path`.
pub fn drain_pending_preview(
    mut player: ResMut<PreviewPlayer>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut cache: ResMut<AudioHandleCache>,
    mut events: MessageWriter<PreviewSwapEvent>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    if player.state.is_busy() {
        return;
    }
    let Some(path) = player.pending_path.take() else {
        return;
    };
    info!("Preview: draining pending {}", path.display());
    // Same-path short-circuit lives inside play(); we still need to
    // resolve the source from the cache so play() can be called. This
    // also re-uses the cached decoded handle, so no extra decode.
    let source = get_or_load(&mut cache, &asset_server, &path);
    let _accepted = player.play(
        &audio,
        source,
        path,
        &mut events,
        PreviewSwapDirection::None,
        &mut instances,
    );
    // play() may legitimately return false only via the same-path
    // short-circuit (returns true) or busy-gate (we cleared busy
    // above). Ignore the result; the path is gone either way.
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
            old: Some(Handle::<AudioInstance>::default()),
            new: Handle::<AudioInstance>::default(),
            elapsed_ms: 0,
            fade_in_started: false,
        };
        assert!(state.is_busy());
        // The "new" handle is reported as current even mid-crossfade.
        assert!(state.current().is_some());
    }

    #[test]
    fn crossfading_with_no_old_still_busy() {
        // Initial fade-in from Idle also gates rapid mashing.
        let state = PreviewState::Crossfading {
            old: None,
            new: Handle::<AudioInstance>::default(),
            elapsed_ms: 0,
            fade_in_started: false,
        };
        assert!(state.is_busy());
        // The "new" handle is the audible one.
        assert!(state.current().is_some());
    }

    #[test]
    fn preview_swap_direction_default_is_none() {
        assert_eq!(PreviewSwapDirection::default(), PreviewSwapDirection::None);
    }

    #[test]
    fn screen_fade_transition_variants() {
        // Just verify the enum exists and Debug is derived.
        let _ = ScreenFadeTransition::Out;
        let _ = ScreenFadeTransition::In;
        let _ = ScreenFadeTransition::Done;
    }

    #[test]
    fn reset_clears_preview_paths() {
        let mut player = PreviewPlayer {
            current_path: Some(PathBuf::from("/songs/a/preview.ogg")),
            pending_path: Some(PathBuf::from("/songs/b/preview.ogg")),
            ..Default::default()
        };
        player.reset();
        assert!(player.current_path.is_none());
        assert!(player.pending_path.is_none());
    }
}
