//! Capture MODAL rendering (Task 6). Draws the pure `CaptureState` machine
//! from `bindings_capture.rs` — this file only renders it and feeds mouse
//! clicks into the SAME `arrived_step` reducer the keyboard drives; the state
//! machine itself is untouched here.
//!
//! Scrim scope: the scrim + card are confined to the LEFT PANEL column
//! (`x in [0, LEFT_PANEL_WIDTH]`, matching `panel.rs`'s own root rect), not
//! the full window. The shrunk playfield preview lives entirely to the right
//! of that column (`stage::preset_rect` reserves `LEFT_PANEL_WIDTH` as its
//! left margin), so the target lane lit by `bindings_spatial` is never under
//! the scrim — no low-alpha compromise needed.
//!
//! Rebuild-on-signature-change (mirrors `panel.rs`'s `last_sig` guard): if
//! this rebuilt every frame like a naive `close_dialog`-style despawn/respawn
//! would, the choice/confirm buttons would be fresh entities every frame,
//! and `Changed<Interaction>` would fire every single frame a button is held
//! (not just on the transition into `Pressed`) — a real mouse-click bug, not
//! just wasted work. `CaptureState` itself can't be used as the change
//! signal (`capture_binding` unconditionally writes it every frame, so
//! `resource_changed` is always true); `modal_lines()`'s `PartialEq` output
//! is the real signal.

use bevy::prelude::*;

use super::bindings_capture::{ArrivedChoice, ArrivedInput, CaptureState, MouseArrivedInput};
use super::chrome;

#[derive(Component)]
struct CaptureModalRoot;

/// The live "note N · velocity V" line shown while `Midi(ch)` is listening;
/// refreshed every frame from `LastMidiHit` independent of modal rebuilds.
#[derive(Component)]
struct CaptureLiveText;

#[derive(Component, Clone, Copy)]
struct CaptureChoiceBtn(ArrivedChoice);

#[derive(Component)]
struct CaptureConfirmBtn;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            sync_capture_modal,
            update_capture_live_text,
            handle_capture_mouse_input.before(super::bindings_capture::capture_binding),
        )
            .run_if(in_state(game_shell::AppState::Performance))
            .run_if(super::editor_open),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_modal);
}

fn despawn_modal(mut commands: Commands, roots: Query<Entity, With<CaptureModalRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

// ===== Pure state → text mapping (unit-tested below, no ECS needed) =====

/// Everything the modal needs to render for one `CaptureState`, minus the
/// live MIDI-listening line (that comes from `LastMidiHit`, refreshed every
/// frame independent of this — see `update_capture_live_text`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalLines {
    pub title: String,
    /// Listening states only ("Esc cancel"); `Arrived` states leave this
    /// `None` since the footer already carries their verbs (Task 2).
    pub subtitle: Option<String>,
    /// `Arrived` states only: the captured key/note preview line.
    pub arrived: Option<String>,
    /// `Arrived` states with a conflict: "also bound to {names}".
    pub owners_caption: Option<String>,
    pub choice: Option<ArrivedChoice>,
    pub has_conflict: bool,
}

fn channel_name(ch: dtx_core::EChannel) -> &'static str {
    ch.short_name().unwrap_or("channel")
}

/// Mirrors `bindings_panel::key_label` / `bindings_spatial::key_label` — each
/// module that renders a `KeyCode` keeps its own copy (established pattern
/// in this codebase; see `bindings_spatial.rs`'s comment on the same fn).
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

fn owners_caption(owners: &[dtx_core::EChannel]) -> String {
    let names = owners.iter().copied().map(channel_name).collect::<Vec<_>>().join(", ");
    format!("also bound to {names}")
}

/// Pure `CaptureState` → modal text/mode mapping. `None` means no modal
/// (`Idle`).
pub fn modal_lines(state: &CaptureState) -> Option<ModalLines> {
    match state {
        CaptureState::Idle => None,
        CaptureState::Keyboard(ch) => Some(ModalLines {
            title: format!("Press a key for {}", channel_name(*ch)),
            subtitle: Some("Esc cancel".to_string()),
            arrived: None,
            owners_caption: None,
            choice: None,
            has_conflict: false,
        }),
        CaptureState::Midi(ch) => Some(ModalLines {
            title: format!("Hit a pad for {}", channel_name(*ch)),
            subtitle: Some("Esc cancel".to_string()),
            arrived: None,
            owners_caption: None,
            choice: None,
            has_conflict: false,
        }),
        CaptureState::KeyArrived { key, owners, choice, .. } => {
            let has_conflict = !owners.is_empty();
            Some(ModalLines {
                title: "Confirm binding".to_string(),
                subtitle: None,
                arrived: Some(key_label(*key)),
                owners_caption: has_conflict.then(|| owners_caption(owners)),
                choice: Some(*choice),
                has_conflict,
            })
        }
        CaptureState::MidiArrived { note, velocity, owners, choice, .. } => {
            let has_conflict = !owners.is_empty();
            Some(ModalLines {
                title: "Confirm binding".to_string(),
                subtitle: None,
                arrived: Some(format!("note {note} · velocity {velocity}")),
                owners_caption: has_conflict.then(|| owners_caption(owners)),
                choice: Some(*choice),
                has_conflict,
            })
        }
    }
}

/// Live "note N · velocity V" line for the `Midi(ch)` listening state.
/// `None` before any hit has been observed since capture armed; `Some((text,
/// muted))` after — `muted` true when the hit was below the velocity
/// threshold (it never reaches the capture machine, but the user should
/// still see it landed).
pub fn live_hit_line(hit: &crate::LastMidiHit) -> Option<(String, bool)> {
    hit.at?;
    if hit.below_threshold {
        Some((format!("note {} · velocity {} — below threshold", hit.note, hit.velocity), true))
    } else {
        Some((format!("note {} · velocity {}", hit.note, hit.velocity), false))
    }
}

/// Click→`ArrivedInput` mapping for a choice button: clicking the choice
/// that's already active COMMITS it (mirrors "press Enter on the default"),
/// clicking the other one TOGGLES to it — the same reducer the keyboard's
/// ←/→ (Toggle) and Enter (Confirm) drive.
pub fn choice_click_input(clicked: ArrivedChoice, current: ArrivedChoice) -> ArrivedInput {
    if clicked == current {
        ArrivedInput::Confirm
    } else {
        ArrivedInput::Toggle
    }
}

// ===== Render =====

fn spawn_choice_btn(p: &mut ChildSpawnerCommands, t: &dtx_ui::theme::Theme, choice: ArrivedChoice, label: &str, current: ArrivedChoice) {
    let active = choice == current;
    p.spawn((
        CaptureChoiceBtn(choice),
        Button,
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(if active { chrome::ROW_SELECTED_BG } else { chrome::CHIP_BG }),
        BorderColor::all(if active { chrome::ACCENT } else { chrome::CHIP_BORDER }),
        children![(Text::new(label.to_owned()), dtx_ui::theme::Theme::font(12.0), TextColor(t.text_primary))],
    ));
}

fn sync_capture_modal(
    mut commands: Commands,
    capture: Res<CaptureState>,
    last_midi: Res<crate::LastMidiHit>,
    theme: Res<dtx_ui::ThemeResource>,
    roots: Query<Entity, With<CaptureModalRoot>>,
    mut last: Local<Option<ModalLines>>,
) {
    let lines = modal_lines(&capture);
    if *last == lines {
        return;
    }
    *last = lines.clone();
    for e in &roots {
        commands.entity(e).despawn();
    }
    let Some(lines) = lines else {
        return;
    };
    let t = theme.0;
    let listening_midi = matches!(*capture, CaptureState::Midi(_));
    let live = live_hit_line(&last_midi);

    commands
        .spawn((
            CaptureModalRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(chrome::TAB_BAR_HEIGHT),
                bottom: Val::Px(0.0),
                width: Val::Px(chrome::LEFT_PANEL_WIDTH),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(crate::ui_z::EDITOR_MODAL),
        ))
        .with_children(|scrim| {
            scrim
                .spawn((
                    Node {
                        width: Val::Px(280.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(20.0)),
                        row_gap: Val::Px(10.0),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(chrome::CARD_BG),
                    BorderColor::all(chrome::CARD_BORDER),
                ))
                .with_children(|card| {
                    card.spawn((Text::new(lines.title.clone()), dtx_ui::theme::Theme::font(16.0), TextColor(t.text_primary)));

                    if let Some(subtitle) = &lines.subtitle {
                        card.spawn((Text::new(subtitle.clone()), dtx_ui::theme::Theme::font(12.0), TextColor(chrome::TEXT_MUTED)));
                    }

                    if listening_midi {
                        let (text, muted) = live.unwrap_or_else(|| ("Waiting for a hit…".to_string(), true));
                        card.spawn((
                            CaptureLiveText,
                            Text::new(text),
                            dtx_ui::theme::Theme::font(13.0),
                            TextColor(if muted { chrome::TEXT_MUTED } else { t.text_primary }),
                        ));
                    }

                    if let Some(arrived) = &lines.arrived {
                        card.spawn((Text::new(arrived.clone()), dtx_ui::theme::Theme::font(20.0), TextColor(t.text_primary)));
                    }

                    if lines.has_conflict {
                        let current = lines.choice.unwrap_or_default();
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_choice_btn(row, &t, ArrivedChoice::Shared, "Add shared", current);
                            spawn_choice_btn(row, &t, ArrivedChoice::Move, "Move here", current);
                        });
                        if let Some(caption) = &lines.owners_caption {
                            card.spawn((Text::new(caption.clone()), dtx_ui::theme::Theme::font(11.0), TextColor(chrome::TEXT_MUTED)));
                        }
                    } else if lines.arrived.is_some() {
                        card.spawn((
                            CaptureConfirmBtn,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(chrome::ACCENT),
                            children![(Text::new("Confirm (Enter)"), dtx_ui::theme::Theme::font(13.0), TextColor(t.text_primary))],
                        ));
                    }
                });
        });
}

/// Refresh the `Midi(ch)` live line every frame — independent of
/// `sync_capture_modal`'s rebuild gate, so a stream of below-threshold hits
/// updates without tearing down (and re-picking-confusing) the modal.
fn update_capture_live_text(
    capture: Res<CaptureState>,
    last_midi: Res<crate::LastMidiHit>,
    theme: Res<dtx_ui::ThemeResource>,
    mut q: Query<(&mut Text, &mut TextColor), With<CaptureLiveText>>,
) {
    if !matches!(*capture, CaptureState::Midi(_)) {
        return;
    }
    let Ok((mut text, mut color)) = q.single_mut() else {
        return;
    };
    let (next_text, muted) = live_hit_line(&last_midi).unwrap_or_else(|| ("Waiting for a hit…".to_string(), true));
    if text.0 != next_text {
        text.0 = next_text;
    }
    *color = TextColor(if muted { chrome::TEXT_MUTED } else { theme.0.text_primary });
}

/// Choice/confirm button clicks → `MouseArrivedInput`, drained by
/// `capture_binding` through the exact same `arrived_step` reducer the
/// keyboard drives (ordered `.before` it above).
fn handle_capture_mouse_input(
    capture: Res<CaptureState>,
    choice_buttons: Query<(&Interaction, &CaptureChoiceBtn), Changed<Interaction>>,
    confirm_buttons: Query<&Interaction, (With<CaptureConfirmBtn>, Changed<Interaction>)>,
    mut mouse_input: ResMut<MouseArrivedInput>,
) {
    let current_choice = match &*capture {
        CaptureState::KeyArrived { choice, .. } | CaptureState::MidiArrived { choice, .. } => Some(*choice),
        _ => None,
    };
    for (interaction, btn) in &choice_buttons {
        if *interaction == Interaction::Pressed {
            if let Some(current) = current_choice {
                mouse_input.0 = Some(choice_click_input(btn.0, current));
            }
        }
    }
    for interaction in &confirm_buttons {
        if *interaction == Interaction::Pressed {
            mouse_input.0 = Some(ArrivedInput::Confirm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_has_no_modal() {
        assert_eq!(modal_lines(&CaptureState::Idle), None);
    }

    #[test]
    fn listening_states_show_title_and_esc_subtitle() {
        let lines = modal_lines(&CaptureState::Keyboard(dtx_core::EChannel::Snare)).unwrap();
        assert_eq!(lines.title, "Press a key for SD");
        assert_eq!(lines.subtitle.as_deref(), Some("Esc cancel"));
        assert_eq!(lines.arrived, None);

        let lines = modal_lines(&CaptureState::Midi(dtx_core::EChannel::HiHatClose)).unwrap();
        assert_eq!(lines.title, "Hit a pad for HH");
        assert_eq!(lines.subtitle.as_deref(), Some("Esc cancel"));
    }

    #[test]
    fn key_arrived_without_conflict_has_no_choice_and_no_caption() {
        let lines = modal_lines(&CaptureState::KeyArrived {
            channel: dtx_core::EChannel::Snare,
            key: KeyCode::KeyX,
            owners: vec![],
            choice: ArrivedChoice::Shared,
        })
        .unwrap();
        assert_eq!(lines.arrived.as_deref(), Some("X"));
        assert!(!lines.has_conflict);
        assert_eq!(lines.owners_caption, None);
    }

    #[test]
    fn key_arrived_with_conflict_reports_owner_names_and_choice() {
        let lines = modal_lines(&CaptureState::KeyArrived {
            channel: dtx_core::EChannel::Snare,
            key: KeyCode::KeyX,
            owners: vec![dtx_core::EChannel::HiHatClose, dtx_core::EChannel::LowTom],
            choice: ArrivedChoice::Move,
        })
        .unwrap();
        assert!(lines.has_conflict);
        assert_eq!(lines.owners_caption.as_deref(), Some("also bound to HH, LT"));
        assert_eq!(lines.choice, Some(ArrivedChoice::Move));
    }

    #[test]
    fn midi_arrived_line_reports_note_and_velocity() {
        let lines = modal_lines(&CaptureState::MidiArrived {
            channel: dtx_core::EChannel::Snare,
            note: 38,
            velocity: 90,
            owners: vec![],
            choice: ArrivedChoice::Shared,
        })
        .unwrap();
        assert_eq!(lines.arrived.as_deref(), Some("note 38 · velocity 90"));
    }

    #[test]
    fn live_hit_line_is_none_before_any_hit() {
        let hit = crate::LastMidiHit::default();
        assert_eq!(live_hit_line(&hit), None);
    }

    #[test]
    fn live_hit_line_mutes_below_threshold_hits() {
        let hit = crate::LastMidiHit {
            note: 38,
            velocity: 5,
            below_threshold: true,
            at: Some(std::time::Instant::now()),
        };
        let (text, muted) = live_hit_line(&hit).unwrap();
        assert!(text.contains("below threshold"));
        assert!(muted);
    }

    #[test]
    fn live_hit_line_shows_plain_line_above_threshold() {
        let hit = crate::LastMidiHit {
            note: 38,
            velocity: 90,
            below_threshold: false,
            at: Some(std::time::Instant::now()),
        };
        let (text, muted) = live_hit_line(&hit).unwrap();
        assert_eq!(text, "note 38 · velocity 90");
        assert!(!muted);
    }

    #[test]
    fn choice_click_on_active_choice_confirms_the_inactive_one_toggles() {
        // Clicking the choice that's already active commits it (mirrors
        // pressing Enter on the default); clicking the other one toggles —
        // matching arrow-key semantics exactly since there are only two
        // choices, so one Toggle always lands on the clicked value.
        assert_eq!(
            choice_click_input(ArrivedChoice::Shared, ArrivedChoice::Shared),
            ArrivedInput::Confirm
        );
        assert_eq!(
            choice_click_input(ArrivedChoice::Move, ArrivedChoice::Shared),
            ArrivedInput::Toggle
        );
        assert_eq!(
            choice_click_input(ArrivedChoice::Shared, ArrivedChoice::Move),
            ArrivedInput::Toggle
        );
        assert_eq!(
            choice_click_input(ArrivedChoice::Move, ArrivedChoice::Move),
            ArrivedInput::Confirm
        );
    }
}
