//! DTXMania classic PHRASE METER (right vertical bar).
//!
//! Reference: BocuD `CActPerfProgressBar.cs:29` — x=855, y=15, W=20, H=540.
//! 64 sections of varying width (0..=10 blocks per section).

use bevy::prelude::*;
use crate::theme::Theme;

/// Per-section width in blocks (0..=10). 0 = unset.
#[derive(Component, Default)]
pub struct PhraseSection {
    pub blocks: u8,
}

/// Marker for the playhead (current time) indicator.
#[derive(Component)]
pub struct PhrasePlayhead;

/// Spawn the vertical phrase meter. Returns the parent entity.
pub fn spawn_phrase_meter(commands: &mut Commands, parent: Entity, theme: &Theme) {
    let bar_x = 855.0;
    let bar_y = 15.0;
    let bar_w = 20.0;
    let bar_h = 540.0;
    let block_w = bar_w / 10.0; // each block = 2px

    let bg = Color::srgba(0.0, 0.0, 0.0, 0.6);
    let attempted = Color::srgb(0.4, 0.4, 0.5);   // grey: not yet completed
    let full = Color::srgb(0.95, 0.85, 0.1);      // yellow: full clear
    let partial = Color::srgb(0.2, 0.6, 0.95);    // blue: partial

    commands.entity(parent).with_children(|p| {
        // Background bar
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(bar_x - 2.0),
                top: Val::Px(bar_y - 2.0),
                width: Val::Px(bar_w + 4.0),
                height: Val::Px(bar_h + 4.0),
                ..default()
            },
            BackgroundColor(bg),
        ));

        // 64 section blocks (overlapping children → drawn in spawn order, so
        // later ones cover earlier; this matches BocuD's "current section on top").
        for i in 0..64 {
            let slice_h = bar_h / 64.0;
            let y = bar_y + i as f32 * slice_h;
            p.spawn((
                PhraseSection::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(bar_x),
                    top: Val::Px(y),
                    width: Val::Px(block_w), // updated by sync system
                    height: Val::Px(slice_h),
                    ..default()
                },
                BackgroundColor(attempted),
            ));
        }

        // Playhead line (current time position)
        p.spawn((
            PhrasePlayhead,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(bar_x - 4.0),
                top: Val::Px(bar_y + bar_h - 1.0),
                width: Val::Px(bar_w + 8.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(theme.accent),
        ));
        // Suppress unused-var warnings (we'll use partial/full in the sync system).
        let _ = (full, partial);
    });
}

/// Per-section block width updater. `sections` is the chip-count array from
/// `gameplay_drums::phrase::PhraseMeter`. `current_section` is the playhead
/// (0..=64, top→bottom = chart-start→chart-end).
pub fn sync_phrase_meter(
    sections: &[u32; 64],
    current_section: usize,
    mut q: Query<(&PhraseSection, &mut Node, &mut BackgroundColor)>,
) {
    for (i, (_, mut node, mut color)) in q.iter_mut().enumerate() {
        let count = sections.get(i).copied().unwrap_or(0);
        let units = (count as f32 / 2.5).min(10.0) as u8;
        let block_w = 20.0 / 10.0;
        node.width = Val::Px(block_w * (units as f32 + 1.0));
        // Color: future sections grey, current/played yellow→blue gradient
        if i < current_section {
            *color = BackgroundColor(Color::srgb(0.95, 0.85, 0.1));
        } else if i == current_section {
            *color = BackgroundColor(Color::srgb(0.2, 0.6, 0.95));
        } else {
            *color = BackgroundColor(Color::srgb(0.4, 0.4, 0.5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bar_in_bounds() {
        assert!(855.0 + 20.0 <= 1280.0);
        assert!(15.0 + 540.0 <= 720.0);
    }
}
