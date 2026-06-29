//! Combo display with bounce on increment.

use bevy::prelude::*;

use crate::easing::EaseFunction;
use crate::tween::ScalarTween;

#[derive(Component)]
pub struct ComboDisplay {
    pub last_combo: u32,
    bounce: ScalarTween,
}

impl Default for ComboDisplay {
    fn default() -> Self {
        Self {
            last_combo: 0,
            bounce: ScalarTween::new(1.0, 1.0, 200.0, EaseFunction::OutQuint),
        }
    }
}

impl ComboDisplay {
    pub fn set_combo(&mut self, combo: u32) {
        if combo > self.last_combo && combo > 0 {
            self.bounce.reset(1.3, 1.0, 200.0, EaseFunction::OutQuint);
        }
        self.last_combo = combo;
    }

    pub fn tick(&mut self, delta_ms: f32) {
        if !self.bounce.finished {
            self.bounce.tick(delta_ms);
        }
    }

    pub fn scale(&self) -> f32 {
        self.bounce.value()
    }
}

pub fn sync_combo_display(
    combo_value: u32,
    time: Res<Time>,
    mut q: Query<(&mut ComboDisplay, &mut Text)>,
) {
    let delta = time.delta_secs() * 1000.0;
    for (mut display, mut text) in &mut q {
        display.set_combo(combo_value);
        display.tick(delta);
        *text = Text::new(format!("{}x", display.last_combo));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_bounce_on_increase() {
        let mut d = ComboDisplay::default();
        d.set_combo(10);
        assert!(!d.bounce.finished);
    }
}
