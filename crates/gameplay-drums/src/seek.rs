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
}

/// Chart time the clock held immediately before the most recent applied
/// seek. `track_attempt_stats` reads it to record a finished attempt's
/// end point: `apply_seek_system` runs at the top of the tick, so by the
/// time stats run (after judge) the clock already holds the new position.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastSeekFrom(pub Option<i64>);

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
            dtx_core::EChannel::SE01
            | dtx_core::EChannel::SE02
            | dtx_core::EChannel::SE03
            | dtx_core::EChannel::SE04
            | dtx_core::EChannel::SE05 => {
                if e.auto_ms < target_ms {
                    played_se.insert(e.chip_idx);
                }
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
}

/// Skip-set + clock parameters for the seek system.
#[derive(SystemParam)]
pub struct SeekState<'w> {
    pub judged: ResMut<'w, JudgedChips>,
    pub played_bgm: ResMut<'w, crate::bgm_scheduler::PlayedBgmChips>,
    pub played_se: ResMut<'w, crate::se_scheduler::PlayedSeChips>,
    pub crossed: ResMut<'w, TimingLineCrossed>,
    pub start_ms: ResMut<'w, GameStartMs>,
    pub clock: ResMut<'w, GameplayClock>,
    pub last_seek_from: ResMut<'w, LastSeekFrom>,
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

    // 1. Stop everything currently sounding (layers, HH, stick SE, drums).
    audio.active.stop_all(&mut audio.instances);
    dtx_audio::stop_polyphony(&mut audio.instances, &audio.polyphony);

    // 2. Rebuild skip-sets from scratch.
    seed_skip_sets(
        &timeline,
        resolved,
        &mut state.judged.0,
        &mut state.played_bgm.0,
        &mut state.played_se.0,
        &mut state.crossed.0,
    );

    // 3. Despawn live notes; the spawner refills from the new `now`.
    for entity in &notes {
        commands.entity(entity).despawn();
    }

    // 4. Queue the BGM restart (started by `start_pending_bgm` while
    //    running — a paused seek must not emit audio).
    audio.pending.0 = None;
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    match timeline.governing_bgm_chip(resolved) {
        Some((idx, chip_ms)) => {
            state.start_ms.0 = chip_ms;
            let start_seconds = (resolved - chip_ms).max(0) as f64 / 1000.0;
            let wav_slot = chart.chart.chips[idx].wav_slot;
            let within_slice = audio
                .sound_bank
                .get(wav_slot)
                .and_then(|s| audio.sources.get(&s.handle))
                .map(|src| start_seconds < src.sound.duration().as_secs_f64())
                // Duration unknown (asset still decoding): try anyway.
                .unwrap_or(true);
            dtx_audio::stop_bgm(&audio.audio, &mut audio.bgm, &mut audio.instances);
            if within_slice {
                if let Some(path) = chip_wav_path(&chart.chart, idx, source_dir) {
                    audio.pending.0 = Some(PendingBgm {
                        wav_slot,
                        path,
                        start_seconds,
                    });
                }
            }
            // else: seek landed past the governing slice's audio — stay
            // silent; the next BGM chip schedules normally.
        }
        None => {
            dtx_audio::stop_bgm(&audio.audio, &mut audio.bgm, &mut audio.instances);
            if !crate::bgm_scheduler::chart_has_bgm_chips(&chart.chart) {
                // Whole-file fallback BGM (no BGM chips): stream position 0
                // is chart time 0.
                state.start_ms.0 = 0;
                if let Some(source_path) = chart.source_path.as_ref() {
                    if let Some(bgm_path) = dtx_core::resolve_bgm_path(source_path, &chart.chart) {
                        audio.pending.0 = Some(PendingBgm {
                            wav_slot: 0,
                            path: bgm_path.to_string_lossy().to_string(),
                            start_seconds: resolved.max(0) as f64 / 1000.0,
                        });
                    }
                }
            } else {
                // Target is before the first BGM chip: leave it unplayed;
                // bgm_scheduler starts it on time. Restore enter-time
                // GameStartMs (first BGM chip's chart time).
                if let Some(&(_, first_ms)) = timeline.bgm_chips.first() {
                    state.start_ms.0 = first_ms;
                }
            }
        }
    }

    // 5. Jump the clock last; next measured BGM position re-snaps it.
    state.last_seek_from.0 = Some(state.clock.current_ms);
    state.clock.seek(resolved);
    info!(
        "seek: target={} resolved={} (snap {:?})",
        seek.target_ms, resolved, seek.snap
    );
}

/// Start a queued BGM restart. Runs only while unpaused so a seek made
/// from the pause menu starts audio exactly on resume.
pub fn start_pending_bgm(
    mut pending: ResMut<PendingBgmStart>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    settings: Res<DrumAudioSettings>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let Some(p) = pending.0.take() else {
        return;
    };
    if let Some(sound) = sound_bank.get(p.wav_slot) {
        dtx_audio::play_bgm_handle_with_mix_from_seconds(
            &audio,
            &mut instances,
            &mut bgm,
            sound.handle.clone(),
            &sound.path.to_string_lossy(),
            sound.volume,
            sound.pan,
            settings.master_volume,
            p.start_seconds,
            0,
        );
    } else {
        dtx_audio::play_bgm_from_seconds(
            &audio,
            &asset_server,
            &mut bgm,
            &mut instances,
            &p.path,
            p.start_seconds,
            0,
        );
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
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),  // 0ms
                Chip::new(0, EChannel::BassDrum, 0.0),     // 0ms
                Chip::new(1, EChannel::Snare, 0.0),        // 2000ms
                Chip::with_wav(2, EChannel::SE01, 0.0, 2), // 4000ms
                Chip::new(3, EChannel::BassDrum, 0.0),     // 6000ms
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
        assert!(!j.contains(&4), "chip after target stays live");
        assert!(b.contains(&0), "bgm chip at 0 marked played");
        assert!(s.contains(&3), "se chip before target marked played");
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
