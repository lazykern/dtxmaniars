//! Stage transform: maps the drums playfield into a sub-rect of the window.
//!
//! `StageRect` is the CURRENT rect every drums layout/picking consumer reads
//! instead of the raw window size. `StageTarget` is the desired rect the
//! Customize surface writes (Task 5); a lerp moves `StageRect` toward it
//! (Task 6). When the surface is closed the rect is the full window (identity),
//! so all gameplay geometry is byte-identical to pre-transform behavior.

use bevy::prelude::*;
use bevy::ui::Val2;
use bevy::window::PrimaryWindow;

/// Window sub-rect the drums stage is mapped into (physical px).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StageRect {
    pub origin: Vec2,
    pub size: Vec2,
}

/// Desired stage rect; `StageRect` animates toward this.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct StageTarget(pub StageRect);

impl StageRect {
    /// Identity: the whole window.
    pub fn full(window: Vec2) -> Self {
        Self {
            origin: Vec2::ZERO,
            size: window,
        }
    }
    /// Center of the rect in window coords (replaces `window/2` half-extent use).
    pub fn center(&self) -> Vec2 {
        self.origin + self.size * 0.5
    }
}

impl Default for StageRect {
    fn default() -> Self {
        // Placeholder until the first `sync_stage_target_to_window`; REF size.
        Self {
            origin: Vec2::ZERO,
            size: Vec2::new(1280.0, 720.0),
        }
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<StageRect>()
        .init_resource::<StageTarget>()
        .add_systems(
            Update,
            (
                sync_stage_target_to_window.run_if(surface_closed),
                animate_stage_rect,
                apply_stage_transform,
            )
                .chain(),
        );
}

/// Uniform transform (scale `s`, translation `t` px) that maps the FULL WINDOW
/// into `rect`, aspect-preserving and centered (osu "SetCustomRect"). Scale
/// pivots about the window center `C` (Bevy `UiTransform` convention), so the
/// translation compensates: `t = D − C·(1−s)` where `D` is the top-left of the
/// scaled window inside `rect`. Closed surface ⇒ `rect == full window` ⇒
/// `s = 1, t = 0` ⇒ identity (normal play byte-identical).
pub fn stage_xform(rect: StageRect, window: Vec2) -> (f32, Vec2) {
    let s = (rect.size.x / window.x)
        .min(rect.size.y / window.y)
        .clamp(0.01, 1.0);
    let d = rect.origin + (rect.size - window * s) * 0.5;
    let c = window * 0.5;
    (s, d - c * (1.0 - s))
}

/// Inverse of `stage_xform`: map a window-space point (e.g. the cursor) into
/// scene space — the full-window coordinates `HudRoot` children lay out in
/// before the stage transform shrinks them. Identity while the surface is
/// closed (rect == full window).
pub fn window_to_scene(pos: Vec2, rect: StageRect, window: Vec2) -> Vec2 {
    let (s, t) = stage_xform(rect, window);
    let c = window * 0.5;
    c + (pos - t - c) / s
}

/// Drive `HudRoot`'s `UiTransform` from the current `StageRect`, shrinking the
/// whole scene (playfield + every HUD widget, all children of `HudRoot`) into
/// the stage rect. Identity while the surface is closed.
fn apply_stage_transform(
    rect: Res<StageRect>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut roots: Query<&mut bevy::ui::UiTransform, With<crate::hud::HudRoot>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let (s, t) = stage_xform(*rect, Vec2::new(win.width(), win.height()));
    for mut tf in &mut roots {
        tf.scale = Vec2::splat(s);
        tf.translation = Val2::new(Val::Px(t.x), Val::Px(t.y));
    }
}

/// True when the Customize surface is NOT open (identity should hold).
fn surface_closed(open: Option<Res<crate::editor::EditorOpen>>) -> bool {
    open.map(|o| !o.0).unwrap_or(true)
}

/// While closed, the target is always the full window.
fn sync_stage_target_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut target: ResMut<StageTarget>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let full = StageRect::full(Vec2::new(win.width(), win.height()));
    if target.0 != full {
        target.0 = full;
    }
}

/// Exponential ease-out step toward `target` over ~`tau` seconds.
/// `dt` = frame seconds. Returns the new rect.
pub fn ease_rect(current: StageRect, target: StageRect, dt: f32) -> StageRect {
    // Frame-rate independent smoothing: alpha = 1 - exp(-dt / TAU)
    const TAU: f32 = 0.12; // ~ reaches target in ~450ms visually
    let a = 1.0 - (-dt / TAU).exp();
    let lerp = |c: Vec2, t: Vec2| c + (t - c) * a;
    let next = StageRect {
        origin: lerp(current.origin, target.origin),
        size: lerp(current.size, target.size),
    };
    // Snap when close to kill the long tail.
    let close =
        (next.origin - target.origin).length() < 0.5 && (next.size - target.size).length() < 0.5;
    if close {
        target
    } else {
        next
    }
}

/// Move `StageRect` toward `StageTarget` with a frame-rate-independent ease-out.
fn animate_stage_rect(time: Res<Time>, target: Res<StageTarget>, mut rect: ResMut<StageRect>) {
    if *rect == target.0 {
        return;
    }
    let next = ease_rect(*rect, target.0, time.delta_secs());
    *rect = next;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_is_origin_zero_and_window_size() {
        let r = StageRect::full(Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::ZERO);
        assert_eq!(r.size, Vec2::new(1600.0, 900.0));
    }

    #[test]
    fn center_of_full_is_window_half() {
        let r = StageRect::full(Vec2::new(1600.0, 900.0));
        assert_eq!(r.center(), Vec2::new(800.0, 450.0));
    }

    #[test]
    fn ease_moves_toward_target_and_snaps_when_close() {
        let c = StageRect::full(Vec2::new(1000.0, 1000.0));
        let t = StageRect {
            origin: Vec2::new(220.0, 0.0),
            size: Vec2::new(1000.0, 1000.0),
        };
        let mid = ease_rect(c, t, 1.0 / 60.0);
        assert!(mid.origin.x > 0.0 && mid.origin.x < 220.0, "moved partway");
        // A big dt (or many steps) snaps exactly.
        let done = ease_rect(t, t, 1.0 / 60.0);
        assert_eq!(done, t);
    }

    #[test]
    fn xform_full_window_is_identity() {
        let w = Vec2::new(1745.0, 1090.0);
        let (s, t) = stage_xform(StageRect::full(w), w);
        assert!((s - 1.0).abs() < 1e-4);
        assert!(t.length() < 1e-3);
    }

    #[test]
    fn xform_shrinks_and_centers_into_rect() {
        let w = Vec2::new(1745.0, 1090.0);
        // A centered sub-rect: uniform scale, window maps inside it.
        let rect = StageRect {
            origin: Vec2::new(500.0, 24.0),
            size: Vec2::new(800.0, 1000.0),
        };
        let (s, t) = stage_xform(rect, w);
        assert!(s < 1.0 && s > 0.0);
        // Window top-left (0,0) maps to D = t + C(1-s); must land inside rect.
        let c = w * 0.5;
        let d = t + c * (1.0 - s);
        assert!(d.x >= rect.origin.x - 0.5 && d.y >= rect.origin.y - 0.5);
        // Scaled window fits within the rect on the binding axis.
        assert!(w.y * s <= rect.size.y + 0.5);
    }

    #[test]
    fn window_to_scene_is_identity_when_full() {
        let w = Vec2::new(1745.0, 1090.0);
        let p = Vec2::new(300.0, 700.0);
        let q = window_to_scene(p, StageRect::full(w), w);
        assert!((q - p).length() < 1e-3);
    }

    #[test]
    fn window_to_scene_inverts_stage_xform() {
        let w = Vec2::new(1745.0, 1090.0);
        let rect = StageRect {
            origin: Vec2::new(496.0, 24.0),
            size: Vec2::new(1233.0, 1042.0),
        };
        let (s, t) = stage_xform(rect, w);
        let c = w * 0.5;
        // Forward: scene point p renders at c + s(p−c) + t.
        let p = Vec2::new(200.0, 900.0);
        let rendered = c + s * (p - c) + t;
        let back = window_to_scene(rendered, rect, w);
        assert!((back - p).length() < 1e-3);
    }

    #[test]
    fn center_of_offset_rect() {
        let r = StageRect {
            origin: Vec2::new(220.0, 0.0),
            size: Vec2::new(1000.0, 720.0),
        };
        assert_eq!(r.center(), Vec2::new(720.0, 360.0));
    }
}
