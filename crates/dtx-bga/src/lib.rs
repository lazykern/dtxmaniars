//! BGA / video playback (Engine layer, M7.1).
//!
//! Renders chart-authored `#BMP` images and (via [`video`]) `#AVI` movies
//! behind drum gameplay, synchronized to the gameplay clock mirrored into
//! [`BgaClock`]. Image layers replace the earlier colored placeholders.
//!
//! References:
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfBGA.cs` (305 lines)
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/CActPerfVideo.cs` (520 lines)

#![warn(missing_docs)]

use std::collections::HashSet;

use bevy::prelude::*;
use dtx_core::bga::BgaLayer;
use thiserror::Error;

pub mod chart;
pub mod video;

pub use chart::{ActiveChartRes, TimedVisualEvent};
pub use video::{DecodedFrame, MovieWorker};

/// Gameplay-clock bridge: `gameplay-drums` copies the authoritative chart time
/// (ms) here each frame so visual playback follows pause and practice seeks.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BgaClock {
    /// Current chart time in ms (mirror of `GameplayClock::current_ms`).
    pub current_ms: i64,
}

/// Live visual settings, derived from `dtx_config::SystemConfig`. Alpha values
/// are pre-divided to the 0.0..=1.0 render range.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct BgaSettings {
    /// Whether static `#BMP` image layers are shown.
    pub images_enabled: bool,
    /// Whether `#AVI` movie playback is shown.
    pub movie_enabled: bool,
    /// Image layer alpha (0.0..=1.0).
    pub image_alpha: f32,
    /// Movie alpha (0.0..=1.0).
    pub movie_alpha: f32,
}

impl Default for BgaSettings {
    fn default() -> Self {
        Self {
            images_enabled: true,
            movie_enabled: true,
            image_alpha: 1.0,
            movie_alpha: 1.0,
        }
    }
}

impl From<&dtx_config::SystemConfig> for BgaSettings {
    fn from(value: &dtx_config::SystemConfig) -> Self {
        Self {
            images_enabled: value.bga_enabled,
            movie_enabled: value.movie_enabled,
            image_alpha: value.bg_alpha as f32 / 255.0,
            movie_alpha: value.movie_alpha as f32 / 255.0,
        }
    }
}

/// System set wrapping the per-frame visual tick, so a Game crate can order its
/// clock-bridge system before `dtx-bga` consumes `BgaClock`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BgaSystems;

/// BGA player runtime state for the active chart.
#[derive(Resource, Debug, Default, Clone)]
pub struct BgaPlayer {
    /// Index into `ActiveChartRes::events` of the next event to process.
    pub next_event_idx: usize,
    /// Total visual events detected for the active chart.
    pub event_count: usize,
    /// `(layer, asset_id)` pairs already warned about (missing asset), so a
    /// broken reference logs once rather than every frame.
    pub warned_missing: HashSet<(BgaLayer, u32)>,
    /// Asset id of the currently-playing movie, if any.
    pub active_movie: Option<u32>,
    /// Chart time (ms) at which the active movie event started.
    pub movie_start_ms: i64,
}

impl BgaPlayer {
    /// Reset for a new chart (called on Performance entry / exit).
    pub fn reset(&mut self) {
        self.next_event_idx = 0;
        self.event_count = 0;
        self.warned_missing.clear();
        self.active_movie = None;
        self.movie_start_ms = 0;
    }
}

/// Marker on the UI entity showing one BGA image layer.
#[derive(Component, Debug, Clone, Copy)]
pub struct BgaLayerOverlay {
    /// Which layer this overlay represents.
    pub layer: BgaLayer,
    /// `#BMP` asset id currently displayed.
    pub asset_id: u32,
}

/// Errors from the BGA module.
#[derive(Debug, Error)]
pub enum BgaError {
    /// Referenced asset file was not found.
    #[error("file not found: {0}")]
    FileNotFound(String),
    /// Movie decode failed.
    #[error("decode failed: {0}")]
    DecodeFailed(String),
}

/// Plugin assembly. Registers resources and the per-frame visual tick.
pub fn plugin(app: &mut App) {
    app.init_resource::<BgaPlayer>()
        .init_resource::<BgaClock>()
        .init_resource::<BgaSettings>()
        .add_systems(
            Update,
            (tick_bga_visuals, apply_image_settings)
                .chain()
                .in_set(BgaSystems),
        );
}

/// Live-apply visual settings to existing image overlays without respawning:
/// toggle visibility from `images_enabled` and set alpha from `image_alpha`.
fn apply_image_settings(
    settings: Res<BgaSettings>,
    mut overlays: Query<(&mut Visibility, &mut ImageNode), With<BgaLayerOverlay>>,
) {
    if !settings.is_changed() {
        return;
    }
    let vis = if settings.images_enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (mut visibility, mut image) in overlays.iter_mut() {
        *visibility = vis;
        image.color = Color::WHITE.with_alpha(settings.image_alpha);
    }
}

/// Per-frame: advance through timed visual events whose `target_ms` has been
/// reached and render/replace static image layers. Movie events are handled by
/// the movie subsystem (Task 6); here they only advance the cursor.
fn tick_bga_visuals(
    clock: Res<BgaClock>,
    settings: Res<BgaSettings>,
    chart_res: Option<Res<ActiveChartRes>>,
    mut player: ResMut<BgaPlayer>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    overlays: Query<(Entity, &BgaLayerOverlay)>,
) {
    let Some(chart_res) = chart_res else {
        return;
    };
    let now = clock.current_ms;

    while player.next_event_idx < chart_res.events.len() {
        let event = chart_res.events[player.next_event_idx];
        if event.target_ms > now {
            break;
        }
        player.next_event_idx += 1;
        if event.layer.is_movie() {
            // Movie rendering handled by the movie subsystem.
            continue;
        }
        apply_image_event(
            &event,
            &chart_res,
            &settings,
            &mut player,
            &mut commands,
            &asset_server,
            &overlays,
        );
    }
}

/// Spawn (replacing any prior entity on the same layer) the image overlay for
/// a static visual event. Missing assets warn once and leave the layer as-is.
fn apply_image_event(
    event: &TimedVisualEvent,
    chart_res: &ActiveChartRes,
    settings: &BgaSettings,
    player: &mut BgaPlayer,
    commands: &mut Commands,
    asset_server: &AssetServer,
    overlays: &Query<(Entity, &BgaLayerOverlay)>,
) {
    let Some(path) = chart_res.bmp_path(event.asset_id) else {
        if player.warned_missing.insert((event.layer, event.asset_id)) {
            warn!(
                "BGA: missing image asset id={} for layer={}",
                event.asset_id,
                event.layer.label()
            );
        }
        return;
    };

    for (entity, overlay) in overlays.iter() {
        if overlay.layer == event.layer {
            commands.entity(entity).despawn();
        }
    }

    let (x, y, w, h) = image_layer_geometry(event.layer);
    let visibility = if settings.images_enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    commands.spawn((
        BgaLayerOverlay {
            layer: event.layer,
            asset_id: event.asset_id,
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(w),
            height: Val::Px(h),
            ..default()
        },
        ImageNode {
            image: asset_server.load(path.to_string_lossy().to_string()),
            color: Color::WHITE.with_alpha(settings.image_alpha),
            ..default()
        },
        visibility,
        ZIndex(-100),
    ));
}

/// Layout rectangle (px) per image layer at the 1280x720 reference. Fullscreen
/// for Layer3; corner/side tiles for the small layers.
fn image_layer_geometry(layer: BgaLayer) -> (f32, f32, f32, f32) {
    match layer {
        BgaLayer::Layer1 => (0.0, 0.0, 320.0, 240.0),
        BgaLayer::Layer2 => (0.0, 240.0, 320.0, 240.0),
        BgaLayer::Layer3 => (0.0, 0.0, 1280.0, 720.0),
        BgaLayer::LayerN(n) => match n {
            4 => (960.0, 0.0, 320.0, 240.0),
            5 => (960.0, 240.0, 320.0, 240.0),
            6 => (0.0, 480.0, 320.0, 240.0),
            7 => (960.0, 480.0, 320.0, 240.0),
            _ => (320.0, 240.0, 640.0, 240.0),
        },
        BgaLayer::Movie | BgaLayer::MovieFull => (0.0, 0.0, 0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bga_player_default_is_idle() {
        let p = BgaPlayer::default();
        assert_eq!(p.next_event_idx, 0);
        assert_eq!(p.event_count, 0);
        assert!(p.warned_missing.is_empty());
        assert!(p.active_movie.is_none());
    }

    #[test]
    fn bga_player_reset_clears_state() {
        let mut p = BgaPlayer {
            next_event_idx: 3,
            event_count: 3,
            active_movie: Some(2),
            movie_start_ms: 500,
            ..Default::default()
        };
        p.warned_missing.insert((BgaLayer::Layer1, 9));
        p.reset();
        assert_eq!(p.next_event_idx, 0);
        assert!(p.warned_missing.is_empty());
        assert!(p.active_movie.is_none());
        assert_eq!(p.movie_start_ms, 0);
    }

    #[test]
    fn bga_settings_map_existing_config_fields() {
        let system = dtx_config::SystemConfig {
            bga_enabled: false,
            movie_enabled: true,
            bg_alpha: 128,
            movie_alpha: 64,
            ..Default::default()
        };
        let settings = BgaSettings::from(&system);
        assert!(!settings.images_enabled);
        assert!(settings.movie_enabled);
        assert!((settings.image_alpha - 128.0 / 255.0).abs() < f32::EPSILON);
        assert!((settings.movie_alpha - 64.0 / 255.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bga_settings_default_enabled_full_alpha() {
        let s = BgaSettings::default();
        assert!(s.images_enabled);
        assert!(s.movie_enabled);
        assert_eq!(s.image_alpha, 1.0);
        assert_eq!(s.movie_alpha, 1.0);
    }

    #[test]
    fn image_layer_geometry_layer3_is_fullscreen() {
        assert_eq!(image_layer_geometry(BgaLayer::Layer3), (0.0, 0.0, 1280.0, 720.0));
    }

    #[test]
    fn image_layer_geometry_small_layers_distinct() {
        assert_ne!(
            image_layer_geometry(BgaLayer::Layer1),
            image_layer_geometry(BgaLayer::Layer2)
        );
    }
}
