//! Bar/beat timing line expansion (BocuD `CDTX.cs:3525-3649`).
//!
//! Auto-inserts measure (`#50`) and beat (`#51`) lines from chart tempo data.
//! Rendering is gameplay-layer; this module only computes positions.

use crate::channel::EChannel;
use crate::chart::{Chart, Chip};

/// NX uses 384 ticks per measure (`CChip.nPlaybackPosition` units).
pub const TICKS_PER_MEASURE: i32 = 384;

/// Marker value on auto-inserted lines (`36 * 36 - 1` in BocuD).
pub const AUTO_LINE_MARKER: i32 = 36 * 36 - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingLineKind {
    /// Measure boundary (`EChannel.BarLine` / `#50`).
    Bar,
    /// Beat subdivision (`EChannel.BeatLine` / `#51`).
    Beat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingLine {
    pub tick: i32,
    pub kind: TimingLineKind,
    pub visible: bool,
}

/// Convert a chip to NX playback-position ticks.
pub fn chip_to_tick(chip: &Chip) -> i32 {
    let frac_ticks = (chip.value * TICKS_PER_MEASURE as f32).round() as i32;
    let clamped = frac_ticks.clamp(0, TICKS_PER_MEASURE - 1);
    chip.measure as i32 * TICKS_PER_MEASURE + clamped
}

impl TimingLine {
    /// Zero-based measure index for bar lines (`n小節番号` in BocuD).
    pub fn measure_number(self) -> Option<u32> {
        match self.kind {
            TimingLineKind::Bar => Some((self.tick / TICKS_PER_MEASURE) as u32),
            TimingLineKind::Beat => None,
        }
    }
}

/// Expand bar/beat timing lines for `chart` (BocuD post-parse insertion).
pub fn expand_timing_lines(chart: &Chart) -> Vec<TimingLine> {
    if chart.chips.is_empty() {
        return Vec::new();
    }

    let max_tick = chart.chips.iter().map(chip_to_tick).max().unwrap_or(0);
    let end_of_song = max_tick + TICKS_PER_MEASURE - (max_tick % TICKS_PER_MEASURE);

    let mut bar_length_changes: Vec<(i32, f64)> = Vec::new();
    let mut beat_shift_chips: Vec<(i32, i32)> = Vec::new();
    let mut display_events: Vec<(i32, bool)> = Vec::new();

    for chip in &chart.chips {
        match chip.channel {
            EChannel::BarLength => {
                let tick = chip.measure as i32 * TICKS_PER_MEASURE;
                bar_length_changes.push((tick, chip.value as f64));
            }
            EChannel::BeatLineShift => {
                let tick = chip_to_tick(chip);
                beat_shift_chips.push((tick, tick % TICKS_PER_MEASURE));
            }
            EChannel::BeatLineDisplay => {
                let visible = match chip.value as i32 {
                    1 => true,
                    2 => false,
                    _ => continue,
                };
                // C2 value is on/off, not fractional position (BocuD nIntegerValue).
                display_events.push((chip.measure as i32 * TICKS_PER_MEASURE, visible));
            }
            _ => {}
        }
    }

    beat_shift_chips.sort_by_key(|(t, _)| *t);
    display_events.sort_by_key(|(t, _)| *t);

    let mut raw: Vec<(i32, TimingLineKind)> = Vec::new();

    let mut tick384 = 0;
    while tick384 <= end_of_song {
        raw.push((tick384, TimingLineKind::Bar));
        tick384 += TICKS_PER_MEASURE;
    }

    let mut bar_length_idx = 0;
    let mut c1_idx = 0;
    let mut bar_length = 1.0_f64;

    tick384 = 0;
    while tick384 < end_of_song {
        let mut shift_in_measure = 0;
        while c1_idx < beat_shift_chips.len() && beat_shift_chips[c1_idx].0 < tick384 + TICKS_PER_MEASURE
        {
            if beat_shift_chips[c1_idx].0 >= tick384 {
                shift_in_measure = beat_shift_chips[c1_idx].1;
            }
            c1_idx += 1;
        }

        while bar_length_idx < bar_length_changes.len()
            && bar_length_changes[bar_length_idx].0 <= tick384
        {
            bar_length = bar_length_changes[bar_length_idx].1;
            bar_length_idx += 1;
        }

        for i in 0..100 {
            let tick_beat = (384.0 * i as f64 / (4.0 * bar_length)) as i32;
            if tick_beat + shift_in_measure >= TICKS_PER_MEASURE {
                break;
            }
            if (tick_beat + shift_in_measure) % TICKS_PER_MEASURE == 0 {
                continue;
            }
            raw.push((
                tick384 + tick_beat + shift_in_measure,
                TimingLineKind::Beat,
            ));
        }

        tick384 += TICKS_PER_MEASURE;
    }

    raw.sort_by_key(|(t, _)| *t);
    let mut lines: Vec<TimingLine> = raw
        .into_iter()
        .map(|(tick, kind)| TimingLine {
            tick,
            kind,
            visible: true,
        })
        .collect();

    apply_display_visibility(&mut lines, &display_events);
    lines
}

/// Apply `#C2` BeatLineDisplay toggles (BocuD `CDTX.cs:3608-3649`).
///
/// C2 chips update a running flag; each line inherits the flag active when
/// the sorted timeline reaches that tick (C2 events sort before lines at same tick).
fn apply_display_visibility(lines: &mut [TimingLine], display_events: &[(i32, bool)]) {
    if display_events.is_empty() {
        return;
    }

    let mut events: Vec<(i32, bool, u8)> = display_events
        .iter()
        .map(|&(t, v)| (t, v, 0u8))
        .collect();
    for line in lines.iter() {
        events.push((line.tick, false, 1));
    }
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));

    let mut show = true;
    let mut visibility = std::collections::HashMap::new();
    for (tick, value, kind) in events {
        if kind == 0 {
            show = value;
        } else {
            visibility.insert(tick, show);
        }
    }

    for line in lines.iter_mut() {
        if let Some(&vis) = visibility.get(&line.tick) {
            line.visible = vis;
        }
    }
}

/// Map NX ticks to measure + fraction for [`dtx_timing::math::chip_time_ms`].
pub fn tick_to_measure_fraction(tick: i32) -> (u32, f32) {
    let measure = tick.div_euclid(TICKS_PER_MEASURE) as u32;
    let frac_tick = tick.rem_euclid(TICKS_PER_MEASURE);
    let fraction = frac_tick as f32 / TICKS_PER_MEASURE as f32;
    (measure, fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chart_yields_no_lines() {
        assert!(expand_timing_lines(&Chart::default()).is_empty());
    }

    #[test]
    fn inserts_bar_lines_every_measure() {
        let chart = Chart {
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 0.0),
                Chip::new(2, EChannel::BassDrum, 0.0),
            ],
            ..Default::default()
        };
        let lines = expand_timing_lines(&chart);
        let bars: Vec<i32> = lines
            .iter()
            .filter(|l| l.kind == TimingLineKind::Bar)
            .map(|l| l.tick)
            .collect();
        assert_eq!(bars, vec![0, 384, 768, 1152]);
    }

    #[test]
    fn default_bar_length_inserts_three_beats_per_measure() {
        let chart = Chart {
            chips: vec![Chip::new(0, EChannel::BassDrum, 0.0)],
            ..Default::default()
        };
        let lines = expand_timing_lines(&chart);
        let beats: Vec<i32> = lines
            .iter()
            .filter(|l| l.kind == TimingLineKind::Beat)
            .map(|l| l.tick)
            .collect();
        assert_eq!(beats, vec![96, 192, 288]);
    }

    #[test]
    fn double_bar_length_quarters_beat_spacing() {
        let chart = Chart {
            chips: vec![
                Chip::new(0, EChannel::BarLength, 2.0),
                Chip::new(0, EChannel::BassDrum, 0.0),
            ],
            ..Default::default()
        };
        let lines = expand_timing_lines(&chart);
        let beats: Vec<i32> = lines
            .iter()
            .filter(|l| l.kind == TimingLineKind::Beat && l.tick < 384)
            .map(|l| l.tick)
            .collect();
        assert_eq!(beats.len(), 7);
        assert_eq!(beats[0], 48);
    }

    #[test]
    fn c2_off_hides_lines_at_or_after_toggle() {
        let chart = Chart {
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 0.0),
                Chip::new(1, EChannel::BeatLineDisplay, 2.0),
            ],
            ..Default::default()
        };
        let lines = expand_timing_lines(&chart);
        let bar_at_0 = lines
            .iter()
            .find(|l| l.kind == TimingLineKind::Bar && l.tick == 0)
            .expect("bar at 0");
        assert!(bar_at_0.visible);
        let bar_at_384 = lines
            .iter()
            .find(|l| l.kind == TimingLineKind::Bar && l.tick == 384)
            .expect("bar at 384");
        assert!(!bar_at_384.visible);
    }

    #[test]
    fn measure_number_on_bar_only() {
        let bar = TimingLine {
            tick: 768,
            kind: TimingLineKind::Bar,
            visible: true,
        };
        let beat = TimingLine {
            tick: 96,
            kind: TimingLineKind::Beat,
            visible: true,
        };
        assert_eq!(bar.measure_number(), Some(2));
        assert_eq!(beat.measure_number(), None);
    }
}
