//! Chart-relative visual asset resolution and BPM-aware event timing.
//!
//! Ports DTXManiaNX's image/movie event scheduling: each visual chip starts
//! its layer at the chip's playback time, resolved through the same BPM and
//! bar-length timing the drum chips use.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CDTX.cs:1296-1476`
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfBGA.cs:61-96`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dtx_core::bga::BgaLayer;
use dtx_core::chart::Chart;
use dtx_core::resolve_chart_asset_path;
use dtx_core::timing::{chip_time_ms_with_bpm_and_bar_changes, ChartTiming};

/// A visual chip resolved to an absolute playback time (ms) on the gameplay
/// clock, tagged with its target layer and the `#BMP`/`#AVI` asset id it
/// references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedVisualEvent {
    /// Playback time in ms from chart start (same timeline as drum chips).
    pub target_ms: i64,
    /// Which BGA layer / movie channel this event drives.
    pub layer: BgaLayer,
    /// `#BMPxx` / `#AVIxx` asset id referenced by the chip.
    pub asset_id: u32,
}

/// Build BPM-and-bar-length-aware visual events for a chart, sorted by time
/// with stable source order for ties.
pub fn timed_visual_events(chart: &Chart) -> Vec<TimedVisualEvent> {
    let bpm_changes = dtx_core::timing::bpm_changes_from_chart(chart);
    let bar_changes = dtx_core::timing::bar_changes_from_chart(chart);

    let timing = ChartTiming {
        bpm_changes: &bpm_changes,
        bar_changes: &bar_changes,
    };
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);

    let mut events: Vec<TimedVisualEvent> = chart
        .chips
        .iter()
        .filter_map(|chip| {
            let layer = BgaLayer::from_channel(chip.channel)?;
            Some(TimedVisualEvent {
                target_ms: chip_time_ms_with_bpm_and_bar_changes(
                    chip.measure,
                    chip.value,
                    base_bpm,
                    timing,
                ),
                layer,
                asset_id: chip.wav_slot,
            })
        })
        .collect();
    events.sort_by_key(|event| event.target_ms);
    events
}

/// Resolve the chart's `#BMP`/`#BGA` and `#AVI` registries to absolute file
/// paths under `chart_dir`. Missing files are simply absent from the maps.
fn resolve_maps(chart: &Chart, chart_dir: &Path) -> (HashMap<u32, PathBuf>, HashMap<u32, PathBuf>) {
    let mut bmp_paths = HashMap::new();
    for (&id, filename) in chart
        .assets
        .bmp
        .by_id
        .iter()
        .chain(chart.assets.bga.by_id.iter())
    {
        if let Some(path) = resolve_chart_asset_path(chart_dir, filename) {
            bmp_paths.insert(id, path);
        }
    }
    let mut avi_paths = HashMap::new();
    for (&id, filename) in &chart.assets.avi.by_id {
        if let Some(path) = resolve_chart_asset_path(chart_dir, filename) {
            avi_paths.insert(id, path);
        }
    }
    (bmp_paths, avi_paths)
}

/// Prepared chart-visual state published to `dtx-bga` for playback: the chart
/// source directory, timed events, and resolved BMP/AVI paths by asset id.
#[derive(bevy::prelude::Resource, Debug, Default, Clone)]
pub struct ActiveChartRes {
    /// Directory containing the chart file (parent of the `.dtx`), if known.
    pub source_dir: Option<PathBuf>,
    /// Timed visual events, sorted by playback time.
    pub events: Vec<TimedVisualEvent>,
    /// Resolved `#BMP`/`#BGA` image paths by asset id.
    pub bmp_paths: HashMap<u32, PathBuf>,
    /// Resolved `#AVI` movie paths by asset id.
    pub avi_paths: HashMap<u32, PathBuf>,
}

impl ActiveChartRes {
    /// Build prepared visual state from a parsed chart and its source path.
    pub fn from_chart(chart: &Chart, source_path: Option<&Path>) -> Self {
        let source_dir = source_path.and_then(Path::parent).map(Path::to_path_buf);
        let (bmp_paths, avi_paths) = match source_dir.as_deref() {
            Some(dir) => resolve_maps(chart, dir),
            None => (HashMap::new(), HashMap::new()),
        };
        Self {
            source_dir,
            events: timed_visual_events(chart),
            bmp_paths,
            avi_paths,
        }
    }

    /// Path for a resolved BMP asset id.
    pub fn bmp_path(&self, id: u32) -> Option<&Path> {
        self.bmp_paths.get(&id).map(PathBuf::as_path)
    }

    /// Path for a resolved AVI asset id.
    pub fn avi_path(&self, id: u32) -> Option<&Path> {
        self.avi_paths.get(&id).map(PathBuf::as_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push("fixtures");
        p
    }

    fn parse_fixture(name: &str) -> Chart {
        let path = fixture_dir().join(name);
        dtx_core::parser::parse(std::fs::File::open(&path).expect("fixture chart"))
            .expect("parse fixture")
    }

    #[test]
    fn active_chart_res_resolves_assets_and_bpm_aware_times() {
        let dir = fixture_dir();
        let chart = parse_fixture("visual.dtx");
        let prepared = ActiveChartRes::from_chart(&chart, Some(&dir.join("visual.dtx")));

        // First event: source measure 0 -> chart measure 1 -> 2000ms at 120 BPM.
        assert_eq!(prepared.events[0].target_ms, 2000);
        assert_eq!(prepared.events[0].asset_id, 1);
        assert_eq!(prepared.bmp_paths.get(&1), Some(&dir.join("red.png")));
    }

    #[test]
    fn timed_events_respect_bpm_change() {
        // A mid-chart BPM change must shift later visual event times off the
        // constant-BPM estimate.
        let src = b"#TITLE: T\n#BPM: 120\n#BMP01: red.png\n#00304: 01\n#00103: C0\n#00504: 01\n";
        let chart = dtx_core::parser::parse(&src[..]).expect("parse");
        let events = timed_visual_events(&chart);
        // Constant 120 BPM would put chart measure 5 at 10000ms; the 192 BPM
        // change at chart measure 2 makes the real time smaller.
        let late = events
            .iter()
            .max_by_key(|e| e.target_ms)
            .expect("has events");
        assert!(
            late.target_ms < 10000,
            "BPM change should shorten later event time, got {}",
            late.target_ms
        );
    }
}
