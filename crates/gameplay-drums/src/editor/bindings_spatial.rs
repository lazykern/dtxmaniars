//! Spatial bind display for the Bindings tab (spec §5), reused by the Lanes
//! tab (Task 8) to light the selected lane.
//!
//! While the Controls tab is active and a channel is selected (by a pad hit,
//! via `SelectedChannel`), the selected channel's lane column is outlined on
//! the shrunk playfield and its bound sources are drawn at the lane bottom
//! (DJMAX-style, e.g. "C  N38"). While the Lanes tab is active and a row is
//! selected (via `lanes_panel::SelectedLane`), the SAME outline lights that
//! lane's column directly by index — no source label, since there's nothing
//! bind-related to show there. The geometry is read straight from
//! `PlayfieldLayout` (`col_left`/`col_width`/`lane_top`/`lane_height`), which is
//! already resolved through the `StageRect`, so the overlay follows the stage
//! transform (Fit preset) for free.
//!
//! Lifecycle mirrors `editor/stage.rs`'s `StageOutline`: one outline node + one
//! label node spawned when the surface opens, despawned on close / `OnExit`. An
//! `editor_open`-gated sync tracks them to the lane rect each frame and hides
//! them when off the Controls/Lanes tabs, when nothing is selected, or while
//! peeking (Tab held to preview the play view).

use bevy::prelude::*;
use dtx_input::BindSource;

use crate::bindings::LiveBindings;
use crate::lanes::Lanes;
use crate::layout::PlayfieldLayout;

use super::bindings_capture::SelectedChannel;
use super::chrome;

/// Transparent, accent-bordered node tracking the selected channel's lane column.
#[derive(Component)]
struct BindLaneOutline;

/// Text node at the lane bottom listing the selected channel's bound sources.
#[derive(Component)]
struct BindSourceLabel;

/// One pooled outline used to light an EXTRA lane while a shared bind chip is
/// hovered (`HighlightedChannels`), separate from the selected-channel
/// outline above. Indexed so `sync_hover_outlines` can pair pool slot `i`
/// with `HighlightedChannels.0[i]`.
#[derive(Component)]
struct HoverOutline(usize);

/// Upper bound on pool slots: a source can never be shared by more channels
/// than exist.
const HOVER_POOL_SIZE: usize = dtx_input::BINDABLE_CHANNELS.len();

/// Channels to additionally outline this frame — set when a shared bind chip
/// in the Controls tab is hovered/pressed (see
/// `bindings_panel::update_chip_hover_highlight`). Empty most of the time;
/// cleared as soon as the pointer leaves the chip.
#[derive(Resource, Default, Debug, Clone)]
pub struct HighlightedChannels(pub Vec<dtx_core::EChannel>);

/// GLOBAL z so the selected-lane accent stacks above the preview scrim (1500)
/// and the stage outline (1900), still under the chrome (2000). The nodes stay
/// `HudRoot` children — `GlobalZIndex` changes stacking only, the stage
/// transform still inherits.
const OUTLINE_Z: i32 = crate::ui_z::BIND_OVERLAY;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<HighlightedChannels>()
        .add_systems(
            Update,
            (
                spawn_overlay_on_open.run_if(in_state(game_shell::AppState::Performance)),
                sync_bind_overlay.run_if(super::editor_open),
                sync_hover_outlines.run_if(super::editor_open),
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
    existing: Query<
        Entity,
        Or<(
            With<BindLaneOutline>,
            With<BindSourceLabel>,
            With<HoverOutline>,
        )>,
    >,
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
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            Text::new(String::new()),
            TextLayout {
                linebreak: bevy::text::LineBreak::NoWrap,
                ..default()
            },
            dtx_ui::theme::Theme::font(12.0),
            TextColor(Color::WHITE),
            Visibility::Hidden,
            GlobalZIndex(OUTLINE_Z),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_children(&[outline, label]);

    let pool: Vec<Entity> = (0..HOVER_POOL_SIZE)
        .map(|slot| {
            commands
                .spawn((
                    HoverOutline(slot),
                    Node {
                        position_type: PositionType::Absolute,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor::all(chrome::ACCENT),
                    Visibility::Hidden,
                    GlobalZIndex(OUTLINE_Z),
                    Pickable::IGNORE,
                ))
                .id()
        })
        .collect();
    commands.entity(root).add_children(&pool);
}

fn despawn_overlay(
    mut commands: Commands,
    existing: Query<
        Entity,
        Or<(
            With<BindLaneOutline>,
            With<BindSourceLabel>,
            With<HoverOutline>,
        )>,
    >,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}

/// Track the outline + label to the selected channel's lane rect (Controls
/// tab) or the selected lane's column (Lanes tab), one at a time, and hide
/// both while peeking.
#[allow(clippy::too_many_arguments)]
fn sync_bind_overlay(
    state: Res<super::PreviewState>,
    selected: Res<SelectedChannel>,
    lane_selected: Res<super::lanes_panel::SelectedLane>,
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
    // Resolve which column to light, and (Controls only) which channel's
    // bound sources to caption underneath it.
    let col = if peeking {
        None
    } else if state.tab == game_shell::CustomizeTab::Controls {
        selected
            .0
            .and_then(|ch| lanes.col_of(ch).map(|c| (Some(ch), c)))
    } else if state.tab == game_shell::CustomizeTab::Lanes {
        lane_selected
            .0
            .filter(|&i| i < lanes.0.lanes.len())
            .map(|i| (None, i))
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

    // Lanes-tab selection: light the column only, no source caption.
    let Some(ch) = ch else {
        *l_vis = Visibility::Hidden;
        return;
    };

    // Bound sources drawn at the lane bottom (just under the judge line). Cap
    // the count so the auto-width pill stays narrow (rightmost lanes would run
    // off the playfield otherwise); the full list lives in the left panel.
    let sources = live.0.map.get(&ch).cloned().unwrap_or_default();
    let shown = 3;
    let mut text: String = sources
        .iter()
        .take(shown)
        .map(source_label)
        .collect::<Vec<_>>()
        .join("  ");
    if sources.len() > shown {
        text.push_str(&format!("  +{}", sources.len() - shown));
    }
    l_node.left = Val::Px(left);
    l_node.top = Val::Px(top + height + 2.0);
    l_text.0 = text;
    *l_color = TextColor(accent);
    *l_vis = Visibility::Inherited;
}

/// Light every lane in `HighlightedChannels` (set while a shared chip is
/// hovered) using the pooled outlines, independent of the selected-channel
/// outline above. Hidden outside the Controls tab, while peeking, or once the
/// highlight set shrinks (extra pool slots just hide).
fn sync_hover_outlines(
    state: Res<super::PreviewState>,
    highlighted: Res<HighlightedChannels>,
    pfl: Res<PlayfieldLayout>,
    lanes: Res<Lanes>,
    mut pool: Query<(&HoverOutline, &mut Node, &mut Visibility, &mut BorderColor)>,
) {
    let show = state.tab == game_shell::CustomizeTab::Controls && !state.peeking;
    for (slot, mut node, mut vis, mut border) in &mut pool {
        let channel = show.then(|| highlighted.0.get(slot.0)).flatten().copied();
        let Some(col) = channel.and_then(|ch| lanes.col_of(ch)) else {
            *vis = Visibility::Hidden;
            continue;
        };
        node.left = Val::Px(pfl.col_left(col));
        node.top = Val::Px(pfl.lane_top());
        node.width = Val::Px(pfl.col_width(col));
        node.height = Val::Px(pfl.lane_height());
        *border = BorderColor::all(chrome::ACCENT);
        *vis = Visibility::Inherited;
    }
}
