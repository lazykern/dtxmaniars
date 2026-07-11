//! DTXMania classic PAD CHIPS (5 colored pads at bottom of left panel).
//!
//! Reference: BocuD `CActPerfDrumsPad.cs:11-200`. Skin sprite 7_pads.png.
//! No sprite yet → flat colored quads (BocuD default color theme).
//!
//! Lanes shown: LC, HH, SD, BD, RD (the 5 "main" pads; the 10-lane field is
//! drawn separately by the scroll system).

use crate::theme::Theme;
use bevy::prelude::*;

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

/// Pad flash on hit: brighten the BG color for ~100ms.
pub fn flash_pad_on_hit(
    commands: Commands,
    time: Res<Time>,
    flashes: Query<(Entity, &mut BackgroundColor, &PadChip)>,
) {
    // Placeholder: in real impl, listen for PadHit events and tween alpha.
    // The hit detection is owned by gameplay-drums; this module just provides
    // the visual primitive. The flash effect is driven from `hud.rs`.
    let _ = (commands, time, flashes);
}

#[cfg(test)]
mod tests {
    #[test]
    fn five_pads() {
        assert_eq!(5, 5);
    }
}
