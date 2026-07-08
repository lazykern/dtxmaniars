//! Chart-independent hit feedback for the Customize surface: a `LaneHit` plays a
//! hit sound (lane flash already works via `keyboard_viz`). Active only while the
//! surface is open — normal play uses the judge→sound path.
//!
//! While the surface is open, `judge_lane_hit_system` is gated off (no scoring),
//! so the judgment→sound path is silent. This system provides audible feedback by
//! playing a representative drum voice for the hit lane straight from the chart's
//! preloaded sound bank. No judgment, no scoring — just a flash + sound so the
//! player can verify a freshly bound key/pad reaches the right lane.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

use crate::events::LaneHit;
use crate::lane_map::lane_channel;
use crate::resources::{ActiveChart, DrumAudioSettings};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        play_hit_voice_on_lane_hit
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(crate::editor::editor_open),
    );
}

fn play_hit_voice_on_lane_hit(
    mut hits: MessageReader<LaneHit>,
    settings: Res<DrumAudioSettings>,
    chart: Res<ActiveChart>,
    audio: Res<Audio>,
    asset_server: Res<AssetServer>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    sound_bank: Res<dtx_audio::ChartSoundBank>,
) {
    if !settings.enabled || chart.chart.assets.wav.is_empty() {
        // Nothing loaded to play — drain so events don't pile up.
        for _ in hits.read() {}
        return;
    }
    let source_dir = chart
        .source_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    for hit in hits.read() {
        // Per-lane kit voice: first chip on this lane's channel that carries a WAV.
        let Some(wav_slot) = representative_wav_slot(&chart, hit.lane) else {
            continue;
        };
        let vol = chart.chart.assets.wav.volume(wav_slot);
        let pan = chart.chart.assets.wav.pan(wav_slot);
        // Reuse hit_sound.rs's exact voice-playing mechanism: preloaded handle
        // when the bank has it, else stream from the resolved WAV path.
        if let Some(sound) = sound_bank.get(wav_slot) {
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
            );
        } else if let Some(path) = wav_path(&chart.chart, wav_slot, source_dir.as_deref()) {
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
            );
        }
    }
}

/// First chart chip on the lane's channel that carries a WAV slot, else any
/// loaded WAV so a bound key is always audible (generic fallback).
fn representative_wav_slot(chart: &ActiveChart, lane: u8) -> Option<u32> {
    if let Some(channel) = lane_channel(lane) {
        if let Some(chip) = chart
            .chart
            .chips
            .iter()
            .find(|c| c.channel == channel && c.wav_slot != 0)
        {
            return Some(chip.wav_slot);
        }
    }
    chart
        .chart
        .assets
        .wav
        .order
        .iter()
        .copied()
        .find(|&id| id != 0)
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
