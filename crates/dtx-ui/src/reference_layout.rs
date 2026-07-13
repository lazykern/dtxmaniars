//! Reference-space layout decisions shared by menus and gameplay overlays.
//!
//! UI geometry is authored against 1280×720.  These helpers keep a small safe
//! inset at every supported aspect ratio and repair only the runtime rectangle
//! of persisted widgets; callers remain responsible for saving an edited
//! position after an explicit player action.

use bevy::math::{Rect, Vec2};

/// A width/height pair in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Usable viewport bounds after the reference-space safety inset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeArea {
    rect: Rect,
    reference_scale: f32,
}

impl SafeArea {
    const REF_WIDTH: f32 = 1280.0;
    const REF_HEIGHT: f32 = 720.0;
    const REF_INSET: f32 = 24.0;

    pub fn for_viewport(width: f32, height: f32) -> Self {
        let width = width.max(1.0);
        let height = height.max(1.0);
        let scale = (width / Self::REF_WIDTH)
            .min(height / Self::REF_HEIGHT)
            .max(f32::EPSILON);
        let inset = Self::REF_INSET * scale;
        Self {
            rect: Rect::from_corners(Vec2::splat(inset), Vec2::new(width - inset, height - inset)),
            reference_scale: scale,
        }
    }

    pub fn reference_720p() -> Self {
        Self::for_viewport(Self::REF_WIDTH, Self::REF_HEIGHT)
    }

    pub const fn rect(self) -> Rect {
        self.rect
    }

    pub const fn reference_scale(self) -> f32 {
        self.reference_scale
    }

    /// Whether at least a focus-sized piece of `rect` is accessible.
    pub fn contains_focus_handle(self, rect: Rect, handle: Size) -> bool {
        let min = rect.min.max(self.rect.min);
        let max = rect.max.min(self.rect.max);
        max.x - min.x >= handle.width.min(rect.width())
            && max.y - min.y >= handle.height.min(rect.height())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitDecision {
    Full,
    CompactScrollable,
}

/// Chooses an overflow-safe overlay variant without reducing semantic text.
pub fn fit_overlay(content: Size, safe: SafeArea, text_scale: f32) -> FitDecision {
    let required = Size::new(content.width * text_scale, content.height * text_scale);
    // Dialogs deliberately occupy only a bounded portion of the screen.  At
    // 720p this switches a 420×180 panel to its compact scrolling variant at
    // XL instead of squeezing the text below its semantic minimum.
    let available = Size::new(
        (560.0 * safe.reference_scale()).min(safe.rect().width()),
        (240.0 * safe.reference_scale()).min(safe.rect().height()),
    );
    if required.width <= available.width && required.height <= available.height {
        FitDecision::Full
    } else {
        FitDecision::CompactScrollable
    }
}

/// Clamp a persisted widget into the runtime safe area.
///
/// The input value is copied, never mutated or written back.  Widgets that fit
/// are made fully visible; oversized widgets retain at least their focus handle.
pub fn repair_runtime_rect(saved: Rect, safe: SafeArea, handle: Size) -> Rect {
    let size = Vec2::new(saved.width().max(0.0), saved.height().max(0.0));
    let safe_rect = safe.rect();
    let x = if size.x <= safe_rect.width() {
        saved.min.x.clamp(safe_rect.min.x, safe_rect.max.x - size.x)
    } else {
        saved.min.x.clamp(
            safe_rect.min.x - size.x + handle.width.min(size.x),
            safe_rect.max.x - handle.width.min(size.x),
        )
    };
    let y = if size.y <= safe_rect.height() {
        saved.min.y.clamp(safe_rect.min.y, safe_rect.max.y - size.y)
    } else {
        saved.min.y.clamp(
            safe_rect.min.y - size.y + handle.height.min(size.y),
            safe_rect.max.y - handle.height.min(size.y),
        )
    };
    Rect::from_corners(Vec2::new(x, y), Vec2::new(x, y) + size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlarge_text_chooses_compact_layout_before_shrinking_below_minimum() {
        let fit = fit_overlay(
            Size::new(420.0, 180.0),
            SafeArea::for_viewport(1280.0, 720.0),
            1.5,
        );
        assert_eq!(fit, FitDecision::CompactScrollable);
    }

    #[test]
    fn standard_text_keeps_the_full_layout() {
        assert_eq!(
            fit_overlay(Size::new(420.0, 180.0), SafeArea::reference_720p(), 1.0,),
            FitDecision::Full
        );
    }

    #[test]
    fn supported_viewports_keep_a_reference_scaled_safe_inset() {
        for (width, height) in [(1280.0, 720.0), (1920.0, 1080.0), (2560.0, 1080.0)] {
            let safe = SafeArea::for_viewport(width, height);
            assert!(safe.rect().min.x > 0.0);
            assert!(safe.rect().min.y > 0.0);
            assert!(safe.rect().max.x < width);
            assert!(safe.rect().max.y < height);
        }
    }

    #[test]
    fn offscreen_widget_is_repaired_for_runtime_without_rewriting_persistence() {
        let saved = bevy::math::Rect::from_corners(
            bevy::math::Vec2::new(1400.0, 900.0),
            bevy::math::Vec2::new(1600.0, 980.0),
        );
        let repaired =
            repair_runtime_rect(saved, SafeArea::reference_720p(), Size::new(24.0, 24.0));
        assert_ne!(repaired, saved);
        assert!(SafeArea::reference_720p().contains_focus_handle(repaired, Size::new(24.0, 24.0)));
    }
}
