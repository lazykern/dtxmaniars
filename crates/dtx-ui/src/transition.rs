//! Fullscreen fade overlay for screen transitions (ADR-0014).

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::theme::SCREEN_TRANSITION_MS;
use crate::tween::ScalarTween;

/// Marker on the fullscreen fade overlay entity.
#[derive(Component)]
pub struct TransitionOverlay;

/// Fade phase for screen transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FadePhase {
    #[default]
    Idle,
    FadeOut,
    FadeIn,
}

/// Resource driving the fade overlay alpha.
#[derive(Resource)]
pub struct ScreenFade {
    pub phase: FadePhase,
    pub alpha: f32,
    pub overlay: Option<Entity>,
    tween: ScalarTween,
    duration_ms: f32,
}

impl Default for ScreenFade {
    fn default() -> Self {
        Self {
            phase: FadePhase::Idle,
            alpha: 0.0,
            overlay: None,
            tween: ScalarTween::new(0.0, 0.0, SCREEN_TRANSITION_MS, EaseFunction::OutQuint),
            duration_ms: SCREEN_TRANSITION_MS,
        }
    }
}

impl ScreenFade {
    pub fn set_duration_ms(&mut self, duration_ms: f32) {
        self.duration_ms = duration_ms.max(1.0);
    }
    pub fn is_busy(&self) -> bool {
        self.phase != FadePhase::Idle
    }

    pub fn start_fade_out(&mut self) {
        self.phase = FadePhase::FadeOut;
        self.tween
            .reset(self.alpha, 1.0, self.duration_ms, EaseFunction::OutQuint);
    }

    pub fn start_fade_in(&mut self) {
        self.phase = FadePhase::FadeIn;
        self.tween
            .reset(self.alpha, 0.0, self.duration_ms, EaseFunction::OutQuint);
    }

    pub fn finish(&mut self) {
        self.phase = FadePhase::Idle;
        self.alpha = 0.0;
        self.tween.finished = true;
    }

    pub fn tick(&mut self, delta_ms: f32) -> bool {
        if self.phase == FadePhase::Idle {
            return false;
        }
        self.tween.tick(delta_ms);
        self.alpha = self.tween.value();
        self.tween.finished
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ScreenFade>()
        .add_systems(Startup, spawn_overlay)
        .add_systems(Update, (tick_overlay, sync_overlay_alpha));
}

fn spawn_overlay(mut commands: Commands, mut fade: ResMut<ScreenFade>) {
    let entity = commands
        .spawn((
            TransitionOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                ..default()
            },
            GlobalZIndex(9999),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            Visibility::Hidden,
        ))
        .id();
    fade.overlay = Some(entity);
}

fn tick_overlay(time: Res<Time>, mut fade: ResMut<ScreenFade>) {
    let _ = fade.tick(time.delta_secs() * 1000.0);
}

fn sync_overlay_alpha(
    fade: Res<ScreenFade>,
    mut q: Query<(&mut BackgroundColor, &mut Visibility), With<TransitionOverlay>>,
) {
    let Some(entity) = fade.overlay else {
        return;
    };
    let Ok((mut bg, mut vis)) = q.get_mut(entity) else {
        return;
    };
    if fade.phase == FadePhase::Idle && fade.alpha <= 0.001 {
        *vis = Visibility::Hidden;
        bg.0 = Color::srgba(0.0, 0.0, 0.0, 0.0);
    } else {
        *vis = Visibility::Visible;
        bg.0 = Color::srgba(0.0, 0.0, 0.0, fade.alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_out_reaches_opaque() {
        let mut fade = ScreenFade::default();
        fade.start_fade_out();
        while !fade.tick(16.0) {}
        assert!((fade.alpha - 1.0).abs() < 0.05);
        assert_eq!(fade.phase, FadePhase::FadeOut);
    }

    #[test]
    fn fade_in_reaches_transparent() {
        let mut fade = ScreenFade {
            alpha: 1.0,
            ..Default::default()
        };
        fade.start_fade_in();
        while !fade.tick(16.0) {}
        assert!(fade.alpha < 0.05);
    }

    #[test]
    fn idle_not_busy() {
        let fade = ScreenFade::default();
        assert!(!fade.is_busy());
    }
}
