use std::collections::BTreeSet;

use bevy::prelude::*;
use dtx_core::{Chart, EChannel};

use crate::resources::ActiveChart;

/// Collect every chart WAV slot that can be needed during performance.
pub fn collect_preload_wav_slots(chart: &Chart) -> BTreeSet<u32> {
    chart
        .chips
        .iter()
        .filter_map(|chip| (chip.wav_slot != 0).then_some(chip.wav_slot))
        .chain(
            chart
                .empty_hit_events
                .iter()
                .filter_map(|event| (event.wav_slot != 0).then_some(event.wav_slot)),
        )
        .collect()
}

/// Preload chart WAV handles into the shared audio bank.
pub fn preload_chart_sounds(
    chart: &ActiveChart,
    asset_server: &AssetServer,
    bank: &mut dtx_audio::ChartSoundBank,
) -> usize {
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    let slots = collect_preload_wav_slots(&chart.chart);
    let mut loaded = 0;
    for slot in slots {
        let Some(filename) = chart.chart.assets.wav.get(slot) else {
            continue;
        };
        dtx_audio::preload_chart_sound(
            asset_server,
            bank,
            source_dir,
            slot,
            filename,
            chart.chart.assets.wav.volume(slot),
            chart.chart.assets.wav.pan(slot),
        );
        loaded += 1;
    }
    loaded
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
