//! Judgment popup — scale up + fade out.

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::theme::Theme;
use crate::tween::ScalarTween;

const POPUP_MS: f32 = 300.0;

#[derive(Component)]
pub struct JudgmentPopup {
    pub label: String,
    elapsed_ms: f32,
    alpha_tween: ScalarTween,
    active: bool,
}

impl Default for JudgmentPopup {
    fn default() -> Self {
        Self {
            label: String::new(),
            elapsed_ms: 0.0,
            alpha_tween: ScalarTween::new(1.0, 0.0, POPUP_MS, EaseFunction::OutQuint),
            active: false,
        }
    }
}

impl JudgmentPopup {
    pub fn trigger(&mut self, label: impl Into<String>, theme: &Theme) -> Color {
        self.label = label.into();
        self.elapsed_ms = 0.0;
        self.alpha_tween
            .reset(1.0, 0.0, POPUP_MS, EaseFunction::OutQuint);
        self.active = true;
        theme.judgment_color(&self.label)
    }

    pub fn tick(&mut self, delta_ms: f32) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }
        self.elapsed_ms += delta_ms;
        self.alpha_tween.tick(delta_ms);
        let alpha = self.alpha_tween.value();
        let scale = 1.0 + 0.2 * (1.0 - alpha);
        if self.alpha_tween.finished {
            self.active = false;
        }
        (alpha, scale)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

pub fn spawn_judgment_popup(commands: &mut Commands, parent: Entity, theme: &Theme) -> Entity {
    let child = commands
        .spawn((
            JudgmentPopup::default(),
            Node {
                position_type: PositionType::Absolute,
                // Strip center at ref res: (295 + 558/2) / 1280 ≈ 44.8%.
                left: Val::Percent(44.8),
                top: Val::Px(200.0),
                ..default()
            },
            Text::new(""),
            Theme::font(56.0),
            TextColor(theme.judgment_perfect),
            UiTransform::default(),
            Visibility::Hidden,
        ))
        .id();
    commands.entity(parent).add_child(child);
    child
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_fades_out() {
        let mut p = JudgmentPopup::default();
        let theme = Theme::default();
        p.trigger("PERFECT", &theme);
        for _ in 0..30 {
            p.tick(16.0);
        }
        assert!(!p.is_active() || p.alpha_tween.value() < 0.5);
    }
}
