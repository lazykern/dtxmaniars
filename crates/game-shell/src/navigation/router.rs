//! Centralized semantic input router: gates `SystemVerbHit`s by the context
//! stack, applies the MIDI grace/debounce guard, and translates verbs for
//! edit contexts before delivery as [`NavAction`] or [`LiveVerb`].

use std::time::Instant;

use bevy::prelude::*;

use super::{
    InputSource, LastIntentionalInputSource, MouseIntent, NavAction, NavContext, NavContextStack,
    NavGuard, SystemVerb,
};

/// A live-system verb accepted by the router. Consumers (pause/restart/
/// open-system-menu) gate themselves on their own state; the router only
/// filters exclusive-capture contexts.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveVerb(pub SystemVerb);

/// One hit's fate. Pure: all Bevy state is passed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routed {
    /// Deliver to the owning menu context.
    Menu(NavAction),
    /// Deliver to live-system consumers.
    Live(SystemVerb),
    /// No owner / gated out.
    Dropped,
}

/// Route one hit. Pure: all Bevy state is passed in. `guard.accept(now)` both
/// checks and records, so only MIDI-sourced menu verbs consult it — keyboard
/// hits bypass grace/debounce entirely (matching the pad mapper's policy).
pub fn route(
    top: Option<NavContext>,
    verb: SystemVerb,
    source: dtx_input::VerbSource,
    coarse: bool,
    guard: &mut NavGuard,
    now: Instant,
) -> Routed {
    use dtx_input::bindings::VerbScope;
    if verb.activation_scope() == VerbScope::LiveSystem {
        return match top {
            Some(ctx) if ctx.exclusive() => Routed::Dropped,
            _ => Routed::Live(verb),
        };
    }
    let Some(ctx) = top else {
        return Routed::Dropped;
    };
    if ctx.exclusive() || ctx == NavContext::LiveGameplay {
        return Routed::Dropped;
    }
    if source == dtx_input::VerbSource::Midi && !guard.accept(now) {
        return Routed::Dropped;
    }
    // Shift is a keyboard gesture: it never modifies MIDI-sourced verbs.
    let coarse = coarse && source == dtx_input::VerbSource::Keyboard;
    let verb = match (ctx.is_edit(), verb, coarse) {
        (true, SystemVerb::NavigateLeft, _) => SystemVerb::Decrease,
        (true, SystemVerb::NavigateRight, _) => SystemVerb::Increase,
        (_, SystemVerb::NextTab, true) => SystemVerb::PreviousTab,
        (_, v, _) => v,
    };
    Routed::Menu(NavAction {
        verb,
        source: match source {
            dtx_input::VerbSource::Keyboard => InputSource::Keyboard,
            dtx_input::VerbSource::Midi => InputSource::MidiKit,
        },
        coarse,
        repeated: false,
    })
}

/// Update-schedule set the router runs in. Context writers order themselves
/// `.before(NavRouterSet)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavRouterSet;

pub(super) fn route_verbs(
    stack: Res<NavContextStack>,
    keys: Res<ButtonInput<KeyCode>>,
    mut guard: ResMut<NavGuard>,
    mut hits: MessageReader<dtx_input::SystemVerbHit>,
    mut mouse: MessageReader<MouseIntent>,
    mut last_source: ResMut<LastIntentionalInputSource>,
    mut menu_out: MessageWriter<NavAction>,
    mut live_out: MessageWriter<LiveVerb>,
) {
    use dtx_input::bindings::VerbScope;
    let now = Instant::now();
    guard.sync(stack.top(), now);
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for hit in hits.read() {
        // Transitional: the layout editor still runs its own
        // keyboard→NavAction emitters; skip keyboard menu delivery there
        // to avoid double-driving it until the editor migrates. MIDI menu
        // verbs DO route to the editor (they replaced the pad mapper).
        if hit.verb.activation_scope() != VerbScope::LiveSystem
            && hit.source == dtx_input::VerbSource::Keyboard
            && matches!(stack.top(), Some(NavContext::LayoutEditor))
        {
            continue;
        }
        match route(stack.top(), hit.verb, hit.source, coarse, &mut guard, now) {
            Routed::Menu(action) => {
                last_source.0 = action.source;
                menu_out.write(action);
            }
            Routed::Live(verb) => {
                last_source.0 = match hit.source {
                    dtx_input::VerbSource::Keyboard => InputSource::Keyboard,
                    dtx_input::VerbSource::Midi => InputSource::MidiKit,
                };
                live_out.write(LiveVerb(verb));
            }
            Routed::Dropped => {}
        }
    }
    if mouse.read().next().is_some() {
        last_source.0 = InputSource::Mouse;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use bevy::prelude::*;
    use dtx_input::VerbSource;

    use super::*;

    fn fresh_guard() -> NavGuard {
        NavGuard::default()
    }

    #[test]
    fn menu_verb_routes_only_when_a_menu_context_owns_input() {
        let now = Instant::now();
        let mut guard = fresh_guard();
        assert_eq!(
            route(
                Some(NavContext::LiveGameplay),
                SystemVerb::NavigateUp,
                VerbSource::Midi,
                false,
                &mut guard,
                now,
            ),
            Routed::Dropped
        );
        assert_eq!(
            route(
                Some(NavContext::LiveGameplay),
                SystemVerb::Pause,
                VerbSource::Midi,
                false,
                &mut guard,
                now,
            ),
            Routed::Live(SystemVerb::Pause)
        );
        assert_eq!(
            route(
                None,
                SystemVerb::Confirm,
                VerbSource::Keyboard,
                false,
                &mut guard,
                now,
            ),
            Routed::Dropped
        );
    }

    #[test]
    fn edit_context_translates_horizontal_navigation_to_adjustment() {
        let now = Instant::now();
        let mut guard = fresh_guard();
        assert_eq!(
            route(
                Some(NavContext::SongReadyEdit),
                SystemVerb::NavigateLeft,
                VerbSource::Keyboard,
                false,
                &mut guard,
                now,
            ),
            Routed::Menu(NavAction {
                verb: SystemVerb::Decrease,
                source: InputSource::Keyboard,
                coarse: false,
                repeated: false,
            })
        );
    }

    #[test]
    fn midi_hits_respect_grace_and_debounce_keyboard_does_not() {
        let t0 = Instant::now();
        let mut guard = fresh_guard();
        guard.sync(Some(NavContext::Home), t0);
        assert!(matches!(
            route(
                Some(NavContext::Home),
                SystemVerb::Confirm,
                VerbSource::Keyboard,
                false,
                &mut guard,
                t0,
            ),
            Routed::Menu(_)
        ));
        assert_eq!(
            route(
                Some(NavContext::Home),
                SystemVerb::Confirm,
                VerbSource::Midi,
                false,
                &mut guard,
                t0,
            ),
            Routed::Dropped,
            "inside the 500 ms entry grace"
        );
        let t1 = t0 + Duration::from_millis(600);
        assert!(matches!(
            route(
                Some(NavContext::Home),
                SystemVerb::Confirm,
                VerbSource::Midi,
                false,
                &mut guard,
                t1,
            ),
            Routed::Menu(_)
        ));
        assert_eq!(
            route(
                Some(NavContext::Home),
                SystemVerb::Confirm,
                VerbSource::Midi,
                false,
                &mut guard,
                t1 + Duration::from_millis(40),
            ),
            Routed::Dropped,
            "inside the 80 ms debounce"
        );
    }

    #[test]
    fn exclusive_context_swallows_everything() {
        let now = Instant::now();
        let mut guard = fresh_guard();
        assert_eq!(
            route(
                Some(NavContext::BindingCapture),
                SystemVerb::Back,
                VerbSource::Keyboard,
                false,
                &mut guard,
                now,
            ),
            Routed::Dropped
        );
        assert_eq!(
            route(
                Some(NavContext::BindingCapture),
                SystemVerb::Pause,
                VerbSource::Midi,
                false,
                &mut guard,
                now,
            ),
            Routed::Dropped
        );
    }

    #[test]
    fn shift_tab_becomes_previous_tab() {
        let now = Instant::now();
        let mut guard = fresh_guard();
        assert_eq!(
            route(
                Some(NavContext::SettingsTabs),
                SystemVerb::NextTab,
                VerbSource::Keyboard,
                true,
                &mut guard,
                now,
            ),
            Routed::Menu(NavAction {
                verb: SystemVerb::PreviousTab,
                source: InputSource::Keyboard,
                coarse: true,
                repeated: false,
            })
        );
    }

    /// Shift is keyboard-only: it must not flip a MIDI NextTab into
    /// PreviousTab nor stamp `coarse` on a MIDI-sourced action.
    #[test]
    fn shift_does_not_modify_midi_sourced_verbs() {
        let t0 = Instant::now();
        let mut guard = fresh_guard();
        guard.sync(Some(NavContext::SettingsTabs), t0);
        assert_eq!(
            route(
                Some(NavContext::SettingsTabs),
                SystemVerb::NextTab,
                VerbSource::Midi,
                true,
                &mut guard,
                t0 + Duration::from_millis(600),
            ),
            Routed::Menu(NavAction {
                verb: SystemVerb::NextTab,
                source: InputSource::MidiKit,
                coarse: false,
                repeated: false,
            })
        );
    }

    fn router_app() -> App {
        let mut app = App::new();
        app.add_message::<dtx_input::SystemVerbHit>()
            .add_message::<NavAction>()
            .add_message::<LiveVerb>()
            .add_message::<MouseIntent>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<NavGuard>()
            .init_resource::<NavContextStack>()
            .insert_resource(LastIntentionalInputSource(InputSource::MidiKit))
            .add_systems(Update, route_verbs);
        app
    }

    fn menu_actions(app: &App) -> Vec<NavAction> {
        app.world()
            .resource::<Messages<NavAction>>()
            .iter_current_update_messages()
            .copied()
            .collect()
    }

    /// End-to-end: keyboard menu verbs are now routed. Confirm on Home
    /// produces a NavAction without touching the guard.
    #[test]
    fn route_verbs_system_delivers_keyboard_menu_actions() {
        let mut app = router_app();
        app.world_mut()
            .resource_mut::<NavContextStack>()
            .push(NavContext::Home);
        app.world_mut().write_message(dtx_input::SystemVerbHit {
            verb: SystemVerb::Confirm,
            source: VerbSource::Keyboard,
        });
        app.update();
        assert_eq!(
            menu_actions(&app),
            vec![NavAction {
                verb: SystemVerb::Confirm,
                source: InputSource::Keyboard,
                coarse: false,
                repeated: false,
            }]
        );
        assert_eq!(
            app.world().resource::<LastIntentionalInputSource>().0,
            InputSource::Keyboard
        );
    }

    /// End-to-end MIDI menu path: a hit inside the 500 ms entry grace is
    /// dropped; after the grace it is accepted exactly once; a second hit
    /// inside the 80 ms debounce is dropped.
    #[test]
    fn route_verbs_system_applies_grace_then_debounce_to_midi_menu_verbs() {
        let mut app = router_app();
        app.world_mut()
            .resource_mut::<NavContextStack>()
            .push(NavContext::Home);
        let hit = dtx_input::SystemVerbHit {
            verb: SystemVerb::NavigateUp,
            source: VerbSource::Midi,
        };
        app.world_mut().write_message(hit);
        app.update();
        assert_eq!(menu_actions(&app), vec![], "inside the entry grace");

        app.world_mut()
            .resource_mut::<NavGuard>()
            .force_ready(NavContext::Home, Instant::now());
        app.world_mut().write_message(hit);
        app.update();
        assert_eq!(
            menu_actions(&app),
            vec![NavAction {
                verb: SystemVerb::NavigateUp,
                source: InputSource::MidiKit,
                coarse: false,
                repeated: false,
            }],
            "accepted once after the grace"
        );

        app.world_mut().write_message(hit);
        app.update();
        assert_eq!(menu_actions(&app), vec![], "inside the debounce");
    }

    /// MIDI menu verbs never reach live gameplay: pads are gameplay input.
    #[test]
    fn route_verbs_system_never_delivers_midi_menu_verbs_in_live_gameplay() {
        let mut app = router_app();
        app.world_mut()
            .resource_mut::<NavContextStack>()
            .push(NavContext::LiveGameplay);
        app.world_mut()
            .resource_mut::<NavGuard>()
            .force_ready(NavContext::LiveGameplay, Instant::now());
        app.world_mut().write_message(dtx_input::SystemVerbHit {
            verb: SystemVerb::NavigateUp,
            source: VerbSource::Midi,
        });
        app.update();
        assert_eq!(menu_actions(&app), vec![]);
    }

    /// Chain coverage by construction: the default profile binds the HH
    /// close note (42) to NavigateUp, so the pump's `SystemVerbHit` for it
    /// carries the verb the router just proved it delivers.
    #[test]
    fn default_profile_binds_hh_note_to_navigate_up() {
        let verbs: Vec<SystemVerb> = dtx_input::BindResolver::default()
            .system_for_note(42)
            .collect();
        assert!(verbs.contains(&SystemVerb::NavigateUp));
    }

    /// MIDI HT/LT arrive as NavigateLeft/Right; the layout editor's panels
    /// consume Decrease/Increase, so LayoutEditor is an edit context and the
    /// router translates (pad parity with the retired pad mapper).
    #[test]
    fn layout_editor_translates_midi_horizontal_navigation_to_adjustment() {
        let now = Instant::now();
        let mut guard = fresh_guard();
        guard.force_ready(NavContext::LayoutEditor, now);
        assert_eq!(
            route(
                Some(NavContext::LayoutEditor),
                SystemVerb::NavigateLeft,
                VerbSource::Midi,
                false,
                &mut guard,
                now,
            ),
            Routed::Menu(NavAction {
                verb: SystemVerb::Decrease,
                source: InputSource::MidiKit,
                coarse: false,
                repeated: false,
            })
        );
    }

    /// Transitional: the layout editor still emits its own keyboard
    /// NavActions, so the router skips keyboard menu delivery there.
    #[test]
    fn route_verbs_system_skips_keyboard_menu_delivery_in_layout_editor() {
        let mut app = router_app();
        app.world_mut()
            .resource_mut::<NavContextStack>()
            .push(NavContext::LayoutEditor);
        app.world_mut().write_message(dtx_input::SystemVerbHit {
            verb: SystemVerb::NavigateDown,
            source: VerbSource::Keyboard,
        });
        app.update();
        assert_eq!(menu_actions(&app), vec![]);
    }

    /// End-to-end wiring: a keyboard Pause hit through the system produces a
    /// LiveVerb and stamps the last-intentional source.
    #[test]
    fn route_verbs_system_delivers_live_verbs_and_stamps_source() {
        let mut app = App::new();
        app.add_message::<dtx_input::SystemVerbHit>()
            .add_message::<NavAction>()
            .add_message::<LiveVerb>()
            .add_message::<MouseIntent>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<NavGuard>()
            .init_resource::<NavContextStack>()
            .insert_resource(LastIntentionalInputSource(InputSource::MidiKit))
            .add_systems(Update, route_verbs);
        app.world_mut()
            .resource_mut::<NavContextStack>()
            .push(NavContext::LiveGameplay);
        app.world_mut().write_message(dtx_input::SystemVerbHit {
            verb: SystemVerb::Pause,
            source: VerbSource::Keyboard,
        });
        app.update();
        let live: Vec<LiveVerb> = app
            .world()
            .resource::<Messages<LiveVerb>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(live, vec![LiveVerb(SystemVerb::Pause)]);
        assert_eq!(
            app.world().resource::<LastIntentionalInputSource>().0,
            InputSource::Keyboard
        );
    }
}
