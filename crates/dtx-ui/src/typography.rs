use bevy::prelude::Resource;
use bevy::text::TextFont;
use dtx_config::TextScale;

use crate::{AccessibilityPolicy, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyRole {
    Display,
    Title,
    Heading,
    Body,
    Label,
    Hint,
    Hud,
}

#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Typography;

impl Typography {
    pub const fn base_px(self, role: TypographyRole) -> f32 {
        match role {
            TypographyRole::Display => 48.0,
            TypographyRole::Title => 36.0,
            TypographyRole::Heading => 24.0,
            TypographyRole::Body => 16.0,
            TypographyRole::Label => 16.0,
            TypographyRole::Hint => 14.0,
            TypographyRole::Hud => 32.0,
        }
    }

    pub fn px(self, role: TypographyRole, scale: TextScale) -> f32 {
        (self.base_px(role) * scale.multiplier()).max(14.0)
    }

    pub fn font(self, role: TypographyRole, policy: AccessibilityPolicy) -> TextFont {
        Theme::font(self.px(role, policy.text_scale()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacingRole {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl SpacingRole {
    pub const fn px(self) -> f32 {
        match self {
            Self::Xs => 4.0,
            Self::Sm => 8.0,
            Self::Md => 16.0,
            Self::Lg => 24.0,
            Self::Xl => 32.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMarker {
    Focus,
    Selected,
    Error,
    Destructive,
    Success,
    Disabled,
}

impl StateMarker {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Focus => ">",
            Self::Selected => "✓",
            Self::Error => "!",
            Self::Destructive => "DELETE",
            Self::Success => "OK",
            Self::Disabled => "—",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionTone {
    Focus,
    Selected,
    Error,
    Destructive,
    Success,
    Disabled,
}

impl InteractionTone {
    pub const ALL: [Self; 6] = [
        Self::Focus,
        Self::Selected,
        Self::Error,
        Self::Destructive,
        Self::Success,
        Self::Disabled,
    ];

    pub const fn marker(self) -> StateMarker {
        match self {
            Self::Focus => StateMarker::Focus,
            Self::Selected => StateMarker::Selected,
            Self::Error => StateMarker::Error,
            Self::Destructive => StateMarker::Destructive,
            Self::Success => StateMarker::Success,
            Self::Disabled => StateMarker::Disabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_text_scales_and_never_drops_below_minimum() {
        let typography = Typography::default();
        assert_eq!(
            typography.px(TypographyRole::Body, dtx_config::TextScale::Large),
            20.0
        );
        assert!(typography.px(TypographyRole::Hint, dtx_config::TextScale::Standard) >= 14.0);
        assert_eq!(
            typography.px(TypographyRole::Hud, dtx_config::TextScale::XLarge),
            48.0
        );
    }

    #[test]
    fn interaction_tones_always_have_shape_or_text_markers() {
        for tone in InteractionTone::ALL {
            assert!(!tone.marker().label().is_empty());
        }
    }
}
