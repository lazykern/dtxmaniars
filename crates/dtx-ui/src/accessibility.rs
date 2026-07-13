use bevy::prelude::Resource;
use dtx_config::AccessibilityConfig;

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

/// Runtime accessibility decisions derived once from persisted configuration.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct AccessibilityPolicy {
    text_multiplier: f32,
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
            text_multiplier: config.text_scale.multiplier(),
            reduce_motion: config.reduce_motion,
            reduce_flashes: config.reduce_flashes,
            background_motion: config.background_motion,
        }
    }
}

impl AccessibilityPolicy {
    pub const fn text_multiplier(&self) -> f32 {
        self.text_multiplier
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
}
