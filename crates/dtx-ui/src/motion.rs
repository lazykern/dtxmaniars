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

/// Exponential approach for numeric readouts (skill, BPM, notes).
#[derive(Component, Debug, Clone)]
pub struct RollingNumber {
    pub shown: f32,
    pub target: f32,
    /// Fraction of remaining distance closed per second (~10 = fast).
    pub rate: f32,
}

impl RollingNumber {
    pub fn new(value: f32) -> Self {
        Self {
            shown: value,
            target: value,
            rate: 10.0,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn tick(&mut self, dt_s: f32) {
        let diff = self.target - self.shown;
        if diff.abs() < 0.005 {
            self.shown = self.target;
            return;
        }
        self.shown += diff * (self.rate * dt_s).min(1.0);
    }
}

/// BPM-synced pulse in [1.0, 1.0+amplitude]; peak on the beat, decays
/// across the beat interval (quadratic falloff).
#[derive(Component, Debug, Clone)]
pub struct BeatPulse {
    pub bpm: f32,
    /// Beat phase in [0, 1).
    pub phase: f32,
    pub amplitude: f32,
}

impl BeatPulse {
    pub fn new(bpm: f32, amplitude: f32) -> Self {
        Self {
            bpm: bpm.max(1.0),
            phase: 0.0,
            amplitude,
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        let beats_per_s = self.bpm / 60.0;
        self.phase = (self.phase + dt_s * beats_per_s).rem_euclid(1.0);
    }

    /// 1.0+amplitude at phase 0, easing back to ~1.0 by phase 1.
    pub fn scale(&self) -> f32 {
        let falloff = (1.0 - self.phase).powi(2);
        1.0 + self.amplitude * falloff
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

    #[test]
    fn rolling_number_approaches_target() {
        let mut r = RollingNumber::new(0.0);
        r.set_target(100.0);
        for _ in 0..30 {
            r.tick(0.016);
        }
        assert!(r.shown > 50.0 && r.shown <= 100.0);
        for _ in 0..300 {
            r.tick(0.016);
        }
        assert!((r.shown - 100.0).abs() < 0.01);
    }

    #[test]
    fn rolling_number_snaps_when_close() {
        let mut r = RollingNumber::new(99.999);
        r.set_target(100.0);
        r.tick(0.016);
        assert_eq!(r.shown, 100.0);
    }

    #[test]
    fn beat_pulse_scale_peaks_on_beat() {
        let mut p = BeatPulse::new(60.0, 0.08);
        // At phase 0 (on-beat) scale is max.
        assert!((p.scale() - 1.08).abs() < 0.001);
        p.tick(0.5); // half a beat at 60bpm
        assert!(p.scale() < 1.04);
        p.tick(0.5); // full beat — wraps to peak
        assert!((p.scale() - 1.08).abs() < 0.01);
    }

    #[test]
    fn beat_pulse_bpm_change_keeps_phase_bounded() {
        let mut p = BeatPulse::new(157.0, 0.05);
        for _ in 0..1000 {
            p.tick(0.016);
        }
        assert!(p.phase >= 0.0 && p.phase < 1.0);
    }
}
