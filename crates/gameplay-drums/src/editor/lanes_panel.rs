//! Lanes tab content: slim reorder rows + a detail card for the selected
//! lane + a Hidden strip for unassigned channels.
//!
//! Rendered by `panel::rebuild_left_content` for the Lanes tab, below the
//! Task 4 profile bar (spawned separately — this module never re-renders
//! it, and never shows a preset/profile label of its own). Edits mutate
//! `crate::lanes::Lanes` (live preview), then `mirror_lane_edits_to_draft`
//! mirrors the arrangement into `LaneProfileDraft` and
//! `apply_lane_draft_preview` mirrors draft changes (undo/profile-select)
//! back onto the live preview.

use bevy::prelude::*;
use dtx_core::EChannel;
use dtx_layout::LaneArrangement;
use dtx_ui::widget::controls::{self, ControlValue, Slider};

use super::chrome;
use super::panel_kit;
use super::undo::{Snapshot, UndoStack};
use crate::lanes::Lanes;
use crate::widget_layout::WidgetLayouts;

/// Which lane row is selected — drives the detail card below the row list
/// and the lane-column highlight in the preview (`bindings_spatial`).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedLane(pub Option<usize>);

/// Whether the detail card's "+ add channel" popup is open.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddChannelPopupOpen(pub bool);

/// One row in the lane list: click selects it.
#[derive(Component, Clone, Copy)]
pub struct LaneRow(pub usize);

/// `×` on a secondary channel chip in the detail card: splits it into its
/// own lane.
#[derive(Component, Debug, Clone, Copy)]
pub struct ChipSplitBtn(pub EChannel);

/// Width slider in the detail card.
#[derive(Component, Debug, Clone, Copy)]
pub struct LaneWidthSlider(pub usize);

/// Numeric px readout next to the width slider.
#[derive(Component, Clone, Copy)]
pub struct LaneWidthValueText(pub usize);

/// "+ add" button: toggles the addable-channel popup.
#[derive(Component)]
pub struct AddChannelBtn;

/// One row in the addable-channel popup: merges that channel into the
/// selected lane.
#[derive(Component, Clone, Copy)]
pub struct AddChannelItem(pub EChannel);

/// "Hide lane" button in the detail card.
#[derive(Component, Clone, Copy)]
pub struct HideLaneBtn(pub usize);

/// One chip in the Hidden strip: restores that channel to its own lane.
#[derive(Component, Clone, Copy)]
pub struct RestoreChannelBtn(pub EChannel);

/// Focus level inside the Lanes tab, mirroring `controls_panel::ControlsFocus`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanesFocus {
    /// Focus rests on the Customize tab bar.
    #[default]
    TabBar,
    /// The lane row list has focus.
    Rows,
    /// The selected lane's detail card has focus.
    Detail,
}

/// What a nav verb did, for the caller to apply — `reduce_lanes_nav` stays
/// pure and never touches `LaneArrangement` or drafts directly, mirroring
/// `reduce_controls_nav`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanesNavEffect {
    None,
    /// Swap the lane at `index` with its neighbor in `dir` (-1 left, +1 right).
    Reorder { index: usize, dir: i32 },
    /// Nudge the lane at `index`'s width by `dir` steps (the driver, once
    /// wired, picks the ref-px step and clamps via `set_lane_width`).
    AdjustWidth { index: usize, dir: i32 },
}

/// Apply one nav verb to the Lanes focus/selection state. `selected` and
/// `lane_count` bound the row cursor. `coarse` is the surface's existing
/// shift-held modifier (`NavAction::coarse`, already used elsewhere for a
/// step multiplier) repurposed here as the "move" modifier: Up/Down while
/// held reorders the focused row instead of just moving the cursor.
pub fn reduce_lanes_nav(
    focus: LanesFocus,
    selected: usize,
    lane_count: usize,
    verb: game_shell::NavVerb,
    coarse: bool,
) -> (LanesFocus, usize, LanesNavEffect) {
    use game_shell::NavVerb;
    match focus {
        LanesFocus::TabBar => match verb {
            NavVerb::Down | NavVerb::Confirm if lane_count > 0 => {
                (LanesFocus::Rows, selected, LanesNavEffect::None)
            }
            _ => (focus, selected, LanesNavEffect::None),
        },
        LanesFocus::Rows => match verb {
            NavVerb::Up if coarse && selected > 0 => (
                focus,
                selected - 1,
                LanesNavEffect::Reorder {
                    index: selected,
                    dir: -1,
                },
            ),
            NavVerb::Down if coarse && selected + 1 < lane_count => (
                focus,
                selected + 1,
                LanesNavEffect::Reorder {
                    index: selected,
                    dir: 1,
                },
            ),
            NavVerb::Up => {
                if selected == 0 {
                    (LanesFocus::TabBar, selected, LanesNavEffect::None)
                } else {
                    (focus, selected - 1, LanesNavEffect::None)
                }
            }
            NavVerb::Down => (
                focus,
                (selected + 1).min(lane_count.saturating_sub(1)),
                LanesNavEffect::None,
            ),
            NavVerb::Confirm if lane_count > 0 => {
                (LanesFocus::Detail, selected, LanesNavEffect::None)
            }
            NavVerb::Back => (LanesFocus::TabBar, selected, LanesNavEffect::None),
            _ => (focus, selected, LanesNavEffect::None),
        },
        LanesFocus::Detail => match verb {
            NavVerb::Dec => (
                focus,
                selected,
                LanesNavEffect::AdjustWidth {
                    index: selected,
                    dir: -1,
                },
            ),
            NavVerb::Inc => (
                focus,
                selected,
                LanesNavEffect::AdjustWidth {
                    index: selected,
                    dir: 1,
                },
            ),
            NavVerb::Up | NavVerb::Back => (LanesFocus::Rows, selected, LanesNavEffect::None),
            // Confirm would cycle between detail sub-controls once the card
            // grows a second adjustable one; width is the only one today, so
            // this is a no-op (mirrors `reduce_controls_nav`'s catch-all arm).
            _ => (focus, selected, LanesNavEffect::None),
        },
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SelectedLane>()
        .init_resource::<AddChannelPopupOpen>()
        .add_systems(
            Update,
            (
                handle_lane_row_click,
                // Chained: manual edits land in Lanes first, then mirror into
                // the draft, and only then may the draft repaint the preview —
                // unordered execution could overwrite a same-frame edit with a
                // stale draft arrangement.
                (
                    handle_chip_split,
                    handle_add_channel_btn,
                    handle_add_channel_item,
                    handle_hide_lane_btn,
                    handle_restore_channel_btn,
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

/// Channels addable to lane `index`: every unassigned channel plus every
/// channel that's currently a SECONDARY chip on some other lane (moving a
/// primary would leave its lane empty — that's not offered here).
fn addable_channels(arr: &LaneArrangement, index: usize) -> Vec<EChannel> {
    dtx_layout::DRUM_CHANNELS
        .into_iter()
        .filter(|ch| match arr.lane_index_of(*ch) {
            None => true,
            Some(cur) => cur != index && arr.lanes[cur].primary != *ch,
        })
        .collect()
}

pub fn spawn_lane_block(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    lanes: &Lanes,
    selected: Option<usize>,
    add_popup_open: bool,
) {
    let body = panel_kit::spawn_card(p, "Lanes");
    // Reorder: drag pads in the preview (Task 9) for mouse; reduce_lanes_nav
    // for keyboard (not yet driven). Rows here are select-only.
    p.commands_mut().entity(body).with_children(|card| {
        for (i, lane) in lanes.0.lanes.iter().enumerate() {
            let is_selected = selected == Some(i);
            let chips = dtx_layout::lane_chips(&lanes.0, i);
            let secondary_names: Vec<&str> = chips
                .iter()
                .filter(|ch| **ch != lane.primary)
                .filter_map(|ch| dtx_layout::channel_short_name(*ch))
                .collect();
            let summary = if secondary_names.is_empty() {
                String::new()
            } else {
                format!("+{}", secondary_names.join(" +"))
            };
            card.spawn((
                LaneRow(i),
                Button,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    border: UiRect::left(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(if is_selected {
                    chrome::ROW_SELECTED_BG
                } else {
                    Color::NONE
                }),
                BorderColor::all(if is_selected {
                    chrome::ACCENT
                } else {
                    Color::NONE
                }),
            ))
            .with_children(|r| {
                r.spawn((
                    Text::new("\u{2261}"), // ≡ drag handle glyph; drag wiring is Task 9
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(chrome::TEXT_MUTED),
                ));
                panel_kit::spawn_channel_dot(r, lanes.column_color(i));
                r.spawn((
                    Text::new(lane.id.clone()),
                    dtx_ui::theme::Theme::font(12.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(28.0),
                        ..default()
                    },
                ));
                if !summary.is_empty() {
                    r.spawn((
                        Text::new(summary),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(chrome::TEXT_MUTED),
                    ));
                }
            });
        }
    });

    if let Some(i) = selected.filter(|&i| i < lanes.0.lanes.len()) {
        spawn_lane_detail_card(p, t, lanes, i, add_popup_open);
    }

    let hidden = dtx_layout::unassigned_channels(&lanes.0);
    if !hidden.is_empty() {
        spawn_hidden_strip(p, &hidden);
    }
}

fn spawn_lane_detail_card(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    lanes: &Lanes,
    index: usize,
    add_popup_open: bool,
) {
    let lane = &lanes.0.lanes[index];
    let title = format!("{} lane", lane.id);
    let body = panel_kit::spawn_card(p, &title);
    let width = lane.width;
    let chips = dtx_layout::lane_chips(&lanes.0, index);
    let primary = lane.primary;
    let addable = addable_channels(&lanes.0, index);
    p.commands_mut().entity(body).with_children(|card| {
        // Width row: label + slider + numeric px readout.
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new("Width"),
                dtx_ui::theme::Theme::font(11.0),
                TextColor(t.text_secondary),
            ));
            r.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|c| {
                let e = controls::spawn_slider(
                    c,
                    t,
                    Slider {
                        min: dtx_layout::MIN_LANE_WIDTH,
                        max: dtx_layout::MAX_LANE_WIDTH,
                    },
                    width,
                );
                c.commands_mut().entity(e).insert(LaneWidthSlider(index));
                c.spawn((
                    LaneWidthValueText(index),
                    Text::new(format!("{width:.0}px")),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(36.0),
                        ..default()
                    },
                ));
            });
        });

        // Channels row: primary flat, secondaries as split chips, `+` opens
        // the addable-channel popup.
        card.spawn((
            Text::new("Channels"),
            dtx_ui::theme::Theme::font(11.0),
            TextColor(t.text_secondary),
        ));
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Relative,
            ..default()
        })
        .with_children(|wrap| {
            wrap.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|r| {
                for ch in &chips {
                    let name = dtx_layout::channel_short_name(*ch).unwrap_or("?");
                    if *ch == primary {
                        r.spawn((
                            Text::new(name),
                            dtx_ui::theme::Theme::font(10.0),
                            TextColor(t.text_secondary),
                        ));
                    } else {
                        panel_kit::spawn_chip(
                            r,
                            &format!("{name} \u{00d7}"),
                            false,
                            (ChipSplitBtn(*ch), Button),
                        );
                    }
                }
                r.spawn((
                    AddChannelBtn,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CHIP_BG),
                    children![(
                        Text::new("+ add"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });

            if add_popup_open {
                spawn_add_channel_popup(wrap, t, &addable);
            }
        });

        card.spawn((
            HideLaneBtn(index),
            Button,
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                margin: UiRect::top(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.3, 0.14, 0.14)),
            children![(
                Text::new("Hide lane"),
                dtx_ui::theme::Theme::font(10.0),
                TextColor(t.text_primary),
            )],
        ));
    });
}

fn spawn_add_channel_popup(p: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, addable: &[EChannel]) {
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(24.0),
            left: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            min_width: Val::Px(110.0),
            row_gap: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(chrome::CARD_BG),
        BorderColor::all(chrome::CARD_BORDER),
        GlobalZIndex(crate::ui_z::EDITOR_MODAL),
    ))
    .with_children(|popup| {
        if addable.is_empty() {
            popup.spawn((
                Text::new("(none)"),
                dtx_ui::theme::Theme::font(10.0),
                TextColor(chrome::TEXT_MUTED),
            ));
        }
        for ch in addable {
            let name = dtx_layout::channel_short_name(*ch).unwrap_or("?");
            popup
                .spawn((
                    AddChannelItem(*ch),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new(name),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(t.text_primary),
                    ));
                });
        }
    });
}

fn spawn_hidden_strip(p: &mut ChildSpawnerCommands, hidden: &[EChannel]) {
    let body = panel_kit::spawn_card(p, "Hidden");
    p.commands_mut().entity(body).with_children(|card| {
        card.spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|r| {
            for ch in hidden {
                let name = dtx_layout::channel_short_name(*ch).unwrap_or("?");
                panel_kit::spawn_chip(r, name, false, (RestoreChannelBtn(*ch), Button));
            }
        });
    });
}

/// Row click: select that lane (closes any open add-channel popup — it was
/// keyed to the previously selected lane).
fn handle_lane_row_click(
    q: Query<(&LaneRow, &Interaction), Changed<Interaction>>,
    mut selected: ResMut<SelectedLane>,
    mut popup: ResMut<AddChannelPopupOpen>,
) {
    for (row, interaction) in &q {
        if *interaction == Interaction::Pressed && selected.0 != Some(row.0) {
            selected.0 = Some(row.0);
            if popup.0 {
                popup.0 = false;
            }
        }
    }
}

fn handle_chip_split(
    q: Query<(&ChipSplitBtn, &Interaction), Changed<Interaction>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<UndoStack>,
) {
    for (btn, interaction) in &q {
        if *interaction == Interaction::Pressed {
            let before = Snapshot {
                layouts: layouts.clone(),
                lanes: lanes.clone(),
            };
            if dtx_layout::split_channel(&mut lanes.0, btn.0) {
                undo.push_snapshot(before);
            }
        }
    }
}

fn handle_add_channel_btn(
    q: Query<&Interaction, (With<AddChannelBtn>, Changed<Interaction>)>,
    mut popup: ResMut<AddChannelPopupOpen>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            popup.0 = !popup.0;
        }
    }
}

fn handle_add_channel_item(
    q: Query<(&AddChannelItem, &Interaction), Changed<Interaction>>,
    selected: Res<SelectedLane>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<UndoStack>,
    mut popup: ResMut<AddChannelPopupOpen>,
) {
    let Some(index) = selected.0 else { return };
    for (item, interaction) in &q {
        if *interaction == Interaction::Pressed {
            let before = Snapshot {
                layouts: layouts.clone(),
                lanes: lanes.clone(),
            };
            if dtx_layout::merge_channel_into_lane(&mut lanes.0, item.0, index) {
                undo.push_snapshot(before);
            }
            popup.0 = false;
        }
    }
}

/// "Hide lane": drop the lane, its channels become unassigned (Hidden
/// strip), and clear the selection since the detail card's lane is gone.
fn handle_hide_lane_btn(
    q: Query<(&HideLaneBtn, &Interaction), Changed<Interaction>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<UndoStack>,
    mut selected: ResMut<SelectedLane>,
    mut popup: ResMut<AddChannelPopupOpen>,
) {
    for (btn, interaction) in &q {
        if *interaction == Interaction::Pressed {
            let before = Snapshot {
                layouts: layouts.clone(),
                lanes: lanes.clone(),
            };
            if !dtx_layout::hide_lane(&mut lanes.0, btn.0).is_empty() {
                undo.push_snapshot(before);
                selected.0 = None;
                popup.0 = false;
            }
        }
    }
}

fn handle_restore_channel_btn(
    q: Query<(&RestoreChannelBtn, &Interaction), Changed<Interaction>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<UndoStack>,
) {
    for (btn, interaction) in &q {
        if *interaction == Interaction::Pressed {
            let before = Snapshot {
                layouts: layouts.clone(),
                lanes: lanes.clone(),
            };
            if dtx_layout::restore_lane(&mut lanes.0, btn.0) {
                undo.push_snapshot(before);
            }
        }
    }
}

/// Width slider → Lanes. One undo snapshot per mouse-hold.
fn apply_lane_width_sliders(
    buttons: Res<ButtonInput<MouseButton>>,
    sliders: Query<(&LaneWidthSlider, &ControlValue), Changed<ControlValue>>,
    mut lanes: ResMut<Lanes>,
    layouts: Res<WidgetLayouts>,
    mut undo: ResMut<UndoStack>,
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

/// External Lanes changes (undo, preset) → refresh the width slider + its
/// numeric readout. Equality-guarded to terminate the write-back loop.
fn refresh_lane_panel_values(
    lanes: Res<Lanes>,
    mut sliders: Query<(&LaneWidthSlider, &mut ControlValue)>,
    mut texts: Query<(&LaneWidthValueText, &mut Text)>,
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
    for (tag, mut text) in &mut texts {
        if let Some(lane) = lanes.0.lanes.get(tag.0) {
            let want = format!("{:.0}px", lane.width);
            if text.0 != want {
                text.0 = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::EChannel;
    use game_shell::NavVerb;

    #[test]
    fn addable_channels_includes_unassigned_and_other_secondaries() {
        let mut arr = dtx_layout::classic();
        let hh = arr.lane_index_of(EChannel::HiHatClose).unwrap();
        let cy = arr.lane_index_of(EChannel::Cymbal).unwrap();
        // HHO is a secondary on the HH lane, so it must be addable to CY.
        assert!(addable_channels(&arr, cy).contains(&EChannel::HiHatOpen));
        // But NOT addable to its own (HH) lane — it's already there.
        assert!(!addable_channels(&arr, hh).contains(&EChannel::HiHatOpen));
        // SD is a primary elsewhere: never offered anywhere.
        assert!(!addable_channels(&arr, cy).contains(&EChannel::Snare));
        // Hide LC: it becomes unassigned and addable everywhere else.
        let lc = arr.lane_index_of(EChannel::LeftCymbal).unwrap();
        dtx_layout::hide_lane(&mut arr, lc);
        assert!(addable_channels(&arr, cy).contains(&EChannel::LeftCymbal));
    }

    #[test]
    fn lanes_tabbar_confirm_enters_rows() {
        let (focus, selected, effect) =
            reduce_lanes_nav(LanesFocus::TabBar, 0, 10, NavVerb::Confirm, false);
        assert_eq!(focus, LanesFocus::Rows);
        assert_eq!(selected, 0);
        assert_eq!(effect, LanesNavEffect::None);
    }

    #[test]
    fn lanes_tabbar_ignores_down_with_no_lanes() {
        let (focus, ..) = reduce_lanes_nav(LanesFocus::TabBar, 0, 0, NavVerb::Down, false);
        assert_eq!(focus, LanesFocus::TabBar);
    }

    #[test]
    fn lanes_rows_down_moves_selection_and_clamps_at_bottom() {
        let (_, selected, effect) =
            reduce_lanes_nav(LanesFocus::Rows, 0, 3, NavVerb::Down, false);
        assert_eq!(selected, 1);
        assert_eq!(effect, LanesNavEffect::None);
        let (_, selected, _) = reduce_lanes_nav(LanesFocus::Rows, 2, 3, NavVerb::Down, false);
        assert_eq!(selected, 2, "clamps at the last row");
    }

    #[test]
    fn lanes_rows_up_at_top_returns_to_tabbar() {
        let (focus, selected, _) = reduce_lanes_nav(LanesFocus::Rows, 0, 3, NavVerb::Up, false);
        assert_eq!(focus, LanesFocus::TabBar);
        assert_eq!(selected, 0);
        let (focus, selected, _) = reduce_lanes_nav(LanesFocus::Rows, 2, 3, NavVerb::Up, false);
        assert_eq!(focus, LanesFocus::Rows);
        assert_eq!(selected, 1);
    }

    #[test]
    fn lanes_rows_coarse_up_reorders_and_moves_selection_with_it() {
        let (focus, selected, effect) =
            reduce_lanes_nav(LanesFocus::Rows, 2, 5, NavVerb::Up, true);
        assert_eq!(focus, LanesFocus::Rows);
        assert_eq!(selected, 1, "selection follows the moved lane");
        assert_eq!(
            effect,
            LanesNavEffect::Reorder { index: 2, dir: -1 }
        );
    }

    #[test]
    fn lanes_rows_coarse_down_at_bottom_is_a_plain_noop() {
        let (focus, selected, effect) =
            reduce_lanes_nav(LanesFocus::Rows, 2, 3, NavVerb::Down, true);
        assert_eq!(focus, LanesFocus::Rows);
        assert_eq!(selected, 2, "already at the bottom, no reorder target");
        assert_eq!(effect, LanesNavEffect::None);
    }

    #[test]
    fn lanes_rows_confirm_enters_detail() {
        let (focus, selected, effect) =
            reduce_lanes_nav(LanesFocus::Rows, 1, 3, NavVerb::Confirm, false);
        assert_eq!(focus, LanesFocus::Detail);
        assert_eq!(selected, 1);
        assert_eq!(effect, LanesNavEffect::None);
    }

    #[test]
    fn lanes_detail_left_right_emit_width_adjust_effects() {
        let (focus, _, effect) = reduce_lanes_nav(LanesFocus::Detail, 3, 5, NavVerb::Dec, false);
        assert_eq!(focus, LanesFocus::Detail, "stays in Detail while adjusting");
        assert_eq!(
            effect,
            LanesNavEffect::AdjustWidth { index: 3, dir: -1 }
        );
        let (_, _, effect) = reduce_lanes_nav(LanesFocus::Detail, 3, 5, NavVerb::Inc, false);
        assert_eq!(effect, LanesNavEffect::AdjustWidth { index: 3, dir: 1 });
    }

    #[test]
    fn lanes_detail_back_returns_to_rows_keeping_selection() {
        let (focus, selected, effect) =
            reduce_lanes_nav(LanesFocus::Detail, 4, 6, NavVerb::Back, false);
        assert_eq!(focus, LanesFocus::Rows);
        assert_eq!(selected, 4);
        assert_eq!(effect, LanesNavEffect::None);
    }
}
