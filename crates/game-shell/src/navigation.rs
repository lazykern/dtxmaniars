//! Semantic menu-navigation: actions, contexts, guard, and the pad mapper.
//!
//! Producers: per-screen keyboard systems and dtx-input's MIDI pump
//! (`PadNavHit`). Consumers: song select, title, pause menu, results,
//! settings overlay. Moved/merged from game-shell `nav.rs` and
//! gameplay-drums `menu_nav.rs` (menu-nav extraction, 2026-07-15 spec).

use std::time::{Duration, Instant};

use bevy::prelude::*;

pub use dtx_input::MidiConnected;

/// What the input means, not what produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavVerb {
    /// Move focus up / previous. Pads: HH.
    Up,
    /// Move focus down / next. Pads: CY/RD.
    Down,
    /// Enter / select / apply. Pads: BD.
    Confirm,
    /// Back out / cancel. Pads: SD.
    Back,
    /// Decrement focused value (keyboard Left; pads reuse Up in adjust mode).
    Dec,
    /// Increment focused value (keyboard Right; pads reuse Down in adjust mode).
    Inc,
    /// Start practice mode (keyboard Shift+Enter; pads FT at difficulty level).
    Practice,
}

/// Which device produced the action. Consumers may branch on this: keyboard
/// keeps its flat navigation model, pads use the two-level model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSource {
    /// Physical keyboard.
    Keyboard,
    /// Drum pad / MIDI device.
    Pad,
}

/// One navigation action. Screens consume these instead of raw keys/pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct NavAction {
    /// Semantic meaning of the action.
    pub verb: NavVerb,
    /// Device that produced it.
    pub source: NavSource,
    /// Shift held (keyboard only) — consumers multiply steps by 10.
    pub coarse: bool,
}

/// Which menu surface pads are currently navigating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavContext {
    /// Title screen.
    Title,
    /// Song select wheel.
    SongSelect,
    /// Post-play results screen.
    Result,
    /// Pause overlay during a performance.
    Paused,
    /// Customize (settings) overlay during a performance.
    Editor,
    /// Chart/audio load in progress. Pads may cancel it (SD = Back).
    Loading,
    /// Practice Setup or Settings, including non-judged preview playback.
    PracticeSetup,
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

/// GITADORA-ish convention. Lane ids per `dtx_input::lane_map::LANE_ORDER`.
pub(crate) fn verb_for_lane(lane: u8) -> Option<NavVerb> {
    match lane {
        0 | 7 => Some(NavVerb::Up),
        6 | 8 => Some(NavVerb::Down),
        2 => Some(NavVerb::Confirm),
        1 => Some(NavVerb::Back),
        3 => Some(NavVerb::Dec),
        4 => Some(NavVerb::Inc),
        5 => Some(NavVerb::Practice),
        _ => None,
    }
}

/// Which menu surface currently owns pad navigation. `None` = pads are
/// gameplay input, or a capture/calibration overlay owns raw hits. Written by
/// the crate that knows the surface state (gameplay-drums publishes it every
/// frame, before [`NavMapSet`]); consumed by [`pad_nav_mapper`].
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActiveNavContext(pub Option<NavContext>);

/// Update-schedule set the pad mapper runs in. Context writers order
/// themselves `.before(NavMapSet)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavMapSet;

fn pad_nav_mapper(
    ctx: Res<ActiveNavContext>,
    mut hits: MessageReader<dtx_input::PadNavHit>,
    mut guard: ResMut<NavGuard>,
    mut out: MessageWriter<NavAction>,
) {
    let now = Instant::now();
    let Some(ctx) = ctx.0 else {
        guard.clear_context();
        hits.clear();
        return;
    };
    guard.enter_context(ctx, now);
    for hit in hits.read() {
        let Some(verb) = verb_for_lane(hit.lane) else {
            continue;
        };
        if !guard.accept(now) {
            continue;
        }
        out.write(NavAction {
            verb,
            source: NavSource::Pad,
            coarse: false,
        });
    }
}

/// Registers the NavAction message, nav resources, and the pad mapper.
pub fn plugin(app: &mut App) {
    app.add_message::<NavAction>()
        .init_resource::<MidiConnected>()
        .init_resource::<NavGuard>()
        .init_resource::<ActiveNavContext>()
        .add_systems(Update, pad_nav_mapper.in_set(NavMapSet));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_action_is_copy_and_comparable() {
        let a = NavAction {
            verb: NavVerb::Up,
            source: NavSource::Pad,
            coarse: false,
        };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn lane_verbs_follow_gitadora_convention() {
        assert_eq!(verb_for_lane(0), Some(NavVerb::Up)); // HH close
        assert_eq!(verb_for_lane(7), Some(NavVerb::Up)); // HH open
        assert_eq!(verb_for_lane(6), Some(NavVerb::Down)); // CY
        assert_eq!(verb_for_lane(8), Some(NavVerb::Down)); // RD
        assert_eq!(verb_for_lane(2), Some(NavVerb::Confirm)); // BD
        assert_eq!(verb_for_lane(1), Some(NavVerb::Back)); // SD
        assert_eq!(verb_for_lane(5), Some(NavVerb::Practice)); // FT
        assert_eq!(verb_for_lane(3), Some(NavVerb::Dec)); // HT
        assert_eq!(verb_for_lane(4), Some(NavVerb::Inc)); // LT
        assert_eq!(verb_for_lane(10), None); // LP unmapped
    }

    #[test]
    fn toms_supply_explicit_quick_setting_adjustment_verbs() {
        assert_eq!(verb_for_lane(3), Some(NavVerb::Dec));
        assert_eq!(verb_for_lane(4), Some(NavVerb::Inc));
    }

    #[test]
    fn guard_enforces_grace_then_debounce() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelect, t0);
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
        g.enter_context(NavContext::SongSelect, t0);
        let t1 = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(t1));
        g.enter_context(NavContext::SongSelect, t1);
        assert!(g.accept(t1 + std::time::Duration::from_millis(100)));
        g.enter_context(
            NavContext::Result,
            t1 + std::time::Duration::from_millis(200),
        );
        assert!(!g.accept(t1 + std::time::Duration::from_millis(300)));
    }

    /// The BD that confirmed a song must not also cancel the load it started:
    /// entering `Loading` resets the grace window, so the next pad hit is only
    /// accepted 500 ms later.
    #[test]
    fn confirm_hit_cannot_cancel_the_load_it_started() {
        let mut g = NavGuard::default();
        let t0 = std::time::Instant::now();
        g.enter_context(NavContext::SongSelect, t0);
        let confirm = t0 + std::time::Duration::from_millis(600);
        assert!(g.accept(confirm), "BD confirms the song");

        // Next frame: SongLoading is active.
        g.enter_context(NavContext::Loading, confirm);
        assert!(!g.accept(confirm), "same-instant hit is inside the grace");
        assert!(!g.accept(confirm + std::time::Duration::from_millis(499)));
        assert!(g.accept(confirm + std::time::Duration::from_millis(500)));
    }

    /// The mapper must read `PadNavHit`, never `LaneHit` — autoplay (forced on
    /// by the Customize surface) and keyboard lane keys write `LaneHit`, and a
    /// chart's autoplay notes would otherwise navigate and close the overlay.
    #[test]
    fn mapper_consumes_pad_nav_hits_not_lane_hits() {
        let src = include_str!("navigation.rs");
        let body = src
            .split("fn pad_nav_mapper(")
            .nth(1)
            .expect("pad_nav_mapper exists");
        let signature = body.split(") {").next().unwrap();
        assert!(
            signature.contains("PadNavHit"),
            "mapper must read PadNavHit"
        );
        assert!(
            !signature.contains("LaneHit"),
            "mapper must not read LaneHit (autoplay + keyboard write those)"
        );
    }
}
