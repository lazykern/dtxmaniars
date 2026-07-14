//! DTX asset registry parser.
//!
//! Port of `#WAVxx: filename`, `#BMPxx: filename`, `#AVIxx: filename`,
//! `#BGAxx: filename`, `#BGAPANxx: filename`, `#AVIPANxx: filename`
//! directive parsing from
//! `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1300-1800`.
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping (functions extracted from
//! the monolithic CDTX class into a focused module).
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CDTX.cs:1300-1800`

use std::collections::HashMap;

use crate::base36;

/// Integer pixel rectangle used by authored BGA/AVI pan definitions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Full NX `#BGAPANxx` / `#AVIPANxx` definition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanDefinition {
    pub asset_slot: u32,
    pub source_start: PixelRect,
    pub source_end: PixelRect,
    pub destination_start: PixelRect,
    pub destination_end: PixelRect,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanTarget {
    Image,
    Movie,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanDefinitionError {
    pub detail: String,
}

/// Parse a pan directive while retaining malformed directives for diagnostics.
pub fn parse_visual_pan_directive(
    line: &str,
) -> Option<Result<(PanTarget, u32, PanDefinition), PanDefinitionError>> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let upper = head.trim().to_ascii_uppercase();
    let (target, suffix) = if let Some(suffix) = upper.strip_prefix("BGAPAN") {
        (PanTarget::Image, suffix)
    } else if let Some(suffix) = upper.strip_prefix("AVIPAN") {
        (PanTarget::Movie, suffix)
    } else {
        return None;
    };

    let invalid = |detail: String| Err(PanDefinitionError { detail });
    if suffix.len() != 2 {
        return Some(invalid(
            "pan definition id must be two base36 digits".into(),
        ));
    }
    let Some(id) = base36::parse_id_suffix(suffix) else {
        return Some(invalid("pan definition id is not base36".into()));
    };
    let fields = value
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, ',' | '(' | ')' | '[' | ']' | 'x' | '|')
        })
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    if fields.len() != 14 {
        return Some(invalid(format!(
            "pan definition requires 14 fields, found {}",
            fields.len()
        )));
    }
    let Some(asset_slot) = base36::parse_id_suffix(fields[0]).filter(|slot| *slot > 0) else {
        return Some(invalid("pan asset id must be 01 through ZZ".into()));
    };
    let mut numbers = [0_i32; 13];
    for (index, field) in fields[1..].iter().enumerate() {
        let Ok(number) = field.parse::<i32>() else {
            return Some(invalid(format!(
                "pan field {} is not an integer",
                index + 2
            )));
        };
        numbers[index] = number;
    }
    let definition = PanDefinition {
        asset_slot,
        source_start: PixelRect {
            x: numbers[4],
            y: numbers[5],
            width: numbers[0],
            height: numbers[1],
        },
        source_end: PixelRect {
            x: numbers[6],
            y: numbers[7],
            width: numbers[2],
            height: numbers[3],
        },
        destination_start: PixelRect {
            x: numbers[8],
            y: numbers[9],
            width: numbers[0],
            height: numbers[1],
        },
        destination_end: PixelRect {
            x: numbers[10],
            y: numbers[11],
            width: numbers[2],
            height: numbers[3],
        },
        duration_ticks: numbers[12].max(0) as u32,
    };
    Some(Ok((target, id, definition)))
}

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
    /// Per-WAV chip size 0..100 percent (default 100). From `#SIZExx`.
    pub sizes: HashMap<u32, i32>,
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

    /// Chip size percent for a WAV id (0..100). Default 100.
    pub fn size(&self, id: u32) -> i32 {
        self.sizes.get(&id).copied().unwrap_or(100)
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

/// Parse `#SIZE01:` line. Returns (id, chip size percent 0..100) or None.
///
/// DTXManiaNX scales a chip's sprite by this percentage (`CChip.dbChipSizeRatio`,
/// clamped to 0..100 in `t入力_行解析_SIZE`).
pub fn parse_size_directive(line: &str) -> Option<(u32, i32)> {
    parse_wav_id_directive(line, &["SIZE"]).and_then(|(id, value)| {
        let v: i32 = value.parse().ok()?;
        Some((id, v.clamp(0, 100)))
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

/// Parse `#BGAxx: <filename>` line.
pub fn parse_bga_directive(line: &str) -> Option<(u32, String)> {
    let body = line.trim().strip_prefix('#')?;
    let (head, value) = body.split_once(':')?;
    let head = head.trim();
    let upper = head.to_ascii_uppercase();
    if let Some(suffix) = upper.strip_prefix("BGA") {
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
    /// `#BGAPANxx` definitions, later definitions replacing earlier ones.
    pub bga_pan: HashMap<u32, PanDefinition>,
    /// `#AVIPANxx` definitions, later definitions replacing earlier ones.
    pub avi_pan: HashMap<u32, PanDefinition>,
    /// `#BPMxx` definition table (BocuD `listBPM`): slot id → BPM value.
    /// Referenced by BPMEx (channel 0x08) chips.
    pub bpm: HashMap<u32, f32>,
}

impl DtxAssets {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process one DTX text line, dispatching to the appropriate registry.
    /// Returns true if the line was an asset directive.
    pub fn process_line(&mut self, line: &str) -> bool {
        if let Some(result) = parse_visual_pan_directive(line) {
            if let Ok((target, id, definition)) = result {
                match target {
                    PanTarget::Image => self.bga_pan.insert(id, definition),
                    PanTarget::Movie => self.avi_pan.insert(id, definition),
                };
            }
            return true;
        }
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
        if let Some((id, size)) = parse_size_directive(line) {
            self.wav.sizes.insert(id, size);
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
        if let Some((id, bpm)) = parse_bpm_directive(line) {
            self.bpm.insert(id, bpm);
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
            if let Some(p) = resolve_chart_asset_path(parent, name) {
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
        "drums.mp3",
        "bgm_d.mp3",
        "bgm.mp3",
        "1.mp3",
    ] {
        if let Some(p) = resolve_chart_asset_path(parent, name) {
            return Some(p);
        }
    }

    if let Some(preview) = chart.metadata.preview_filename.as_deref() {
        if let Some(p) = resolve_chart_asset_path(parent, preview) {
            return Some(p);
        }
    }

    let stem = dtx_path.file_stem()?.to_str()?;
    for ext in &["ogg", "wav", "mp3"] {
        if let Some(p) = resolve_chart_asset_path(parent, &format!("{stem}.{ext}")) {
            return Some(p);
        }
    }

    None
}

/// Resolve a chart-relative asset filename against `chart_dir`.
///
/// Tries a direct join first, then a case-insensitive match on every nested
/// component (DTX charts authored on Windows frequently disagree with the
/// on-disk case). Returns `None` when no match exists or when the supplied
/// path escapes the chart directory.
/// Shared by audio and visual asset loaders so both use one filesystem
/// algorithm.
pub fn resolve_chart_asset_path(
    chart_dir: &std::path::Path,
    filename: &str,
) -> Option<std::path::PathBuf> {
    let normalized = filename.replace('\\', "/");
    let relative = std::path::Path::new(&normalized);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return None;
    }

    let direct = chart_dir.join(relative);
    if direct.is_file() {
        return Some(direct);
    }

    let mut current = chart_dir.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            continue;
        };
        let wanted = component.to_str()?;
        let candidate = current.join(component);
        if candidate.exists() {
            current = candidate;
            continue;
        }
        current = std::fs::read_dir(&current)
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
            })?
            .path();
    }
    current.is_file().then_some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    // === SIZE directive ===

    #[test]
    fn parse_size_clamps_and_defaults() {
        assert_eq!(parse_size_directive("#SIZE07: 80").unwrap(), (7, 80));
        assert_eq!(parse_size_directive("#size0I: 140").unwrap(), (18, 100));
        assert!(parse_size_directive("#SIZE: 80").is_none());

        let mut assets = DtxAssets::new();
        assert!(assets.process_line("#SIZE07: 80"));
        assert_eq!(assets.wav.size(7), 80);
        assert_eq!(assets.wav.size(8), 100);
    }

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
        let (target, id, pan) =
            parse_visual_pan_directive("#BGAPAN03: 02,100,100,50,50,0,0,10,10,20,20,30,30,96")
                .expect("pan directive")
                .expect("valid pan");
        assert_eq!(target, PanTarget::Image);
        assert_eq!(id, 3);
        assert_eq!(pan.asset_slot, 2);
        assert_eq!(pan.duration_ticks, 96);
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

    #[test]
    fn resolve_chart_asset_path_matches_case_insensitively() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-core-visual-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create temp chart dir");
        std::fs::write(dir.join("Jacket.PNG"), b"x").expect("write fixture");

        assert_eq!(
            resolve_chart_asset_path(&dir, "jacket.png"),
            Some(dir.join("Jacket.PNG"))
        );

        std::fs::remove_dir_all(dir).expect("remove temp chart dir");
    }

    #[test]
    fn resolve_chart_asset_path_matches_nested_windows_path_case_insensitively() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-core-nested-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let nested = dir.join("Kit").join("Cymbals");
        std::fs::create_dir_all(&nested).expect("create nested chart dir");
        let fixture = nested.join("Crash.WAV");
        std::fs::write(&fixture, b"x").expect("write fixture");

        assert_eq!(
            resolve_chart_asset_path(&dir, "kit\\cymbals\\crash.wav"),
            Some(fixture)
        );

        std::fs::remove_dir_all(dir).expect("remove temp chart dir");
    }

    #[test]
    fn resolve_bgm_path_falls_back_to_case_insensitive_mp3() {
        let dir = std::env::temp_dir().join(format!(
            "dtx-core-mp3-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).expect("create temp chart dir");
        let mp3 = dir.join("BGM.MP3");
        std::fs::write(&mp3, b"not decoded in this resolver test").expect("write fixture");

        let chart = crate::chart::Chart::default();
        assert_eq!(resolve_bgm_path(&dir.join("song.dtx"), &chart), Some(mp3));

        std::fs::remove_dir_all(dir).expect("remove temp chart dir");
    }
}
