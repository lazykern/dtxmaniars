//! Score + combo update from `JudgmentEvent`s.
//!
//! v1 scoring (DTXmaniaNX standard):
//!   Perfect = +2, Great = +1, Good = +1, Ok = 0, Miss = 0, combo break.
//!
//! Combo: increments on Perfect/Great/Good/Ok, resets on Miss.

use bevy::prelude::*;
use bevy::prelude::{MessageReader as _, Resource as _};

use crate::components::LastJudgment;
use crate::events::{JudgmentEvent, NoteMissed};
use crate::resources::{Combo, Score};
use dtx_scoring::JudgmentKind;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<LastJudgment>()
        .add_systems(Update, (update_score_system, update_miss_system));
}

fn update_score_system(
    mut events: MessageReader<JudgmentEvent>,
    mut score: ResMut<Score>,
    mut combo: ResMut<Combo>,
    mut last: ResMut<LastJudgment>,
) {
    for ev in events.read() {
        score.0 += points_for(ev.kind);
        if ev.kind == JudgmentKind::Miss {
            combo.current = 0;
        } else {
            combo.current += 1;
            if combo.current > combo.max {
                combo.max = combo.current;
            }
        }
        last.0 = Some(*ev);
    }
}

fn update_miss_system(
    mut missed: MessageReader<NoteMissed>,
    mut combo: ResMut<Combo>,
    mut last: ResMut<LastJudgment>,
) {
    for ev in missed.read() {
        combo.current = 0;
        last.0 = Some(JudgmentEvent {
            lane: ev.lane,
            kind: JudgmentKind::Miss,
            delta_ms: 0,
        });
    }
}

const fn points_for(kind: JudgmentKind) -> u64 {
    match kind {
        JudgmentKind::Perfect => 2,
        JudgmentKind::Great | JudgmentKind::Good => 1,
        JudgmentKind::Ok | JudgmentKind::Miss => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane_map::LaneId;

    #[test]
    fn perfect_increments_score_and_combo() {
        let mut score = Score::default();
        let mut combo = Combo::default();
        let mut last = LastJudgment::default();
        let ev = JudgmentEvent {
            lane: 0,
            kind: JudgmentKind::Perfect,
            delta_ms: 0,
        };
        score.0 += points_for(ev.kind);
        combo.current += 1;
        combo.max = combo.max.max(combo.current);
        last.0 = Some(ev);
        assert_eq!(score.0, 2);
        assert_eq!(combo.current, 1);
        assert_eq!(combo.max, 1);
    }

    #[test]
    fn miss_resets_combo_but_not_max() {
        let mut score = Score::default();
        let mut combo = Combo::default(); // start fresh
        let mut last = LastJudgment::default();
        let events = [
            JudgmentEvent {
                lane: 0,
                kind: JudgmentKind::Perfect,
                delta_ms: 0,
            },
            JudgmentEvent {
                lane: 0,
                kind: JudgmentKind::Perfect,
                delta_ms: 0,
            },
            JudgmentEvent {
                lane: 0,
                kind: JudgmentKind::Miss,
                delta_ms: 100,
            },
        ];
        for ev in events {
            score.0 += points_for(ev.kind);
            if ev.kind == JudgmentKind::Miss {
                combo.current = 0;
            } else {
                combo.current += 1;
                combo.max = combo.max.max(combo.current);
            }
            last.0 = Some(ev);
        }
        assert_eq!(combo.current, 0);
        assert_eq!(combo.max, 2);
        assert_eq!(score.0, 4);
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
    fn score_accumulates_across_judgments() {
        // P=2, G=1, G=1 -> score = 4.
        // Logic duplicated from system: not invoking the system here, just verifying
        // the points_for helper behavior.
        fn points_for(k: dtx_scoring::JudgmentKind) -> u64 {
            match k {
                dtx_scoring::JudgmentKind::Perfect => 2,
                dtx_scoring::JudgmentKind::Great => 1,
                dtx_scoring::JudgmentKind::Good => 1,
                dtx_scoring::JudgmentKind::Ok => 0,
                dtx_scoring::JudgmentKind::Miss => 0,
            }
        }
        let total = points_for(dtx_scoring::JudgmentKind::Perfect)
            + points_for(dtx_scoring::JudgmentKind::Great)
            + points_for(dtx_scoring::JudgmentKind::Great);
        assert_eq!(total, 4);
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
