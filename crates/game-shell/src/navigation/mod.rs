//! Semantic menu-navigation: actions, contexts, guard, and the router.
//!
//! Producer: dtx-input's pump (`SystemVerbHit`, keyboard and MIDI, from
//! configured profile bindings) → [`router::route_verbs`] → [`NavAction`].
//! Consumers: song select, title, pause menu, results, settings overlay,
//! layout editor. Moved/merged from game-shell `nav.rs` and gameplay-drums
//! `menu_nav.rs` (menu-nav extraction, 2026-07-15 spec).

use std::time::{Duration, Instant};

use bevy::prelude::*;

pub use dtx_input::MidiConnected;
/// The canonical semantic action vocabulary (see ADR: unified SystemVerb).
/// `NavAction` is an envelope around it, not a second vocabulary.
pub use dtx_input::SystemVerb;

pub mod context;
pub mod router;
pub mod source;

pub use context::{NavContext, NavContextStack};
pub use router::{LiveVerb, NavRouterSet, Routed, route};
pub use source::{InputSource, LastIntentionalInputSource, MouseIntent, PromptSourcePreference};

/// One navigation action. Screens consume these instead of raw keys/pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct NavAction {
    /// Semantic meaning of the action.
    pub verb: SystemVerb,
    /// Device that produced it.
    pub source: InputSource,
    /// Shift held (keyboard only) — consumers multiply steps by 10.
    pub coarse: bool,
    /// Repeat of a held input; initial presses are false. No producer emits
    /// true yet.
    pub repeated: bool,
}

/// Minimum gap between accepted pad nav actions (double-trigger/flam guard).
const DEBOUNCE: Duration = Duration::from_millis(80);
/// Pad nav ignored for this long after entering a screen/context.
const ENTER_GRACE: Duration = Duration::from_millis(500);

/// Debounce + screen-enter grace bookkeeping.
#[derive(Resource, Debug, Default)]
pub struct NavGuard {
    context: Option<NavContext>,
    entered_at: Option<Instant>,
    last_accept: Option<Instant>,
}

impl NavGuard {
    /// Record the active context; resets the grace window on change.
    pub fn enter_context(&mut self, ctx: NavContext, now: Instant) {
        if self.context != Some(ctx) {
            self.context = Some(ctx);
            self.entered_at = Some(now);
            self.last_accept = None;
        }
    }

    /// Forget the active context — pads are gameplay input again.
    pub fn clear_context(&mut self) {
        self.context = None;
        self.entered_at = None;
        self.last_accept = None;
    }

    /// Follow the context stack: entering a different top resets the grace
    /// window; an empty stack clears the guard.
    pub fn sync(&mut self, top: Option<NavContext>, now: Instant) {
        match top {
            Some(ctx) => self.enter_context(ctx, now),
            None => self.clear_context(),
        }
    }

    /// Test-only: pretend the context was entered long ago so MIDI hits
    /// clear the entry grace without waiting out wall-clock time.
    #[doc(hidden)]
    pub fn force_ready(&mut self, ctx: NavContext, now: Instant) {
        self.context = Some(ctx);
        self.entered_at = Some(now - Duration::from_secs(1));
        self.last_accept = None;
    }

    /// True if a pad hit at `now` may become a [`NavAction`].
    pub fn accept(&mut self, now: Instant) -> bool {
        let Some(entered) = self.entered_at else {
            return false;
        };
        if now.saturating_duration_since(entered) < ENTER_GRACE {
            return false;
        }
        if let Some(last) = self.last_accept
            && now.saturating_duration_since(last) < DEBOUNCE
        {
            return false;
        }
        self.last_accept = Some(now);
        true
    }
}

/// Update-schedule set for systems that write [`NavContextStack`]. Stack
/// consumers (the router) order themselves `.after(NavStackWriteSet)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavStackWriteSet;

/// Update-schedule set for systems that refine the published stack top with
/// screen-local state (e.g. Song Ready layers, practice preview focus). Runs
/// after [`NavStackWriteSet`] and before the router.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavStackRefineSet;

/// Registers the NavAction message, nav resources, and the router.
pub fn plugin(app: &mut App) {
    app.add_message::<NavAction>()
        .add_message::<LiveVerb>()
        .add_message::<MouseIntent>()
        .init_resource::<MidiConnected>()
        .init_resource::<NavGuard>()
        .init_resource::<NavContextStack>()
        .init_resource::<LastIntentionalInputSource>()
        .init_resource::<PromptSourcePreference>()
        .configure_sets(Update, NavStackRefineSet.after(NavStackWriteSet))
        .add_systems(
            Update,
            router::route_verbs
                .in_set(NavRouterSet)
                .after(NavStackRefineSet),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_action_is_copy_and_comparable() {
        let a = NavAction {
            verb: SystemVerb::NavigateUp,
            source: InputSource::MidiKit,
            coarse: false,
            repeated: false,
        };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn guard_enforces_grace_then_debounce() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelectSongs, t0);
        assert!(!g.accept(t0 + std::time::Duration::from_millis(100)));
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        assert!(!g.accept(t1 + std::time::Duration::from_millis(40)));
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
    }

    #[test]
    fn guard_resets_grace_on_context_change() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelectSongs, t0);
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        g.enter_context(NavContext::SongSelectSongs, t1);
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
        g.enter_context(
            NavContext::Results,
            t1 + std::time::Duration::from_millis(200),
        );
        assert!(!g.accept(t1 + std::time::Duration::from_millis(300)));
    }

    /// The BD that confirmed a song must not also cancel the load it started:
    /// entering `Loading` resets the grace window, so the next MIDI hit is
    /// only accepted 500 ms later. Exercised through `route()` — the router
    /// owns the guard now.
    #[test]
    fn confirm_hit_cannot_cancel_the_load_it_started() {
        use dtx_input::VerbSource;
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.sync(Some(NavContext::SongSelectSongs), t0);
        let confirm = t0 + std::time::Duration::from_millis(600);
        assert!(
            matches!(
                route(
                    Some(NavContext::SongSelectSongs),
                    SystemVerb::Confirm,
                    VerbSource::Midi,
                    false,
                    &mut g,
                    confirm,
                ),
                Routed::Menu(_)
            ),
            "BD confirms the song"
        );

        // Next frame: SongLoading is active; the router syncs the guard.
        g.sync(Some(NavContext::SongLoading), confirm);
        let back = |g: &mut NavGuard, at| {
            route(
                Some(NavContext::SongLoading),
                SystemVerb::Back,
                VerbSource::Midi,
                false,
                g,
                at,
            )
        };
        assert_eq!(
            back(&mut g, confirm),
            Routed::Dropped,
            "same-instant hit is inside the grace"
        );
        assert_eq!(
            back(&mut g, confirm + std::time::Duration::from_millis(499)),
            Routed::Dropped
        );
        assert!(matches!(
            back(&mut g, confirm + std::time::Duration::from_millis(500)),
            Routed::Menu(_)
        ));
    }
}
