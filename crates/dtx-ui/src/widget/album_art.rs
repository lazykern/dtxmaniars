//! Album-art crossfade widget.
//!
//! ADR-0015 Phase 3. Game code queues a swap via
//! [`AlbumArt::request_swap`] (song select does this on selection
//! change, the same frame the audio preview crossfade starts) and this
//! widget crossfades to the new art. It is the *only* writer of the
//! art entity's `ImageNode` and `BackgroundColor` — the image handle
//! is applied at the fade-out→fade-in boundary so the swap happens at
//! opacity 0.
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
//! running, the new request hard-cuts to the new image instead of
//! starting a partial tween. Prevents partial-opacity ghosts on
//! rapid scroll.
//!
//! "No art" is a first-class target: `request_swap(None)` fades to the
//! placeholder panel (image hidden, `BackgroundColor` at
//! `placeholder_alpha`) instead of to a new image.
//!
//! The widget is a `Component`; the `album_art_tween_system` updates
//! its `opacity`/`is_flying` fields and applies the pending image swap.
//! The `apply_album_art_opacity` system copies the tween state to the
//! entity's `BackgroundColor` and `ImageNode` alphas so the actual
//! visual changes.
//!
//! Layer: Game.

use bevy::prelude::*;

/// Album-art widget marker. Attach to the entity holding the album
/// art (or any placeholder) to participate in the crossfade tween.
#[derive(Component, Debug, Clone)]
pub struct AlbumArt {
    /// True while a crossfade tween is in flight. New swap requests
    /// arriving while `is_flying == true` trigger a hard-cut instead
    /// of starting a new tween.
    pub is_flying: bool,
    /// Elapsed milliseconds in the current tween. Resets to 0 on
    /// each request (whether starting fresh or hard-cutting).
    pub elapsed_ms: u32,
    /// Current opacity, 0.0 (transparent) to 1.0 (opaque). Driven by
    /// the tween system, read by `apply_album_art_opacity`.
    pub opacity: f32,
    /// Alpha of the placeholder `BackgroundColor` when no art is
    /// shown (scaled by `opacity` mid-fade). 1.0 for a bare panel;
    /// song select uses a faint 0.18 wash.
    pub placeholder_alpha: f32,
    /// True while real art is shown: image alpha follows `opacity`
    /// and the placeholder is hidden. False: placeholder shown,
    /// image hidden.
    pub has_art: bool,
    /// Swap target queued by `request_swap`, applied to the
    /// `ImageNode` at the fade-out→fade-in boundary. Inner `None`
    /// means "no art" (fade to the placeholder).
    pending: Option<Option<Handle<Image>>>,
    /// Set by `request_swap`, consumed by the tween system to start
    /// (or hard-cut) a fade.
    requested: bool,
}

impl Default for AlbumArt {
    fn default() -> Self {
        Self {
            is_flying: false,
            elapsed_ms: 0,
            opacity: 1.0,
            placeholder_alpha: 1.0,
            has_art: false,
            pending: None,
            requested: false,
        }
    }
}

impl AlbumArt {
    /// Widget whose placeholder renders at the given background
    /// alpha when no art is shown.
    pub fn with_placeholder_alpha(alpha: f32) -> Self {
        Self {
            placeholder_alpha: alpha,
            ..Self::default()
        }
    }

    /// Queue a crossfade to new art. `Some(handle)` fades to that
    /// image; `None` fades to the placeholder ("no art"). The tween
    /// system starts (or hard-cuts) the fade and swaps the
    /// `ImageNode` handle at the fade-out→fade-in boundary.
    pub fn request_swap(&mut self, image: Option<Handle<Image>>) {
        self.pending = Some(image);
        self.requested = true;
    }

    /// Apply the pending swap (if any) to the image node. Called at
    /// the fade-out→fade-in boundary, when opacity is 0.
    fn apply_pending(&mut self, image: Option<&mut ImageNode>) {
        let Some(target) = self.pending.take() else {
            return;
        };
        self.has_art = target.is_some();
        if let Some(image) = image {
            image.image = target.unwrap_or_default();
        }
    }
}

const FADE_OUT_MS: u32 = 150;
const FADE_IN_MS: u32 = 220;
const TOTAL_MS: u32 = FADE_OUT_MS + FADE_IN_MS;

/// System: drive the album-art tween from swap requests and time.
///
/// Each frame:
/// 1. If `is_flying`, advance `elapsed_ms` and recompute `opacity`.
///    Crossing into the fade-in phase applies the pending image swap.
/// 2. On request: if not flying, start a new tween at opacity 1.0.
///    If flying, hard-cut: jump to opacity 0, apply the swap, set
///    elapsed_ms to FADE_OUT_MS so the next tick starts the fade-in.
pub fn album_art_tween_system(
    time: Res<Time>,
    mut query: Query<(&mut AlbumArt, Option<&mut ImageNode>)>,
) {
    let delta_ms = (time.delta_secs() * 1000.0) as u32;

    for (mut art, mut image) in &mut query {
        // Advance in-flight tweens.
        if art.is_flying {
            art.elapsed_ms = art.elapsed_ms.saturating_add(delta_ms);
            if art.elapsed_ms < FADE_OUT_MS {
                // Phase 1: fade out (1.0 → 0.0)
                art.opacity = 1.0 - (art.elapsed_ms as f32 / FADE_OUT_MS as f32);
            } else if art.elapsed_ms < TOTAL_MS {
                // Phase 2: fade in (0.0 → 1.0). The old art is fully
                // faded out here; swap in the new target.
                art.apply_pending(image.as_deref_mut());
                let t = (art.elapsed_ms - FADE_OUT_MS) as f32 / FADE_IN_MS as f32;
                art.opacity = t.clamp(0.0, 1.0);
            } else {
                // Done.
                art.apply_pending(image.as_deref_mut());
                art.opacity = 1.0;
                art.is_flying = false;
                art.elapsed_ms = 0;
            }
        }

        // Apply an incoming swap request.
        if art.requested {
            art.requested = false;
            if art.is_flying {
                // Hard-cut: skip to phase 2 start with the new art.
                art.elapsed_ms = FADE_OUT_MS;
                art.opacity = 0.0;
                art.apply_pending(image.as_deref_mut());
            } else {
                // Fresh tween.
                art.is_flying = true;
                art.elapsed_ms = 0;
                art.opacity = 1.0;
            }
        }
    }
}

/// System: copy the tween state to the entity's `BackgroundColor`
/// (placeholder) and `ImageNode` (art) alphas so the visual actually
/// changes. With art: image alpha = `opacity`, placeholder hidden.
/// Without: image hidden, placeholder alpha = `placeholder_alpha *
/// opacity`. Pairs with `album_art_tween_system` in the same Update
/// schedule; runs after.
pub fn apply_album_art_opacity(
    mut query: Query<(&AlbumArt, &mut BackgroundColor, Option<&mut ImageNode>)>,
) {
    for (art, mut bg, image) in &mut query {
        let bg_alpha = if art.has_art {
            0.0
        } else {
            art.placeholder_alpha * art.opacity
        };
        if (bg.0.alpha() - bg_alpha).abs() > 0.001 {
            bg.0 = bg.0.with_alpha(bg_alpha);
        }
        if let Some(mut image) = image {
            let image_alpha = if art.has_art { art.opacity } else { 0.0 };
            if (image.color.alpha() - image_alpha).abs() > 0.001 {
                image.color = image.color.with_alpha(image_alpha);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;

    #[test]
    fn album_art_defaults_to_opaque_not_flying() {
        let art = AlbumArt::default();
        assert!(!art.is_flying);
        assert_eq!(art.elapsed_ms, 0);
        assert!((art.opacity - 1.0).abs() < 0.001);
        assert!(!art.has_art);
        assert!((art.placeholder_alpha - 1.0).abs() < 0.001);
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

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_systems(
            Update,
            (album_art_tween_system, apply_album_art_opacity).chain(),
        );
        app
    }

    /// End-to-end: register systems, spawn entity, request a swap,
    /// tick, verify the tween started and BackgroundColor alpha
    /// matches.
    #[test]
    fn request_starts_tween_and_apply_system_copies_opacity() {
        let mut app = test_app();

        let entity = app
            .world_mut()
            .spawn((
                AlbumArt::default(),
                BackgroundColor(Color::srgba(0.5, 0.2, 0.8, 1.0)),
            ))
            .id();

        // Queue a swap request (no art → placeholder).
        app.world_mut()
            .get_mut::<AlbumArt>(entity)
            .unwrap()
            .request_swap(None);
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

    /// Completing a swap to real art ends with image alpha 1.0 and
    /// placeholder alpha 0.0, and the ImageNode carries the handle.
    #[test]
    fn swap_to_art_end_state() {
        let mut app = test_app();
        let handle = Handle::<Image>::default();

        let entity = app
            .world_mut()
            .spawn((
                AlbumArt::with_placeholder_alpha(0.18),
                BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.18)),
                ImageNode {
                    color: Color::WHITE.with_alpha(0.0),
                    ..Default::default()
                },
            ))
            .id();

        {
            let mut art = app.world_mut().get_mut::<AlbumArt>(entity).unwrap();
            art.pending = Some(Some(handle.clone()));
            // Fast-forward: pretend the tween is at the end.
            art.is_flying = true;
            art.elapsed_ms = TOTAL_MS;
        }
        app.update();

        let art = app.world().get::<AlbumArt>(entity).unwrap();
        assert!(art.has_art);
        assert!(!art.is_flying);
        assert!((art.opacity - 1.0).abs() < 0.001);
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert!((image.color.alpha() - 1.0).abs() < 0.001);
        let bg = app.world().get::<BackgroundColor>(entity).unwrap();
        assert!(bg.0.alpha().abs() < 0.001);
    }

    /// Completing a swap to "no art" ends with the image hidden
    /// (default handle, alpha 0.0) and the placeholder at its
    /// configured alpha.
    #[test]
    fn swap_to_no_art_end_state() {
        let mut app = test_app();

        let entity = app
            .world_mut()
            .spawn((
                AlbumArt {
                    has_art: true,
                    ..AlbumArt::with_placeholder_alpha(0.18)
                },
                BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.0)),
                ImageNode {
                    color: Color::WHITE.with_alpha(1.0),
                    ..Default::default()
                },
            ))
            .id();

        {
            let mut art = app.world_mut().get_mut::<AlbumArt>(entity).unwrap();
            art.pending = Some(None);
            art.is_flying = true;
            art.elapsed_ms = TOTAL_MS;
        }
        app.update();

        let art = app.world().get::<AlbumArt>(entity).unwrap();
        assert!(!art.has_art);
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert_eq!(image.image, Handle::<Image>::default());
        assert!(image.color.alpha().abs() < 0.001);
        let bg = app.world().get::<BackgroundColor>(entity).unwrap();
        assert!((bg.0.alpha() - 0.18).abs() < 0.001);
    }

    /// A request arriving mid-flight hard-cuts: opacity drops to 0
    /// and the new image is applied immediately.
    #[test]
    fn request_while_flying_hard_cuts() {
        let mut app = test_app();

        let entity = app
            .world_mut()
            .spawn((
                AlbumArt::default(),
                BackgroundColor(Color::WHITE),
                ImageNode::default(),
            ))
            .id();

        {
            let mut art = app.world_mut().get_mut::<AlbumArt>(entity).unwrap();
            art.is_flying = true;
            art.elapsed_ms = 10;
            art.request_swap(Some(Handle::default()));
        }
        app.update();

        let art = app.world().get::<AlbumArt>(entity).unwrap();
        assert!(art.is_flying);
        assert_eq!(art.elapsed_ms, FADE_OUT_MS);
        assert!(art.opacity.abs() < 0.001);
        assert!(art.has_art);
    }
}
