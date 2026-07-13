//! Controls tab content: a `Keyboard | MIDI` segment selector, a Device card
//! (MIDI segment only: port cycler, live status dot, velocity threshold +
//! meter) and a Pads card (both segments: one row per bindable drum channel
//! with its segment-filtered bind chips and a `+` to start capture).
//!
//! The block is spawned by `panel::rebuild_left_content` for the Controls
//! tab, below the Task 4 profile bar (spawned separately — this module never
//! re-renders it). Edits mutate `crate::bindings::LiveBindings` (the resolver
//! and disk follow) and bump `BindingsRev`, which re-triggers the left-panel
//! rebuild so chips repaint.

use bevy::prelude::*;
use dtx_input::{BindSource, InputBindings, SystemVerb, BINDABLE_CHANNELS, SYSTEM_VERBS};
use dtx_layout::LaneArrangement;

use super::bindings_capture::CaptureState;
use super::chrome;
use super::controls_panel::{ControlsFocus, ControlsSegment};
use super::panel_kit;
use crate::bindings::LiveBindings;
use crate::lanes::Lanes;

/// One channel row in the Pads card.
#[derive(Component, Clone, Copy)]
pub struct BindChannelRow(pub dtx_core::EChannel);

/// Marks a channel row with no source bound in the active segment (drives
/// the WARN_TINT baseline `highlight_selected_row` restores outside
/// selection).
#[derive(Component, Clone, Copy)]
pub struct UnboundRow;

/// The `×` remove button on a chip: removes `source[index]` from `channel`.
/// Lives on a dedicated small child node — the chip body itself is inert.
#[derive(Component, Clone, Copy)]
pub struct BindChipRemove {
    pub channel: dtx_core::EChannel,
    pub index: usize,
}

/// Carried by the (inert, hoverable) chip body so hovering it can light every
/// OTHER channel that also owns this source (see
/// `bindings_spatial::sync_hover_outlines`). Holds the chip's own channel so
/// that channel is excluded from the highlight.
#[derive(Component, Clone, Copy)]
pub struct ChipSource {
    pub channel: dtx_core::EChannel,
    pub source: BindSource,
}

/// `+` on a channel row: starts capturing a new source for `channel`.
#[derive(Component, Clone, Copy)]
pub struct BindCaptureStart(pub dtx_core::EChannel);

/// One system-verb row in the System card.
#[derive(Component, Clone, Copy)]
pub struct BindSystemRow(pub SystemVerb);

/// The `×` remove button on a system chip: removes `source[index]` from `verb`.
#[derive(Component, Clone, Copy)]
pub struct BindSystemChipRemove {
    pub verb: SystemVerb,
    pub index: usize,
}

/// `+` on a system row: starts capturing a new source for `verb`.
#[derive(Component, Clone, Copy)]
pub struct BindSystemCaptureStart(pub SystemVerb);

/// `Keyboard` / `MIDI` segment-selector button.
#[derive(Component, Clone, Copy)]
pub struct SegmentBtn(pub ControlsSegment);

/// ◂ / ▸ on the velocity-threshold row (dir = -1 / +1).
#[derive(Component, Clone, Copy)]
pub struct VelocityThresholdAdjust(pub i32);

/// ◂ / ▸ on the MIDI port row (dir = -1 / +1): cycles `MidiPortList`.
#[derive(Component, Clone, Copy)]
pub struct PortCycle(pub i32);

/// "Rescan" button in the Device card: re-enumerates MIDI input ports.
#[derive(Component, Clone, Copy)]
pub struct RescanPorts;

/// Fill node of the velocity meter (width % driven by `update_velocity_meter`).
#[derive(Component, Clone, Copy)]
pub struct VelocityMeterFill;

/// Threshold tick on the velocity meter (left % = threshold / 127).
#[derive(Component, Clone, Copy)]
pub struct VelocityMeterMark;

/// Enumerated MIDI input port names. Refreshed on editor open + on rescan.
/// Empty (no `midi` feature or no hardware) → the port row shows "(no MIDI
/// devices)" and cycling is a no-op.
#[derive(Resource, Default, Debug, Clone)]
pub struct MidiPortList(pub Vec<String>);

/// Bumped by every bindings edit so `rebuild_left_content` repaints the block
/// (chip add/remove and same-length steals don't change the map length, so a
/// length-based signature would miss them — a monotonic revision never does).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct BindingsRev(pub u64);

/// Reset confirmation for the Controls tab. Kept separate from capture so Esc
/// continues to cancel capture without also altering a pending reset.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BindingsResetState {
    #[default]
    Idle,
    Confirming,
}

#[derive(Component)]
pub struct ResetBindingsButton;

#[derive(Component)]
pub struct ConfirmResetBindingsButton;

#[derive(Component)]
pub struct CancelResetBindingsButton;

pub fn plugin(app: &mut App) {
    app.init_resource::<BindingsRev>()
        .init_resource::<BindingsResetState>()
        .init_resource::<MidiPortList>()
        .add_systems(
            Update,
            refresh_ports_on_open
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(resource_changed::<super::EditorOpen>),
        )
        .add_systems(
            Update,
            (
                handle_velocity_adjust,
                handle_bind_chip_remove,
                handle_capture_start,
                handle_system_chip_remove,
                handle_system_capture_start,
                handle_port_cycle,
                handle_rescan,
                handle_bindings_reset,
                update_velocity_meter,
                update_chip_hover_highlight,
            )
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        )
        .add_systems(
            Update,
            handle_segment_btn
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open)
                .run_if(super::profile_dialog::profile_dialog_closed),
        );
}

/// Short label for a `KeyCode`: strip the `Key`/`Digit` prefix (`KeyX` → "X",
/// `Digit1` → "1"), leave named keys as-is (`Space` → "Space"), fall back to the
/// Debug form for anything else.
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

/// Bindings are shown in the active display arrangement, delegating to the
/// shared display-order contract; bindable channels missing from the
/// arrangement still get a row at the end.
pub(super) fn bindable_channels_in_order(arrangement: &LaneArrangement) -> Vec<dtx_core::EChannel> {
    let mut channels: Vec<_> = super::controls_panel::channels_in_display_order(arrangement)
        .into_iter()
        .filter(|channel| BINDABLE_CHANNELS.contains(channel))
        .collect();
    for channel in BINDABLE_CHANNELS {
        if !channels.contains(&channel) {
            channels.push(channel);
        }
    }
    channels
}

/// One bind-source chip in a Controls-tab row. `index` is the position in the
/// channel's full (unfiltered, both segments mixed) source list, so
/// `BindChipRemove` keeps removing the right entry regardless of segment
/// filtering. `shared` marks a source another channel also holds.
pub struct SegmentChip {
    pub source: BindSource,
    pub label: String,
    pub index: usize,
    pub shared: bool,
}

/// One channel row for the active segment.
pub struct SegmentRow {
    pub channel: dtx_core::EChannel,
    pub chips: Vec<SegmentChip>,
    pub unbound: bool,
}

fn segment_matches(segment: ControlsSegment, source: &BindSource) -> bool {
    matches!(
        (segment, source),
        (ControlsSegment::Keyboard, BindSource::Key(_)) | (ControlsSegment::Midi, BindSource::Midi { .. })
    )
}

/// Index (into the channel's FULL, unfiltered source list) of the LAST
/// source belonging to `segment` — the target of a keyboard Backspace on the
/// Controls rows. `None` = nothing to delete (Backspace no-ops).
pub(super) fn last_segment_source_index(
    b: &InputBindings,
    channel: dtx_core::EChannel,
    segment: ControlsSegment,
) -> Option<usize> {
    b.map
        .get(&channel)?
        .iter()
        .rposition(|source| segment_matches(segment, source))
}

/// Rows for the active segment: only that segment's sources; `shared` = the
/// source is also held by another channel; `unbound` = the channel has no
/// source in this segment.
pub fn segment_rows(b: &InputBindings, segment: ControlsSegment, lanes: &LaneArrangement) -> Vec<SegmentRow> {
    bindable_channels_in_order(lanes)
        .into_iter()
        .map(|channel| {
            let sources = b.map.get(&channel).cloned().unwrap_or_default();
            let chips: Vec<SegmentChip> = sources
                .iter()
                .enumerate()
                .filter(|(_, source)| segment_matches(segment, source))
                .map(|(index, source)| SegmentChip {
                    source: *source,
                    label: source_label(source),
                    index,
                    shared: b
                        .map
                        .iter()
                        .any(|(other, v)| *other != channel && v.contains(source)),
                })
                .collect();
            let unbound = chips.is_empty();
            SegmentRow { channel, chips, unbound }
        })
        .collect()
}

/// One system-verb row for the active segment.
pub struct SystemSegmentRow {
    pub verb: SystemVerb,
    pub chips: Vec<SegmentChip>,
    pub unbound: bool,
}

/// Index (into the verb's FULL, unfiltered source list) of the LAST source
/// belonging to `segment` — the target of a keyboard Backspace on a System row.
/// `None` = nothing to delete (Backspace no-ops).
pub(super) fn last_system_source_index(
    b: &InputBindings,
    verb: SystemVerb,
    segment: ControlsSegment,
) -> Option<usize> {
    b.system
        .get(&verb)?
        .iter()
        .rposition(|source| segment_matches(segment, source))
}

/// Rows for the System card in the active segment. `shared` is always false:
/// a source a lane owns can never be bound to a verb (`lane_owner` refuses it),
/// so a system chip is never shared with a lane.
pub fn system_segment_rows(b: &InputBindings, segment: ControlsSegment) -> Vec<SystemSegmentRow> {
    SYSTEM_VERBS
        .into_iter()
        .map(|verb| {
            let chips: Vec<SegmentChip> = b
                .system_sources(verb)
                .iter()
                .enumerate()
                .filter(|(_, source)| segment_matches(segment, source))
                .map(|(index, source)| SegmentChip {
                    source: *source,
                    label: source_label(source),
                    index,
                    shared: false,
                })
                .collect();
            let unbound = chips.is_empty();
            SystemSegmentRow {
                verb,
                chips,
                unbound,
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_bindings_block(
    commands: &mut Commands,
    root: Entity,
    theme: &dtx_ui::theme::Theme,
    live: &LiveBindings,
    lanes: &Lanes,
    ports: &MidiPortList,
    reset: BindingsResetState,
    segment: ControlsSegment,
    focus: ControlsFocus,
    selected: Option<dtx_core::EChannel>,
) {
    let t = theme;
    commands.entity(root).with_children(|p| {
        spawn_segment_selector(p, t, segment, focus, reset);
        if segment == ControlsSegment::Midi {
            spawn_device_card(p, t, live, ports);
        }
        spawn_pads_card(p, t, live, lanes, segment, selected);
        spawn_system_card(p, t, live, segment);
    });
}

/// `Keyboard | MIDI` segment selector + the (small) reset-tab control, docked
/// above the segment's card(s).
fn spawn_segment_selector(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    segment: ControlsSegment,
    focus: ControlsFocus,
    reset: BindingsResetState,
) {
    let focused = focus == ControlsFocus::SegmentSelector;
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        margin: UiRect::bottom(Val::Px(2.0)),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                padding: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                column_gap: Val::Px(2.0),
                ..default()
            },
            BorderColor::all(if focused { chrome::ACCENT } else { Color::NONE }),
        ))
        .with_children(|group| {
            for seg in [ControlsSegment::Keyboard, ControlsSegment::Midi] {
                let active = seg == segment;
                group.spawn((
                    SegmentBtn(seg),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(if active { chrome::ACCENT } else { chrome::CHIP_BG }),
                    children![(
                        Text::new(seg.label()),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(if active { Color::WHITE } else { chrome::TEXT_MUTED }),
                    )],
                ));
            }
        });

        spawn_reset_controls(row, t, reset, segment);
    });
}

/// Small segment-scoped reset button (Idle) / scope text + confirm-cancel pair
/// (Confirming), docked at the right edge of the segment-selector row.
/// Rebuilt on segment change (the left panel repaints on `ControlsSegment`),
/// so the label always names the active segment.
fn spawn_reset_controls(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    reset: BindingsResetState,
    segment: ControlsSegment,
) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(4.0),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|actions| match reset {
        BindingsResetState::Idle => {
            actions.spawn((
                ResetBindingsButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.14, 0.14)),
                children![(
                    Text::new(match segment {
                        ControlsSegment::Keyboard => "Reset keyboard",
                        ControlsSegment::Midi => "Reset MIDI",
                    }),
                    dtx_ui::theme::Theme::font(9.0),
                    TextColor(t.text_primary),
                )],
            ));
        }
        BindingsResetState::Confirming => {
            actions.spawn((
                Text::new(match segment {
                    ControlsSegment::Keyboard => "Reset keyboard bindings to defaults?",
                    ControlsSegment::Midi => "Reset MIDI bindings, port and threshold to defaults?",
                }),
                dtx_ui::theme::Theme::font(9.0),
                TextColor(chrome::TEXT_MUTED),
            ));
            actions.spawn((
                ConfirmResetBindingsButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.45, 0.22, 0.12)),
                children![(
                    Text::new("Confirm reset"),
                    dtx_ui::theme::Theme::font(9.0),
                    TextColor(t.text_primary),
                )],
            ));
            actions.spawn((
                CancelResetBindingsButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(chrome::CHIP_BG),
                children![(
                    Text::new("Cancel"),
                    dtx_ui::theme::Theme::font(9.0),
                    TextColor(t.text_primary),
                )],
            ));
        }
    });
}

/// Small ◂/▸ stepper button sharing one style across the port cycler and the
/// velocity-threshold stepper.
fn spawn_stepper_button(parent: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, bundle: impl Bundle, label: &str) {
    parent.spawn((
        bundle,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(chrome::CHIP_BG),
        children![(
            Text::new(label.to_string()),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
        )],
    ));
}

/// MIDI-segment-only Device card: live status dot, port cycler + Rescan,
/// velocity threshold stepper + meter.
fn spawn_device_card(p: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, live: &LiveBindings, ports: &MidiPortList) {
    let threshold = live.0.midi.velocity_threshold;
    let port_label = port_display_label(&live.0.midi.port, &ports.0);
    let mark_pct = threshold as f32 / 127.0 * 100.0;
    let connected = !ports.0.is_empty()
        && !matches!(
            super::controls_panel::match_midi_port(live.0.midi.port.as_deref(), &ports.0),
            super::controls_panel::PortMatch::Disconnected
        );

    let body = panel_kit::spawn_card(p, "Device");
    p.commands_mut().entity(body).with_children(|c| {
        // Port row: status dot + "Port" label, ◂ name ▸ + Rescan.
        c.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|label_row| {
                panel_kit::spawn_channel_dot(label_row, if connected { chrome::OK } else { chrome::ERR });
                label_row.spawn((
                    Text::new("Port"),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_secondary),
                ));
            });
            r.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|c2| {
                spawn_stepper_button(c2, t, PortCycle(-1), "<");
                c2.spawn((
                    Text::new(port_label),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    TextLayout {
                        linebreak: bevy::text::LineBreak::NoWrap,
                        ..default()
                    },
                    Node {
                        max_width: Val::Px(140.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ));
                spawn_stepper_button(c2, t, PortCycle(1), ">");
                c2.spawn((
                    RescanPorts,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        margin: UiRect::left(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CHIP_BG),
                    children![(
                        Text::new("Rescan"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_secondary)
                    )],
                ));
            });
        });

        // Velocity threshold row (◂ value ▸).
        c.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new("Velocity threshold"),
                dtx_ui::theme::Theme::font(11.0),
                TextColor(t.text_secondary),
            ));
            r.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|c2| {
                spawn_stepper_button(c2, t, VelocityThresholdAdjust(-1), "<");
                c2.spawn((
                    Text::new(threshold.to_string()),
                    dtx_ui::theme::Theme::font(12.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ));
                spawn_stepper_button(c2, t, VelocityThresholdAdjust(1), ">");
            });
        });

        // Velocity meter: a dark track with an accent fill (last velocity /
        // 127) and a thin threshold tick. Width/color driven each frame by
        // `update_velocity_meter`; the tick is placed here at spawn.
        c.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                margin: UiRect::top(Val::Px(2.0)),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(chrome::PANEL_BG),
        ))
        .with_children(|m| {
            m.spawn((
                VelocityMeterFill,
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(t.accent),
            ));
            m.spawn((
                VelocityMeterMark,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(mark_pct),
                    width: Val::Px(2.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            ));
        });
    });
}

/// Pads card: one row per bindable channel, segment-filtered chips, unbound
/// warning tint, selected-row accent.
fn spawn_pads_card(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    live: &LiveBindings,
    lanes: &Lanes,
    segment: ControlsSegment,
    selected: Option<dtx_core::EChannel>,
) {
    let body = panel_kit::spawn_card(p, "Pads");
    let rows = segment_rows(&live.0, segment, &lanes.0);
    p.commands_mut().entity(body).with_children(|card| {
        for row in rows {
            let ch = row.channel;
            let swatch = lanes.col_of(ch).map(|c| lanes.column_color(c)).unwrap_or(Color::WHITE);
            let name = ch.short_name().unwrap_or("?");
            let is_selected = selected == Some(ch);
            let unbound = row.unbound;
            let mut row_cmds = card.spawn((
                BindChannelRow(ch),
                Button,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                    border: UiRect::left(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                BackgroundColor(if is_selected {
                    chrome::ROW_SELECTED_BG
                } else if unbound {
                    chrome::WARN_TINT
                } else {
                    Color::NONE
                }),
                BorderColor::all(if is_selected { chrome::ACCENT } else { Color::NONE }),
                // Zero-width baseline; `highlight_selected_row` widens it to
                // the FOCUS_RING while keyboard focus sits on the rows.
                Outline::new(Val::Px(0.0), Val::Px(1.0), Color::NONE),
            ));
            if unbound {
                row_cmds.insert(UnboundRow);
            }
            row_cmds.with_children(|r| {
                panel_kit::spawn_channel_dot(r, swatch);
                r.spawn((
                    Text::new(name),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(34.0),
                        ..default()
                    },
                ));
                for chip in &row.chips {
                    // Chip body: hoverable (Button → Interaction drives the
                    // shared-lane preview) but inert — no BindChipRemove, so
                    // hovering to preview can't also delete. The `×` glyph is
                    // its own small button carrying the remove marker.
                    let chip_id = panel_kit::spawn_chip(
                        r,
                        &chip.label,
                        chip.shared,
                        (ChipSource { channel: ch, source: chip.source }, Button),
                    );
                    r.commands_mut().entity(chip_id).with_children(|cc| {
                        cc.spawn((
                            BindChipRemove { channel: ch, index: chip.index },
                            Button,
                            // Clickable but does NOT block the chip body below
                            // it from hovering — so hovering the × keeps the
                            // shared-lane preview lit (bevy_ui picking ignores
                            // FocusPolicy; Pickable is the real knob).
                            Pickable {
                                should_block_lower: false,
                                is_hoverable: true,
                            },
                            Node {
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)),
                                margin: UiRect::left(Val::Px(2.0)),
                                ..default()
                            },
                            children![(
                                Text::new("\u{00d7}"),
                                dtx_ui::theme::Theme::font(11.0),
                                TextColor(chrome::TEXT_MUTED),
                            )],
                        ));
                    });
                }
                if unbound {
                    r.spawn((
                        Text::new("no binding"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(chrome::TEXT_MUTED),
                    ));
                }
                r.spawn((
                    BindCaptureStart(ch),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CHIP_BG),
                    children![(
                        Text::new("+"),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });
        }
    });
}

/// System card: one row per bindable system verb (Pause, Restart), with the
/// same segment-filtered chips / `×` / `+` grammar as a lane row. Unbound by
/// default — Escape keeps working, and note maps vary by brand.
fn spawn_system_card(
    p: &mut ChildSpawnerCommands,
    t: &dtx_ui::theme::Theme,
    live: &LiveBindings,
    segment: ControlsSegment,
) {
    let body = panel_kit::spawn_card(p, "System");
    let rows = system_segment_rows(&live.0, segment);
    p.commands_mut().entity(body).with_children(|card| {
        for row in rows {
            let verb = row.verb;
            let unbound = row.unbound;
            let mut row_cmds = card.spawn((
                BindSystemRow(verb),
                Button,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                    border: UiRect::left(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                // Zero-width baseline; `highlight_selected_system_row` widens it
                // to the FOCUS_RING while keyboard focus sits on the rows.
                Outline::new(Val::Px(0.0), Val::Px(1.0), Color::NONE),
            ));
            if unbound {
                row_cmds.insert(UnboundRow);
            }
            row_cmds.with_children(|r| {
                r.spawn((
                    Text::new(verb.label()),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(60.0),
                        ..default()
                    },
                ));
                for chip in &row.chips {
                    let chip_id = panel_kit::spawn_chip(r, &chip.label, false, ());
                    r.commands_mut().entity(chip_id).with_children(|cc| {
                        cc.spawn((
                            BindSystemChipRemove {
                                verb,
                                index: chip.index,
                            },
                            Button,
                            Pickable {
                                should_block_lower: false,
                                is_hoverable: true,
                            },
                            Node {
                                padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)),
                                margin: UiRect::left(Val::Px(2.0)),
                                ..default()
                            },
                            children![(
                                Text::new("\u{00d7}"),
                                dtx_ui::theme::Theme::font(11.0),
                                TextColor(chrome::TEXT_MUTED),
                            )],
                        ));
                    });
                }
                if unbound {
                    r.spawn((
                        Text::new("unbound"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(chrome::TEXT_MUTED),
                    ));
                }
                r.spawn((
                    BindSystemCaptureStart(verb),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CHIP_BG),
                    children![(
                        Text::new("+"),
                        dtx_ui::theme::Theme::font(11.0),
                        TextColor(t.text_primary),
                    )],
                ));
            });
        }
    });
}

/// ◂ / ▸ on the velocity-threshold row: clamp-adjust in `[0, 127]`.
fn handle_velocity_adjust(
    q: Query<(&Interaction, &VelocityThresholdAdjust), Changed<Interaction>>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    for (interaction, adj) in &q {
        if *interaction == Interaction::Pressed {
            let next = (live.0.midi.velocity_threshold as i32 + adj.0).clamp(0, 127) as u8;
            if next != live.0.midi.velocity_threshold {
                live.0.midi.velocity_threshold = next;
                rev.0 = rev.0.wrapping_add(1);
            }
        }
    }
}

/// Reset only the active segment: that segment's sources return to defaults,
/// the other segment's sources are untouched. MIDI reset also restores the
/// device fields (port, velocity threshold).
fn reset_segment(live: &mut LiveBindings, rev: &mut BindingsRev, segment: ControlsSegment) {
    let defaults = dtx_input::InputBindings::default();
    let channels: std::collections::HashSet<_> =
        live.0.map.keys().chain(defaults.map.keys()).copied().collect();
    let mut map = std::collections::HashMap::new();
    for ch in channels {
        let mut sources: Vec<_> = live
            .0
            .map
            .get(&ch)
            .into_iter()
            .flatten()
            .filter(|s| !segment_matches(segment, s))
            .cloned()
            .collect();
        sources.extend(
            defaults
                .map
                .get(&ch)
                .into_iter()
                .flatten()
                .filter(|s| segment_matches(segment, s))
                .cloned(),
        );
        if !sources.is_empty() {
            map.insert(ch, sources);
        }
    }
    live.0.map = map;
    if segment == ControlsSegment::Midi {
        live.0.midi = defaults.midi;
    }
    rev.0 = rev.0.wrapping_add(1);
}

fn cancel_bindings_reset(state: &mut BindingsResetState) {
    *state = BindingsResetState::Idle;
}

fn handle_bindings_reset(
    reset: Query<&Interaction, (With<ResetBindingsButton>, Changed<Interaction>)>,
    confirm: Query<&Interaction, (With<ConfirmResetBindingsButton>, Changed<Interaction>)>,
    cancel: Query<&Interaction, (With<CancelResetBindingsButton>, Changed<Interaction>)>,
    segment: Res<ControlsSegment>,
    mut state: ResMut<BindingsResetState>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    if reset
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        *state = BindingsResetState::Confirming;
        rev.0 = rev.0.wrapping_add(1);
        return;
    }
    if confirm
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        reset_segment(&mut live, &mut rev, *segment);
        *state = BindingsResetState::Idle;
        return;
    }
    if cancel
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        cancel_bindings_reset(&mut state);
        rev.0 = rev.0.wrapping_add(1);
    }
}

/// The `×` button on a chip: drop that source from the channel's list
/// (bounds-checked). The chip body itself is inert — only the `×` removes.
fn handle_bind_chip_remove(
    q: Query<(&Interaction, &BindChipRemove), Changed<Interaction>>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    for (interaction, chip) in &q {
        if *interaction == Interaction::Pressed {
            if let Some(sources) = live.0.map.get_mut(&chip.channel) {
                if chip.index < sources.len() {
                    sources.remove(chip.index);
                    rev.0 = rev.0.wrapping_add(1);
                }
            }
        }
    }
}

/// `+` on a channel row: arm source-specific capture for that channel. The
/// active Controls segment decides which device the capture listens to.
fn handle_capture_start(
    q: Query<(&Interaction, &BindCaptureStart), Changed<Interaction>>,
    segment: Res<ControlsSegment>,
    mut capture: ResMut<CaptureState>,
) {
    for (interaction, start) in &q {
        if *interaction == Interaction::Pressed {
            *capture = match *segment {
                ControlsSegment::Keyboard => CaptureState::Keyboard(start.0),
                ControlsSegment::Midi => CaptureState::Midi(start.0),
            };
        }
    }
}

/// The `×` button on a system chip: drop that source from the verb's list.
fn handle_system_chip_remove(
    q: Query<(&Interaction, &BindSystemChipRemove), Changed<Interaction>>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    for (interaction, chip) in &q {
        if *interaction == Interaction::Pressed {
            if let Some(sources) = live.0.system.get_mut(&chip.verb) {
                if chip.index < sources.len() {
                    sources.remove(chip.index);
                    rev.0 = rev.0.wrapping_add(1);
                }
            }
        }
    }
}

/// `+` on a system row: arm segment-specific capture for that verb.
fn handle_system_capture_start(
    q: Query<(&Interaction, &BindSystemCaptureStart), Changed<Interaction>>,
    segment: Res<ControlsSegment>,
    mut capture: ResMut<CaptureState>,
) {
    for (interaction, start) in &q {
        if *interaction == Interaction::Pressed {
            *capture = match *segment {
                ControlsSegment::Keyboard => CaptureState::SystemKey {
                    verb: start.0,
                    refused: None,
                },
                ControlsSegment::Midi => CaptureState::SystemMidi {
                    verb: start.0,
                    refused: None,
                },
            };
        }
    }
}

/// `Keyboard` / `MIDI` segment button click: switches the active segment.
/// Gated (in `plugin`) by both `editor_open` and `profile_dialog_closed` so a
/// segment can never switch while a profile dialog is open.
fn handle_segment_btn(
    q: Query<(&Interaction, &SegmentBtn), Changed<Interaction>>,
    mut segment: ResMut<ControlsSegment>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed && *segment != btn.0 {
            *segment = btn.0;
        }
    }
}

/// Each frame, light every OTHER channel that shares the source of whichever
/// chip is currently hovered/pressed (first match wins); nothing hovered, or a
/// source only this channel owns, clears the highlight. A single deterministic
/// pass — not per-entity `Changed<Interaction>` deltas, which could clobber
/// the shared resource when the pointer jumps between two chips in one frame.
fn update_chip_hover_highlight(
    q: Query<(&Interaction, &ChipSource)>,
    live: Res<LiveBindings>,
    mut highlighted: ResMut<super::bindings_spatial::HighlightedChannels>,
) {
    let owners = q
        .iter()
        .find(|(interaction, _)| matches!(interaction, Interaction::Hovered | Interaction::Pressed))
        .map(|(_, chip)| {
            let all = match chip.source {
                BindSource::Key(k) => live.0.channels_for_key(k),
                BindSource::Midi { note } => live.0.channels_for_note(note),
            };
            all.into_iter().filter(|c| *c != chip.channel).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if highlighted.0 != owners {
        highlighted.0 = owners;
    }
}

/// Truncate a long device name to `max` chars with a trailing ellipsis.
fn truncate_label(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Text shown in the port row: the selected port, or a placeholder when the
/// selection is unset / the device list is empty.
fn port_display_label(selected: &Option<String>, list: &[String]) -> String {
    if list.is_empty() {
        return "(no MIDI devices)".to_string();
    }
    match selected {
        Some(p) => truncate_label(p, 22),
        None => "(first available)".to_string(),
    }
}

/// Cycle a port index by `dir`, wrapping within `[0, len)`. `cur == None`
/// (unset / not in list) is treated as index 0. Caller guarantees `len > 0`.
fn port_cycle_index(cur: Option<usize>, dir: i32, len: usize) -> usize {
    let cur = cur.unwrap_or(0) as i32;
    (cur + dir).rem_euclid(len as i32) as usize
}

/// On editor open, re-enumerate MIDI ports; bump `BindingsRev` on change so the
/// panel repaints with the fresh list.
fn refresh_ports_on_open(
    open: Res<super::EditorOpen>,
    mut ports: ResMut<MidiPortList>,
    mut rev: ResMut<BindingsRev>,
) {
    if open.0 {
        let fresh = dtx_input::midi::available_ports();
        if fresh != ports.0 {
            ports.0 = fresh;
            rev.0 = rev.0.wrapping_add(1);
        }
    }
}

/// ◂ / ▸ on the port row: cycle `MidiPortList`, set `LiveBindings.midi.port`
/// (which triggers the reconnect), and bump `BindingsRev` to repaint. No-op
/// when no ports are available.
fn handle_port_cycle(
    q: Query<(&Interaction, &PortCycle), Changed<Interaction>>,
    ports: Res<MidiPortList>,
    mut live: ResMut<LiveBindings>,
    mut rev: ResMut<BindingsRev>,
) {
    if ports.0.is_empty() {
        return;
    }
    for (interaction, cyc) in &q {
        if *interaction == Interaction::Pressed {
            let cur = live
                .0
                .midi
                .port
                .as_ref()
                .and_then(|p| ports.0.iter().position(|n| n == p));
            let next = port_cycle_index(cur, cyc.0, ports.0.len());
            live.0.midi.port = Some(ports.0[next].clone());
            rev.0 = rev.0.wrapping_add(1);
        }
    }
}

/// "Rescan" button: re-enumerate ports and repaint.
fn handle_rescan(
    q: Query<&Interaction, (Changed<Interaction>, With<RescanPorts>)>,
    mut ports: ResMut<MidiPortList>,
    mut rev: ResMut<BindingsRev>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            ports.0 = dtx_input::midi::available_ports();
            rev.0 = rev.0.wrapping_add(1);
        }
    }
}

/// Drive the velocity meter each frame from `LastMidiHit`: fill width = velocity
/// / 127, accent color (amber when the hit was below threshold), decaying to 0
/// when the last hit is older than 150 ms. The threshold tick tracks the live
/// threshold.
fn update_velocity_meter(
    last: Res<crate::LastMidiHit>,
    live: Res<LiveBindings>,
    theme: Res<dtx_ui::ThemeResource>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), With<VelocityMeterFill>>,
    mut mark: Query<&mut Node, (With<VelocityMeterMark>, Without<VelocityMeterFill>)>,
) {
    let fresh = last
        .at
        .map(|t| t.elapsed().as_millis() <= 150)
        .unwrap_or(false);
    let pct = if fresh {
        (last.velocity as f32 / 127.0 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };
    for (mut node, mut bg) in &mut fill {
        node.width = Val::Percent(pct);
        bg.0 = if fresh && last.below_threshold {
            chrome::DIRTY
        } else {
            theme.0.accent
        };
    }
    let mark_pct = live.0.midi.velocity_threshold as f32 / 127.0 * 100.0;
    for mut node in &mut mark {
        node.left = Val::Percent(mark_pct);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_label_strips_prefixes() {
        assert_eq!(key_label(KeyCode::KeyX), "X");
        assert_eq!(key_label(KeyCode::Digit1), "1");
        assert_eq!(key_label(KeyCode::Space), "Space");
    }

    #[test]
    fn port_cycle_index_wraps() {
        assert_eq!(port_cycle_index(Some(0), 1, 3), 1);
        assert_eq!(port_cycle_index(Some(2), 1, 3), 0);
        assert_eq!(port_cycle_index(Some(0), -1, 3), 2);
        assert_eq!(port_cycle_index(None, 1, 3), 1);
        assert_eq!(port_cycle_index(None, -1, 3), 2);
    }

    #[test]
    fn truncate_label_adds_ellipsis() {
        assert_eq!(truncate_label("short", 22), "short");
        let long = "NUX NTK-61:NUX NTK-61 Midi 32:0";
        let out = truncate_label(long, 22);
        assert_eq!(out.chars().count(), 22);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn port_display_label_variants() {
        assert_eq!(port_display_label(&None, &[]), "(no MIDI devices)");
        assert_eq!(
            port_display_label(&None, &["A".to_string()]),
            "(first available)"
        );
        assert_eq!(
            port_display_label(&Some("Kit".to_string()), &["Kit".to_string()]),
            "Kit"
        );
    }

    #[test]
    fn binding_rows_follow_classic_display_order() {
        use dtx_core::EChannel;

        assert_eq!(
            bindable_channels_in_order(&dtx_layout::classic()),
            [
                EChannel::LeftCymbal,
                EChannel::HiHatClose,
                EChannel::HiHatOpen,
                EChannel::LeftPedal,
                EChannel::Snare,
                EChannel::HighTom,
                EChannel::BassDrum,
                EChannel::LeftBassDrum,
                EChannel::LowTom,
                EChannel::FloorTom,
                EChannel::Cymbal,
                EChannel::RideCymbal,
            ]
        );
    }

    #[test]
    fn binding_rows_group_type_b_pedals_once() {
        use dtx_core::EChannel;

        let rows = bindable_channels_in_order(&dtx_layout::nx_type_b());

        assert_eq!(rows.len(), BINDABLE_CHANNELS.len());
        for channel in BINDABLE_CHANNELS {
            assert_eq!(rows.iter().filter(|&&row| row == channel).count(), 1);
        }
        let lp = rows.iter().position(|&row| row == EChannel::LeftPedal);
        let lbd = rows.iter().position(|&row| row == EChannel::LeftBassDrum);
        assert_eq!(lbd, lp.map(|index| index + 1));
    }

    #[test]
    fn segment_filters_sources_and_flags_shared() {
        use dtx_input::{BindSource, InputBindings};

        let mut b = InputBindings::default();
        b.bind_shared(dtx_core::EChannel::LeftBassDrum, BindSource::Key(KeyCode::Space));
        let rows = segment_rows(&b, ControlsSegment::Keyboard, &dtx_layout::classic());

        let bd = rows.iter().find(|r| r.channel == dtx_core::EChannel::BassDrum).unwrap();
        assert!(bd.chips.iter().all(|c| matches!(c.source, BindSource::Key(_))));
        assert!(bd.chips.iter().any(|c| c.shared), "Space now on BD+LBD");

        let hh = rows.iter().find(|r| r.channel == dtx_core::EChannel::HiHatClose).unwrap();
        assert!(hh.chips.iter().all(|c| !c.shared));
    }

    #[test]
    fn segment_rows_preserve_unfiltered_source_index() {
        // A mixed source vec: the MIDI entry sits at index 0, so the two key
        // chips MUST report index 1 and 2 (their position in the full list),
        // not 0 and 1 — otherwise `BindChipRemove` would delete the wrong
        // entry once MIDI sources are filtered out of the Keyboard segment.
        use dtx_core::EChannel;
        use dtx_input::{BindSource, InputBindings};

        let mut b = InputBindings::default();
        b.map.insert(
            EChannel::Snare,
            vec![
                BindSource::Midi { note: 60 },
                BindSource::Key(KeyCode::KeyA),
                BindSource::Key(KeyCode::KeyB),
            ],
        );
        let rows = segment_rows(&b, ControlsSegment::Keyboard, &dtx_layout::classic());
        let sd = rows.iter().find(|r| r.channel == EChannel::Snare).unwrap();
        assert_eq!(sd.chips.len(), 2, "only the two key chips show in Keyboard");
        assert_eq!(sd.chips[0].index, 1);
        assert!(matches!(sd.chips[0].source, BindSource::Key(KeyCode::KeyA)));
        assert_eq!(sd.chips[1].index, 2);
        assert!(matches!(sd.chips[1].source, BindSource::Key(KeyCode::KeyB)));
    }

    #[test]
    fn segment_rows_flag_channel_with_no_segment_source_as_unbound() {
        // LeftBassDrum has no MIDI default (keyboard-only channel), so it must
        // read `unbound` on the MIDI segment even though it IS bound overall.
        let b = InputBindings::default();
        let rows = segment_rows(&b, ControlsSegment::Midi, &dtx_layout::classic());
        let lbd = rows
            .iter()
            .find(|r| r.channel == dtx_core::EChannel::LeftBassDrum)
            .unwrap();
        assert!(lbd.unbound);
        assert!(lbd.chips.is_empty());
    }

    #[test]
    fn keyboard_reset_keeps_midi_map_and_device() {
        let mut live = LiveBindings(dtx_input::InputBindings::default());
        let mut rev = BindingsRev(0);
        live.0.midi.velocity_threshold = 64;
        for sources in live.0.map.values_mut() {
            sources.retain(|s| !matches!(s, dtx_input::BindSource::Key(_)));
        }
        reset_segment(&mut live, &mut rev, ControlsSegment::Keyboard);
        let defaults = dtx_input::InputBindings::default();
        for (ch, def_sources) in &defaults.map {
            for s in def_sources
                .iter()
                .filter(|s| matches!(s, dtx_input::BindSource::Key(_)))
            {
                assert!(
                    live.0.map.get(ch).is_some_and(|v| v.contains(s)),
                    "{ch:?} missing {s:?}"
                );
            }
        }
        // MIDI sources untouched by a keyboard reset.
        assert!(live
            .0
            .map
            .get(&dtx_core::EChannel::Snare)
            .is_some_and(|v| v.contains(&BindSource::Midi { note: 38 })));
        assert_eq!(live.0.midi.velocity_threshold, 64);
        assert_eq!(rev.0, 1);
    }

    #[test]
    fn midi_reset_keeps_keyboard_map_and_resets_device() {
        let mut live = LiveBindings(dtx_input::InputBindings::default());
        let mut rev = BindingsRev(0);
        live.0.midi.velocity_threshold = 64;
        live.0.midi.port = Some("test-port".into());
        // Custom (non-default) keyboard bind: a regression back to whole-state
        // reset would restore defaults and wipe it.
        let custom = dtx_input::BindSource::Key(KeyCode::KeyQ);
        live.0
            .map
            .entry(dtx_core::EChannel::Snare)
            .or_default()
            .push(custom);
        reset_segment(&mut live, &mut rev, ControlsSegment::Midi);
        assert_eq!(
            live.0.midi.velocity_threshold,
            dtx_input::InputBindings::default().midi.velocity_threshold
        );
        assert_eq!(live.0.midi.port, None, "MIDI reset restores default port");
        assert!(
            live.0
                .map
                .get(&dtx_core::EChannel::Snare)
                .is_some_and(|v| v.contains(&custom)),
            "custom keyboard bind survives MIDI reset"
        );
        let defaults = dtx_input::InputBindings::default();
        for (ch, def_sources) in &defaults.map {
            for s in def_sources
                .iter()
                .filter(|s| matches!(s, dtx_input::BindSource::Key(_)))
            {
                assert!(live.0.map.get(ch).is_some_and(|v| v.contains(s)));
            }
        }
    }

    #[test]
    fn last_segment_source_index_picks_last_matching_segment() {
        use dtx_core::EChannel;
        use dtx_input::{BindSource, InputBindings};

        let mut b = InputBindings::default();
        b.map.insert(
            EChannel::Snare,
            vec![
                BindSource::Key(KeyCode::KeyA),
                BindSource::Midi { note: 60 },
                BindSource::Key(KeyCode::KeyB),
                BindSource::Midi { note: 61 },
            ],
        );
        assert_eq!(
            last_segment_source_index(&b, EChannel::Snare, ControlsSegment::Keyboard),
            Some(2),
            "last KEY source, full-list index"
        );
        assert_eq!(
            last_segment_source_index(&b, EChannel::Snare, ControlsSegment::Midi),
            Some(3)
        );
        // No source in the segment → None (Backspace no-ops).
        b.map.insert(EChannel::HighTom, vec![BindSource::Midi { note: 48 }]);
        assert_eq!(
            last_segment_source_index(&b, EChannel::HighTom, ControlsSegment::Keyboard),
            None
        );
        // Unknown channel → None.
        b.map.remove(&EChannel::LowTom);
        assert_eq!(
            last_segment_source_index(&b, EChannel::LowTom, ControlsSegment::Keyboard),
            None
        );
    }

    #[test]
    fn system_rows_are_segment_filtered_and_flag_unbound() {
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));

        let midi_rows = system_segment_rows(&b, ControlsSegment::Midi);
        let pause = midi_rows
            .iter()
            .find(|r| r.verb == SystemVerb::Pause)
            .expect("Pause row exists");
        assert_eq!(pause.chips.len(), 1);
        assert_eq!(pause.chips[0].label, "N37");
        assert_eq!(pause.chips[0].index, 0, "index into the FULL source list");
        assert!(!pause.chips[0].shared, "a verb source is never a lane's");

        let restart = midi_rows
            .iter()
            .find(|r| r.verb == SystemVerb::Restart)
            .expect("Restart row exists");
        assert!(restart.unbound, "unbound by default");

        let kb_rows = system_segment_rows(&b, ControlsSegment::Keyboard);
        let pause = kb_rows
            .iter()
            .find(|r| r.verb == SystemVerb::Pause)
            .expect("Pause row exists");
        assert_eq!(pause.chips.len(), 1);
        assert_eq!(pause.chips[0].index, 1, "full-list index, not filtered");
    }

    #[test]
    fn last_system_source_index_picks_last_in_segment() {
        use dtx_input::{BindSource, InputBindings, SystemVerb};

        let mut b = InputBindings::default();
        b.bind_system(SystemVerb::Pause, BindSource::Key(KeyCode::F9));
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 37 });
        b.bind_system(SystemVerb::Pause, BindSource::Midi { note: 53 });

        assert_eq!(
            last_system_source_index(&b, SystemVerb::Pause, ControlsSegment::Midi),
            Some(2)
        );
        assert_eq!(
            last_system_source_index(&b, SystemVerb::Pause, ControlsSegment::Keyboard),
            Some(0)
        );
        assert_eq!(
            last_system_source_index(&b, SystemVerb::Restart, ControlsSegment::Midi),
            None,
            "unbound verb: Backspace no-ops"
        );
    }

    #[test]
    fn cancel_reset_leaves_bindings_unchanged() {
        let mut state = BindingsResetState::Confirming;

        cancel_bindings_reset(&mut state);

        assert_eq!(state, BindingsResetState::Idle);
    }
}
