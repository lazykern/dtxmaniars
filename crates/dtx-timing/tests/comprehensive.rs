//! Comprehensive tests for dtx-timing — BPM change math, target time.

use dtx_timing::math::BpmChange;

#[test]
fn bpm_change_struct_basic() {
    let c = BpmChange {
        measure: 2,
        bpm: 180.0,
        fraction: 0.0,
    };
    assert_eq!(c.measure, 2);
    assert!((c.bpm - 180.0).abs() < 0.01);
}

#[test]
fn bpm_change_struct_zero_measure() {
    let c = BpmChange {
        measure: 0,
        bpm: 120.0,
        fraction: 0.0,
    };
    assert_eq!(c.measure, 0);
    assert_eq!(c.bpm, 120.0);
}

#[test]
fn bpm_change_struct_equality() {
    let a = BpmChange {
        measure: 1,
        bpm: 120.0,
        fraction: 0.0,
    };
    let b = BpmChange {
        measure: 1,
        bpm: 120.0,
        fraction: 0.0,
    };
    let c = BpmChange {
        measure: 2,
        bpm: 120.0,
        fraction: 0.0,
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn bpm_change_high_bpm() {
    let c = BpmChange {
        measure: 1,
        bpm: 999.0,
        fraction: 0.0,
    };
    assert_eq!(c.bpm, 999.0);
}

#[test]
fn bpm_change_low_bpm() {
    let c = BpmChange {
        measure: 1,
        bpm: 30.0,
        fraction: 0.0,
    };
    assert_eq!(c.bpm, 30.0);
}

#[test]
fn bpm_change_very_high() {
    let c = BpmChange {
        measure: 5,
        bpm: 1000.0,
        fraction: 0.0,
    };
    assert_eq!(c.bpm, 1000.0);
}

#[test]
fn bpm_change_very_low() {
    let c = BpmChange {
        measure: 5,
        bpm: 1.0,
        fraction: 0.0,
    };
    assert_eq!(c.bpm, 1.0);
}

#[test]
fn bpm_change_clone() {
    let c = BpmChange {
        measure: 1,
        bpm: 120.0,
        fraction: 0.0,
    };
    let c2 = c;
    assert_eq!(c, c2);
}

#[test]
fn bpm_change_debug() {
    let c = BpmChange {
        measure: 1,
        bpm: 120.0,
        fraction: 0.0,
    };
    let s = format!("{:?}", c);
    assert!(s.contains("BpmChange"));
    assert!(s.contains("120"));
}

#[test]
fn bpm_change_copy() {
    let c = BpmChange {
        measure: 1,
        bpm: 120.0,
        fraction: 0.0,
    };
    let c2 = c;
    let c3 = c;
    assert_eq!(c2.measure, c3.measure);
}
