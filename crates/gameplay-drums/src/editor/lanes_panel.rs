//! Lanes tab content: extracted from `panel.rs` (Task 8, mechanical move).
//!
//! Rendered by `panel::rebuild_left_content` for the Lanes tab, below the
//! Task 4 profile bar (spawned separately — this module never re-renders
//! it). Edits mutate `crate::lanes::Lanes` (live preview), then
//! `mirror_lane_edits_to_draft` mirrors the arrangement into
//! `LaneProfileDraft` and `apply_lane_draft_preview` mirrors draft changes
//! (undo/profile-select) back onto the live preview.

use bevy::prelude::*;
use dtx_ui::widget::controls::{self, ControlValue, Slider};

use crate::lanes::Lanes;

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

#[derive(Component)]
pub struct PresetLabel;

fn preset_name(p: dtx_layout::LanePreset) -> &'static str {
    match p {
        dtx_layout::LanePreset::Classic => "classic",
        dtx_layout::LanePreset::NxTypeB => "nx type-b",
        dtx_layout::LanePreset::NxTypeD => "nx type-d",
        dtx_layout::LanePreset::Custom => "custom",
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            // Chained: manual edits land in Lanes first, then mirror into
            // the draft, and only then may the draft repaint the preview —
            // unordered execution could overwrite a same-frame edit with a
            // stale draft arrangement.
            (
                handle_lane_buttons,
                apply_lane_width_sliders,
                mirror_lane_edits_to_draft,
                apply_lane_draft_preview,
            )
                .chain(),
            refresh_lane_panel_values,
        )
            .run_if(super::editor_open)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

pub fn spawn_lane_block(p: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, lanes: &Lanes) {
    p.spawn((
        Text::new("Lanes"),
        dtx_ui::theme::Theme::font(13.0),
        TextColor(t.text_primary),
    ));

    // Profile row: the lane profile draft's name (built-in cycling is gone —
    // profile selection owns built-ins now).
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|r| {
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

fn handle_lane_buttons(
    reorders: Query<(&LaneReorderBtn, &Interaction), Changed<Interaction>>,
    merges: Query<(&LaneMergeBtn, &Interaction), Changed<Interaction>>,
    splits: Query<(&ChipSplitBtn, &Interaction), Changed<Interaction>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<crate::widget_layout::WidgetLayouts>,
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

/// Manual lane edits (buttons, sliders, undo) flow into the lane profile
/// draft: the arrangement changes but the selected profile name is kept, so
/// a user profile stays itself while edited instead of becoming a generic
/// Custom. Equality-guarded against the preview mirror to terminate.
pub fn mirror_lane_edits_to_draft(
    lanes: Res<Lanes>,
    mut draft: ResMut<super::profile_state::LaneProfileDraft>,
) {
    if !lanes.is_changed() {
        return;
    }
    if draft.0.value.arrangement != lanes.0 {
        draft.0.value.arrangement = lanes.0.clone();
    }
}

/// Draft arrangement → live playfield preview (`Lanes`). Selecting or
/// reverting a profile updates the draft, and the playfield follows without
/// touching the committed registry. Equality-guarded like the mirror above.
pub fn apply_lane_draft_preview(
    draft: Res<super::profile_state::LaneProfileDraft>,
    mut lanes: ResMut<Lanes>,
) {
    if !draft.is_changed() {
        return;
    }
    if lanes.0 != draft.0.value.arrangement {
        lanes.0 = draft.0.value.arrangement.clone();
    }
}

/// Width slider → Lanes. One undo snapshot per mouse-hold.
fn apply_lane_width_sliders(
    buttons: Res<ButtonInput<MouseButton>>,
    sliders: Query<(&LaneWidthSlider, &ControlValue), Changed<ControlValue>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<crate::widget_layout::WidgetLayouts>,
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
