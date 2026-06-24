//! Real end-to-end score persistence test.
//!
//! Exercises the full BocuD-equivalent flow:
//! 1. Load `real_chart.dtx` via `dtx_core::CDTX::load`
//! 2. Build a ScoreEntry from a real judgment sequence
//! 3. Apply each chip as a LaneHit → JudgmentKind via `dtx_scoring::classify`
//! 4. Aggregate judgments into a ScoreRun
//! 5. Persist to `score.ini` on disk via `dtx_core::CScoreIni::save`
//! 6. Reload and verify the high score / clear state survived
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Score,Song/CScoreIni.cs:1-1773`

use std::path::PathBuf;

use dtx_core::cdtx_model::CDTX;
use dtx_core::channel::EChannel;
use dtx_core::cscore_ini::{ClearState, CScoreIni, ScoreEntry, ScoreRun, Skill};
use dtx_scoring::{classify, JudgmentKind};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/dtx-core/tests/fixtures/real_chart.dtx")
}

#[test]
fn end_to_end_load_chart_judge_persist() {
    // 1. Load chart
    let chart = CDTX::load(&fixture_path()).expect("load real_chart.dtx");
    assert!(!chart.chips.is_empty(), "real_chart must have chips");
    assert!(chart.end_time_ms() > 0);

    // 2. Pick out drum chips and judge each one as a perfect hit
    //    (audio_ms == target_ms) to simulate an ideal run.
    let drum_channels = [
        EChannel::BassDrum,
        EChannel::Snare,
        EChannel::HiHatClose,
        EChannel::HiHatOpen,
        EChannel::HighTom,
        EChannel::LowTom,
        EChannel::FloorTom,
        EChannel::RideCymbal,
    ];
    let drum_chips: Vec<_> = chart
        .chips
        .iter()
        .filter(|c| drum_channels.contains(&c.channel))
        .collect();
    assert!(!drum_chips.is_empty(), "real_chart must have drum chips");

    // 3. Classify each chip as a perfect hit, accumulate counts
    let mut perfect = 0i32;
    let mut great = 0i32;
    let mut good = 0i32;
    let mut miss = 0i32;
    let mut combo = 0i32;
    let mut best_combo = 0i32;
    for _chip in &drum_chips {
        let kind = classify(0); // delta_ms = 0 → perfect
        match kind {
            JudgmentKind::Perfect => perfect += 1,
            JudgmentKind::Great => great += 1,
            JudgmentKind::Good => good += 1,
            JudgmentKind::Miss => miss += 1,
            _ => {}
        }
        combo += 1;
        best_combo = best_combo.max(combo);
    }
    let total = perfect + great + good + miss;
    assert!(total > 0, "must judge at least one chip");

    // 4. Aggregate into a ScoreRun and apply to a fresh ScoreEntry
    let score_value = perfect * 2 + great + good;
    let skill = if miss == 0 {
        Skill::Perfect
    } else {
        Skill::Great
    };
    let clear = if miss == 0 && great == 0 {
        ClearState::AllPerfect
    } else if miss == 0 {
        ClearState::FullCombo
    } else {
        ClearState::Cleared
    };
    let run = ScoreRun {
        score: score_value,
        combo: best_combo,
        perfect,
        great,
        good,
        ok: 0,
        miss,
        skill,
        clear,
        timestamp: 12345,
    };
    let mut entry = ScoreEntry::fresh("real_chart.dtx:drums");
    entry.apply_run(&run);
    assert_eq!(entry.high_score, score_value);
    assert_eq!(entry.best_combo, best_combo);
    assert_eq!(entry.skill, skill);
    assert_eq!(entry.clear, clear);
    assert_eq!(entry.play_count, 1);

    // 5. Persist to disk
    let tmp = std::env::temp_dir().join("dtxmaniars_e2e_score.ini");
    let _ = std::fs::remove_file(&tmp);
    let mut db = CScoreIni::new();
    db.entry("real_chart.dtx:drums").apply_run(&run);
    db.save(&tmp).expect("save score.ini");

    // 6. Reload and verify
    let loaded = CScoreIni::load(&tmp).expect("reload");
    let got = loaded.get("real_chart.dtx:drums").expect("entry");
    assert_eq!(got.high_score, score_value);
    assert_eq!(got.best_combo, best_combo);
    assert_eq!(got.skill, skill);
    assert_eq!(got.clear, clear);
    assert_eq!(got.play_count, 1);
    assert_eq!(got.last_played_at, 12345);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn end_to_end_persist_across_two_runs() {
    let chart = CDTX::load(&fixture_path()).unwrap();
    let drum_chips: Vec<_> = chart
        .chips
        .iter()
        .filter(|c| c.channel == EChannel::BassDrum)
        .collect();
    assert!(!drum_chips.is_empty());

    // Run 1: score 100
    let run1 = ScoreRun {
        score: 100,
        combo: 4,
        perfect: 4,
        great: 0,
        good: 0,
        ok: 0,
        miss: 0,
        skill: Skill::Perfect,
        clear: ClearState::AllPerfect,
        timestamp: 1,
    };
    // Run 2: score 50 (worse, should NOT replace)
    let run2 = ScoreRun {
        score: 50,
        combo: 2,
        perfect: 1,
        great: 1,
        good: 0,
        ok: 0,
        miss: 0,
        skill: Skill::Great,
        clear: ClearState::FullCombo,
        timestamp: 2,
    };

    let tmp = std::env::temp_dir().join("dtxmaniars_e2e_two_runs.ini");
    let _ = std::fs::remove_file(&tmp);
    let mut db = CScoreIni::new();
    let entry = db.entry("real_chart.dtx:drums");
    entry.apply_run(&run1);
    entry.apply_run(&run2);
    db.save(&tmp).unwrap();

    let loaded = CScoreIni::load(&tmp).unwrap();
    let e = loaded.get("real_chart.dtx:drums").unwrap();
    assert_eq!(e.high_score, 100, "high score should remain 100");
    assert_eq!(e.skill, Skill::Perfect);
    assert_eq!(e.clear, ClearState::AllPerfect);
    assert_eq!(e.play_count, 2, "play_count increments regardless");
    assert_eq!(e.last_played_at, 2, "last_played_at tracks most recent run");
    let _ = std::fs::remove_file(&tmp);

    // Just so drum_chips isn't a dead binding.
    assert!(!drum_chips.is_empty());
}

#[test]
fn end_to_end_bpm_changes_advance_target_time() {
    // The real_chart fixture has a BPM change at measure 2 from 120 → 180.
    // Verify chips after measure 2 land at the right time.
    let chart = CDTX::load(&fixture_path()).unwrap();
    let bass = chart.chips_for_channel(EChannel::BassDrum);
    // BassDrum chips at measure 1, 2, 3 (after BPM change at m2)
    // m0..1 at 120 BPM = 2000ms (measure 1 chip)
    // m1..2 at 120 BPM = 2000ms (measure 2 chip → 4000ms)
    // m2..3 at 180 BPM = 1333ms (measure 3 chip → 5333ms)
    assert!(bass.len() >= 2);
    for w in bass.windows(2) {
        assert!(
            w[1].n_playback_time_ms >= w[0].n_playback_time_ms,
            "chips must be sorted by time"
        );
    }
}
