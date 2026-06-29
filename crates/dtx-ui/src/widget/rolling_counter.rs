//! Rolling number display (osu RollingCounter pattern).

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::tween::ScalarTween;

/// Marker root for a rolling counter widget.
#[derive(Component)]
pub struct RollingCounter {
    pub displayed: u64,
    pub target: u64,
    tween: ScalarTween,
}

impl RollingCounter {
    pub fn set_target(&mut self, value: u64) {
        if value == self.target {
            return;
        }
        self.target = value;
        let duration = (40.0 + (value.abs_diff(self.displayed) as f32).sqrt() * 8.0).min(400.0);
        self.tween.reset(
            self.displayed as f32,
            value as f32,
            duration,
            EaseFunction::OutQuad,
        );
    }

    pub fn tick(&mut self, delta_ms: f32) {
        if self.tween.finished {
            return;
        }
        self.tween.tick(delta_ms);
        self.displayed = self.tween.value().round() as u64;
    }

    pub fn display_text(&self, prefix: &str) -> String {
        format!("{prefix}{}", self.displayed)
    }
}

impl Default for RollingCounter {
    fn default() -> Self {
        Self {
            displayed: 0,
            target: 0,
            tween: ScalarTween::new(0.0, 0.0, 1.0, EaseFunction::OutQuad),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_animates_toward_target() {
        let mut c = RollingCounter::default();
        c.set_target(100);
        for _ in 0..50 {
            c.tick(16.0);
        }
        assert!(c.displayed > 0);
        assert!(c.displayed <= 100);
    }
}
