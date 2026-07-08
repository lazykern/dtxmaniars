//! Stage transform: maps the drums playfield into a sub-rect of the window.
//!
//! `StageRect` is the CURRENT rect every drums layout/picking consumer reads
//! instead of the raw window size. `StageTarget` is the desired rect the
//! Customize surface writes (Task 5); a lerp moves `StageRect` toward it
//! (Task 6). When the surface is closed the rect is the full window (identity),
//! so all gameplay geometry is byte-identical to pre-transform behavior.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Window sub-rect the drums stage is mapped into (physical px).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct StageRect {
    pub origin: Vec2,
    pub size: Vec2,
}

/// Desired stage rect; `StageRect` animates toward this.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
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

impl Default for StageTarget {
    fn default() -> Self {
        StageTarget(StageRect::default())
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
            )
                .chain(),
        );
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
    fn center_of_offset_rect() {
        let r = StageRect {
            origin: Vec2::new(220.0, 0.0),
            size: Vec2::new(1000.0, 720.0),
        };
        assert_eq!(r.center(), Vec2::new(720.0, 360.0));
    }
}
