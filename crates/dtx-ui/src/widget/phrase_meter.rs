//! Vertical phrase-density meter at the playfield's right edge.
//!
//! 64 horizontal blocks stacked top→bottom; block width encodes drum-chip
//! density for that slice of the chart (top = chart end, bottom = chart start,
//! matching `gameplay-drums` phrase math). Widths + played-portion tint are
//! driven by the HUD sync system.

use crate::theme::Theme;
use crate::widget::hud_ref::HudRefRect;
use bevy::prelude::*;

/// Block count (mirrors `phrase::PHRASE_SECTION_COUNT`).
pub const PHRASE_BLOCKS: usize = 64;
/// Meter top edge (ref px) — below the NOW PLAYING card.
pub const PHRASE_BAR_Y: f32 = 96.0;
/// Meter height (ref px).
pub const PHRASE_BAR_H: f32 = 490.0;
/// Meter width (ref px).
pub const PHRASE_BAR_W: f32 = 30.0;

#[derive(Component)]
pub struct PhraseMeterRoot;

#[derive(Component)]
pub struct PhraseSection {
    /// Section index, 0 = top (chart end) … 63 = bottom (chart start).
    pub index: usize,
}

#[derive(Component)]
pub struct PhrasePlayhead;

pub fn spawn_phrase_meter(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    bar_ref_x: f32,
) {
    let bar_y = PHRASE_BAR_Y;
    let bar_w = PHRASE_BAR_W;
    let bar_h = PHRASE_BAR_H;
    let slice_h = bar_h / PHRASE_BLOCKS as f32;
    let block_w = bar_w / 10.0;

    commands.entity(parent).with_children(|p| {
        p.spawn((
            PhraseMeterRoot,
            HudRefRect::new(bar_ref_x - 2.0, bar_y - 2.0, bar_w + 4.0, bar_h + 4.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((bar_ref_x - 2.0) * scale),
                top: Val::Px((bar_y - 2.0) * scale),
                width: Val::Px((bar_w + 4.0) * scale),
                height: Val::Px((bar_h + 4.0) * scale),
                border: UiRect::all(Val::Px(1.0 * scale)),
                ..default()
            },
            BackgroundColor(theme.stage_panel_bg),
            BorderColor::all(theme.stage_panel_border),
        ))
        .with_children(|bar| {
            for i in 0..PHRASE_BLOCKS {
                let y = i as f32 * slice_h;
                bar.spawn((
                    PhraseSection { index: i },
                    HudRefRect::new(2.0, y, block_w, (slice_h - 0.5).max(0.5)),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(2.0 * scale),
                        top: Val::Px(y * scale),
                        width: Val::Px(block_w * scale),
                        height: Val::Px((slice_h - 0.5).max(0.5) * scale),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.36, 0.44)),
                ));
            }

            bar.spawn((
                PhrasePlayhead,
                HudRefRect::new(0.0, bar_h - 1.0, bar_w + 4.0, 2.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px((bar_h - 1.0) * scale),
                    width: Val::Px((bar_w + 4.0) * scale),
                    height: Val::Px(2.0 * scale),
                    ..default()
                },
                BackgroundColor(theme.accent),
            ));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bar_in_bounds() {
        // Meter must fit within the 1280-wide reference frame from its anchor.
        let bar_ref_x = 934.0;
        assert!(bar_ref_x + PHRASE_BAR_W + 4.0 <= 1280.0);
    }
}
