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

    pub fn apply(&self, scale: f32, node: &mut Node) {
        node.left = Val::Px(self.left * scale);
        node.top = Val::Px(self.top * scale);
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
