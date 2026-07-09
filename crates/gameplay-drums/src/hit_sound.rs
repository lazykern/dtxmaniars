//! Hit sound playback on judgments and empty hits.
//!
//! Reference: BocuD `CStagePerfDrumsScreen.cs:tProcessDrumHit`, `tPlaySound`;
//! `CStagePerfCommonScreen.cs:r空うちChip`.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_core::EChannel;
use dtx_scoring::JudgmentKind;

use crate::drum_groups::{
    chip_over_pad, empty_hit_fallback_lanes, nearest_chip_on_channel, sound_pad_channel, DrumPad,
};
use crate::events::{EmptyHit, JudgmentEvent};
use crate::judge::{auto_chip_target_ms, chip_target_ms, BarLengthChangeList, BpmChangeList};
use crate::lane_map::lane_channel;
use crate::resources::{
    ActiveChart, ActiveDrumSounds, BgmAdjustState, CurrentEmptyHitTemplates, DrumAudioSettings,
    DrumGameplaySettings, GameplayClock,
};
use dtx_timing::math::ChartTiming;
use game_shell::{AppState, PauseState};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CurrentEmptyHitTemplates>()
        .init_resource::<ActiveDrumSounds>()
        .add_systems(
            FixedUpdate,
            (
                capture_empty_hit_templates,
                play_judgment_sounds,
                play_empty_hit_sounds,
            )
                .chain()
                .in_set(super::DrumsSets::Score)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running)),
        );
}

fn capture_empty_hit_templates(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    mut templates: ResMut<CurrentEmptyHitTemplates>,
) {
    if !clock.is_ready() || chart.chart.empty_hit_events.is_empty() {
        return;
    }
    let now = clock.current_ms;
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let bgm_shift = bgm_adjust.total_ms();
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    for event in &chart.chart.empty_hit_events {
        let target_ms = auto_chip_target_ms(
            &dtx_core::Chip::with_wav(
                event.measure,
                EChannel::HiHatClose,
                event.value,
                event.wav_slot,
            ),
            base_bpm,
            timing,
            bgm_shift,
        );
        if target_ms <= now {
            templates.set(event.lane, *event);
        }
    }
}

fn play_judgment_sounds(
    mut events: MessageReader<JudgmentEvent>,
    settings: Res<DrumAudioSettings>,
    drum_settings: Res<DrumGameplaySettings>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut active: ResMut<ActiveDrumSounds>,
) {
    if !settings.drum_enabled || chart.chart.assets.wav.is_empty() {
        return;
    }
    let source_dir = chart
        .source_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };

    for ev in events.read() {
        if ev.kind == JudgmentKind::Miss {
            continue;
        }
        let Some(pad) = DrumPad::from_lane(ev.lane) else {
            continue;
        };
        let Some((wav_slot, channel)) = resolve_judgment_sound(
            pad,
            ev.chip_idx,
            ev.delta_ms + chip_target_ms(&chart.chart.chips[ev.chip_idx], base_bpm, timing),
            &chart,
            &drum_settings,
            timing,
        ) else {
            continue;
        };
        let Some(path) = wav_path(&chart.chart, wav_slot, source_dir.as_deref()) else {
            continue;
        };
        let vol = chart.chart.assets.wav.volume(wav_slot);
        let pan = chart.chart.assets.wav.pan(wav_slot);
        maybe_mute_hh_on_close(channel, &audio, &mut instances, &mut active);
        let handle = if let Some(sound) = sound_bank.get(wav_slot) {
            dtx_audio::play_drum_hit_handle(
                &audio,
                &mut instances,
                &mut polyphony,
                sound.handle.clone(),
                wav_slot,
                sound.volume,
                sound.pan,
                settings.master_volume,
                settings.drum_volume,
            )
        } else {
            dtx_audio::play_drum_hit(
                &audio,
                &asset_server,
                &mut instances,
                &mut polyphony,
                &path,
                wav_slot,
                vol,
                pan,
                settings.master_volume,
                settings.drum_volume,
            )
        };
        track_hh_instance(channel, handle, &mut active);
    }
}

fn play_empty_hit_sounds(
    mut events: MessageReader<EmptyHit>,
    settings: Res<DrumAudioSettings>,
    drum_settings: Res<DrumGameplaySettings>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    templates: Res<CurrentEmptyHitTemplates>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut active: ResMut<ActiveDrumSounds>,
) {
    if !settings.drum_enabled || chart.chart.assets.wav.is_empty() {
        return;
    }
    let source_dir = chart
        .source_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };

    for hit in events.read() {
        let Some(pad) = DrumPad::from_lane(hit.lane) else {
            continue;
        };
        let (wav_slot, channel) = resolve_empty_hit_sound(
            pad,
            hit.audio_ms,
            &chart,
            timing,
            &templates,
            &drum_settings,
        );
        let Some(path) = wav_path(&chart.chart, wav_slot, source_dir.as_deref()) else {
            continue;
        };
        let vol = chart.chart.assets.wav.volume(wav_slot);
        let pan = chart.chart.assets.wav.pan(wav_slot);
        maybe_mute_hh_on_close(channel, &audio, &mut instances, &mut active);
        let handle = if let Some(sound) = sound_bank.get(wav_slot) {
            dtx_audio::play_drum_hit_handle(
                &audio,
                &mut instances,
                &mut polyphony,
                sound.handle.clone(),
                wav_slot,
                sound.volume,
                sound.pan,
                settings.master_volume,
                settings.drum_volume,
            )
        } else {
            dtx_audio::play_drum_hit(
                &audio,
                &asset_server,
                &mut instances,
                &mut polyphony,
                &path,
                wav_slot,
                vol,
                pan,
                settings.master_volume,
                settings.drum_volume,
            )
        };
        track_hh_instance(channel, handle, &mut active);
    }
}

fn resolve_judgment_sound(
    pad: DrumPad,
    judged_idx: usize,
    audio_ms: i64,
    chart: &ActiveChart,
    drum_settings: &DrumGameplaySettings,
    timing: ChartTiming<'_>,
) -> Option<(u32, EChannel)> {
    let judged = chart.chart.chips.get(judged_idx)?;
    if chip_over_pad(pad, &drum_settings.config) {
        if judged.wav_slot == 0 {
            return None;
        }
        return Some((judged.wav_slot, judged.channel));
    }
    let pad_ch = sound_pad_channel(pad, &drum_settings.presence);
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    if let Some((_idx, wav_slot, channel)) =
        nearest_chip_on_channel(pad_ch, audio_ms, &chart.chart, base_bpm, timing)
    {
        if wav_slot != 0 {
            return Some((wav_slot, channel));
        }
    }
    if judged.wav_slot == 0 {
        return None;
    }
    Some((judged.wav_slot, judged.channel))
}

fn resolve_empty_hit_sound(
    pad: DrumPad,
    audio_ms: i64,
    chart: &ActiveChart,
    timing: ChartTiming<'_>,
    templates: &CurrentEmptyHitTemplates,
    drum_settings: &DrumGameplaySettings,
) -> (u32, EChannel) {
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    for &lane in empty_hit_fallback_lanes(pad, &drum_settings.groups) {
        if let Some(ev) = templates.get(lane) {
            if ev.wav_slot != 0 {
                return (
                    ev.wav_slot,
                    lane_channel(lane).unwrap_or(EChannel::HiHatClose),
                );
            }
        }
        if let Some((wav_slot, channel)) =
            find_nearest_chip_wav(&chart.chart, lane, audio_ms, base_bpm, timing)
        {
            return (wav_slot, channel);
        }
    }
    (0, lane_channel(pad.lane()).unwrap_or(EChannel::HiHatClose))
}

fn find_nearest_chip_wav(
    chart: &dtx_core::Chart,
    lane: u8,
    audio_ms: i64,
    base_bpm: f32,
    timing: ChartTiming<'_>,
) -> Option<(u32, EChannel)> {
    let lane_ch = lane_channel(lane)?;
    let mut best: Option<(u32, EChannel, i64)> = None;
    for chip in chart.chips.iter() {
        if chip.channel != lane_ch || chip.wav_slot == 0 {
            continue;
        }
        let target_ms = chip_target_ms(chip, base_bpm, timing);
        let dist = (audio_ms - target_ms).abs();
        match best {
            Some((_, _, d)) if d <= dist => {}
            _ => best = Some((chip.wav_slot, chip.channel, dist)),
        }
    }
    best.map(|(w, c, _)| (w, c))
}

fn wav_path(
    chart: &dtx_core::Chart,
    wav_slot: u32,
    source_dir: Option<&std::path::Path>,
) -> Option<String> {
    if wav_slot == 0 {
        return None;
    }
    let filename = chart.assets.wav.get(wav_slot)?;
    Some(match source_dir {
        Some(dir) => dir.join(filename).to_string_lossy().to_string(),
        None => filename.to_string(),
    })
}

fn maybe_mute_hh_on_close(
    channel: EChannel,
    _audio: &Audio,
    instances: &mut Assets<AudioInstance>,
    active: &mut ActiveDrumSounds,
) {
    if !should_mute_tracked_hh(channel) {
        return;
    }
    for handle in active.hh_open_instances.drain(..) {
        if let Some(mut inst) = instances.get_mut(&handle) {
            inst.stop(AudioTween::default());
        }
    }
}

const fn should_mute_tracked_hh(channel: EChannel) -> bool {
    matches!(channel, EChannel::HiHatClose | EChannel::LeftPedal)
}

fn track_hh_instance(
    channel: EChannel,
    handle: Handle<AudioInstance>,
    active: &mut ActiveDrumSounds,
) {
    if matches!(channel, EChannel::HiHatOpen) {
        active.hh_open_instances.push(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane_map::{LaneId, LANE_COUNT, LANE_ORDER};

    #[test]
    fn chip_wav_slot_used_directly() {
        let chip = dtx_core::Chip::with_wav(0, dtx_core::EChannel::BassDrum, 0.5, 3);
        assert_eq!(chip.wav_slot, 3);
    }

    #[test]
    fn empty_hit_templates_reset() {
        let mut t = CurrentEmptyHitTemplates::default();
        t.set(
            0,
            dtx_core::EmptyHitEvent {
                lane: 0,
                measure: 0,
                value: 0.0,
                wav_slot: 1,
            },
        );
        t.reset();
        assert!(t.get(0).is_none());
    }

    #[test]
    fn lp_and_closed_hh_mute_tracked_open_hh() {
        assert!(should_mute_tracked_hh(EChannel::HiHatClose));
        assert!(should_mute_tracked_hh(EChannel::LeftPedal));
        assert!(!should_mute_tracked_hh(EChannel::HiHatOpen));
        assert!(!should_mute_tracked_hh(EChannel::Snare));
    }

    #[test]
    fn lane_order_has_twelve_lanes() {
        let _: LaneId = 0;
        assert_eq!(LANE_COUNT, 12);
        assert_eq!(LANE_ORDER.len(), 12);
    }
}
