//! Chart-relative visual asset resolution and deterministic chart-time state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dtx_core::assets::{PanDefinition, PixelRect};
use dtx_core::bga::BgaLayer;
use dtx_core::chart::Chart;
use dtx_core::resolve_chart_asset_path;
use dtx_core::timing::{chip_time_ms_with_bpm_and_bar_changes, ChartTiming};

/// Typed visual operation carried by a chart chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualEventKind {
    /// Replace a layer with a static image or movie asset.
    Replace {
        /// Target visual layer.
        layer: BgaLayer,
        /// Registered BMP/AVI slot.
        asset_slot: u32,
    },
    /// Replace a BGA scope through one of the eight swap channels.
    Swap {
        /// Target BGA scope.
        layer: BgaLayer,
        /// Registered BMP slot.
        asset_slot: u32,
    },
    /// Animate an image crop and destination rectangle.
    ImagePan {
        /// Target BGA layer.
        layer: BgaLayer,
        /// Authored pan definition.
        definition: PanDefinition,
    },
    /// Animate a movie crop and destination rectangle.
    MoviePan {
        /// Authored pan definition.
        definition: PanDefinition,
    },
}

impl VisualEventKind {
    /// Target layer of this operation.
    pub fn layer(self) -> BgaLayer {
        match self {
            Self::Replace { layer, .. }
            | Self::Swap { layer, .. }
            | Self::ImagePan { layer, .. } => layer,
            Self::MoviePan { .. } => BgaLayer::Movie,
        }
    }

    /// Underlying BMP/AVI asset id.
    pub fn asset_id(self) -> u32 {
        match self {
            Self::Replace { asset_slot, .. } | Self::Swap { asset_slot, .. } => asset_slot,
            Self::ImagePan { definition, .. } | Self::MoviePan { definition } => {
                definition.asset_slot
            }
        }
    }
}

/// A visual operation resolved onto the gameplay clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedVisualEvent {
    /// Chart-time start in milliseconds.
    pub target_ms: i64,
    /// Chart-time end in milliseconds; equal to start for static operations.
    pub end_ms: i64,
    /// Typed visual operation.
    pub kind: VisualEventKind,
}

impl TimedVisualEvent {
    /// Construct a zero-duration replacement event.
    pub fn replace(target_ms: i64, layer: BgaLayer, asset_id: u32) -> Self {
        Self {
            target_ms,
            end_ms: target_ms,
            kind: VisualEventKind::Replace {
                layer,
                asset_slot: asset_id,
            },
        }
    }

    /// Target layer.
    pub fn layer(self) -> BgaLayer {
        self.kind.layer()
    }

    /// Underlying asset id.
    pub fn asset_id(self) -> u32 {
        self.kind.asset_id()
    }
}

/// Floating-point pixel rectangle used by interpolated render state.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RectF32 {
    /// Left coordinate.
    pub x: f32,
    /// Top coordinate.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// Interpolated source crop and stage destination.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VisualGeometry {
    /// Source-media crop rectangle.
    pub source: RectF32,
    /// Destination rectangle in the 1280x720 authored stage.
    pub destination: RectF32,
}

/// Reconstructed image state for one BGA layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerVisualState {
    /// Active BMP asset id.
    pub asset_id: u32,
    /// Pan geometry when the active operation is a pan.
    pub geometry: Option<VisualGeometry>,
}

/// Reconstructed active movie state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovieVisualState {
    /// Active AVI asset id.
    pub asset_id: u32,
    /// Chart-time start used to seek the decoder.
    pub start_ms: i64,
    /// Pan geometry when the active operation is an AVIPAN.
    pub geometry: Option<VisualGeometry>,
}

/// Complete deterministic visual state at one chart-time position.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VisualState {
    /// Latest image state per BGA layer.
    pub layers: HashMap<BgaLayer, LayerVisualState>,
    /// Latest active movie, if motion is enabled.
    pub movie: Option<MovieVisualState>,
}

fn interpolate(a: i32, b: i32, progress: f32) -> f32 {
    a as f32 + (b - a) as f32 * progress
}

fn interpolate_rect(start: PixelRect, end: PixelRect, progress: f32) -> RectF32 {
    RectF32 {
        x: interpolate(start.x, end.x, progress).max(0.0),
        y: interpolate(start.y, end.y, progress).max(0.0),
        width: interpolate(start.width, end.width, progress).max(0.0),
        height: interpolate(start.height, end.height, progress).max(0.0),
    }
}

fn pan_geometry(definition: PanDefinition, progress: f32) -> VisualGeometry {
    let source = interpolate_rect(definition.source_start, definition.source_end, progress);
    let mut destination = interpolate_rect(
        definition.destination_start,
        definition.destination_end,
        progress,
    );
    // NX's authored stage is 1280x720. Keep optional visuals inside that safe
    // area even when a chart contains negative or oversized destinations.
    destination.x = destination.x.min(1280.0);
    destination.y = destination.y.min(720.0);
    destination.width = destination.width.min((1280.0 - destination.x).max(0.0));
    destination.height = destination.height.min((720.0 - destination.y).max(0.0));
    VisualGeometry {
        source,
        destination,
    }
}

fn event_progress(event: TimedVisualEvent, now_ms: i64, motion_enabled: bool) -> f32 {
    if !motion_enabled || event.end_ms <= event.target_ms {
        return 1.0;
    }
    ((now_ms - event.target_ms) as f32 / (event.end_ms - event.target_ms) as f32).clamp(0.0, 1.0)
}

/// Reconstruct visual state with authored motion enabled.
pub fn visual_state_at(events: &[TimedVisualEvent], now_ms: i64) -> VisualState {
    visual_state_at_with_motion(events, now_ms, true)
}

/// Reconstruct visual state, resolving pans immediately and skipping movies
/// when background motion is disabled.
pub fn visual_state_at_with_motion(
    events: &[TimedVisualEvent],
    now_ms: i64,
    motion_enabled: bool,
) -> VisualState {
    let mut state = VisualState::default();
    for event in events
        .iter()
        .copied()
        .filter(|event| event.target_ms <= now_ms)
    {
        match event.kind {
            VisualEventKind::Replace { layer, asset_slot }
            | VisualEventKind::Swap { layer, asset_slot } => {
                if layer.is_movie() {
                    if motion_enabled {
                        state.movie = Some(MovieVisualState {
                            asset_id: asset_slot,
                            start_ms: event.target_ms,
                            geometry: None,
                        });
                    }
                } else {
                    state.layers.insert(
                        layer,
                        LayerVisualState {
                            asset_id: asset_slot,
                            geometry: None,
                        },
                    );
                }
            }
            VisualEventKind::ImagePan { layer, definition } => {
                state.layers.insert(
                    layer,
                    LayerVisualState {
                        asset_id: definition.asset_slot,
                        geometry: Some(pan_geometry(
                            definition,
                            event_progress(event, now_ms, motion_enabled),
                        )),
                    },
                );
            }
            VisualEventKind::MoviePan { definition } => {
                if motion_enabled {
                    state.movie = Some(MovieVisualState {
                        asset_id: definition.asset_slot,
                        start_ms: event.target_ms,
                        geometry: Some(pan_geometry(
                            definition,
                            event_progress(event, now_ms, true),
                        )),
                    });
                }
            }
        }
    }
    state
}

/// Build BPM-and-bar-length-aware visual events for a chart.
pub fn timed_visual_events(chart: &Chart) -> Vec<TimedVisualEvent> {
    let bpm_changes = dtx_core::timing::bpm_changes_from_chart(chart);
    let bar_changes = dtx_core::timing::bar_changes_from_chart(chart);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes,
        bar_changes: &bar_changes,
    };
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);

    let mut events = chart
        .chips
        .iter()
        .filter_map(|chip| {
            let target_ms =
                chip_time_ms_with_bpm_and_bar_changes(chip.measure, chip.value, base_bpm, timing);
            let kind = if let Some(layer) = BgaLayer::from_swap_channel(chip.channel) {
                VisualEventKind::Swap {
                    layer,
                    asset_slot: chip.wav_slot,
                }
            } else {
                let layer = BgaLayer::from_channel(chip.channel)?;
                if layer.is_movie() {
                    chart.assets.avi_pan.get(&chip.wav_slot).map_or(
                        VisualEventKind::Replace {
                            layer,
                            asset_slot: chip.wav_slot,
                        },
                        |definition| VisualEventKind::MoviePan {
                            definition: *definition,
                        },
                    )
                } else {
                    chart.assets.bga_pan.get(&chip.wav_slot).map_or(
                        VisualEventKind::Replace {
                            layer,
                            asset_slot: chip.wav_slot,
                        },
                        |definition| VisualEventKind::ImagePan {
                            layer,
                            definition: *definition,
                        },
                    )
                }
            };
            let duration_ticks = match kind {
                VisualEventKind::ImagePan { definition, .. }
                | VisualEventKind::MoviePan { definition } => definition.duration_ticks,
                _ => 0,
            };
            let end_ms = chip_time_ms_with_bpm_and_bar_changes(
                chip.measure,
                chip.value + duration_ticks as f32 / 384.0,
                base_bpm,
                timing,
            );
            Some(TimedVisualEvent {
                target_ms,
                end_ms,
                kind,
            })
        })
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.target_ms);
    events
}

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

/// Prepared visual events and resolved chart-relative media paths.
#[derive(bevy::prelude::Resource, Debug, Default, Clone)]
pub struct ActiveChartRes {
    /// Chart directory, when the chart has a source path.
    pub source_dir: Option<PathBuf>,
    /// Typed visual events in chart-time order.
    pub events: Vec<TimedVisualEvent>,
    /// Resolved BMP paths by asset id.
    pub bmp_paths: HashMap<u32, PathBuf>,
    /// Resolved AVI paths by asset id.
    pub avi_paths: HashMap<u32, PathBuf>,
}

impl ActiveChartRes {
    /// Prepare visual playback state from a parsed chart.
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

    /// Resolved BMP path for an asset id.
    pub fn bmp_path(&self, id: u32) -> Option<&Path> {
        self.bmp_paths.get(&id).map(PathBuf::as_path)
    }

    /// Resolved AVI path for an asset id.
    pub fn avi_path(&self, id: u32) -> Option<&Path> {
        self.avi_paths.get(&id).map(PathBuf::as_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_events_respect_bpm_change() {
        let src = b"#TITLE: T\n#BPM: 120\n#BMP01: red.png\n#00304: 01\n#00103: C0\n#00504: 01\n";
        let chart = dtx_core::parser::parse(&src[..]).expect("parse");
        let events = timed_visual_events(&chart);
        let late = events.iter().max_by_key(|event| event.target_ms).unwrap();
        assert!(late.target_ms < 10_000);
    }
}
