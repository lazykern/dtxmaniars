//! Guitar score + judgment counter system.
//!
//! Mirror of gameplay-drums::score::update_score_system. Consumes
//! `JudgmentEvent` messages, increments Score + Combo + JudgmentCounts.
//!
//! Note: M6b does not yet port the full judge system (chip → judgment). It
//! only consumes already-judged events. The actual judge is M6.1.

use bevy::prelude::*;
use bevy::prelude::{MessageReader as _, Resource as _};

use crate::events::{JudgmentEvent, NoteMissed};
use crate::resources::{Combo, JudgmentCounts, Score};
use dtx_scoring::JudgmentKind;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (update_score_system, update_miss_system));
}

fn update_score_system(
    mut events: MessageReader<JudgmentEvent>,
    mut score: ResMut<Score>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
) {
    for ev in events.read() {
        score.0 += points_for(ev.kind);
        match ev.kind {
            JudgmentKind::Perfect => counts.perfect += 1,
            JudgmentKind::Great => counts.great += 1,
            JudgmentKind::Good => counts.good += 1,
            JudgmentKind::Ok => counts.ok += 1,
            JudgmentKind::Miss => counts.miss += 1,
        }
        if ev.kind == JudgmentKind::Miss {
            combo.current = 0;
        } else {
            combo.current += 1;
            if combo.current > combo.max {
                combo.max = combo.current;
            }
        }
    }
}

fn update_miss_system(
    mut missed: MessageReader<NoteMissed>,
    mut combo: ResMut<Combo>,
    mut counts: ResMut<JudgmentCounts>,
) {
    for _ev in missed.read() {
        combo.current = 0;
        counts.miss += 1;
    }
}

fn points_for(kind: JudgmentKind) -> u64 {
    match kind {
        JudgmentKind::Perfect => 1000,
        JudgmentKind::Great => 500,
        JudgmentKind::Good => 200,
        JudgmentKind::Ok => 100,
        JudgmentKind::Miss => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_perfect_is_1000() {
        assert_eq!(points_for(JudgmentKind::Perfect), 1000);
    }

    #[test]
    fn points_miss_is_zero() {
        assert_eq!(points_for(JudgmentKind::Miss), 0);
    }

    #[test]
    fn points_great_less_than_perfect() {
        assert!(points_for(JudgmentKind::Great) < points_for(JudgmentKind::Perfect));
    }
}
