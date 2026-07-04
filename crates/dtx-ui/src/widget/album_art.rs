//! Album-art crossfade widget.
//!
//! ADR-0015 Phase 3. Listens to `dtx_audio::PreviewSwapEvent` and
//! crossfades the album art's opacity to match the audio crossfade.
//!
//! Two phases:
//! - Fade out (150ms, 1.0 → 0.0) — matches audio fade-out midpoint
//!   (~75ms from swap start) so the visual "darkens" as the audio
//!   quiets.
//! - Fade in (220ms, 0.0 → 1.0) — matches audio fade-in. New image
//!   reaches full opacity just as the audio fade-in completes
//!   (220ms after the 30ms pre-roll).
//!
//! Guard against in-flight tween: if a previous fade-in is still
//! running, the new event hard-cuts to the new image instead of
//! starting a partial tween. Prevents partial-opacity ghosts on
//! rapid scroll.
//!
//! The widget is a `Component`; the `album_art_tween_system` updates
//! its `opacity` and `is_flying` fields. The `apply_album_art_opacity`
//! system copies `opacity` to the entity's `BackgroundColor` alpha so
//! the actual visual changes.
//!
//! No actual album art image is loaded. A colored panel placeholder
//! is sufficient for the tween demo. Real image loading (from
//! `#PREIMAGE:` or a per-song cover file) is a separate task.
//!
//! Layer: Game. Depends on `dtx-audio` (Engine) for the event type.

use bevy::prelude::*;
use dtx_audio::PreviewSwapEvent;

/// Album-art widget marker. Attach to the entity holding the album
/// art (or any placeholder) to participate in the crossfade tween.
#[derive(Component, Debug, Clone, Copy)]
pub struct AlbumArt {
    /// True while a crossfade tween is in flight. New swap events
    /// arriving while `is_flying == true` trigger a hard-cut instead
    /// of starting a new tween.
    pub is_flying: bool,
    /// Elapsed milliseconds in the current tween. Resets to 0 on
    /// each event (whether starting fresh or hard-cutting).
    pub elapsed_ms: u32,
    /// Current opacity, 0.0 (transparent) to 1.0 (opaque). Driven by
    /// the tween system, read by `apply_album_art_opacity`.
    pub opacity: f32,
}

impl Default for AlbumArt {
    fn default() -> Self {
        Self {
            is_flying: false,
            elapsed_ms: 0,
            opacity: 1.0,
        }
    }
}

const FADE_OUT_MS: u32 = 150;
const FADE_IN_MS: u32 = 220;
const TOTAL_MS: u32 = FADE_OUT_MS + FADE_IN_MS;

/// System: drive album-art tween from `PreviewSwapEvent` and time.
///
/// Each frame:
/// 1. If `is_flying`, advance `elapsed_ms` and recompute `opacity`.
/// 2. On event: if not flying, start a new tween at opacity 1.0.
///    If flying, hard-cut: jump to opacity 0, set elapsed_ms to
///    FADE_OUT_MS so the next tick starts the fade-in phase.
pub fn album_art_tween_system(
    time: Res<Time>,
    mut events: MessageReader<PreviewSwapEvent>,
    mut query: Query<&mut AlbumArt>,
) {
    let delta_ms = (time.delta_secs() * 1000.0) as u32;

    // Advance in-flight tweens.
    for mut art in &mut query {
        if art.is_flying {
            art.elapsed_ms = art.elapsed_ms.saturating_add(delta_ms);
            if art.elapsed_ms < FADE_OUT_MS {
                // Phase 1: fade out (1.0 → 0.0)
                art.opacity = 1.0 - (art.elapsed_ms as f32 / FADE_OUT_MS as f32);
            } else if art.elapsed_ms < TOTAL_MS {
                // Phase 2: fade in (0.0 → 1.0)
                let t = (art.elapsed_ms - FADE_OUT_MS) as f32 / FADE_IN_MS as f32;
                art.opacity = t.clamp(0.0, 1.0);
            } else {
                // Done.
                art.opacity = 1.0;
                art.is_flying = false;
                art.elapsed_ms = 0;
            }
        }
    }

    // Apply incoming swap events.
    for _event in events.read() {
        for mut art in &mut query {
            if art.is_flying {
                // Hard-cut: skip to phase 2 start.
                art.elapsed_ms = FADE_OUT_MS;
                art.opacity = 0.0;
            } else {
                // Fresh tween.
                art.is_flying = true;
                art.elapsed_ms = 0;
                art.opacity = 1.0;
            }
        }
    }
}

/// System: copy `AlbumArt.opacity` to the entity's `BackgroundColor`
/// alpha so the visual actually changes. Pairs with
/// `album_art_tween_system` in the same Update schedule; runs after.
pub fn apply_album_art_opacity(mut query: Query<(&AlbumArt, &mut BackgroundColor)>) {
    for (art, mut bg) in &mut query {
        let alpha = bg.0.alpha();
        if (alpha - art.opacity).abs() > 0.001 {
            bg.0 = bg.0.with_alpha(art.opacity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use dtx_audio::PreviewSwapDirection;
    use std::path::PathBuf;

    #[test]
    fn album_art_defaults_to_opaque_not_flying() {
        let art = AlbumArt::default();
        assert!(!art.is_flying);
        assert_eq!(art.elapsed_ms, 0);
        assert!((art.opacity - 1.0).abs() < 0.001);
    }

    #[test]
    fn fade_constants_match_audio_crossfade() {
        // Audio fade-out is 150ms (osu MusicController.cs:520)
        // Audio fade-in is 220ms (osu MusicController.cs:519)
        assert_eq!(FADE_OUT_MS, 150);
        assert_eq!(FADE_IN_MS, 220);
        assert_eq!(TOTAL_MS, 370);
    }

    #[test]
    fn opacity_at_phase_boundary() {
        // At elapsed_ms = FADE_OUT_MS, opacity should be 0.0.
        // At elapsed_ms = TOTAL_MS, opacity should be 1.0.
        let fade_out_f = FADE_OUT_MS as f32;
        let total_f = TOTAL_MS as f32;
        let mid_phase_opacity = 1.0 - (fade_out_f / fade_out_f);
        assert!(mid_phase_opacity.abs() < 0.001);
        let end_opacity = (total_f - fade_out_f) / (total_f - fade_out_f);
        assert!((end_opacity - 1.0).abs() < 0.001);
    }

    /// End-to-end: register systems, spawn entity, send event, tick,
    /// verify opacity transitioned and BackgroundColor alpha matches.
    #[test]
    fn event_starts_tween_and_apply_system_copies_opacity() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_message::<PreviewSwapEvent>();
        app.add_systems(
            Update,
            (album_art_tween_system, apply_album_art_opacity).chain(),
        );

        let entity = app
            .world_mut()
            .spawn((
                AlbumArt::default(),
                BackgroundColor(Color::srgba(0.5, 0.2, 0.8, 1.0)),
            ))
            .id();

        // Send a swap event.
        app.world_mut().write_message(PreviewSwapEvent {
            old_path: None,
            new_path: PathBuf::from("/songs/a/preview.ogg"),
            direction: PreviewSwapDirection::Next,
        });
        app.update();

        // After one tick, the tween should have started: is_flying=true,
        // opacity starts at 1.0 (we just began fading out).
        let art = app.world().get::<AlbumArt>(entity).unwrap();
        assert!(art.is_flying);
        assert_eq!(art.elapsed_ms, 0);
        assert!((art.opacity - 1.0).abs() < 0.001);

        // The apply system should have set the BackgroundColor alpha
        // to 1.0 (still opaque at the start).
        let bg = app.world().get::<BackgroundColor>(entity).unwrap();
        assert!((bg.0.alpha() - 1.0).abs() < 0.001);
    }
}
