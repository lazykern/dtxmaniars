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

/// How a chart audio reference resolves before decoder submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioResolution {
    Native(PathBuf),
    Substituted(PathBuf),
    Missing,
    Unsupported,
}

/// Whether losing an audio slot makes the chart unplayable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioRequirement {
    RequiredBgm,
    Optional,
}

/// One chart audio slot rejected during preloading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreloadIssue {
    pub slot: u32,
    pub path: PathBuf,
    pub kind: PreloadIssueKind,
    pub requirement: AudioRequirement,
    pub guidance: String,
}

/// A legacy XA reference recovered through a supported same-stem file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSubstitution {
    pub slot: u32,
    pub requested: PathBuf,
    pub resolved: PathBuf,
}

/// Usage-sensitive audio diagnostics for one chart.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChartAudioReport {
    pub substitutions: Vec<AudioSubstitution>,
    pub warnings: Vec<PreloadIssue>,
    pub required_failures: Vec<PreloadIssue>,
}

/// A chart audio slot accepted by preflight and submitted to the asset server.
#[derive(Debug, Clone)]
pub struct PreloadedAudio {
    pub slot: u32,
    pub path: PathBuf,
    pub requirement: AudioRequirement,
    pub handle: Handle<KiraAudioSource>,
}

/// Complete outcome of chart-audio preloading.
#[derive(Debug, Default)]
pub struct PreloadBatch {
    pub assets: Vec<PreloadedAudio>,
    pub report: ChartAudioReport,
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
#[cfg(test)]
fn preflight_chart_audio(
    source_dir: Option<&Path>,
    slot: u32,
    filename: &str,
) -> Result<PathBuf, PreloadIssue> {
    let path = requested_audio_path(source_dir, filename);
    match source_dir.map_or_else(
        || resolve_audio_path_without_chart_dir(filename),
        |dir| resolve_chart_audio(dir, filename),
    ) {
        AudioResolution::Native(path) | AudioResolution::Substituted(path) => Ok(path),
        AudioResolution::Missing => Err(audio_issue(
            slot,
            path,
            PreloadIssueKind::Missing,
            AudioRequirement::Optional,
        )),
        AudioResolution::Unsupported => Err(audio_issue(
            slot,
            path,
            PreloadIssueKind::Unsupported,
            AudioRequirement::Optional,
        )),
    }
}

fn requested_audio_path(source_dir: Option<&Path>, filename: &str) -> PathBuf {
    source_dir.map_or_else(
        || PathBuf::from(filename.replace('\\', "/")),
        |dir| {
            dtx_core::resolve_chart_asset_path(dir, filename)
                .unwrap_or_else(|| dir.join(filename.replace('\\', "/")))
        },
    )
}

fn resolve_audio_path_without_chart_dir(filename: &str) -> AudioResolution {
    let path = PathBuf::from(filename.replace('\\', "/"));
    if !path.is_file() {
        AudioResolution::Missing
    } else if dtx_audio::supported_audio_format(&path).is_some() {
        AudioResolution::Native(path)
    } else {
        AudioResolution::Unsupported
    }
}

/// Resolve supported chart audio. Legacy XA references recover through a
/// same-directory, same-stem OGG/WAV/MP3 file in that priority order.
pub fn resolve_chart_audio(chart_dir: &Path, filename: &str) -> AudioResolution {
    let requested = dtx_core::resolve_chart_asset_path(chart_dir, filename)
        .unwrap_or_else(|| chart_dir.join(filename.replace('\\', "/")));
    if requested.is_file() && dtx_audio::supported_audio_format(&requested).is_some() {
        return AudioResolution::Native(requested);
    }

    let is_xa = Path::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xa"));
    if !is_xa {
        return if requested.is_file() {
            AudioResolution::Unsupported
        } else {
            AudioResolution::Missing
        };
    }

    let normalized = filename.replace('\\', "/");
    let requested_relative = Path::new(&normalized);
    let parent = requested_relative.parent().unwrap_or_else(|| Path::new(""));
    let Some(stem) = requested_relative
        .file_stem()
        .and_then(|stem| stem.to_str())
    else {
        return AudioResolution::Unsupported;
    };
    for extension in ["ogg", "wav", "mp3"] {
        let fallback = parent.join(format!("{stem}.{extension}"));
        if let Some(resolved) =
            dtx_core::resolve_chart_asset_path(chart_dir, &fallback.to_string_lossy())
        {
            return AudioResolution::Substituted(resolved);
        }
    }
    if requested.is_file() {
        AudioResolution::Unsupported
    } else {
        AudioResolution::Missing
    }
}

fn audio_requirement(chart: &Chart, slot: u32) -> AudioRequirement {
    let used_by_bgm_chip = chart
        .chips
        .iter()
        .any(|chip| classify(chip.channel) == ChipClass::BGM && chip.wav_slot == slot);
    if used_by_bgm_chip || chart.metadata.bgm_wav_slots.contains(&slot) {
        AudioRequirement::RequiredBgm
    } else {
        AudioRequirement::Optional
    }
}

fn audio_issue(
    slot: u32,
    path: PathBuf,
    kind: PreloadIssueKind,
    requirement: AudioRequirement,
) -> PreloadIssue {
    PreloadIssue {
        slot,
        path,
        kind,
        requirement,
        guidance:
            "provide a same-stem OGG, WAV, or MP3 file; XA conversion is not run by DTXManiaRS"
                .into(),
    }
}

/// Inspect every requested slot without touching Bevy's asset server.
pub fn preflight_chart_audio_report(
    chart: &Chart,
    source_dir: Option<&Path>,
    slots: &BTreeSet<u32>,
) -> ChartAudioReport {
    let mut report = ChartAudioReport::default();
    for &slot in slots {
        let Some(filename) = chart.assets.wav.get(slot) else {
            continue;
        };
        let requirement = audio_requirement(chart, slot);
        let requested = requested_audio_path(source_dir, filename);
        let resolution = source_dir.map_or_else(
            || resolve_audio_path_without_chart_dir(filename),
            |dir| resolve_chart_audio(dir, filename),
        );
        match resolution {
            AudioResolution::Native(_) => {}
            AudioResolution::Substituted(resolved) => {
                report.substitutions.push(AudioSubstitution {
                    slot,
                    requested,
                    resolved,
                });
            }
            AudioResolution::Missing | AudioResolution::Unsupported => {
                let kind = if matches!(resolution, AudioResolution::Missing) {
                    PreloadIssueKind::Missing
                } else {
                    PreloadIssueKind::Unsupported
                };
                let issue = audio_issue(slot, requested, kind, requirement);
                let is_xa = Path::new(filename)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("xa"));
                if is_xa && requirement == AudioRequirement::RequiredBgm {
                    report.required_failures.push(issue);
                } else {
                    report.warnings.push(issue);
                }
            }
        }
    }
    report
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
    let report = preflight_chart_audio_report(&chart.chart, source_dir, slots);
    let mut batch = PreloadBatch {
        report,
        ..default()
    };
    for &slot in slots {
        let Some(filename) = chart.chart.assets.wav.get(slot) else {
            continue;
        };
        let resolution = source_dir.map_or_else(
            || resolve_audio_path_without_chart_dir(filename),
            |dir| resolve_chart_audio(dir, filename),
        );
        let path = match resolution {
            AudioResolution::Native(path) | AudioResolution::Substituted(path) => path,
            AudioResolution::Missing | AudioResolution::Unsupported => continue,
        };
        let handle = dtx_audio::preload_chart_sound(
            asset_server,
            bank,
            None,
            slot,
            &path.to_string_lossy(),
            chart.chart.assets.wav.volume(slot),
            chart.chart.assets.wav.pan(slot),
        );
        batch.assets.push(PreloadedAudio {
            slot,
            path,
            requirement: audio_requirement(&chart.chart, slot),
            handle,
        });
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
    for issue in batch
        .report
        .warnings
        .iter()
        .chain(&batch.report.required_failures)
    {
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
    use super::{
        collect_preload_wav_slots, preflight_chart_audio, AudioRequirement, PreloadIssue,
        PreloadIssueKind,
    };
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
                requirement: AudioRequirement::Optional,
                guidance: "provide a same-stem OGG, WAV, or MP3 file; XA conversion is not run by DTXManiaRS".into(),
            })
        );
        assert_eq!(
            preflight_chart_audio(Some(&dir), 3, "legacy.xa"),
            Err(PreloadIssue {
                slot: 3,
                path: dir.join("legacy.xa"),
                kind: PreloadIssueKind::Unsupported,
                requirement: AudioRequirement::Optional,
                guidance: "provide a same-stem OGG, WAV, or MP3 file; XA conversion is not run by DTXManiaRS".into(),
            })
        );
        assert_eq!(
            preflight_chart_audio(Some(&dir), 4, "kit\\snare.wav"),
            Ok(wav)
        );

        std::fs::remove_dir_all(dir).expect("remove fixture dir");
    }
}
