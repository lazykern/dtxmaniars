//! Audio playback wrapper around `bevy_kira_audio`.
//!
//! Engine layer. Owns the [`BgmHandle`] resource that `dtx-timing` polls
//! each frame to populate `AudioClock`.
//!
//! Reference: `references/DTXmaniaNX-BocuD/FDK/Sound/CSoundTimer.cs` (92 LOC).
//! ADR-0002: audio-clock authoritative for hit-window judgment.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::AudioSource as KiraAudioSource;

pub mod crossfade;
pub mod preview;

/// Audio formats accepted by the runtime decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Ogg,
    Wav,
    Mp3,
}

/// Return the supported audio format for `path`, ignoring extension case.
pub fn supported_audio_format(path: &Path) -> Option<AudioFormat> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "ogg" => Some(AudioFormat::Ogg),
        "wav" => Some(AudioFormat::Wav),
        "mp3" => Some(AudioFormat::Mp3),
        _ => None,
    }
}

pub use preview::{
    drain_pending_preview, get_or_load as get_or_load_audio_handle, preview_tick_system,
    screen_fade_responder_system, AudioHandleCache, PreviewPlayer, PreviewState,
    PreviewSwapDirection, PreviewSwapEvent, ScreenFadeTransition,
};

/// The currently-playing BGM instance, if any.
///
/// `instance` is `None` when nothing is playing. The `dtx-timing` plugin reads
/// this each frame to update the authoritative `AudioClock` resource.
#[derive(Resource, Default, Debug, Clone)]
pub struct BgmHandle {
    pub instance: Option<Handle<AudioInstance>>,
    /// Asset path of the tracked BGM stream (for dedupe against BGM chips).
    pub path: Option<String>,
}

/// Loaded chart audio keyed by DTX WAV slot.
#[derive(Resource, Default, Debug, Clone)]
pub struct ChartSoundBank {
    by_wav_slot: HashMap<u32, LoadedChartSound>,
}

/// One preloaded chart WAV entry.
#[derive(Debug, Clone)]
pub struct LoadedChartSound {
    /// Reusable Bevy audio handle.
    pub handle: Handle<KiraAudioSource>,
    /// Resolved filesystem path, including case-insensitive match.
    pub path: PathBuf,
    /// DTX volume (0..100).
    pub volume: i32,
    /// DTX pan (-100..100).
    pub pan: i32,
}

/// Root plugin. Add to your `App` next to `DefaultPlugins` / `MinimalPlugins`.
///
/// Re-exports `bevy_kira_audio::AudioPlugin` so callers don't need to touch
/// the underlying crate directly.
pub fn plugin(app: &mut App) {
    app.add_plugins(AudioPlugin)
        .init_resource::<BgmHandle>()
        .init_resource::<ChartSoundBank>()
        .init_resource::<DrumPolyphony>()
        .init_resource::<AudioHandleCache>()
        .init_resource::<PreviewPlayer>()
        .add_message::<PreviewSwapEvent>()
        .add_message::<ScreenFadeTransition>()
        .add_systems(
            Update,
            (
                preview_tick_system,
                screen_fade_responder_system,
                drain_pending_preview,
            ),
        );
}

impl ChartSoundBank {
    /// Remove all cached chart audio.
    pub fn clear(&mut self) {
        self.by_wav_slot.clear();
    }

    /// Look up a preloaded WAV slot.
    pub fn get(&self, wav_slot: u32) -> Option<&LoadedChartSound> {
        self.by_wav_slot.get(&wav_slot)
    }

    /// Insert a loaded WAV slot.
    pub fn insert(&mut self, wav_slot: u32, sound: LoadedChartSound) {
        self.by_wav_slot.insert(wav_slot, sound);
    }

    /// Number of loaded WAV slots.
    pub fn len(&self) -> usize {
        self.by_wav_slot.len()
    }

    /// Iterate over every cached audio handle (for load-state polling).
    pub fn handles(&self) -> impl Iterator<Item = &Handle<KiraAudioSource>> {
        self.by_wav_slot.values().map(|sound| &sound.handle)
    }

    /// True when no slots are loaded.
    pub fn is_empty(&self) -> bool {
        self.by_wav_slot.is_empty()
    }

    fn handle_for_path(&self, path: &Path) -> Option<Handle<KiraAudioSource>> {
        self.by_wav_slot
            .values()
            .find(|sound| sound.path == path)
            .map(|sound| sound.handle.clone())
    }
}

/// Preload a chart WAV slot into the current chart sound bank.
pub fn preload_chart_sound(
    asset_server: &AssetServer,
    bank: &mut ChartSoundBank,
    source_dir: Option<&Path>,
    wav_slot: u32,
    filename: &str,
    volume: i32,
    pan: i32,
) -> Handle<KiraAudioSource> {
    let path = match source_dir {
        Some(dir) => resolve_chart_audio_path(dir, filename),
        None => PathBuf::from(filename),
    };
    let handle = bank.handle_for_path(&path).unwrap_or_else(|| {
        asset_server
            .load_builder()
            .override_unapproved()
            .load(path.to_string_lossy().to_string())
    });
    bank.insert(
        wav_slot,
        LoadedChartSound {
            handle: handle.clone(),
            path,
            volume,
            pan,
        },
    );
    handle
}

/// Resolve a chart-relative audio filename, matching case-insensitively if needed.
///
/// DTX charts authored on Windows use `\` as the path separator and may nest
/// samples in per-instrument subfolders (e.g. `Kit\snare 01.ogg`). We normalise
/// `\` to `/` and resolve each component in turn, falling back to a
/// case-insensitive scan per directory level.
pub fn resolve_chart_audio_path(chart_dir: &Path, filename: &str) -> PathBuf {
    let normalized = filename.replace('\\', "/");

    let direct = chart_dir.join(&normalized);
    if direct.exists() {
        return direct;
    }

    let mut current = chart_dir.to_path_buf();
    for component in normalized.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        let candidate = current.join(component);
        if candidate.exists() {
            current = candidate;
            continue;
        }
        match resolve_component_ci(&current, component) {
            Some(matched) => current = matched,
            None => return direct,
        }
    }
    current
}

#[cfg(test)]
mod audio_format_tests {
    use super::*;

    #[test]
    fn supported_audio_format_accepts_case_insensitive_extensions() {
        assert_eq!(
            supported_audio_format(Path::new("song.ogg")),
            Some(AudioFormat::Ogg)
        );
        assert_eq!(
            supported_audio_format(Path::new("song.WaV")),
            Some(AudioFormat::Wav)
        );
        assert_eq!(
            supported_audio_format(Path::new("song.Mp3")),
            Some(AudioFormat::Mp3)
        );
    }

    #[test]
    fn supported_audio_format_rejects_unsupported_or_extensionless_paths() {
        assert_eq!(supported_audio_format(Path::new("song.xa")), None);
        assert_eq!(supported_audio_format(Path::new("song")), None);
    }
}

/// Case-insensitive lookup of a single path component within `dir`.
fn resolve_component_ci(dir: &Path, component: &str) -> Option<PathBuf> {
    let needle = component.to_lowercase();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        if name.to_string_lossy().to_lowercase() == needle {
            return Some(path);
        }
    }
    None
}

/// Per-WAV round-robin voice index for drum polyphony.
///
/// Reference: `CDTX.cs:tチップの再生` — `(n現在 + 1) % nPolyphonicSounds`.
#[derive(Resource, Debug)]
pub struct DrumPolyphony {
    voices: u8,
    next_index: HashMap<u32, u8>,
    active: HashMap<(u32, u8), Handle<AudioInstance>>,
}

impl Default for DrumPolyphony {
    fn default() -> Self {
        Self {
            voices: 4,
            next_index: HashMap::new(),
            active: HashMap::new(),
        }
    }
}

impl DrumPolyphony {
    pub fn set_voices(&mut self, voices: u8) {
        self.voices = voices.clamp(1, 8);
    }

    pub fn voices(&self) -> u8 {
        self.voices
    }

    pub fn reset(&mut self) {
        self.next_index.clear();
        self.active.clear();
    }

    /// Advance round-robin for `wav_slot` and return the slot index used.
    pub fn advance(&mut self, wav_slot: u32) -> u8 {
        let current = self.next_index.get(&wav_slot).copied().unwrap_or(0);
        let next = (current + 1) % self.voices.max(1);
        self.next_index.insert(wav_slot, next);
        next
    }

    /// Return the active handle for a WAV/voice slot.
    pub fn active_voice_handle(&self, wav_slot: u32, voice: u8) -> Option<Handle<AudioInstance>> {
        self.active.get(&(wav_slot, voice)).cloned()
    }

    /// Iterate all active drum voice instances (for pause/resume).
    pub fn active_handles(&self) -> impl Iterator<Item = &Handle<AudioInstance>> {
        self.active.values()
    }

    /// Store a handle in a WAV/voice slot, returning the replaced handle.
    pub fn replace_voice_handle(
        &mut self,
        wav_slot: u32,
        voice: u8,
        handle: Handle<AudioInstance>,
    ) -> Option<Handle<AudioInstance>> {
        self.active.insert((wav_slot, voice), handle)
    }
}

/// Stop the currently-playing BGM instance, if any.
///
/// Stops only the tracked [`BgmHandle`] kira instance — never falls
/// back to a global `audio.stop()`. If the handle is stale (no longer
/// in `Assets<AudioInstance>`, e.g. consumed by another play call),
/// this is a no-op + warn. Falling back to a global stop would kill
/// every kira instance in the mixer, including in-flight previews and
/// the just-started new BGM if called from `play_bgm` re-entrantly.
///
/// The `_audio` parameter is retained for API stability; it is no
/// longer used. Callers can keep passing `&audio` as before.
pub fn stop_bgm(_audio: &Audio, bgm: &mut BgmHandle, instances: &mut Assets<AudioInstance>) {
    if let Some(prev) = bgm.instance.take() {
        if let Some(mut instance) = instances.get_mut(&prev) {
            instance.stop(AudioTween::default());
        } else {
            warn!(
                "stop_bgm: tracked instance {:?} not in Assets<AudioInstance>; \
                 skipping (global stop would kill other kira audio)",
                prev
            );
        }
    }
    bgm.path = None;
}

/// Play a BGM file (path is loaded via `AssetServer`), looped, at default gain.
/// Stops any currently-playing BGM first. Returns the new instance handle.
///
/// `fade_in_ms` — if non-zero, kira fades from silence to the volume set
/// by `with_volume` over that many milliseconds (linear). Used to align
/// BGM onset with a screen fade-in (see `ScreenFadeTransition::In`).
pub fn play_bgm(
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
    path: &str,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    play_bgm_with_volume(audio, asset_server, bgm, instances, path, 1.0, fade_in_ms)
}

/// Play a BGM file at a linear gain (1.0 = unchanged), looped.
pub fn play_bgm_with_volume(
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
    path: &str,
    volume: f32,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    play_bgm_from_seconds_with_volume(
        audio,
        asset_server,
        bgm,
        instances,
        path,
        0.0,
        volume,
        fade_in_ms,
    )
}

/// Play a BGM file from a stream-local offset in seconds.
///
/// See [`play_bgm`] for the `fade_in_ms` semantics.
pub fn play_bgm_from_seconds(
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
    path: &str,
    start_seconds: f64,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    play_bgm_from_seconds_with_volume(
        audio,
        asset_server,
        bgm,
        instances,
        path,
        start_seconds,
        1.0,
        fade_in_ms,
    )
}

/// Play a BGM file from a stream-local offset at a linear gain.
// Orthogonal playback knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn play_bgm_from_seconds_with_volume(
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
    path: &str,
    start_seconds: f64,
    volume: f32,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    stop_bgm(audio, bgm, instances);
    let source = asset_server
        .load_builder()
        .override_unapproved()
        .load(path.to_owned());
    let db = linear_gain_to_db(volume.clamp(0.0, 1.0));
    let handle = if fade_in_ms > 0 {
        audio
            .play(source)
            .looped()
            .start_from(start_seconds.max(0.0))
            .with_volume(db)
            .fade_in(AudioTween::new(
                Duration::from_millis(fade_in_ms as u64),
                AudioEasing::Linear,
            ))
            .handle()
    } else {
        audio
            .play(source)
            .looped()
            .start_from(start_seconds.max(0.0))
            .with_volume(db)
            .handle()
    };
    bgm.instance = Some(handle.clone());
    bgm.path = Some(path.to_owned());
    handle
}

/// Play a preloaded BGM handle, looped, at default gain.
///
/// `fade_in_ms` — see [`play_bgm`].
pub fn play_bgm_handle(
    audio: &Audio,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
    source: Handle<KiraAudioSource>,
    path: &str,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    play_bgm_handle_with_mix(audio, bgm, instances, source, path, 100, 0, 1.0, fade_in_ms)
}

/// Play a preloaded BGM handle, looped, with DTX mix settings.
///
/// `fade_in_ms` — see [`play_bgm`].
// Orthogonal playback knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn play_bgm_handle_with_mix(
    audio: &Audio,
    bgm: &mut BgmHandle,
    instances: &mut Assets<AudioInstance>,
    source: Handle<KiraAudioSource>,
    path: &str,
    dtx_volume: i32,
    dtx_pan: i32,
    master: f32,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    play_bgm_handle_with_mix_from_seconds(
        audio, instances, bgm, source, path, dtx_volume, dtx_pan, master, 0.0, fade_in_ms,
    )
}

/// Play a preloaded BGM handle from a stream-local offset in seconds.
///
/// `fade_in_ms` — see [`play_bgm`].
// Orthogonal playback knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn play_bgm_handle_with_mix_from_seconds(
    audio: &Audio,
    instances: &mut Assets<AudioInstance>,
    bgm: &mut BgmHandle,
    source: Handle<KiraAudioSource>,
    path: &str,
    dtx_volume: i32,
    dtx_pan: i32,
    master: f32,
    start_seconds: f64,
    fade_in_ms: u32,
) -> Handle<AudioInstance> {
    stop_bgm(audio, bgm, instances);
    let gain = dtx_linear(dtx_volume) * master.clamp(0.0, 1.0);
    let panning = (dtx_pan as f32 / 100.0).clamp(-1.0, 1.0);
    let handle = if fade_in_ms > 0 {
        audio
            .play(source)
            .looped()
            .start_from(start_seconds.max(0.0))
            .with_volume(linear_gain_to_db(gain))
            .fade_in(AudioTween::new(
                Duration::from_millis(fade_in_ms as u64),
                AudioEasing::Linear,
            ))
            .with_panning(panning)
            .handle()
    } else {
        audio
            .play(source)
            .looped()
            .start_from(start_seconds.max(0.0))
            .with_volume(linear_gain_to_db(gain))
            .with_panning(panning)
            .handle()
    };
    bgm.instance = Some(handle.clone());
    bgm.path = Some(path.to_owned());
    handle
}

/// System: stop the currently-playing BGM cleanly via `Assets<AudioInstance>`.
pub fn stop_bgm_system(
    audio: Res<Audio>,
    mut bgm: ResMut<BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    stop_bgm(&audio, &mut bgm, &mut instances);
}

/// Play a one-shot sound effect (no loop). Fire-and-forget.
pub fn play_sfx(audio: &Audio, asset_server: &AssetServer, path: &str) {
    let _ = play_sfx_path(audio, asset_server, path, 100, 0, 1.0, 1.0);
}

/// Play a one-shot SFX from a chart path with DTX volume/pan; returns the instance handle.
pub fn play_sfx_path(
    audio: &Audio,
    asset_server: &AssetServer,
    path: &str,
    dtx_volume: i32,
    dtx_pan: i32,
    master: f32,
    channel: f32,
) -> Handle<AudioInstance> {
    let source = asset_server
        .load_builder()
        .override_unapproved()
        .load(path.to_owned());
    play_sfx_handle(audio, source, dtx_volume, dtx_pan, master, channel)
}

/// Pause a single audio instance, if it exists.
pub fn pause_audio_instance(instances: &mut Assets<AudioInstance>, handle: &Handle<AudioInstance>) {
    if let Some(mut inst) = instances.get_mut(handle) {
        inst.pause(AudioTween::default());
    }
}

/// Resume a single audio instance, if it exists.
pub fn resume_audio_instance(
    instances: &mut Assets<AudioInstance>,
    handle: &Handle<AudioInstance>,
) {
    if let Some(mut inst) = instances.get_mut(handle) {
        inst.resume(AudioTween::default());
    }
}

/// Pause every active drum polyphony voice.
pub fn pause_polyphony(instances: &mut Assets<AudioInstance>, polyphony: &DrumPolyphony) {
    for handle in polyphony.active_handles() {
        pause_audio_instance(instances, handle);
    }
}

/// Resume every active drum polyphony voice.
pub fn resume_polyphony(instances: &mut Assets<AudioInstance>, polyphony: &DrumPolyphony) {
    for handle in polyphony.active_handles() {
        resume_audio_instance(instances, handle);
    }
}

/// Stop every active drum polyphony voice.
pub fn stop_polyphony(instances: &mut Assets<AudioInstance>, polyphony: &DrumPolyphony) {
    for handle in polyphony.active_handles() {
        if let Some(mut inst) = instances.get_mut(handle) {
            inst.stop(AudioTween::default());
        }
    }
}

/// Play a preloaded one-shot sound effect with DTX volume/pan.
pub fn play_sfx_handle(
    audio: &Audio,
    source: Handle<KiraAudioSource>,
    dtx_volume: i32,
    dtx_pan: i32,
    master: f32,
    channel: f32,
) -> Handle<AudioInstance> {
    let gain = dtx_linear(dtx_volume) * master.clamp(0.0, 1.0) * channel.clamp(0.0, 1.0);
    audio
        .play(source)
        .with_volume(linear_gain_to_db(gain))
        .with_panning((dtx_pan as f32 / 100.0).clamp(-1.0, 1.0))
        .handle()
}

/// Play a drum hit with polyphony round-robin per WAV slot.
// Orthogonal playback knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn play_drum_hit(
    audio: &Audio,
    asset_server: &AssetServer,
    instances: &mut Assets<AudioInstance>,
    polyphony: &mut DrumPolyphony,
    path: &str,
    wav_slot: u32,
    dtx_volume: i32,
    dtx_pan: i32,
    master: f32,
    drum_channel: f32,
) -> Handle<AudioInstance> {
    let source = asset_server
        .load_builder()
        .override_unapproved()
        .load(path.to_owned());
    play_drum_hit_handle(
        audio,
        instances,
        polyphony,
        source,
        wav_slot,
        dtx_volume,
        dtx_pan,
        master,
        drum_channel,
    )
}

/// Play a preloaded drum hit with polyphony round-robin per WAV slot.
// Orthogonal playback knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn play_drum_hit_handle(
    audio: &Audio,
    instances: &mut Assets<AudioInstance>,
    polyphony: &mut DrumPolyphony,
    source: Handle<KiraAudioSource>,
    wav_slot: u32,
    dtx_volume: i32,
    dtx_pan: i32,
    master: f32,
    drum_channel: f32,
) -> Handle<AudioInstance> {
    let voice = polyphony.advance(wav_slot);
    let gain = dtx_linear(dtx_volume) * master.clamp(0.0, 1.0) * drum_channel.clamp(0.0, 1.0);
    let db = linear_gain_to_db(gain);
    let pan = (dtx_pan as f32 / 100.0).clamp(-1.0, 1.0);
    let handle = audio
        .play(source)
        .with_volume(db)
        .with_panning(pan)
        .handle();
    let _ = instances.get(&handle);
    if let Some(prev) = polyphony.replace_voice_handle(wav_slot, voice, handle.clone()) {
        if let Some(mut instance) = instances.get_mut(&prev) {
            instance.stop(AudioTween::default());
        }
    }
    handle
}

fn dtx_linear(vol: i32) -> f32 {
    if vol <= 0 {
        0.0
    } else {
        (vol as f32 / 100.0).clamp(0.0, 1.0)
    }
}

fn linear_gain_to_db(gain: f32) -> f32 {
    if gain <= 0.0 {
        -100.0
    } else {
        20.0 * gain.log10()
    }
}

/// Set linear gain on an active audio instance.
pub fn set_instance_volume(
    instances: &mut Assets<AudioInstance>,
    handle: &Handle<AudioInstance>,
    gain: f32,
) {
    if let Some(mut instance) = instances.get_mut(handle) {
        instance.set_decibels(
            linear_gain_to_db(gain.clamp(0.0, 1.0)),
            AudioTween::default(),
        );
    }
}

/// Set linear gain on the tracked BGM instance if it exists.
pub fn set_bgm_volume(bgm: &BgmHandle, instances: &mut Assets<AudioInstance>, gain: f32) {
    if let Some(handle) = bgm.instance.as_ref() {
        set_instance_volume(instances, handle, gain);
    }
}

/// Get the current playback position in milliseconds, if BGM is playing.
///
/// Returns `None` for Queued/Stopped states or when no BGM is loaded.
pub fn position_ms(audio: &Audio, bgm: &BgmHandle) -> Option<i64> {
    let handle = bgm.instance.as_ref()?;
    match audio.state(handle) {
        PlaybackState::Playing { position } => Some((position * 1000.0) as i64),
        PlaybackState::Paused { position } => Some((position * 1000.0) as i64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_polyphony_is_four() {
        let p = DrumPolyphony::default();
        assert_eq!(p.voices(), 4);
    }

    #[test]
    fn polyphony_round_robin() {
        let mut p = DrumPolyphony::default();
        assert_eq!(p.advance(1), 1);
        assert_eq!(p.advance(1), 2);
        assert_eq!(p.advance(1), 3);
        assert_eq!(p.advance(1), 0);
    }

    #[test]
    fn linear_gain_zero_is_silent_db() {
        assert_eq!(linear_gain_to_db(0.0), -100.0);
    }

    #[test]
    fn linear_gain_one_is_identity_db() {
        assert!(linear_gain_to_db(1.0).abs() < 1e-6);
    }

    #[test]
    fn polyphony_replaces_handle_in_voice_slot() {
        let mut p = DrumPolyphony::default();
        let voice = p.advance(7);
        let first = Handle::<AudioInstance>::default();
        assert!(p.replace_voice_handle(7, voice, first.clone()).is_none());

        let second = Handle::<AudioInstance>::default();
        let replaced = p.replace_voice_handle(7, voice, second);

        assert_eq!(replaced, Some(first));
    }

    #[test]
    fn polyphony_reset_clears_voice_handles() {
        let mut p = DrumPolyphony::default();
        let voice = p.advance(7);
        p.replace_voice_handle(7, voice, Handle::<AudioInstance>::default());

        p.reset();

        assert!(p.active_voice_handle(7, voice).is_none());
    }

    #[test]
    fn resolve_chart_audio_path_matches_case_insensitively() {
        let dir = std::env::temp_dir().join(format!("dtx_audio_case_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let actual = dir.join("Snare.WAV");
        std::fs::write(&actual, b"not real wav").unwrap();

        let resolved = resolve_chart_audio_path(&dir, "snare.wav");

        assert_eq!(resolved, actual);
        let _ = std::fs::remove_file(actual);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn resolve_chart_audio_path_handles_windows_nested_backslash() {
        let dir = std::env::temp_dir().join(format!("dtx_audio_nest_{}", std::process::id()));
        let sub = dir.join("Kit A");
        std::fs::create_dir_all(&sub).unwrap();
        let actual = sub.join("snare 01.ogg");
        std::fs::write(&actual, b"not real ogg").unwrap();

        let resolved = resolve_chart_audio_path(&dir, "Kit A\\snare 01.ogg");

        assert_eq!(resolved, actual);
        let _ = std::fs::remove_file(actual);
        let _ = std::fs::remove_dir(sub);
        let _ = std::fs::remove_dir(dir);
    }
}
