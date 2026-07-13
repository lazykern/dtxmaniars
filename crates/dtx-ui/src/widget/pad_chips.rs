//! DTXMania classic PAD CHIPS (5 colored pads at bottom of left panel).
//!
//! Reference: BocuD `CActPerfDrumsPad.cs:11-200`. Skin sprite 7_pads.png.
//! No sprite yet → flat colored quads (BocuD default color theme).
//!
//! Lanes shown: LC, HH, SD, BD, RD (the 5 "main" pads; the 10-lane field is
//! drawn separately by the scroll system).

use crate::theme::Theme;
use bevy::prelude::*;

use crate::accessibility::FlashDecision;

/// Duration of one pad-hit acknowledgement.
pub const PAD_FLASH_DURATION_MS: u32 = 120;

/// Render intent derived from one pad's feedback state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PadFlashPresentation {
    /// No active hit acknowledgement.
    #[default]
    Idle,
    /// Full-strength brightness feedback.
    FullFlash,
    /// Stable outline used when flashes are reduced.
    StableOutline,
}

/// Pure, frame-rate-independent pad feedback reducer.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PadFlashState {
    remaining_ms: u32,
    reduced: bool,
}

impl PadFlashState {
    /// Start or restart feedback under the active accessibility policy.
    pub fn trigger(&mut self, decision: FlashDecision) {
        self.remaining_ms = PAD_FLASH_DURATION_MS;
        self.reduced = decision == FlashDecision::Reduced;
    }

    /// Advance feedback by elapsed wall time.
    pub fn tick(&mut self, elapsed_ms: u32) {
        self.remaining_ms = self.remaining_ms.saturating_sub(elapsed_ms);
    }

    /// Milliseconds left in the current acknowledgement.
    pub const fn remaining_ms(self) -> u32 {
        self.remaining_ms
    }

    /// Presentation the renderer should use now.
    pub const fn presentation(self) -> PadFlashPresentation {
        if self.remaining_ms == 0 {
            PadFlashPresentation::Idle
        } else if self.reduced {
            PadFlashPresentation::StableOutline
        } else {
            PadFlashPresentation::FullFlash
        }
    }
}

/// Marker for one pad chip. `lane_index` 0..=4.
#[derive(Component)]
pub struct PadChip {
    pub lane_index: u8,
}

/// Spawn 5 pad chips at the bottom of the left status panel.
///
/// BocuD positions are config-driven; we use approximate positions matching
/// the screenshot 1 layout (5 chips centered under the left panel).
pub fn spawn_pad_chips(commands: &mut Commands, parent: Entity, _theme: &Theme) {
    // BocuD default pad colors (from skin config):
    let pad_colors = [
        Color::srgb(0.20, 0.50, 0.95), // LC = blue
        Color::srgb(0.95, 0.85, 0.10), // HH = yellow
        Color::srgb(0.95, 0.20, 0.20), // SD = red
        Color::srgb(0.20, 0.50, 0.95), // BD = blue
        Color::srgb(0.10, 0.85, 0.85), // RD = cyan
    ];
    let pad_labels = ["LC", "HH", "SD", "BD", "RD"];
    let pad_w = 44.0;
    let pad_h = 60.0;
    let pad_x_start = 18.0;

    commands.entity(parent).with_children(|p| {
        for (i, (color, label)) in pad_colors.iter().zip(pad_labels.iter()).enumerate() {
            let x = pad_x_start + i as f32 * (pad_w + 4.0);
            p.spawn((
                PadChip {
                    lane_index: i as u8,
                },
                PadFlashState::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    bottom: Val::Px(20.0),
                    width: Val::Px(pad_w),
                    height: Val::Px(pad_h),
                    ..default()
                },
                BackgroundColor(*color),
            ))
            .with_children(|pad| {
                pad.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(pad_h - 18.0),
                        width: Val::Px(pad_w),
                        height: Val::Px(18.0),
                        ..default()
                    },
                    Text::new(*label),
                    crate::theme::Theme::font(14.0),
                    TextColor(Color::WHITE),
                ));
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessibility::FlashDecision;

    #[test]
    fn pad_flash_triggers_for_exactly_120_ms() {
        let mut state = PadFlashState::default();

        state.trigger(FlashDecision::Full);
        assert_eq!(state.presentation(), PadFlashPresentation::FullFlash);
        assert_eq!(state.remaining_ms(), 120);

        state.tick(119);
        assert_eq!(state.presentation(), PadFlashPresentation::FullFlash);
        assert_eq!(state.remaining_ms(), 1);

        state.tick(1);
        assert_eq!(state.presentation(), PadFlashPresentation::Idle);
        assert_eq!(state.remaining_ms(), 0);
    }

    #[test]
    fn reduced_flash_uses_stable_outline_for_the_same_decay() {
        let mut state = PadFlashState::default();

        state.trigger(FlashDecision::Reduced);
        assert_eq!(state.presentation(), PadFlashPresentation::StableOutline);
        assert_eq!(state.remaining_ms(), 120);

        state.tick(120);
        assert_eq!(state.presentation(), PadFlashPresentation::Idle);
    }
}
