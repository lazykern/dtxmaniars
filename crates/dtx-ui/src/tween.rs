//! Hand-rolled tween helpers (ADR-0014, ADR-0007).

use crate::easing::EaseFunction;

/// Scalar tween state — advance with `tick` each frame.
#[derive(Debug, Clone)]
pub struct ScalarTween {
    pub from: f32,
    pub to: f32,
    pub elapsed_ms: f32,
    pub duration_ms: f32,
    pub easing: EaseFunction,
    pub finished: bool,
}

impl ScalarTween {
    pub fn new(from: f32, to: f32, duration_ms: f32, easing: EaseFunction) -> Self {
        Self {
            from,
            to,
            elapsed_ms: 0.0,
            duration_ms: duration_ms.max(0.001),
            easing,
            finished: false,
        }
    }

    pub fn value(&self) -> f32 {
        if self.finished {
            return self.to;
        }
        let t = (self.elapsed_ms / self.duration_ms).clamp(0.0, 1.0);
        let e = self.easing.ease(t);
        self.from + (self.to - self.from) * e
    }

    pub fn tick(&mut self, delta_ms: f32) {
        if self.finished {
            return;
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.elapsed_ms = self.duration_ms;
            self.finished = true;
        }
    }

    pub fn reset(&mut self, from: f32, to: f32, duration_ms: f32, easing: EaseFunction) {
        self.from = from;
        self.to = to;
        self.duration_ms = duration_ms.max(0.001);
        self.easing = easing;
        self.elapsed_ms = 0.0;
        self.finished = false;
    }

    pub fn reset_target(&mut self, to: f32) {
        self.reset(self.to, to, self.duration_ms, self.easing);
    }
}

/// Lerp two f32 values with easing.
pub fn lerp_eased(from: f32, to: f32, t: f32, easing: EaseFunction) -> f32 {
    from + (to - from) * easing.ease(t.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tween_reaches_target() {
        let mut tw = ScalarTween::new(0.0, 1.0, 100.0, EaseFunction::Linear);
        tw.tick(100.0);
        assert!(tw.finished);
        assert!((tw.value() - 1.0).abs() < 0.001);
    }

    #[test]
    fn tween_midpoint_linear() {
        let mut tw = ScalarTween::new(0.0, 100.0, 100.0, EaseFunction::Linear);
        tw.tick(50.0);
        assert!((tw.value() - 50.0).abs() < 0.1);
    }
}
