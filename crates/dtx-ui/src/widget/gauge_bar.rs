//! Smooth-tweened gauge bar.

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::tween::ScalarTween;

#[derive(Component)]
pub struct GaugeBarWidget {
    pub pct: f32,
    tween: ScalarTween,
    pub track_width: f32,
}

impl Default for GaugeBarWidget {
    fn default() -> Self {
        Self {
            pct: 20.0,
            tween: ScalarTween::new(20.0, 20.0, 150.0, EaseFunction::OutQuad),
            track_width: 280.0,
        }
    }
}

impl GaugeBarWidget {
    pub fn set_pct(&mut self, pct: f32) {
        let clamped = pct.clamp(0.0, 100.0);
        if (clamped - self.pct).abs() < 0.01 {
            return;
        }
        self.tween
            .reset(self.pct, clamped, 150.0, EaseFunction::OutQuad);
    }

    pub fn tick(&mut self, delta_ms: f32) {
        if !self.tween.finished {
            self.tween.tick(delta_ms);
            self.pct = self.tween.value();
        }
    }

    pub fn fill_width(&self) -> f32 {
        self.track_width * (self.pct / 100.0)
    }
}

#[derive(Component)]
pub struct GaugeFill;

/// White notch at the track's left edge marking "empty = failed".
#[derive(Component)]
pub struct GaugeThresholdTick;

/// Horizontal stage gauge across the top of the playfield strip, just under
/// the 60 ref-px frame-chrome speaker bar. `ref_x`/`ref_w` are ref-px strip
/// bounds; `s` is layout scale.
pub fn spawn_stage_gauge(
    commands: &mut Commands,
    parent: Entity,
    theme: &crate::theme::Theme,
    s: f32,
    ref_x: f32,
    ref_w: f32,
) -> Entity {
    let track = commands
        .spawn((
            GaugeBarWidget {
                track_width: ref_w * s,
                ..Default::default()
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * s),
                top: Val::Px(64.0 * s),
                width: Val::Px(ref_w * s),
                height: Val::Px(10.0 * s),
                ..default()
            },
            BackgroundColor(theme.gauge_track),
        ))
        .id();
    let fill = commands
        .spawn((
            GaugeFill,
            Node {
                width: Val::Px(0.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(theme.gauge_fill),
        ))
        .id();
    commands.entity(track).add_child(fill);
    let tick = commands
        .spawn((
            GaugeThresholdTick,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(-2.0 * s),
                width: Val::Px(2.0 * s),
                height: Val::Px(14.0 * s),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        ))
        .id();
    commands.entity(track).add_child(tick);
    commands.entity(parent).add_child(track);
    track
}

pub fn sync_gauge_bar(
    pct: f32,
    time: Res<Time>,
    mut bars: Query<&mut GaugeBarWidget>,
    mut fills: Query<&mut Node, With<GaugeFill>>,
) {
    let delta = time.delta_secs() * 1000.0;
    for mut bar in &mut bars {
        bar.set_pct(pct);
        bar.tick(delta);
        for mut fill in &mut fills {
            fill.width = Val::Px(bar.fill_width());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_width_scales() {
        let g = GaugeBarWidget {
            pct: 50.0,
            ..Default::default()
        };
        assert!((g.fill_width() - 140.0).abs() < 0.1);
    }

    #[test]
    fn stage_gauge_track_width_follows_ref_width() {
        let g = GaugeBarWidget {
            track_width: 558.0,
            pct: 50.0,
            ..Default::default()
        };
        assert!((g.fill_width() - 279.0).abs() < 0.1);
    }
}
