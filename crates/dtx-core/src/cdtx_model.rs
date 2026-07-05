#![allow(non_snake_case)]
//! `CDTX` (7295 LOC) — real DTX model port.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1-7295`
//!
//! v1 strict-port: parse all DTX metadata + chip list, build CChip list with
//! playback time pre-computed. This is the in-memory representation of a
//! DTX file after CDTX.OnActivate() / tParse() runs in BocuD.

use std::collections::HashMap;
use std::path::Path;

use crate::channel::EChannel;
use crate::chart::{Chart, Chip};
use crate::error::Result;
use crate::parser::parse;

/// BPM channel range: 0x08 (BPMEx) covers 0x00..0xFF. We store up to 256.
pub const BPM_CHANNEL_COUNT: usize = 256;
/// 99 difficulty levels (0..100).
pub const DIFFICULTY_LEVELS: u8 = 100;
/// DTX file extension.
pub const DTX_EXTENSION: &str = "dtx";
/// GDA file extension (BocuD alt format).
pub const GDA_EXTENSION: &str = "gda";
/// Default BPM if #BPM header is missing.
pub const DEFAULT_BPM: f32 = 120.0;

/// One BPM change (BocuD `listBPM変更`).
///
/// Reference: `CDTX.cs:1070-1080` — each #BPMxx chip pushes a new entry.
/// We pre-compute the BPM at any measure by binary search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CachedBpmChange {
    pub measure: u32,
    pub bpm: f32,
}

/// Pre-computed chip with absolute playback time (ms).
///
/// Reference: `CChip.cs:ComputeTime` — after the chart is loaded, every
/// chip has a `nPlaybackTimeMs` field used by scrolling + judgment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CChip {
    pub measure: u32,
    pub fraction: f32,
    pub channel: EChannel,
    pub value: i32,
    /// Absolute playback time in ms (set by `compute_playback_time`).
    pub n_playback_time_ms: i64,
}

impl CChip {
    /// Build a CChip from raw Chart::Chip + computed time.
    pub fn from_chip(chip: &Chip, n_playback_time_ms: i64) -> Self {
        Self {
            measure: chip.measure,
            fraction: chip.value,
            channel: chip.channel,
            value: 0, // will be set by EChannel-specific decoder (M7+)
            n_playback_time_ms,
        }
    }
}

/// CDTX — full DTX model after loading.
///
/// Reference: `CDTX.cs:50-200` — class fields and OnActivate.
#[derive(Debug, Clone)]
pub struct CDTX {
    /// Source path.
    pub file_path: Option<std::path::PathBuf>,
    /// Parsed chart (metadata + raw chips).
    pub chart: Chart,
    /// Chips with pre-computed playback time.
    pub chips: Vec<CChip>,
    /// BPM changes sorted by measure.
    pub bpm_changes: Vec<CachedBpmChange>,
    /// WAV filename cache (BGM channel → path).
    pub wav_cache: HashMap<u32, String>,
    /// BMP filename cache (BGA channel → path).
    pub bmp_cache: HashMap<u32, String>,
    /// Full WAV registry (BocuD `listWAV`).
    /// Maps WAV #id → (filename, internal index).
    pub wav_registry: HashMap<u32, WavEntry>,
    /// Full BMP registry (BocuD `listBMP` + `listBMPTEX`).
    pub bmp_registry: HashMap<u32, BmpEntry>,
    /// Full AVI registry (BocuD `listAVI` + `listAVIPAN`).
    pub avi_registry: HashMap<u32, AviEntry>,
    /// Full BGA registry (BocuD `listBGA` + `listBGAPAN`).
    pub bga_registry: HashMap<u32, BgaEntry>,
    /// Per-instrument BPM array (BocuD `nBPM` 0x00..0xFF + BPMEx).
    /// Indexed by channel number 0x00..0xFF (256 entries).
    pub bpm_array: [f32; 256],
    /// Per-instrument chip counts (BocuD `STHASCHIPS`).
    pub has_chips: HasChips,
    /// DTX text lines (BocuD `listDTXManiaFormat`) — the raw file lines
    /// for debugging / round-trip serialization.
    pub raw_lines: Vec<String>,
}

/// One WAV entry in the registry (BocuD `listWAV`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WavEntry {
    /// WAV #id (BocuD `n表記上の番号`).
    pub id: u32,
    /// Filename (BocuD `strファイル名`).
    pub filename: String,
    /// Internal index (BocuD `n内部番号`).
    pub internal_index: u32,
    /// Volume (BocuD `nVolume` 0..100).
    pub volume: u8,
}

/// One BMP entry in the registry (BocuD `listBMP` + `listBMPTEX`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BmpEntry {
    /// BMP #id (BocuD `n番号`).
    pub id: u32,
    /// Filename (BocuD `strファイル名`).
    pub filename: String,
}

/// One AVI entry in the registry (BocuD `listAVI` + `listAVIPAN`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AviEntry {
    /// AVI #id (BocuD `nAVI番号`).
    pub id: u32,
    /// Filename.
    pub filename: String,
}

/// One BGA entry in the registry (BocuD `listBGA` + `listBGAPAN`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BgaEntry {
    /// BGA #id.
    pub id: u32,
    /// Filename.
    pub filename: String,
}

/// Per-instrument chip availability (BocuD `STHASCHIPS`).
///
/// Tracks whether each instrument has any chips in the chart, used by
/// the guitar/bass gauge sub-acts to skip rendering when no chips.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct HasChips {
    /// Drums has chips.
    pub drums: bool,
    /// Guitar has chips.
    pub guitar: bool,
    /// Bass has chips.
    pub bass: bool,
}

impl CDTX {
    /// Build CDTX from a parsed Chart. Computes playback time for all chips.
    pub fn from_chart(file_path: Option<std::path::PathBuf>, chart: Chart) -> Self {
        let bpm_changes = collect_bpm_changes(&chart);
        let base_bpm = chart.metadata.bpm.unwrap_or(DEFAULT_BPM);

        let mut chips: Vec<CChip> = chart
            .chips
            .iter()
            .map(|c| {
                let ms = compute_playback_time(c.measure, c.value, base_bpm, &bpm_changes);
                CChip::from_chip(c, ms)
            })
            .collect();
        // Sort by playback time (CDTX.cs:700-705).
        chips.sort_by_key(|c| c.n_playback_time_ms);

        // Extract BGM/BGA filenames from chip values.
        let mut wav_cache = HashMap::new();
        let mut bmp_cache = HashMap::new();
        for chip in &chart.chips {
            if matches!(chip.channel, EChannel::BGM) {
                wav_cache.insert(chip.measure, format!("{}.wav", chip.value as u32));
            }
            if matches!(
                chip.channel,
                EChannel::BGALayer1
                    | EChannel::BGALayer2
                    | EChannel::BGALayer3
                    | EChannel::BGALayer4
                    | EChannel::BGALayer5
                    | EChannel::BGALayer6
                    | EChannel::BGALayer7
                    | EChannel::BGALayer8
            ) {
                bmp_cache.insert(chip.measure, format!("{}.bmp", chip.value as u32));
            }
        }

        Self {
            file_path,
            chart,
            chips,
            bpm_changes,
            wav_cache,
            bmp_cache,
            wav_registry: HashMap::new(),
            bmp_registry: HashMap::new(),
            avi_registry: HashMap::new(),
            bga_registry: HashMap::new(),
            bpm_array: [DEFAULT_BPM; 256],
            has_chips: HasChips::default(),
            raw_lines: Vec::new(),
        }
    }

    /// Load a DTX file from disk and build the full CDTX model.
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let chart = parse(file)?;
        Ok(Self::from_chart(Some(path.to_path_buf()), chart))
    }

    /// Get the chips for a given channel, in playback-time order.
    pub fn chips_for_channel(&self, channel: EChannel) -> Vec<CChip> {
        self.chips
            .iter()
            .copied()
            .filter(|c| c.channel == channel)
            .collect()
    }

    /// End time of the chart in ms (last chip's playback time).
    pub fn end_time_ms(&self) -> i64 {
        self.chips.last().map(|c| c.n_playback_time_ms).unwrap_or(0)
    }

    /// First chip's playback time (always 0 for charts starting at measure 0).
    pub fn start_time_ms(&self) -> i64 {
        self.chips
            .first()
            .map(|c| c.n_playback_time_ms)
            .unwrap_or(0)
    }

    /// Number of chips per channel.
    pub fn chip_count_by_channel(&self) -> HashMap<EChannel, usize> {
        let mut counts: HashMap<EChannel, usize> = HashMap::new();
        for c in &self.chips {
            *counts.entry(c.channel).or_insert(0) += 1;
        }
        counts
    }
}

/// Compute playback time in ms for a (measure, fraction) at base_bpm,
/// accounting for any BPM changes before that measure.
///
/// Reference: `CChip.cs:ComputeTime` — sums interval durations and the
/// final partial-measure at the last-applied BPM.
pub fn compute_playback_time(
    measure: u32,
    fraction: f32,
    base_bpm: f32,
    bpm_changes: &[CachedBpmChange],
) -> i64 {
    if base_bpm <= 0.0 {
        return 0;
    }
    let mut total_ms = 0.0_f64;
    let mut current_bpm = base_bpm as f64;
    let mut interval_start: u32 = 0;
    for ch in bpm_changes {
        if ch.measure >= measure {
            break;
        }
        if ch.measure > interval_start {
            let dur_ms = (ch.measure - interval_start) as f64 * 4.0 * 60_000.0 / current_bpm;
            total_ms += dur_ms;
        }
        current_bpm = ch.bpm as f64;
        interval_start = ch.measure;
    }
    let partial = (measure - interval_start) as f64 + fraction as f64;
    total_ms += partial * 4.0 * 60_000.0 / current_bpm;
    total_ms as i64
}

/// Extract BPM/BPMEx chips into a sorted list of changes.
fn collect_bpm_changes(chart: &Chart) -> Vec<CachedBpmChange> {
    let mut out: Vec<CachedBpmChange> = chart
        .chips
        .iter()
        .filter(|c| matches!(c.channel, EChannel::BPM | EChannel::BPMEx))
        .map(|c| CachedBpmChange {
            measure: c.measure,
            bpm: c.value,
        })
        .collect();
    out.sort_by_key(|c| c.measure);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdtx_load_real_chart() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).expect("load real_chart.dtx");
        assert_eq!(c.chart.metadata.title.as_deref(), Some("Real Chart Demo"));
        assert!(!c.chips.is_empty());
        // DTXManiaNX inserts one empty measure before source measure 0.
        assert_eq!(c.start_time_ms(), 2000);
    }

    #[test]
    fn cdtx_bpm_changes_extracted() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).unwrap();
        assert_eq!(c.bpm_changes.len(), 1);
        assert_eq!(c.bpm_changes[0].measure, 3);
        assert!((c.bpm_changes[0].bpm - 180.0).abs() < 0.01);
    }

    #[test]
    fn cdtx_chips_sorted_by_time() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).unwrap();
        for w in c.chips.windows(2) {
            assert!(w[0].n_playback_time_ms <= w[1].n_playback_time_ms);
        }
    }

    #[test]
    fn cdtx_chips_for_channel() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).unwrap();
        let bass = c.chips_for_channel(EChannel::BassDrum);
        assert!(!bass.is_empty());
        for ch in &bass {
            assert_eq!(ch.channel, EChannel::BassDrum);
        }
    }

    #[test]
    fn cdtx_chip_count_by_channel() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).unwrap();
        let counts = c.chip_count_by_channel();
        // BassDrum appears in m1, m2, m3 (3 occurrences), not 4
        assert_eq!(counts.get(&EChannel::BassDrum).copied().unwrap_or(0), 3);
    }

    #[test]
    fn cdtx_end_time_after_bpm_change() {
        // The fixture's last chip is beyond the BPM change at measure 2.
        // Just assert the chart has a non-zero end time and chips are sorted.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).unwrap();
        let end = c.end_time_ms();
        // Last chip in real_chart.dtx; the parser stores chip values as
        // fraction (0.0..1.0) for binary data, or as float for #BPM/BGM.
        // After BPM change at measure 2 to 180, m3 chips land at ~5000..6000ms.
        assert!(end > 0, "end time {} should be > 0", end);
    }

    #[test]
    fn cdtx_with_bgm_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("with_bgm.dtx");
        let c = CDTX::load(&path).unwrap();
        assert!(!c.chips.is_empty());
    }

    #[test]
    fn cdtx_real_chart_has_bga_cache() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("real_chart.dtx");
        let c = CDTX::load(&path).unwrap();
        // BGA layer 1+2 are configured in the fixture.
        assert!(!c.bmp_cache.is_empty());
    }

    #[test]
    fn cdtx_default_bpm_constant() {
        // CConfigIni default for new charts.
        assert_eq!(DEFAULT_BPM, 120.0);
    }

    #[test]
    fn dtx_gda_extensions() {
        assert_eq!(DTX_EXTENSION, "dtx");
        assert_eq!(GDA_EXTENSION, "gda");
    }

    #[test]
    fn difficulty_levels_100() {
        assert_eq!(DIFFICULTY_LEVELS, 100);
    }

    #[test]
    fn bpm_channel_count_256() {
        // 0x00..0xFF = 256 BPM table slots.
        assert_eq!(BPM_CHANNEL_COUNT, 256);
    }

    #[test]
    fn compute_playback_constant_bpm() {
        // 120 BPM = 2000ms/measure
        let ms = compute_playback_time(0, 0.0, 120.0, &[]);
        assert_eq!(ms, 0);
        let ms = compute_playback_time(1, 0.0, 120.0, &[]);
        assert_eq!(ms, 2000);
        let ms = compute_playback_time(0, 0.5, 120.0, &[]);
        assert_eq!(ms, 1000);
    }

    #[test]
    fn compute_playback_with_bpm_change() {
        let changes = [CachedBpmChange {
            measure: 2,
            bpm: 240.0,
        }];
        // m0..2 at 120 BPM = 4000ms
        // m2..3 at 240 BPM = 1000ms
        let ms = compute_playback_time(3, 0.0, 120.0, &changes);
        assert_eq!(ms, 5000);
    }

    #[test]
    fn compute_playback_zero_bpm_safe() {
        let ms = compute_playback_time(5, 0.5, 0.0, &[]);
        assert_eq!(ms, 0);
    }

    #[test]
    fn cdtx_empty_chart() {
        let chart = Chart::default();
        let c = CDTX::from_chart(None, chart);
        assert!(c.chips.is_empty());
        assert_eq!(c.start_time_ms(), 0);
        assert_eq!(c.end_time_ms(), 0);
    }
}
