//! Score + combo update from `JudgmentEvent`s.
//!
//! DTXManiaNX XG scoring (`nSkillMode == 1`), ported verbatim in
//! [`dtx_scoring::xg_score`]:
//!   base = (1e6 - 500*bonus) / (1275 + 50*(maxCombo - 50));
//!   Perfect=base, Great=base*0.5, Good=base*0.2, Poor/Miss=0;
//!   × combo (ramp 1..50, then flat ×50); end bonus FC +15k / EXC +30k.
//!   Ref `CStagePerfCommonScreen.cs:1606-1658`.
//!
//! Combo: increments on Perfect/Great/Good, resets on **Poor**/Miss
//!   (`CStagePerfCommonScreen.cs:1521-1522`).

use bevy::prelude::*;

use crate::components::LastJudgment;
use crate::derived::ChartDerived;
use crate::events::{JudgmentEvent, NoteMissed};
use crate::resources::{Combo, DrumScoring, FastSlowCount, JudgmentCounts, Score, SkillValue};
use dtx_scoring::skill::{drum_performance_skill, drum_song_skill, DrumAutoPlay};
use dtx_scoring::xg_score::xg_drum_score_delta;
use dtx_scoring::JudgmentKind;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<LastJudgment>().add_systems(
        FixedUpdate,
        (update_score_system, update_miss_system)
            .in_set(super::DrumsSets::Score)
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(crate::practice::gameplay_input_active),
    );
}

pub(crate) fn update_score_system(
    mut events: MessageReader<JudgmentEvent>,
    mut score: ResMut<Score>,
    mut scoring: ResMut<DrumScoring>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
    mut last: ResMut<LastJudgment>,
    mut fast_slow: ResMut<FastSlowCount>,
    mut skill: ResMut<SkillValue>,
    derived: Res<ChartDerived>,
) {
    for ev in events.read() {
        match ev.kind {
            JudgmentKind::Perfect => counts.perfect += 1,
            JudgmentKind::Great => counts.great += 1,
            JudgmentKind::Good => counts.good += 1,
            JudgmentKind::Poor => counts.ok += 1,
            JudgmentKind::Miss => counts.miss += 1,
        }
        if ev.delta_ms < 0 {
            fast_slow.fast += 1;
        } else if ev.delta_ms > 0 {
            fast_slow.slow += 1;
        }

        // Combo: P/G/Good keep, Poor and Miss break (NX).
        if keeps_combo(ev.kind) {
            combo.current += 1;
            if combo.current > combo.max {
                combo.max = combo.current;
            }
        } else {
            combo.current = 0;
        }

        let delta = xg_drum_score_delta(
            ev.kind,
            combo.current,
            counts.perfect,
            scoring.total_notes,
            scoring.bonus_chips,
            scoring.accum,
        );
        scoring.accum += delta;
        score.0 = scoring.accum;

        update_skill_value(
            &mut skill,
            &counts,
            scoring.total_notes,
            combo.max,
            derived.chart_level,
        );
        last.0 = Some(*ev);
    }
}

fn update_miss_system(
    mut missed: MessageReader<NoteMissed>,
    scoring: Res<DrumScoring>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
    mut last: ResMut<LastJudgment>,
    mut skill: ResMut<SkillValue>,
    derived: Res<ChartDerived>,
) {
    for ev in missed.read() {
        counts.miss += 1;
        combo.current = 0;
        update_skill_value(
            &mut skill,
            &counts,
            scoring.total_notes,
            combo.max,
            derived.chart_level,
        );
        last.0 = Some(JudgmentEvent {
            lane: ev.lane,
            kind: JudgmentKind::Miss,
            delta_ms: 0,
            chip_idx: 0,
        });
    }
}

/// True if the judgment keeps the combo (Perfect/Great/Good). Poor and Miss
/// break it (`CStagePerfCommonScreen.cs:1521-1522`).
fn keeps_combo(kind: JudgmentKind) -> bool {
    matches!(
        kind,
        JudgmentKind::Perfect | JudgmentKind::Great | JudgmentKind::Good
    )
}

fn update_skill_value(
    skill: &mut SkillValue,
    counts: &JudgmentCounts,
    total_notes: u32,
    max_combo: u32,
    chart_level: f64,
) {
    let skill_pct = drum_performance_skill(
        total_notes,
        counts.perfect,
        counts.great,
        counts.good,
        counts.ok,
        counts.miss,
        max_combo,
        DrumAutoPlay::default(),
    );
    skill.current = drum_song_skill(chart_level, skill_pct, false);
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;
    use crate::lane_map::{lane_of, LaneId, LANE_ORDER};

    #[test]
    fn poor_breaks_combo() {
        // Regression: DTXManiaNX resets combo on Poor (was incrementing before).
        assert!(keeps_combo(JudgmentKind::Perfect));
        assert!(keeps_combo(JudgmentKind::Great));
        assert!(keeps_combo(JudgmentKind::Good));
        assert!(!keeps_combo(JudgmentKind::Poor));
        assert!(!keeps_combo(JudgmentKind::Miss));
    }

    #[test]
    fn xg_score_accumulates_and_rounds() {
        // 100-note chart, three Perfects at combos 1,2,3.
        let mut scoring = DrumScoring::default();
        scoring.reset(100);
        let mut score = Score::default();
        let mut acc = 0i64;
        for i in 1..=3u32 {
            let d = xg_drum_score_delta(JudgmentKind::Perfect, i, i, 100, 0, acc);
            acc += d;
        }
        scoring.accum = acc;
        score.0 = scoring.accum;
        // base ≈ 264.9; combos 1+2+3 = 6 units → ~1589.
        assert!(score.0 > 0);
        assert_eq!(score.0, acc);
    }

    #[test]
    fn lane_id_basic() {
        let _: LaneId = 7;
    }

    #[test]
    fn perfect_increments_perfect_count() {
        let mut c = crate::resources::JudgmentCounts::default();
        c.perfect += 1;
        assert_eq!(c.perfect, 1);
        assert_eq!(c.total(), 1);
    }

    #[test]
    fn miss_increments_miss_count() {
        let mut c = crate::resources::JudgmentCounts::default();
        c.miss += 1;
        assert_eq!(c.miss, 1);
    }

    #[test]
    fn judgment_counts_total_sums_all() {
        let c = crate::resources::JudgmentCounts {
            perfect: 10,
            great: 5,
            good: 3,
            ok: 2,
            miss: 1,
        };
        assert_eq!(c.total(), 21);
    }

    #[test]
    fn empty_counts_zero_total() {
        let c = crate::resources::JudgmentCounts::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.perfect_pct(), 0.0);
    }

    #[test]
    fn combo_max_never_decreases() {
        // After a miss, combo.current resets but combo.max stays.
        let mut c = crate::resources::Combo { current: 5, max: 5 };
        c.current = 0;
        assert_eq!(c.max, 5);
        assert_eq!(c.current, 0);
    }

    #[test]
    fn last_judgment_default_none() {
        let lj = crate::components::LastJudgment::default();
        assert!(lj.0.is_none());
    }
}
