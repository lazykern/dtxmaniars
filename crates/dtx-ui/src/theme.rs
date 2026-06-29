//! Hardcoded dark theme tokens (ADR-0014 v1).

use bevy::prelude::*;
use bevy::text::FontSource;

/// Design resolution baseline.
pub const REF_WIDTH: f32 = 1280.0;
pub const REF_HEIGHT: f32 = 720.0;

/// Screen transition duration (osu-style OutQuint).
pub const SCREEN_TRANSITION_MS: f32 = 300.0;

/// Theme color + typography tokens.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg_top: Color,
    pub bg_bottom: Color,
    pub panel_bg: Color,
    pub accent: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub judgment_perfect: Color,
    pub judgment_great: Color,
    pub judgment_good: Color,
    pub judgment_miss: Color,
    pub gauge_fill: Color,
    pub gauge_track: Color,
    pub selection_highlight: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg_top: Color::srgb(0.102, 0.102, 0.180),    // #1a1a2e
            bg_bottom: Color::srgb(0.086, 0.129, 0.243), // #16213e
            panel_bg: Color::srgba(1.0, 1.0, 1.0, 0.04),
            accent: Color::srgb(0.0, 0.831, 0.667), // cyan accent
            text_primary: Color::srgb(1.0, 1.0, 1.0),
            text_secondary: Color::srgba(1.0, 1.0, 1.0, 0.5),
            judgment_perfect: Color::srgb(1.0, 0.843, 0.0), // gold
            judgment_great: Color::srgb(0.298, 0.851, 0.392),
            judgment_good: Color::srgb(0.392, 0.584, 0.929),
            judgment_miss: Color::srgb(0.937, 0.267, 0.267),
            gauge_fill: Color::srgb(0.298, 0.851, 0.392),
            gauge_track: Color::srgba(0.0, 0.0, 0.0, 0.5),
            selection_highlight: Color::srgba(0.0, 0.831, 0.667, 0.15),
        }
    }
}

/// Bevy resource wrapping the active theme.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ThemeResource(pub Theme);

impl Default for ThemeResource {
    fn default() -> Self {
        Self(Theme::default())
    }
}

impl Theme {
    pub fn judgment_color(&self, label: &str) -> Color {
        match label.to_uppercase().as_str() {
            "PERFECT" | "PG" => self.judgment_perfect,
            "GREAT" | "GR" => self.judgment_great,
            "GOOD" | "GO" => self.judgment_good,
            "MISS" => self.judgment_miss,
            _ => self.text_primary,
        }
    }

    pub fn gradient_background_bundle(&self) -> (Node, BackgroundColor) {
        (
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(self.bg_bottom),
        )
    }

    pub fn font(size: f32) -> TextFont {
        TextFont {
            font: FontSource::SansSerif,
            font_size: FontSize::Px(size),
            ..default()
        }
    }

    pub fn title_font() -> TextFont {
        Self::font(48.0)
    }

    pub fn body_font() -> TextFont {
        Self::font(16.0)
    }

    pub fn hud_font() -> TextFont {
        Self::font(32.0)
    }

    pub fn label_font() -> TextFont {
        Self::font(18.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_default_has_accent() {
        let t = Theme::default();
        assert!(t.accent.to_srgba().green > 0.5);
    }

    #[test]
    fn judgment_colors_distinct() {
        let t = Theme::default();
        assert_ne!(t.judgment_perfect, t.judgment_miss);
    }
}
