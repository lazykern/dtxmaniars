//! Spatial bind display for the Bindings tab (spec §5).
//!
//! While the Bindings tab is active and a channel is selected (by a pad hit, via
//! `SelectedChannel`), the selected channel's lane column is outlined on the
//! shrunk playfield and its bound sources are drawn at the lane bottom
//! (DJMAX-style, e.g. "C  N38"). The geometry is read straight from
//! `PlayfieldLayout` (`col_left`/`col_width`/`lane_top`/`lane_height`), which is
//! already resolved through the `StageRect`, so the overlay follows the stage
//! transform (Fit preset) for free.
//!
//! Lifecycle mirrors `editor/stage.rs`'s `StageOutline`: one outline node + one
//! label node spawned when the surface opens, despawned on close / `OnExit`. An
//! `editor_open`-gated sync tracks them to the lane rect each frame and hides
//! them when off the Bindings tab, when no channel is selected, or while peeking
//! (Tab held to preview the play view).

use bevy::prelude::*;
use dtx_config::BindSource;

use crate::bindings::LiveBindings;
use crate::lanes::Lanes;
use crate::layout::PlayfieldLayout;

use super::bindings_capture::SelectedChannel;

/// Transparent, accent-bordered node tracking the selected channel's lane column.
#[derive(Component)]
struct BindLaneOutline;

/// Text node at the lane bottom listing the selected channel's bound sources.
#[derive(Component)]
struct BindSourceLabel;

/// GLOBAL z so the selected-lane accent stacks above the preview scrim (1500)
/// and the stage outline (1900), still under the chrome (2000). The nodes stay
/// `HudRoot` children — `GlobalZIndex` changes stacking only, the stage
/// transform still inherits.
const OUTLINE_Z: i32 = 1910;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_overlay_on_open.run_if(in_state(game_shell::AppState::Performance)),
            sync_bind_overlay.run_if(super::editor_open),
        ),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_overlay);
}

/// Short label for a `KeyCode` (`KeyX` → "X", `Digit1` → "1"); mirrors the
/// `bindings_panel` helper (kept private there, replicated here to stay decoupled).
fn key_label(k: KeyCode) -> String {
    let s = format!("{k:?}");
    if let Some(rest) = s.strip_prefix("Key") {
        return rest.to_string();
    }
    if let Some(rest) = s.strip_prefix("Digit") {
        return rest.to_string();
    }
    s
}

/// Human label for a bind source (keyboard key name or `N{note}` for MIDI).
fn source_label(src: &BindSource) -> String {
    match src {
        BindSource::Key(k) => key_label(*k),
        BindSource::Midi { note } => format!("N{note}"),
    }
}

/// Spawn the outline + label nodes when the surface opens; despawn when it
/// closes. Because they are despawned on close, the `editor_open`-gated sync
/// never runs while closed, so the overlay is guaranteed hidden then.
fn spawn_overlay_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    roots: Query<Entity, With<crate::hud::HudRoot>>,
    existing: Query<Entity, Or<(With<BindLaneOutline>, With<BindSourceLabel>)>>,
) {
    if !open.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    // The overlay reads full-window `PlayfieldLayout` coords, so it must ride the
    // same `HudRoot` stage transform as the playfield to stay glued to the lane
    // while the scene is shrunk into the miniature. Parent it under HudRoot.
    let Ok(root) = roots.single() else {
        return;
    };
    let outline = commands
        .spawn((
            BindLaneOutline,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor::all(Color::WHITE),
            Visibility::Hidden,
            GlobalZIndex(OUTLINE_Z),
            Pickable::IGNORE,
        ))
        .id();
    let label = commands
        .spawn((
            BindSourceLabel,
            Node {
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Text::new(String::new()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(Color::WHITE),
            Visibility::Hidden,
            GlobalZIndex(OUTLINE_Z),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_children(&[outline, label]);
}

fn despawn_overlay(
    mut commands: Commands,
    existing: Query<Entity, Or<(With<BindLaneOutline>, With<BindSourceLabel>)>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}

/// Track the outline + label to the selected channel's lane rect and show them
/// only on the Bindings tab with a live selection and no peek in progress.
#[allow(clippy::too_many_arguments)]
fn sync_bind_overlay(
    state: Res<super::PreviewState>,
    selected: Res<SelectedChannel>,
    pfl: Res<PlayfieldLayout>,
    lanes: Res<Lanes>,
    live: Res<LiveBindings>,
    mut outline: Query<
        (&mut Node, &mut Visibility, &mut BorderColor),
        (With<BindLaneOutline>, Without<BindSourceLabel>),
    >,
    mut label: Query<
        (&mut Node, &mut Visibility, &mut Text, &mut TextColor),
        (With<BindSourceLabel>, Without<BindLaneOutline>),
    >,
) {
    let peeking = state.peeking;
    let on_bindings = state.tab == game_shell::CustomizeTab::Bindings;
    // Resolve the selected channel's column (None → nothing to draw).
    let col = if on_bindings && !peeking {
        selected.0.and_then(|ch| lanes.col_of(ch).map(|c| (ch, c)))
    } else {
        None
    };

    let Ok((mut o_node, mut o_vis, mut o_border)) = outline.single_mut() else {
        return;
    };
    let Ok((mut l_node, mut l_vis, mut l_text, mut l_color)) = label.single_mut() else {
        return;
    };

    let Some((ch, col)) = col else {
        *o_vis = Visibility::Hidden;
        *l_vis = Visibility::Hidden;
        return;
    };

    let left = pfl.col_left(col);
    let width = pfl.col_width(col);
    let top = pfl.lane_top();
    let height = pfl.lane_height();
    let accent = lanes.column_color(col);

    o_node.left = Val::Px(left);
    o_node.top = Val::Px(top);
    o_node.width = Val::Px(width);
    o_node.height = Val::Px(height);
    *o_border = BorderColor::all(accent);
    *o_vis = Visibility::Inherited;

    // Bound sources drawn at the lane bottom (just under the judge line).
    let sources = live.0.map.get(&ch).cloned().unwrap_or_default();
    let text: String = sources
        .iter()
        .map(source_label)
        .collect::<Vec<_>>()
        .join("  ");
    l_node.left = Val::Px(left);
    l_node.top = Val::Px(top + height + 2.0);
    l_node.width = Val::Px(width.max(1.0));
    l_text.0 = text;
    *l_color = TextColor(accent);
    *l_vis = Visibility::Inherited;
}
