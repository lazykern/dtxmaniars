//! Playfield layout — ref-resolution scaling for drums HUD (dtxpt-inspired).

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResized};
use dtx_ui::theme::{REF_HEIGHT, REF_WIDTH};
use game_shell::AppState;

use crate::lane_map::LANE_COUNT;

pub const REF_JUDGE_Y: f32 = 620.0;
pub const REF_LANE_TOP: f32 = 80.0;
pub const REF_LANE_HEIGHT: f32 = 540.0;
pub const REF_LANE_LEFT: f32 = 200.0;
pub const REF_LANE_W: f32 = 80.0;
pub const REF_LABEL_OFFSET: f32 = 28.0;
pub const REF_KEY_VIZ_OFFSET: f32 = 64.0;
pub const REF_BACKBOARD_PAD: f32 = 12.0;

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

    pub fn lane_left(&self, lane: usize) -> f32 {
        (REF_LANE_LEFT + lane as f32 * REF_LANE_W) * self.scale
    }

    pub fn lane_width(&self) -> f32 {
        REF_LANE_W * self.scale
    }

    pub fn lane_strip_width(&self) -> f32 {
        LANE_COUNT as f32 * self.lane_width()
    }

    pub fn lane_strip_left(&self) -> f32 {
        self.lane_left(0)
    }

    /// NX prints measure# at x≈858 (`CStagePerfDrumsScreen.cs:3588`).
    pub fn measure_label_left(&self) -> f32 {
        self.lane_strip_left() + self.lane_strip_width() + 8.0 * self.scale
    }

    pub fn label_top(&self) -> f32 {
        self.judge_y() + REF_LABEL_OFFSET * self.scale
    }

    pub fn key_viz_top(&self) -> f32 {
        self.judge_y() + REF_KEY_VIZ_OFFSET * self.scale
    }

    pub fn key_cap_height(&self) -> f32 {
        36.0 * self.scale
    }

    pub fn backboard_left(&self) -> f32 {
        self.lane_strip_left() - REF_BACKBOARD_PAD * self.scale
    }

    pub fn backboard_top(&self) -> f32 {
        self.lane_top() - REF_BACKBOARD_PAD * self.scale
    }

    pub fn backboard_width(&self) -> f32 {
        self.lane_strip_width() + REF_BACKBOARD_PAD * self.scale * 2.0
    }

    pub fn backboard_height(&self) -> f32 {
        self.lane_height() + REF_BACKBOARD_PAD * self.scale * 2.0
    }

    pub fn note_width(&self) -> f32 {
        self.lane_width() - 8.0 * self.scale
    }

    pub fn note_height(&self) -> f32 {
        8.0 * self.scale
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
}
