//! Easing functions for UI animations (ADR-0014).
//!
//! Pure math — no crate dependency. osu-lazer uses OutQuint for most UI fades.

/// Easing curve selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EaseFunction {
    #[default]
    Linear,
    OutQuint,
    OutQuad,
    InOutCubic,
    OutElastic,
}

impl EaseFunction {
    /// Map normalized time `t` in [0, 1] to eased progress in [0, 1].
    pub fn ease(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::OutQuint => 1.0 - (1.0 - t).powi(5),
            Self::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Self::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::OutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_endpoints() {
        assert!((EaseFunction::Linear.ease(0.0) - 0.0).abs() < 0.001);
        assert!((EaseFunction::Linear.ease(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn out_quint_endpoints() {
        assert!((EaseFunction::OutQuint.ease(0.0) - 0.0).abs() < 0.001);
        assert!((EaseFunction::OutQuint.ease(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn out_quint_mid_fast_start() {
        let mid = EaseFunction::OutQuint.ease(0.5);
        assert!(mid > 0.5);
    }
}
