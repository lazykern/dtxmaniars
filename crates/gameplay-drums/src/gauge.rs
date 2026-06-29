//! Stage gauge — BocuD-style 0..1 gauge (dtxpt `gauge.rs` deltas).

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;
use game_shell::AppState;

use crate::events::JudgmentEvent;

pub const GAUGE_START: f32 = 0.80;
pub const GAUGE_CLEAR: f32 = 0.80;

pub fn gauge_delta(kind: JudgmentKind) -> f32 {
    match kind {
        JudgmentKind::Perfect => 0.005,
        JudgmentKind::Great => 0.002,
        JudgmentKind::Good => 0.0,
        JudgmentKind::Poor => -0.03,
        JudgmentKind::Miss => -0.06,
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct StageGauge {
    pub value: f32,
    pub failed: bool,
}

impl StageGauge {
    pub fn reset(&mut self) {
        self.value = GAUGE_START;
        self.failed = false;
    }

    pub fn apply_judgment(&mut self, kind: JudgmentKind) {
        if self.failed {
            return;
        }
        self.value = (self.value + gauge_delta(kind)).clamp(0.0, 1.0);
        if self.value <= 0.0 {
            self.value = 0.0;
            self.failed = true;
        }
    }

    pub fn pct(&self) -> f32 {
        self.value * 100.0
    }
}

pub fn gauge_fill_color(gauge: f32, failed: bool) -> Color {
    if failed {
        return Color::srgb(0.95, 0.2, 0.25);
    }
    if gauge >= GAUGE_CLEAR {
        Color::srgb(0.25, 0.9, 0.45)
    } else if gauge >= 0.4 {
        Color::srgb(0.95, 0.85, 0.2)
    } else {
        Color::srgb(0.95, 0.45, 0.2)
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<StageGauge>()
        .add_systems(OnEnter(AppState::Performance), reset_gauge_on_enter)
        .add_systems(
            FixedUpdate,
            apply_gauge_on_judgment.run_if(in_state(AppState::Performance)),
        );
}

fn reset_gauge_on_enter(mut gauge: ResMut<StageGauge>) {
    gauge.reset();
}

fn apply_gauge_on_judgment(
    mut events: MessageReader<JudgmentEvent>,
    mut gauge: ResMut<StageGauge>,
) {
    for ev in events.read() {
        gauge.apply_judgment(ev.kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_increases_gauge() {
        let mut g = StageGauge {
            value: GAUGE_START,
            failed: false,
        };
        g.apply_judgment(JudgmentKind::Perfect);
        assert!(g.value > GAUGE_START);
    }

    #[test]
    fn miss_can_fail() {
        let mut g = StageGauge {
            value: 0.01,
            failed: false,
        };
        g.apply_judgment(JudgmentKind::Miss);
        assert!(g.failed);
    }
}
