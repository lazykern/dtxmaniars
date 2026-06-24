//! Guitar judge: LaneHit → JudgmentEvent.
//!
//! Consumes `LaneHit` messages, finds the closest un-judged note in the same
//! lane within the judgment window, emits a `JudgmentEvent`.
//!
//! Gated on `EGameMode::Guitar` so only the active mode's pipeline runs.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use dtx_timing::AudioClock;
use game_shell::EGameMode;

use crate::components::{Note, NoteVisual};
use crate::events::{JudgmentEvent, LaneHit};
use crate::lane_map::lane_channel;
use crate::resources::{ActiveChart, Combo, JudgmentCounts, Score};

/// Max ms from target for a hit to count at all. Beyond this = Miss.
const MAX_JUDGE_WINDOW_MS: i64 = 200;

#[derive(Default, Resource, Debug)]
pub struct JudgedChips(pub std::collections::HashSet<usize>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<JudgedChips>()
        .add_systems(Update, judge_lane_hit);
}

fn judge_lane_hit(
    mut events: MessageReader<LaneHit>,
    mut score: ResMut<Score>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
    clock: Res<AudioClock>,
    mode: Res<EGameMode>,
    chart: Res<ActiveChart>,
    mut judged: ResMut<JudgedChips>,
    mut out: MessageWriter<JudgmentEvent>,
) {
    if *mode != EGameMode::Guitar {
        return;
    }
    let Some(now) = clock.current_ms else {
        return;
    };
    for ev in events.read() {
        let Some(target_channel) = lane_channel(ev.lane) else {
            continue;
        };
        let mut best: Option<(usize, i64)> = None;
        for (idx, chip) in chart.chart.chips.iter().enumerate() {
            if chip.channel != target_channel || judged.0.contains(&idx) {
                continue;
            }
            let target_ms = dtx_timing::math::chip_time_ms(
                chip.measure,
                chip.value,
                chart.chart.metadata.bpm.unwrap_or(120.0),
            );
            let delta = now - target_ms;
            if delta.abs() > MAX_JUDGE_WINDOW_MS {
                continue;
            }
            match best {
                Some((_, d)) if d.abs() <= delta.abs() => {}
                _ => best = Some((idx, delta)),
            }
        }
        let Some((idx, delta)) = best else {
            continue;
        };
        judged.0.insert(idx);

        let kind = dtx_scoring::classify(delta as i32);

        // Mirror score.rs scoring to keep this consistent.
        let pts: u64 = match kind {
            JudgmentKind::Perfect => 1000,
            JudgmentKind::Great => 500,
            JudgmentKind::Good => 200,
            JudgmentKind::Ok => 100,
            JudgmentKind::Miss => 0,
        };
        score.0 += pts;
        match kind {
            JudgmentKind::Perfect => counts.perfect += 1,
            JudgmentKind::Great => counts.great += 1,
            JudgmentKind::Good => counts.good += 1,
            JudgmentKind::Ok => counts.ok += 1,
            JudgmentKind::Miss => counts.miss += 1,
        }
        if kind == JudgmentKind::Miss {
            combo.current = 0;
        } else {
            combo.current += 1;
            if combo.current > combo.max {
                combo.max = combo.current;
            }
        }
        out.write(JudgmentEvent {
            lane: ev.lane,
            kind,
            delta_ms: delta as i32,
        });
    }
}
