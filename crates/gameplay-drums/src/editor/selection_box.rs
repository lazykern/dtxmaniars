//! Selection overlay: border + name tag + anchor line + corner scale handles
//! around the selected widget's AABB, and a lighter hover outline. Overlay
//! nodes are `HudRoot` children so the scene-space AABB coords render 1:1
//! under the stage transform; they keep `GlobalZIndex` (stacking-only, the
//! transform still inherits) to stay above the preview scrim and the sidebar.

use bevy::prelude::*;
use bevy::ui::UiTransform;
use dtx_layout::WidgetKind;

use super::drag::Selection;
use super::picking::{Hovered, WidgetAabbs};
use crate::widget_layout::{WidgetLayouts, widget_visible};

/// Every editor-overlay entity (cleanup marker).
#[derive(Component)]
pub struct EditorOverlay;

/// The selection border box (one, reused; hidden when no selection).
#[derive(Component)]
pub struct SelectionBoxRoot;

/// One of the four corner scale handles; index 0..4 = TL, TR, BL, BR.
#[derive(Component, Clone, Copy)]
pub struct ScaleHandle(pub usize);

#[derive(Component)]
pub struct SelectionNameTag;

#[derive(Component)]
pub struct AnchorLine;

#[derive(Component)]
pub struct AnchorDot;

#[derive(Component)]
pub struct OriginDot;

/// Root of the hover outline (separate from selection so both can show).
#[derive(Component)]
pub struct HoverOutlineRoot;

const ACCENT: Color = Color::srgb(1.0, 0.75, 0.1);
const HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.5);
pub const HANDLE_SIZE: f32 = 10.0;
const DOT_SIZE: f32 = 6.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_overlay_on_open,
            sync_selection_border,
            sync_anchor_viz,
            sync_hover_outline,
        )
            .chain()
            .after(super::EditorGestureSet)
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_overlay);
}

fn despawn_overlay(mut commands: Commands, roots: Query<Entity, With<EditorOverlay>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Spawn the overlay entities when the editor opens; despawn when it closes.
fn spawn_overlay_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    roots: Query<Entity, With<crate::hud::HudRoot>>,
    existing: Query<Entity, With<EditorOverlay>>,
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
    let Ok(root) = roots.single() else {
        return;
    };
    let hover = commands
        .spawn((
            EditorOverlay,
            HoverOutlineRoot,
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            BorderColor::all(HOVER),
            Visibility::Hidden,
            GlobalZIndex(2100),
            Pickable::IGNORE,
        ))
        .id();
    let selection = commands
        .spawn((
            EditorOverlay,
            SelectionBoxRoot,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(ACCENT),
            Visibility::Hidden,
            GlobalZIndex(2200),
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            for i in 0..4usize {
                let (l, t) = match i {
                    0 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Px(-HANDLE_SIZE / 2.0)),
                    1 => (Val::Auto, Val::Px(-HANDLE_SIZE / 2.0)),
                    2 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Auto),
                    _ => (Val::Auto, Val::Auto),
                };
                let (r, b) = match i {
                    1 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Auto),
                    3 => (Val::Px(-HANDLE_SIZE / 2.0), Val::Px(-HANDLE_SIZE / 2.0)),
                    2 => (Val::Auto, Val::Px(-HANDLE_SIZE / 2.0)),
                    _ => (Val::Auto, Val::Auto),
                };
                p.spawn((
                    ScaleHandle(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: l,
                        top: t,
                        right: r,
                        bottom: b,
                        width: Val::Px(HANDLE_SIZE),
                        height: Val::Px(HANDLE_SIZE),
                        ..default()
                    },
                    BackgroundColor(ACCENT),
                    Pickable::IGNORE,
                ));
            }
            p.spawn((
                SelectionNameTag,
                Text::new(""),
                dtx_ui::theme::Theme::font(11.0),
                TextColor(ACCENT),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-18.0),
                    left: Val::Px(0.0),
                    ..default()
                },
                Pickable::IGNORE,
            ));
        })
        .id();
    // Anchor viz nodes live outside the box (positions are unrelated rects).
    let line = commands
        .spawn((
            EditorOverlay,
            AnchorLine,
            Node {
                position_type: PositionType::Absolute,
                height: Val::Px(2.0),
                ..default()
            },
            UiTransform::default(),
            BackgroundColor(Color::srgba(1.0, 0.3, 0.3, 0.9)),
            Visibility::Hidden,
            GlobalZIndex(2150),
            Pickable::IGNORE,
        ))
        .id();
    let a_dot = commands
        .spawn((
            EditorOverlay,
            AnchorDot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(DOT_SIZE),
                height: Val::Px(DOT_SIZE),
                border_radius: BorderRadius::all(Val::Px(DOT_SIZE / 2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 0.3, 0.3)),
            Visibility::Hidden,
            GlobalZIndex(2150),
            Pickable::IGNORE,
        ))
        .id();
    let o_dot = commands
        .spawn((
            EditorOverlay,
            OriginDot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(DOT_SIZE),
                height: Val::Px(DOT_SIZE),
                border_radius: BorderRadius::all(Val::Px(DOT_SIZE / 2.0)),
                ..default()
            },
            BackgroundColor(ACCENT),
            Visibility::Hidden,
            GlobalZIndex(2150),
            Pickable::IGNORE,
        ))
        .id();
    commands
        .entity(root)
        .add_children(&[hover, selection, line, a_dot, o_dot]);
}

/// The selected widget's kind + AABB, or None (nothing selected / no AABB).
fn selected_aabb(
    open: &super::EditorOpen,
    selection: &Selection,
    aabbs: &WidgetAabbs,
) -> Option<(WidgetKind, Rect)> {
    if !open.0 {
        return None;
    }
    let kind = selection.0?;
    Some((kind, *aabbs.0.get(&kind)?))
}

/// Parent-space rect for a widget's anchor (logical px): screen or playfield.
fn parent_rect(
    space: dtx_layout::AnchorSpace,
    window: &Window,
    pfl: &crate::layout::PlayfieldLayout,
) -> Rect {
    match space {
        dtx_layout::AnchorSpace::Screen => Rect::new(0.0, 0.0, window.width(), window.height()),
        dtx_layout::AnchorSpace::Playfield => Rect::new(
            pfl.strip_left(),
            pfl.lane_top(),
            pfl.strip_left() + pfl.strip_width(),
            pfl.lane_top() + pfl.lane_height(),
        ),
    }
}

fn sync_selection_border(
    open: Res<super::EditorOpen>,
    selection: Res<Selection>,
    aabbs: Res<WidgetAabbs>,
    layouts: Res<WidgetLayouts>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    mut box_q: Query<(&mut Node, &mut Visibility, &mut BorderColor), With<SelectionBoxRoot>>,
    mut tag_q: Query<&mut Text, With<SelectionNameTag>>,
    mut handles: Query<&mut Visibility, (With<ScaleHandle>, Without<SelectionBoxRoot>)>,
) {
    let Ok((mut node, mut vis, mut border)) = box_q.single_mut() else {
        return;
    };
    let Some((kind, aabb)) = selected_aabb(&open, &selection, &aabbs) else {
        *vis = Visibility::Hidden;
        return;
    };
    let inst = layouts.get(kind);
    let is_practice = practice.is_some();

    node.left = Val::Px(aabb.min.x);
    node.top = Val::Px(aabb.min.y);
    node.width = Val::Px(aabb.width());
    node.height = Val::Px(aabb.height());
    *vis = Visibility::Visible;
    // Hidden-in-mode widget: dim the border (selected from the sidebar list).
    let alpha = if widget_visible(inst, is_practice) {
        1.0
    } else {
        0.35
    };
    *border = BorderColor::all(ACCENT.with_alpha(alpha));

    if let Ok(mut text) = tag_q.single_mut() {
        text.0 = kind.display_name().to_string();
    }
    let show_handles = kind != WidgetKind::Playfield;
    for mut hv in handles.iter_mut() {
        *hv = if show_handles {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn sync_anchor_viz(
    open: Res<super::EditorOpen>,
    selection: Res<Selection>,
    aabbs: Res<WidgetAabbs>,
    layouts: Res<WidgetLayouts>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    windows: Query<&Window>,
    mut viz: ParamSet<(
        Query<(&mut Node, &mut Visibility, &mut UiTransform), With<AnchorLine>>,
        Query<(&mut Node, &mut Visibility), With<AnchorDot>>,
        Query<(&mut Node, &mut Visibility), With<OriginDot>>,
    )>,
) {
    let sel = selected_aabb(&open, &selection, &aabbs);
    let Some((kind, aabb)) = sel else {
        if let Ok((_, mut v, _)) = viz.p0().single_mut() {
            *v = Visibility::Hidden;
        }
        if let Ok((_, mut v)) = viz.p1().single_mut() {
            *v = Visibility::Hidden;
        }
        if let Ok((_, mut v)) = viz.p2().single_mut() {
            *v = Visibility::Hidden;
        }
        return;
    };
    let Ok(window) = windows.single() else { return };
    let inst = layouts.get(kind);
    let parent = parent_rect(inst.space, window, &pfl);
    let (af_x, af_y) = inst.anchor.frac();
    let anchor_pt = Vec2::new(
        parent.min.x + af_x * parent.width(),
        parent.min.y + af_y * parent.height(),
    );
    let (of_x, of_y) = inst.origin.frac();
    let origin_pt = Vec2::new(
        aabb.min.x + of_x * aabb.width(),
        aabb.min.y + of_y * aabb.height(),
    );

    if let Ok((mut ln, mut lv, mut lt)) = viz.p0().single_mut() {
        let seg = anchor_pt - origin_pt;
        let len = seg.length();
        let mid = (anchor_pt + origin_pt) / 2.0;
        ln.left = Val::Px(mid.x - len / 2.0);
        ln.top = Val::Px(mid.y - 1.0);
        ln.width = Val::Px(len);
        lt.rotation = Rot2::radians(seg.y.atan2(seg.x));
        *lv = if len > 4.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok((mut dn, mut dv)) = viz.p1().single_mut() {
        dn.left = Val::Px(anchor_pt.x - DOT_SIZE / 2.0);
        dn.top = Val::Px(anchor_pt.y - DOT_SIZE / 2.0);
        *dv = Visibility::Visible;
    }
    if let Ok((mut dn, mut dv)) = viz.p2().single_mut() {
        dn.left = Val::Px(origin_pt.x - DOT_SIZE / 2.0);
        dn.top = Val::Px(origin_pt.y - DOT_SIZE / 2.0);
        *dv = Visibility::Visible;
    }
}

fn sync_hover_outline(
    open: Res<super::EditorOpen>,
    hovered: Res<Hovered>,
    selection: Res<Selection>,
    aabbs: Res<WidgetAabbs>,
    mut q: Query<(&mut Node, &mut Visibility), With<HoverOutlineRoot>>,
) {
    let Ok((mut node, mut vis)) = q.single_mut() else {
        return;
    };
    let show = open.0 && hovered.0.is_some() && hovered.0 != selection.0;
    let Some(aabb) = hovered
        .0
        .and_then(|k| aabbs.0.get(&k).copied())
        .filter(|_| show)
    else {
        *vis = Visibility::Hidden;
        return;
    };
    node.left = Val::Px(aabb.min.x);
    node.top = Val::Px(aabb.min.y);
    node.width = Val::Px(aabb.width());
    node.height = Val::Px(aabb.height());
    node.border = UiRect::all(Val::Px(1.0));
    *vis = Visibility::Visible;
}
