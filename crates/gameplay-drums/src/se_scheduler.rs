//! Auto SE chip scheduler — plays chart-timed sound effects.
//!
//! Schedules SE01-SE32 chips when their target time is reached.
//! Reference: BocuD chip scroll loop + dtxpt `schedule_auto_se`.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use std::collections::HashSet;

use crate::judge::{auto_chip_target_ms, BarLengthChangeList, BpmChangeList};
use crate::resources::{
    ActiveChart, ActiveDrumSounds, BgmAdjustState, DrumAudioSettings, GameplayClock,
};
use dtx_core::EChannel;
use dtx_timing::math::ChartTiming;
use game_shell::{AppState, PauseState};

#[derive(Resource, Default, Debug)]
pub struct PlayedSeChips(pub HashSet<usize>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PlayedSeChips>().add_systems(
        FixedUpdate,
        schedule_se_chips
            .in_set(super::DrumsSets::NoteSpawn)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running)),
    );
}

pub fn reset_played_se(mut played: ResMut<PlayedSeChips>) {
    played.0.clear();
}

fn schedule_se_chips(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    bgm_adjust: Res<BgmAdjustState>,
    settings: Res<DrumAudioSettings>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
    mut active: ResMut<ActiveDrumSounds>,
    mut played: ResMut<PlayedSeChips>,
) {
    if !clock.is_ready() || !settings.drum_enabled {
        return;
    }
    if chart.chart.assets.wav.is_empty() {
        return;
    }
    let now = clock.current_ms;
    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    let bgm_shift = bgm_adjust.total_ms();
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());

    for (idx, chip) in chart.chart.chips.iter().enumerate() {
        if !chip.channel.is_se() && !chip.channel.is_timed_system_sound() {
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
        played.0.insert(idx);
        let Some(filename) = chart.chart.assets.wav.get(chip.wav_slot) else {
            continue;
        };
        let path = match source_dir {
            Some(dir) => dir.join(filename).to_string_lossy().to_string(),
            None => filename.to_string(),
        };
        let vol = chart.chart.assets.wav.volume(chip.wav_slot);
        let pan = chart.chart.assets.wav.pan(chip.wav_slot);
        let handle = if let Some(sound) = sound_bank.get(chip.wav_slot) {
            dtx_audio::play_drum_hit_handle(
                &audio,
                &mut instances,
                &mut polyphony,
                sound.handle.clone(),
                chip.wav_slot,
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
                chip.wav_slot,
                vol,
                pan,
                settings.master_volume,
                settings.drum_volume,
            )
        };
        if auto_se_replaces_previous(chip.channel) {
            if let Some(prev) = active.stick_se_instances.insert(chip.channel, handle) {
                if let Some(mut inst) = instances.get_mut(&prev) {
                    inst.stop(AudioTween::default());
                }
            }
        }
    }
}

const fn auto_se_replaces_previous(ch: EChannel) -> bool {
    ch.is_se()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn se_channel_detection() {
        assert!(EChannel::SE01.is_se());
        assert!(EChannel::SE32.is_se());
        assert!(!EChannel::BassDrum.is_se());
        assert!(!EChannel::BGM.is_se());
    }

    #[test]
    fn modeled_auto_se_channels_replace_previous_instance() {
        assert!(auto_se_replaces_previous(EChannel::SE01));
        assert!(auto_se_replaces_previous(EChannel::SE32));
        assert!(!auto_se_replaces_previous(EChannel::BGM));
    }
}
