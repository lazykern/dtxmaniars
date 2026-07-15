//! Seek engine op: position playback at an arbitrary chart time.
//!
//! `SeekToChartTime` is the ONLY entry point to seeking. One system
//! (`apply_seek_system`) owns the ordering: stop audio → reseed
//! skip-sets → despawn notes → queue BGM restart → jump the clock.
//! Consumers: practice UI, A/B loop watcher, (later) live preview and
//! trainers.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::AudioSource as KiraAudioSource;

use crate::bgm_scheduler::chip_wav_path;
use crate::components::NoteVisual;
use crate::judge::JudgedChips;
use crate::resources::{
    ActiveChart, ActiveDrumSounds, DrumAudioSettings, GameStartMs, GameplayClock, TimingLineCrossed,
};
use crate::timeline::{ChipTimeline, SnapDivisor};

/// Request to jump playback to a chart time.
#[derive(Message, Debug, Clone, Copy)]
pub struct SeekToChartTime {
    /// Requested chart time (ms). Snapped by the engine when `snap` is set.
    pub target_ms: i64,
    /// Snap the target down to this grid before applying.
    pub snap: Option<SnapDivisor>,
    /// Chart time the *attempt* conceptually starts at (e.g. the A-marker
    /// when `target_ms` includes pre-roll). Consumers that track section
    /// stats read this; `None` means "same as the applied target".
    pub attempt_start_ms: Option<i64>,
}

/// BGM restart queued by a seek; started by [`start_pending_bgm`] on the
/// next running tick. Deferring the start (instead of playing inside the
/// seek) keeps paused-seek correct: audio only starts once unpaused.
#[derive(Resource, Default, Debug, Clone)]
pub struct PendingBgmStart(pub Option<PendingBgm>);

#[derive(Debug, Clone)]
pub struct PendingBgm {
    /// WAV slot to fetch from the sound bank; 0 = load `path` directly.
    pub wav_slot: u32,
    pub path: String,
    pub start_seconds: f64,
    pub volume: i32,
    pub pan: i32,
}

impl PendingBgm {
    pub fn playback_mix(&self, sound_bank: &dtx_audio::ChartSoundBank) -> (i32, i32) {
        sound_bank
            .get(self.wav_slot)
            .map_or((self.volume, self.pan), |sound| (sound.volume, sound.pan))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAudioKind {
    LayerBgm,
    AutoSe(dtx_core::EChannel),
}

#[derive(Debug, Clone)]
pub struct PendingAudioSlice {
    pub chip_idx: usize,
    pub wav_slot: u32,
    pub path: String,
    pub start_seconds: f64,
    pub volume: i32,
    pub pan: i32,
    pub kind: PendingAudioKind,
}

impl PendingAudioSlice {
    pub fn playback_mix(&self, sound_bank: &dtx_audio::ChartSoundBank) -> (i32, i32) {
        sound_bank
            .get(self.wav_slot)
            .map_or((self.volume, self.pan), |sound| (sound.volume, sound.pan))
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct PendingAudioStarts(pub Vec<PendingAudioSlice>);

/// Chart time the clock held immediately before the most recent applied
/// seek. `track_attempt_stats` reads it to record a finished attempt's
/// end point: `apply_seek_system` runs at the top of the tick, so by the
/// time stats run (after judge) the clock already holds the new position.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastSeekFrom(pub Option<i64>);

/// Chip indices already passed by non-judged Setup/Editing preview playback.
/// This keeps preview reconstruction independent from gameplay judgment state.
#[derive(Resource, Default, Debug, Clone)]
pub struct PreviewSkippedChips(pub HashSet<usize>);

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct StoppedSeekRebuild(pub bool);

pub(crate) fn stopped_seek_rebuild_pending(pending: Res<StoppedSeekRebuild>) -> bool {
    pending.0
}

pub(crate) fn clear_stopped_seek_rebuild(mut pending: ResMut<StoppedSeekRebuild>) {
    pending.0 = false;
}

pub(crate) fn reset_preview_skipped_chips(mut skipped: ResMut<PreviewSkippedChips>) {
    skipped.0.clear();
}

pub(crate) fn reset_seek_transients(
    mut pending_bgm: ResMut<PendingBgmStart>,
    mut pending_audio: ResMut<PendingAudioStarts>,
    mut stopped_rebuild: ResMut<StoppedSeekRebuild>,
    mut last_seek: ResMut<LastSeekFrom>,
    mut seeks: ResMut<Messages<SeekToChartTime>>,
) {
    pending_bgm.0 = None;
    pending_audio.0.clear();
    stopped_rebuild.0 = false;
    last_seek.0 = None;
    seeks.clear();
}

/// Rebuild all skip-sets for playback positioned at `target_ms`.
///
/// - `judged`: every chip strictly before the target in the judgement
///   timebase (playable or not — mirrors what a played-through stage
///   would contain, so spawner/judge/miss/autoplay all skip them).
/// - `played_bgm`: BGM chips at or before the target in the auto
///   timebase (the governing chip is restarted manually by the caller).
/// - `played_se`: SE chips strictly before the target (auto timebase).
/// - `crossed`: timing lines strictly before the target.
pub fn seed_skip_sets(
    timeline: &ChipTimeline,
    target_ms: i64,
    judged: &mut HashSet<usize>,
    played_bgm: &mut HashSet<usize>,
    played_se: &mut HashSet<usize>,
    crossed: &mut HashSet<usize>,
) {
    judged.clear();
    played_bgm.clear();
    played_se.clear();
    crossed.clear();
    for e in &timeline.entries {
        if e.judge_ms < target_ms {
            judged.insert(e.chip_idx);
        }
        match e.channel {
            dtx_core::EChannel::BGM => {
                if e.auto_ms <= target_ms {
                    played_bgm.insert(e.chip_idx);
                }
            }
            channel
                if (channel.is_se() || channel.is_timed_system_sound())
                    && e.auto_ms < target_ms =>
            {
                played_se.insert(e.chip_idx);
            }
            _ => {}
        }
    }
    for (i, &ms) in timeline.timing_line_ms.iter().enumerate() {
        if ms < target_ms {
            crossed.insert(i);
        }
    }
}

/// Audio-side parameters for the seek system, bundled to stay under
/// Bevy's system-param ceiling (see orchestrator.rs:75-81).
#[derive(SystemParam)]
pub struct SeekAudio<'w> {
    pub audio: Res<'w, Audio>,
    pub sound_bank: Res<'w, dtx_audio::ChartSoundBank>,
    pub sources: Res<'w, Assets<KiraAudioSource>>,
    pub bgm: ResMut<'w, dtx_audio::BgmHandle>,
    pub instances: ResMut<'w, Assets<AudioInstance>>,
    pub polyphony: ResMut<'w, dtx_audio::DrumPolyphony>,
    pub active: ResMut<'w, ActiveDrumSounds>,
    pub pending: ResMut<'w, PendingBgmStart>,
    pub pending_slices: ResMut<'w, PendingAudioStarts>,
}

/// Skip-set + clock parameters for the seek system.
#[derive(SystemParam)]
pub struct SeekState<'w> {
    pub judged: ResMut<'w, JudgedChips>,
    pub preview_skipped: ResMut<'w, PreviewSkippedChips>,
    pub practice_flow: Option<Res<'w, crate::practice::PracticeFlow>>,
    pub played_bgm: ResMut<'w, crate::bgm_scheduler::PlayedBgmChips>,
    pub primary_bgm: Res<'w, crate::bgm_scheduler::PrimaryBgmChip>,
    pub played_se: ResMut<'w, crate::se_scheduler::PlayedSeChips>,
    pub crossed: ResMut<'w, TimingLineCrossed>,
    pub start_ms: ResMut<'w, GameStartMs>,
    pub clock: ResMut<'w, GameplayClock>,
    pub last_seek_from: ResMut<'w, LastSeekFrom>,
    pub bga_clock: ResMut<'w, dtx_bga::BgaClock>,
    pub stopped_rebuild: ResMut<'w, StoppedSeekRebuild>,
    pub mixer_cursor: Option<ResMut<'w, crate::mixer_events::MixerEventCursor>>,
    pub mixer_eligibility: Option<ResMut<'w, crate::mixer_events::MixerEligibility>>,
}

pub fn apply_seek_system(
    mut seeks: MessageReader<SeekToChartTime>,
    timeline: Res<ChipTimeline>,
    chart: Res<ActiveChart>,
    mut audio: SeekAudio,
    mut state: SeekState,
    notes: Query<Entity, With<NoteVisual>>,
    mut commands: Commands,
) {
    // Coalesce: only the last seek this tick wins.
    let Some(seek) = seeks.read().last().copied() else {
        return;
    };
    if !state.clock.is_started() || timeline.entries.is_empty() {
        return;
    }

    let resolved = match seek.snap {
        Some(snap) => timeline.resolve_snap(seek.target_ms, snap),
        None => seek.target_ms.clamp(0, timeline.end_ms.max(0)),
    };
    state.stopped_rebuild.0 = state.practice_flow.as_ref().is_some_and(|flow| {
        flow.phase != crate::practice::PracticePhase::Running
            && flow.preview == crate::practice::PreviewState::Stopped
    });

    if let (Some(cursor), Some(eligibility)) =
        (&mut state.mixer_cursor, &mut state.mixer_eligibility)
    {
        cursor.rebuild_at(resolved, eligibility);
    }

    // 1. Stop everything currently sounding (layers, HH, stick SE, drums).
    audio.active.stop_all(&mut audio.instances);
    dtx_audio::stop_polyphony(&mut audio.instances, &audio.polyphony);
    audio.active.reset();
    audio.polyphony.reset();

    // 2. Rebuild skip-sets from scratch.
    if state
        .practice_flow
        .as_ref()
        .is_some_and(|flow| flow.phase != crate::practice::PracticePhase::Running)
    {
        seed_skip_sets(
            &timeline,
            resolved,
            &mut state.preview_skipped.0,
            &mut state.played_bgm.0,
            &mut state.played_se.0,
            &mut state.crossed.0,
        );
    } else {
        seed_skip_sets(
            &timeline,
            resolved,
            &mut state.judged.0,
            &mut state.played_bgm.0,
            &mut state.played_se.0,
            &mut state.crossed.0,
        );
        state.preview_skipped.0.clone_from(&state.judged.0);
    }

    // 3. Despawn live notes; the spawner refills from the new `now`.
    for entity in &notes {
        commands.entity(entity).despawn();
    }

    // 4. Queue every audio slice still spanning the target. Starts are
    //    deferred until the seek has fully reconstructed runtime state.
    audio.pending.0 = None;
    audio.pending_slices.0.clear();
    dtx_audio::stop_bgm(&audio.audio, &mut audio.bgm, &mut audio.instances);
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    let authoritative_bgm_path = state
        .primary_bgm
        .0
        .and_then(|primary| chip_wav_path(&chart.chart, primary, source_dir));
    let mut active = timeline
        .entries
        .iter()
        .filter(|entry| {
            (entry.channel == dtx_core::EChannel::BGM && entry.auto_ms <= resolved)
                || ((entry.channel.is_se() || entry.channel.is_timed_system_sound())
                    && entry.auto_ms < resolved)
        })
        .filter_map(|entry| {
            let chip = chart.chart.chips.get(entry.chip_idx)?;
            if chip.wav_slot == 0
                || state
                    .mixer_eligibility
                    .as_ref()
                    .is_some_and(|eligibility| !eligibility.is_slot_eligible(chip.wav_slot))
            {
                return None;
            }
            let mut start_seconds = (resolved - entry.auto_ms).max(0) as f64 / 1000.0;
            let decoded_duration = audio
                .sound_bank
                .get(chip.wav_slot)
                .and_then(|sound| audio.sources.get(&sound.handle))
                .map(|source| source.sound.duration().as_secs_f64());
            if state.primary_bgm.0 == Some(entry.chip_idx) {
                if let Some(duration) = decoded_duration.filter(|duration| *duration > 0.0) {
                    start_seconds %= duration;
                }
            } else if decoded_duration.is_some_and(|duration| start_seconds >= duration) {
                return None;
            }
            Some((
                entry.auto_ms,
                PendingAudioSlice {
                    chip_idx: entry.chip_idx,
                    wav_slot: chip.wav_slot,
                    path: chip_wav_path(&chart.chart, entry.chip_idx, source_dir)?,
                    start_seconds,
                    volume: chart.chart.assets.wav.volume(chip.wav_slot),
                    pan: chart.chart.assets.wav.pan(chip.wav_slot),
                    kind: if entry.channel == dtx_core::EChannel::BGM {
                        PendingAudioKind::LayerBgm
                    } else {
                        PendingAudioKind::AutoSe(entry.channel)
                    },
                },
            ))
        })
        .collect::<Vec<_>>();
    active.sort_by_key(|(at_ms, slice)| (*at_ms, slice.chip_idx));

    let authoritative_bgm = state.primary_bgm.0.filter(|primary| {
        active.iter().any(|(_, slice)| {
            slice.chip_idx == *primary && slice.kind == PendingAudioKind::LayerBgm
        })
    });
    for (at_ms, slice) in active {
        if authoritative_bgm == Some(slice.chip_idx) {
            state.start_ms.0 = at_ms;
            audio.pending.0 = Some(PendingBgm {
                wav_slot: slice.wav_slot,
                path: slice.path,
                start_seconds: slice.start_seconds,
                volume: slice.volume,
                pan: slice.pan,
            });
        } else {
            if slice.kind == PendingAudioKind::LayerBgm
                && authoritative_bgm_path.as_deref() == Some(slice.path.as_str())
            {
                continue;
            }
            if let PendingAudioKind::AutoSe(channel) = slice.kind {
                if channel.is_se() {
                    audio.pending_slices.0.retain(|queued| {
                        !matches!(queued.kind, PendingAudioKind::AutoSe(old) if old == channel)
                    });
                }
            }
            audio.pending_slices.0.push(slice);
        }
    }

    let mut voices_by_slot = std::collections::HashMap::<u32, u8>::new();
    let mut keep = vec![true; audio.pending_slices.0.len()];
    for (index, slice) in audio.pending_slices.0.iter().enumerate().rev() {
        if matches!(slice.kind, PendingAudioKind::AutoSe(_)) {
            let count = voices_by_slot.entry(slice.wav_slot).or_default();
            if *count >= audio.polyphony.voices() {
                keep[index] = false;
            } else {
                *count += 1;
            }
        }
    }
    let mut index = 0;
    audio.pending_slices.0.retain(|_| {
        let retain = keep[index];
        index += 1;
        retain
    });

    if authoritative_bgm.is_none() {
        if !crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart) {
            state.start_ms.0 = 0;
            if let Some(source_path) = chart.source_path.as_ref() {
                if let Some(bgm_path) = dtx_core::resolve_bgm_path(source_path, &chart.chart) {
                    audio.pending.0 = Some(PendingBgm {
                        wav_slot: 0,
                        path: bgm_path.to_string_lossy().to_string(),
                        start_seconds: resolved.max(0) as f64 / 1000.0,
                        volume: 100,
                        pan: 0,
                    });
                }
            }
        } else if let Some(&(_, first_ms)) = timeline.bgm_chips.first() {
            state.start_ms.0 = first_ms;
        }
    }

    // 5. Jump the clock last; next measured BGM position re-snaps it.
    state.last_seek_from.0 = Some(state.clock.current_ms);
    state.clock.seek(resolved);
    state.bga_clock.current_ms = resolved;
    info!(
        "seek: target={} resolved={} (snap {:?})",
        seek.target_ms, resolved, seek.snap
    );
}

/// Start a queued BGM restart. Runs only while unpaused so a seek made
/// from the pause menu starts audio exactly on resume.
pub fn start_pending_bgm(
    mut pending: ResMut<PendingBgmStart>,
    mut pending_slices: ResMut<PendingAudioStarts>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    settings: Res<DrumAudioSettings>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mixer: Option<Res<crate::mixer_events::MixerEligibility>>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    mut active: ResMut<ActiveDrumSounds>,
) {
    if let Some(p) = pending.0.take() {
        if settings.bgm_enabled
            && (p.wav_slot == 0
                || mixer
                    .as_ref()
                    .is_none_or(|eligibility| eligibility.is_slot_eligible(p.wav_slot)))
        {
            let (volume, pan) = p.playback_mix(&sound_bank);
            if let Some(sound) = sound_bank.get(p.wav_slot) {
                dtx_audio::play_bgm_handle_with_mix_from_seconds(
                    &audio,
                    &mut instances,
                    &mut bgm,
                    sound.handle.clone(),
                    &sound.path.to_string_lossy(),
                    sound.volume,
                    sound.pan,
                    settings.bgm_gain(),
                    p.start_seconds,
                    0,
                );
            } else {
                let source = asset_server
                    .load_builder()
                    .override_unapproved()
                    .load(p.path.clone());
                dtx_audio::play_bgm_handle_with_mix_from_seconds(
                    &audio,
                    &mut instances,
                    &mut bgm,
                    source,
                    &p.path,
                    volume,
                    pan,
                    settings.bgm_gain(),
                    p.start_seconds,
                    0,
                );
            }
        }
    }

    for slice in std::mem::take(&mut pending_slices.0) {
        if mixer
            .as_ref()
            .is_some_and(|eligibility| !eligibility.is_slot_eligible(slice.wav_slot))
        {
            continue;
        }
        let source = sound_bank.get(slice.wav_slot).map_or_else(
            || {
                asset_server
                    .load_builder()
                    .override_unapproved()
                    .load(slice.path.clone())
            },
            |sound| sound.handle.clone(),
        );
        let (volume, pan) = slice.playback_mix(&sound_bank);
        match slice.kind {
            PendingAudioKind::LayerBgm if settings.bgm_enabled => {
                let handle = dtx_audio::play_sfx_handle_from_seconds(
                    &audio,
                    source,
                    volume,
                    pan,
                    settings.bgm_gain(),
                    1.0,
                    slice.start_seconds,
                );
                active.track_layer_bgm(handle);
            }
            PendingAudioKind::AutoSe(channel) if settings.drum_enabled => {
                let handle = dtx_audio::play_drum_hit_handle_from_seconds(
                    &audio,
                    &mut instances,
                    &mut polyphony,
                    source,
                    slice.wav_slot,
                    volume,
                    pan,
                    settings.master_volume,
                    settings.drum_volume,
                    slice.start_seconds,
                );
                if channel.is_se() {
                    if let Some(previous) = active.stick_se_instances.insert(channel, handle) {
                        if let Some(mut instance) = instances.get_mut(&previous) {
                            instance.stop(AudioTween::default());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::assets::DtxAssets;
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM: measure = 2000ms.
    fn chart() -> Chart {
        let mut assets = DtxAssets::default();
        assets.wav.insert(1, "bgm.ogg".into());
        assets.wav.insert(2, "se.ogg".into());
        Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),   // 0ms
                Chip::new(0, EChannel::BassDrum, 0.0),      // 0ms
                Chip::new(1, EChannel::Snare, 0.0),         // 2000ms
                Chip::with_wav(2, EChannel::SE32, 0.0, 2),  // 4000ms
                Chip::with_wav(2, EChannel::Click, 0.5, 2), // 5000ms
                Chip::new(3, EChannel::BassDrum, 0.0),      // 6000ms
            ],
            assets,
            ..Default::default()
        }
    }

    fn timeline() -> ChipTimeline {
        let c = chart();
        let bpm = BpmChangeList::from_chart(&c);
        let bar = BarLengthChangeList::from_chart(&c);
        ChipTimeline::from_chart(&c, &bpm, &bar, 0, 8_000)
    }

    fn seeded(
        target: i64,
    ) -> (
        HashSet<usize>,
        HashSet<usize>,
        HashSet<usize>,
        HashSet<usize>,
    ) {
        let tl = timeline();
        let (mut j, mut b, mut s, mut c) = (
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
        );
        seed_skip_sets(&tl, target, &mut j, &mut b, &mut s, &mut c);
        (j, b, s, c)
    }

    #[test]
    fn forward_seek_marks_everything_before_target() {
        let (j, b, s, _) = seeded(5_000);
        assert!(
            j.contains(&1) && j.contains(&2),
            "drum chips before target judged"
        );
        assert!(!j.contains(&5), "chip after target stays live");
        assert!(b.contains(&0), "bgm chip at 0 marked played");
        assert!(s.contains(&3), "se chip before target marked played");
    }

    #[test]
    fn forward_seek_marks_timed_system_sounds_played() {
        let (_, _, played_se, _) = seeded(5_500);
        assert!(played_se.contains(&4), "click before target marked played");
    }

    #[test]
    fn backward_seek_to_zero_clears_everything() {
        let (j, b, s, c) = seeded(0);
        assert!(j.is_empty());
        assert!(
            b.contains(&0),
            "bgm chip exactly at target is governing → marked"
        );
        assert!(s.is_empty());
        assert!(c.is_empty());
    }

    #[test]
    fn sets_are_rebuilt_not_patched() {
        let tl = timeline();
        let mut j: HashSet<usize> = (0..100).collect();
        let (mut b, mut s, mut c) = (HashSet::new(), HashSet::new(), HashSet::new());
        seed_skip_sets(&tl, 1_000, &mut j, &mut b, &mut s, &mut c);
        assert!(j.len() <= tl.entries.len(), "stale indices must be gone");
        assert!(!j.contains(&99));
    }

    #[test]
    fn timing_lines_before_target_marked_crossed() {
        let tl = timeline();
        let (_, _, _, c) = seeded(2_001);
        // Lines at 0..=2000 crossed, later ones not.
        assert!(!c.is_empty());
        for &i in &c {
            assert!(tl.timing_line_ms[i] < 2_001);
        }
    }
}
