//! Horizontal time-density strip: N bars over song length, plus
//! percent-positioning helpers for playhead / A / B markers.
//!
//! Practice transport uses it; any time-indexed overview can reuse it.
//! (Distinct from `density_graph`, which is per-lane, not time-indexed.)

use bevy::prelude::*;

use crate::theme::Theme;

/// Marker for the strip container (relative-positioned).
#[derive(Component)]
pub struct DensityStrip;

/// One density bar; index into the samples array.
#[derive(Component)]
pub struct DensityBar(pub usize);

/// Bar height as percent of strip height for a normalized sample.
pub fn bar_height_pct(v: f32) -> f32 {
    8.0 + v.clamp(0.0, 1.0) * 92.0
}

/// Left position (percent) for a chart time on a strip of `end_ms` length.
pub fn time_to_pct(ms: i64, end_ms: i64) -> f32 {
    if end_ms <= 0 {
        0.0
    } else {
        (ms.clamp(0, end_ms) as f64 / end_ms as f64 * 100.0) as f32
    }
}

/// Spawn the strip with one bar per sample as a child of `parent`.
/// Returns the strip entity so callers can attach marker children.
pub fn spawn_density_strip(
    parent: &mut ChildSpawnerCommands,
    samples: &[f32],
    theme: &Theme,
) -> Entity {
    let mut strip = parent.spawn((
        DensityStrip,
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(1.0),
            ..default()
        },
    ));
    strip.with_children(|bars| {
        for (i, &v) in samples.iter().enumerate() {
            bars.spawn((
                DensityBar(i),
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(bar_height_pct(v)),
                    ..default()
                },
                BackgroundColor(theme.text_secondary.with_alpha(0.55)),
            ));
        }
    });
    strip.id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_height_has_floor_and_ceiling() {
        assert!((bar_height_pct(0.0) - 8.0).abs() < 1e-6);
        assert!((bar_height_pct(1.0) - 100.0).abs() < 1e-6);
        assert!((bar_height_pct(5.0) - 100.0).abs() < 1e-6);
    }

    #[test]
    fn time_to_pct_clamps_and_scales() {
        assert_eq!(time_to_pct(0, 10_000), 0.0);
        assert!((time_to_pct(5_000, 10_000) - 50.0).abs() < 1e-4);
        assert_eq!(time_to_pct(-100, 10_000), 0.0);
        assert!((time_to_pct(99_999, 10_000) - 100.0).abs() < 1e-4);
        assert_eq!(time_to_pct(500, 0), 0.0);
    }
}
