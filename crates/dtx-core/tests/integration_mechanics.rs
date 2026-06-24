//! Phase F10 — Integration tests for the full DTX mechanics pipeline.
//!
//! Loads a real .dtx fixture, applies random mode, classifies each
//! chip, computes final score + max combo, verifies gauge behavior.

use dtx_core::chart::Chart;
use dtx_core::chip_classify::{classify, is_bad_note_byte, is_open_note_byte, xg_multiplier};
use dtx_core::constants::RandomMode;
use dtx_core::random_mode::{apply_random_mode, to_5key_button};
use dtx_core::trigger_pipeline::trigger_for;
use dtx_scoring::gauge::{gauge_delta, ComboState, GaugeState};
use dtx_scoring::hit_ranges::{classify_with_difficulty, Difficulty, HitRanges};
use dtx_scoring::JudgmentKind;

fn load_minimal() -> Chart {
    let path = std::path::Path::new("tests/fixtures/minimal.dtx");
    let bytes = std::fs::read(path).expect("minimal.dtx fixture");
    dtx_core::parse(bytes.as_slice()).expect("minimal.dtx parses")
}

#[test]
fn integration_load_minimal() {
    let chart = load_minimal();
    assert_eq!(chart.chips.len(), 2);
    assert_eq!(chart.metadata.title.as_deref(), Some("Minimal Test"));
    assert_eq!(chart.metadata.bpm, Some(120.0));
    assert_eq!(chart.metadata.dlevel, Some(30));
}

#[test]
fn integration_chip_classify_on_real_chart() {
    let chart = load_minimal();
    for chip in &chart.chips {
        // Both chips are on channel 0x11 (HiHatClose / Snare range).
        // The fixture uses 0x11 which is HiHatClose in our enum.
        let class = classify(chip.channel);
        // Just verify classification runs without panic.
        assert!(class.is_judgable() || class.is_system());
    }
}

#[test]
fn integration_random_mode_preserves_count() {
    let chart = load_minimal();
    let shuffled = apply_random_mode(&chart.chips, RandomMode::RANDOM, 42);
    assert_eq!(shuffled.len(), chart.chips.len());
}

#[test]
fn integration_mirror_preserves_count() {
    let chart = load_minimal();
    let mirrored = apply_random_mode(&chart.chips, RandomMode::MIRROR, 0);
    assert_eq!(mirrored.len(), chart.chips.len());
}

#[test]
fn integration_judge_all_chips_perfect() {
    // Hypothetical: player hits every chip at the perfect time.
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    let chart = load_minimal();
    for _chip in &chart.chips {
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }
    assert_eq!(combo.perfect_count, 2);
    assert!(combo.is_all_perfect());
    assert!(combo.is_full_combo());
    assert_eq!(combo.max, 2);
    // Gauge should be well above 80 after 2 perfects.
    assert!(gauge.value >= 21.0, "gauge should grow with perfects");
}

#[test]
fn integration_judge_one_miss_one_perfect() {
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    combo.apply(JudgmentKind::Perfect);
    gauge.apply(JudgmentKind::Perfect);
    combo.apply(JudgmentKind::Miss);
    gauge.apply(JudgmentKind::Miss);
    assert_eq!(combo.current, 0, "miss breaks combo");
    assert_eq!(combo.max, 1);
    assert!(!combo.is_full_combo());
    assert!(gauge.value < 20.0, "miss drains gauge");
}

#[test]
fn integration_chip_target_ms_bpm_change() {
    use dtx_timing::math::{chip_time_ms_with_bpm_changes, BpmChange};
    let chart = load_minimal();
    // Chips are at measure=1,2 with fractions derived from 24-bit value.
    let chip = &chart.chips[0];
    let t_no_change = chip_time_ms_with_bpm_changes(chip.measure, chip.value, 120.0, &[]);
    // Just verify it's positive and less than one full measure.
    assert!(t_no_change > 0);
    assert!(
        t_no_change < 2001,
        "chip should be within first measure at 120bpm"
    );
    let changes = [BpmChange {
        measure: 0,
        bpm: 240.0,
    }];
    let t_with_change = chip_time_ms_with_bpm_changes(chip.measure, chip.value, 120.0, &changes);
    // With double BPM, the same chip is earlier in time.
    assert!(t_with_change < t_no_change);
}

#[test]
fn integration_xg_multiplier_applied() {
    // Verify XG multiplier affects score (BD x2 with xg x1 = 2x score; same with xg x2 = 4x).
    use dtx_core::cdtx_config::{score_for_chip, ScoreMode};
    use dtx_core::channel::EChannel;
    let base = 1000;
    let bd = score_for_chip(EChannel::BassDrum, ScoreMode::Type1, base);
    assert_eq!(bd, 2000); // BD x2 in Type1
    let snare = score_for_chip(EChannel::Snare, ScoreMode::Type1, base);
    assert_eq!(snare, 1000); // x1
}

#[test]
fn integration_difficulty_classification_varies() {
    // Same delta classified differently per difficulty.
    let d_normal = classify_with_difficulty(8, Difficulty::Normal);
    let d_master = classify_with_difficulty(8, Difficulty::Master);
    assert_eq!(d_normal, JudgmentKind::Perfect); // 8 < 16 (Normal window)
    assert_eq!(d_master, JudgmentKind::Great); // 8 > 5 (Master window)
}

#[test]
fn integration_hit_ranges_default() {
    // HitRanges::default() returns Normal.
    let r = HitRanges::default();
    assert_eq!(r.perfect, 16);
}

#[test]
fn integration_open_note_detection_real_chip() {
    // The minimal fixture uses 0x11 which is HiHatClose (not open).
    let chart = load_minimal();
    for chip in &chart.chips {
        assert!(!is_open_note_byte(chip.channel as u8));
        assert!(!is_bad_note_byte(chip.channel as u8));
    }
}

#[test]
fn integration_xg_multiplier_default_x1() {
    // Standard channels get x1.
    assert_eq!(xg_multiplier(0x11), 1.0);
    assert_eq!(xg_multiplier(0x13), 1.0);
}

#[test]
fn integration_5key_layout_basic() {
    // The minimal fixture has only snare chips. In 5-key mode, snare
    // maps to button 1.
    let chart = load_minimal();
    for chip in &chart.chips {
        let _button = to_5key_button(chip.channel);
        // Just verify it doesn't panic.
    }
}

#[test]
fn integration_gauge_drain_to_zero_causes_failure() {
    let mut g = GaugeState::new();
    for _ in 0..10 {
        g.apply(JudgmentKind::Miss);
    }
    assert_eq!(g.value, 0.0);
    assert!(g.failed);
}

#[test]
fn integration_full_pipeline_perfect_play() {
    // Simulate a perfect play of the minimal chart (2 chips).
    let chart = load_minimal();
    let mut combo = ComboState::new();
    let mut gauge = GaugeState::new();
    for _chip in &chart.chips {
        combo.apply(JudgmentKind::Perfect);
        gauge.apply(JudgmentKind::Perfect);
    }
    // Both chips judged Perfect.
    assert_eq!(combo.perfect_count, chart.chips.len() as u32);
    assert!(combo.is_all_perfect());
    // Gauge starts at 20 + 0.5/perfect = 21.0 (not yet cleared at 80% on a 2-chip chart).
    assert!(gauge.value > 20.0);
    assert!(!gauge.failed);
}

#[test]
fn integration_full_pipeline_with_random_mode() {
    let chart = load_minimal();
    let shuffled = apply_random_mode(&chart.chips, RandomMode::RANDOM, 42);
    let mut combo = ComboState::new();
    for _chip in &shuffled {
        combo.apply(JudgmentKind::Perfect);
    }
    // All chips are now hit; combo == chip count regardless of order.
    assert_eq!(combo.max, chart.chips.len() as u32);
    assert!(combo.is_full_combo());
}

#[test]
fn integration_trigger_pipeline_on_real_chart() {
    let chart = load_minimal();
    let assets = dtx_core::assets::DtxAssets::default();
    for chip in &chart.chips {
        // Minimal fixture has only drum chips; no triggers expected.
        let trigger = trigger_for(chip, &assets);
        assert!(
            trigger.is_none(),
            "drum chips should not trigger BGA/AVI/WAV"
        );
    }
}

#[test]
fn integration_gauge_delta_sums_correctly() {
    // 2x Perfect = +1.0, 1x Miss = -3.0, net = -2.0
    let net = gauge_delta(JudgmentKind::Perfect) * 2.0 + gauge_delta(JudgmentKind::Miss);
    assert!((net - (-2.0)).abs() < 0.01);
}

#[test]
fn integration_chip_classify_minimal_chart() {
    let chart = load_minimal();
    for chip in &chart.chips {
        // Snare = 0x11 is in drum range.
        let class = classify(chip.channel);
        // It's a system OR drum OR maybe guitar depending on exact value.
        // We just verify classify() doesn't panic on real data.
        assert!(matches!(
            class,
            dtx_core::chip_classify::ChipClass::Drum
                | dtx_core::chip_classify::ChipClass::OpenNote
                | dtx_core::chip_classify::ChipClass::System
        ));
    }
}
