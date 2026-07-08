//! Bindings tab content block: a DEVICE sub-section (velocity threshold) and a
//! CHANNELS list — one row per bindable drum channel with a color swatch, its
//! bind chips (each removable via `×`) and a `+` to start capture.
//!
//! The block is spawned by `panel::rebuild_left_content` for the Bindings tab.
//! Edits mutate `crate::bindings::LiveBindings` (the resolver + disk follow) and
//! bump `BindingsRev`, which re-triggers the left-panel rebuild so chips repaint.

use bevy::prelude::*;
use dtx_config::{BindSource, BINDABLE_CHANNELS};

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

/// Bumped by every bindings edit so `rebuild_left_content` repaints the block
/// (chip add/remove and same-length steals don't change the map length, so a
/// length-based signature would miss them — a monotonic revision never does).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct BindingsRev(pub u64);

/// Keyboard-capture state machine. Minimal for Phase 3a (Task 4 only sets
/// `Capturing`); Task 5 extends this with steal-confirm.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub enum CaptureState {
    #[default]
    Idle,
    Capturing(dtx_core::EChannel),
}

pub fn plugin(app: &mut App) {
    app.init_resource::<BindingsRev>()
        .init_resource::<CaptureState>()
        .add_systems(
            Update,
            (
                handle_velocity_adjust,
                handle_bind_chip_remove,
                handle_capture_start,
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
) {
    let t = theme;
    let threshold = live.0.midi.velocity_threshold;
    commands.entity(root).with_children(|p| {
        p.spawn((
            Text::new("Bindings"),
            dtx_ui::theme::Theme::font(13.0),
            TextColor(t.text_primary),
        ));

        // DEVICE sub-section: velocity threshold row (◂ value ▸).
        p.spawn((
            Text::new("DEVICE"),
            dtx_ui::theme::Theme::font(10.0),
            TextColor(t.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ));
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
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    padding: UiRect::vertical(Val::Px(2.0)),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_label_strips_prefixes() {
        assert_eq!(key_label(KeyCode::KeyX), "X");
        assert_eq!(key_label(KeyCode::Digit1), "1");
        assert_eq!(key_label(KeyCode::Space), "Space");
    }
}
