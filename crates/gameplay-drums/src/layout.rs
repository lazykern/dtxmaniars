//! Playfield layout — ref-resolution scaling for drums HUD (dtxpt-inspired).

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResized};
use dtx_ui::theme::{REF_HEIGHT, REF_WIDTH};
use game_shell::AppState;

use crate::lane_geometry::{COLUMNS, STRIP_REF_LEFT, STRIP_REF_WIDTH};

pub const REF_JUDGE_Y: f32 = 620.0;
pub const REF_LANE_TOP: f32 = 80.0;
pub const REF_LANE_HEIGHT: f32 = 540.0;
pub const REF_KEY_CAP_H: f32 = 42.0;
pub const REF_LABEL_OFFSET: f32 = 28.0;
pub const REF_KEY_VIZ_OFFSET: f32 = 64.0;
pub const REF_BACKBOARD_PAD: f32 = 12.0;

/// Centered strip left edge at ref resolution (redesign: symmetric panels).
pub const STRIP_REF_CENTERED_LEFT: f32 = (REF_WIDTH - STRIP_REF_WIDTH) / 2.0;

/// A column's left edge in ref px, translated from NX absolute into the
/// centered strip (columns keep their NX proportional widths + gaps).
#[inline]
pub fn col_ref_x(col: usize) -> f32 {
    STRIP_REF_CENTERED_LEFT + (COLUMNS[col].ref_x - STRIP_REF_LEFT)
}

/// Phrase meter sits just right of the lane strip, clear of the side pillar.
#[inline]
pub fn ref_phrase_x() -> f32 {
    STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH + 15.0
}

/// Right HUD column (song info, combo, gauge) anchor, just right of the strip.
#[inline]
pub fn ref_hud_right_x() -> f32 {
    STRIP_REF_CENTERED_LEFT + STRIP_REF_WIDTH + 24.0
}

pub const REF_COMBO_Y: f32 = 72.0;

#[derive(Resource, Clone, Copy, Debug)]
pub struct PlayfieldLayout {
    pub scale: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for PlayfieldLayout {
    fn default() -> Self {
        Self::from_size(REF_WIDTH, REF_HEIGHT)
    }
}

impl PlayfieldLayout {
    pub fn from_window(window: &Window) -> Self {
        Self::from_size(window.width(), window.height())
    }

    pub fn from_size(width: f32, height: f32) -> Self {
        let scale = (width / REF_WIDTH).min(height / REF_HEIGHT);
        Self {
            scale,
            width,
            height,
        }
    }

    pub fn judge_y(&self) -> f32 {
        REF_JUDGE_Y * self.scale
    }

    pub fn lane_top(&self) -> f32 {
        REF_LANE_TOP * self.scale
    }

    pub fn lane_height(&self) -> f32 {
        REF_LANE_HEIGHT * self.scale
    }

    pub fn col_left(&self, col: usize) -> f32 {
        col_ref_x(col) * self.scale
    }

    pub fn col_width(&self, col: usize) -> f32 {
        COLUMNS[col].ref_w * self.scale
    }

    pub fn strip_left(&self) -> f32 {
        STRIP_REF_CENTERED_LEFT * self.scale
    }

    pub fn strip_width(&self) -> f32 {
        STRIP_REF_WIDTH * self.scale
    }

    /// NX prints measure# just right of the strip (`CStagePerfDrumsScreen.cs:3588`).
    pub fn measure_label_left(&self) -> f32 {
        self.strip_left() + self.strip_width() + 8.0 * self.scale
    }

    /// Lane abbreviations (HH, SD, …) sit just above the playfield.
    pub fn lane_header_top(&self) -> f32 {
        self.lane_top() - 8.0 * self.scale
    }

    pub fn label_top(&self) -> f32 {
        self.judge_y() + REF_LABEL_OFFSET * self.scale
    }

    pub fn key_viz_top(&self) -> f32 {
        self.judge_y() + REF_KEY_VIZ_OFFSET * self.scale
    }

    pub fn key_cap_height(&self) -> f32 {
        REF_KEY_CAP_H * self.scale
    }

    pub fn backboard_left(&self) -> f32 {
        self.strip_left() - REF_BACKBOARD_PAD * self.scale
    }

    pub fn backboard_top(&self) -> f32 {
        self.lane_top() - REF_BACKBOARD_PAD * self.scale
    }

    pub fn backboard_width(&self) -> f32 {
        self.strip_width() + REF_BACKBOARD_PAD * self.scale * 2.0
    }

    pub fn backboard_height(&self) -> f32 {
        self.lane_height() + REF_BACKBOARD_PAD * self.scale * 2.0
    }

    pub fn note_width(&self, col: usize) -> f32 {
        (self.col_width(col) - 4.0 * self.scale).max(2.0 * self.scale)
    }

    pub fn phrase_x(&self) -> f32 {
        ref_phrase_x() * self.scale
    }

    pub fn progress_bar_left(&self) -> f32 {
        self.strip_left()
    }

    pub fn progress_bar_width(&self) -> f32 {
        self.strip_width()
    }

    pub fn progress_bar_top(&self) -> f32 {
        696.0 * self.scale
    }

    pub fn ref_hud_right(&self) -> f32 {
        ref_hud_right_x() * self.scale
    }

    pub fn px(&self, ref_px: f32) -> f32 {
        ref_px * self.scale
    }

    pub fn combo_left(&self) -> f32 {
        self.ref_hud_right()
    }

    pub fn combo_top(&self) -> f32 {
        self.px(REF_COMBO_Y)
    }

    pub fn note_height(&self) -> f32 {
        14.0 * self.scale
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PlayfieldLayout>()
        .add_systems(Startup, init_playfield_layout)
        .add_systems(
            Update,
            sync_playfield_layout
                .run_if(in_state(AppState::Performance).or_else(in_state(AppState::SongLoading))),
        );
}

fn init_playfield_layout(
    mut layout: ResMut<PlayfieldLayout>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if let Ok(window) = windows.single() {
        *layout = PlayfieldLayout::from_window(window);
    }
}

fn sync_playfield_layout(
    mut resize_events: MessageReader<WindowResized>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut layout: ResMut<PlayfieldLayout>,
    mut dirty: Local<bool>,
) {
    if resize_events.read().next().is_some() {
        *dirty = true;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let next = PlayfieldLayout::from_window(window);
    if *dirty || next.scale != layout.scale || next.width != layout.width {
        *layout = next;
        *dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane_geometry::{COLUMN_COUNT, STRIP_REF_WIDTH};
    use dtx_ui::theme::REF_WIDTH;

    #[test]
    fn judge_below_lane_top() {
        let layout = PlayfieldLayout::default();
        assert!(layout.judge_y() > layout.lane_top());
    }

    #[test]
    fn lane_height_spans_to_judge() {
        let layout = PlayfieldLayout::default();
        assert!(
            (layout.lane_top() + layout.lane_height() - layout.judge_y()).abs() < 1.0,
            "lane bottom should align with judge line"
        );
    }

    #[test]
    fn strip_centered_at_default_scale() {
        let layout = PlayfieldLayout::default(); // scale 1.0 at 1280x720
        let expected_left = (REF_WIDTH - STRIP_REF_WIDTH) / 2.0; // 361.0
        assert!((layout.strip_left() - expected_left).abs() < 0.01);
        assert!((layout.col_left(0) - expected_left).abs() < 0.01);
        let last = COLUMN_COUNT - 1;
        assert!(
            (layout.col_left(last) + layout.col_width(last)
                - (expected_left + STRIP_REF_WIDTH))
                .abs()
                < 0.5,
            "strip right edge should be centered"
        );
    }

    #[test]
    fn columns_monotonic() {
        let layout = PlayfieldLayout::default();
        for c in 1..COLUMN_COUNT {
            assert!(layout.col_left(c) > layout.col_left(c - 1));
        }
    }

    #[test]
    fn strip_width_matches_ref() {
        let layout = PlayfieldLayout::default();
        assert!((layout.strip_width() - STRIP_REF_WIDTH).abs() < 0.5);
    }
}
