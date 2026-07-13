//! Bounded normal-play timing telemetry and player-facing analysis.

use std::collections::BTreeMap;

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use game_shell::AppState;

use crate::events::{JudgmentEvent, NoteMissed};
use crate::lane_map::LaneId;
use crate::practice::PracticeSession;
use crate::timeline::ChipTimeline;
use crate::DrumsSets;

/// Maximum number of events retained for one normal play.
pub const MAX_NORMAL_PLAY_EVENTS: usize = 8_192;

/// One judged chart chip retained for post-play analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordedJudgment {
    pub lane: LaneId,
    pub kind: JudgmentKind,
    /// Negative is early and positive is late.
    pub delta_ms: i64,
    pub chip_idx: usize,
    pub chart_ms: i64,
}

impl RecordedJudgment {
    pub const fn new(
        lane: LaneId,
        kind: JudgmentKind,
        delta_ms: i64,
        chip_idx: usize,
        chart_ms: i64,
    ) -> Self {
        Self {
            lane,
            kind,
            delta_ms,
            chip_idx,
            chart_ms,
        }
    }
}

/// Ephemeral telemetry for the current normal play.
#[derive(Resource, Debug, Clone, Default)]
pub struct NormalPlayEventStream {
    pub events: Vec<RecordedJudgment>,
    pub truncated: bool,
}

impl NormalPlayEventStream {
    pub fn clear(&mut self) {
        self.events.clear();
        self.truncated = false;
    }

    pub fn push(&mut self, event: RecordedJudgment) {
        if self.events.len() < MAX_NORMAL_PLAY_EVENTS {
            self.events.push(event);
        } else {
            self.truncated = true;
        }
    }
}

/// Average error weight for one lane, lower means more consistent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneWeakness {
    pub lane: LaneId,
    pub average_weight: f32,
    pub event_count: u32,
}

/// Weakest evidence-bearing bar and the safe loop surrounding it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionWeakness {
    pub bar_start_ms: i64,
    pub bar_end_ms: i64,
    pub loop_start_ms: i64,
    pub loop_end_ms: i64,
    pub average_weight: f32,
    pub event_count: u32,
}

/// Derived data consumed by Results. Missing fields mean insufficient evidence.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PerformanceAnalysis {
    pub bias_ms: Option<i64>,
    pub spread_ms: Option<i64>,
    pub lane_weaknesses: Vec<LaneWeakness>,
    pub weakest_lane: Option<LaneWeakness>,
    pub weakest_section: Option<SectionWeakness>,
    pub truncated: bool,
}

impl PerformanceAnalysis {
    pub fn from_stream(stream: &NormalPlayEventStream, bar_ms: &[i64]) -> Self {
        let mut analysis = analyze_normal_play(&stream.events, bar_ms);
        analysis.truncated = stream.truncated;
        analysis
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<NormalPlayEventStream>()
        .add_systems(
            OnEnter(AppState::Performance),
            clear_normal_play_events.before(crate::orchestrator::DrumsEnterSet),
        )
        .add_systems(
            FixedUpdate,
            record_normal_play_events
                .after(DrumsSets::Score)
                .run_if(in_state(AppState::Performance)),
        );
}

fn clear_normal_play_events(mut stream: ResMut<NormalPlayEventStream>) {
    stream.clear();
}

fn record_normal_play_events(
    mut judgments: MessageReader<JudgmentEvent>,
    mut misses: MessageReader<NoteMissed>,
    timeline: Res<ChipTimeline>,
    practice: Option<Res<PracticeSession>>,
    mut stream: ResMut<NormalPlayEventStream>,
) {
    let normal_play = practice.is_none();
    for event in judgments.read() {
        if !normal_play {
            continue;
        }
        let Some(&chart_ms) = timeline.judge_ms_by_idx.get(event.chip_idx) else {
            continue;
        };
        stream.push(RecordedJudgment::new(
            event.lane,
            event.kind,
            event.delta_ms,
            event.chip_idx,
            chart_ms,
        ));
    }
    for event in misses.read() {
        if !normal_play {
            continue;
        }
        let Some(&chart_ms) = timeline.judge_ms_by_idx.get(event.chip_idx) else {
            continue;
        };
        stream.push(RecordedJudgment::new(
            event.lane,
            JudgmentKind::Miss,
            0,
            event.chip_idx,
            chart_ms,
        ));
    }
}

/// Derive normal-play timing/lane/section information without Bevy state.
pub fn analyze_normal_play(events: &[RecordedJudgment], bar_ms: &[i64]) -> PerformanceAnalysis {
    let hit_errors: Vec<i64> = events
        .iter()
        .filter(|event| event.kind != JudgmentKind::Miss)
        .map(|event| event.delta_ms)
        .collect();
    let bias_ms = median(&hit_errors);
    let spread_ms = bias_ms.map(|bias| {
        let deviations: Vec<i64> = hit_errors
            .iter()
            .map(|error| (error - bias).abs())
            .collect();
        median(&deviations).unwrap_or_default()
    });

    let lane_weaknesses = lane_weaknesses(events);
    let weakest_lane = lane_weaknesses.first().copied();

    PerformanceAnalysis {
        bias_ms,
        spread_ms,
        weakest_lane,
        weakest_section: weakest_section(events, bar_ms),
        lane_weaknesses,
        truncated: false,
    }
}

fn median(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() - 1) / 2])
}

fn error_weight(kind: JudgmentKind) -> f32 {
    match kind {
        JudgmentKind::Perfect => 0.0,
        JudgmentKind::Great => 0.2,
        JudgmentKind::Good => 0.4,
        JudgmentKind::Poor => 0.7,
        JudgmentKind::Miss => 1.0,
    }
}

fn lane_weaknesses(events: &[RecordedJudgment]) -> Vec<LaneWeakness> {
    let mut lanes = BTreeMap::<LaneId, (f32, u32)>::new();
    for event in events {
        let entry = lanes.entry(event.lane).or_default();
        entry.0 += error_weight(event.kind);
        entry.1 += 1;
    }
    let mut weaknesses: Vec<_> = lanes
        .into_iter()
        .filter_map(|(lane, (weight_sum, event_count))| {
            (event_count >= 3).then_some(LaneWeakness {
                lane,
                average_weight: weight_sum / event_count as f32,
                event_count,
            })
        })
        .collect();
    weaknesses.sort_by(|left, right| {
        right
            .average_weight
            .total_cmp(&left.average_weight)
            .then(left.lane.cmp(&right.lane))
    });
    weaknesses
}

fn weakest_section(events: &[RecordedJudgment], bar_ms: &[i64]) -> Option<SectionWeakness> {
    if bar_ms.len() < 2 {
        return None;
    }
    let last_bar = bar_ms.len() - 2;
    let mut sections = BTreeMap::<usize, (f32, u32)>::new();
    for event in events {
        let bar_index = bar_ms
            .partition_point(|&bar_start| bar_start <= event.chart_ms)
            .saturating_sub(1)
            .min(last_bar);
        let entry = sections.entry(bar_index).or_default();
        entry.0 += error_weight(event.kind);
        entry.1 += 1;
    }

    let (bar_index, (weight_sum, event_count)) = sections
        .into_iter()
        .filter(|(_, (_, event_count))| *event_count >= 3)
        .max_by(
            |(left_index, (left_weight, left_count)),
             (right_index, (right_weight, right_count))| {
                let left_average = left_weight / *left_count as f32;
                let right_average = right_weight / *right_count as f32;
                left_average
                    .total_cmp(&right_average)
                    .then_with(|| right_index.cmp(left_index))
            },
        )?;
    let loop_start_index = bar_index.saturating_sub(1);
    let loop_end_index = (bar_index + 2).min(bar_ms.len() - 1);
    let loop_start_ms = bar_ms[loop_start_index];
    let loop_end_ms = bar_ms[loop_end_index];
    (loop_end_ms > loop_start_ms).then_some(SectionWeakness {
        bar_start_ms: bar_ms[bar_index],
        bar_end_ms: bar_ms[bar_index + 1],
        loop_start_ms,
        loop_end_ms,
        average_weight: weight_sum / event_count as f32,
        event_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insufficient_evidence_omits_lane_and_section() {
        let events = [RecordedJudgment::new(1, JudgmentKind::Miss, 0, 0, 0)];
        let analysis = analyze_normal_play(&events, &[0, 2_000]);
        assert_eq!(analysis.bias_ms, None);
        assert_eq!(analysis.weakest_lane, None);
        assert_eq!(analysis.weakest_section, None);
    }

    #[test]
    fn stream_drops_events_after_its_cap() {
        let mut stream = NormalPlayEventStream::default();
        for index in 0..=MAX_NORMAL_PLAY_EVENTS {
            stream.push(RecordedJudgment::new(
                1,
                JudgmentKind::Perfect,
                0,
                index,
                index as i64,
            ));
        }
        assert_eq!(stream.events.len(), MAX_NORMAL_PLAY_EVENTS);
        assert!(stream.truncated);
    }
}
