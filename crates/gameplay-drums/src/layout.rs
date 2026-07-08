//! Playfield layout — ref-resolution scaling for drums HUD (dtxpt-inspired).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use dtx_ui::theme::{REF_HEIGHT, REF_WIDTH};
use game_shell::AppState;

use crate::lanes::Lanes;

/// System set for the playfield-layout rebuild, so consumers that read column
/// geometry can order themselves after it within Update.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayfieldLayoutSync;

pub const REF_JUDGE_Y: f32 = 620.0;
pub const REF_BACKBOARD_PAD: f32 = 12.0;
/// Flush to the backboard's top pad so the playfield fills the screen top
/// edge (no empty band above the lanes).
pub const REF_LANE_TOP: f32 = REF_BACKBOARD_PAD;
pub const REF_LANE_HEIGHT: f32 = REF_JUDGE_Y - REF_LANE_TOP;
pub const REF_KEY_CAP_H: f32 = 60.0;
/// Pads sit flush under the judge line (GITADORA-style, no black band).
pub const REF_KEY_VIZ_OFFSET: f32 = 5.0;

pub const REF_COMBO_Y: f32 = 72.0;

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PlayfieldLayout {
    pub scale: f32,
    /// Absolute window-space offset of the stage rect (px); `(0,0)` = identity.
    pub origin: Vec2,
    pub width: f32,
    pub height: f32,
    strip_ref_width: f32,
    cols: Vec<(f32, f32)>,
}

impl Default for PlayfieldLayout {
    fn default() -> Self {
        Self::from_size(REF_WIDTH, REF_HEIGHT, &Lanes::default())
    }
}

impl PlayfieldLayout {
    pub fn from_window(window: &Window, lanes: &Lanes) -> Self {
        Self::from_size(window.width(), window.height(), lanes)
    }

    pub fn from_size(width: f32, height: f32, lanes: &Lanes) -> Self {
        Self::from_rect(
            crate::stage_rect::StageRect::full(Vec2::new(width, height)),
            lanes,
        )
    }

    pub fn from_rect(rect: crate::stage_rect::StageRect, lanes: &Lanes) -> Self {
        let scale = (rect.size.x / REF_WIDTH).min(rect.size.y / REF_HEIGHT);
        let cols = (0..lanes.count())
            .map(|i| (lanes.ref_offset(i), lanes.ref_width(i)))
            .collect();
        Self {
            scale,
            origin: rect.origin,
            width: rect.size.x,
            height: rect.size.y,
            strip_ref_width: lanes.strip_ref_width(),
            cols,
        }
    }

    pub fn ref_strip_left(&self) -> f32 {
        (REF_WIDTH - self.strip_ref_width) / 2.0
    }

    pub fn ref_strip_width(&self) -> f32 {
        self.strip_ref_width
    }

    pub fn ref_phrase_x(&self) -> f32 {
        self.ref_strip_left() + self.strip_ref_width + 15.0
    }

    pub fn ref_hud_right_x(&self) -> f32 {
        self.ref_strip_left() + self.strip_ref_width + 24.0
    }

    pub fn col_count(&self) -> usize {
        self.cols.len()
    }

    pub fn judge_y(&self) -> f32 {
        REF_JUDGE_Y * self.scale + self.origin.y
    }

    pub fn lane_top(&self) -> f32 {
        REF_LANE_TOP * self.scale + self.origin.y
    }

    pub fn lane_height(&self) -> f32 {
        REF_LANE_HEIGHT * self.scale
    }

    pub fn col_left(&self, col: usize) -> f32 {
        let off = self.cols.get(col).map(|c| c.0).unwrap_or(0.0);
        (self.ref_strip_left() + off) * self.scale + self.origin.x
    }

    pub fn col_width(&self, col: usize) -> f32 {
        let w = self.cols.get(col).map(|c| c.1).unwrap_or(0.0);
        w * self.scale
    }

    pub fn strip_left(&self) -> f32 {
        self.ref_strip_left() * self.scale + self.origin.x
    }

    pub fn strip_width(&self) -> f32 {
        self.strip_ref_width * self.scale
    }

    /// NX prints measure# just right of the strip (`CStagePerfDrumsScreen.cs:3588`).
    pub fn measure_label_left(&self) -> f32 {
        self.strip_left() + self.strip_width() + 8.0 * self.scale
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
        self.ref_phrase_x() * self.scale + self.origin.x
    }

    pub fn progress_bar_left(&self) -> f32 {
        self.strip_left()
    }

    pub fn progress_bar_width(&self) -> f32 {
        self.strip_width()
    }

    pub fn progress_bar_top(&self) -> f32 {
        696.0 * self.scale + self.origin.y
    }

    pub fn ref_hud_right(&self) -> f32 {
        self.ref_hud_right_x() * self.scale + self.origin.x
    }

    pub fn px(&self, ref_px: f32) -> f32 {
        ref_px * self.scale
    }

    pub fn combo_left(&self) -> f32 {
        self.ref_hud_right()
    }

    pub fn combo_top(&self) -> f32 {
        self.px(REF_COMBO_Y) + self.origin.y
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
                .in_set(PlayfieldLayoutSync)
                .run_if(in_state(AppState::Performance).or_else(in_state(AppState::SongLoading))),
        );
}

fn init_playfield_layout(
    mut layout: ResMut<PlayfieldLayout>,
    lanes: Res<Lanes>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if let Ok(window) = windows.single() {
        *layout = PlayfieldLayout::from_window(window, &lanes);
    }
}

fn sync_playfield_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    lanes: Res<Lanes>,
    mut layout: ResMut<PlayfieldLayout>,
) {
    // The playfield always lays out at FULL WINDOW size. The Customize surface's
    // "shrink into a miniature" is a single UiTransform on `HudRoot` (osu
    // SetCustomRect model — see `stage_rect::apply_stage_transform`), NOT a
    // layout-space rescale, so this only rebuilds on a real resize or lane edit.
    let Ok(window) = windows.single() else {
        return;
    };
    let want = PlayfieldLayout::from_window(window, &lanes);
    if *layout != want {
        *layout = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;
    use dtx_ui::theme::REF_WIDTH;

    fn classic_layout() -> PlayfieldLayout {
        PlayfieldLayout::from_size(REF_WIDTH, REF_HEIGHT, &Lanes::default())
    }

    #[test]
    fn judge_below_lane_top() {
        let layout = classic_layout();
        assert!(layout.judge_y() > layout.lane_top());
    }

    #[test]
    fn lane_height_spans_to_judge() {
        let layout = classic_layout();
        assert!(
            (layout.lane_top() + layout.lane_height() - layout.judge_y()).abs() < 1.0,
            "lane bottom should align with judge line"
        );
    }

    #[test]
    fn strip_centered_at_default_scale() {
        let layout = classic_layout();
        let expected_left = (REF_WIDTH - 558.0) / 2.0;
        assert!((layout.strip_left() - expected_left).abs() < 0.01);
        assert!((layout.col_left(0) - expected_left).abs() < 0.01);
        let last = layout.col_count() - 1;
        assert!(
            (layout.col_left(last) + layout.col_width(last) - (expected_left + 558.0)).abs() < 0.5,
            "strip right edge should be centered"
        );
    }

    #[test]
    fn columns_monotonic() {
        let layout = classic_layout();
        for c in 1..layout.col_count() {
            assert!(layout.col_left(c) > layout.col_left(c - 1));
        }
    }

    #[test]
    fn strip_width_matches_ref() {
        let layout = classic_layout();
        assert!((layout.strip_width() - 558.0).abs() < 0.5);
    }

    #[test]
    fn from_rect_full_window_equals_from_size() {
        let lanes = Lanes::default();
        let win = bevy::math::Vec2::new(1600.0, 900.0);
        let from_size = PlayfieldLayout::from_size(win.x, win.y, &lanes);
        let from_rect = PlayfieldLayout::from_rect(crate::stage_rect::StageRect::full(win), &lanes);
        assert_eq!(
            from_rect, from_size,
            "identity rect must reproduce from_size exactly"
        );
    }

    #[test]
    fn from_rect_offset_shifts_all_x_by_origin() {
        let lanes = Lanes::default();
        let win = bevy::math::Vec2::new(1600.0, 900.0);
        let base = PlayfieldLayout::from_rect(crate::stage_rect::StageRect::full(win), &lanes);
        let shifted = PlayfieldLayout::from_rect(
            crate::stage_rect::StageRect {
                origin: bevy::math::Vec2::new(220.0, 0.0),
                size: win,
            },
            &lanes,
        );
        assert_eq!(shifted.scale, base.scale);
        assert!((shifted.col_left(0) - (base.col_left(0) + 220.0)).abs() < 0.01);
        assert!((shifted.strip_left() - (base.strip_left() + 220.0)).abs() < 0.01);
    }

    #[test]
    fn wider_arrangement_widens_and_recenters_strip() {
        let section = dtx_layout::LanesSection {
            preset: dtx_layout::LanePreset::Custom,
            order: Some(
                [
                    "LC", "HH", "HHO", "LP", "SD", "HT", "BD", "LT", "FT", "CY", "RD",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ),
            map: Some([("HHO".to_string(), "HHO".to_string())].into()),
            ..Default::default()
        };
        let lanes = Lanes(section.resolve());
        let layout = PlayfieldLayout::from_size(REF_WIDTH, REF_HEIGHT, &lanes);
        assert_eq!(layout.col_count(), 11);
        assert!((layout.strip_width() - (558.0 + 49.0)).abs() < 0.01);
        let expected_left = (REF_WIDTH - (558.0 + 49.0)) / 2.0;
        assert!((layout.strip_left() - expected_left).abs() < 0.01);
    }
}
