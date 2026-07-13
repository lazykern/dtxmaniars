//! Parallax info wedge — slides the info panel on selection change.
//!
//! ADR-0015 deferred item (d). Listens to `dtx_audio::PreviewSwapEvent`
//! and animates the wedge's `Node::top` so it slides in from the
//! direction matching the user's scroll:
//!
//! - `Next` direction: new info slides in from below, old slides up
//! - `Prev` direction: new info slides in from above, old slides down
//! - `None` (first play): no animation
//!
//! Layer: Game. Depends on `dtx-audio` (Engine) for the event type.

use bevy::prelude::*;
use dtx_audio::{PreviewSwapDirection, PreviewSwapEvent};

/// Pixel distance the wedge has slid from its rest position.
/// Positive = below rest (slides in from below).
/// Negative = above rest (slides in from above).
/// Zero = at rest.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct ParallaxInfo {
    pub offset_px: f32,
    pub is_flying: bool,
    pub direction: i8, // -1, 0, +1; matches PreviewSwapDirection
    /// The wedge's "rest" `Node::top` in pixels. The tween shifts the
    /// node's top by `offset_px` while flying, then restores.
    pub rest_top_px: f32,
}

/// Total slide distance in pixels. Sized to be visible but not
/// jarring; matches the album-art fade duration roughly.
pub const SLIDE_DISTANCE_PX: f32 = 48.0;

/// Slide duration in milliseconds (each direction: out + in).
const SLIDE_OUT_MS: u32 = 150;
const SLIDE_IN_MS: u32 = 220;
const SLIDE_TOTAL_MS: u32 = SLIDE_OUT_MS + SLIDE_IN_MS;

/// System: drive the parallax slide from `PreviewSwapEvent` and time.
///
/// On each event, set `is_flying = true` and `direction` from the
/// event payload. Each frame, interpolate `offset_px` toward zero
/// over `SLIDE_TOTAL_MS`. Apply the offset to the node's top.
pub fn parallax_info_tween_system(
    time: Res<Time>,
    policy: Option<Res<crate::AccessibilityPolicy>>,
    mut events: MessageReader<PreviewSwapEvent>,
    mut query: Query<(&mut ParallaxInfo, &mut Node)>,
) {
    let delta_ms = (time.delta_secs() * 1000.0) as u32;
    let motion_allowed = policy
        .as_deref()
        .is_none_or(|policy| policy.background_motion());

    // Apply incoming events.
    for event in events.read() {
        if !motion_allowed {
            continue;
        }
        let dir = match event.direction {
            PreviewSwapDirection::Next => 1i8,
            PreviewSwapDirection::Prev => -1i8,
            PreviewSwapDirection::None => continue,
        };
        for (mut info, _node) in &mut query {
            info.is_flying = true;
            info.direction = dir;
            // Start at the far side: SLIDE_DISTANCE_PX on the
            // direction's side. The tween brings it back to 0.
            info.offset_px = SLIDE_DISTANCE_PX * dir as f32;
        }
    }

    // Advance in-flight tweens. Approximate "out then in" with a
    // single linear interpolation across SLIDE_TOTAL_MS. Close
    // enough visually; a frame counter would be the "true" out-then-in.
    for (mut info, mut node) in &mut query {
        if !motion_allowed {
            info.offset_px = 0.0;
            info.is_flying = false;
            info.direction = 0;
            node.top = Val::Px(info.rest_top_px);
            continue;
        }
        if !info.is_flying {
            if info.offset_px != 0.0 {
                info.offset_px = 0.0;
            }
            // Always rest at the recorded rest_top_px.
            node.top = Val::Px(info.rest_top_px);
            continue;
        }

        let t = (delta_ms as f32 / SLIDE_TOTAL_MS as f32).min(1.0);
        let target = 0.0;
        info.offset_px += (target - info.offset_px) * t;

        if info.offset_px.abs() < 0.5 {
            info.offset_px = 0.0;
            info.is_flying = false;
            info.direction = 0;
        }

        node.top = Val::Px(info.rest_top_px + info.offset_px);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parallax_info_defaults_to_at_rest() {
        let info = ParallaxInfo::default();
        assert_eq!(info.offset_px, 0.0);
        assert!(!info.is_flying);
        assert_eq!(info.direction, 0);
    }

    // Guards the tuned constant against future edits; clippy folds the const
    // comparison to a literal, hence the allow (clippy::assertions_on_constants).
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn slide_distance_is_visible_but_small() {
        assert!(SLIDE_DISTANCE_PX > 16.0);
        assert!(SLIDE_DISTANCE_PX < 96.0);
    }

    #[test]
    fn slide_total_matches_audio_crossfade() {
        // Audio fade-out 150 + fade-in 220 = 370ms. Slide should be
        // roughly comparable so visual and audio land together.
        assert_eq!(SLIDE_TOTAL_MS, SLIDE_OUT_MS + SLIDE_IN_MS);
        assert_eq!(SLIDE_TOTAL_MS, 370);
    }

    /// End-to-end: register system, spawn entity, send event, tick,
    /// verify state transitions.
    #[test]
    fn event_drives_parallax_state() {
        use bevy::app::App;

        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_message::<PreviewSwapEvent>();
        app.add_systems(Update, parallax_info_tween_system);

        let entity = app
            .world_mut()
            .spawn((
                ParallaxInfo {
                    rest_top_px: 100.0,
                    ..default()
                },
                Node {
                    top: Val::Px(100.0),
                    ..default()
                },
            ))
            .id();

        // Send a Next-direction event.
        app.world_mut().write_message(PreviewSwapEvent {
            old_path: Some(PathBuf::from("/songs/a/preview.ogg")),
            new_path: PathBuf::from("/songs/b/preview.ogg"),
            direction: PreviewSwapDirection::Next,
        });
        app.update();

        let info = app.world().get::<ParallaxInfo>(entity).unwrap();
        assert!(info.is_flying);
        assert_eq!(info.direction, 1);
        // After the first tick, offset is at or near SLIDE_DISTANCE_PX
        // (delta_ms=0 in test, so interpolation is a no-op). Just
        // assert non-zero to confirm the event set the field.
        assert!(info.offset_px > 0.0);
        assert!(info.offset_px <= SLIDE_DISTANCE_PX);
    }
}
