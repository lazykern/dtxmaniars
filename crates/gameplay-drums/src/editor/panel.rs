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

/// "auto" cell below the anchor grid: toggles `anchor_auto` (closest-anchor
/// snap while dragging).
#[derive(Component)]
pub struct AnchorAutoCell;

/// Reset-this-widget button.
#[derive(Component)]
pub struct PanelResetWidget;

/// Lane panel controls (Playfield selected).
#[derive(Component, Debug, Clone, Copy)]
pub struct LaneReorderBtn {
    pub index: usize,
    pub dir: i32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct LaneMergeBtn(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct ChipSplitBtn(pub dtx_core::EChannel);

#[derive(Component, Debug, Clone, Copy)]
pub struct LaneWidthSlider(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct PresetCycleBtn(pub i32);

#[derive(Component)]
pub struct PresetLabel;

/// Tags a settings row control with its index into the active tab's item list.
#[derive(Component, Clone, Copy)]
pub struct SettingRow(pub usize);

/// Tags the ◂ / ▸ adjust buttons on a settings row (dir = -1 / +1).
#[derive(Component, Clone, Copy)]
pub struct SettingAdjust {
    pub index: usize,
    pub dir: i32,
}

/// Tags the value text of a settings row for live refresh.
#[derive(Component, Clone, Copy)]
pub struct SettingValueText(pub usize);

fn preset_name(p: dtx_layout::LanePreset) -> &'static str {
    match p {
        dtx_layout::LanePreset::Classic => "classic",
        dtx_layout::LanePreset::NxTypeB => "nx type-b",
        dtx_layout::LanePreset::NxTypeD => "nx type-d",
        dtx_layout::LanePreset::Custom => "custom",
    }
}

pub const PANEL_WIDTH: f32 = 240.0;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            // Only when a rebuild trigger actually changed — avoids allocating
            // a fresh signature string every idle frame. The Local guard inside
            // still debounces width-only Lanes changes (no rebuild mid-drag).
            rebuild_panel.run_if(
                resource_changed::<Selection>
                    .or_else(resource_changed::<EditorOpen>)
                    .or_else(resource_changed::<Lanes>)
                    .or_else(resource_changed::<super::tabs::ActiveTab>),
            ),
            (
                apply_panel_controls,
                apply_anchor_cells,
                handle_anchor_auto_cell,
                handle_reset,
                refresh_panel_values,
                handle_lane_buttons,
                apply_lane_width_sliders,
                refresh_lane_panel_values,
                handle_settings_adjust,
                refresh_settings_values,
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
    lanes: Res<Lanes>,
    active: Res<super::tabs::ActiveTab>,
    draft: Res<super::tabs::ConfigDraft>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<PanelRoot>>,
    mut last_sig: Local<Option<(Option<WidgetKind>, bool, String, game_shell::CustomizeTab)>>,
) {
    let sig = (
        selection.0,
        open.0,
        dtx_layout::structure_signature(&lanes.0),
        active.0,
    );
    if last_sig.as_ref() == Some(&sig) {
        return;
    }
    *last_sig = Some(sig);
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    // Settings tabs render without a widget selection; the Lanes tab renders the
    // lane block directly. The Widgets tab still needs a selected widget.
    let is_lanes = active.0 == game_shell::CustomizeTab::Lanes;
    if !active.0.is_settings() && !is_lanes && selection.0.is_none() {
        return;
    }
    let t = theme.0;
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

    if active.0.is_settings() {
        spawn_settings_block(&mut commands, root, &t, active.0, &draft);
        return;
    }
    if is_lanes {
        commands.entity(root).with_children(|p| {
            spawn_lane_block(p, &t, &lanes);
        });
        return;
    }
    let Some(kind) = selection.0 else { return };

    commands.entity(root).with_children(|p| {
        if kind == WidgetKind::Playfield {
            spawn_lane_block(p, &t, &lanes);
            return;
        }
        let inst = layouts.get(kind).clone();
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
            grid.spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    AnchorAutoCell,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(if inst.anchor_auto {
                        t.accent
                    } else {
                        Color::srgb(0.14, 0.14, 0.18)
                    }),
                    children![(
                        Text::new("auto"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });
        });

        // Offset / scale / z rows.
        row(p, &t, "offset x", |p| {
            let e = controls::spawn_stepper(
                p,
                &t,
                Stepper {
                    step: 1.0,
                    min: -2000.0,
                    max: 2000.0,
                    decimals: 0,
                },
                inst.offset.0,
            );
            p.commands_mut().entity(e).insert(PanelField::OffsetX);
        });
        row(p, &t, "offset y", |p| {
            let e = controls::spawn_stepper(
                p,
                &t,
                Stepper {
                    step: 1.0,
                    min: -2000.0,
                    max: 2000.0,
                    decimals: 0,
                },
                inst.offset.1,
            );
            p.commands_mut().entity(e).insert(PanelField::OffsetY);
        });
        row(p, &t, "scale", |p| {
            let e = controls::spawn_slider(
                p,
                &t,
                Slider {
                    min: MIN_WIDGET_SCALE,
                    max: MAX_WIDGET_SCALE,
                },
                inst.scale,
            );
            p.commands_mut().entity(e).insert(PanelField::Scale);
        });
        row(p, &t, "z", |p| {
            let e = controls::spawn_stepper(
                p,
                &t,
                Stepper {
                    step: 1.0,
                    min: -100.0,
                    max: 100.0,
                    decimals: 0,
                },
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
            p.commands_mut()
                .entity(e)
                .insert(PanelField::VisiblePractice);
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

fn spawn_lane_block(p: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, lanes: &Lanes) {
    p.spawn((
        Text::new("Lanes"),
        dtx_ui::theme::Theme::font(13.0),
        TextColor(t.text_primary),
    ));

    // Preset row: < name >
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn((
            PresetCycleBtn(-1),
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
            children![(
                Text::new("<"),
                dtx_ui::theme::Theme::font(12.0),
                TextColor(t.text_primary)
            )],
        ));
        r.spawn((
            PresetLabel,
            Text::new(preset_name(lanes.0.preset).to_string()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
            Node {
                min_width: Val::Px(70.0),
                ..default()
            },
        ));
        r.spawn((
            PresetCycleBtn(1),
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
            children![(
                Text::new(">"),
                dtx_ui::theme::Theme::font(12.0),
                TextColor(t.text_primary)
            )],
        ));
    });

    // One row per lane: [^][v] ID (chips…) width-slider [x]
    let last = lanes.0.lanes.len().saturating_sub(1);
    for (i, lane) in lanes.0.lanes.iter().enumerate() {
        let chips = dtx_layout::lane_chips(&lanes.0, i);
        let can_merge = lanes.0.lanes.len() > 1;
        let width = lane.width;
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::vertical(Val::Px(2.0)),
            ..default()
        })
        .with_children(|lane_col| {
            lane_col
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|r| {
                    for (dir, sym, enabled) in [(-1, "^", i > 0), (1, "v", i < last)] {
                        if enabled {
                            r.spawn((
                                LaneReorderBtn { index: i, dir },
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                                children![(
                                    Text::new(sym),
                                    dtx_ui::theme::Theme::font(11.0),
                                    TextColor(t.text_primary)
                                )],
                            ));
                        } else {
                            r.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                                    ..default()
                                },
                                children![(
                                    Text::new(sym),
                                    dtx_ui::theme::Theme::font(11.0),
                                    TextColor(t.text_secondary)
                                )],
                            ));
                        }
                    }
                    r.spawn((
                        Text::new(lane.id.clone()),
                        dtx_ui::theme::Theme::font(12.0),
                        TextColor(t.text_primary),
                        Node {
                            min_width: Val::Px(34.0),
                            ..default()
                        },
                    ));
                    // Chips: primary shown flat; secondaries are split buttons.
                    for ch in &chips {
                        let name = dtx_layout::channel_short_name(*ch).unwrap_or("?");
                        if *ch == lane.primary {
                            r.spawn((
                                Text::new(name),
                                dtx_ui::theme::Theme::font(10.0),
                                TextColor(t.text_secondary),
                            ));
                        } else {
                            r.spawn((
                                ChipSplitBtn(*ch),
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.18, 0.22, 0.28)),
                                children![(
                                    Text::new(format!("{name} x")),
                                    dtx_ui::theme::Theme::font(10.0),
                                    TextColor(t.text_primary),
                                )],
                            ));
                        }
                    }
                    if can_merge {
                        r.spawn((
                            LaneMergeBtn(i),
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.3, 0.14, 0.14)),
                            children![(
                                Text::new("x"),
                                dtx_ui::theme::Theme::font(11.0),
                                TextColor(t.text_primary)
                            )],
                        ));
                    }
                });
            lane_col
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    margin: UiRect::left(Val::Px(20.0)),
                    ..default()
                })
                .with_children(|r| {
                    let e = controls::spawn_slider(
                        r,
                        t,
                        Slider {
                            min: dtx_layout::MIN_LANE_WIDTH,
                            max: dtx_layout::MAX_LANE_WIDTH,
                        },
                        width,
                    );
                    r.commands_mut().entity(e).insert(LaneWidthSlider(i));
                });
        });
    }
}

fn spawn_settings_block(
    commands: &mut Commands,
    root: Entity,
    t: &dtx_ui::theme::Theme,
    tab: game_shell::CustomizeTab,
    draft: &super::tabs::ConfigDraft,
) {
    let items = crate::editor::settings_data::settings_items(tab);
    commands.entity(root).with_children(|p| {
        p.spawn((
            Text::new(tab.label()),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(t.text_primary),
        ));
        for (i, item) in items.iter().enumerate() {
            p.spawn((
                SettingRow(i),
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .with_children(|r| {
                r.spawn((
                    Text::new(item.label),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_secondary),
                ));
                r.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn((
                        SettingAdjust { index: i, dir: -1 },
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                        children![(
                            Text::new("<"),
                            dtx_ui::theme::Theme::font(12.0),
                            TextColor(t.text_primary)
                        )],
                    ));
                    c.spawn((
                        SettingValueText(i),
                        Text::new((item.value)(&draft.0)),
                        dtx_ui::theme::Theme::font(12.0),
                        TextColor(t.text_primary),
                        Node {
                            min_width: Val::Px(60.0),
                            ..default()
                        },
                    ));
                    c.spawn((
                        SettingAdjust { index: i, dir: 1 },
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                        children![(
                            Text::new(">"),
                            dtx_ui::theme::Theme::font(12.0),
                            TextColor(t.text_primary)
                        )],
                    ));
                });
            });
        }
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
    rect: Res<crate::stage_rect::StageRect>,
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
    for (field, val, boolean) in &values {
        let Some(inst) = layouts.0.get_mut(&kind) else {
            continue;
        };
        // Only Scale forces Anchored placement (Natural renders scale as 1).
        // Offset works in BOTH modes — it's the ref-px delta — so an offset
        // stepper must NOT convert; converting first rewrites `inst.offset`
        // into the anchored frame, then the stale (Natural-frame) control
        // value clobbers it and the widget teleports.
        let needs_anchor = matches!(field, PanelField::Scale);
        if needs_anchor {
            if let Some(g) = geoms.0.get(&kind).copied() {
                let sc = rect.center();
                let visual_min = crate::widget_layout::transform_point(
                    g.unscaled.min,
                    sc,
                    g.applied_translation,
                    g.applied_scale,
                );
                let parent = crate::widget_layout::parent_rect_px(inst.space, *rect, &pfl);
                super::drag::ensure_anchored(
                    inst,
                    visual_min,
                    g.unscaled.size(),
                    parent,
                    pfl.scale,
                );
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
    rect: Res<crate::stage_rect::StageRect>,
    mut cell_bg: Query<(&AnchorCell, &mut BackgroundColor)>,
    mut auto_bg: Query<&mut BackgroundColor, (With<AnchorAutoCell>, Without<AnchorCell>)>,
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
    let Some(g) = geoms.0.get(&kind).copied() else {
        return;
    };
    undo.push(&layouts, &lanes);
    let Some(inst) = layouts.0.get_mut(&kind) else {
        return;
    };
    let sc = rect.center();
    let visual_min = crate::widget_layout::transform_point(
        g.unscaled.min,
        sc,
        g.applied_translation,
        g.applied_scale,
    );
    let parent = crate::widget_layout::parent_rect_px(inst.space, *rect, &pfl);
    super::drag::ensure_anchored(inst, visual_min, g.unscaled.size(), parent, pfl.scale);
    inst.anchor = new_anchor;
    inst.origin = new_anchor;
    // A manual anchor pick pins the anchor (osu parity): auto-snap turns off.
    inst.anchor_auto = false;
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
    for mut bg in &mut auto_bg {
        bg.0 = Color::srgb(0.14, 0.14, 0.18);
    }
}

/// "auto" cell click: toggle `anchor_auto` for the selected widget.
fn handle_anchor_auto_cell(
    cells: Query<&Interaction, (With<AnchorAutoCell>, Changed<Interaction>)>,
    selection: Res<Selection>,
    mut layouts: ResMut<WidgetLayouts>,
    lanes: Res<Lanes>,
    mut undo: ResMut<super::undo::UndoStack>,
    mut cell_bg: Query<&mut BackgroundColor, With<AnchorAutoCell>>,
    theme: Res<dtx_ui::ThemeResource>,
) {
    for interaction in &cells {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(kind) = selection.0 else { continue };
        undo.push(&layouts, &lanes);
        let Some(inst) = layouts.0.get_mut(&kind) else {
            continue;
        };
        inst.anchor_auto = !inst.anchor_auto;
        for mut bg in &mut cell_bg {
            bg.0 = if inst.anchor_auto {
                theme.0.accent
            } else {
                Color::srgb(0.14, 0.14, 0.18)
            };
        }
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
    mut values: Query<(
        &PanelField,
        Option<&mut ControlValue>,
        Option<&mut ControlBool>,
    )>,
) {
    if !layouts.is_changed() {
        return;
    }
    let Some(kind) = selection.0 else { return };
    let Some(inst) = layouts.0.get(&kind) else {
        return;
    };
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

/// Preset cycle order for the < > buttons (named presets only; any manual
/// edit lands on Custom via the transforms).
const PRESET_ORDER: [dtx_layout::LanePreset; 3] = [
    dtx_layout::LanePreset::Classic,
    dtx_layout::LanePreset::NxTypeB,
    dtx_layout::LanePreset::NxTypeD,
];

fn handle_lane_buttons(
    reorders: Query<(&LaneReorderBtn, &Interaction), Changed<Interaction>>,
    merges: Query<(&LaneMergeBtn, &Interaction), Changed<Interaction>>,
    splits: Query<(&ChipSplitBtn, &Interaction), Changed<Interaction>>,
    presets: Query<(&PresetCycleBtn, &Interaction), Changed<Interaction>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<super::undo::UndoStack>,
) {
    let mut mutate: Option<Box<dyn FnOnce(&mut dtx_layout::LaneArrangement) -> bool>> = None;
    for (btn, i) in &reorders {
        if *i == Interaction::Pressed {
            let (index, dir) = (btn.index, btn.dir);
            mutate = Some(Box::new(move |arr| {
                dtx_layout::reorder_lane(arr, index, dir)
            }));
        }
    }
    for (btn, i) in &merges {
        if *i == Interaction::Pressed {
            let index = btn.0;
            mutate = Some(Box::new(move |arr| dtx_layout::merge_lane(arr, index)));
        }
    }
    for (btn, i) in &splits {
        if *i == Interaction::Pressed {
            let ch = btn.0;
            mutate = Some(Box::new(move |arr| dtx_layout::split_channel(arr, ch)));
        }
    }
    for (btn, i) in &presets {
        if *i == Interaction::Pressed {
            let dir = btn.0;
            mutate = Some(Box::new(move |arr| {
                let cur = PRESET_ORDER.iter().position(|p| *p == arr.preset);
                let next = match cur {
                    Some(idx) => {
                        let n = PRESET_ORDER.len() as i32;
                        PRESET_ORDER[((idx as i32 + dir).rem_euclid(n)) as usize]
                    }
                    // From Custom: either direction lands on Classic.
                    None => dtx_layout::LanePreset::Classic,
                };
                *arr = dtx_layout::arrangement_for(next);
                true
            }));
        }
    }
    if let Some(f) = mutate {
        // Snapshot BEFORE mutating; drop the snapshot if the op was a no-op.
        let before = super::undo::Snapshot {
            layouts: layouts.clone(),
            lanes: lanes.clone(),
        };
        if f(&mut lanes.0) {
            undo.push_snapshot(before);
        }
    }
}

/// Width slider → Lanes. One undo snapshot per mouse-hold.
fn apply_lane_width_sliders(
    buttons: Res<ButtonInput<MouseButton>>,
    sliders: Query<(&LaneWidthSlider, &ControlValue), Changed<ControlValue>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<super::undo::UndoStack>,
    mut snapped_this_hold: Local<bool>,
) {
    if !buttons.pressed(MouseButton::Left) {
        *snapped_this_hold = false;
    }
    let mut pending: Vec<(usize, f32)> = Vec::new();
    for (slider, value) in &sliders {
        let idx = slider.0;
        let differs = lanes
            .0
            .lanes
            .get(idx)
            .map(|l| (l.width - value.0).abs() > 0.01)
            .unwrap_or(false);
        if differs {
            pending.push((idx, value.0));
        }
    }
    if pending.is_empty() {
        return;
    }
    if !*snapped_this_hold {
        undo.push(&layouts, &lanes);
        *snapped_this_hold = true;
    }
    for (idx, w) in pending {
        dtx_layout::set_lane_width(&mut lanes.0, idx, w);
    }
}

/// External Lanes changes (undo, preset) → refresh slider values + preset
/// label. Equality-guarded to terminate the write-back loop.
fn refresh_lane_panel_values(
    lanes: Res<Lanes>,
    mut sliders: Query<(&LaneWidthSlider, &mut ControlValue)>,
    mut preset_label: Query<&mut Text, With<PresetLabel>>,
) {
    if !lanes.is_changed() {
        return;
    }
    for (slider, mut value) in &mut sliders {
        if let Some(lane) = lanes.0.lanes.get(slider.0) {
            if (value.0 - lane.width).abs() > 0.01 {
                value.0 = lane.width;
            }
        }
    }
    if let Ok(mut text) = preset_label.single_mut() {
        let want = preset_name(lanes.0.preset);
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

/// Settings-row ◂/▸ clicks: adjust the matching config draft item.
fn handle_settings_adjust(
    q: Query<(&Interaction, &SettingAdjust), Changed<Interaction>>,
    active: Res<super::tabs::ActiveTab>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
) {
    if !active.0.is_settings() {
        return;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for (interaction, adj) in &q {
        if *interaction == Interaction::Pressed {
            if let Some(item) = items.get(adj.index) {
                (item.adjust)(&mut draft.0, adj.dir);
            }
        }
    }
}

/// Draft changes → refresh the visible settings values.
fn refresh_settings_values(
    active: Res<super::tabs::ActiveTab>,
    draft: Res<super::tabs::ConfigDraft>,
    mut q: Query<(&SettingValueText, &mut Text)>,
) {
    if !draft.is_changed() || !active.0.is_settings() {
        return;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for (tag, mut text) in &mut q {
        if let Some(item) = items.get(tag.0) {
            let want = (item.value)(&draft.0);
            if text.0 != want {
                text.0 = want;
            }
        }
    }
}
