//! CDTX nested types (BocuD CDTX.cs:65-200) — real ports of inner classes.
//!
//! `non_snake_case` is allowed per ADR-0010: BocuD field/method names
//! (`db_実BPM`, `n_番号`, etc.) are preserved verbatim for port fidelity
//! and to keep diffs against the reference small.
#![allow(non_snake_case)]
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1-7295`
//!
//! Replaces constants-only nested types with real structures used by CDTX
//! for BGA panels, AVI panels, BPM tables, WAV files, BMP files, etc.

use std::path::PathBuf;

/// 2D point (BocuD Point struct used throughout CDTX).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub const ZERO: Self = Self { x: 0, y: 0 };
}

/// 2D size (BocuD Size struct).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

/// Rectangle (BocuD Rectangle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rectangle {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether a point is inside the rectangle.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.width && p.y >= self.y && p.y < self.y + self.height
    }

    /// Area (BocuD Rectangle.cs:Area).
    pub fn area(&self) -> i64 {
        self.width as i64 * self.height as i64
    }
}

/// AVI panel (BocuD CDTX.cs:65-91).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CAVIPAN {
    pub n_avi_number: i32,
    pub n_移動時間ct: i32,
    pub n_番号: i32,
    pub pt_動画側開始位置: Point,
    pub pt_動画側終了位置: Point,
    pub pt_表示側開始位置: Point,
    pub pt_表示側終了位置: Point,
    pub sz_開始サイズ: Size,
    pub sz_終了サイズ: Size,
}

impl CAVIPAN {
    pub fn new(n_avi_number: i32, n_番号: i32) -> Self {
        Self {
            n_avi_number,
            n_番号,
            ..Default::default()
        }
    }

    /// String representation (BocuD CDTX.cs:77-91).
    pub fn to_bocuD_string(&self) -> String {
        format!(
            "CAVIPAN(avi={}, num={}, dur={})",
            self.n_avi_number, self.n_番号, self.n_移動時間ct
        )
    }
}

/// BGA frame (BocuD CDTX.cs:93-110).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CBGA {
    pub n_bmp_number: i32,
    pub n_番号: i32,
    pub pt_画像側右下座標: Point,
    pub pt_画像側左上座標: Point,
    pub pt_表示座標: Point,
}

impl CBGA {
    pub fn new(n_bmp_number: i32, n_番号: i32) -> Self {
        Self {
            n_bmp_number,
            n_番号,
            ..Default::default()
        }
    }
}

/// BGA panel (BocuD CDTX.cs:112-150).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CBGAPAN {
    pub n_bmp_number: i32,
    pub n_移動時間ct: i32,
    pub n_番号: i32,
    pub pt_画像側開始位置: Point,
    pub pt_画像側終了位置: Point,
    pub pt_表示側開始位置: Point,
    pub pt_表示側終了位置: Point,
    pub sz_開始サイズ: Size,
    pub sz_終了サイズ: Size,
}

impl CBGAPAN {
    pub fn new(n_bmp_number: i32, n_番号: i32) -> Self {
        Self {
            n_bmp_number,
            n_番号,
            ..Default::default()
        }
    }
}

/// BMP base class (BocuD CDTX.cs:152-220).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CBMPbase {
    /// Filename without extension.
    pub filename: String,
    /// Path on disk.
    pub path: PathBuf,
    /// Image width in pixels.
    pub width: i32,
    /// Image height in pixels.
    pub height: i32,
}

impl CBMPbase {
    pub fn new(filename: String, path: PathBuf) -> Self {
        Self {
            filename,
            path,
            width: 0,
            height: 0,
        }
    }
}

/// BMP wrapper (BocuD CDTX.cs:CBMP).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CBMP {
    pub base: CBMPbase,
}

impl CBMP {
    pub fn new(filename: String, path: PathBuf) -> Self {
        Self {
            base: CBMPbase::new(filename, path),
        }
    }

    /// Aspect ratio (BocuD CBMP.cs:AspectRatio).
    pub fn aspect_ratio(&self) -> f32 {
        if self.base.height == 0 {
            0.0
        } else {
            self.base.width as f32 / self.base.height as f32
        }
    }
}

/// BMP texture (BocuD CDTX.cs:CBMPTEX).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CBMPTEX {
    pub base: CBMPbase,
    /// Texture handle ID (BocuD CBMPTEX.cs:nTexture).
    pub n_texture: u32,
}

impl CBMPTEX {
    pub fn new(filename: String, path: PathBuf, n_texture: u32) -> Self {
        Self {
            base: CBMPbase::new(filename, path),
            n_texture,
        }
    }
}

/// BPM table entry (BocuD CDTX.cs:CBPM).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CBPM {
    pub n_bpm: i32,
    pub n_内部番号: i32,
    pub db_実BPM: f32,
}

impl CBPM {
    pub fn new(n_bpm: i32, db_実BPM: f32) -> Self {
        Self {
            n_bpm,
            n_内部番号: 0,
            db_実BPM,
        }
    }
}

/// WAV wrapper (BocuD CDTX.cs:CWAV).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CWAV {
    pub filename: String,
    pub path: PathBuf,
    /// Whether the WAV is currently loaded.
    pub loaded: bool,
    /// Sound handle ID (BocuD CWAV.cs:nHandle).
    pub n_handle: u32,
}

impl CWAV {
    pub fn new(filename: String, path: PathBuf) -> Self {
        Self {
            filename,
            path,
            loaded: false,
            n_handle: 0,
        }
    }

    /// Mark as loaded with a handle.
    pub fn mark_loaded(&mut self, handle: u32) {
        self.loaded = true;
        self.n_handle = handle;
    }
}

/// Lane intersection (BocuD CDTX.cs:STLANEINT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct STLANEINT {
    pub n_小節: i32,
    pub n_チップ: i32,
}

impl STLANEINT {
    pub const fn new(n_小節: i32, n_チップ: i32) -> Self {
        Self {
            n_小節, n_チップ
        }
    }
}

/// Result state (BocuD CDTX.cs:STRESULT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct STRESULT {
    pub n_スコア: i32,
    pub n_連打数: i32,
    pub n_最大連打数: i32,
    pub n_正解数: i32,
    pub n_不正解数: i32,
    pub n_perfect: i32,
    pub n_great: i32,
    pub n_good: i32,
    pub n_ok: i32,
    pub n_miss: i32,
    pub b_クリア: bool,
}

impl STRESULT {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total chips (BocuD STRESULT.cs:TotalChips).
    pub fn total_chips(&self) -> i32 {
        self.n_perfect + self.n_great + self.n_good + self.n_ok + self.n_miss
    }

    /// Hit ratio (BocuD STRESULT.cs:HitRatio).
    pub fn hit_ratio(&self) -> f32 {
        let total = self.total_chips();
        if total == 0 {
            0.0
        } else {
            (self.n_perfect + self.n_great + self.n_good) as f32 / total as f32
        }
    }

    /// Whether this result is a full combo (BocuD STRESULT.cs:IsFullCombo).
    pub fn is_full_combo(&self) -> bool {
        self.n_miss == 0 && self.n_ok == 0 && self.total_chips() > 0
    }

    /// Whether this result is all perfect (BocuD STRESULT.cs:IsAllPerfect).
    pub fn is_all_perfect(&self) -> bool {
        self.total_chips() > 0
            && self.n_great == 0
            && self.n_good == 0
            && self.n_ok == 0
            && self.n_miss == 0
    }
}

/// Chips in chart flag (BocuD CDTX.cs:STHASCHIPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct STHASCHIPS {
    pub b_楽器: bool,
    pub b_小節線: bool,
    pub b_拍線: bool,
    pub b_小節内拍線: bool,
    pub b_休符: bool,
    pub b_全自動: bool,
}

impl STHASCHIPS {
    pub const fn new() -> Self {
        Self {
            b_楽器: false,
            b_小節線: false,
            b_拍線: false,
            b_小節内拍線: false,
            b_休符: false,
            b_全自動: false,
        }
    }

    /// Whether any flag is set.
    pub fn any(&self) -> bool {
        self.b_楽器
            || self.b_小節線
            || self.b_拍線
            || self.b_小節内拍線
            || self.b_休符
            || self.b_全自動
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_default_and_new() {
        assert_eq!(Point::default(), Point::ZERO);
        assert_eq!(Point::new(1, 2), Point { x: 1, y: 2 });
    }

    #[test]
    fn size_new() {
        let s = Size::new(10, 20);
        assert_eq!(s.width, 10);
        assert_eq!(s.height, 20);
    }

    #[test]
    fn rectangle_contains() {
        let r = Rectangle::new(0, 0, 10, 10);
        assert!(r.contains(Point::new(5, 5)));
        assert!(!r.contains(Point::new(20, 20)));
        assert!(!r.contains(Point::new(-1, 0)));
    }

    #[test]
    fn rectangle_area() {
        let r = Rectangle::new(0, 0, 10, 20);
        assert_eq!(r.area(), 200);
    }

    #[test]
    fn cavipan_new_and_string() {
        let p = CAVIPAN::new(1, 5);
        assert_eq!(p.n_avi_number, 1);
        assert_eq!(p.n_番号, 5);
        let s = p.to_bocuD_string();
        assert!(s.contains("CAVIPAN"));
        assert!(s.contains("avi=1"));
    }

    #[test]
    fn cbga_new() {
        let g = CBGA::new(7, 0);
        assert_eq!(g.n_bmp_number, 7);
    }

    #[test]
    fn cbgapan_new() {
        let p = CBGAPAN::new(7, 1);
        assert_eq!(p.n_bmp_number, 7);
    }

    #[test]
    fn cbmp_aspect_ratio() {
        let mut m = CBMP::new("test".into(), PathBuf::from("/t.bmp"));
        m.base.width = 1920;
        m.base.height = 1080;
        assert!((m.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn cbmp_aspect_zero_height() {
        let m = CBMP::default();
        assert_eq!(m.aspect_ratio(), 0.0);
    }

    #[test]
    fn cbmptx_new() {
        let t = CBMPTEX::new("tex".into(), PathBuf::from("/t.bmp"), 42);
        assert_eq!(t.n_texture, 42);
    }

    #[test]
    fn cbpm_new() {
        let b = CBPM::new(120, 120.0);
        assert_eq!(b.n_bpm, 120);
        assert!((b.db_実BPM - 120.0).abs() < 0.01);
    }

    #[test]
    fn cwav_mark_loaded() {
        let mut w = CWAV::new("test.wav".into(), PathBuf::from("/test.wav"));
        assert!(!w.loaded);
        w.mark_loaded(7);
        assert!(w.loaded);
        assert_eq!(w.n_handle, 7);
    }

    #[test]
    fn stlaneint_new() {
        let l = STLANEINT::new(5, 10);
        assert_eq!(l.n_小節, 5);
        assert_eq!(l.n_チップ, 10);
    }

    #[test]
    fn stresult_total_chips() {
        let mut r = STRESULT::new();
        r.n_perfect = 10;
        r.n_great = 5;
        r.n_miss = 2;
        assert_eq!(r.total_chips(), 17);
    }

    #[test]
    fn stresult_hit_ratio() {
        let mut r = STRESULT::new();
        r.n_perfect = 8;
        r.n_great = 2;
        r.n_miss = 0;
        assert!((r.hit_ratio() - 1.0).abs() < 0.01);
    }

    #[test]
    fn stresult_hit_ratio_zero() {
        let r = STRESULT::new();
        assert_eq!(r.hit_ratio(), 0.0);
    }

    #[test]
    fn stresult_full_combo() {
        let mut r = STRESULT::new();
        r.n_perfect = 10;
        r.n_miss = 0;
        r.n_ok = 0;
        assert!(r.is_full_combo());
        r.n_miss = 1;
        assert!(!r.is_full_combo());
    }

    #[test]
    fn stresult_full_combo_zero_chips() {
        let r = STRESULT::new();
        assert!(!r.is_full_combo()); // 0 chips doesn't count
    }

    #[test]
    fn stresult_all_perfect() {
        let mut r = STRESULT::new();
        r.n_perfect = 50;
        r.n_miss = 0;
        r.n_ok = 0;
        r.n_good = 0;
        r.n_great = 0;
        assert!(r.is_all_perfect());
        r.n_great = 1;
        assert!(!r.is_all_perfect());
    }

    #[test]
    fn sthaschips_new() {
        let h = STHASCHIPS::new();
        assert!(!h.any());
    }

    #[test]
    fn sthaschips_any_true() {
        let h = STHASCHIPS {
            b_楽器: true,
            ..STHASCHIPS::new()
        };
        assert!(h.any());
    }

    #[test]
    fn sthaschips_default_eq_new() {
        assert_eq!(STHASCHIPS::default(), STHASCHIPS::new());
    }
}
