//! Count-in metronome: click schedule computed at seek time, fired as
//! the clock crosses each beat, plus the quick-tier countdown number.
//! Spec: docs/superpowers/specs/2026-07-11-practice-count-in-metronome-design.md

use bevy::prelude::*;
use bevy_kira_audio::prelude::{Frame, StaticSoundData, StaticSoundSettings};
use bevy_kira_audio::AudioSource as KiraAudioSource;

use super::session::{preroll_target, PrerollSetting};
use crate::timeline::ChipTimeline;

/// One scheduled count-in click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    /// Chart time the click fires at (a `ChipTimeline.beat_ms` line).
    pub at_ms: i64,
    /// First click of the schedule is accented.
    pub accent: bool,
    /// Number shown by the countdown UI ("4 3 2 1"): clicks left
    /// including this one.
    pub beats_remaining: u8,
}

/// Click times for one pre-roll window, sorted ascending.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClickSchedule {
    pub clicks: Vec<Click>,
}

/// Clicks on beat-grid lines in `[preroll_target, intent_ms)`. No click
/// at `intent_ms` itself — the music entry is the implicit "1".
pub fn build_preroll_schedule(
    timeline: &ChipTimeline,
    preroll: PrerollSetting,
    intent_ms: i64,
) -> ClickSchedule {
    let window_start = preroll_target(timeline, preroll, intent_ms);
    if window_start >= intent_ms {
        return ClickSchedule::default();
    }
    let beats: Vec<i64> = timeline
        .beat_ms
        .iter()
        .copied()
        .filter(|&ms| ms >= window_start && ms < intent_ms)
        .collect();
    let n = beats.len();
    ClickSchedule {
        clicks: beats
            .into_iter()
            .enumerate()
            .map(|(i, at_ms)| Click {
                at_ms,
                accent: i == 0,
                beats_remaining: (n - i) as u8,
            })
            .collect(),
    }
}

const CLICK_SAMPLE_RATE: u32 = 44_100;
const CLICK_LEN_S: f32 = 0.03;
const ACCENT_HZ: f32 = 2_000.0;
const TICK_HZ: f32 = 1_000.0;

/// ~30ms sine burst with exponential decay (pure; unit-tested).
pub fn synth_click_frames(freq_hz: f32, sample_rate: u32) -> Vec<Frame> {
    let n = (CLICK_LEN_S * sample_rate as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let env = (-t * 150.0).exp();
            Frame::from_mono((t * freq_hz * std::f32::consts::TAU).sin() * env * 0.8)
        })
        .collect()
}

fn click_source(freq_hz: f32) -> KiraAudioSource {
    KiraAudioSource {
        sound: StaticSoundData {
            sample_rate: CLICK_SAMPLE_RATE,
            frames: synth_click_frames(freq_hz, CLICK_SAMPLE_RATE).into(),
            settings: StaticSoundSettings::default(),
            slice: None,
        },
    }
}

/// Handles to the two synthesized click samples.
#[derive(Resource, Default)]
pub struct MetronomeSounds {
    pub accent: Handle<KiraAudioSource>,
    pub tick: Handle<KiraAudioSource>,
}

/// Build the click samples once per Performance enter (practice only —
/// the plugin gates on `PracticeSession`).
pub fn build_metronome_sounds(
    mut sounds: ResMut<MetronomeSounds>,
    mut sources: ResMut<Assets<KiraAudioSource>>,
) {
    sounds.accent = sources.add(click_source(ACCENT_HZ));
    sounds.tick = sources.add(click_source(TICK_HZ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms, beat = 500ms. Chart spans 4 bars.
    fn timeline() -> ChipTimeline {
        let chart = Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 0.0),
                Chip::new(3, EChannel::Snare, 0.75), // keeps 4 bars of lines
            ],
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 8_000)
    }

    #[test]
    fn one_bar_preroll_yields_four_clicks_counting_down() {
        let tl = timeline();
        // Intent at bar 2 start (4000ms): window = [2000, 4000).
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        let times: Vec<i64> = s.clicks.iter().map(|c| c.at_ms).collect();
        assert_eq!(times, vec![2_000, 2_500, 3_000, 3_500]);
        let remaining: Vec<u8> = s.clicks.iter().map(|c| c.beats_remaining).collect();
        assert_eq!(remaining, vec![4, 3, 2, 1]);
    }

    #[test]
    fn only_first_click_is_accented() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        assert!(s.clicks[0].accent);
        assert!(s.clicks[1..].iter().all(|c| !c.accent));
    }

    #[test]
    fn no_click_at_intent_itself() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        assert!(s.clicks.iter().all(|c| c.at_ms < 4_000));
    }

    #[test]
    fn seconds_preroll_takes_beats_inside_window() {
        let tl = timeline();
        // 1.2s window before 4000ms: [2800, 4000) → beats 3000, 3500.
        let s = build_preroll_schedule(&tl, PrerollSetting::Seconds(1.2), 4_000);
        let times: Vec<i64> = s.clicks.iter().map(|c| c.at_ms).collect();
        assert_eq!(times, vec![3_000, 3_500]);
        assert_eq!(s.clicks[0].beats_remaining, 2);
    }

    #[test]
    fn off_preroll_yields_empty_schedule() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::Off, 4_000);
        assert!(s.clicks.is_empty());
    }

    #[test]
    fn intent_at_chart_start_yields_empty_schedule() {
        let tl = timeline();
        // preroll_target clamps to 0 == intent → empty window.
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 0);
        assert!(s.clicks.is_empty());
    }

    #[test]
    fn synth_click_is_short_and_non_silent() {
        let frames = synth_click_frames(2_000.0, 44_100);
        // ~30ms at 44.1kHz ≈ 1323 frames.
        assert!((1_300..=1_350).contains(&frames.len()));
        assert!(frames.iter().any(|f| f.left.abs() > 0.05));
        // Exponential decay: tail quieter than head.
        assert!(frames[frames.len() - 1].left.abs() < frames[10].left.abs());
    }
}
