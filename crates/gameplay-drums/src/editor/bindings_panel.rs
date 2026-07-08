//! Bindings tab content block: a DEVICE sub-section (velocity threshold) and a
//! CHANNELS list — one row per bindable drum channel with a color swatch, its
//! bind chips (each removable via `×`) and a `+` to start capture.
//!
//! The block is spawned by `panel::rebuild_left_content` for the Bindings tab.
//! Edits mutate `crate::bindings::LiveBindings` (the resolver + disk follow) and
//! bump `BindingsRev`, which re-triggers the left-panel rebuild so chips repaint.

use bevy::prelude::*;
use dtx_config::{BINDABLE_CHANNELS, BindSource};

use super::bindings_capture::CaptureState;
use crate::bindings::LiveBindings;
use crate::lanes::Lanes;

/// One channel row in the CHANNELS list.
#[derive(Component, Clone, Copy)]
pub struct BindChannelRow(pub dtx_core::EChannel);

/// `×` on a bind chip: removes `source[index]` from `channel`.
#[derive(Component, Clone, Copy)]
pub struct BindChipRemove {
    pub channel: dtx_core::EChannel,
    pub index: usize,
}

/// `+` on a channel row: starts capturing a new source for `channel`.
#[derive(Component, Clone, Copy)]
pub struct BindCaptureStart(pub dtx_core::EChannel);

/// ◂ / ▸ on the velocity-threshold row (dir = -1 / +1).
#[derive(Component, Clone, Copy)]
pub struct VelocityThresholdAdjust(pub i32);

/// ◂ / ▸ on the MIDI port row (dir = -1 / +1): cycles `MidiPortList`.
#[derive(Component, Clone, Copy)]
pub struct PortCycle(pub i32);

/// "Rescan" button in the DEVICE box: re-enumerates MIDI input ports.
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

pub fn plugin(app: &mut App) {
    app.init_resource::<BindingsRev>()
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
                handle_port_cycle,
                handle_rescan,
                update_velocity_meter,
            )
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
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

pub fn spawn_bindings_block(
    commands: &mut Commands,
    root: Entity,
    theme: &dtx_ui::theme::Theme,
    live: &LiveBindings,
    lanes: &Lanes,
    ports: &MidiPortList,
) {
    let t = theme;
    let threshold = live.0.midi.velocity_threshold;
    let port_label = port_display_label(&live.0.midi.port, &ports.0);
    let mark_pct = threshold as f32 / 127.0 * 100.0;
    commands.entity(root).with_children(|p| {
        p.spawn((
            Text::new("Bindings"),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(t.text_primary),
        ));

        // DEVICE sub-section: port selector, velocity threshold, velocity meter.
        p.spawn((
            Text::new("DEVICE"),
            dtx_ui::theme::Theme::font(10.0),
            TextColor(t.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ));

        // Port row: label + ◂ name ▸ cycler.
        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new("Port"),
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
                    PortCycle(-1),
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
                    Text::new(port_label),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    Node {
                        max_width: Val::Px(150.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ));
                c.spawn((
                    PortCycle(1),
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
                c.spawn((
                    RescanPorts,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        margin: UiRect::left(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                    children![(
                        Text::new("Rescan"),
                        dtx_ui::theme::Theme::font(10.0),
                        TextColor(t.text_secondary)
                    )],
                ));
            });
        });

        // Velocity threshold row (◂ value ▸).
        p.spawn(Node {
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
            .with_children(|c| {
                c.spawn((
                    VelocityThresholdAdjust(-1),
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
                    Text::new(threshold.to_string()),
                    dtx_ui::theme::Theme::font(12.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                ));
                c.spawn((
                    VelocityThresholdAdjust(1),
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

        // Velocity meter: a dark track with an accent fill (last velocity /
        // 127) and a thin threshold tick. Width/color driven each frame by
        // `update_velocity_meter`; the tick is placed here at spawn.
        p.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(8.0),
                margin: UiRect::top(Val::Px(3.0)),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.10, 0.10, 0.13)),
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

        // CHANNELS list: one row per bindable drum channel.
        p.spawn((
            Text::new("CHANNELS"),
            dtx_ui::theme::Theme::font(10.0),
            TextColor(t.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
        ));
        for ch in BINDABLE_CHANNELS {
            let swatch = lanes
                .col_of(ch)
                .map(|c| lanes.column_color(c))
                .unwrap_or(Color::WHITE);
            let name = ch.short_name().unwrap_or("?");
            let sources = live.0.map.get(&ch).cloned().unwrap_or_default();
            p.spawn((
                BindChannelRow(ch),
                Button,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|r| {
                r.spawn((
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(swatch),
                ));
                r.spawn((
                    Text::new(name),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_primary),
                    Node {
                        min_width: Val::Px(34.0),
                        ..default()
                    },
                ));
                for (index, src) in sources.iter().enumerate() {
                    r.spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(2.0),
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.22, 0.28)),
                    ))
                    .with_children(|chip| {
                        chip.spawn((
                            Text::new(source_label(src)),
                            dtx_ui::theme::Theme::font(10.0),
                            TextColor(t.text_primary),
                        ));
                        chip.spawn((
                            BindChipRemove { channel: ch, index },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(2.0), Val::Px(0.0)),
                                ..default()
                            },
                            children![(
                                Text::new("×"),
                                dtx_ui::theme::Theme::font(11.0),
                                TextColor(t.text_secondary),
                            )],
                        ));
                    });
                }
                r.spawn((
                    BindCaptureStart(ch),
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.14, 0.18)),
                    children![(
                        Text::new("+"),
                        dtx_ui::theme::Theme::font(12.0),
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

/// `×` on a chip: drop that source from the channel's list (bounds-checked).
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

/// `+` on a channel row: arm keyboard/MIDI capture for that channel.
fn handle_capture_start(
    q: Query<(&Interaction, &BindCaptureStart), Changed<Interaction>>,
    mut capture: ResMut<CaptureState>,
) {
    for (interaction, start) in &q {
        if *interaction == Interaction::Pressed {
            *capture = CaptureState::Capturing(start.0);
        }
    }
}

/// Text shown in the port row: the selected port, or a placeholder when the
/// selection is unset / the device list is empty.
fn port_display_label(selected: &Option<String>, list: &[String]) -> String {
    if list.is_empty() {
        return "(no MIDI devices)".to_string();
    }
    match selected {
        Some(p) => p.clone(),
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
    let amber = Color::srgb(0.85, 0.6, 0.1);
    for (mut node, mut bg) in &mut fill {
        node.width = Val::Percent(pct);
        bg.0 = if fresh && last.below_threshold {
            amber
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
}
