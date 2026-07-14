//! Stage life gauge — DTXManiaNX `CActPerfCommonGauge` port.
//!
//! Reference:
//! `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CActPerfCommonGauge.cs:37-154`.
//!
//! - Range `[GAUGE_MIN, GAUGE_MAX]` = `[-0.1, 1.0]`; starts at `2/3`.
//! - Fails when the value falls to `GAUGE_MIN` (`-0.1`), NOT at zero.
//! - Per-judgment deltas differ by skill mode (Classic vs XG). Miss damage is
//!   scaled by the configured damage level.

use bevy::prelude::*;
use dtx_core::constants::DamageLevel;
use dtx_scoring::JudgmentKind;
use game_shell::AppState;

use crate::events::{JudgmentEvent, NoteMissed};

/// Full gauge.
pub const GAUGE_MAX: f32 = 1.0;
/// Starting gauge value (`2/3`). `CActPerfCommonGauge.cs:60`.
pub const GAUGE_INITIAL: f32 = 2.0 / 3.0;
/// Fail threshold (`-0.1`). `CActPerfCommonGauge.cs:39,146`.
pub const GAUGE_MIN: f32 = -0.1;
/// Danger threshold used by the HUD (`0.3`).
pub const GAUGE_DANGER: f32 = 0.3;

/// XG-mode (`nSkillMode == 1`) drum gauge deltas. `CActPerfCommonGauge.cs` XG branch.
pub fn gauge_delta_xg(kind: JudgmentKind) -> f32 {
    match kind {
        JudgmentKind::Perfect => 0.005,
        JudgmentKind::Great => 0.001,
        JudgmentKind::Good => 0.0,
        JudgmentKind::Poor => 0.0,
        JudgmentKind::Miss => -0.017,
    }
}

/// Classic-mode drum gauge deltas. `CActPerfCommonGauge.cs` classic branch.
pub fn gauge_delta_classic(kind: JudgmentKind) -> f32 {
    match kind {
        JudgmentKind::Perfect => 0.004,
        JudgmentKind::Great => 0.002,
        JudgmentKind::Good => 0.0,
        JudgmentKind::Poor => 0.0,
        JudgmentKind::Miss => -0.020,
    }
}

/// Damage-level scaling applied to the Miss drain. `CActPerfCommonGauge.cs:97-120`.
pub fn miss_damage_factor(level: DamageLevel) -> f32 {
    match level {
        DamageLevel::None => 0.0,
        DamageLevel::Small => 0.25,
        DamageLevel::Normal => 0.5,
        DamageLevel::High => 0.75,
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct StageGauge {
    pub value: f32,
    pub failed: bool,
    /// XG scoring mode (DTXManiaNX default). Toggles the delta table.
    pub xg_mode: bool,
    /// Damage level controlling Miss drain scaling.
    pub damage_level: DamageLevel,
}

impl Default for StageGauge {
    fn default() -> Self {
        Self {
            value: GAUGE_INITIAL,
            failed: false,
            xg_mode: true,
            damage_level: DamageLevel::Normal,
        }
    }
}

impl StageGauge {
    pub fn reset(&mut self) {
        self.value = GAUGE_INITIAL;
        self.failed = false;
    }

    /// Per-judgment gauge delta including Miss damage scaling.
    pub fn delta(&self, kind: JudgmentKind) -> f32 {
        let base = if self.xg_mode {
            gauge_delta_xg(kind)
        } else {
            gauge_delta_classic(kind)
        };
        if kind == JudgmentKind::Miss {
            base * miss_damage_factor(self.damage_level)
        } else {
            base
        }
    }

    pub fn apply_judgment(&mut self, kind: JudgmentKind) {
        if self.failed {
            return;
        }
        self.value = (self.value + self.delta(kind)).clamp(GAUGE_MIN, GAUGE_MAX);
        if self.value <= GAUGE_MIN {
            self.failed = true;
        }
    }

    /// Displayed fill percentage (0..100); negative internal values read as 0.
    pub fn pct(&self) -> f32 {
        self.value.clamp(0.0, GAUGE_MAX) * 100.0
    }

    /// True while the gauge sits in the danger zone.
    pub fn in_danger(&self) -> bool {
        self.value < GAUGE_DANGER
    }
}

pub fn gauge_fill_color(gauge: f32, failed: bool) -> Color {
    if failed {
        return Color::srgb(0.95, 0.2, 0.25);
    }
    if gauge >= GAUGE_INITIAL {
        Color::srgb(0.25, 0.9, 0.45)
    } else if gauge >= GAUGE_DANGER {
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
            apply_gauge_on_judgment
                .run_if(in_state(AppState::Performance))
                .run_if(crate::practice::gameplay_input_active),
        );
}

fn reset_gauge_on_enter(mut gauge: ResMut<StageGauge>) {
    gauge.reset();
}

fn apply_gauge_on_judgment(
    mut events: MessageReader<JudgmentEvent>,
    mut missed: MessageReader<NoteMissed>,
    mut gauge: ResMut<StageGauge>,
) {
    for ev in events.read() {
        gauge.apply_judgment(ev.kind);
    }
    // Unplayed chips arrive as `NoteMissed`, not `JudgmentEvent` — they drain
    // the gauge like any other Miss (`CActPerfCommonGauge.cs` applies the Miss
    // delta to every missed chip). Without this the gauge only reacted to
    // hit-time judgments and pure neglect could never fail the stage.
    for _ in missed.read() {
        gauge.apply_judgment(JudgmentKind::Miss);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_two_thirds() {
        let g = StageGauge::default();
        assert!((g.value - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn perfect_increases_gauge() {
        let mut g = StageGauge::default();
        let before = g.value;
        g.apply_judgment(JudgmentKind::Perfect);
        assert!(g.value > before);
    }

    #[test]
    fn poor_does_not_drain_xg() {
        let mut g = StageGauge::default();
        let before = g.value;
        g.apply_judgment(JudgmentKind::Poor);
        assert!((g.value - before).abs() < 1e-6);
    }

    #[test]
    fn miss_scaled_by_damage_level() {
        let mut normal = StageGauge {
            damage_level: DamageLevel::Normal,
            ..Default::default()
        };
        let mut high = StageGauge {
            damage_level: DamageLevel::High,
            ..Default::default()
        };
        normal.apply_judgment(JudgmentKind::Miss);
        high.apply_judgment(JudgmentKind::Miss);
        // High damage drains more than Normal.
        assert!(high.value < normal.value);
    }

    #[test]
    fn fails_at_minus_point_one() {
        let mut g = StageGauge {
            value: GAUGE_MIN + 0.005,
            ..Default::default()
        };
        g.apply_judgment(JudgmentKind::Miss);
        assert!(g.failed);
        assert!(g.value <= GAUGE_MIN);
    }

    #[test]
    fn unplayed_note_missed_drains_gauge() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<bevy::ecs::message::Messages<JudgmentEvent>>();
        world.init_resource::<bevy::ecs::message::Messages<NoteMissed>>();
        world.insert_resource(StageGauge::default());
        let before = world.resource::<StageGauge>().value;
        world.write_message(NoteMissed {
            lane: 0,
            audio_ms: 0,
            chip_idx: 0,
        });
        world
            .run_system_once(apply_gauge_on_judgment)
            .expect("system runs");
        assert!(world.resource::<StageGauge>().value < before);
    }

    #[test]
    fn none_damage_level_never_drains_on_miss() {
        let mut g = StageGauge {
            damage_level: DamageLevel::None,
            ..Default::default()
        };
        let before = g.value;
        g.apply_judgment(JudgmentKind::Miss);
        assert!((g.value - before).abs() < 1e-6);
    }
}
