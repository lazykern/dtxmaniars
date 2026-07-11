//! In-Performance layout editor overlay. Opens only via an editor session
//! (Customize entry from Title/SongSelect); never toggleable mid-gameplay.
//!
//! Opening force-enables autoplay (notes flow hands-free), gates drum input +
//! pause, and spawns the sidebar. Closing restores the prior autoplay flag and
//! despawns the UI. All mutation targets `WidgetLayouts` / `Lanes`, which the
//! HUD already reacts to (plan 1 + 2).

use bevy::prelude::*;
use game_shell::AppState;

pub mod bindings_capture;
pub mod bindings_panel;
pub mod bindings_spatial;
pub mod calibration;
pub mod chrome;
pub mod controls_panel;
pub mod drag;
pub mod footer;
pub mod hotkeys;
pub mod keyboard_nav;
pub mod panel;
pub mod picking;
pub mod profile_bar;
pub mod profile_dialog;
pub mod profile_state;
pub mod save;
pub mod selection_box;
pub mod session;
pub mod settings_data;
pub mod snap;
pub mod stage;
pub mod tabs;
pub mod ui;
pub mod undo;

/// True while the editor overlay is open. Default false — normal play/practice.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EditorOpen(pub bool);

/// Request to close the overlay through the same save-on-close path as Esc.
#[derive(Debug, Clone, Copy, Message)]
pub struct EditorCloseRequest;

/// Remembers the autoplay flag from before the editor forced it on.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PrevAutoplay(pub bool);

/// Single source of truth for the Customize preview's frame state. Computed
/// once per frame (before the editor sets); systems read this instead of
/// re-deriving open/peek/tab/inspector themselves.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PreviewState {
    pub open: bool,
    /// Tab held: full play view peek (chrome + overlays hidden, identity rect).
    pub peeking: bool,
    pub tab: game_shell::CustomizeTab,
    /// Widgets tab with a live selection → right inspector reserves space.
    pub has_inspector: bool,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            open: false,
            peeking: false,
            // Mirrors `tabs::ActiveTab::default()` (Widgets landing).
            tab: game_shell::CustomizeTab::Widgets,
            has_inspector: false,
        }
    }
}

fn update_preview_state(
    open: Res<EditorOpen>,
    keys: Res<ButtonInput<KeyCode>>,
    active: Res<tabs::ActiveTab>,
    selection: Res<drag::Selection>,
    mut state: ResMut<PreviewState>,
) {
    let next = PreviewState {
        open: open.0,
        peeking: open.0 && keys.pressed(KeyCode::Tab),
        tab: active.0,
        has_inspector: active.0 == game_shell::CustomizeTab::Widgets && selection.0.is_some(),
    };
    if *state != next {
        *state = next;
    }
}

/// Ordering: picking (AABBs/hover) → gestures (drag) → overlay sync.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorPickSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorGestureSet;

pub fn plugin(app: &mut App) {
    app.add_message::<EditorCloseRequest>()
        .init_resource::<EditorOpen>()
        .init_resource::<PrevAutoplay>()
        .init_resource::<PreviewState>()
        .init_resource::<drag::Selection>()
        .init_resource::<undo::UndoStack>()
        .init_resource::<controls_panel::ControlsSegment>()
        .init_resource::<controls_panel::ControlsFocus>()
        .init_resource::<profile_state::LaneProfileDraft>()
        .init_resource::<profile_state::CustomizeSession>()
        .init_resource::<profile_state::PendingCloseState>()
        .init_resource::<profile_dialog::ProfileDialogState>()
        .add_systems(
            Update,
            (sync_drafts_to_session, resolve_pending_close)
                .chain()
                .run_if(in_state(AppState::Performance)),
        )
        .add_systems(
            Update,
            (
                update_preview_state,
                clear_canvas_interaction_outside_widgets.run_if(editor_open),
            )
                .before(EditorPickSet)
                .run_if(in_state(AppState::Performance)),
        )
        .add_systems(OnExit(AppState::Performance), close_editor_on_exit)
        .configure_sets(Update, (EditorPickSet, EditorGestureSet).chain())
        .add_plugins((
            (
                bindings_panel::plugin,
                bindings_capture::plugin,
                bindings_spatial::plugin,
                drag::plugin,
            ),
            hotkeys::plugin,
            keyboard_nav::plugin,
            undo::plugin,
            save::plugin,
            ui::plugin,
            picking::plugin,
            selection_box::plugin,
            panel::plugin,
            snap::plugin,
            stage::plugin,
            session::plugin,
            tabs::plugin,
            footer::plugin,
            calibration::plugin,
        ));
}

/// Leaving Performance with the editor still open (e.g. the song ended mid-edit)
/// must restore autoplay and clear `EditorOpen`, else the next song starts with
/// drum input + pause dead and no sidebar (the sidebar despawn is in ui.rs).
fn close_editor_on_exit(
    mut open: ResMut<EditorOpen>,
    prev: Res<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut gesture: ResMut<drag::ActiveGesture>,
    mut hovered: ResMut<picking::Hovered>,
    mut selection: ResMut<drag::Selection>,
    mut session: ResMut<game_shell::EditorSession>,
    layouts: Res<crate::widget_layout::WidgetLayouts>,
    profile_session: Res<profile_state::CustomizeSession>,
    draft: Res<tabs::ConfigDraft>,
    mut perf_draft: ResMut<crate::perf_hotkeys::PerfHotkeyDraft>,
    show_perf_info: Res<crate::resources::ShowPerfInfo>,
) {
    if open.0 {
        // Config and widget layout keep their auto-save policy. Profile
        // drafts are NOT saved here: committed profile state changes only
        // through explicit registry transactions (dirty close guard). The
        // lane snapshot is the last committed profile, not the preview.
        let file = save::layout_file_from(&layouts, &profile_session.0.lanes.saved.arrangement);
        if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
            warn!("layout save on exit failed: {e}");
        }
        if let Err(e) = dtx_config::save(&dtx_config::default_path(), &draft.0) {
            warn!("config save on exit failed: {e}");
        }
        perf_draft.sync_from_editor(&draft.0, show_perf_info.0);
        autoplay.0 = prev.0;
        open.0 = false;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
    selection.0 = None;
    // Covers non-Esc exits (song ended, forced transition): a stale session
    // flag would make the next Performance force-open the editor.
    session.0 = false;
}

/// Mirror the transitional edit surfaces into the profile session so dirty
/// tracking sees every edit: LiveBindings splits into the keyboard/MIDI
/// draft values; the lane draft copies over wholesale.
fn sync_drafts_to_session(
    live: Res<crate::bindings::LiveBindings>,
    lane_draft: Res<profile_state::LaneProfileDraft>,
    mut session: ResMut<profile_state::CustomizeSession>,
) {
    if live.is_changed() {
        let (keyboard, midi) = dtx_input::profiles::split_bindings(&live.0);
        if session.0.keyboard.value != keyboard {
            session.0.keyboard.value = keyboard;
        }
        if session.0.midi.value != midi {
            session.0.midi.value = midi;
        }
    }
    if lane_draft.is_changed() && session.0.lanes != lane_draft.0 {
        session.0.lanes = lane_draft.0.clone();
    }
}

/// Write one dirty profile kind to its registry: load the canonical file,
/// apply Save (or an auto-named Save As when a built-in is selected), and
/// persist atomically. Returns success per kind for the close guard.
fn save_dirty_kind(
    kind: profile_state::ProfileKind,
    session: &profile_state::ProfileSession,
) -> bool {
    use dtx_input::profiles as cfg;
    use dtx_persistence::suggest_copy_name;
    use profile_state::ProfileKind;

    fn commit<T: Clone + PartialEq>(
        registry_startup: cfg::RegistryStartup<cfg::ProfileRegistry<T>>,
        builtins: &std::collections::BTreeMap<String, T>,
        selected: &str,
        value: T,
        save: impl FnOnce(&cfg::ProfileRegistry<T>) -> Result<(), cfg::RegistryIoError>,
    ) -> bool {
        let registry = match registry_startup {
            cfg::RegistryStartup::Ready(r)
            | cfg::RegistryStartup::LegacySession { registry: r, .. } => r,
            cfg::RegistryStartup::ReadOnlyBuiltins(error) => {
                warn!("profile save skipped, registry unusable: {error}");
                return false;
            }
        };
        let action = if builtins.contains_key(selected) {
            let names: Vec<&str> = builtins
                .keys()
                .map(String::as_str)
                .chain(registry.profiles.keys().map(String::as_str))
                .collect();
            let name = suggest_copy_name(selected, names.iter().copied());
            cfg::RegistryAction::SaveAs {
                name: match dtx_persistence::validate_profile_name(
                    &name,
                    builtins.keys().map(String::as_str),
                    registry.profiles.keys().map(String::as_str),
                    None,
                ) {
                    Ok(name) => name,
                    Err(error) => {
                        warn!("profile save-as name invalid: {error}");
                        return false;
                    }
                },
                value,
            }
        } else {
            cfg::RegistryAction::Save(value)
        };
        let next = match cfg::reduce_registry(&registry, builtins, action) {
            Ok(next) => next,
            Err(error) => {
                warn!("profile save rejected: {error}");
                return false;
            }
        };
        match save(&next) {
            Ok(()) => true,
            Err(error) => {
                warn!("profile save failed: {error}");
                false
            }
        }
    }

    let legacy = dtx_input::default_bindings_path();
    match kind {
        ProfileKind::Keyboard => commit(
            cfg::load_keyboard_registry(&crate::bindings::keyboard_registry_path(), &legacy),
            &cfg::keyboard_builtins(),
            &session.keyboard.selected,
            session.keyboard.value.clone(),
            |next| cfg::save_keyboard_registry(&crate::bindings::keyboard_registry_path(), next),
        ),
        ProfileKind::Midi => commit(
            cfg::load_midi_registry(&crate::bindings::midi_registry_path(), &legacy),
            &cfg::midi_builtins(),
            &session.midi.selected,
            session.midi.value.clone(),
            |next| cfg::save_midi_registry(&crate::bindings::midi_registry_path(), next),
        ),
        ProfileKind::Lanes => {
            use dtx_layout::profiles as lp;
            let path = crate::lanes::lane_registry_path();
            let layout = dtx_layout::default_path();
            let mut registry = match lp::load_lane_registry(&path, &layout) {
                lp::LaneRegistryStartup::Ready(r)
                | lp::LaneRegistryStartup::LegacySession { registry: r, .. } => r,
                lp::LaneRegistryStartup::ReadOnlyBuiltins(error) => {
                    warn!("lane profile save skipped, registry unusable: {error}");
                    return false;
                }
            };
            let builtins = lp::lane_builtins();
            let name = if builtins.contains_key(&session.lanes.selected) {
                let names: Vec<&str> = builtins
                    .keys()
                    .map(String::as_str)
                    .chain(registry.profiles.keys().map(String::as_str))
                    .collect();
                suggest_copy_name(&session.lanes.selected, names.iter().copied())
            } else {
                session.lanes.selected.clone()
            };
            registry
                .profiles
                .insert(name.clone(), session.lanes.value.clone());
            registry.active = name;
            match lp::save_lane_registry(&path, &registry) {
                Ok(()) => true,
                Err(error) => {
                    warn!("lane profile save failed: {error}");
                    false
                }
            }
        }
    }
}

/// Resolve the dirty-close guard: Enter saves everything, Escape cancels;
/// Discard requires an explicit dialog action (handled by dialog UI). Close
/// finalizes only after a discard or once every dirty save succeeded.
fn resolve_pending_close(
    keys: Res<ButtonInput<KeyCode>>,
    mut pending: ResMut<profile_state::PendingCloseState>,
    mut session: ResMut<profile_state::CustomizeSession>,
    mut open: ResMut<EditorOpen>,
    prev: Res<PrevAutoplay>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
    mut editor_session: ResMut<game_shell::EditorSession>,
    mut requests: MessageWriter<game_shell::TransitionRequest>,
) {
    // Skip the frame the guard was armed (or updated): the Esc/Enter press
    // that raised it must not immediately resolve it.
    if pending.is_changed() {
        return;
    }
    let profile_state::PendingCloseState::Pending(close) = pending.clone() else {
        return;
    };
    let Some(decision) = profile_state::close_decision_for_key(
        keys.just_pressed(KeyCode::Enter),
        keys.just_pressed(KeyCode::Escape),
    ) else {
        return;
    };
    let save_results: Vec<_> = if decision == profile_state::CloseDecision::SaveAll {
        close
            .dirty
            .iter()
            .map(|kind| (*kind, save_dirty_kind(*kind, &session.0)))
            .collect()
    } else {
        Vec::new()
    };
    match profile_state::reduce_close_decision(&close, decision, &mut session.0, &save_results) {
        profile_state::CloseOutcome::Cancelled => {
            *pending = profile_state::PendingCloseState::None;
        }
        profile_state::CloseOutcome::Close(_) => {
            *pending = profile_state::PendingCloseState::None;
            open.0 = false;
            autoplay.0 = prev.0;
            if editor_session.0 {
                editor_session.0 = false;
                game_shell::request_transition(&mut requests, game_shell::AppState::Title);
            }
        }
        profile_state::CloseOutcome::StayOpen { failed } => {
            *pending = profile_state::PendingCloseState::Pending(profile_state::PendingClose {
                intent: close.intent,
                dirty: failed,
            });
        }
    }
}

fn clear_canvas_interaction_outside_widgets(
    active: Res<tabs::ActiveTab>,
    mut gesture: ResMut<drag::ActiveGesture>,
    mut hovered: ResMut<picking::Hovered>,
) {
    if active.0 == game_shell::CustomizeTab::Widgets {
        return;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
}

pub(super) fn just_closed(open: bool, was_open: &mut bool) -> bool {
    let closed = *was_open && !open;
    *was_open = open;
    closed
}

pub(super) fn should_persist_close(open: bool, in_performance: bool, was_open: &mut bool) -> bool {
    just_closed(open, was_open) && in_performance
}

/// Run condition: editor is open.
pub fn editor_open(open: Res<EditorOpen>) -> bool {
    open.0
}

/// Run condition: the Widgets layout tab is active.
pub fn widgets_tab_active(active: Res<tabs::ActiveTab>) -> bool {
    active.0 == game_shell::CustomizeTab::Widgets
}

/// Run condition: editor is closed (for gating gameplay systems).
pub fn editor_closed(open: Res<EditorOpen>) -> bool {
    !open.0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_open_default_false() {
        assert!(!EditorOpen::default().0);
    }

    #[test]
    fn initial_closed_state_is_not_a_close_transition() {
        let mut was_open = false;
        assert!(!just_closed(false, &mut was_open));
    }

    #[test]
    fn open_to_closed_is_a_close_transition() {
        let mut was_open = false;
        assert!(!just_closed(true, &mut was_open));
        assert!(just_closed(false, &mut was_open));
        assert!(!just_closed(false, &mut was_open));
    }

    #[test]
    fn forced_exit_consumes_close_outside_performance() {
        let mut was_open = false;
        assert!(!should_persist_close(true, true, &mut was_open));
        assert!(!should_persist_close(false, false, &mut was_open));
        assert!(!should_persist_close(false, true, &mut was_open));
    }
}
