//! Lane flush alpha flash (70ms @ 60fps ≈ 4 frames).

use bevy::prelude::*;

use crate::theme::Theme;

pub const FLUSH_FRAMES: u32 = 4;

#[derive(Component, Clone, Copy, Default)]
pub struct LaneFlushWidget {
    pub frames_remaining: u32,
}

impl LaneFlushWidget {
    pub fn trigger(&mut self) {
        self.frames_remaining = FLUSH_FRAMES;
    }

    pub fn tick(&mut self) {
        if self.frames_remaining > 0 {
            self.frames_remaining -= 1;
        }
    }

    pub fn alpha(&self) -> f32 {
        if self.frames_remaining == 0 {
            0.0
        } else {
            self.frames_remaining as f32 / FLUSH_FRAMES as f32
        }
    }

    pub fn is_active(&self) -> bool {
        self.frames_remaining > 0
    }
}

// Orthogonal display knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn spawn_lane_flush_strip(
    commands: &mut Commands,
    parent: Entity,
    lane_count: usize,
    lane_width: f32,
    left_x: f32,
    top_y: f32,
    height: f32,
    theme: &Theme,
) -> Vec<Entity> {
    let mut entities = Vec::with_capacity(lane_count);
    for i in 0..lane_count {
        let e = commands
            .spawn((
                LaneFlushWidget::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left_x + i as f32 * lane_width),
                    top: Val::Px(top_y),
                    width: Val::Px(lane_width - 4.0),
                    height: Val::Px(height),
                    ..default()
                },
                BackgroundColor(Color::srgba(
                    theme.accent.to_srgba().red,
                    theme.accent.to_srgba().green,
                    theme.accent.to_srgba().blue,
                    0.0,
                )),
                Visibility::Hidden,
            ))
            .id();
        commands.entity(parent).add_child(e);
        entities.push(e);
    }
    entities
}

pub fn tick_lane_flushes(
    theme: &Theme,
    mut q: Query<(&mut LaneFlushWidget, &mut BackgroundColor, &mut Visibility)>,
) {
    let accent = theme.accent.to_srgba();
    for (mut flush, mut bg, mut vis) in &mut q {
        flush.tick();
        let a = flush.alpha();
        if flush.is_active() {
            *vis = Visibility::Visible;
            bg.0 = Color::srgba(accent.red, accent.green, accent.blue, a * 0.6);
        } else {
            *vis = Visibility::Hidden;
            bg.0 = Color::srgba(accent.red, accent.green, accent.blue, 0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_decays() {
        let mut f = LaneFlushWidget::default();
        f.trigger();
        assert!(f.alpha() > 0.0);
        for _ in 0..FLUSH_FRAMES {
            f.tick();
        }
        assert!((f.alpha() - 0.0).abs() < 0.001);
    }
}
