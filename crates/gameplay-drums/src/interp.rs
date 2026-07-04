//! Sub-frame interpolation between FixedUpdate ticks.
//!
//! `GameplayClock` advances on a fixed 60Hz cadence. A render frame at a higher
//! rate sees 0, 1, or 2 fixed ticks per frame, so reading the clock directly
//! makes notes step. To get sub-frame motion we lerp between the visual-clock
//! values captured at the start and end of the latest fixed tick, using
//! `Time<Fixed>::overstep_fraction()` as alpha.
//!
//! `snapshot_render_clock` runs in `RunFixedMainLoopSystems::AfterFixedMainLoop`
//! so it observes the latest fixed tick's output before Update render systems.
//!
//! Ported from dtxpt `gameplay/interp.rs`.

use bevy::app::RunFixedMainLoopSystems;
use bevy::prelude::*;

use crate::resources::GameplayClock;
use game_shell::{AppState, PauseState};

/// Snapshot of the visual clock at the boundaries of the most recent fixed tick
/// plus the sub-frame alpha. Render systems read [`Self::now_ms`] for note Y.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct RenderClock {
    /// `GameplayClock::prev_visual_ms` at the end of the most recent fixed tick.
    pub prev_ms: f64,
    /// `GameplayClock::visual_ms` at the end of the most recent fixed tick.
    pub current_ms: f64,
    /// `Time<Fixed>::overstep_fraction()` at snapshot time. In [0.0, 1.0).
    pub alpha: f64,
}

impl RenderClock {
    /// Sub-frame interpolated visual clock (ms): `lerp(prev, current, alpha)`.
    pub fn now_ms(&self) -> f64 {
        self.prev_ms + (self.current_ms - self.prev_ms) * self.alpha
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RenderClock>().add_systems(
        RunFixedMainLoop,
        snapshot_render_clock
            .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running)),
    );
}

fn snapshot_render_clock(
    fixed_time: Res<Time<Fixed>>,
    clock: Res<GameplayClock>,
    mut render: ResMut<RenderClock>,
) {
    render.prev_ms = clock.prev_visual_ms();
    render.current_ms = clock.visual_ms();
    render.alpha = fixed_time.overstep_fraction() as f64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_lerps_prev_to_current() {
        let r = RenderClock {
            prev_ms: 100.0,
            current_ms: 200.0,
            alpha: 0.3,
        };
        assert!((r.now_ms() - 130.0).abs() < 1e-6);
    }

    #[test]
    fn now_at_alpha_zero_is_prev() {
        let r = RenderClock {
            prev_ms: 500.0,
            current_ms: 1000.0,
            alpha: 0.0,
        };
        assert!((r.now_ms() - 500.0).abs() < 1e-6);
    }

    #[test]
    fn now_at_alpha_one_is_current() {
        let r = RenderClock {
            prev_ms: 500.0,
            current_ms: 1000.0,
            alpha: 1.0,
        };
        assert!((r.now_ms() - 1000.0).abs() < 1e-6);
    }
}
