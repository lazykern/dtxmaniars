//! Right settings panel: per-widget knobs for the selected widget. Rebuilt
//! whenever the selection changes; control changes write straight into
//! `WidgetLayouts` (single mutation path — undo/save cover it).

use bevy::prelude::*;
use dtx_layout::{Anchor9, WidgetKind, MAX_WIDGET_SCALE, MIN_WIDGET_SCALE};
use dtx_ui::widget::controls::{self, ControlBool, ControlValue, Slider, Stepper};

use super::drag::Selection;
use super::picking::EditorChrome;
use super::EditorOpen;
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

#[derive(Component)]
pub struct PanelRoot;

/// Which widget field a control edits.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum PanelField {
    OffsetX,
    OffsetY,
    Scale,
    Z,
    VisiblePlay,
    VisiblePractice,
}

/// 3×3 anchor grid cell.
#[derive(Component, Debug, Clone, Copy)]
pub struct AnchorCell(pub Anchor9);

/// Reset-this-widget button.
#[derive(Component)]
pub struct PanelResetWidget;

pub const PANEL_WIDTH: f32 = 240.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            rebuild_panel.run_if(
                resource_changed::<Selection>.or_else(resource_changed::<super::EditorOpen>),
            ),
            (
                apply_panel_controls,
                apply_anchor_cells,
                handle_reset,
                refresh_panel_values,
            )
                .run_if(super::editor_open),
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_panel);
}

fn despawn_panel(mut commands: Commands, q: Query<Entity, With<PanelRoot>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn rebuild_panel(
    mut commands: Commands,
    open: Res<EditorOpen>,
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<PanelRoot>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    let Some(kind) = selection.0 else { return };
    if kind == WidgetKind::Playfield {
        return; // plan 3 adds the lane panel here
    }
    let t = theme.0;
    let inst = layouts.get(kind).clone();
    let root = commands
        .spawn((
            PanelRoot,
            EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.92)),
            GlobalZIndex(2000),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        p.spawn((
            Text::new(format!("Settings ({})", kind.display_name())),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(t.text_primary),
        ));

        // Anchor 3×3 grid.
        p.spawn((
            Text::new("anchor"),
            dtx_ui::theme::Theme::font(11.0),
            TextColor(t.text_secondary),
        ));
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|grid| {
            for row in 0..3 {
                grid.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|r| {
                    for col in 0..3 {
                        let a = Anchor9::ALL[row * 3 + col];
                        let selected = inst.anchor == a;
                        r.spawn((
                            AnchorCell(a),
                            Button,
                            Node {
                                width: Val::Px(20.0),
                                height: Val::Px(20.0),
                                ..default()
                            },
                            BackgroundColor(if selected {
                                t.accent
                            } else {
                                Color::srgb(0.14, 0.14, 0.18)
                            }),
                        ));
                    }
                });
            }
        });

        // Offset / scale / z rows.
        row(p, &t, "offset x", |p| {
            let e = controls::spawn_stepper(
                p,
                &t,
                Stepper { step: 1.0, min: -2000.0, max: 2000.0, decimals: 0 },
                inst.offset.0,
            );
            p.commands_mut().entity(e).insert(PanelField::OffsetX);
        });
        row(p, &t, "offset y", |p| {
            let e = controls::spawn_stepper(
                p,
                &t,
                Stepper { step: 1.0, min: -2000.0, max: 2000.0, decimals: 0 },
                inst.offset.1,
            );
            p.commands_mut().entity(e).insert(PanelField::OffsetY);
        });
        row(p, &t, "scale", |p| {
            let e = controls::spawn_slider(
                p,
                &t,
                Slider { min: MIN_WIDGET_SCALE, max: MAX_WIDGET_SCALE },
                inst.scale,
            );
            p.commands_mut().entity(e).insert(PanelField::Scale);
        });
        row(p, &t, "z", |p| {
            let e = controls::spawn_stepper(
                p,
                &t,
                Stepper { step: 1.0, min: -100.0, max: 100.0, decimals: 0 },
                inst.z as f32,
            );
            p.commands_mut().entity(e).insert(PanelField::Z);
        });
        row(p, &t, "show in play", |p| {
            let e = controls::spawn_toggle(p, &t, inst.visible_play);
            p.commands_mut().entity(e).insert(PanelField::VisiblePlay);
        });
        row(p, &t, "show in practice", |p| {
            let e = controls::spawn_toggle(p, &t, inst.visible_practice);
            p.commands_mut().entity(e).insert(PanelField::VisiblePractice);
        });

        p.spawn((
            PanelResetWidget,
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
            children![(
                Text::new("Reset Widget"),
                dtx_ui::theme::Theme::font(12.0),
                TextColor(t.text_primary),
            )],
        ));
    });
}

fn row(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    label: &str,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            Text::new(label.to_string()),
            dtx_ui::theme::Theme::font(11.0),
            TextColor(t.text_secondary),
        ));
        content(r);
    });
}

/// One undo snapshot per discrete panel change; slider drags snapshot on the
/// first change of a mouse-hold (tracked via Local).
fn apply_panel_controls(
    selection: Res<Selection>,
    buttons: Res<ButtonInput<MouseButton>>,
    values: Query<
        (&PanelField, Option<&ControlValue>, Option<&ControlBool>),
        Or<(Changed<ControlValue>, Changed<ControlBool>)>,
    >,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut snapped_this_hold: Local<bool>,
) {
    let Some(kind) = selection.0 else { return };
    if values.is_empty() {
        if !buttons.pressed(MouseButton::Left) {
            *snapped_this_hold = false;
        }
        return;
    }
    // Panel rebuilds mark every fresh control Changed with values equal to the
    // instance — pushing undo then would flood the stack with no-op snapshots.
    // Only snapshot (and write) when an incoming value actually differs.
    let dirty = {
        let inst = layouts.get(kind).clone();
        values
            .iter()
            .any(|(field, val, boolean)| match (field, val, boolean) {
                (PanelField::OffsetX, Some(v), _) => (v.0 - inst.offset.0).abs() > 0.0005,
                (PanelField::OffsetY, Some(v), _) => (v.0 - inst.offset.1).abs() > 0.0005,
                (PanelField::Scale, Some(v), _) => (v.0 - inst.scale).abs() > 0.0005,
                (PanelField::Z, Some(v), _) => v.0 as i32 != inst.z,
                (PanelField::VisiblePlay, _, Some(b)) => b.0 != inst.visible_play,
                (PanelField::VisiblePractice, _, Some(b)) => b.0 != inst.visible_practice,
                _ => false,
            })
    };
    if !dirty {
        if !buttons.pressed(MouseButton::Left) {
            *snapped_this_hold = false;
        }
        return;
    }
    if !*snapped_this_hold {
        undo.push(&layouts, &lanes);
        *snapped_this_hold = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        *snapped_this_hold = false;
    }

    // Geometry-dependent conversion context.
    let Ok(window) = windows.single() else { return };
    let wsize = Vec2::new(window.width(), window.height());

    for (field, val, boolean) in &values {
        let Some(inst) = layouts.0.get_mut(&kind) else { continue };
        // Position/scale edits require Anchored; visibility/z don't.
        let needs_anchor = matches!(
            field,
            PanelField::OffsetX | PanelField::OffsetY | PanelField::Scale
        );
        if needs_anchor {
            if let Some(g) = geoms.0.get(&kind).copied() {
                let sc = wsize / 2.0;
                let visual_min = crate::widget_layout::transform_point(
                    g.unscaled.min,
                    sc,
                    g.applied_translation,
                    g.applied_scale,
                );
                let parent = crate::widget_layout::parent_rect_px(inst.space, wsize, &pfl);
                super::drag::ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
            }
        }
        match (field, val, boolean) {
            (PanelField::OffsetX, Some(v), _) => inst.offset.0 = v.0,
            (PanelField::OffsetY, Some(v), _) => inst.offset.1 = v.0,
            (PanelField::Scale, Some(v), _) => inst.scale = super::drag::clamp_scale(v.0),
            (PanelField::Z, Some(v), _) => inst.z = v.0 as i32,
            (PanelField::VisiblePlay, _, Some(b)) => inst.visible_play = b.0,
            (PanelField::VisiblePractice, _, Some(b)) => inst.visible_practice = b.0,
            _ => {}
        }
    }
}

/// Anchor grid clicks: rewrite anchor+origin, keep the widget's visual
/// position (no-jump — recompute offset via offset_for_top_left).
fn apply_anchor_cells(
    selection: Res<Selection>,
    cells: Query<(&AnchorCell, &Interaction), Changed<Interaction>>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
    geoms: Res<crate::widget_layout::WidgetGeoms>,
    pfl: Res<crate::layout::PlayfieldLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cell_bg: Query<(&AnchorCell, &mut BackgroundColor)>,
    theme: Res<dtx_ui::ThemeResource>,
) {
    let Some(kind) = selection.0 else { return };
    let mut clicked: Option<Anchor9> = None;
    for (cell, interaction) in &cells {
        if *interaction == Interaction::Pressed {
            clicked = Some(cell.0);
        }
    }
    let Some(new_anchor) = clicked else { return };
    let Ok(window) = windows.single() else { return };
    let Some(g) = geoms.0.get(&kind).copied() else { return };
    undo.push(&layouts, &lanes);
    let Some(inst) = layouts.0.get_mut(&kind) else { return };
    let wsize = Vec2::new(window.width(), window.height());
    let sc = wsize / 2.0;
    let visual_min = crate::widget_layout::transform_point(
        g.unscaled.min,
        sc,
        g.applied_translation,
        g.applied_scale,
    );
    let parent = crate::widget_layout::parent_rect_px(inst.space, wsize, &pfl);
    super::drag::ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
    inst.anchor = new_anchor;
    inst.origin = new_anchor;
    let off_px = dtx_layout::offset_for_top_left(
        inst.anchor,
        inst.origin,
        (g.unscaled.width(), g.unscaled.height()),
        inst.scale,
        (visual_min.x, visual_min.y),
        parent,
    );
    inst.offset = (off_px.0 / pfl.scale, off_px.1 / pfl.scale);
    for (cell, mut bg) in &mut cell_bg {
        bg.0 = if cell.0 == new_anchor {
            theme.0.accent
        } else {
            Color::srgb(0.14, 0.14, 0.18)
        };
    }
}

fn handle_reset(
    resets: Query<&Interaction, (With<PanelResetWidget>, Changed<Interaction>)>,
    selection: Res<Selection>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
) {
    for interaction in &resets {
        if *interaction == Interaction::Pressed {
            if let Some(kind) = selection.0 {
                undo.push(&layouts, &lanes);
                super::save::reset_widget(&mut layouts, kind);
            }
        }
    }
}

/// External mutations (undo/redo, canvas drag, reset) → push values back into
/// the visible controls so the panel never shows stale numbers. Guarded to
/// avoid write-back loops: only touch a control whose value actually differs.
fn refresh_panel_values(
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    mut values: Query<(&PanelField, Option<&mut ControlValue>, Option<&mut ControlBool>)>,
) {
    if !layouts.is_changed() {
        return;
    }
    let Some(kind) = selection.0 else { return };
    let Some(inst) = layouts.0.get(&kind) else { return };
    for (field, val, boolean) in &mut values {
        let want = match field {
            PanelField::OffsetX => Some(inst.offset.0),
            PanelField::OffsetY => Some(inst.offset.1),
            PanelField::Scale => Some(inst.scale),
            PanelField::Z => Some(inst.z as f32),
            _ => None,
        };
        if let (Some(w), Some(mut v)) = (want, val) {
            if (v.0 - w).abs() > 0.0005 {
                v.0 = w;
            }
        }
        let want_b = match field {
            PanelField::VisiblePlay => Some(inst.visible_play),
            PanelField::VisiblePractice => Some(inst.visible_practice),
            _ => None,
        };
        if let (Some(w), Some(mut b)) = (want_b, boolean) {
            if b.0 != w {
                b.0 = w;
            }
        }
    }
}
