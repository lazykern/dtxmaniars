//! Right settings panel: per-widget knobs for the selected widget. Rebuilt
//! whenever the selection changes; control changes write straight into
//! `WidgetLayouts` (single mutation path — undo/save cover it).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use dtx_layout::{Anchor9, WidgetKind, MAX_WIDGET_SCALE, MIN_WIDGET_SCALE};
use dtx_ui::widget::controls::{self, ControlBool, ControlValue, Slider, Stepper};

use super::drag::Selection;
use super::picking::EditorChrome;
use super::EditorOpen;
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

/// Left content panel (docked right of the rail): renders the active tab's
/// content (settings rows / lane list / widget list / bindings). Rebuilds on
/// tab/content change only — NOT on selection — so picking a widget no longer
/// respawns the whole left list.
#[derive(Component)]
pub struct LeftContentRoot;

/// Right inspector panel (docked to the window's right edge): renders the
/// selected widget's knobs. Rebuilds on selection change; present only on the
/// Widgets tab with a non-Playfield widget selected.
#[derive(Component)]
pub struct RightInspectorRoot;

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

/// Settings rows are sized for readability from behind the kit, not just for a
/// mouse at desk distance.
const ROW_PAD_V: f32 = 8.0;
const ROW_GAP: f32 = 4.0;
const ROW_LABEL_FONT: f32 = 14.0;
const ROW_VALUE_FONT: f32 = 15.0;
const STEP_PAD_H: f32 = 11.0;
const STEP_PAD_V: f32 = 4.0;

/// Ring around the row holding nav focus.
pub const FOCUS_RING: Color = Color::srgb(0.89, 0.20, 0.20);
/// Ring around the row being adjusted (pad adjust mode).
pub const ADJUST_RING: Color = Color::srgb(0.16, 0.62, 0.36);

/// Tags a settings row control with its index into the active tab's item list.
#[derive(Component, Clone, Copy)]
pub struct SettingRow(pub usize);

/// Tags a stepper's glyph text so adjust mode can swap `<`/`>` for `−`/`+`.
#[derive(Component, Clone, Copy)]
pub struct StepperGlyph {
    pub row: usize,
    pub dir: i32,
}

/// Carries a settings row's one-line description, surfaced in the footer while
/// the row is hovered.
#[derive(Component, Clone, Copy)]
struct RowDesc(&'static str);

/// Tags the ◂ / ▸ adjust buttons on a settings row (dir = -1 / +1).
#[derive(Component, Clone, Copy)]
pub struct SettingAdjust {
    pub index: usize,
    pub dir: i32,
}

/// Tags the value text of a settings row for live refresh.
#[derive(Component, Clone, Copy)]
pub struct SettingValueText(pub usize);

/// Tags a settings-row slider with its index into the active tab's item list.
#[derive(Component, Clone, Copy)]
pub struct SettingSlider(pub usize);

/// "RESET TAB" button at the top of the left content panel: restores the active
/// settings tab's values to `Config::default()`.
#[derive(Component)]
pub struct ResetTabButton;

#[derive(Component)]
pub struct CalibrateButton;

pub use super::chrome::INSPECTOR_WIDTH as PANEL_WIDTH;

use super::chrome::LEFT_PANEL_WIDTH;

use super::chrome::TAB_BAR_HEIGHT;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            // Left content rebuilds on tab/content change only (NOT selection),
            // so picking a widget no longer respawns the whole left list. The
            // Local guard inside still debounces width-only Lanes changes.
            rebuild_left_content
                .after(super::profile_bar_ui::ProfileBarInteractionSet)
                .run_if(
                    resource_changed::<super::tabs::ActiveTab>
                        .or_else(resource_changed::<EditorOpen>)
                        .or_else(resource_changed::<Lanes>)
                        .or_else(resource_changed::<super::bindings_panel::BindingsRev>)
                        .or_else(resource_changed::<super::controls_panel::ControlsSegment>)
                        .or_else(resource_changed::<super::controls_panel::ControlsFocus>)
                        .or_else(resource_changed::<super::lanes_panel::SelectedLane>)
                        .or_else(resource_changed::<super::lanes_panel::LanesFocus>)
                        .or_else(resource_changed::<super::lanes_panel::AddChannelPopupOpen>)
                        .or_else(profile_popup_changed),
                ),
            // Right inspector rebuilds on selection change (+ tab/open) — this
            // is the only panel that reacts to Selection.
            rebuild_right_inspector.run_if(
                resource_changed::<Selection>
                    .or_else(resource_changed::<EditorOpen>)
                    .or_else(resource_changed::<super::tabs::ActiveTab>),
            ),
            (
                apply_panel_controls,
                apply_anchor_cells,
                handle_anchor_auto_cell,
                handle_reset,
                refresh_panel_values,
                handle_settings_adjust,
                apply_settings_sliders,
                handle_reset_tab,
                handle_calibrate_button,
                refresh_settings_values,
                update_hovered_desc,
                update_editor_legend.after(rebuild_left_content),
            )
                .run_if(super::editor_open),
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_panel);
}

fn profile_popup_changed(popup: Res<super::profile_bar_ui::ProfileBarPopup>) -> bool {
    popup.is_changed()
}

fn despawn_panel(
    mut commands: Commands,
    q: Query<Entity, Or<(With<LeftContentRoot>, With<RightInspectorRoot>)>>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Debounce signature for `rebuild_left_content`: everything the profile bar
/// and tab content visually depend on. `bar` holds the active bar kind's
/// selected name and dirty flag, `None` on tabs with no profile bar.
#[derive(PartialEq, Clone)]
struct LeftPanelSig {
    open: bool,
    lanes: String,
    tab: game_shell::CustomizeTab,
    bindings_rev: u64,
    segment: super::controls_panel::ControlsSegment,
    controls_focus: super::controls_panel::ControlsFocus,
    popup: super::profile_bar_ui::ProfileBarPopup,
    bar: Option<(String, bool)>,
    error: Option<super::profile_bar::ProfileUiError>,
    lane_selected: Option<usize>,
    lane_add_popup: bool,
    lanes_focus: super::lanes_panel::LanesFocus,
}

/// Controls-tab inputs bundled to stay under the system-param ceiling
/// alongside `ProfileBarInputs`: the segment-selector focus ring and the
/// currently selected channel (initial paint only — `highlight_selected_row`
/// keeps it live between rebuilds).
#[derive(SystemParam)]
struct ControlsInputs<'w> {
    focus: Res<'w, super::controls_panel::ControlsFocus>,
    selected: Res<'w, super::bindings_capture::SelectedChannel>,
    reset: Res<'w, super::bindings_panel::BindingsResetState>,
    ports: Res<'w, super::bindings_panel::MidiPortList>,
}

/// Profile-bar inputs, bundled to stay under Bevy's system-param ceiling
/// (`rebuild_left_content` already has a full plate of tab-content params).
#[derive(SystemParam)]
struct ProfileBarInputs<'w> {
    segment: Res<'w, super::controls_panel::ControlsSegment>,
    session: Res<'w, super::profile_state::CustomizeSession>,
    popup: Res<'w, super::profile_bar_ui::ProfileBarPopup>,
    error: Res<'w, super::profile_bar_ui::ProfileUiErrorState>,
}

/// Lanes-tab inputs, bundled to stay under Bevy's system-param ceiling
/// alongside `ProfileBarInputs`/`ControlsInputs`.
#[derive(SystemParam)]
struct LanesInputs<'w> {
    selected: Res<'w, super::lanes_panel::SelectedLane>,
    add_popup: Res<'w, super::lanes_panel::AddChannelPopupOpen>,
    focus: Res<'w, super::lanes_panel::LanesFocus>,
}

/// Left content panel: renders the profile bar (Controls/Lanes only) above
/// the active tab's content. Rebuilds on tab/content change only (NOT
/// selection). The debounce signature drops `selection.0` so picking a
/// widget never respawns this list.
#[allow(clippy::too_many_arguments)]
fn rebuild_left_content(
    mut commands: Commands,
    open: Res<EditorOpen>,
    selection: Res<Selection>,
    lanes: Res<Lanes>,
    active: Res<super::tabs::ActiveTab>,
    draft: Res<super::tabs::ConfigDraft>,
    live: Res<crate::bindings::LiveBindings>,
    rev: Res<super::bindings_panel::BindingsRev>,
    theme: Res<dtx_ui::ThemeResource>,
    midi: Option<Res<game_shell::MidiConnected>>,
    bar: ProfileBarInputs,
    controls: ControlsInputs,
    lanes_ui: LanesInputs,
    existing: Query<Entity, With<LeftContentRoot>>,
    mut last_sig: Local<Option<LeftPanelSig>>,
) {
    let segment = *bar.segment;
    let session = &bar.session;
    let popup = *bar.popup;
    let bar_error = &bar.error;
    let bar_kind = super::profile_bar_ui::bar_kind(active.0, segment);
    let bar_sig = bar_kind.map(|kind| {
        let info = super::profile_bar_ui::bar_info(kind, session);
        (info.selected, info.dirty)
    });
    let sig = LeftPanelSig {
        open: open.0,
        lanes: dtx_layout::structure_signature(&lanes.0),
        tab: active.0,
        bindings_rev: rev.0,
        segment,
        controls_focus: *controls.focus,
        popup,
        bar: bar_sig,
        error: bar_error.0.clone(),
        lane_selected: lanes_ui.selected.0,
        lane_add_popup: lanes_ui.add_popup.0,
        lanes_focus: *lanes_ui.focus,
    };
    if last_sig.as_ref() == Some(&sig) {
        return;
    }
    last_sig.replace(sig);
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    let t = theme.0;
    let root = commands
        .spawn((
            LeftContentRoot,
            EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(TAB_BAR_HEIGHT),
                bottom: Val::Px(0.0),
                width: Val::Px(LEFT_PANEL_WIDTH),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.92)),
            GlobalZIndex(crate::ui_z::EDITOR_CHROME),
        ))
        .id();

    if let Some(kind) = bar_kind {
        commands.entity(root).with_children(|p| {
            // Scope the error to this bar's kind: the error state is global
            // and only clears on the next successful action, so an unfiltered
            // pass would bleed a failed Keyboard rename under the MIDI/Lanes
            // bar after a tab/segment switch.
            let scoped_error = bar_error.0.as_ref().filter(|e| e.kind == kind);
            super::profile_bar_ui::spawn_bar(p, &t, kind, session, popup, scoped_error);
        });
    }

    // Pads can reach these tabs on the rail but cannot work their content.
    if midi.is_some_and(|m| m.0) && super::keyboard_nav::pad_excluded(active.0) {
        commands.entity(root).with_children(|p| {
            p.spawn((
                Text::new("keyboard/mouse required — pads: SD to go back"),
                dtx_ui::theme::Theme::font(10.0),
                TextColor(Color::srgba(1.0, 0.8, 0.3, 0.9)),
                Node {
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
            ));
        });
    }

    // The Controls tab renders its own block, so branch on it BEFORE the
    // generic settings-rows path.
    if active.0 == game_shell::CustomizeTab::Controls {
        super::bindings_panel::spawn_bindings_block(
            &mut commands,
            root,
            &t,
            &live,
            &lanes,
            &controls.ports,
            *controls.reset,
            segment,
            *controls.focus,
            controls.selected.0,
        );
        return;
    }
    if active.0.is_settings() {
        spawn_settings_block(&mut commands, root, &t, active.0, &draft);
        return;
    }
    // Kit tabs: the Lanes tab owns the lane block; the Widgets tab owns the
    // widget picker list (selecting Playfield here just shows no inspector —
    // the lane block lives on the dedicated Lanes tab).
    commands.entity(root).with_children(|p| match active.0 {
        game_shell::CustomizeTab::Lanes => super::lanes_panel::spawn_lane_block(
            p,
            &t,
            &lanes,
            lanes_ui.selected.0,
            lanes_ui.add_popup.0,
            *lanes_ui.focus,
        ),
        game_shell::CustomizeTab::Widgets => spawn_widget_list(p, &t, &selection),
        _ => {}
    });
}

/// Widget picker list (migrated from the rail): one Select button per widget
/// kind, the selected one tinted. Clicks are processed by ui.rs's
/// `handle_buttons` Select arm; `highlight_selection` keeps the tint in sync.
fn spawn_widget_list(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    selection: &Selection,
) {
    p.spawn((
        Text::new("Widgets"),
        dtx_ui::theme::Theme::font(13.0),
        TextColor(t.text_primary),
    ));
    for kind in WidgetKind::ALL {
        let e = super::ui::spawn_button(
            p,
            t,
            super::ui::EditorButton::Select(kind),
            kind.display_name(),
        );
        if selection.0 == Some(kind) {
            p.commands_mut()
                .entity(e)
                .insert(BackgroundColor(Color::srgb(0.22, 0.3, 0.42)));
        }
    }
}

/// Right inspector: the selected widget's knobs. Rebuilds on selection change
/// (+ tab/open). Present only on the Widgets tab with a non-Playfield widget
/// selected; otherwise the right edge stays empty.
fn rebuild_right_inspector(
    mut commands: Commands,
    open: Res<EditorOpen>,
    selection: Res<Selection>,
    layouts: Res<WidgetLayouts>,
    active: Res<super::tabs::ActiveTab>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<RightInspectorRoot>>,
    mut last_sig: Local<Option<(Option<WidgetKind>, game_shell::CustomizeTab, bool)>>,
) {
    let sig = (selection.0, active.0, open.0);
    if last_sig.as_ref() == Some(&sig) {
        return;
    }
    *last_sig = Some(sig);
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 || active.0 != game_shell::CustomizeTab::Widgets {
        return;
    }
    let Some(kind) = selection.0 else { return };
    if kind == WidgetKind::Playfield {
        return;
    }
    let t = theme.0;
    let root = commands
        .spawn((
            RightInspectorRoot,
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
            GlobalZIndex(crate::ui_z::EDITOR_CHROME),
        ))
        .id();

    commands.entity(root).with_children(|p| {
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

fn spawn_settings_block(
    commands: &mut Commands,
    root: Entity,
    t: &dtx_ui::theme::Theme,
    tab: game_shell::CustomizeTab,
    draft: &super::tabs::ConfigDraft,
) {
    use crate::editor::settings_data::SettingControl;
    let items = crate::editor::settings_data::settings_items(tab);
    commands.entity(root).with_children(|p| {
        // Header row: tab title on the left, RESET TAB on the right.
        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            h.spawn((
                Text::new(tab.label()),
                dtx_ui::theme::Theme::font(13.0),
                TextColor(t.text_primary),
            ));
            if tab == game_shell::CustomizeTab::Gameplay {
                h.spawn((
                    CalibrateButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        margin: UiRect::right(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.16, 0.24, 0.30)),
                    children![(
                        Text::new("Calibrate"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_primary),
                    )],
                ));
            }
            h.spawn((
                ResetTabButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.14, 0.14)),
                children![(
                    Text::new("RESET TAB"),
                    dtx_ui::theme::Theme::font(10.0),
                    TextColor(t.text_primary),
                )],
            ));
        });

        let mut prev_group = "";
        for (i, item) in items.iter().enumerate() {
            if item.group != prev_group && !item.group.is_empty() {
                p.spawn((
                    Text::new(item.group),
                    dtx_ui::theme::Theme::font(10.0),
                    TextColor(t.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                ));
            }
            prev_group = item.group;

            let modified = (item.value)(&draft.0) != (item.value)(&dtx_config::Config::default());

            p.spawn((
                SettingRow(i),
                RowDesc(item.desc),
                Interaction::default(),
                Outline::new(Val::Px(0.0), Val::Px(2.0), Color::NONE),
                Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(ROW_PAD_V)),
                    margin: UiRect::bottom(Val::Px(ROW_GAP)),
                    ..default()
                },
            ))
            .with_children(|r| {
                r.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|l| {
                    let mut dot = l.spawn(Node {
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    });
                    if modified {
                        dot.insert(BackgroundColor(t.select_yellow));
                    }
                    l.spawn((
                        Text::new(item.label),
                        dtx_ui::theme::Theme::font(ROW_LABEL_FONT),
                        TextColor(t.text_secondary),
                    ));
                });
                r.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|c| match item.control {
                    SettingControl::Slider { min, max, .. } => {
                        let e =
                            controls::spawn_slider(c, t, Slider { min, max }, (item.raw)(&draft.0));
                        c.commands_mut().entity(e).insert(SettingSlider(i));
                        c.spawn((
                            SettingValueText(i),
                            Text::new((item.value)(&draft.0)),
                            dtx_ui::theme::Theme::font(ROW_VALUE_FONT),
                            TextColor(t.text_primary),
                            TextLayout {
                                linebreak: bevy::text::LineBreak::NoWrap,
                                ..default()
                            },
                            Node {
                                min_width: Val::Px(52.0),
                                ..default()
                            },
                        ));
                    }
                    SettingControl::Stepper => {
                        c.spawn((
                            SettingAdjust { index: i, dir: -1 },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(STEP_PAD_H), Val::Px(STEP_PAD_V)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                            children![(
                                StepperGlyph { row: i, dir: -1 },
                                Text::new("<"),
                                dtx_ui::theme::Theme::font(ROW_VALUE_FONT),
                                TextColor(t.text_primary)
                            )],
                        ));
                        c.spawn((
                            SettingValueText(i),
                            Text::new((item.value)(&draft.0)),
                            dtx_ui::theme::Theme::font(ROW_VALUE_FONT),
                            TextColor(t.text_primary),
                            TextLayout {
                                linebreak: bevy::text::LineBreak::NoWrap,
                                ..default()
                            },
                            Node {
                                min_width: Val::Px(96.0),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ));
                        c.spawn((
                            SettingAdjust { index: i, dir: 1 },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(STEP_PAD_H), Val::Px(STEP_PAD_V)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                            children![(
                                StepperGlyph { row: i, dir: 1 },
                                Text::new(">"),
                                dtx_ui::theme::Theme::font(ROW_VALUE_FONT),
                                TextColor(t.text_primary)
                            )],
                        ));
                    }
                });
            });
        }
    });
}

/// Legend verbs for the current pad nav level.
fn legend_items(level: &super::keyboard_nav::NavLevel) -> &'static [(&'static str, &'static str)] {
    use super::keyboard_nav::NavLevel;
    match level {
        NavLevel::Rail => &[
            ("HH", "prev tab"),
            ("CY", "next tab"),
            ("BD", "enter"),
            ("SD", "close"),
        ],
        NavLevel::Rows => &[
            ("HH", "up"),
            ("CY", "down"),
            ("BD", "adjust"),
            ("SD", "tabs"),
        ],
        NavLevel::Adjust { .. } => &[
            ("HH", "−"),
            ("CY", "+"),
            ("BD", "confirm"),
            ("SD", "cancel"),
        ],
    }
}

/// Discriminant of the nav level, for change detection without cloning `saved`.
fn level_key(level: &super::keyboard_nav::NavLevel) -> u8 {
    use super::keyboard_nav::NavLevel;
    match level {
        NavLevel::Rail => 0,
        NavLevel::Rows => 1,
        NavLevel::Adjust { .. } => 2,
    }
}

/// Rebuild the legend bar at the panel bottom whenever the nav level, tab, or
/// MIDI presence changes — or whenever a panel rebuild despawned it. Hidden
/// entirely when no MIDI device is connected.
fn update_editor_legend(
    mut commands: Commands,
    midi: Option<Res<game_shell::MidiConnected>>,
    level: Res<super::keyboard_nav::NavLevel>,
    active: Res<super::tabs::ActiveTab>,
    theme: Res<dtx_ui::ThemeResource>,
    roots: Query<Entity, With<LeftContentRoot>>,
    legends: Query<Entity, With<dtx_ui::widget::nav_legend::NavLegend>>,
    mut last_sig: Local<Option<(u8, bool, game_shell::CustomizeTab)>>,
) {
    let connected = midi.is_some_and(|m| m.0);
    let sig = (level_key(&level), connected, active.0);
    let missing = connected && legends.is_empty();
    if last_sig.as_ref() == Some(&sig) && !missing {
        return;
    }
    *last_sig = Some(sig);
    for e in &legends {
        commands.entity(e).despawn();
    }
    if !connected {
        return;
    }
    let Ok(root) = roots.single() else {
        return;
    };
    let t = theme.0;
    let items = legend_items(&level);
    commands.entity(root).with_children(|p| {
        dtx_ui::widget::nav_legend::spawn_nav_legend(p, &t, items);
    });
}

/// Push the hovered settings row's description into the footer resource so the
/// footer chrome can render it.
fn update_hovered_desc(
    rows: Query<(&Interaction, &RowDesc), Changed<Interaction>>,
    mut hovered: ResMut<super::footer::HoveredDesc>,
) {
    for (interaction, row_desc) in &rows {
        if *interaction == Interaction::Hovered {
            hovered.0 = row_desc.0.to_string();
        }
    }
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

/// Settings-row slider drags → apply the snapped value to the config draft.
/// Snaps to the control's `step` and only writes on a full-step move, so the
/// draft never fights the continuous cursor position (mirrors the lane-width
/// slider pattern; ConfigDraft mutation drives the existing live-apply).
fn apply_settings_sliders(
    active: Res<super::tabs::ActiveTab>,
    sliders: Query<(&SettingSlider, &ControlValue), Changed<ControlValue>>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
) {
    use crate::editor::settings_data::SettingControl;
    if !active.0.is_settings() {
        return;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for (slider, value) in &sliders {
        let Some(item) = items.get(slider.0) else {
            continue;
        };
        if let SettingControl::Slider { min, max, step } = item.control {
            let snapped = (((value.0 - min) / step).round() * step + min).clamp(min, max);
            let cur = (item.raw)(&draft.0);
            if (snapped - cur).abs() > step * 0.5 {
                (item.set)(&mut draft.0, snapped);
            }
        }
    }
}

/// RESET TAB click: restore every row of the active settings tab to its
/// `Config::default()` value. Kit tabs don't spawn the button, so this is a
/// no-op there (settings-only for now).
fn handle_reset_tab(
    q: Query<&Interaction, (With<ResetTabButton>, Changed<Interaction>)>,
    active: Res<super::tabs::ActiveTab>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
) {
    if !active.0.is_settings() {
        return;
    }
    let pressed = q.iter().any(|i| *i == Interaction::Pressed);
    if !pressed {
        return;
    }
    let d = dtx_config::Config::default();
    for item in crate::editor::settings_data::settings_items(active.0) {
        (item.reset)(&mut draft.0, &d);
    }
}

/// Calibrate click: start the input-offset tap-test overlay.
fn handle_calibrate_button(
    q: Query<&Interaction, (With<CalibrateButton>, Changed<Interaction>)>,
    mut state: ResMut<super::calibration::CalibrationState>,
    mut metronome_on: ResMut<crate::resources::MetronomeEnabled>,
    mut timing_lines: ResMut<crate::resources::ShowTimingLines>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        super::calibration::start_calibration(
            &mut state,
            &mut metronome_on,
            &mut timing_lines,
            &mut autoplay,
        );
    }
}

/// Draft changes → refresh the visible settings values (text + slider knobs).
fn refresh_settings_values(
    active: Res<super::tabs::ActiveTab>,
    draft: Res<super::tabs::ConfigDraft>,
    mut texts: Query<(&SettingValueText, &mut Text)>,
    mut sliders: Query<(&SettingSlider, &mut ControlValue)>,
) {
    use crate::editor::settings_data::SettingControl;
    if !draft.is_changed() || !active.0.is_settings() {
        return;
    }
    let items = crate::editor::settings_data::settings_items(active.0);
    for (tag, mut text) in &mut texts {
        if let Some(item) = items.get(tag.0) {
            let want = (item.value)(&draft.0);
            if text.0 != want {
                text.0 = want;
            }
        }
    }
    // External changes (RESET TAB, live edits) → resync slider knobs. Guarded
    // by a half-step threshold so an active drag isn't yanked back mid-motion.
    for (slider, mut value) in &mut sliders {
        if let Some(item) = items.get(slider.0) {
            if let SettingControl::Slider { step, .. } = item.control {
                let want = (item.raw)(&draft.0);
                if (value.0 - want).abs() > step * 0.5 {
                    value.0 = want;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct RebuildCount(u32);

    fn count_rebuild(mut count: ResMut<RebuildCount>) {
        count.0 += 1;
    }

    #[test]
    fn profile_popup_change_requests_left_panel_rebuild() {
        let mut app = App::new();
        app.init_resource::<super::super::profile_bar_ui::ProfileBarPopup>()
            .init_resource::<RebuildCount>()
            .add_systems(Update, count_rebuild.run_if(profile_popup_changed));

        app.update();
        app.world_mut().resource_mut::<RebuildCount>().0 = 0;
        app.update();
        assert_eq!(app.world().resource::<RebuildCount>().0, 0);

        *app.world_mut()
            .resource_mut::<super::super::profile_bar_ui::ProfileBarPopup>() =
            super::super::profile_bar_ui::ProfileBarPopup::Selector;
        app.update();

        assert_eq!(app.world().resource::<RebuildCount>().0, 1);
    }
}
