#![allow(non_snake_case)]
//! `CBoxDef` (260 LOC) + `CSetDef` (240 LOC) — chip-pattern definitions.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CBoxDef.cs:1-260`
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CSetDef.cs:1-240`
//!
//! CBoxDef: a single 36-bit chip pattern + delay used to express
//! "a chip at position X with delay Y". In the DTX format chips are
//! described in compact box units.
//!
//! CSetDef: a collection of CBoxDef instances, defining a measure's
//! chip pattern (a "set" in BocuD terminology).

/// Maximum boxes per set (BocuD CSetDef.cs:50).
pub const MAX_BOXES_PER_SET: usize = 4000;

/// One box: position (0-35) + value + delay (BocuD CBoxDef.cs:30-50).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CBoxDef {
    /// Position in 36-cell measure (0-35).
    pub position: u8,
    /// Optional chip value (WAV index for SE channels, BGA layer for BGA).
    pub nValue: i32,
    /// Delay in 1/36th measure units.
    pub nDelay: u16,
}

impl CBoxDef {
    /// Build a new box at a specific position with a value.
    pub fn new(position: u8, nValue: i32) -> Self {
        Self {
            position,
            nValue,
            nDelay: 0,
        }
    }

    /// Convert the (position, delay) pair to a fraction (0.0..1.0) of a measure.
    ///
    /// Reference: `CBoxDef.cs:GetTime` — total_steps = 36 + delay, fraction = pos / total.
    pub fn fraction(&self) -> f32 {
        let total_steps = 36 + self.nDelay as u32;
        if total_steps == 0 {
            return 0.0;
        }
        self.position as f32 / total_steps as f32
    }

    /// Convert the box to absolute chip time in ms at a given base BPM.
    pub fn time_ms(&self, measure: u32, base_bpm: f32) -> i64 {
        let measure_ms = (60000.0 / base_bpm) * 4.0;
        let ms = measure as f64 * measure_ms as f64 + self.fraction() as f64 * measure_ms as f64;
        ms as i64
    }
}

/// One set: a measure's worth of boxes (BocuD CSetDef.cs:30-60).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CSetDef {
    /// Boxes in this set.
    pub boxes: Vec<CBoxDef>,
}

impl CSetDef {
    /// Empty set.
    pub fn new() -> Self {
        Self { boxes: Vec::new() }
    }

    /// Add a box to the set.
    pub fn add(&mut self, bx: CBoxDef) {
        if self.boxes.len() < MAX_BOXES_PER_SET {
            self.boxes.push(bx);
        }
    }

    /// Number of boxes.
    pub fn count(&self) -> usize {
        self.boxes.len()
    }

    /// Find boxes at a specific position.
    pub fn boxes_at(&self, position: u8) -> impl Iterator<Item = &CBoxDef> {
        self.boxes.iter().filter(move |b| b.position == position)
    }

    /// Find boxes with the given value (e.g. all BGA-7 chips).
    pub fn boxes_with_value(&self, value: i32) -> impl Iterator<Item = &CBoxDef> {
        self.boxes.iter().filter(move |b| b.nValue == value)
    }

    /// Highest position used (max 35).
    pub fn last_position(&self) -> Option<u8> {
        self.boxes.iter().map(|b| b.position).max()
    }

    /// All unique values across the set (BocuD CSetDef.cs:GetUniqueValues).
    pub fn unique_values(&self) -> Vec<i32> {
        let mut seen: Vec<i32> = self.boxes.iter().map(|b| b.nValue).collect();
        seen.sort_unstable();
        seen.dedup();
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_box_def_new() {
        let b = CBoxDef::new(0, 1);
        assert_eq!(b.position, 0);
        assert_eq!(b.nValue, 1);
        assert_eq!(b.nDelay, 0);
    }

    #[test]
    fn c_box_def_fraction_no_delay() {
        // 0/36 = 0.0
        assert_eq!(CBoxDef::new(0, 0).fraction(), 0.0);
        // 18/36 = 0.5
        assert!((CBoxDef::new(18, 0).fraction() - 0.5).abs() < 0.01);
        // 36/36 = 1.0 (but position is 0..=35 normally; test edge)
        assert!((CBoxDef::new(36, 0).fraction() - 1.0).abs() < 0.01);
    }

    #[test]
    fn c_box_def_fraction_with_delay() {
        // 0/72 (delay 36) = 0.0
        let b = CBoxDef {
            position: 0,
            nValue: 0,
            nDelay: 36,
        };
        assert_eq!(b.fraction(), 0.0);
        // 18/72 = 0.25
        let b = CBoxDef {
            position: 18,
            nValue: 0,
            nDelay: 36,
        };
        assert!((b.fraction() - 0.25).abs() < 0.01);
    }

    #[test]
    fn c_box_def_time_ms_120bpm() {
        // 120 BPM = 2000ms/measure, 1/36th = 55.55ms
        let b = CBoxDef::new(0, 1);
        assert_eq!(b.time_ms(0, 120.0), 0);
        let b = CBoxDef::new(36, 1);
        assert_eq!(b.time_ms(0, 120.0), 2000);
    }

    #[test]
    fn c_set_def_add_count() {
        let mut s = CSetDef::new();
        s.add(CBoxDef::new(0, 1));
        s.add(CBoxDef::new(18, 2));
        assert_eq!(s.count(), 2);
    }

    #[test]
    fn c_set_def_max_boxes() {
        let mut s = CSetDef::new();
        for i in 0..MAX_BOXES_PER_SET + 100 {
            s.add(CBoxDef::new((i % 36) as u8, i as i32));
        }
        assert_eq!(s.count(), MAX_BOXES_PER_SET);
    }

    #[test]
    fn c_set_def_boxes_at() {
        let mut s = CSetDef::new();
        s.add(CBoxDef::new(5, 1));
        s.add(CBoxDef::new(5, 2));
        s.add(CBoxDef::new(6, 1));
        let at_5: Vec<_> = s.boxes_at(5).collect();
        assert_eq!(at_5.len(), 2);
    }

    #[test]
    fn c_set_def_boxes_with_value() {
        let mut s = CSetDef::new();
        s.add(CBoxDef::new(0, 7));
        s.add(CBoxDef::new(1, 8));
        s.add(CBoxDef::new(2, 7));
        let v7: Vec<_> = s.boxes_with_value(7).collect();
        assert_eq!(v7.len(), 2);
    }

    #[test]
    fn c_set_def_last_position() {
        let mut s = CSetDef::new();
        assert_eq!(s.last_position(), None);
        s.add(CBoxDef::new(10, 1));
        s.add(CBoxDef::new(20, 2));
        s.add(CBoxDef::new(5, 3));
        assert_eq!(s.last_position(), Some(20));
    }

    #[test]
    fn c_set_def_unique_values_sorted() {
        let mut s = CSetDef::new();
        s.add(CBoxDef::new(0, 3));
        s.add(CBoxDef::new(1, 1));
        s.add(CBoxDef::new(2, 3));
        s.add(CBoxDef::new(3, 2));
        s.add(CBoxDef::new(4, 1));
        let v = s.unique_values();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn max_boxes_per_set_constant() {
        // CSetDef.cs:50 — 最大4000
        assert_eq!(MAX_BOXES_PER_SET, 4000);
    }
}
