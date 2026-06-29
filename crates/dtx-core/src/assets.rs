//! DTX asset registry parser.
//!
//! Port of `#WAVxx: filename`, `#BMPxx: filename`, `#AVIxx: filename`,
//! `#BGAxx: filename`, `#BGAPANxx: filename`, `#AVIPANxx: filename`
//! directive parsing from
//! `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1300-1800`.
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping (functions extracted from
//! the monolithic CDTX class into a focused module).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1300-1800`

use std::collections::HashMap;

use crate::base36;

/// WAV asset registry (BocuD `listWAV`).
///
/// Maps WAV #id (1..256, encoded as hex 0x01..0xFF in chip channels) →
/// filename. Built by parsing `#WAVxx: <filename>` lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WavRegistry {
    /// id → filename.
    pub by_id: HashMap<u32, String>,
    /// Insertion order (BocuD preserves order in `listWAV`).
    pub order: Vec<u32>,
    /// Per-WAV volume 0..100 (default 100). From `#VOLUME` / `#WAVVOL`.
    pub volumes: HashMap<u32, i32>,
    /// Per-WAV pan -100..100 (default 0). From `#PAN` / `#WAVPAN`.
    pub pans: HashMap<u32, i32>,
}

impl WavRegistry {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a WAV file.
    pub fn insert(&mut self, id: u32, filename: String) {
        if !self.by_id.contains_key(&id) {
            self.order.push(id);
        }
        self.by_id.insert(id, filename);
    }

    /// Get filename by id.
    pub fn get(&self, id: u32) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// Volume for a WAV id (0..100). Default 100.
    pub fn volume(&self, id: u32) -> i32 {
        self.volumes.get(&id).copied().unwrap_or(100)
    }

    /// Pan for a WAV id (-100..100). Default 0.
    pub fn pan(&self, id: u32) -> i32 {
        self.pans.get(&id).copied().unwrap_or(0)
    }

    /// Total registered WAVs.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// BMP asset registry (BocuD `listBMP` + `listBMPTEX`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BmpRegistry {
    /// id → filename.
    pub by_id: HashMap<u32, String>,
    /// Insertion order.
    pub order: Vec<u32>,
}

impl BmpRegistry {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a BMP file.
    pub fn insert(&mut self, id: u32, filename: String) {
        if !self.by_id.contains_key(&id) {
            self.order.push(id);
        }
        self.by_id.insert(id, filename);
    }

    /// Get filename by id.
    pub fn get(&self, id: u32) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// Total registered BMPs.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// AVI asset registry (BocuD `listAVI` + `listAVIPAN`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AviRegistry {
    /// id → filename.
    pub by_id: HashMap<u32, String>,
    /// Insertion order.
    pub order: Vec<u32>,
}

impl AviRegistry {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an AVI file.
    pub fn insert(&mut self, id: u32, filename: String) {
        if !self.by_id.contains_key(&id) {
            self.order.push(id);
        }
        self.by_id.insert(id, filename);
    }

    /// Get filename by id.
    pub fn get(&self, id: u32) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// Total registered AVIs.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True if no AVIs are registered.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// BGA asset registry (BocuD `listBGA` + `listBGAPAN`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BgaRegistry {
    /// id → filename.
    pub by_id: HashMap<u32, String>,
    /// Insertion order.
    pub order: Vec<u32>,
}

impl BgaRegistry {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a BGA file.
    pub fn insert(&mut self, id: u32, filename: String) {
        if !self.by_id.contains_key(&id) {
            self.order.push(id);
        }
        self.by_id.insert(id, filename);
    }

    /// Get filename by id.
    pub fn get(&self, id: u32) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// Total registered BGAs.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True if no BGAs are registered.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Parse `#VOLUME01:` / `#WAVVOL01:` line. Returns (id, volume 0..100) or None.
pub fn parse_volume_directive(line: &str) -> Option<(u32, i32)> {
    parse_wav_id_directive(line, &["VOLUME", "WAVVOL"]).and_then(|(id, value)| {
        let v: i32 = value.parse().ok()?;
        Some((id, v.clamp(0, 100)))
    })
}

/// Parse `#PAN01:` / `#WAVPAN01:` line. Returns (id, pan -100..100) or None.
pub fn parse_pan_directive(line: &str) -> Option<(u32, i32)> {
    parse_wav_id_directive(line, &["PAN", "WAVPAN"]).and_then(|(id, value)| {
        let v: i32 = value.trim().parse().ok()?;
        Some((id, v.clamp(-100, 100)))
    })
}

fn parse_wav_id_directive(line: &str, prefixes: &[&str]) -> Option<(u32, String)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    for prefix in prefixes {
        if let Some(suffix) = upper.strip_prefix(prefix) {
            if suffix.is_empty() || suffix.len() > 2 {
                continue;
            }
            let id = base36::parse_id_suffix(suffix)?;
            return Some((id, value.trim().to_string()));
        }
    }
    None
}

/// Parse `#WAVxx: <filename>` line. Returns (id, filename) or None.
pub fn parse_wav_directive(line: &str) -> Option<(u32, String)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    let suffix = upper.strip_prefix("WAV")?;
    if suffix.is_empty() || suffix.len() > 2 {
        return None;
    }
    let id = base36::parse_id_suffix(suffix)?;
    let filename = strip_dtx_param(value);
    Some((id, filename.to_string()))
}

fn strip_dtx_param(s: &str) -> &str {
    s.split([';', '\t']).next().unwrap_or(s).trim()
}

/// Parse `#BMPxx: <filename>` line. Returns (id, filename) or None.
pub fn parse_bmp_directive(line: &str) -> Option<(u32, String)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    let suffix = upper.strip_prefix("BMP")?;
    if suffix.is_empty() || suffix.len() > 2 {
        return None;
    }
    let id = base36::parse_id_suffix(suffix)?;
    Some((id, value.trim().to_string()))
}

/// Parse `#AVIxx: <filename>` line. Returns (id, filename) or None.
pub fn parse_avi_directive(line: &str) -> Option<(u32, String)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    let suffix = upper.strip_prefix("AVI")?;
    if suffix.is_empty() || suffix.len() > 2 {
        return None;
    }
    let id = base36::parse_id_suffix(suffix)?;
    Some((id, value.trim().to_string()))
}

/// Parse `#BGAxx: <filename>` or `#BGAPANxx: <filename>` line.
pub fn parse_bga_directive(line: &str) -> Option<(u32, String)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    if let Some(suffix) = upper.strip_prefix("BGAPAN") {
        let id = base36::parse_id_suffix(suffix)?;
        Some((id, value.trim().to_string()))
    } else if let Some(suffix) = upper.strip_prefix("BGA") {
        if suffix.is_empty() || suffix.len() > 2 {
            return None;
        }
        let id = base36::parse_id_suffix(suffix)?;
        Some((id, value.trim().to_string()))
    } else {
        None
    }
}

/// Parse `#BPMxx: <value>` line. Returns (id, value) or None.
pub fn parse_bpm_directive(line: &str) -> Option<(u32, f32)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    let suffix = upper.strip_prefix("BPM")?;
    if suffix.is_empty() || suffix.len() > 2 {
        return None;
    }
    let id = base36::parse_id_suffix(suffix)?;
    let v: f32 = value.trim().parse().ok()?;
    Some((id, v))
}

/// Asset registry bundle — all registries in one struct.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DtxAssets {
    /// WAV registry.
    pub wav: WavRegistry,
    /// BMP registry.
    pub bmp: BmpRegistry,
    /// AVI registry.
    pub avi: AviRegistry,
    /// BGA registry.
    pub bga: BgaRegistry,
}

impl DtxAssets {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process one DTX text line, dispatching to the appropriate registry.
    /// Returns true if the line was an asset directive.
    pub fn process_line(&mut self, line: &str) -> bool {
        if let Some((id, filename)) = parse_wav_directive(line) {
            self.wav.insert(id, filename);
            return true;
        }
        if let Some((id, vol)) = parse_volume_directive(line) {
            self.wav.volumes.insert(id, vol);
            return true;
        }
        if let Some((id, pan)) = parse_pan_directive(line) {
            self.wav.pans.insert(id, pan);
            return true;
        }
        if let Some((id, filename)) = parse_bmp_directive(line) {
            self.bmp.insert(id, filename);
            return true;
        }
        if let Some((id, filename)) = parse_avi_directive(line) {
            self.avi.insert(id, filename);
            return true;
        }
        if let Some((id, filename)) = parse_bga_directive(line) {
            self.bga.insert(id, filename);
            return true;
        }
        false
    }

    /// Process all lines from an iterator (typically `BufRead::lines`).
    /// Returns the number of asset directives found.
    pub fn process_lines<I, S>(&mut self, lines: I) -> usize
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut count = 0;
        for line in lines {
            if self.process_line(line.as_ref()) {
                count += 1;
            }
        }
        count
    }
}

/// Resolve the BGM audio file for a chart.
///
/// Priority (BocuD / real-world DTX conventions):
/// 1. `#BGMWAV:` slot → `#WAVxx:` filename in the same folder
/// 2. Common drum filenames (`drums.ogg`, `bgm_d.ogg`, …)
/// 3. `#PREVIEW:` file
/// 4. `<dtx_stem>.ogg` / `1.ogg` legacy heuristic
pub fn resolve_bgm_path(
    dtx_path: &std::path::Path,
    chart: &crate::chart::Chart,
) -> Option<std::path::PathBuf> {
    let parent = dtx_path.parent()?;

    for &slot in &chart.metadata.bgm_wav_slots {
        if let Some(name) = chart.assets.wav.get(slot) {
            let p = parent.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    for name in [
        "drums.ogg",
        "bgm_d.ogg",
        "bgm.ogg",
        "1.ogg",
        "drums.wav",
        "bgm.wav",
        "1.wav",
    ] {
        let p = parent.join(name);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Some(preview) = chart.metadata.preview_filename.as_deref() {
        let p = parent.join(preview);
        if p.is_file() {
            return Some(p);
        }
    }

    let stem = dtx_path.file_stem()?.to_str()?;
    for ext in &["ogg", "wav"] {
        let p = parent.join(format!("{stem}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // === WAV directive ===

    #[test]
    fn parse_wav_basic() {
        let (id, name) = parse_wav_directive("#WAV01: foo.wav").unwrap();
        assert_eq!(id, 1);
        assert_eq!(name, "foo.wav");
    }

    #[test]
    fn parse_wav_lowercase() {
        let (id, name) = parse_wav_directive("#wav0a: bar.ogg").unwrap();
        assert_eq!(id, 10);
        assert_eq!(name, "bar.ogg");
    }

    #[test]
    fn parse_wav_with_whitespace() {
        let (id, name) = parse_wav_directive("  #WAVFF:   baz.wav  ").unwrap();
        assert_eq!(id, 555); // base36 "FF" = 15*36+15
        assert_eq!(name, "baz.wav");
    }

    #[test]
    fn parse_wav_rejects_non_wav() {
        assert!(parse_wav_directive("#BPM01: 120").is_none());
        assert!(parse_wav_directive("#WAVXYZ: bad").is_none());
        assert!(parse_wav_directive("not a directive").is_none());
    }

    // === BMP directive ===

    #[test]
    fn parse_bmp_basic() {
        let (id, name) = parse_bmp_directive("#BMP01: image.bmp").unwrap();
        assert_eq!(id, 1);
        assert_eq!(name, "image.bmp");
    }

    // === AVI directive ===

    #[test]
    fn parse_avi_basic() {
        let (id, name) = parse_avi_directive("#AVI05: movie.avi").unwrap();
        assert_eq!(id, 5);
        assert_eq!(name, "movie.avi");
    }

    // === BGA directive ===

    #[test]
    fn parse_bga_basic() {
        let (id, name) = parse_bga_directive("#BGA03: bg.bmp").unwrap();
        assert_eq!(id, 3);
        assert_eq!(name, "bg.bmp");
    }

    #[test]
    fn parse_bgapan_basic() {
        let (id, name) = parse_bga_directive("#BGAPAN03: pan.bmp").unwrap();
        assert_eq!(id, 3);
        assert_eq!(name, "pan.bmp");
    }

    // === BPM directive ===

    #[test]
    fn parse_bpm_basic() {
        let (id, value) = parse_bpm_directive("#BPM01: 180.0").unwrap();
        assert_eq!(id, 1);
        assert!((value - 180.0).abs() < 0.01);
    }

    // === WavRegistry ===

    #[test]
    fn wav_registry_insert_get() {
        let mut r = WavRegistry::new();
        r.insert(1, "foo.wav".into());
        r.insert(2, "bar.ogg".into());
        assert_eq!(r.get(1), Some("foo.wav"));
        assert_eq!(r.get(2), Some("bar.ogg"));
        assert_eq!(r.get(99), None);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn wav_registry_preserves_insertion_order() {
        let mut r = WavRegistry::new();
        r.insert(5, "c".into());
        r.insert(1, "a".into());
        r.insert(3, "b".into());
        assert_eq!(r.order, vec![5, 1, 3]);
    }

    #[test]
    fn wav_registry_duplicate_id_no_double_order() {
        let mut r = WavRegistry::new();
        r.insert(1, "a".into());
        r.insert(1, "b".into());
        assert_eq!(r.order, vec![1]);
        assert_eq!(r.len(), 1);
    }

    // === DtxAssets process_line ===

    #[test]
    fn dtx_assets_process_mixed_lines() {
        let mut a = DtxAssets::new();
        // Non-asset lines return false.
        assert!(!a.process_line("#TITLE: My Song"));
        assert!(!a.process_line("#BPM: 120"));
        // Asset lines return true.
        assert!(a.process_line("#WAV01: foo.wav"));
        assert!(a.process_line("#BMP02: bar.bmp"));
        assert!(a.process_line("#AVI03: baz.avi"));
        assert!(a.process_line("#BGA04: qux.bmp"));
        assert_eq!(a.wav.len(), 1);
        assert_eq!(a.bmp.len(), 1);
        assert_eq!(a.avi.len(), 1);
        assert_eq!(a.bga.len(), 1);
        assert_eq!(a.wav.get(1), Some("foo.wav"));
    }

    #[test]
    fn dtx_assets_process_lines_iter() {
        let mut a = DtxAssets::new();
        let lines = vec!["#WAV01: a.wav", "#TITLE: ignored", "#WAV02: b.wav"];
        let n = a.process_lines(lines);
        assert_eq!(n, 2);
        assert_eq!(a.wav.len(), 2);
    }

    // === BmpRegistry / AviRegistry / BgaRegistry ===

    #[test]
    fn bmp_registry_insert_get() {
        let mut r = BmpRegistry::new();
        r.insert(1, "x.bmp".into());
        assert_eq!(r.get(1), Some("x.bmp"));
        assert!(r.get(2).is_none());
    }

    #[test]
    fn avi_registry_insert_get() {
        let mut r = AviRegistry::new();
        r.insert(1, "x.avi".into());
        assert_eq!(r.get(1), Some("x.avi"));
    }

    #[test]
    fn bga_registry_insert_get() {
        let mut r = BgaRegistry::new();
        r.insert(1, "x.bmp".into());
        assert_eq!(r.get(1), Some("x.bmp"));
    }
}
