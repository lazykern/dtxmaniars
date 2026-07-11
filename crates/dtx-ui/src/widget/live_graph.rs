//! Live accuracy graph — 128 vertical bars vs rank threshold lines.

use crate::theme::Theme;
use crate::widget::hud_ref::HudRefRect;
use bevy::prelude::*;

pub const GRAPH_SLOTS: usize = 128;

/// Rank threshold lines drawn across the graph (percent, high→low).
pub const RANK_THRESHOLDS: [(f32, &str); 3] = [(95.0, "S"), (85.0, "A"), (70.0, "B")];

#[derive(Component)]
pub struct LiveGraphRoot;

#[derive(Component)]
pub struct LiveGraphBar {
    pub slot: usize,
}

/// Slot index for a song position (`pos_ms` of `total_ms`), clamped to 0..127.
pub fn slot_for_pos(pos_ms: i64, total_ms: i64) -> usize {
    if total_ms <= 0 {
        return 0;
    }
    let frac = (pos_ms as f64 / total_ms as f64).clamp(0.0, 1.0);
    ((frac * GRAPH_SLOTS as f64) as usize).min(GRAPH_SLOTS - 1)
}

/// Bar height in ref px for an accuracy percent over a graph of `bar_area_h`.
pub fn bar_height(accuracy_pct: f32, bar_area_h: f32) -> f32 {
    (accuracy_pct.clamp(0.0, 100.0) / 100.0) * bar_area_h
}

/// Usable bar-drawing height inside a graph panel of ref height `ref_h`
/// (leaves a small bottom margin). Shared by spawn + the HUD sync system so
/// bar heights and threshold lines stay aligned.
pub fn bar_area_h(ref_h: f32) -> f32 {
    ref_h - 4.0
}

/// Spawn the graph panel: background plate, threshold lines with labels, and
/// `GRAPH_SLOTS` zero-height bars anchored to the panel bottom (grow upward).
// Orthogonal display knobs; a params struct would only relocate the list (clippy::too_many_arguments).
#[allow(clippy::too_many_arguments)]
pub fn spawn_live_graph(
    commands: &mut Commands,
    parent: Entity,
    theme: &Theme,
    scale: f32,
    ref_x: f32,
    ref_y: f32,
    ref_w: f32,
    ref_h: f32,
) {
    let bar_area_h = bar_area_h(ref_h);
    let bar_w = ref_w / GRAPH_SLOTS as f32;
    let bg = theme.stage_panel_bg;
    let bar_color = theme.accent;
    let line_color = Color::srgba(1.0, 0.85, 0.1, 0.4);

    commands.entity(parent).with_children(|p| {
        p.spawn((
            LiveGraphRoot,
            HudRefRect::new(ref_x, ref_y, ref_w, ref_h),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(ref_x * scale),
                top: Val::Px(ref_y * scale),
                width: Val::Px(ref_w * scale),
                height: Val::Px(ref_h * scale),
                border: UiRect::all(Val::Px(1.0 * scale)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(theme.stage_panel_border),
        ));

        for (pct, label) in RANK_THRESHOLDS {
            // Bars anchor at the panel bottom and grow up by
            // `pct/100 * bar_area_h`, so a threshold line must land on that same
            // bar top (bottom-anchored), not top-anchored — else it sits ~4px off.
            let line_y = ref_y + ref_h - (pct / 100.0) * bar_area_h;
            p.spawn((
                HudRefRect::new(ref_x, line_y, ref_w, 1.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(ref_x * scale),
                    top: Val::Px(line_y * scale),
                    width: Val::Px(ref_w * scale),
                    height: Val::Px(1.0 * scale),
                    ..default()
                },
                BackgroundColor(line_color),
            ));
            p.spawn((
                HudRefRect::new(ref_x + ref_w - 14.0, line_y - 6.0, 14.0, 12.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((ref_x + ref_w - 14.0) * scale),
                    top: Val::Px((line_y - 6.0) * scale),
                    width: Val::Px(14.0 * scale),
                    height: Val::Px(12.0 * scale),
                    ..default()
                },
                Text::new(label),
                Theme::font(10.0 * scale),
                TextColor(theme.text_secondary),
            ));
        }

        for slot in 0..GRAPH_SLOTS {
            let bx = ref_x + slot as f32 * bar_w;
            p.spawn((
                LiveGraphBar { slot },
                HudRefRect::new(bx, ref_y + ref_h, bar_w, 0.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(bx * scale),
                    top: Val::Px((ref_y + ref_h) * scale),
                    width: Val::Px(bar_w.max(1.0) * scale),
                    height: Val::Px(0.0),
                    ..default()
                },
                BackgroundColor(bar_color),
            ));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_zero_at_start() {
        assert_eq!(slot_for_pos(0, 10_000), 0);
    }

    #[test]
    fn slot_last_at_end() {
        assert_eq!(slot_for_pos(10_000, 10_000), GRAPH_SLOTS - 1);
    }

    #[test]
    fn slot_mid() {
        assert_eq!(slot_for_pos(5_000, 10_000), GRAPH_SLOTS / 2);
    }

    #[test]
    fn slot_guards_zero_total() {
        assert_eq!(slot_for_pos(1_000, 0), 0);
    }

    #[test]
    fn bar_full_at_100() {
        assert!((bar_height(100.0, 200.0) - 200.0).abs() < 0.01);
    }

    #[test]
    fn bar_half_at_50() {
        assert!((bar_height(50.0, 200.0) - 100.0).abs() < 0.01);
    }

    #[test]
    fn bar_clamps_out_of_range() {
        assert!((bar_height(150.0, 200.0) - 200.0).abs() < 0.01);
        assert!((bar_height(-10.0, 200.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn bar_area_leaves_margin() {
        assert!((bar_area_h(300.0) - 296.0).abs() < 0.01);
    }

    #[test]
    fn thresholds_match_rank_boundaries() {
        assert_eq!(RANK_THRESHOLDS[0], (95.0, "S"));
        assert_eq!(RANK_THRESHOLDS[1], (85.0, "A"));
        assert_eq!(RANK_THRESHOLDS[2], (70.0, "B"));
    }
}
