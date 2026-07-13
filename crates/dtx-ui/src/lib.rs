//! dtx-ui — Bevy UI widgets, theme, transitions (ADR-0014).
//!
//! osu-inspired UX redesign. Game mechanics stay BocuD-ported; visuals are new.

pub mod accessibility;
pub mod core_sub_acts;
pub mod easing;
pub mod motion;
pub mod parallax;
pub mod perf_common;
pub mod theme;
pub mod transition;
pub mod tween;
pub mod widget;

use bevy::asset::Handle;
use bevy::prelude::*;
use bevy::text::Font;

pub use accessibility::{AccessibilityPolicy, FlashDecision, MotionDecision, StartupConfigWarning};
pub use theme::{Theme, ThemeResource, REF_HEIGHT, REF_WIDTH, SCREEN_TRANSITION_MS};
pub use transition::{FadePhase, ScreenFade, TransitionOverlay};

/// Legacy alias — ADR-0014 uses 300ms OutQuint (not 1500ms BocuD snapshot).
pub const SCREEN_FADE_MS: u32 = 300;
pub const LOAD_HOLD_MS: u32 = 0;
pub const INPUT_LATENCY_MS: u32 = 16;

pub const DEFAULT_FONT_PATH: &str = "fonts/FiraMono-subset.ttf";
pub const DEFAULT_LABEL_PT: f32 = 18.0;
pub const DEFAULT_HUD_PT: f32 = 36.0;
pub const DEFAULT_TITLE_PT: f32 = 48.0;

pub fn load_font_handle(asset_server: &AssetServer, path: &str) -> Handle<Font> {
    let owned: String = if path.is_empty() {
        DEFAULT_FONT_PATH.to_string()
    } else {
        path.to_string()
    };
    asset_server.load(owned)
}

pub fn default_text_font(size_pt: f32) -> TextFont {
    TextFont {
        font_size: pt_to_px(size_pt).into(),
        ..default()
    }
}

pub fn pt_to_px(pt: f32) -> f32 {
    (pt * 1.333).round()
}

pub fn load_texture_handle(asset_server: &AssetServer, path: &str) -> Handle<bevy::image::Image> {
    asset_server.load(path.to_string())
}

pub fn load_audio_handle(
    asset_server: &AssetServer,
    path: &str,
) -> Handle<bevy::audio::AudioSource> {
    asset_server.load(path.to_string())
}

pub fn stage_label_color(state: &str) -> Color {
    let theme = Theme::default();
    match state {
        "Title" | "SongSelect" | "Performance" | "Result" | "Config" => theme.text_primary,
        _ => theme.text_secondary,
    }
}

pub fn absolute_label(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: impl Into<String>,
    font_size_pt: f32,
    color: Color,
) -> (Node, Text, TextFont, TextColor) {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        },
        Text::new(text),
        default_text_font(font_size_pt),
        TextColor(color),
    )
}

pub fn plugin(app: &mut App) {
    app.init_resource::<AccessibilityPolicy>()
        .init_resource::<StartupConfigWarning>()
        .init_resource::<ThemeResource>()
        .init_resource::<widget::density_graph::DensityData>()
        .init_resource::<widget::difficulty_grid::DifficultyGridData>()
        .init_resource::<widget::play_history::PlayHistoryData>()
        .init_resource::<widget::song_wheel::WheelSpring>()
        .add_plugins((
            transition::plugin,
            bevy_tweening::TweeningPlugin,
            widget::controls::ControlsPlugin,
        ))
        .add_message::<dtx_audio::PreviewSwapEvent>()
        .add_systems(
            Update,
            (
                widget::album_art::album_art_tween_system,
                widget::album_art::apply_album_art_opacity,
                parallax::parallax_info_tween_system,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                motion::enter_choreo_system,
                motion::beat_pulse_system,
                widget::density_graph::density_graph_system,
            ),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;

    #[test]
    fn plugin_builds_with_theme_and_tweening() {
        let mut app = App::new();
        app.add_plugins(plugin);
        assert!(app.world().get_resource::<ThemeResource>().is_some());
        assert!(app.world().get_resource::<AccessibilityPolicy>().is_some());
        assert!(app.world().get_resource::<StartupConfigWarning>().is_some());
        assert!(app.world().get_resource::<ScreenFade>().is_some());
    }

    #[test]
    fn screen_fade_ms_is_300() {
        assert_eq!(SCREEN_FADE_MS, 300);
    }

    #[test]
    fn pt_to_px_18pt() {
        assert!((pt_to_px(18.0) - 24.0).abs() < 0.01);
    }
}
