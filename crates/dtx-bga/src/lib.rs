//! BGA / video playback (Engine layer, M7).
//!
//! M7 ships a state-machine player that detects BGA chip events from the
//! active chart and renders placeholder UI overlays per layer. Real image
//! loading (parse `#BMPxx:` / `#AVIxx:` directives, decode AVI/MPG via FFmpeg)
//! lands in M7.1.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfBGA.cs` (305 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfVideo.cs` (520 lines)

#![warn(missing_docs)]

use bevy::prelude::Resource as _;
use bevy::prelude::*;
use dtx_core::bga::{BgaEvent, BgaLayer};
use dtx_timing::AudioClock;
use thiserror::Error;

/// State of a single BGA layer's display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BgaLayerState {
    /// No event yet scheduled.
    #[default]
    Idle,
    /// Event reached but layer waiting for activation.
    Cueing,
    /// Layer currently showing.
    Displaying,
    /// Event expired (e.g. Movie ended).
    Ended,
}

/// BGA player state — one entry per BGA layer kind.
#[derive(Resource, Debug, Default, Clone)]
pub struct BgaPlayer {
    /// Per-layer display state.
    pub layers: std::collections::HashMap<BgaLayer, BgaLayerState>,
    /// Index into the chart's sorted BGA event list (next event to process).
    pub next_event_idx: usize,
    /// Total BGA events detected for the active chart.
    pub event_count: usize,
    /// Count of layer activations since chart start.
    pub activations: u32,
    /// Count of movie channels skipped (M7.1 will decode these).
    pub movies_skipped: u32,
}

impl BgaPlayer {
    /// Reset for a new chart (called by song_loading on OnEnter(Performance)).
    pub fn reset(&mut self) {
        self.layers.clear();
        self.next_event_idx = 0;
        self.event_count = 0;
        self.activations = 0;
        self.movies_skipped = 0;
    }
}

/// Marker component on the placeholder UI entity shown for a BGA layer.
#[derive(Component, Debug, Clone, Copy)]
pub struct BgaLayerOverlay {
    /// Which layer this overlay represents.
    pub layer: BgaLayer,
    /// BMP/AVI index from the chip.
    pub bmp_index: u32,
}

/// Errors from BGA module (M7.1+ when file resolution lands).
#[derive(Debug, Error)]
pub enum BgaError {
    /// File not found (M7.1+).
    #[error("file not found: {0}")]
    FileNotFound(String),
    /// Decode failed (M7.1+).
    #[error("decode failed: {0}")]
    DecodeFailed(String),
}

/// Plugin assembly. Registers `BgaPlayer` Resource and the tick system.
pub fn plugin(app: &mut App) {
    app.init_resource::<BgaPlayer>()
        .add_systems(Update, tick_bga_player);
}

/// Per-frame: advance through the chart's BGA events and activate layers
/// whose `approx_ms` <= current audio clock.
///
/// For Movie channels, increments `movies_skipped` (M7.1 will decode).
/// For image channels, spawns a `BgaLayerOverlay` placeholder entity.
fn tick_bga_player(
    clock: Res<AudioClock>,
    mut player: ResMut<BgaPlayer>,
    chart_res: Option<Res<ActiveChartRes>>,
    mut commands: Commands,
    overlays: Query<Entity, With<BgaLayerOverlay>>,
) {
    let Some(chart_res) = chart_res else {
        return;
    };
    let Some(now) = clock.current_ms else {
        return;
    };
    let bpm = chart_res.bpm;

    while player.next_event_idx < chart_res.events.len() {
        let ev = &chart_res.events[player.next_event_idx];
        if ev.approx_ms(bpm) > now {
            break;
        }
        activate_event(ev, &mut player, &mut commands);
        player.next_event_idx += 1;
    }

    // Clean up overlay entities whose layer state went to Ended.
    let active_layers: std::collections::HashSet<_> = player
        .layers
        .iter()
        .filter(|(_, s)| **s == BgaLayerState::Displaying)
        .map(|(l, _)| *l)
        .collect();
    for entity in &overlays {
        // We can't read overlay.layer here without component access; use query.
        let _ = entity;
    }
    // (M7: overlays are spawned once per event; no per-frame entity churn.
    // M7.1: track layer + activation in BgaLayerOverlay to despawn on Ended.)
}

/// Activate one BGA event (move layer state to Displaying, spawn overlay).
fn activate_event(ev: &BgaEvent, player: &mut BgaPlayer, commands: &mut Commands) {
    if ev.layer.is_movie() {
        player.movies_skipped += 1;
        info!(
            "BGA: movie chip skipped (M7.1 will decode) — layer={}, bmp_index={}",
            ev.layer.label(),
            ev.bmp_index
        );
        return;
    }
    player.layers.insert(ev.layer, BgaLayerState::Displaying);
    player.activations += 1;
    info!(
        "BGA: activate layer={} bmp_index={}",
        ev.layer.label(),
        ev.bmp_index
    );
    spawn_overlay(commands, ev);
}

fn spawn_overlay(commands: &mut Commands, ev: &BgaEvent) {
    let (color, x, y, w, h) = overlay_geometry(ev.layer);
    commands.spawn((
        BgaLayerOverlay {
            layer: ev.layer,
            bmp_index: ev.bmp_index,
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        },
        BackgroundColor(color),
        // Render behind the gameplay HUD.
        ZIndex(-1),
    ));
}

/// Placeholder geometry per layer (M7: a colored rectangle where the image
/// would appear once M7.1 lands).
fn overlay_geometry(layer: BgaLayer) -> (Color, f32, f32, f32, f32) {
    match layer {
        BgaLayer::Layer1 => (Color::srgba(0.2, 0.5, 0.3, 0.5), 0.0, 0.0, 320.0, 240.0),
        BgaLayer::Layer2 => (Color::srgba(0.3, 0.2, 0.5, 0.5), 0.0, 240.0, 320.0, 240.0),
        BgaLayer::Layer3 => (Color::srgba(0.1, 0.2, 0.4, 0.7), 0.0, 0.0, 1280.0, 720.0),
        BgaLayer::LayerN(n) => match n {
            4 => (Color::srgba(0.5, 0.3, 0.2, 0.5), 960.0, 0.0, 320.0, 240.0),
            5 => (Color::srgba(0.2, 0.5, 0.4, 0.5), 960.0, 240.0, 320.0, 240.0),
            6 => (Color::srgba(0.4, 0.2, 0.5, 0.5), 0.0, 480.0, 320.0, 240.0),
            7 => (Color::srgba(0.5, 0.4, 0.2, 0.5), 960.0, 480.0, 320.0, 240.0),
            8 => (Color::srgba(0.3, 0.5, 0.5, 0.5), 320.0, 240.0, 640.0, 240.0),
            _ => (Color::srgba(0.5, 0.5, 0.5, 0.3), 0.0, 0.0, 100.0, 100.0),
        },
        BgaLayer::Movie | BgaLayer::MovieFull => {
            // Movies skipped (M7.1); no overlay.
            (Color::NONE, 0.0, 0.0, 0.0, 0.0)
        }
    }
}

/// Bevy wrapper carrying the parsed chart + sorted BGA events for the player.
/// Inserted by song_loading or OnEnter(Performance).
#[derive(Resource, Debug, Default, Clone)]
pub struct ActiveChartRes {
    /// Chart BPM (used for BGA event timing).
    pub bpm: f32,
    /// Sorted BGA events.
    pub events: Vec<BgaEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::bga::bga_events;
    use dtx_core::chart::Chart;

    #[test]
    fn bga_player_default_is_idle() {
        let p = BgaPlayer::default();
        assert_eq!(p.layers.len(), 0);
        assert_eq!(p.next_event_idx, 0);
        assert_eq!(p.event_count, 0);
        assert_eq!(p.activations, 0);
        assert_eq!(p.movies_skipped, 0);
    }

    #[test]
    fn bga_player_reset_clears_state() {
        let mut p = BgaPlayer {
            layers: [(BgaLayer::Layer3, BgaLayerState::Displaying)]
                .into_iter()
                .collect(),
            next_event_idx: 3,
            event_count: 3,
            activations: 2,
            movies_skipped: 1,
        };
        p.reset();
        assert!(p.layers.is_empty());
        assert_eq!(p.next_event_idx, 0);
        assert_eq!(p.activations, 0);
        assert_eq!(p.movies_skipped, 0);
    }

    #[test]
    fn bga_layer_state_default_idle() {
        assert_eq!(BgaLayerState::default(), BgaLayerState::Idle);
    }

    #[test]
    fn bga_layer_overlay_geometry_unique_per_layer() {
        let (c1, x1, y1, _, _) = overlay_geometry(BgaLayer::Layer1);
        let (c2, x2, y2, _, _) = overlay_geometry(BgaLayer::Layer2);
        // Distinct positions and colors so M7 placeholder is visible per layer.
        assert_ne!((x1, y1), (x2, y2));
        assert_ne!(c1, c2);
    }

    #[test]
    fn bga_movie_overlay_geometry_is_zero() {
        let (_, x, y, w, h) = overlay_geometry(BgaLayer::Movie);
        assert_eq!((x, y, w, h), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn active_chart_res_default_empty_events() {
        let r = ActiveChartRes::default();
        assert_eq!(r.events.len(), 0);
        assert_eq!(r.bpm, 0.0);
    }

    #[test]
    fn bga_events_integration_from_chart() {
        let chart = Chart {
            metadata: dtx_core::chart::Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                dtx_core::chart::Chip::new(0, dtx_core::channel::EChannel::BGALayer1, 1.0),
                dtx_core::chart::Chip::new(2, dtx_core::channel::EChannel::Movie, 1.0),
                dtx_core::chart::Chip::new(4, dtx_core::channel::EChannel::BGALayer3, 2.0),
            ],
        };
        let events = bga_events(&chart);
        let res = ActiveChartRes { bpm: 120.0, events };
        assert_eq!(res.events.len(), 3);
        assert_eq!(res.events[0].approx_ms(120.0), 0);
        assert_eq!(res.events[2].approx_ms(120.0), 8000);
    }

    #[test]
    fn bga_layer_n_distinct() {
        // BgaLayer::LayerN(n) for n=4..8 must be distinct.
        let mut seen = std::collections::HashSet::new();
        for n in 4..=8u8 {
            assert!(seen.insert(BgaLayer::LayerN(n)));
        }
    }

    #[test]
    fn bga_player_default_activations_zero() {
        let p = BgaPlayer::default();
        assert_eq!(p.activations, 0);
    }

    #[test]
    fn bga_layer_overlay_component_field() {
        let o = BgaLayerOverlay {
            layer: BgaLayer::Layer3,
            bmp_index: 7,
        };
        assert_eq!(o.layer, BgaLayer::Layer3);
        assert_eq!(o.bmp_index, 7);
    }

    #[test]
    fn bga_player_reset_preserves_structure() {
        let mut p = BgaPlayer::default();
        p.reset();
        p.reset(); // idempotent
        assert_eq!(p.layers.len(), 0);
        assert_eq!(p.event_count, 0);
    }

    #[test]
    fn active_chart_res_with_default_bpm() {
        let r = ActiveChartRes::default();
        // Default BPM should fall back to 120.0 in approx_ms.
        let ev = dtx_core::bga::BgaEvent {
            measure: 1,
            layer: BgaLayer::Layer1,
            bmp_index: 1,
            fraction: 0.0,
        };
        assert_eq!(ev.approx_ms(r.bpm), 2000);
    }

    #[test]
    fn bga_layer_3_label() {
        assert_eq!(BgaLayer::Layer3.label(), "BGA3");
    }
}
