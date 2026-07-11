//! Reference-resolution anchor for HUD nodes (scaled by `PlayfieldLayout::scale`).

use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug)]
pub struct HudRefRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl HudRefRect {
    pub fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn apply(&self, scale: f32, origin: Vec2, node: &mut Node) {
        node.left = Val::Px(self.left * scale + origin.x);
        node.top = Val::Px(self.top * scale + origin.y);
        if self.width > 0.0 {
            node.width = Val::Px(self.width * scale);
        }
        if self.height > 0.0 {
            node.height = Val::Px(self.height * scale);
        }
    }
}

pub fn scaled_font(scale: f32, ref_size: f32) -> TextFont {
    crate::theme::Theme::font(ref_size * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_zero_origin_is_scale_only() {
        let rect = HudRefRect::new(40.0, 12.0, 100.0, 20.0);
        let mut node = Node::default();
        rect.apply(2.0, Vec2::ZERO, &mut node);
        assert_eq!(node.left, Val::Px(80.0));
        assert_eq!(node.top, Val::Px(24.0));
        assert_eq!(node.width, Val::Px(200.0));
        assert_eq!(node.height, Val::Px(40.0));
    }

    #[test]
    fn apply_offsets_left_top_by_origin() {
        let rect = HudRefRect::new(40.0, 12.0, 100.0, 20.0);
        let mut node = Node::default();
        rect.apply(1.0, Vec2::new(220.0, 10.0), &mut node);
        assert_eq!(node.left, Val::Px(40.0 + 220.0));
        assert_eq!(node.top, Val::Px(12.0 + 10.0));
        // width/height stay scale-only (no origin).
        assert_eq!(node.width, Val::Px(100.0));
        assert_eq!(node.height, Val::Px(20.0));
    }
}
