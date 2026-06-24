//! Guitar mode integration tests.
//!
//! Exercises the guitar lane_map, judge, scroll, and orchestrator to
//! verify the 5-lane gameplay path works end-to-end.

use bevy::prelude::*;
use dtx_core::channel::EChannel;
use dtx_core::chart::{Chart, Chip};
use dtx_timing::AudioClock;
use game_shell::EGameMode;
use gameplay_guitar::events::{LaneHit, LaneHitKind};
use gameplay_guitar::judge::{judge_lane_hit, BpmChangeList, JudgedChips};
use gameplay_guitar::lane_map::{lane_channel, lane_of, LaneMap, GUITAR_LANES};
use gameplay_guitar::resources::{ActiveChart, Combo, GameStartMs, JudgmentCounts, Score};

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_resource::<ActiveChart>()
        .init_resource::<AudioClock>()
        .init_resource::<Score>()
        .init_resource::<Combo>()
        .init_resource::<GameStartMs>()
        .init_resource::<JudgmentCounts>()
        .init_resource::<LaneMap>()
        .init_resource::<JudgedChips>()
        .init_resource::<BpmChangeList>()
        .init_resource::<EGameMode>()
        .add_message::<LaneHit>()
        .add_message::<gameplay_guitar::events::JudgmentEvent>()
        .add_systems(Update, judge_lane_hit);
    app
}

fn set_guitar_mode(app: &mut App) {
    *app.world_mut().resource_mut::<EGameMode>() = EGameMode::Guitar;
}

#[test]
fn guitar_lane_5_total() {
    assert_eq!(GUITAR_LANES.len(), 5);
}

#[test]
fn guitar_lane_discriminants() {
    use gameplay_guitar::guitar_perf::GuitarLane;
    assert_eq!(GuitarLane::R as usize, 0);
    assert_eq!(GuitarLane::G as usize, 1);
    assert_eq!(GuitarLane::B as usize, 2);
    assert_eq!(GuitarLane::Y as usize, 3);
    assert_eq!(GuitarLane::P as usize, 4);
}

#[test]
fn guitar_lane_of_recognizes_rxxxx() {
    let lane = lane_of(EChannel::GuitarRxxxx);
    assert!(lane.is_some());
}

#[test]
fn guitar_lane_of_returns_none_for_drum() {
    assert_eq!(lane_of(EChannel::BassDrum), None);
    assert_eq!(lane_of(EChannel::HiHatClose), None);
}

#[test]
fn guitar_lane_channel_round_trip() {
    for i in 0..5 {
        if let Some(ch) = lane_channel(i) {
            assert_eq!(lane_of(ch), Some(i), "lane {i} round trip");
        }
    }
}

#[test]
fn guitar_lane_channel_invalid_returns_none() {
    assert!(lane_channel(99).is_none());
}

#[test]
fn guitar_chart_with_5_chips_judges_all_perfect() {
    let mut app = build_app();
    set_guitar_mode(&mut app);
    let mut chart = Chart::default();
    chart.metadata.bpm = Some(120.0);
    for m in 0..5 {
        chart.chips.push(Chip {
            measure: m,
            channel: EChannel::GuitarRxxxx,
            value: 0.0,
        });
    }
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;

    let rxxxx_lane = lane_of(EChannel::GuitarRxxxx).unwrap();
    // Send 5 events, advancing the clock past each chip's target time so
    // the judge can match them.
    for m in 0..5 {
        let target_ms = 2000 * (m as i64);
        app.world_mut().resource_mut::<AudioClock>().current_ms = Some(target_ms + 10);
        app.world_mut().write_message(LaneHit {
            lane: rxxxx_lane,
            audio_ms: target_ms,
            kind: LaneHitKind::Press,
        });
        app.update();
    }

    let judged = app.world().resource::<JudgedChips>();
    assert_eq!(judged.0.len(), 5, "all 5 chips judged");
}

#[test]
fn guitar_judged_chips_default_empty() {
    let app = build_app();
    let judged = app.world().resource::<JudgedChips>();
    assert!(judged.0.is_empty());
}

#[test]
fn guitar_bpm_change_default_empty() {
    let app = build_app();
    let bpm = app.world().resource::<BpmChangeList>();
    assert!(bpm.changes.is_empty());
}

#[test]
fn guitar_lane_flush_press_and_tick() {
    use gameplay_guitar::guitar_perf::{GuitarLane, GuitarLaneFlush};
    use std::time::Duration;
    let mut f = GuitarLaneFlush::new();
    f.press(GuitarLane::R);
    assert!(f.pressed[0]);
    f.tick(Duration::from_millis(10000));
    assert_eq!(f.ct_flush[0], GuitarLaneFlush::CT_FLUSH_MAX);
}

#[test]
fn guitar_gauge_state_init() {
    use gameplay_guitar::guitar_perf::GuitarGaugeState;
    let s = GuitarGaugeState::new();
    assert_eq!(s.gauge_guitar, 0.0);
    assert_eq!(s.gauge_bass, 0.0);
}

#[test]
fn guitar_danger_state_init() {
    use gameplay_guitar::guitar_perf::GuitarDangerState;
    let s = GuitarDangerState::new();
    assert!(!s.guitar_danger);
    assert!(!s.bass_danger);
}

#[test]
fn guitar_danger_state_update_from_gauges() {
    use gameplay_guitar::guitar_perf::GuitarDangerState;
    let mut s = GuitarDangerState::new();
    s.update_from_gauges(0.1, 0.5, 0.25);
    assert!(s.guitar_danger);
    assert!(!s.bass_danger);
    assert!(s.any_danger());
}

#[test]
fn guitar_rgb_state_press_release_clear() {
    use gameplay_guitar::guitar_perf::{GuitarLane, GuitarRgbState};
    let mut s = GuitarRgbState::new();
    s.press(GuitarLane::G);
    assert!(s.pressed[1]);
    s.release(GuitarLane::G);
    assert!(!s.pressed[1]);
    s.clear();
    assert!(!s.pressed.iter().any(|p| *p));
}

#[test]
fn guitar_wailing_bonus_start_and_tick() {
    use gameplay_guitar::guitar_perf::GuitarWailingBonus;
    use std::time::Duration;
    let mut w = GuitarWailingBonus::new();
    w.start(0, 0);
    assert!(w.active[0][0]);
    // Wailing bonus expires after 120 frames. 120 frames at 60fps = 2s.
    w.tick(Duration::from_millis(3000));
    assert!(!w.active[0][0], "wailing bonus expires after 2s");
}

#[test]
fn guitar_bonus_increment() {
    use gameplay_guitar::guitar_perf::GuitarBonus;
    let mut b = GuitarBonus::new();
    for _ in 0..50 {
        b.increment();
    }
    assert_eq!(b.count, 50);
    assert!(!b.active);
    for _ in 0..60 {
        b.increment();
    }
    assert_eq!(b.count, 110);
    assert!(b.active, "bonus activates at 100+");
}

#[test]
fn guitar_hold_note_progress() {
    use gameplay_guitar::guitar_perf::HoldNote;
    let n = HoldNote {
        chip_id: 1,
        lane: 0,
        head_ms: 1000,
        tail_ms: 2000,
        is_held: true,
    };
    assert!(!n.is_ended(1500));
    assert!(n.is_ended(2000));
    assert!((n.progress(1500) - 0.5).abs() < 0.01);
}

#[test]
fn guitar_chart_with_guitar_channels_judges_correctly() {
    let mut app = build_app();
    set_guitar_mode(&mut app);
    let mut chart = Chart::default();
    chart.metadata.bpm = Some(120.0);
    chart.chips.push(Chip {
        measure: 0,
        channel: EChannel::GuitarRxxxx,
        value: 0.0,
    });
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;

    let lane = lane_of(EChannel::GuitarRxxxx).unwrap();
    app.world_mut().resource_mut::<AudioClock>().current_ms = Some(100);
    app.world_mut().write_message(LaneHit {
        lane,
        audio_ms: 0,
        kind: LaneHitKind::Press,
    });
    app.update();

    let judged = app.world().resource::<JudgedChips>();
    assert_eq!(judged.0.len(), 1);
}

#[test]
fn guitar_active_chart_default() {
    let app = build_app();
    let chart = app.world().resource::<ActiveChart>();
    assert_eq!(chart.chart.chips.len(), 0);
    assert_eq!(chart.chart.metadata.title, None);
}

#[test]
fn guitar_score_starts_at_zero() {
    let app = build_app();
    let score = app.world().resource::<Score>();
    assert_eq!(score.0, 0);
}

#[test]
fn guitar_combo_starts_at_zero() {
    let app = build_app();
    let combo = app.world().resource::<Combo>();
    assert_eq!(combo.current, 0);
    assert_eq!(combo.max, 0);
}

#[test]
fn guitar_judgment_counts_default() {
    let app = build_app();
    let counts = app.world().resource::<JudgmentCounts>();
    assert_eq!(counts.perfect, 0);
    assert_eq!(counts.great, 0);
    assert_eq!(counts.good, 0);
    assert_eq!(counts.ok, 0);
    assert_eq!(counts.miss, 0);
    assert_eq!(counts.total(), 0);
    assert_eq!(counts.perfect_pct(), 0.0);
}

#[test]
fn guitar_judgment_counts_perfect_pct() {
    use gameplay_guitar::resources::JudgmentCounts;
    let counts = JudgmentCounts {
        perfect: 7,
        great: 2,
        good: 1,
        ok: 0,
        miss: 0,
    };
    assert_eq!(counts.total(), 10);
    assert!((counts.perfect_pct() - 70.0).abs() < 0.01);
}

#[test]
fn guitar_hold_note_clamped_progress() {
    use gameplay_guitar::guitar_perf::HoldNote;
    let n = HoldNote {
        chip_id: 1,
        lane: 0,
        head_ms: 1000,
        tail_ms: 2000,
        is_held: true,
    };
    assert_eq!(n.progress(500), 0.0);
    assert_eq!(n.progress(3000), 1.0);
}

#[test]
fn guitar_hold_note_zero_duration() {
    use gameplay_guitar::guitar_perf::HoldNote;
    let n = HoldNote {
        chip_id: 1,
        lane: 0,
        head_ms: 1000,
        tail_ms: 1000,
        is_held: true,
    };
    assert_eq!(n.progress(1000), 1.0, "zero-duration hold = full");
}

#[test]
fn guitar_wailing_bonus_out_of_range_ignored() {
    use gameplay_guitar::guitar_perf::GuitarWailingBonus;
    let mut w = GuitarWailingBonus::new();
    w.start(10, 10);
    assert!(!w.active[0][0]);
    assert!(!w.active[2][3]);
}

#[test]
fn guitar_danger_both_danger() {
    use gameplay_guitar::guitar_perf::GuitarDangerState;
    let mut s = GuitarDangerState::new();
    s.update_from_gauges(0.1, 0.1, 0.25);
    assert!(s.guitar_danger);
    assert!(s.bass_danger);
    assert!(s.any_danger());
}

#[test]
fn guitar_danger_no_danger() {
    use gameplay_guitar::guitar_perf::GuitarDangerState;
    let mut s = GuitarDangerState::new();
    s.update_from_gauges(0.8, 0.9, 0.25);
    assert!(!s.guitar_danger);
    assert!(!s.bass_danger);
    assert!(!s.any_danger());
}

#[test]
fn guitar_bonus_reset() {
    use gameplay_guitar::guitar_perf::GuitarBonus;
    let mut b = GuitarBonus::new();
    b.increment();
    b.reset();
    assert_eq!(b.count, 0);
    assert!(!b.active);
}

#[test]
fn guitar_wailing_bonus_inactive_at_init() {
    use gameplay_guitar::guitar_perf::GuitarWailingBonus;
    let w = GuitarWailingBonus::new();
    assert!(!w.active.iter().flatten().any(|x| *x));
}

#[test]
fn guitar_lane_flush_any_active_after_press() {
    use gameplay_guitar::guitar_perf::{GuitarLane, GuitarLaneFlush};
    use std::time::Duration;
    let mut f = GuitarLaneFlush::new();
    // Press all 5 lanes so all are "active" before tick.
    for lane in [
        GuitarLane::R,
        GuitarLane::G,
        GuitarLane::B,
        GuitarLane::Y,
        GuitarLane::P,
    ] {
        f.press(lane);
    }
    assert!(f.any_active());
    f.tick(Duration::from_millis(10000));
    assert!(!f.any_active(), "fully decayed = not active");
}

#[test]
fn guitar_rgb_shutter_defaults() {
    use gameplay_guitar::guitar_perf::GuitarRgbState;
    let s = GuitarRgbState::new();
    assert_eq!(s.shutter_up, 0.0);
    assert_eq!(s.shutter_down, 0.0);
}

#[test]
fn guitar_gauge_state_tick() {
    use gameplay_guitar::guitar_perf::GuitarGaugeState;
    use std::time::Duration;
    let mut s = GuitarGaugeState::new();
    s.gauge_guitar = 0.5;
    s.tick(Duration::from_millis(17));
    assert_eq!(s.ct_move, GuitarGaugeState::CT_MOVE_STEP);
    assert_eq!(s.ct_vibration, GuitarGaugeState::CT_VIBRATION_STEP);
}

#[test]
fn guitar_bpm_change_list_from_chart_extracts_bpm_chips() {
    use dtx_core::channel::EChannel;
    let mut chart = Chart::default();
    chart.chips.push(Chip {
        measure: 4,
        channel: EChannel::BPM,
        value: 180.0,
    });
    chart.chips.push(Chip {
        measure: 8,
        channel: EChannel::BPMEx,
        value: 200.0,
    });
    chart.chips.push(Chip {
        measure: 12,
        channel: EChannel::BassDrum,
        value: 1.0,
    });
    let list = BpmChangeList::from_chart(&chart);
    assert_eq!(list.changes.len(), 2);
    assert_eq!(list.changes[0].measure, 4);
    assert!((list.changes[0].bpm - 180.0).abs() < 0.01);
    assert_eq!(list.changes[1].measure, 8);
}

#[test]
fn guitar_bpm_change_list_empty_when_no_bpm_chips() {
    let chart = Chart::default();
    let list = BpmChangeList::from_chart(&chart);
    assert!(list.changes.is_empty());
}
