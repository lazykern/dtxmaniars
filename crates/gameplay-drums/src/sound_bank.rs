use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioSource as KiraAudioSource;
use dtx_core::{
    chip_classify::{classify, ChipClass},
    Chart, EChannel,
};

use crate::lane_map::lane_of;
use crate::resources::ActiveChart;

/// Why a chart audio slot was not submitted to Bevy's decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadIssueKind {
    Missing,
    Unsupported,
}

/// One chart audio slot rejected during preloading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadIssue {
    pub slot: u32,
    pub path: PathBuf,
    pub kind: PreloadIssueKind,
}

/// A chart audio slot accepted by preflight and submitted to the asset server.
#[derive(Debug, Clone)]
pub struct PreloadedAudio {
    pub slot: u32,
    pub path: PathBuf,
    pub handle: Handle<KiraAudioSource>,
}

/// Complete outcome of chart-audio preloading.
#[derive(Debug, Default)]
pub struct PreloadBatch {
    pub assets: Vec<PreloadedAudio>,
    pub issues: Vec<PreloadIssue>,
}

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

/// Tier 2: WAV slots referenced by audio-bearing non-lane channels (BGM,
/// auto-SE, guitar). Kept separate for preload priority; SongLoading waits for
/// both tiers before gameplay starts.
pub fn collect_deferred_wav_slots(chart: &Chart) -> BTreeSet<u32> {
    let immediate = collect_immediate_wav_slots(chart);
    chart
        .chips
        .iter()
        .filter(|chip| lane_of(chip.channel).is_none())
        .filter(|chip| channel_uses_wav(chip.channel))
        .filter_map(|chip| (chip.wav_slot != 0).then_some(chip.wav_slot))
        .filter(|slot| !immediate.contains(slot))
        .collect()
}

const fn channel_uses_wav(channel: EChannel) -> bool {
    matches!(
        classify(channel),
        ChipClass::Drum
            | ChipClass::OpenNote
            | ChipClass::Guitar
            | ChipClass::Bass
            | ChipClass::LongNote
            | ChipClass::Wailing
            | ChipClass::BGM
            | ChipClass::SE
            | ChipClass::Click
            | ChipClass::Mixer
    )
}

/// Classify a chart-relative audio reference before submitting it to Bevy.
fn preflight_chart_audio(
    source_dir: Option<&Path>,
    slot: u32,
    filename: &str,
) -> Result<PathBuf, PreloadIssue> {
    let path = source_dir.map_or_else(
        || PathBuf::from(filename),
        |dir| {
            dtx_core::resolve_chart_asset_path(dir, filename)
                .unwrap_or_else(|| dir.join(filename.replace('\\', "/")))
        },
    );
    if !path.is_file() {
        return Err(PreloadIssue {
            slot,
            path,
            kind: PreloadIssueKind::Missing,
        });
    }
    if dtx_audio::supported_audio_format(&path).is_none() {
        return Err(PreloadIssue {
            slot,
            path,
            kind: PreloadIssueKind::Unsupported,
        });
    }
    Ok(path)
}

/// Preload a specific set of WAV slots into the shared audio bank, retaining
/// asset identity for load diagnostics.
pub fn preload_slots_report(
    chart: &ActiveChart,
    asset_server: &AssetServer,
    bank: &mut dtx_audio::ChartSoundBank,
    slots: &BTreeSet<u32>,
) -> PreloadBatch {
    let source_dir = chart.source_path.as_ref().and_then(|p| p.parent());
    let mut batch = PreloadBatch::default();
    for &slot in slots {
        let Some(filename) = chart.chart.assets.wav.get(slot) else {
            continue;
        };
        let path = match preflight_chart_audio(source_dir, slot, filename) {
            Ok(path) => path,
            Err(issue) => {
                batch.issues.push(issue);
                continue;
            }
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
        batch.assets.push(PreloadedAudio { slot, path, handle });
    }
    batch
}

/// Compatibility wrapper for callers that only need handles for polling.
pub fn preload_slots(
    chart: &ActiveChart,
    asset_server: &AssetServer,
    bank: &mut dtx_audio::ChartSoundBank,
    slots: &BTreeSet<u32>,
) -> Vec<Handle<KiraAudioSource>> {
    preload_slots_report(chart, asset_server, bank, slots)
        .assets
        .into_iter()
        .map(|asset| asset.handle)
        .collect()
}

/// Preload every chart WAV handle into the shared audio bank (both tiers).
pub fn preload_chart_sounds(
    chart: &ActiveChart,
    asset_server: &AssetServer,
    bank: &mut dtx_audio::ChartSoundBank,
) -> usize {
    let slots = collect_preload_wav_slots(&chart.chart);
    let batch = preload_slots_report(chart, asset_server, bank, &slots);
    for issue in &batch.issues {
        warn!(
            "Performance: {:?} chart audio slot {} at {}",
            issue.kind,
            issue.slot,
            issue.path.display()
        );
    }
    batch.assets.len()
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
    ch.is_se() || ch.is_timed_system_sound()
}

#[cfg(test)]
mod tests {
    use super::{collect_preload_wav_slots, preflight_chart_audio, PreloadIssue, PreloadIssueKind};
    use dtx_core::{Chart, Chip, EChannel, EmptyHitEvent};
    use std::collections::BTreeSet;

    #[test]
    fn immediate_tier_is_lane_notes_and_empty_hits_only() {
        use super::{collect_deferred_wav_slots, collect_immediate_wav_slots};
        let chart = Chart {
            chips: vec![
                Chip::with_wav(0, EChannel::BGM, 0.0, 1),       // deferred
                Chip::with_wav(0, EChannel::Snare, 0.5, 2),     // immediate (lane)
                Chip::with_wav(0, EChannel::SE01, 0.75, 3),     // deferred (non-lane)
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
                Chip::with_wav(0, EChannel::SE32, 0.75, 3),
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

    #[test]
    fn collect_preload_wav_slots_ignores_bpm_ids() {
        let chart = Chart {
            chips: vec![
                Chip::with_wav(0, EChannel::BPMEx, 0.0, 1),
                Chip::with_wav(0, EChannel::BGM, 0.0, 2),
            ],
            ..Default::default()
        };

        let slots = collect_preload_wav_slots(&chart);

        assert_eq!(slots, [2].into_iter().collect::<BTreeSet<_>>());
    }

    #[test]
    fn preflight_classifies_supported_missing_and_unsupported_audio() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-preflight-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let nested = dir.join("Kit");
        std::fs::create_dir_all(&nested).expect("create fixture dir");
        std::fs::write(dir.join("tone.mp3"), b"not decoded by preflight")
            .expect("write mp3 fixture");
        std::fs::write(dir.join("legacy.xa"), b"unsupported").expect("write xa fixture");
        let wav = nested.join("Snare.WAV");
        std::fs::write(&wav, b"not decoded by preflight").expect("write wav fixture");

        assert_eq!(
            preflight_chart_audio(Some(&dir), 1, "tone.mp3"),
            Ok(dir.join("tone.mp3"))
        );
        assert_eq!(
            preflight_chart_audio(Some(&dir), 2, "missing.ogg"),
            Err(PreloadIssue {
                slot: 2,
                path: dir.join("missing.ogg"),
                kind: PreloadIssueKind::Missing,
            })
        );
        assert_eq!(
            preflight_chart_audio(Some(&dir), 3, "legacy.xa"),
            Err(PreloadIssue {
                slot: 3,
                path: dir.join("legacy.xa"),
                kind: PreloadIssueKind::Unsupported,
            })
        );
        assert_eq!(
            preflight_chart_audio(Some(&dir), 4, "kit\\snare.wav"),
            Ok(wav)
        );

        std::fs::remove_dir_all(dir).expect("remove fixture dir");
    }
}
