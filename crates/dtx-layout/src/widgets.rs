//! HUD widget placement model (display/arrangement axis for widgets).
//!
//! Anchor/origin use a 3×3 grid; `resolve_top_left` computes the ref-px top-left
//! of a widget given its anchor, origin, natural size, and offset within a
//! parent rect. Pure — no bevy.

use serde::{Deserialize, Serialize};

/// 9-point anchor/origin grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor9 {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor9 {
    /// Fractional position within a unit rect: (0,0)=TopLeft .. (1,1)=BottomRight.
    pub fn frac(self) -> (f32, f32) {
        let x = match self {
            Anchor9::TopLeft | Anchor9::CenterLeft | Anchor9::BottomLeft => 0.0,
            Anchor9::TopCenter | Anchor9::Center | Anchor9::BottomCenter => 0.5,
            Anchor9::TopRight | Anchor9::CenterRight | Anchor9::BottomRight => 1.0,
        };
        let y = match self {
            Anchor9::TopLeft | Anchor9::TopCenter | Anchor9::TopRight => 0.0,
            Anchor9::CenterLeft | Anchor9::Center | Anchor9::CenterRight => 0.5,
            Anchor9::BottomLeft | Anchor9::BottomCenter | Anchor9::BottomRight => 1.0,
        };
        (x, y)
    }

    pub const ALL: [Anchor9; 9] = [
        Anchor9::TopLeft,
        Anchor9::TopCenter,
        Anchor9::TopRight,
        Anchor9::CenterLeft,
        Anchor9::Center,
        Anchor9::CenterRight,
        Anchor9::BottomLeft,
        Anchor9::BottomCenter,
        Anchor9::BottomRight,
    ];
}

/// Which anchor space the widget lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnchorSpace {
    /// Anchored to the full screen ref rect (1280×720).
    Screen,
    /// Anchored to the playfield strip rect (dynamic; resolved by the consumer).
    Playfield,
}

impl Default for AnchorSpace {
    fn default() -> Self {
        Self::Screen
    }
}

/// How the widget's position is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// v1 semantics: widget stays at its code-natural position, translated by
    /// `offset` (ref-px). Anchor/origin/scale are inert (scale renders as 1).
    #[default]
    Natural,
    /// Absolute: `resolve_top_left(anchor, origin, size, scale, offset·s, parent)`.
    Anchored,
}

/// The gameplay HUD widgets that can be arranged. Serialized kebab-case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetKind {
    ScorePanel,
    Combo,
    JudgmentPopup,
    PhraseMeter,
    SongProgress,
    NowPlaying,
    LiveGraph,
    SpeedReadout,
    FrameChrome,
    Playfield,
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 10] = [
        WidgetKind::ScorePanel,
        WidgetKind::Combo,
        WidgetKind::JudgmentPopup,
        WidgetKind::PhraseMeter,
        WidgetKind::SongProgress,
        WidgetKind::NowPlaying,
        WidgetKind::LiveGraph,
        WidgetKind::SpeedReadout,
        WidgetKind::FrameChrome,
        WidgetKind::Playfield,
    ];

    /// Human-readable name for the editor sidebar.
    pub fn display_name(self) -> &'static str {
        match self {
            WidgetKind::ScorePanel => "Score Panel",
            WidgetKind::Combo => "Combo",
            WidgetKind::JudgmentPopup => "Judgment Popup",
            WidgetKind::PhraseMeter => "Phrase Meter",
            WidgetKind::SongProgress => "Song Progress",
            WidgetKind::NowPlaying => "Now Playing",
            WidgetKind::LiveGraph => "Live Graph",
            WidgetKind::SpeedReadout => "Speed Readout",
            WidgetKind::FrameChrome => "Frame Chrome",
            WidgetKind::Playfield => "Playfield",
        }
    }
}

pub const MIN_WIDGET_SCALE: f32 = 0.25;
pub const MAX_WIDGET_SCALE: f32 = 3.0;

/// A placed widget instance (one per kind in v1).
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetInstance {
    pub kind: WidgetKind,
    pub space: AnchorSpace,
    pub placement: Placement,
    pub anchor: Anchor9,
    pub origin: Anchor9,
    /// When true (default) the anchor auto-follows the widget's center across
    /// the parent's thirds during a drag; a manual anchor pick pins it (false).
    pub anchor_auto: bool,
    /// Ref-px offset from the anchored/origin-aligned base position.
    pub offset: (f32, f32),
    /// Uniform scale, clamped [MIN_WIDGET_SCALE, MAX_WIDGET_SCALE].
    pub scale: f32,
    pub z: i32,
    pub visible_play: bool,
    pub visible_practice: bool,
}

/// Resolve the ref-px top-left of a widget of natural size `size` placed at
/// `offset` with `anchor`/`origin` inside a parent rect `parent` (x, y, w, h).
///
/// anchor point A = parent.origin + anchor.frac * parent.size
/// origin point O within the widget = origin.frac * (size * scale)
/// top-left = A + offset - O
pub fn resolve_top_left(
    anchor: Anchor9,
    origin: Anchor9,
    size: (f32, f32),
    scale: f32,
    offset: (f32, f32),
    parent: (f32, f32, f32, f32),
) -> (f32, f32) {
    let (px, py, pw, ph) = parent;
    let (af_x, af_y) = anchor.frac();
    let (of_x, of_y) = origin.frac();
    let ax = px + af_x * pw;
    let ay = py + af_y * ph;
    let ox = of_x * size.0 * scale;
    let oy = of_y * size.1 * scale;
    (ax + offset.0 - ox, ay + offset.1 - oy)
}

/// Inverse of `resolve_top_left`: the offset that places the widget's top-left
/// at `top_left` given everything else. Same unit convention as resolve.
pub fn offset_for_top_left(
    anchor: Anchor9,
    origin: Anchor9,
    size: (f32, f32),
    scale: f32,
    top_left: (f32, f32),
    parent: (f32, f32, f32, f32),
) -> (f32, f32) {
    let (px, py, pw, ph) = parent;
    let (af_x, af_y) = anchor.frac();
    let (of_x, of_y) = origin.frac();
    (
        top_left.0 - (px + af_x * pw) + of_x * size.0 * scale,
        top_left.1 - (py + af_y * ph) + of_y * size.1 * scale,
    )
}

/// Nearest ninth for a fractional position within the parent (thirds rule:
/// <1/3 → start, 1/3..=2/3 → center, >2/3 → end, per axis).
pub fn nearest_anchor(frac_x: f32, frac_y: f32) -> Anchor9 {
    let col = if frac_x < 1.0 / 3.0 {
        0
    } else if frac_x <= 2.0 / 3.0 {
        1
    } else {
        2
    };
    let row = if frac_y < 1.0 / 3.0 {
        0
    } else if frac_y <= 2.0 / 3.0 {
        1
    } else {
        2
    };
    Anchor9::ALL[row * 3 + col]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f32, f32, f32, f32) = (0.0, 0.0, 1280.0, 720.0);

    #[test]
    fn frac_corners() {
        assert_eq!(Anchor9::TopLeft.frac(), (0.0, 0.0));
        assert_eq!(Anchor9::Center.frac(), (0.5, 0.5));
        assert_eq!(Anchor9::BottomRight.frac(), (1.0, 1.0));
    }

    #[test]
    fn top_left_anchor_origin_is_pure_offset() {
        let tl = resolve_top_left(
            Anchor9::TopLeft,
            Anchor9::TopLeft,
            (100.0, 40.0),
            1.0,
            (16.0, 78.0),
            SCREEN,
        );
        assert_eq!(tl, (16.0, 78.0));
    }

    #[test]
    fn center_center_zero_offset_centers_widget() {
        let (l, t) = resolve_top_left(
            Anchor9::Center,
            Anchor9::Center,
            (200.0, 100.0),
            1.0,
            (0.0, 0.0),
            SCREEN,
        );
        assert!((l - (640.0 - 100.0)).abs() < 0.01);
        assert!((t - (360.0 - 50.0)).abs() < 0.01);
    }

    #[test]
    fn bottom_right_anchor_origin_pins_to_corner() {
        let (l, t) = resolve_top_left(
            Anchor9::BottomRight,
            Anchor9::BottomRight,
            (120.0, 30.0),
            1.0,
            (0.0, 0.0),
            SCREEN,
        );
        assert!((l - (1280.0 - 120.0)).abs() < 0.01);
        assert!((t - (720.0 - 30.0)).abs() < 0.01);
    }

    #[test]
    fn scale_grows_from_origin() {
        let (l, t) = resolve_top_left(
            Anchor9::TopLeft,
            Anchor9::BottomRight,
            (100.0, 50.0),
            2.0,
            (0.0, 0.0),
            SCREEN,
        );
        assert!((l - (0.0 - 200.0)).abs() < 0.01);
        assert!((t - (0.0 - 100.0)).abs() < 0.01);
    }

    #[test]
    fn all_nine_anchors_have_distinct_points() {
        let mut seen = std::collections::HashSet::new();
        for a in Anchor9::ALL {
            let (fx, fy) = a.frac();
            assert!(seen.insert(((fx * 2.0) as i32, (fy * 2.0) as i32)));
        }
        assert_eq!(seen.len(), 9);
    }

    #[test]
    fn offset_for_top_left_round_trips_resolve() {
        let parent = (100.0, 50.0, 800.0, 600.0);
        for anchor in Anchor9::ALL {
            for origin in Anchor9::ALL {
                let offset =
                    offset_for_top_left(anchor, origin, (120.0, 40.0), 1.5, (300.0, 200.0), parent);
                let tl = resolve_top_left(anchor, origin, (120.0, 40.0), 1.5, offset, parent);
                assert!(
                    (tl.0 - 300.0).abs() < 0.001 && (tl.1 - 200.0).abs() < 0.001,
                    "{anchor:?}/{origin:?}"
                );
            }
        }
    }

    #[test]
    fn placement_default_is_natural() {
        assert_eq!(Placement::default(), Placement::Natural);
    }

    #[test]
    fn nearest_anchor_nine_regions() {
        assert_eq!(nearest_anchor(0.1, 0.1), Anchor9::TopLeft);
        assert_eq!(nearest_anchor(0.5, 0.1), Anchor9::TopCenter);
        assert_eq!(nearest_anchor(0.9, 0.1), Anchor9::TopRight);
        assert_eq!(nearest_anchor(0.1, 0.5), Anchor9::CenterLeft);
        assert_eq!(nearest_anchor(0.5, 0.5), Anchor9::Center);
        assert_eq!(nearest_anchor(0.9, 0.5), Anchor9::CenterRight);
        assert_eq!(nearest_anchor(0.1, 0.9), Anchor9::BottomLeft);
        assert_eq!(nearest_anchor(0.5, 0.9), Anchor9::BottomCenter);
        assert_eq!(nearest_anchor(0.9, 0.9), Anchor9::BottomRight);
    }

    #[test]
    fn widget_kind_serde_kebab() {
        let s = toml::to_string(&std::collections::BTreeMap::from([(
            "k",
            WidgetKind::ScorePanel,
        )]))
        .unwrap();
        assert_eq!(s.trim(), r#"k = "score-panel""#);
    }
}
