use std::collections::BTreeSet;

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioSource as KiraAudioSource;
use dtx_core::{Chart, EChannel};

use crate::lane_map::lane_of;
use crate::resources::ActiveChart;

/// Collect every chart WAV slot that can be needed during performance.
pub fn collect_preload_wav_slots(chart: &Chart) -> BTreeSet<u32> {
    let mut slots = collect_immediate_wav_slots(chart);
    slots.extend(collect_deferred_wav_slots(chart));
    slots
}

/// Tier 1 (immediate): WAV slots that can be triggered the instant gameplay
/// starts — playable drum-lane note chips and their empty-hit templates. The
/// loading screen waits on these so the first hits never miss their sound.
pub fn collect_immediate_wav_slots(chart: &Chart) -> BTreeSet<u32> {
    chart
        .chips
        .iter()
        .filter(|chip| lane_of(chip.channel).is_some())
        .filter_map(|chip| (chip.wav_slot != 0).then_some(chip.wav_slot))
        .chain(
            chart
                .empty_hit_events
                .iter()
                .filter_map(|event| (event.wav_slot != 0).then_some(event.wav_slot)),
        )
        .collect()
}

/// Tier 2 (deferred): WAV slots only referenced by non-lane channels (BGM
/// stems, auto-SE). These are scheduled later in the song, so their decode can
/// finish in the background after gameplay begins — the loader does not block
/// on them.
pub fn collect_deferred_wav_slots(chart: &Chart) -> BTreeSet<u32> {
    let immediate = collect_immediate_wav_slots(chart);
    chart
        .chips
        .iter()
        .filter(|chip| lane_of(chip.channel).is_none())
        .filter_map(|chip| (chip.wav_slot != 0).then_some(chip.wav_slot))
        .filter(|slot| !immediate.contains(slot))
        .collect()
}

/// Preload a specific set of WAV slots into the shared audio bank, returning
/// the handles requested (for load-state polling).
pub fn preload_slots(
    chart: &ActiveChart,
    asset_server: &AssetServer,
    bank: &mut dtx_audio::ChartSoundBank,
    slots: &BTreeSet<u32>,
) -> Vec<Handle<KiraAudioSource>> {
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    let mut handles = Vec::new();
    for &slot in slots {
        let Some(filename) = chart.chart.assets.wav.get(slot) else {
            continue;
        };
        let handle = dtx_audio::preload_chart_sound(
            asset_server,
            bank,
            source_dir,
            slot,
            filename,
            chart.chart.assets.wav.volume(slot),
            chart.chart.assets.wav.pan(slot),
        );
        handles.push(handle);
    }
    handles
}

/// Preload every chart WAV handle into the shared audio bank (both tiers).
pub fn preload_chart_sounds(
    chart: &ActiveChart,
    asset_server: &AssetServer,
    bank: &mut dtx_audio::ChartSoundBank,
) -> usize {
    let slots = collect_preload_wav_slots(&chart.chart);
    preload_slots(chart, asset_server, bank, &slots).len()
}

/// System wrapper for performance entry.
pub fn preload_chart_sounds_on_enter(
    chart: Res<ActiveChart>,
    asset_server: Res<AssetServer>,
    mut bank: ResMut<dtx_audio::ChartSoundBank>,
) {
    let loaded = preload_chart_sounds(&chart, &asset_server, &mut bank);
    info!("Performance: preloaded {loaded} chart WAV slots");
}

/// True for chart-timed auto-SE channels.
pub const fn is_auto_se_channel(ch: EChannel) -> bool {
    matches!(
        ch,
        EChannel::SE01 | EChannel::SE02 | EChannel::SE03 | EChannel::SE04 | EChannel::SE05
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::collect_preload_wav_slots;
    use dtx_core::{Chart, Chip, EChannel, EmptyHitEvent};

    #[test]
    fn immediate_tier_is_lane_notes_and_empty_hits_only() {
        use super::{collect_deferred_wav_slots, collect_immediate_wav_slots};
        let chart = Chart {
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),      // deferred
                Chip::with_wav(0, EChannel::Snare, 0.5, 2),    // immediate (lane)
                Chip::with_wav(0, EChannel::SE01, 0.75, 3),    // deferred (non-lane)
                Chip::with_wav(0, EChannel::BassDrum, 0.25, 4), // immediate (lane)
            ],
            empty_hit_events: vec![EmptyHitEvent {
                lane: 0,
                measure: 0,
                value: 0.0,
                wav_slot: 5,
            }],
            ..Default::default()
        };

        let immediate = collect_immediate_wav_slots(&chart);
        let deferred = collect_deferred_wav_slots(&chart);

        assert_eq!(immediate, [2, 4, 5].into_iter().collect::<BTreeSet<_>>());
        assert_eq!(deferred, [1, 3].into_iter().collect::<BTreeSet<_>>());
        // Tiers must be disjoint.
        assert!(immediate.is_disjoint(&deferred));
    }

    #[test]
    fn collect_preload_wav_slots_includes_bgm_se_drums_and_empty_hits() {
        let chart = Chart {
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),
                Chip::with_wav(0, EChannel::Snare, 0.5, 2),
                Chip::with_wav(0, EChannel::SE01, 0.75, 3),
                Chip::with_wav(0, EChannel::BassDrum, 0.25, 0),
            ],
            empty_hit_events: vec![EmptyHitEvent {
                lane: 0,
                measure: 0,
                value: 0.0,
                wav_slot: 4,
            }],
            ..Default::default()
        };

        let slots = collect_preload_wav_slots(&chart);

        assert_eq!(slots, [1, 2, 3, 4].into_iter().collect::<BTreeSet<_>>());
    }
}
