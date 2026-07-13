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
// Bevy systems take many params; a Bevy-idiomatic false-positive, allowed
// crate-wide (same as game-results).
#![allow(clippy::too_many_arguments)]

use std::collections::HashSet;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
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

/// Optional parent entity for BGA overlays. A Game crate sets this to its scene
/// root (e.g. drums `HudRoot`) so image/movie overlays become children of that
/// root — inheriting its stage `UiTransform` (so they shrink/align with the
/// scene in the Customize editor) and its stacking context (so negative `ZIndex`
/// keeps them behind lanes/HUD but still above the root's own background).
/// `None` (default) spawns overlays at the window root, as before.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct BgaParent(pub Option<Entity>);

/// Live visual settings, derived from `dtx_config::SystemConfig`. Alpha values
/// are pre-divided to the 0.0..=1.0 render range.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct BgaSettings {
    /// Whether static `#BMP` image layers are shown.
    pub images_enabled: bool,
    /// Whether `#AVI` movie playback is shown.
    pub movie_enabled: bool,
    /// Whether authored pan/swap animation may advance.
    pub motion_enabled: bool,
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
            motion_enabled: true,
            image_alpha: 1.0,
            movie_alpha: 1.0,
        }
    }
}

impl From<&dtx_config::SystemConfig> for BgaSettings {
    fn from(value: &dtx_config::SystemConfig) -> Self {
        Self::from_configs(value, &dtx_config::AccessibilityConfig::default())
    }
}

impl BgaSettings {
    /// Resolve effective visual capabilities from chart-display and
    /// accessibility settings. Static images remain available when motion is off.
    pub fn from_configs(
        system: &dtx_config::SystemConfig,
        accessibility: &dtx_config::AccessibilityConfig,
    ) -> Self {
        Self {
            images_enabled: system.bga_enabled,
            movie_enabled: system.movie_enabled && accessibility.background_motion,
            motion_enabled: accessibility.background_motion,
            image_alpha: system.bg_alpha as f32 / 255.0,
            movie_alpha: system.movie_alpha as f32 / 255.0,
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
    /// Last observed `BgaClock` time, for seek/discontinuity detection.
    pub last_clock_ms: i64,
}

impl BgaPlayer {
    /// Reset for a new chart (called on Performance entry / exit).
    pub fn reset(&mut self) {
        self.next_event_idx = 0;
        self.event_count = 0;
        self.warned_missing.clear();
        self.active_movie = None;
        self.movie_start_ms = 0;
        self.last_clock_ms = 0;
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

/// Marker on the fullscreen movie container node (aspect-fit letterbox parent).
#[derive(Component, Debug, Clone, Copy)]
pub struct MovieOverlay;

/// Marker on the inner node whose `ImageNode` shows the decoded movie texture.
#[derive(Component, Debug, Clone, Copy)]
pub struct MovieImage;

/// Runtime state for the active movie: decode worker, reusable texture, and its
/// current dimensions. Not `Clone` (owns a decode thread), so it lives outside
/// [`BgaPlayer`].
#[derive(Resource, Default)]
pub struct MovieRuntime {
    worker: Option<MovieWorker>,
    /// Asset id the worker is currently decoding.
    current_id: Option<u32>,
    /// Chart time (ms) the active movie event started at.
    start_ms: i64,
    /// Reusable texture handle the movie frames upload into.
    texture: Option<Handle<Image>>,
    /// Current texture dimensions (width, height).
    dims: (u32, u32),
    /// Asset ids already warned about (missing AVI path).
    warned_missing: HashSet<u32>,
}

impl MovieRuntime {
    /// Stop the worker and drop texture/tracking state.
    pub fn stop_movie(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
        self.current_id = None;
        self.start_ms = 0;
        self.texture = None;
        self.dims = (0, 0);
        self.warned_missing.clear();
    }
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

/// True when the clock moved backward or jumped forward by more than 250 ms —
/// a practice seek or restart rather than a normal per-frame advance.
pub fn clock_discontinuity(prev_ms: i64, now_ms: i64) -> bool {
    now_ms < prev_ms || now_ms - prev_ms > 250
}

/// Verify a decoded frame's byte length matches `width * height * 4` (RGBA).
fn validate_frame_len(width: u32, height: u32, len: usize) -> Result<(), BgaError> {
    let expected = width as usize * height as usize * 4;
    if len == expected {
        Ok(())
    } else {
        Err(BgaError::DecodeFailed(format!(
            "frame length {len} != {expected} ({width}x{height})"
        )))
    }
}

/// For a seek to `now_ms`, the last image event per layer and the last movie
/// event, both at or before `now_ms`. Used to reconstruct visible state.
fn rebuild_state(
    events: &[TimedVisualEvent],
    now_ms: i64,
) -> (std::collections::HashMap<BgaLayer, u32>, Option<(u32, i64)>) {
    let mut images: std::collections::HashMap<BgaLayer, u32> = std::collections::HashMap::new();
    let mut movie: Option<(u32, i64)> = None;
    for event in events.iter().filter(|e| e.target_ms <= now_ms) {
        if event.layer.is_movie() {
            movie = Some((event.asset_id, event.target_ms));
        } else {
            images.insert(event.layer, event.asset_id);
        }
    }
    (images, movie)
}

/// Plugin assembly. Registers resources and the per-frame visual tick.
pub fn plugin(app: &mut App) {
    app.init_resource::<BgaPlayer>()
        .init_resource::<BgaClock>()
        .init_resource::<BgaSettings>()
        .init_resource::<BgaParent>()
        .init_resource::<MovieRuntime>()
        .add_systems(
            Update,
            (tick_bga_visuals, apply_image_settings, drive_movie)
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
    bga_parent: Res<BgaParent>,
) {
    let Some(chart_res) = chart_res else {
        return;
    };
    let now = clock.current_ms;

    // Practice seek / restart: rebuild the cursor and visible layers instead of
    // replaying every intervening event.
    if clock_discontinuity(player.last_clock_ms, now) {
        rebuild_on_seek(
            now,
            &chart_res,
            &settings,
            &mut player,
            &mut commands,
            &asset_server,
            &overlays,
            &bga_parent,
        );
    }
    player.last_clock_ms = now;

    while player.next_event_idx < chart_res.events.len() {
        let event = chart_res.events[player.next_event_idx];
        if event.target_ms > now {
            break;
        }
        player.next_event_idx += 1;
        if event.layer.is_movie() {
            // Hand the movie subsystem the newest movie to play.
            player.active_movie = Some(event.asset_id);
            player.movie_start_ms = event.target_ms;
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
            &bga_parent,
        );
    }
}

/// Reconstruct the event cursor, static layers, and active movie for a seek to
/// `now_ms`: despawn all image overlays, respawn the last image per layer, and
/// select the last movie at or before `now_ms`.
#[allow(clippy::too_many_arguments)]
fn rebuild_on_seek(
    now_ms: i64,
    chart_res: &ActiveChartRes,
    settings: &BgaSettings,
    player: &mut BgaPlayer,
    commands: &mut Commands,
    asset_server: &AssetServer,
    overlays: &Query<(Entity, &BgaLayerOverlay)>,
    bga_parent: &BgaParent,
) {
    player.next_event_idx = chart_res
        .events
        .partition_point(|event| event.target_ms <= now_ms);

    for (entity, _) in overlays.iter() {
        commands.entity(entity).despawn();
    }

    let (images, movie) = rebuild_state(&chart_res.events, now_ms);
    for (layer, asset_id) in images {
        spawn_image_overlay(
            &TimedVisualEvent {
                target_ms: now_ms,
                layer,
                asset_id,
            },
            chart_res,
            settings,
            player,
            commands,
            asset_server,
            bga_parent,
        );
    }
    match movie {
        Some((id, start)) => {
            player.active_movie = Some(id);
            player.movie_start_ms = start;
        }
        None => player.active_movie = None,
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
    bga_parent: &BgaParent,
) {
    for (entity, overlay) in overlays.iter() {
        if overlay.layer == event.layer {
            commands.entity(entity).despawn();
        }
    }
    spawn_image_overlay(
        event,
        chart_res,
        settings,
        player,
        commands,
        asset_server,
        bga_parent,
    );
}

/// Spawn one image overlay entity (no despawn of existing layers). Missing
/// assets warn once and spawn nothing.
fn spawn_image_overlay(
    event: &TimedVisualEvent,
    chart_res: &ActiveChartRes,
    settings: &BgaSettings,
    player: &mut BgaPlayer,
    commands: &mut Commands,
    asset_server: &AssetServer,
    bga_parent: &BgaParent,
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

    let (x, y, w, h) = image_layer_geometry(event.layer);
    let visibility = if settings.images_enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let overlay = commands
        .spawn((
            BgaLayerOverlay {
                layer: event.layer,
                asset_id: event.asset_id,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(x),
                top: Val::Percent(y),
                width: Val::Percent(w),
                height: Val::Percent(h),
                ..default()
            },
            ImageNode {
                image: asset_server.load(path.to_string_lossy().to_string()),
                color: Color::WHITE.with_alpha(settings.image_alpha),
                ..default()
            },
            visibility,
            // Above the movie (-3) and playfield backboard, below the lane
            // backdrop (-1) and lanes/HUD.
            ZIndex(-2),
        ))
        .id();
    parent_overlay(commands, overlay, bga_parent);
}

/// Attach a freshly-spawned BGA overlay to the configured scene root, if any, so
/// it inherits the stage transform and stacking context. No-op at the window
/// root (default), where the overlay stays a top-level node.
fn parent_overlay(commands: &mut Commands, overlay: Entity, bga_parent: &BgaParent) {
    if let Some(parent) = bga_parent.0 {
        commands.entity(parent).add_child(overlay);
    }
}

/// Layout rectangle per image layer as a **percentage** of the parent (the scene
/// root / window), derived from the DTXMania 1280x720 reference. Percent keeps
/// layers resolution-independent — Layer3 fills the frame; small layers tile the
/// corners/edges. (320/1280 = 25%, 240/720 ≈ 33.33%.)
fn image_layer_geometry(layer: BgaLayer) -> (f32, f32, f32, f32) {
    const W: f32 = 25.0; // 320 / 1280
    const H: f32 = 100.0 / 3.0; // 240 / 720
    match layer {
        BgaLayer::Layer1 => (0.0, 0.0, W, H),
        BgaLayer::Layer2 => (0.0, H, W, H),
        BgaLayer::Layer3 => (0.0, 0.0, 100.0, 100.0),
        BgaLayer::LayerN(n) => match n {
            4 => (75.0, 0.0, W, H),
            5 => (75.0, H, W, H),
            6 => (0.0, 2.0 * H, W, H),
            7 => (75.0, 2.0 * H, W, H),
            _ => (W, H, 50.0, H),
        },
        BgaLayer::Movie | BgaLayer::MovieFull => (0.0, 0.0, 0.0, 0.0),
    }
}

/// Drive the active movie: (re)start the worker when the chart selects a new
/// AVI, upload the newest due frame into the reusable texture, and keep one
/// aspect-fit fullscreen overlay behind lanes and HUD in sync with settings.
#[allow(clippy::too_many_arguments)]
fn drive_movie(
    clock: Res<BgaClock>,
    settings: Res<BgaSettings>,
    chart_res: Option<Res<ActiveChartRes>>,
    player: Res<BgaPlayer>,
    mut runtime: ResMut<MovieRuntime>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
    root_q: Query<Entity, With<MovieOverlay>>,
    mut image_q: Query<(&mut ImageNode, &mut Visibility, &mut Node), With<MovieImage>>,
    bga_parent: Res<BgaParent>,
) {
    let Some(chart_res) = chart_res else {
        return;
    };
    let now = clock.current_ms;

    if !settings.movie_enabled {
        if runtime.worker.is_some() || runtime.texture.is_some() {
            runtime.stop_movie();
        }
        for entity in &root_q {
            commands.entity(entity).despawn();
        }
        return;
    }

    // (Re)start the worker when the chart selects a different movie; stop and
    // tear down the overlay when a seek lands before any movie event.
    match player.active_movie {
        Some(id) if runtime.current_id != Some(id) => {
            runtime.stop_movie();
            runtime.current_id = Some(id);
            runtime.start_ms = player.movie_start_ms;
            match chart_res.avi_path(id) {
                Some(path) => runtime.worker = Some(MovieWorker::spawn(path.to_path_buf())),
                None => {
                    if runtime.warned_missing.insert(id) {
                        warn!("BGA: missing movie asset id={id}");
                    }
                }
            }
        }
        Some(_) => {}
        None => {
            if runtime.worker.is_some() || runtime.texture.is_some() {
                runtime.stop_movie();
            }
            for entity in &root_q {
                commands.entity(entity).despawn();
            }
            return;
        }
    }

    // Pull the newest due frame for the current movie time.
    let want = (now - runtime.start_ms).max(0);
    let frame = if let Some(worker) = runtime.worker.as_ref() {
        worker.set_target_ms(want);
        let frame = worker.newest_due_frame(want);
        if let Some(err) = worker.take_error() {
            warn!("BGA movie: {err}");
        }
        frame
    } else {
        None
    };
    if let Some(frame) = frame {
        upload_movie_frame(&mut runtime, &mut images, &frame);
    }

    let Some(texture) = runtime.texture.clone() else {
        return;
    };
    let aspect = if runtime.dims.1 > 0 {
        runtime.dims.0 as f32 / runtime.dims.1 as f32
    } else {
        16.0 / 9.0
    };
    let visibility = if settings.movie_enabled {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    if root_q.is_empty() {
        spawn_movie_overlay(
            &mut commands,
            texture,
            aspect,
            visibility,
            settings.movie_alpha,
            &bga_parent,
        );
    } else {
        for (mut image, mut vis, mut node) in image_q.iter_mut() {
            image.image = texture.clone();
            image.color = Color::WHITE.with_alpha(settings.movie_alpha);
            *vis = visibility;
            node.aspect_ratio = Some(aspect);
        }
    }
}

/// Upload a decoded frame into the reusable texture, recreating it when the
/// dimensions change. Invalid frame lengths are dropped, keeping the old frame.
fn upload_movie_frame(
    runtime: &mut MovieRuntime,
    images: &mut Assets<Image>,
    frame: &DecodedFrame,
) {
    if validate_frame_len(frame.width, frame.height, frame.rgba.len()).is_err() {
        return;
    }
    if runtime.dims == (frame.width, frame.height) {
        if let Some(handle) = runtime.texture.clone() {
            if let Some(mut image) = images.get_mut(&handle) {
                image.data = Some(frame.rgba.clone());
            }
            return;
        }
    }
    let image = Image::new(
        Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        frame.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    runtime.texture = Some(images.add(image));
    runtime.dims = (frame.width, frame.height);
}

/// Spawn the aspect-fit fullscreen movie overlay: a centering fullscreen parent
/// (`MovieOverlay`) holding an aspect-constrained child (`MovieImage`). Negative
/// Z keeps lanes and HUD above it.
fn spawn_movie_overlay(
    commands: &mut Commands,
    texture: Handle<Image>,
    aspect: f32,
    visibility: Visibility,
    alpha: f32,
    bga_parent: &BgaParent,
) {
    let overlay = commands
        .spawn((
            MovieOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            // Behind image layers (-2) and lanes/HUD, above playfield backboard.
            ZIndex(-3),
        ))
        .with_children(|parent| {
            parent.spawn((
                MovieImage,
                Node {
                    max_width: Val::Percent(100.0),
                    max_height: Val::Percent(100.0),
                    aspect_ratio: Some(aspect),
                    ..default()
                },
                ImageNode {
                    image: texture,
                    color: Color::WHITE.with_alpha(alpha),
                    ..default()
                },
                visibility,
            ));
        })
        .id();
    parent_overlay(commands, overlay, bga_parent);
}

/// Despawn all BGA image and movie overlays and reset player + movie runtime.
/// Idempotent: safe to call from any Performance-exit route.
#[allow(clippy::type_complexity)]
pub fn clear_visuals(
    mut commands: Commands,
    mut player: ResMut<BgaPlayer>,
    mut runtime: ResMut<MovieRuntime>,
    overlays: Query<Entity, Or<(With<BgaLayerOverlay>, With<MovieOverlay>)>>,
) {
    runtime.stop_movie();
    player.reset();
    for entity in &overlays {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_motion_off_keeps_static_images_only() {
        let accessibility = dtx_config::AccessibilityConfig {
            background_motion: false,
            ..Default::default()
        };
        let settings =
            BgaSettings::from_configs(&dtx_config::SystemConfig::default(), &accessibility);
        assert!(settings.images_enabled);
        assert!(!settings.movie_enabled);
        assert!(!settings.motion_enabled);
    }

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
        // Percent of the frame — the geometry is resolution-independent.
        assert_eq!(
            image_layer_geometry(BgaLayer::Layer3),
            (0.0, 0.0, 100.0, 100.0)
        );
    }

    #[test]
    fn image_layer_geometry_small_layers_distinct() {
        assert_ne!(
            image_layer_geometry(BgaLayer::Layer1),
            image_layer_geometry(BgaLayer::Layer2)
        );
    }

    #[test]
    fn clock_discontinuity_detects_seek_not_normal_frame() {
        assert!(!clock_discontinuity(1_000, 1_016));
        assert!(clock_discontinuity(5_000, 2_000));
        assert!(clock_discontinuity(1_000, 5_000));
    }

    #[test]
    fn validate_frame_len_matches_dimensions() {
        assert!(validate_frame_len(16, 16, 16 * 16 * 4).is_ok());
        assert!(validate_frame_len(16, 16, 10).is_err());
    }

    #[test]
    fn rebuild_state_picks_last_image_per_layer_and_last_movie() {
        let events = vec![
            TimedVisualEvent {
                target_ms: 0,
                layer: BgaLayer::Layer3,
                asset_id: 1,
            },
            TimedVisualEvent {
                target_ms: 100,
                layer: BgaLayer::Movie,
                asset_id: 7,
            },
            TimedVisualEvent {
                target_ms: 200,
                layer: BgaLayer::Layer3,
                asset_id: 2,
            },
            TimedVisualEvent {
                target_ms: 300,
                layer: BgaLayer::Movie,
                asset_id: 8,
            },
            TimedVisualEvent {
                target_ms: 500,
                layer: BgaLayer::Layer3,
                asset_id: 9,
            },
        ];
        let (images, movie) = rebuild_state(&events, 350);
        assert_eq!(images.get(&BgaLayer::Layer3), Some(&2));
        assert_eq!(movie, Some((8, 300)));
    }
}
