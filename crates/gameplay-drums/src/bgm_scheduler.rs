//! BGM chip scheduler — plays WAV sounds at their scheduled times.
//!
//! DTX charts use BGM channel (0x01) chips to trigger layered sounds
//! (drums, bass, guitar backing) at specific positions in the chart.
//! The earliest BGM chip drives the tracked `BgmHandle` (AudioClock);
//! later chips play as one-shot SFX layers. Duplicate paths already
//! tracked in `BgmHandle` are skipped.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_core::chart::Chart;
use std::collections::HashSet;
use std::path::Path;

use crate::judge::{auto_chip_target_ms, BarLengthChangeList, BpmChangeList};
use crate::resources::{ActiveChart, ActiveDrumSounds, BgmAdjustState, DrumAudioSettings};
use dtx_core::EChannel;
use dtx_timing::math::ChartTiming;
use game_shell::{AppState, PauseState};

#[derive(Resource, Default, Debug)]
pub struct PlayedBgmChips(pub HashSet<usize>);

/// Index of the chronologically first BGM chip with a non-zero WAV slot.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PrimaryBgmChip(pub Option<usize>);

#[derive(Resource, Default, Debug)]
pub struct BgmRecoveryState {
    attempts: u8,
    last_restart_ms: i64,
    gave_up: bool,
}

const BGM_RECOVERY_COOLDOWN_MS: i64 = 1000;
const BGM_RECOVERY_MAX_ATTEMPTS: u8 = 3;

pub fn bootstrap_primary_bgm_chip(
    chart: &ActiveChart,
    _bpm_changes: &BpmChangeList,
    primary: &PrimaryBgmChip,
    played: &mut PlayedBgmChips,
    audio: &Audio,
    asset_server: &AssetServer,
    bgm: &mut dtx_audio::BgmHandle,
    instances: &mut Assets<AudioInstance>,
    sound_bank: &dtx_audio::ChartSoundBank,
    bgm_enabled: bool,
    master_volume: f32,
    bgm_volume: f32,
) -> bool {
    if !bgm_enabled {
        return true;
    }
    if bgm.instance.is_some() {
        return true;
    }
    let Some(idx) = primary.0 else {
        return false;
    };
    if played.0.contains(&idx) {
        return true;
    }
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    let Some(path) = chip_wav_path(&chart.chart, idx, source_dir) else {
        return false;
    };
    info!("Performance: bootstrap BGM chip {idx} ({path})");
    if let Some(sound) = sound_bank.get(chart.chart.chips[idx].wav_slot) {
        dtx_audio::play_bgm_handle_with_mix(
            audio,
            bgm,
            instances,
            sound.handle.clone(),
            &sound.path.to_string_lossy(),
            sound.volume,
            sound.pan,
            master_volume * bgm_volume,
            // Bootstrap is called from OnEnter(Performance) → start_bgm_on_enter;
            // pass the screen-fade duration so the BGM fades in aligned
            // with the visual fade-in (matches osu's seamless feel).
            dtx_ui::SCREEN_TRANSITION_MS as u32,
        );
    } else {
        dtx_audio::play_bgm_with_volume(
            audio,
            asset_server,
            bgm,
            instances,
            &path,
            dtx_ui::SCREEN_TRANSITION_MS as u32,
            master_volume * bgm_volume,
        );
    }
    true
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PlayedBgmChips>()
        .init_resource::<PrimaryBgmChip>()
        .init_resource::<BgmRecoveryState>()
        .add_systems(
            FixedUpdate,
            (schedule_bgm_chips, recover_primary_bgm)
                .chain()
                .in_set(super::DrumsSets::NoteSpawn)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running)),
        );
}

pub fn reset_played_bgm(
    mut played: ResMut<PlayedBgmChips>,
    mut recovery: ResMut<BgmRecoveryState>,
) {
    played.0.clear();
    *recovery = BgmRecoveryState::default();
}

/// True when the chart schedules at least one BGM chip with a WAV reference.
pub fn chart_has_bgm_chips(chart: &Chart) -> bool {
    chart
        .chips
        .iter()
        .any(|c| c.channel == EChannel::BGM && c.wav_slot != 0)
}

/// Resolve the asset path for a BGM chip's WAV slot.
pub fn chip_wav_path(chart: &Chart, chip_idx: usize, source_dir: Option<&Path>) -> Option<String> {
    let chip = chart.chips.get(chip_idx)?;
    if chip.wav_slot == 0 {
        return None;
    }
    let wav_filename = chart.assets.wav.get(chip.wav_slot)?;
    Some(match source_dir {
        Some(dir) => dir.join(wav_filename).to_string_lossy().to_string(),
        None => wav_filename.to_string(),
    })
}

/// Find the chip index of the earliest BGM chip (by chart time).
pub fn find_primary_bgm_chip(chart: &Chart, timing: ChartTiming<'_>) -> Option<usize> {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    chart
        .chips
        .iter()
        .enumerate()
        .filter(|(_, c)| c.channel == EChannel::BGM && c.wav_slot != 0)
        .min_by_key(|(_, c)| {
            dtx_timing::math::chip_time_ms_with_bpm_and_bar_changes(
                c.measure, c.value, base_bpm, timing,
            )
        })
        .map(|(idx, _)| idx)
}

fn schedule_bgm_chips(
    gameplay_clock: Res<crate::resources::GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    primary: Res<PrimaryBgmChip>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    settings: Res<DrumAudioSettings>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut played: ResMut<PlayedBgmChips>,
    mut active: ResMut<ActiveDrumSounds>,
) {
    if !gameplay_clock.is_started() {
        return;
    }
    let now = gameplay_clock.current_ms;
    let clock_ready = gameplay_clock.is_ready();
    if chart.chart.assets.wav.is_empty() {
        return;
    }
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    let bgm_shift = bgm_adjust.total_ms();
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());

    for (idx, chip) in chart.chart.chips.iter().enumerate() {
        if chip.channel != EChannel::BGM {
            continue;
        }
        if played.0.contains(&idx) {
            continue;
        }
        if chip.wav_slot == 0 {
            played.0.insert(idx);
            continue;
        }
        let target_ms = auto_chip_target_ms(chip, base_bpm, timing, bgm_shift);
        if now < target_ms {
            continue;
        }
        if !settings.bgm_enabled {
            played.0.insert(idx);
            continue;
        }
        let Some(path) = chip_wav_path(&chart.chart, idx, source_dir) else {
            continue;
        };

        if bgm.path.as_deref() == Some(path.as_str()) {
            if bgm_chip_is_confirmed_played(primary.0 == Some(idx), clock_ready, true) {
                played.0.insert(idx);
            }
            continue;
        }

        if primary.0 == Some(idx) {
            info!("Performance: BGM chip {idx} → play_bgm ({path})");
            if let Some(sound) = sound_bank.get(chip.wav_slot) {
                dtx_audio::play_bgm_handle_with_mix(
                    &audio,
                    &mut bgm,
                    &mut instances,
                    sound.handle.clone(),
                    &sound.path.to_string_lossy(),
                    sound.volume,
                    sound.pan,
                    settings.master_volume * settings.bgm_volume,
                    // Subsequent chip swaps: no fade-in (already in Performance).
                    0,
                );
            } else {
                dtx_audio::play_bgm_with_volume(
                    &audio,
                    &asset_server,
                    &mut bgm,
                    &mut instances,
                    &path,
                    0,
                    settings.master_volume * settings.bgm_volume,
                );
            }
        } else {
            let vol = chart.chart.assets.wav.volume(chip.wav_slot);
            let pan = chart.chart.assets.wav.pan(chip.wav_slot);
            let handle = if let Some(sound) = sound_bank.get(chip.wav_slot) {
                dtx_audio::play_sfx_handle(
                    &audio,
                    sound.handle.clone(),
                    sound.volume,
                    sound.pan,
                    settings.master_volume,
                    settings.bgm_volume,
                )
            } else {
                dtx_audio::play_sfx_path(
                    &audio,
                    &asset_server,
                    &path,
                    vol,
                    pan,
                    settings.master_volume,
                    settings.bgm_volume,
                )
            };
            active.track_layer_bgm(handle);
        }
        if bgm_chip_is_confirmed_played(primary.0 == Some(idx), clock_ready, true) {
            played.0.insert(idx);
        }
    }
}

fn recover_primary_bgm(
    gameplay_clock: Res<crate::resources::GameplayClock>,
    start_ms: Res<crate::resources::GameStartMs>,
    chart: Res<ActiveChart>,
    primary: Res<PrimaryBgmChip>,
    completion: Res<crate::orchestrator::DrumsStageCompletion>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    settings: Res<DrumAudioSettings>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut recovery: ResMut<BgmRecoveryState>,
) {
    if completion.end_requested || !gameplay_clock.is_ready() || !settings.bgm_enabled {
        return;
    }
    let Some(handle) = bgm.instance.as_ref() else {
        return;
    };
    if !matches!(audio.state(handle), PlaybackState::Stopped) {
        return;
    }
    if recovery.gave_up {
        return;
    }
    if gameplay_clock.current_ms - recovery.last_restart_ms < BGM_RECOVERY_COOLDOWN_MS {
        return;
    }
    if recovery.attempts >= BGM_RECOVERY_MAX_ATTEMPTS {
        recovery.gave_up = true;
        warn!(
            "Performance: BGM stopped; recovery gave up after {} attempts",
            recovery.attempts
        );
        return;
    }

    let Some(idx) = primary.0 else {
        return;
    };
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    let Some(path) = chip_wav_path(&chart.chart, idx, source_dir) else {
        return;
    };
    let start_seconds = (gameplay_clock.current_ms - start_ms.0).max(0) as f64 / 1000.0;
    warn!(
        "Performance: BGM stopped early; restarting chip {idx} at {:.3}s ({path})",
        start_seconds
    );
    if let Some(sound) = sound_bank.get(chart.chart.chips[idx].wav_slot) {
        dtx_audio::play_bgm_handle_with_mix_from_seconds(
            &audio,
            &mut instances,
            &mut bgm,
            sound.handle.clone(),
            &sound.path.to_string_lossy(),
            sound.volume,
            sound.pan,
            settings.master_volume * settings.bgm_volume,
            start_seconds,
            // Recovery restart mid-performance: no fade-in.
            0,
        );
    } else {
        dtx_audio::play_bgm_from_seconds_with_volume(
            &audio,
            &asset_server,
            &mut bgm,
            &mut instances,
            &path,
            start_seconds,
            0,
            settings.master_volume * settings.bgm_volume,
        );
    }
    recovery.attempts += 1;
    recovery.last_restart_ms = gameplay_clock.current_ms;
}

fn bgm_chip_is_confirmed_played(is_primary: bool, clock_ready: bool, play_requested: bool) -> bool {
    play_requested && (!is_primary || clock_ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::assets::DtxAssets;
    use dtx_core::chart::{Chip, Metadata};

    fn chart_with_bgm_chip(wav_slot: u32) -> Chart {
        let mut assets = DtxAssets::default();
        assets.wav.insert(wav_slot, "drums.ogg".into());
        Chart {
            metadata: Metadata::default(),
            chips: vec![Chip::with_wav(0, EChannel::BGM, 1.0, wav_slot)],
            assets,
            ..Default::default()
        }
    }

    #[test]
    fn chart_has_bgm_chips_true_when_wav_set() {
        assert!(chart_has_bgm_chips(&chart_with_bgm_chip(1)));
    }

    #[test]
    fn chart_has_bgm_chips_false_when_slot_zero() {
        assert!(!chart_has_bgm_chips(&chart_with_bgm_chip(0)));
    }

    #[test]
    fn find_primary_bgm_chip_picks_earliest() {
        let mut assets = DtxAssets::default();
        assets.wav.insert(1, "bass.ogg".into());
        assets.wav.insert(2, "drums.ogg".into());
        let chart = Chart {
            metadata: Metadata::default(),
            chips: vec![
                Chip::with_wav(2, EChannel::BGM, 1.0, 1),
                Chip::with_wav(0, EChannel::BGM, 1.0, 2),
            ],
            assets,
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let timing = ChartTiming {
            bpm_changes: &bpm.changes,
            bar_changes: &[],
        };
        assert_eq!(find_primary_bgm_chip(&chart, timing), Some(1));
    }

    #[test]
    fn primary_bgm_is_not_confirmed_played_until_clock_ready() {
        assert!(!bgm_chip_is_confirmed_played(true, false, true));
        assert!(bgm_chip_is_confirmed_played(true, true, true));
        assert!(bgm_chip_is_confirmed_played(false, false, true));
    }
}
