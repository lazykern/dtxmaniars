//! Comprehensive tests for chip_time_ms_with_bpm_changes — covers all 5
//! cases required by Phase F1:
//!   (1) no changes (constant BPM)
//!   (2) one mid-chart change
//!   (3) multiple changes (3+)
//!   (4) fraction > 1.0
//!   (5) same-measure edge case
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CChip.cs:ComputeTime`

use dtx_timing::math::{chip_time_ms, chip_time_ms_with_bpm_changes, BpmChange};

#[test]
fn case1_no_changes_matches_constant() {
    // (1) no changes: must equal plain chip_time_ms
    let t1 = chip_time_ms(7, 0.25, 120.0);
    let t2 = chip_time_ms_with_bpm_changes(7, 0.25, 120.0, &[]);
    assert_eq!(t1, t2);
}

#[test]
fn case1_no_changes_specific_value() {
    // 120 BPM = 2000ms/measure. 5 measures + 0.5 = 11000ms.
    let t = chip_time_ms_with_bpm_changes(5, 0.5, 120.0, &[]);
    assert_eq!(t, 11000);
}

#[test]
fn case2_one_mid_chart_change_doubles_bpm() {
    // (2) one mid-chart change: BPM doubles at measure 4 from 120 to 240
    let changes = [BpmChange {
        measure: 4,
        bpm: 240.0,
        fraction: 0.0,
    }];
    // First 4 measures at 120 BPM = 4 * 2000 = 8000ms
    // Measure 4 onward at 240 BPM = 2000ms/2 = 1000ms/measure
    // Chip at measure 8, fraction 0.0: 8000 + 4*1000 = 12000ms
    let t = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &changes);
    assert_eq!(t, 12000);
}

#[test]
fn case2_one_change_with_fraction() {
    // Same setup; chip at measure 4, fraction 0.5.
    // The change sits at measure 4, position 0.0 — BEFORE the chip inside that
    // measure — so the chip's own partial measure runs at the NEW bpm.
    // DTXManiaNX walks chips in position order and updates the running BPM as
    // it passes each one (CDTX.cs:1070-1080); it does not defer a change to the
    // next bar line.
    //   [0,4) at 120 = 8000
    //   partial 0.5 of measure 4 at 240 (1000ms/measure) = 500
    let changes = [BpmChange {
        measure: 4,
        bpm: 240.0,
        fraction: 0.0,
    }];
    let t = chip_time_ms_with_bpm_changes(4, 0.5, 120.0, &changes);
    assert_eq!(t, 8500);
}

#[test]
fn change_later_in_measure_does_not_affect_earlier_chip() {
    // Change at measure 4 position 0.75; the chip at position 0.5 comes first,
    // so it must still be timed at the OLD bpm.
    let changes = [BpmChange {
        measure: 4,
        bpm: 240.0,
        fraction: 0.75,
    }];
    // [0,4) at 120 = 8000, partial 0.5 at 120 (2000ms/measure) = 1000
    let t = chip_time_ms_with_bpm_changes(4, 0.5, 120.0, &changes);
    assert_eq!(t, 9000);
}

#[test]
fn mid_measure_change_splits_the_measure() {
    // The core regression: two changes inside ONE measure at different
    // positions. The measure must be integrated piecewise, not collapsed to a
    // single BPM.
    //
    // Real chart: `465 - Yoru wa hikari (Rock Lady)` ext, `#20208: 090B`
    // → #BPM09 = 71 at position 0.0, #BPM0B = 179 at position 0.5.
    //
    // Measure duration = half at 71 + half at 179:
    //   half at 71  = 0.5 * 4 * 60000 / 71  ≈ 1690.1ms
    //   half at 179 = 0.5 * 4 * 60000 / 179 ≈  670.4ms
    //   total       ≈ 2360.5ms
    // Snapping both changes to the bar line (the old bug) timed the whole
    // measure at 179 ≈ 1340.8ms — ~1s short, which made every note after the
    // slow section arrive early.
    let changes = [
        BpmChange {
            measure: 0,
            bpm: 71.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 0,
            bpm: 179.0,
            fraction: 0.5,
        },
    ];
    // Chip at the start of the NEXT measure = the full split measure.
    let t = chip_time_ms_with_bpm_changes(1, 0.0, 120.0, &changes);
    assert!((t - 2360).abs() <= 1, "expected ~2360ms, got {t}");

    // A chip exactly at the change point sees only the slow half.
    let t_mid = chip_time_ms_with_bpm_changes(0, 0.5, 120.0, &changes);
    assert!((t_mid - 1690).abs() <= 1, "expected ~1690ms, got {t_mid}");
}

#[test]
fn case3_multiple_changes_three() {
    // (3) 3 BPM changes: 120 → 60 → 240 → 180
    let changes = [
        BpmChange {
            measure: 2,
            bpm: 60.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 4,
            bpm: 240.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 6,
            bpm: 180.0,
            fraction: 0.0,
        },
    ];
    // 120 BPM = 2000ms/measure
    // 60 BPM = 4000ms/measure
    // 240 BPM = 1000ms/measure
    // 180 BPM = 1333.33ms/measure
    // Chip at measure 8, fraction 0.0:
    //   [0, 2) at 120 = 4000
    //   [2, 4) at 60  = 8000
    //   [4, 6) at 240 = 2000
    //   [6, 8) at 180 = 2666.67
    //   [8, 8) at 180 = 0
    //   Total ≈ 16666.67 → 16666 (truncated to i64)
    let t = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &changes);
    assert!((t - 16666).abs() <= 1, "expected ~16666, got {t}");
}

#[test]
fn case3_multiple_changes_within_chip_measure() {
    // Multiple changes where chip falls between two changes
    let changes = [
        BpmChange {
            measure: 1,
            bpm: 60.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 2,
            bpm: 240.0,
            fraction: 0.0,
        },
    ];
    // [0,1) at 120 = 2000
    // [1,2) at 60 = 4000
    // [2,5) at 240 = 3000
    // Chip at measure 5: 2000 + 4000 + 3000 = 9000
    let t = chip_time_ms_with_bpm_changes(5, 0.0, 120.0, &changes);
    assert_eq!(t, 9000);
}

#[test]
fn case3_unsorted_changes_produce_same_result() {
    // Same as above but changes provided in arbitrary order — must sort.
    let a = [BpmChange {
        measure: 2,
        bpm: 240.0,
        fraction: 0.0,
    }];
    let b = [BpmChange {
        measure: 2,
        bpm: 240.0,
        fraction: 0.0,
    }];
    let t_a = chip_time_ms_with_bpm_changes(5, 0.0, 120.0, &a);
    let t_b = chip_time_ms_with_bpm_changes(5, 0.0, 120.0, &b);
    assert_eq!(t_a, t_b);
}

#[test]
fn case3_reversed_changes() {
    // Provide changes in reverse order; must still sort.
    let changes = [
        BpmChange {
            measure: 6,
            bpm: 180.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 2,
            bpm: 60.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 4,
            bpm: 240.0,
            fraction: 0.0,
        },
    ];
    let t_sorted = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &changes);
    let mut sorted = changes.to_vec();
    sorted.sort_by_key(|c| c.measure);
    let t_check = chip_time_ms_with_bpm_changes(8, 0.0, 120.0, &sorted);
    assert_eq!(t_sorted, t_check);
}

#[test]
fn case4_fraction_greater_than_one() {
    // (4) fraction > 1.0 — equivalent to next measure + remainder
    let changes = [BpmChange {
        measure: 2,
        bpm: 60.0,
        fraction: 0.0,
    }];
    // 120 BPM = 2000ms/measure, 60 BPM = 4000ms/measure
    // Chip at measure 1, fraction 1.5
    //   [0, 1) at 120 = 2000
    //   partial [1, 1 + 1.5 = 2.5) at 120 = 1.5 * 2000 = 3000
    //   Total = 5000ms
    let t = chip_time_ms_with_bpm_changes(1, 1.5, 120.0, &changes);
    assert_eq!(t, 5000);
}

#[test]
fn case4_fraction_two_with_changes() {
    // fraction = 2.0 = two full measures. Mid-chart change at measure 2.
    // Algorithm considers only integer measure (2 >= 1 → change skipped).
    // So chip stays at base 120 BPM: [0, 3) at 120 = 6000ms.
    let changes = [BpmChange {
        measure: 2,
        bpm: 60.0,
        fraction: 0.0,
    }];
    let t = chip_time_ms_with_bpm_changes(1, 2.0, 120.0, &changes);
    assert_eq!(t, 6000);
}

#[test]
fn case5_change_at_chip_position_does_not_move_it() {
    // (5) A change sitting exactly at the chip's own position spans zero time
    // before that chip, so it cannot move it — it only affects later chips.
    let changes = [BpmChange {
        measure: 4,
        bpm: 60.0,
        fraction: 0.0,
    }];
    // [0, 4) at 120 = 8000, then zero width at the chip → 8000
    let t = chip_time_ms_with_bpm_changes(4, 0.0, 120.0, &changes);
    assert_eq!(t, 8000);
}

#[test]
fn case5_multiple_changes_at_same_position() {
    // Two changes at the same measure AND position. Both sit at the chip's own
    // position, so both span zero time and the chip is unmoved.
    let changes = [
        BpmChange {
            measure: 4,
            bpm: 60.0,
            fraction: 0.0,
        },
        BpmChange {
            measure: 4,
            bpm: 240.0,
            fraction: 0.0,
        },
    ];
    let t = chip_time_ms_with_bpm_changes(4, 0.0, 120.0, &changes);
    assert_eq!(t, 8000);

    // ...but a chip later in that measure sees the LAST of them (240 BPM).
    // [0,4) at 120 = 8000, partial 0.5 at 240 (1000ms/measure) = 500
    let t_after = chip_time_ms_with_bpm_changes(4, 0.5, 120.0, &changes);
    assert_eq!(t_after, 8500);
}

#[test]
fn case5_change_before_chip_uses_new_bpm() {
    // Change at measure 2 (before chip at 4) is applied.
    let changes = [BpmChange {
        measure: 2,
        bpm: 240.0,
        fraction: 0.0,
    }];
    // [0, 2) at 120 = 4000
    // [2, 4) at 240 = 2000
    // Total = 6000
    let t = chip_time_ms_with_bpm_changes(4, 0.0, 120.0, &changes);
    assert_eq!(t, 6000);
}

#[test]
fn chip_at_measure_zero() {
    // Edge case: chip at measure 0 (no time has passed).
    let t = chip_time_ms_with_bpm_changes(0, 0.0, 120.0, &[]);
    assert_eq!(t, 0);
    let t = chip_time_ms_with_bpm_changes(0, 0.5, 120.0, &[]);
    assert_eq!(t, 1000);
}

#[test]
fn change_at_measure_zero() {
    // BPM change at measure 0 — applied immediately.
    // 240 BPM = 1000ms/measure, 4 measures = 4000ms.
    let changes = [BpmChange {
        measure: 0,
        bpm: 240.0,
        fraction: 0.0,
    }];
    let t = chip_time_ms_with_bpm_changes(4, 0.0, 120.0, &changes);
    assert_eq!(t, 4000);
}

#[test]
fn many_changes_stable() {
    // Stress: 10 BPM changes in a row.
    let changes: Vec<BpmChange> = (1..=10)
        .map(|i| BpmChange {
            measure: i * 2,
            bpm: 100.0 + (i as f32) * 10.0,
            fraction: 0.0,
        })
        .collect();
    // Result should be deterministic (no panics, no NaN, monotonic with measure).
    let t1 = chip_time_ms_with_bpm_changes(5, 0.0, 120.0, &changes);
    let t2 = chip_time_ms_with_bpm_changes(6, 0.0, 120.0, &changes);
    assert!(t2 > t1, "later measure must be later: t1={t1} t2={t2}");
    assert!(t1 > 0);
}

#[test]
fn zero_base_bpm_returns_zero() {
    // Defensive: base BPM=0 should not panic.
    let changes = [BpmChange {
        measure: 4,
        bpm: 240.0,
        fraction: 0.0,
    }];
    let t = chip_time_ms_with_bpm_changes(5, 0.5, 0.0, &changes);
    assert_eq!(t, 0);
}
