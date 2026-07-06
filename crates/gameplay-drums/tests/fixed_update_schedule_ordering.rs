//! Regression guard for the FixedUpdate ordering cycle found in final
//! integration review (Critical): `apply_seek_system` was wired
//! `.before(update_audio_clock)`, `judge_lane_hit_system` sits in
//! `DrumsSets::Judge` (transitively `.after(update_audio_clock)`), and
//! `track_attempt_stats` was `.after(judge).before(apply_seek_system)` —
//! closing a cycle: apply_seek < judge < track_attempt < apply_seek.
//!
//! Bevy rejects that at schedule-build time, which happens on the very
//! first FixedUpdate tick — on the title screen, before practice mode is
//! ever entered, and `run_if` conditions do not remove nodes from the
//! graph. No existing test caught this because the other integration
//! tests hand-wire the systems under test into `Update`, never building
//! the real FixedUpdate graph from `gameplay_drums::plugin`.
//!
//! This mirrors the ordering from `gameplay-drums/src/lib.rs`
//! (`DrumsSets` + FixedUpdate wiring) and
//! `gameplay-drums/src/practice/stats.rs` (`track_attempt_stats`
//! ordering) with dummy stand-in systems. If that wiring changes, update
//! the mirror here too.

use bevy::prelude::*;
use gameplay_drums::DrumsSets;

fn update_audio_clock_stub() {}
fn sync_gameplay_clock_stub() {}
fn apply_seek_stub() {}
fn judge_stub() {}
fn track_attempt_stub() {}

/// Build an `App` with the FixedUpdate ordering graph mirrored from
/// `lib.rs` + `practice/stats.rs`. `cyclic` reproduces the pre-fix wiring
/// (`track_attempt_stub` also `.before(apply_seek_stub)`).
fn build_app(cyclic: bool) -> App {
    let mut app = App::new();

    app.configure_sets(
        FixedUpdate,
        (
            DrumsSets::ClockSync.after(update_audio_clock_stub),
            DrumsSets::Input.after(DrumsSets::ClockSync),
            DrumsSets::NoteSpawn.after(DrumsSets::Input),
            DrumsSets::Judge.after(DrumsSets::NoteSpawn),
            DrumsSets::Score.after(DrumsSets::Judge),
        ),
    )
    .add_systems(
        FixedUpdate,
        (
            update_audio_clock_stub,
            sync_gameplay_clock_stub.in_set(DrumsSets::ClockSync),
        )
            .chain(),
    )
    .add_systems(
        FixedUpdate,
        apply_seek_stub.before(update_audio_clock_stub),
    )
    .add_systems(FixedUpdate, judge_stub.in_set(DrumsSets::Judge));

    if cyclic {
        app.add_systems(
            FixedUpdate,
            track_attempt_stub
                .after(judge_stub)
                .before(apply_seek_stub),
        );
    } else {
        app.add_systems(FixedUpdate, track_attempt_stub.after(judge_stub));
    }

    app
}

#[test]
fn fixed_update_ordering_builds_without_a_cycle() {
    let mut app = build_app(false);
    // Force the FixedUpdate schedule to build + run once, independent of
    // Time<Fixed> accumulation (which a bare `app.update()` cannot
    // guarantee triggers on the very first frame).
    app.world_mut().run_schedule(FixedUpdate);
}

#[test]
#[should_panic(expected = "before/after cycle")]
fn fixed_update_ordering_with_old_cyclic_edge_panics() {
    // Sanity check that this construction actually exercises the cycle
    // property: reproducing the pre-fix wiring (`track_attempt_stats`
    // also `.before(apply_seek_system)`) must panic at schedule-build
    // time, confirming the positive test above is a real guard and not
    // a vacuously-passing one.
    let mut app = build_app(true);
    app.world_mut().run_schedule(FixedUpdate);
}
