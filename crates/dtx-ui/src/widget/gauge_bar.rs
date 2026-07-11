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
}
