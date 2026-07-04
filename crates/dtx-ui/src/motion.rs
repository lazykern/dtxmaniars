//! Menu-kit motion primitives (spec 2026-07-05): spring scroll,
//! rolling numbers, beat pulse, staggered enter choreography.

use bevy::prelude::*;
use bevy::ui::Val2;

use crate::easing::EaseFunction;

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
    /// Exponential rate (1/s); remaining distance shrinks by e^-rate each second.
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
        let dt = dt_s.max(0.0);
        let diff = self.target - self.shown;
        if diff.abs() < 0.005 {
            self.shown = self.target;
            return;
        }
        self.shown += diff * (1.0 - (-self.rate * dt).exp());
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
        let beats_per_s = self.bpm.max(1.0) / 60.0;
        self.phase = (self.phase + dt_s.max(0.0) * beats_per_s).rem_euclid(1.0);
    }

    /// 1.0+amplitude at phase 0, easing back to ~1.0 by phase 1.
    pub fn scale(&self) -> f32 {
        let falloff = (1.0 - self.phase).powi(2);
        1.0 + self.amplitude * falloff
    }
}

/// Staggered screen-enter animation: node starts at `offset` px and
/// slides to rest after `delay_ms`, over `duration_ms` with OutQuint.
#[derive(Component, Debug, Clone)]
pub struct EnterChoreo {
    pub offset: Vec2,
    pub delay_ms: f32,
    pub duration_ms: f32,
    pub elapsed_ms: f32,
    pub easing: EaseFunction,
}

impl EnterChoreo {
    pub fn slide(offset: Vec2, delay_ms: f32, duration_ms: f32) -> Self {
        Self {
            offset,
            delay_ms,
            duration_ms: duration_ms.max(0.001),
            elapsed_ms: 0.0,
            easing: EaseFunction::OutQuint,
        }
    }

    pub fn tick(&mut self, delta_ms: f32) {
        self.elapsed_ms += delta_ms;
    }

    /// 0 before delay, eased 0..1 across duration, 1 after.
    pub fn progress(&self) -> f32 {
        let t = ((self.elapsed_ms - self.delay_ms) / self.duration_ms).clamp(0.0, 1.0);
        self.easing.ease(t)
    }

    pub fn finished(&self) -> bool {
        self.elapsed_ms >= self.delay_ms + self.duration_ms
    }

    pub fn current_offset(&self) -> Vec2 {
        self.offset * (1.0 - self.progress())
    }
}

/// Drives every `EnterChoreo` + `UiTransform`; removes the component
/// when finished so idle nodes cost nothing.
pub fn enter_choreo_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut EnterChoreo, &mut UiTransform)>,
) {
    let dt_ms = time.delta_secs() * 1000.0;
    for (entity, mut choreo, mut tf) in &mut q {
        choreo.tick(dt_ms);
        let off = choreo.current_offset();
        tf.translation = Val2::px(off.x, off.y);
        if choreo.finished() {
            tf.translation = Val2::ZERO;
            commands.entity(entity).remove::<EnterChoreo>();
        }
    }
}

/// Drives every standalone `BeatPulse` + `UiTransform` scale.
pub fn beat_pulse_system(time: Res<Time>, mut q: Query<(&mut BeatPulse, &mut UiTransform)>) {
    for (mut pulse, mut tf) in &mut q {
        pulse.tick(time.delta_secs());
        let s = pulse.scale();
        tf.scale = Vec2::splat(s);
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
    fn beat_pulse_phase_stays_bounded() {
        let mut p = BeatPulse::new(157.0, 0.05);
        for i in 0..1000 {
            if i == 500 {
                p.bpm = 0.0;
            }
            p.tick(0.016);
        }
        assert!(p.phase >= 0.0 && p.phase < 1.0);
    }

    #[test]
    fn enter_choreo_waits_for_delay_then_progresses() {
        let mut c = EnterChoreo::slide(Vec2::new(-40.0, 0.0), 60.0, 200.0);
        c.tick(30.0);
        assert_eq!(c.progress(), 0.0);
        c.tick(130.0); // 160ms total = 100ms into 200ms anim
        let p = c.progress();
        assert!(p > 0.0 && p < 1.0);
        c.tick(500.0);
        assert_eq!(c.progress(), 1.0);
        assert!(c.finished());
    }

    #[test]
    fn enter_choreo_offset_shrinks_to_zero() {
        let mut c = EnterChoreo::slide(Vec2::new(-40.0, 0.0), 0.0, 200.0);
        assert_eq!(c.current_offset(), Vec2::new(-40.0, 0.0));
        c.tick(1000.0);
        assert_eq!(c.current_offset(), Vec2::ZERO);
    }

    #[test]
    fn choreo_system_moves_ui_transform() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_systems(Update, enter_choreo_system);
        let e = app
            .world_mut()
            .spawn((
                EnterChoreo::slide(Vec2::new(-40.0, 0.0), 0.0, 200.0),
                UiTransform::default(),
            ))
            .id();
        app.update();
        app.update();
        let tf = app.world().get::<UiTransform>(e).unwrap();
        // after two ticks the node moved off its start offset
        assert_ne!(tf.translation, Val2::px(-40.0, 0.0));
    }
}
