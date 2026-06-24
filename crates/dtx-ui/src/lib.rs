//! dtx-ui — Bevy UI/asset helpers + DTXManiaNX-derived constants.
//!
//! Strict-port-first. Phase 0 ships:
//! - DTXManiaNX constant: SCREEN_FADE_MS = 1500 (StageManager.cs:29)
//! - font loading helpers wrapping `bevy::prelude::AssetServer` and `bevy::text::Font`
//! - texture + audio handle wrappers for future skin/audio use
//! - per-stage label color helpers
//!
//! ## Reference
//! - `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs:29` — FadeDurationMs = 1500
//! - `references/DTXmaniaNX-BocuD/FDK/Skin/CSkin.cs` — skin subfolder resolution
//! - `references/DTXmaniaNX-BocuD/DTXMania/UI/UIFonts.cs` — font handle per stage
//!
//! ## What lands later
//! - Full skin subfolder swap (Phase 5 p5-1)
//! - Animation/transition helpers (Phase 3, M3.1 framebuffer snapshot)

#![allow(dead_code)] // Some helpers used by future Phase 0/1+ sub-acts.

pub mod core_sub_acts;
pub mod perf_common;

use bevy::asset::Handle;
use bevy::prelude::*;
use bevy::text::Font;

/// DTXManiaNX-derived fluidity constants. See ADR-0010 + `docs/BEVY_PATTERNS.md`.
/// IMPORTANT: these are the DTXManiaNX baseline values, NOT osu-lazer aspirational
/// ones. Do not "modernize" without an ADR override.
pub const SCREEN_FADE_MS: u32 = 1500; // StageManager.cs:29 FadeDurationMs = 1500f
pub const LOAD_HOLD_MS: u32 = 0; // DTXManiaNX has no load hold (no min wait)
pub const INPUT_LATENCY_MS: u32 = 16; // bevy_framepace target

/// Default font asset path (Bevy-shipped FiraMono subset).
///
/// Reference: CConfigIni.cs:99 (default font) — DTXManiaNX picks `texgyreadventor`,
/// but we don't ship a font file yet, so fall back to Bevy's default.
pub const DEFAULT_FONT_PATH: &str = "fonts/FiraMono-subset.ttf";

/// Default font size for SongSelect / Title / Config labels.
pub const DEFAULT_LABEL_PT: f32 = 18.0;
/// Default font size for HUD numerals (score, combo).
pub const DEFAULT_HUD_PT: f32 = 36.0;
/// Default font size for large title text.
pub const DEFAULT_TITLE_PT: f32 = 48.0;

/// Load a font from `path` and return a strong handle.
///
/// Reference: UIFonts.cs:25 — `CFontManagement.t指定したttfファイルを読み込む`.
/// Falls back to Bevy default if `path` is empty.
pub fn load_font_handle(asset_server: &AssetServer, path: &str) -> Handle<Font> {
    let owned: String = if path.is_empty() {
        DEFAULT_FONT_PATH.to_string()
    } else {
        path.to_string()
    };
    asset_server.load(owned)
}

/// Construct a `TextFont` with project-default sizing.
///
/// Reference: UIFonts.cs:15 — `pt` to px scale (BocuD uses pt; Bevy uses px).
/// Conversion: 1pt ≈ 1.333px at default DPI; rounded to integer for crispness.
pub fn default_text_font(size_pt: f32) -> TextFont {
    TextFont {
        font_size: pt_to_px(size_pt).into(),
        ..default()
    }
}

/// Convert points to Bevy-ui pixels (1pt = 1.333px).
///
/// Reference: UIFonts.cs:11 — pt→px conversion.
pub fn pt_to_px(pt: f32) -> f32 {
    (pt * 1.333).round()
}

/// Load a 2D texture from `path` and return a strong handle.
///
/// Reference: FDK/Skin/CSkin.cs:35 — `tSkin指定texture読込`.
pub fn load_texture_handle(asset_server: &AssetServer, path: &str) -> Handle<bevy::image::Image> {
    let owned = path.to_string();
    asset_server.load(owned)
}

/// Load an audio source from `path` and return a strong handle.
///
/// Reference: FDK/Sound/CSoundManager.cs:50 — `tサウンドファイルを読み込む`.
/// Returns the handle so the caller can play it via `bevy_kira_audio::Audio`.
pub fn load_audio_handle(
    asset_server: &AssetServer,
    path: &str,
) -> Handle<bevy::audio::AudioSource> {
    let owned = path.to_string();
    asset_server.load(owned)
}

/// Stage-label color helper. Different colors per state in BocuD.
///
/// Reference: CStageTitle.cs:18 (white default) + CActSelectArtistComment.cs:7 (silver).
pub fn stage_label_color(state: &str) -> Color {
    match state {
        "Title" => Color::srgb(0.95, 0.95, 0.95),
        "Config" => Color::srgb(0.85, 0.85, 0.95),
        "SongSelect" => Color::srgb(0.95, 0.95, 0.85),
        "Performance" => Color::srgb(0.95, 0.85, 0.85),
        "Result" => Color::srgb(0.85, 0.95, 0.85),
        _ => Color::WHITE,
    }
}

/// Build a centered absolute-positioned label node.
///
/// Reference: many (CStageTitle.cs:30, CStageConfig.cs:50, etc.) — all stages spawn labels
/// at fixed pixel coordinates. This helper is the minimum for v1 strict-port.
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

/// Root plugin; currently a no-op. UI widgets land in Phase 0 p0-2.
pub fn plugin() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_fade_ms_matches_dtx_mania_nx() {
        // StageManager.cs:29 — FadeDurationMs = 1500f
        assert_eq!(SCREEN_FADE_MS, 1500);
    }

    #[test]
    fn load_hold_ms_is_zero() {
        // DTXManiaNX has no load-hold. Be true to reference.
        assert_eq!(LOAD_HOLD_MS, 0);
    }

    #[test]
    fn input_latency_ms_target() {
        assert_eq!(INPUT_LATENCY_MS, 16);
    }

    #[test]
    fn default_font_path_not_empty() {
        assert!(!DEFAULT_FONT_PATH.is_empty());
        assert!(DEFAULT_FONT_PATH.ends_with(".ttf"));
    }

    #[test]
    fn default_label_pt_is_18() {
        // CStageTitle.cs:30 — main label font size = 18pt
        assert!((DEFAULT_LABEL_PT - 18.0).abs() < 0.01);
    }

    #[test]
    fn default_hud_pt_is_36() {
        // CActPerfDrumsScore.cs:46 — score digit size 36px
        assert!((DEFAULT_HUD_PT - 36.0).abs() < 0.01);
    }

    #[test]
    fn pt_to_px_18pt() {
        // 18pt → 24px (1.333x).
        assert!((pt_to_px(18.0) - 24.0).abs() < 0.01);
    }

    #[test]
    fn pt_to_px_36pt() {
        // 36pt → 48px.
        assert!((pt_to_px(36.0) - 48.0).abs() < 0.01);
    }

    #[test]
    fn default_text_font_has_size() {
        let f = default_text_font(24.0);
        let px = match f.font_size {
            bevy::text::FontSize::Px(px) => px,
            _ => panic!("expected Px size"),
        };
        // 24pt * 1.333 = 32px
        assert!((px - 32.0).abs() < 0.5);
    }

    #[test]
    fn stage_label_color_title_white() {
        // CStageTitle.cs:18 — white label
        let c = stage_label_color("Title");
        let v = c.to_srgba();
        assert!((v.red - 0.95).abs() < 0.01);
    }

    #[test]
    fn stage_label_color_unknown_white() {
        let c = stage_label_color("UnknownState");
        let v = c.to_srgba();
        assert!((v.red - 1.0).abs() < 0.01);
    }

    #[test]
    fn absolute_label_has_text_and_node() {
        let (_, text, font, color) =
            absolute_label(10.0, 20.0, 100.0, 30.0, "hello", 18.0, Color::WHITE);
        assert_eq!(text.0, "hello");
        let _ = font;
        let _ = color;
    }

    #[test]
    fn absolute_label_node_position() {
        let (node, _, _, _) = absolute_label(40.0, 13.0, 200.0, 50.0, "x", 18.0, Color::WHITE);
        // PositionType::Absolute at (40, 13) per CActPerfDrumsScore.cs:12-13
        assert!(matches!(node.position_type, PositionType::Absolute));
        if let Val::Px(left) = node.left {
            assert!((left - 40.0).abs() < 0.01);
        } else {
            panic!("expected Px");
        }
        if let Val::Px(top) = node.top {
            assert!((top - 13.0).abs() < 0.01);
        } else {
            panic!("expected Px");
        }
    }

    #[test]
    fn plugin_is_callable() {
        // No-op for Phase 0 p0-2. Widgets land later.
        plugin();
    }
}
