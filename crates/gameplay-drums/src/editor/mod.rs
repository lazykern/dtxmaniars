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
pub mod capture_modal;
pub mod chrome;
pub mod close_dialog;
pub mod controls_panel;
pub mod drag;
pub mod footer;
pub mod hotkeys;
pub mod keyboard_nav;
pub mod lane_drag;
pub mod lanes_panel;
pub mod panel;
pub mod panel_kit;
pub mod picking;
pub mod profile_bar;
pub mod profile_bar_ui;
pub mod profile_dialog;
pub mod profile_dialog_ui;
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
                capture_modal::plugin,
                drag::plugin,
                close_dialog::plugin,
                profile_bar_ui::plugin,
                profile_dialog_ui::plugin,
                lanes_panel::plugin,
                lane_drag::plugin,
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
    mut capture: ResMut<bindings_capture::CaptureState>,
    mut mouse_arrived: ResMut<bindings_capture::MouseArrivedInput>,
    time: Res<Time>,
    mut save_err: ResMut<footer::EditorSaveError>,
) {
    if open.0 {
        // Config and widget layout keep their auto-save policy. Profile
        // drafts are NOT saved here: committed profile state changes only
        // through explicit registry transactions (dirty close guard). The
        // lane snapshot is the last committed profile, not the preview.
        let file = save::layout_file_from(&layouts, &profile_session.0.lanes.saved.arrangement);
        if let Err(e) = dtx_layout::save(&dtx_layout::default_path(), &file) {
            warn!("layout save on exit failed: {e}");
            save_err.set(time.elapsed_secs_f64(), format!("save failed: {e}"));
        }
        if let Err(e) = dtx_config::save(&dtx_config::default_path(), &draft.0) {
            warn!("config save on exit failed: {e}");
            save_err.set(time.elapsed_secs_f64(), format!("save failed: {e}"));
        }
        perf_draft.sync_from_editor(&draft.0, show_perf_info.0);
        autoplay.0 = prev.0;
        open.0 = false;
    }
    gesture.0 = drag::Gesture::None;
    hovered.0 = None;
    selection.0 = None;
    // A capture armed at exit (song ended mid-KeyArrived/MidiArrived) would
    // otherwise survive into the next Performance: the modal despawns on
    // OnExit but `capture_binding` keeps running against the stale Arrived
    // state, and the modal's PartialEq rebuild gate holds the SAME lines so
    // it never respawns — a stray Enter/hit would silently commit an unseen
    // binding. Reset both the state and its mouse inlet here.
    *capture = bindings_capture::CaptureState::Idle;
    mouse_arrived.0 = None;
    // Covers non-Esc exits (song ended, forced transition): a stale session
    // flag would make the next Performance force-open the editor.
    session.0 = false;
}

/// Mirror the transitional edit surfaces into the profile session so dirty
/// tracking sees every edit: LiveBindings splits into the keyboard/MIDI
/// draft values; the lane draft copies over wholesale.
fn sync_drafts_to_session(
    open: Res<EditorOpen>,
    live: Res<crate::bindings::LiveBindings>,
    lane_draft: Res<profile_state::LaneProfileDraft>,
    mut session: ResMut<profile_state::CustomizeSession>,
    mut was_open: Local<bool>,
) {
    // Seed loaded profiles as the clean baseline once per Customize session.
    // Later LiveBindings changes remain dirty edits.
    let just_opened = open.0 && !*was_open;
    *was_open = open.0;
    if just_opened {
        let (keyboard, midi) = dtx_input::profiles::split_bindings(&live.0);
        session.0.keyboard =
            profile_state::ProfileDraft::clean(&session.0.keyboard.selected, keyboard);
        session.0.midi = profile_state::ProfileDraft::clean(&session.0.midi.selected, midi);
    } else if live.is_changed() {
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

/// Load, apply one or more registry actions in sequence, and write once.
/// Shared by every profile-bar action (Select/SaveAs/Rename/Delete and the
/// dirty-guard's combined save+select) that needs a keyboard/MIDI-shaped
/// registry transaction.
pub(super) fn commit_registry_actions<T: Clone + PartialEq>(
    registry_startup: dtx_input::profiles::RegistryStartup<dtx_input::profiles::ProfileRegistry<T>>,
    builtins: &std::collections::BTreeMap<String, T>,
    actions: Vec<dtx_input::profiles::RegistryAction<T>>,
    save: impl FnOnce(&dtx_input::profiles::ProfileRegistry<T>) -> Result<(), dtx_input::profiles::RegistryIoError>,
) -> Result<dtx_input::profiles::ProfileRegistry<T>, String> {
    use dtx_input::profiles::RegistryStartup;
    let mut registry = match registry_startup {
        RegistryStartup::Ready(r) | RegistryStartup::LegacySession { registry: r, .. } => r,
        RegistryStartup::ReadOnlyBuiltins(error) => {
            return Err(format!("registry unusable: {error}"));
        }
    };
    for action in actions {
        registry = dtx_input::profiles::reduce_registry(&registry, builtins, action)
            .map_err(|error| error.to_string())?;
    }
    save(&registry).map_err(|error| error.to_string())?;
    Ok(registry)
}

pub(super) fn load_keyboard_startup(
) -> dtx_input::profiles::RegistryStartup<dtx_input::profiles::ProfileRegistry<dtx_input::profiles::KeyboardProfile>>
{
    dtx_input::profiles::load_keyboard_registry(
        &crate::bindings::keyboard_registry_path(),
        &dtx_input::default_bindings_path(),
    )
}

pub(super) fn load_midi_startup(
) -> dtx_input::profiles::RegistryStartup<dtx_input::profiles::ProfileRegistry<dtx_input::profiles::MidiProfile>>
{
    dtx_input::profiles::load_midi_registry(
        &crate::bindings::midi_registry_path(),
        &dtx_input::default_bindings_path(),
    )
}

pub(super) fn load_lane_startup() -> dtx_layout::profiles::LaneRegistryStartup {
    dtx_layout::profiles::load_lane_registry(&crate::lanes::lane_registry_path(), &dtx_layout::default_path())
}

/// The lane registry has no generic `RegistryAction`/`reduce_registry` (its
/// registry type isn't the generic `ProfileRegistry<T>`), so the profile bar
/// gets its own small mirror of the keyboard/MIDI action set.
pub(super) enum LaneRegAction {
    Select(String),
    Save(dtx_layout::profiles::LaneProfile),
    SaveAs {
        name: String,
        value: dtx_layout::profiles::LaneProfile,
    },
    Rename(String),
    Delete,
}

fn reduce_lane_registry(
    registry: &dtx_layout::profiles::LaneProfileRegistry,
    builtins: &std::collections::BTreeMap<String, dtx_layout::profiles::LaneProfile>,
    action: LaneRegAction,
) -> Result<dtx_layout::profiles::LaneProfileRegistry, String> {
    let mut updated = registry.clone();
    match action {
        LaneRegAction::Select(name) => {
            if !builtins.contains_key(&name) && !updated.profiles.contains_key(&name) {
                return Err(format!("profile not found: {name}"));
            }
            updated.active = name;
        }
        LaneRegAction::Save(value) => {
            if builtins.contains_key(&updated.active) {
                return Err(format!(
                    "built-in profile cannot be modified: {}",
                    updated.active
                ));
            }
            let Some(slot) = updated.profiles.get_mut(&updated.active) else {
                return Err(format!("profile not found: {}", updated.active));
            };
            *slot = value;
        }
        LaneRegAction::SaveAs { name, value } => {
            if builtins.contains_key(&name) {
                return Err(format!("built-in profile cannot be modified: {name}"));
            }
            if updated.profiles.contains_key(&name) {
                return Err(format!("profile already exists: {name}"));
            }
            updated.profiles.insert(name.clone(), value);
            updated.active = name;
        }
        LaneRegAction::Rename(name) => {
            if builtins.contains_key(&updated.active) {
                return Err(format!(
                    "built-in profile cannot be modified: {}",
                    updated.active
                ));
            }
            let old = updated.active.clone();
            let Some(value) = updated.profiles.remove(&old) else {
                return Err(format!("profile not found: {old}"));
            };
            if builtins.contains_key(&name) {
                return Err(format!("built-in profile cannot be modified: {name}"));
            }
            if updated.profiles.contains_key(&name) {
                return Err(format!("profile already exists: {name}"));
            }
            updated.profiles.insert(name.clone(), value);
            updated.active = name;
        }
        LaneRegAction::Delete => {
            if builtins.contains_key(&updated.active) {
                return Err(format!(
                    "built-in profile cannot be modified: {}",
                    updated.active
                ));
            }
            let active = updated.active.clone();
            if updated.profiles.remove(&active).is_none() {
                return Err(format!("profile not found: {active}"));
            }
            // Fall back to the named default, not whatever BTreeMap ordering
            // happens to surface first.
            updated.active = dtx_layout::profiles::LANE_DEFAULT_NAME.to_owned();
        }
    }
    Ok(updated)
}

/// Persist the lane registry to disk. Split out so `commit_lane_actions`
/// takes a save closure (like `commit_registry_actions`) and stays unit-
/// testable without touching the real config path.
pub(super) fn save_lane_to_disk(
    registry: &dtx_layout::profiles::LaneProfileRegistry,
) -> Result<(), dtx_layout::profiles::LaneRegistryError> {
    dtx_layout::profiles::save_lane_registry(&crate::lanes::lane_registry_path(), registry)
}

pub(super) fn commit_lane_actions(
    registry_startup: dtx_layout::profiles::LaneRegistryStartup,
    actions: Vec<LaneRegAction>,
    save: impl FnOnce(&dtx_layout::profiles::LaneProfileRegistry) -> Result<(), dtx_layout::profiles::LaneRegistryError>,
) -> Result<dtx_layout::profiles::LaneProfileRegistry, String> {
    use dtx_layout::profiles::LaneRegistryStartup;
    let mut registry = match registry_startup {
        LaneRegistryStartup::Ready(r) | LaneRegistryStartup::LegacySession { registry: r, .. } => r,
        LaneRegistryStartup::ReadOnlyBuiltins(error) => {
            return Err(format!("registry unusable: {error}"));
        }
    };
    let builtins = dtx_layout::profiles::lane_builtins();
    for action in actions {
        registry = reduce_lane_registry(&registry, &builtins, action)?;
    }
    save(&registry).map_err(|error| error.to_string())?;
    Ok(registry)
}

/// Write one dirty profile kind to its registry: load the canonical file,
/// apply Save (or an auto-named Save As when a built-in is selected), and
/// persist atomically. Shared by the close guard's Save All and the profile
/// bar's Save button — the only two places a draft's *own* value gets
/// written back under its own name.
fn save_dirty_kind(
    kind: profile_state::ProfileKind,
    session: &profile_state::ProfileSession,
) -> Result<(), String> {
    use dtx_input::profiles as cfg;
    use dtx_persistence::suggest_copy_name;
    use profile_state::ProfileKind;

    fn commit<T: Clone + PartialEq>(
        registry_startup: cfg::RegistryStartup<cfg::ProfileRegistry<T>>,
        builtins: &std::collections::BTreeMap<String, T>,
        selected: &str,
        value: T,
        save: impl FnOnce(&cfg::ProfileRegistry<T>) -> Result<(), cfg::RegistryIoError>,
    ) -> Result<(), String> {
        let registry = match registry_startup {
            cfg::RegistryStartup::Ready(r)
            | cfg::RegistryStartup::LegacySession { registry: r, .. } => r,
            cfg::RegistryStartup::ReadOnlyBuiltins(error) => {
                return Err(format!("registry unusable: {error}"));
            }
        };
        let action = if builtins.contains_key(selected) {
            let names: Vec<&str> = builtins
                .keys()
                .map(String::as_str)
                .chain(registry.profiles.keys().map(String::as_str))
                .collect();
            let name = suggest_copy_name(selected, names.iter().copied());
            let name = dtx_persistence::validate_profile_name(
                &name,
                builtins.keys().map(String::as_str),
                registry.profiles.keys().map(String::as_str),
                None,
            )
            .map_err(|error| format!("save-as name invalid: {error}"))?;
            cfg::RegistryAction::SaveAs { name, value }
        } else {
            cfg::RegistryAction::Save(value)
        };
        let next =
            cfg::reduce_registry(&registry, builtins, action).map_err(|error| error.to_string())?;
        save(&next).map_err(|error| error.to_string())
    }

    match kind {
        ProfileKind::Keyboard => commit(
            load_keyboard_startup(),
            &cfg::keyboard_builtins(),
            &session.keyboard.selected,
            session.keyboard.value.clone(),
            |next| cfg::save_keyboard_registry(&crate::bindings::keyboard_registry_path(), next),
        ),
        ProfileKind::Midi => commit(
            load_midi_startup(),
            &cfg::midi_builtins(),
            &session.midi.selected,
            session.midi.value.clone(),
            |next| cfg::save_midi_registry(&crate::bindings::midi_registry_path(), next),
        ),
        ProfileKind::Lanes => {
            use dtx_layout::profiles as lp;
            let builtins = lp::lane_builtins();
            let startup = load_lane_startup();
            let action = if builtins.contains_key(&session.lanes.selected) {
                let registry = match &startup {
                    lp::LaneRegistryStartup::Ready(r)
                    | lp::LaneRegistryStartup::LegacySession { registry: r, .. } => r.clone(),
                    lp::LaneRegistryStartup::ReadOnlyBuiltins(error) => {
                        return Err(format!("registry unusable: {error}"));
                    }
                };
                let names: Vec<&str> = builtins
                    .keys()
                    .map(String::as_str)
                    .chain(registry.profiles.keys().map(String::as_str))
                    .collect();
                let name = suggest_copy_name(&session.lanes.selected, names.iter().copied());
                LaneRegAction::SaveAs {
                    name,
                    value: session.lanes.value.clone(),
                }
            } else {
                LaneRegAction::Save(session.lanes.value.clone())
            };
            commit_lane_actions(startup, vec![action], save_lane_to_disk).map(|_| ())
        }
    }
}

/// Resolve the dirty-close guard: Enter saves everything, Escape cancels;
/// Discard requires an explicit dialog action (handled by dialog UI). Close
/// finalizes only after a discard or once every dirty save succeeded.
fn resolve_pending_close(
    keys: Res<ButtonInput<KeyCode>>,
    mut requested: MessageReader<close_dialog::CloseDecisionRequest>,
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
    let decision = requested
        .read()
        .map(|request| request.0)
        .last()
        .or_else(|| {
            profile_state::close_decision_for_key(
                keys.just_pressed(KeyCode::Enter),
                keys.just_pressed(KeyCode::Escape),
            )
        });
    let Some(decision) = decision else {
        return;
    };
    let save_results: Vec<_> = if decision == profile_state::CloseDecision::SaveAll {
        close
            .dirty
            .iter()
            .map(|kind| (*kind, save_dirty_kind(*kind, &session.0).is_ok()))
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

/// Run condition: the Lanes tab is active.
pub fn lanes_tab_active(active: Res<tabs::ActiveTab>) -> bool {
    active.0 == game_shell::CustomizeTab::Lanes
}

/// Run condition: editor is closed (for gating gameplay systems).
pub fn editor_closed(open: Res<EditorOpen>) -> bool {
    !open.0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Profile registry engine (pure reducers + closure-injected I/O) =====

    fn lane_user_registry(active: &str, users: &[(&str, dtx_layout::LaneArrangement)]) -> dtx_layout::profiles::LaneProfileRegistry {
        let mut registry = dtx_layout::profiles::lane_registry();
        registry.active = active.to_owned();
        for (name, arrangement) in users {
            registry.profiles.insert(
                (*name).to_owned(),
                dtx_layout::profiles::LaneProfile::from_arrangement(arrangement.clone()),
            );
        }
        registry
    }

    #[test]
    fn reduce_lane_save_rejects_builtin_in_place() {
        // Classic is a built-in: an in-place Save must error (forces Save As).
        let builtins = dtx_layout::profiles::lane_builtins();
        let registry = dtx_layout::profiles::lane_registry(); // active == "Classic"
        let err = reduce_lane_registry(
            &registry,
            &builtins,
            LaneRegAction::Save(dtx_layout::profiles::LaneProfile::from_arrangement(dtx_layout::nx_type_b())),
        )
        .expect_err("built-in cannot be saved in place");
        assert!(err.contains("built-in"), "{err}");
    }

    #[test]
    fn reduce_lane_rename_rejects_existing_name() {
        // Renaming "Desk" onto the existing user profile "Studio" collides.
        let builtins = dtx_layout::profiles::lane_builtins();
        let registry = lane_user_registry(
            "Desk",
            &[("Desk", dtx_layout::classic()), ("Studio", dtx_layout::nx_type_b())],
        );
        let err = reduce_lane_registry(&registry, &builtins, LaneRegAction::Rename("Studio".to_owned()))
            .expect_err("rename onto existing name collides");
        assert!(err.contains("already exists"), "{err}");
    }

    #[test]
    fn reduce_lane_delete_falls_back_to_named_default() {
        // Deleting the active user profile drops it and selects the named
        // default, not whatever BTreeMap ordering happens to surface first.
        let builtins = dtx_layout::profiles::lane_builtins();
        let registry = lane_user_registry("Desk", &[("Desk", dtx_layout::nx_type_b())]);
        let next = reduce_lane_registry(&registry, &builtins, LaneRegAction::Delete).expect("delete succeeds");
        assert!(!next.profiles.contains_key("Desk"), "deleted profile is gone");
        assert_eq!(next.active, dtx_layout::profiles::LANE_DEFAULT_NAME);
    }

    #[test]
    fn commit_registry_actions_never_writes_on_reducer_failure() {
        // A Rename onto an existing name fails mid-sequence: the save closure
        // must never fire and the whole commit errors (no partial write).
        use dtx_input::profiles as cfg;
        let mut registry = cfg::keyboard_registry();
        registry.active = "Desk".to_owned();
        registry.profiles.insert("Desk".to_owned(), cfg::KeyboardProfile::default());
        registry.profiles.insert("Studio".to_owned(), cfg::KeyboardProfile::default());
        let name = dtx_persistence::validate_profile_name("Studio", [cfg::KEYBOARD_DEFAULT_NAME], [], None)
            .expect("valid name");
        let mut saved = false;
        let result = commit_registry_actions(
            cfg::RegistryStartup::Ready(registry),
            &cfg::keyboard_builtins(),
            vec![cfg::RegistryAction::Rename(name)],
            |_| {
                saved = true;
                Ok(())
            },
        );
        assert!(result.is_err(), "rename-to-existing must fail the commit");
        assert!(!saved, "save closure must never run when the reducer fails");
    }

    #[test]
    fn commit_registry_actions_propagates_save_failure() {
        use dtx_input::profiles as cfg;
        let name = dtx_persistence::validate_profile_name("Desk", [cfg::KEYBOARD_DEFAULT_NAME], [], None)
            .expect("valid name");
        let err = commit_registry_actions(
            cfg::RegistryStartup::Ready(cfg::keyboard_registry()),
            &cfg::keyboard_builtins(),
            vec![cfg::RegistryAction::SaveAs {
                name,
                value: cfg::KeyboardProfile::default(),
            }],
            |_| Err(cfg::RegistryIoError::ConfirmationRequired { path: "disk".into() }),
        )
        .expect_err("save failure propagates");
        assert!(err.contains("disk") || err.contains("confirmation"), "{err}");
    }

    #[test]
    fn commit_lane_actions_never_writes_on_reducer_failure() {
        // Delete on a built-in (Classic) fails: save closure never fires.
        let mut saved = false;
        let result = commit_lane_actions(
            dtx_layout::profiles::LaneRegistryStartup::Ready(dtx_layout::profiles::lane_registry()),
            vec![LaneRegAction::Delete],
            |_| {
                saved = true;
                Ok(())
            },
        );
        assert!(result.is_err(), "deleting a built-in must fail the commit");
        assert!(!saved, "save closure must never run when the reducer fails");
    }

    #[test]
    fn commit_lane_actions_propagates_save_failure() {
        let err = commit_lane_actions(
            dtx_layout::profiles::LaneRegistryStartup::Ready(dtx_layout::profiles::lane_registry()),
            vec![LaneRegAction::SaveAs {
                name: "Desk".to_owned(),
                value: dtx_layout::profiles::LaneProfile::from_arrangement(dtx_layout::nx_type_b()),
            }],
            |_| Err(dtx_layout::profiles::LaneRegistryError::ConfirmationRequired { path: "disk".into() }),
        )
        .expect_err("save failure propagates");
        assert!(err.contains("disk") || err.contains("confirmation"), "{err}");
    }

    #[test]
    fn reduce_lane_saveas_inserts_new_user_profile() {
        let builtins = dtx_layout::profiles::lane_builtins();
        let registry = dtx_layout::profiles::lane_registry(); // active == "Classic"
        let next = reduce_lane_registry(
            &registry,
            &builtins,
            LaneRegAction::SaveAs {
                name: "Desk".to_owned(),
                value: dtx_layout::profiles::LaneProfile::from_arrangement(dtx_layout::nx_type_b()),
            },
        )
        .expect("save as succeeds");
        assert_eq!(next.active, "Desk", "save as selects the new profile");
        assert_eq!(next.profiles["Desk"].arrangement, dtx_layout::nx_type_b());
    }

    #[test]
    fn commit_registry_actions_builds_next_registry_from_action() {
        // Happy path with an injected no-op save: SaveAs on the keyboard
        // registry inserts the profile and selects it.
        use dtx_input::profiles as cfg;
        let name = dtx_persistence::validate_profile_name("Desk", [cfg::KEYBOARD_DEFAULT_NAME], [], None)
            .expect("valid name");
        let registry = commit_registry_actions(
            cfg::RegistryStartup::Ready(cfg::keyboard_registry()),
            &cfg::keyboard_builtins(),
            vec![cfg::RegistryAction::SaveAs {
                name,
                value: cfg::KeyboardProfile::default(),
            }],
            |_| Ok(()), // no disk write in the test
        )
        .expect("commit succeeds");
        assert_eq!(registry.active, "Desk");
        assert!(registry.profiles.contains_key("Desk"));
    }

    #[test]
    fn commit_lane_actions_builds_next_registry_from_action() {
        // Happy path with an injected no-op save: SaveAs then the resulting
        // registry has the new profile active.
        let registry = commit_lane_actions(
            dtx_layout::profiles::LaneRegistryStartup::Ready(dtx_layout::profiles::lane_registry()),
            vec![LaneRegAction::SaveAs {
                name: "Desk".to_owned(),
                value: dtx_layout::profiles::LaneProfile::from_arrangement(dtx_layout::nx_type_d()),
            }],
            |_| Ok(()),
        )
        .expect("commit succeeds");
        assert_eq!(registry.active, "Desk");
        assert_eq!(registry.profiles["Desk"].arrangement, dtx_layout::nx_type_d());
    }

    #[test]
    fn editor_open_default_false() {
        assert!(!EditorOpen::default().0);
    }

    #[test]
    fn opening_customize_seeds_clean_binding_drafts() {
        let mut bindings = dtx_input::InputBindings::default();
        bindings.bind(
            dtx_core::EChannel::Snare,
            dtx_input::BindSource::Key(KeyCode::KeyQ),
        );
        let mut app = App::new();
        app.insert_resource(EditorOpen(true))
            .insert_resource(crate::bindings::LiveBindings(bindings))
            .init_resource::<profile_state::LaneProfileDraft>()
            .init_resource::<profile_state::CustomizeSession>()
            .add_systems(Update, sync_drafts_to_session);

        app.update();

        let session = &app.world().resource::<profile_state::CustomizeSession>().0;
        assert!(profile_state::dirty_profile_kinds(session).is_empty());
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
