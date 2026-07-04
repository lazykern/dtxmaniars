//! Menu-kit motion primitives (spec 2026-07-05): spring scroll,
//! rolling numbers, beat pulse, staggered enter choreography.

use bevy::prelude::*;

/// Critically-damp-able spring toward `target`. Tick with real dt.
#[derive(Component, Debug, Clone)]
pub struct SpringValue {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    /// Stiffness (1/s^2). Higher = snappier.
    pub stiffness: f32,
    /// Damping ratio. 1.0 = critical (no overshoot), <1 overshoots.
    pub damping_ratio: f32,
}

impl SpringValue {
    pub fn new(value: f32, stiffness: f32, damping_ratio: f32) -> Self {
        Self {
            value,
            target: value,
            velocity: 0.0,
            stiffness,
            damping_ratio,
        }
    }

    /// Song-wheel default: slight overshoot, settles in ~250ms.
    pub fn wheel(value: f32) -> Self {
        Self::new(value, 400.0, 0.82)
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Semi-implicit Euler integration; clamps dt to avoid explosion
    /// on hitches. Snaps when close and slow.
    pub fn tick(&mut self, dt_s: f32) {
        let omega = self.stiffness.max(0.0).sqrt();
        let dt = dt_s.clamp(0.0, (1.0 / 30.0_f32).min(1.0 / omega.max(1.0)));
        let accel = -self.stiffness * (self.value - self.target)
            - 2.0 * self.damping_ratio * omega * self.velocity;
        self.velocity += accel * dt;
        self.value += self.velocity * dt;
        if (self.value - self.target).abs() < 0.0005 && self.velocity.abs() < 0.01 {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }

    pub fn settled(&self) -> bool {
        self.value == self.target && self.velocity == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(spring: &mut SpringValue, seconds: f32) {
        let steps = (seconds / 0.016).ceil() as usize;
        for _ in 0..steps {
            spring.tick(0.016);
        }
    }

    #[test]
    fn spring_settles_on_target() {
        let mut s = SpringValue::wheel(0.0);
        s.set_target(5.0);
        run(&mut s, 2.0);
        assert!(s.settled(), "value={} vel={}", s.value, s.velocity);
        assert_eq!(s.value, 5.0);
    }

    #[test]
    fn spring_underdamped_overshoots() {
        let mut s = SpringValue::new(0.0, 400.0, 0.5);
        s.set_target(1.0);
        let mut max = 0.0f32;
        for _ in 0..200 {
            s.tick(0.016);
            max = max.max(s.value);
        }
        assert!(max > 1.0);
    }

    #[test]
    fn spring_moves_toward_target_immediately() {
        let mut s = SpringValue::wheel(0.0);
        s.set_target(1.0);
        s.tick(0.016);
        assert!(s.value > 0.0);
    }

    #[test]
    fn spring_clamps_huge_dt() {
        let mut s = SpringValue::wheel(0.0);
        s.set_target(1.0);
        s.tick(10.0); // hitch — must not explode
        assert!(s.value.is_finite() && s.value.abs() < 10.0);
    }
}
