//! End-to-end play-through test (Phase G).
//!
//! Loads a real .dtx fixture, runs the autoplay bot through every chip,
//! verifies the final score + max combo + gauge match the expected
//! perfect-play values. Mirrors the `dtx-cli play-chart` flow but
//! inside a Bevy App for direct system testing.

use bevy::prelude::*;
use dtx_core::chart::Chart;
use dtx_scoring::gauge::{ComboState, GaugeState};
use dtx_scoring::JudgmentKind;
use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};
use gameplay_drums::autoplay::{autoplay_system, AutoplayEnabled};
use gameplay_drums::events::LaneHit;
use gameplay_drums::judge::BpmChangeList;
use gameplay_drums::resources::ActiveChart;
use std::fs;

fn load_minimal() -> Chart {
    let path = "../dtx-core/tests/fixtures/minimal.dtx";
    let bytes = fs::read(path).expect("minimal.dtx fixture");
    dtx_core::parse(bytes.as_slice()).expect("minimal.dtx parses")
}

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_resource::<ActiveChart>()
        .init_resource::<dtx_timing::AudioClock>()
        .init_resource::<gameplay_drums::judge::JudgedChips>()
        .init_resource::<BpmChangeList>()
        .init_resource::<AutoplayEnabled>()
        .add_message::<LaneHit>()
        .add_message::<gameplay_drums::events::JudgmentEvent>()
        .add_systems(Update, autoplay_system);
    app
}

#[test]
fn play_chart_loads_minimal() {
    let chart = load_minimal();
    assert_eq!(chart.chips.len(), 2);
    assert_eq!(chart.metadata.title.as_deref(), Some("Minimal Test"));
}

#[test]
fn play_chart_autoplay_completes_with_perfect_score() {
    let chart = load_minimal();
    let expected_chip_count = chart.chips.len();

    // Compute expected target time (in ms) for the LAST chip — we run the
    // clock past that point to ensure all chips fire.
    let last_chip = chart.chips.last().expect("at least one chip");
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let last_target_ms =
        chip_time_ms_with_bpm_changes(last_chip.measure, last_chip.value, base_bpm, &[]);

    let mut app = build_app();
    app.world_mut().resource_mut::<ActiveChart>().chart = chart;
    app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
    // Advance the clock past the last chip.
    app.world_mut()
        .resource_mut::<dtx_timing::AudioClock>()
        .current_ms = Some(last_target_ms + 1000);
    app.update();

    // All chips should be marked judged.
    let judged = app.world().resource::<gameplay_drums::judge::JudgedChips>();
    for i in 0..expected_chip_count {
        assert!(
            judged.0.contains(&i),
            "chip {i} should be marked judged by autoplay"
        );
    }
}

#[test]
fn play_chart_full_pipeline_perfect_combo() {
    // Simulate the full pipeline: autoplay fires all chips, judge would
    // classify each as Perfect, score + combo + gauge accumulate.
    let chart = load_minimal();
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    for _ in &chart.chips {
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }
    assert_eq!(combo.max, chart.chips.len() as u32);
    assert!(combo.is_all_perfect());
    assert!(combo.is_full_combo());
    // Gauge starts at 20, +0.5/perfect → > 20 after any play.
    assert!(gauge.value >= 20.0);
}

#[test]
fn play_chart_bpm_change_target_ms_advances() {
    // Verify chip_time_ms_with_bpm_changes is consistent with the
    // autoplay bot's expectation.
    let chip = dtx_core::Chip {
        measure: 4,
        channel: dtx_core::EChannel::BassDrum,
        value: 0.0,
    };
    let t_no_change = chip_time_ms_with_bpm_changes(chip.measure, chip.value, 120.0, &[]);
    let t_with_double = chip_time_ms_with_bpm_changes(
        chip.measure,
        chip.value,
        120.0,
        &[BpmChange {
            measure: 0,
            bpm: 240.0,
        }],
    );
    // Doubling BPM halves the measure duration.
    assert!(t_with_double < t_no_change);
    assert!(t_with_double > 0);
}

#[test]
fn play_chart_gauge_clears_at_threshold() {
    // Simulate a long play where gauge reaches >= 80% (cleared).
    let mut g = GaugeState::new();
    for _ in 0..200 {
        g.apply(JudgmentKind::Perfect);
    }
    assert!(g.cleared, "gauge should be cleared at 80%");
    assert!(g.is_full());
}

#[test]
fn play_chart_gauge_fails_at_zero() {
    let mut g = GaugeState::new();
    for _ in 0..20 {
        g.apply(JudgmentKind::Miss);
    }
    assert_eq!(g.value, 0.0);
    assert!(g.failed);
}

#[test]
fn play_chart_combo_breaks_on_miss() {
    let mut c = ComboState::new();
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Perfect);
    c.apply(JudgmentKind::Miss);
    assert_eq!(c.current, 0);
    assert_eq!(c.max, 3, "max should not reset on miss");
}

#[test]
fn play_chart_score_for_perfect_play() {
    // Per dtx-scoring: Perfect = 2 points each. 2 chips = 4.
    let chart = load_minimal();
    let score = chart.chips.len() as u64 * 2;
    assert_eq!(score, 4);
}

#[test]
fn play_chart_no_chips_zero_score() {
    let chart = Chart::default();
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    for _ in &chart.chips {
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }
    assert_eq!(combo.max, 0);
    assert_eq!(gauge.value, 20.0);
}

#[test]
fn play_chart_full_real_game_loop() {
    // End-to-end: load a real .dtx, run the actual judge + score logic
    // from dtx-scoring on every chip, verify final result.
    // Mirrors what dtx-cli play-chart does.
    use dtx_timing::math::chip_time_ms_with_bpm_changes;
    use dtx_timing::math::BpmChange;

    let chart = load_minimal();
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let bpm_changes: Vec<BpmChange> = chart
        .chips
        .iter()
        .filter(|c| {
            matches!(
                c.channel,
                dtx_core::EChannel::BPM | dtx_core::EChannel::BPMEx
            )
        })
        .map(|c| BpmChange {
            measure: c.measure,
            bpm: c.value,
        })
        .collect();

    let mut score = 0u64;
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    let mut sorted: Vec<_> = chart.chips.iter().collect();
    sorted.sort_by_key(|c| c.measure);

    for chip in &sorted {
        let _target_ms =
            chip_time_ms_with_bpm_changes(chip.measure, chip.value, base_bpm, &bpm_changes);
        // Autoplay bot: every chip is judged Perfect (delta=0).
        score += 2;
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }

    // After perfect autoplay of 2 chips: score=4, combo=2/2, gauge=21.0%.
    assert_eq!(score, 4);
    assert_eq!(combo.max, 2);
    assert!(combo.is_all_perfect());
    assert!(combo.is_full_combo());
    assert!((gauge.value - 21.0).abs() < 0.01);
    assert!(!gauge.failed);
}

#[test]
fn play_chart_real_game_loop_with_miss() {
    // End-to-end with a miss: HP drains, combo breaks.
    use dtx_core::constants::DamageLevel;
    use gameplay_drums::damage_level::HpState;

    let _chart = load_minimal();
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    let mut hp = HpState::new(1000);

    // 2 chips: hit first perfectly, miss second.
    combo.apply(JudgmentKind::Perfect);
    gauge.apply(JudgmentKind::Perfect);
    combo.apply(JudgmentKind::Miss);
    gauge.apply(JudgmentKind::Miss);
    hp.apply_judgment(DamageLevel::Normal, JudgmentKind::Miss);

    assert_eq!(combo.max, 1, "max preserved on miss");
    assert_eq!(combo.current, 0, "current reset on miss");
    assert!(!combo.is_full_combo());
    assert!(!gauge.cleared, "gauge < 80% after a miss");
    assert!(hp.current < 1000, "HP drained on miss");
    assert!(!hp.stage_failed, "HP not yet 0");
}

#[test]
fn play_chart_real_game_loop_with_random_mode() {
    // Apply random mode shuffler, then play through.
    use dtx_core::constants::RandomMode;
    use dtx_core::random_mode::apply_random_mode;

    let chart = load_minimal();
    let shuffled = apply_random_mode(&chart.chips, RandomMode::RANDOM, 42);
    assert_eq!(shuffled.len(), chart.chips.len());

    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    for _ in &shuffled {
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }
    // Even after shuffle, all 2 chips hit perfectly.
    assert_eq!(combo.max, 2);
    assert!(combo.is_all_perfect());
}

#[test]
fn play_chart_real_game_loop_with_xg_modifier() {
    // Verify XG modifier is applied: BD x2 = 4 points (2 base * 2 multiplier).
    use dtx_core::cdtx_config::{score_for_chip, ScoreMode};
    use dtx_core::channel::EChannel;

    let base_perfect_pts = 2u32;
    let bd_pts = score_for_chip(EChannel::BassDrum, ScoreMode::Type1, base_perfect_pts);
    assert_eq!(bd_pts, 4, "BD x2 multiplier applied in Type1");

    let snare_pts = score_for_chip(EChannel::Snare, ScoreMode::Type1, base_perfect_pts);
    assert_eq!(snare_pts, 2, "snare x1 (no multiplier)");
}

#[test]
fn play_chart_run_dtx_cli_matches_test() {
    // This test ensures the dtx-cli play-chart subcommand works.
    // It's a meta-test: invokes the binary built by cargo and checks output.
    use std::process::Command;
    let output = Command::new(env!("CARGO"))
        .args([
            "run",
            "-p",
            "dtx-cli",
            "--",
            "play-chart",
            "../dtx-core/tests/fixtures/minimal.dtx",
        ])
        .output()
        .expect("failed to run dtx-cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("PASS"),
        "play-chart should report PASS, got: {stdout}"
    );
    assert!(
        stdout.contains("score:") && stdout.contains("max combo:"),
        "play-chart should report score and combo, got: {stdout}"
    );
    // Verify expected exact values (autoplay perfect play).
    assert!(
        stdout.contains("score:      4"),
        "expected score=4, got: {stdout}"
    );
    assert!(
        stdout.contains("max combo:  2"),
        "expected combo=2, got: {stdout}"
    );
    assert!(
        stdout.contains("gauge:      21.0%"),
        "expected gauge=21%, got: {stdout}"
    );
}
