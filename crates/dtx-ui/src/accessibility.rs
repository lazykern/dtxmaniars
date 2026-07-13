use bevy::prelude::Resource;
use dtx_config::{AccessibilityConfig, TextScale};

/// Whether decorative motion should play at full strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDecision {
    Full,
    Reduced,
}

/// Whether flash effects should use their full presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashDecision {
    Full,
    Reduced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntranceEffect {
    FullMotion { duration_ms: u32 },
    OpacityOnly { duration_ms: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitEffect {
    FullFlash { duration_ms: u32 },
    StableOutline { duration_ms: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerEffect {
    PulsingBorder,
    ConstantBorder,
}

pub const fn entrance_effect(policy: AccessibilityPolicy) -> EntranceEffect {
    match policy.motion_decision() {
        MotionDecision::Full => EntranceEffect::FullMotion { duration_ms: 300 },
        MotionDecision::Reduced => EntranceEffect::OpacityOnly { duration_ms: 120 },
    }
}

pub const fn hit_effect(policy: AccessibilityPolicy) -> HitEffect {
    match policy.flash_decision() {
        FlashDecision::Full => HitEffect::FullFlash { duration_ms: 180 },
        FlashDecision::Reduced => HitEffect::StableOutline { duration_ms: 120 },
    }
}

pub const fn danger_effect(policy: AccessibilityPolicy) -> DangerEffect {
    match policy.flash_decision() {
        FlashDecision::Full => DangerEffect::PulsingBorder,
        FlashDecision::Reduced => DangerEffect::ConstantBorder,
    }
}

/// Runtime accessibility decisions derived once from persisted configuration.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct AccessibilityPolicy {
    text_scale: TextScale,
    reduce_motion: bool,
    reduce_flashes: bool,
    background_motion: bool,
}

impl Default for AccessibilityPolicy {
    fn default() -> Self {
        Self::from(&AccessibilityConfig::default())
    }
}

impl From<&AccessibilityConfig> for AccessibilityPolicy {
    fn from(config: &AccessibilityConfig) -> Self {
        Self {
            text_scale: config.text_scale,
            reduce_motion: config.reduce_motion,
            reduce_flashes: config.reduce_flashes,
            background_motion: config.background_motion,
        }
    }
}

impl AccessibilityPolicy {
    pub const fn text_multiplier(&self) -> f32 {
        self.text_scale.multiplier()
    }

    pub const fn text_scale(&self) -> TextScale {
        self.text_scale
    }

    pub const fn screen_transition_ms(&self) -> u32 {
        if self.reduce_motion {
            120
        } else {
            300
        }
    }

    pub const fn motion_decision(&self) -> MotionDecision {
        if self.reduce_motion {
            MotionDecision::Reduced
        } else {
            MotionDecision::Full
        }
    }

    pub const fn flash_decision(&self) -> FlashDecision {
        if self.reduce_flashes {
            FlashDecision::Reduced
        } else {
            FlashDecision::Full
        }
    }

    pub const fn background_motion(&self) -> bool {
        self.background_motion
    }
}

/// A recoverable startup warning waiting for the notification surface.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupConfigWarning(pub Option<String>);

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_config::TextScale;

    #[test]
    fn policy_maps_independent_controls_without_a_preset() {
        let cfg = AccessibilityConfig {
            text_scale: TextScale::XLarge,
            reduce_motion: true,
            reduce_flashes: false,
            background_motion: false,
        };
        let policy = AccessibilityPolicy::from(&cfg);
        assert_eq!(policy.text_multiplier(), 1.5);
        assert_eq!(policy.screen_transition_ms(), 120);
        assert_eq!(policy.motion_decision(), MotionDecision::Reduced);
        assert_eq!(policy.flash_decision(), FlashDecision::Full);
        assert!(!policy.background_motion());
    }

    #[test]
    fn default_policy_preserves_existing_effects() {
        let policy = AccessibilityPolicy::default();
        assert_eq!(policy.text_multiplier(), 1.0);
        assert_eq!(policy.screen_transition_ms(), 300);
        assert_eq!(policy.motion_decision(), MotionDecision::Full);
        assert_eq!(policy.flash_decision(), FlashDecision::Full);
        assert!(policy.background_motion());
    }

    #[test]
    fn reduced_effects_keep_feedback_but_remove_oscillation() {
        let policy = AccessibilityPolicy::from(&AccessibilityConfig {
            reduce_motion: true,
            reduce_flashes: true,
            background_motion: false,
            ..Default::default()
        });
        assert_eq!(
            entrance_effect(policy),
            EntranceEffect::OpacityOnly { duration_ms: 120 }
        );
        assert_eq!(
            hit_effect(policy),
            HitEffect::StableOutline { duration_ms: 120 }
        );
        assert_eq!(danger_effect(policy), DangerEffect::ConstantBorder);
    }
}
