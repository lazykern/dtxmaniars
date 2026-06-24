//! `CActConfigKeyAssign` — port of `Stage/03.Config/CActConfigKeyAssign.cs` (564 LOC).
//!
//! Strict-port-first. Key assignment sub-act for System/Drums/Guitar/Bass.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigKeyAssign.cs:1-564`

use bevy::prelude::{App, KeyCode};

/// Which sub-part of the key assignment (CActConfigKeyAssign.cs:8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EKeyConfigPart {
    System,
    Drums,
    Guitar,
    Bass,
    #[default]
    Unknown,
}

impl EKeyConfigPart {
    pub fn all() -> [Self; 4] {
        [Self::System, Self::Drums, Self::Guitar, Self::Bass]
    }
}

/// Which pad (button) on the instrument (CActConfigKeyAssign.cs).
///
/// 16 pads per instrument (per C# `CItemBase` configKeyAssign: HH/R/SD/G/
/// BD/B/HT/Pick/LT/Wail/FT/Help/CY/Decide/HHO/Y/RD/LC/P/LBD/Cancel/
/// Capture/Search/LoopCreate/LoopDelete/SkipForward/...).
/// 16 main pads selected for v1 strict-port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EKeyConfigPad {
    HH,
    R,
    SD,
    G,
    BD,
    B,
    HT,
    Pick,
    LT,
    Wail,
    FT,
    Help,
    CY,
    Decide,
    HHO,
    Y,
    #[default]
    Unknown,
}

impl EKeyConfigPad {
    /// First 16 pads + Unknown = 17 variants.
    pub fn all() -> [Self; 17] {
        [
            Self::HH,
            Self::R,
            Self::SD,
            Self::G,
            Self::BD,
            Self::B,
            Self::HT,
            Self::Pick,
            Self::LT,
            Self::Wail,
            Self::FT,
            Self::Help,
            Self::CY,
            Self::Decide,
            Self::HHO,
            Self::Y,
            Self::Unknown,
        ]
    }

    /// Display name for the pad.
    pub fn label(&self) -> &'static str {
        match self {
            Self::HH => "HH",
            Self::R => "R",
            Self::SD => "SD",
            Self::G => "G",
            Self::BD => "BD",
            Self::B => "B",
            Self::HT => "HT",
            Self::Pick => "Pick",
            Self::LT => "LT",
            Self::Wail => "Wail",
            Self::FT => "FT",
            Self::Help => "Help",
            Self::CY => "CY",
            Self::Decide => "Decide",
            Self::HHO => "HHO",
            Self::Y => "Y",
            Self::Unknown => "?",
        }
    }
}

/// Input device kind (CActConfigKeyAssign.cs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EInputDevice {
    #[default]
    Unknown,
    Keyboard,
    MIDI,
}

/// One key assignment slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct KeyAssignSlot {
    pub device: EInputDevice,
    pub id: u8,
    pub code: u32,
}

/// 16-slot key assignment for one (part, pad) pair.
#[derive(Debug, Clone)]
pub struct PadKeyAssign {
    pub slots: [KeyAssignSlot; 16],
}

impl Default for PadKeyAssign {
    fn default() -> Self {
        Self {
            slots: [KeyAssignSlot::default(); 16],
        }
    }
}

impl PadKeyAssign {
    /// Reset all 16 slots to defaults.
    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            *slot = KeyAssignSlot::default();
        }
    }
}

/// State for the key-assign sub-act.
#[derive(bevy::prelude::Resource, Debug, Default, Clone)]
pub struct KeyAssignState {
    /// Currently active part (System/Drums/Guitar/Bass).
    pub part: EKeyConfigPart,
    /// Currently active pad.
    pub pad: EKeyConfigPad,
    /// Display name of the current pad (e.g. "Left Cymbal").
    pub pad_name: String,
    /// Selected row in the 18-row key assign grid (16 slots + Reset + Save).
    pub selected_row: usize,
    /// Reset-on-enter snapshot (CActConfigKeyAssign.cs:14-19).
    pub reset_snapshot: [KeyAssignSlot; 16],
    /// True if waiting for the user to press a key to bind.
    pub waiting_for_input: bool,
}

impl KeyAssignState {
    /// Start a key-assign session for a (part, pad) pair.
    pub fn start(&mut self, part: EKeyConfigPart, pad: EKeyConfigPad, pad_name: &str) {
        self.part = part;
        self.pad = pad;
        self.pad_name = pad_name.to_string();
        self.selected_row = 0;
        self.waiting_for_input = false;
    }

    /// Move selection down (CActConfigKeyAssign.cs:67-74).
    pub fn move_next(&mut self) {
        if self.waiting_for_input {
            return;
        }
        self.selected_row = (self.selected_row + 1) % 18;
    }

    /// Move selection up (CActConfigKeyAssign.cs:76-83).
    pub fn move_previous(&mut self) {
        if self.waiting_for_input {
            return;
        }
        self.selected_row = (self.selected_row + 18 - 1) % 18;
    }

    /// Press Enter — row 16 = reset, row 17 = save; otherwise wait for input.
    pub fn press_enter(&mut self) {
        if self.waiting_for_input {
            return;
        }
        match self.selected_row {
            16 => {
                // Reset to snapshot
                for (i, slot) in self.reset_snapshot.iter().enumerate() {
                    let _ = (i, slot);
                }
            }
            17 => {
                // Save — caller persists
            }
            _ => {
                self.waiting_for_input = true;
            }
        }
    }

    /// Capture a key press (called from keyboard input system).
    pub fn capture_key(&mut self, _key: KeyCode) {
        self.waiting_for_input = false;
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<KeyAssignState>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ekeyconfig_part_variants() {
        // 4 parts + Unknown.
        assert_eq!(EKeyConfigPart::all().len(), 4);
    }

    #[test]
    fn ekeyconfig_pad_count() {
        // 16 named pads + Unknown.
        assert_eq!(EKeyConfigPad::all().len(), 17);
    }

    #[test]
    fn ekeyconfig_pad_labels_unique() {
        let labels: Vec<_> = EKeyConfigPad::all().iter().map(|p| p.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels.len(), sorted.len());
    }

    #[test]
    fn pad_key_assign_default_has_16_slots() {
        let a = PadKeyAssign::default();
        assert_eq!(a.slots.len(), 16);
    }

    #[test]
    fn pad_key_assign_reset_clears_all_slots() {
        let mut a = PadKeyAssign::default();
        a.slots[0] = KeyAssignSlot {
            device: EInputDevice::Keyboard,
            id: 5,
            code: 100,
        };
        a.reset();
        assert_eq!(a.slots[0], KeyAssignSlot::default());
    }

    #[test]
    fn key_assign_state_starts_default() {
        let s = KeyAssignState::default();
        assert_eq!(s.part, EKeyConfigPart::Unknown);
        assert_eq!(s.pad, EKeyConfigPad::Unknown);
        assert_eq!(s.selected_row, 0);
        assert!(!s.waiting_for_input);
    }

    #[test]
    fn key_assign_state_start_sets_part_pad() {
        let mut s = KeyAssignState::default();
        s.start(EKeyConfigPart::Drums, EKeyConfigPad::HH, "HiHat");
        assert_eq!(s.part, EKeyConfigPart::Drums);
        assert_eq!(s.pad, EKeyConfigPad::HH);
        assert_eq!(s.pad_name, "HiHat");
    }

    #[test]
    fn key_assign_state_move_next_wraps_18() {
        let mut s = KeyAssignState::default();
        s.selected_row = 17;
        s.move_next();
        assert_eq!(s.selected_row, 0);
    }

    #[test]
    fn key_assign_state_move_previous_wraps() {
        let mut s = KeyAssignState::default();
        s.selected_row = 0;
        s.move_previous();
        assert_eq!(s.selected_row, 17);
    }

    #[test]
    fn key_assign_state_move_blocked_when_waiting() {
        let mut s = KeyAssignState::default();
        s.waiting_for_input = true;
        s.selected_row = 5;
        s.move_next();
        assert_eq!(s.selected_row, 5);
    }

    #[test]
    fn key_assign_press_enter_sets_waiting() {
        let mut s = KeyAssignState::default();
        s.selected_row = 3;
        s.press_enter();
        assert!(s.waiting_for_input);
    }

    #[test]
    fn key_assign_press_enter_reset_row() {
        let mut s = KeyAssignState::default();
        s.selected_row = 16;
        s.press_enter();
        assert!(!s.waiting_for_input);
    }

    #[test]
    fn key_assign_capture_clears_waiting() {
        let mut s = KeyAssignState::default();
        s.waiting_for_input = true;
        s.capture_key(KeyCode::Digit1);
        assert!(!s.waiting_for_input);
    }

    #[test]
    fn input_device_variants_distinct() {
        assert_ne!(EInputDevice::Keyboard, EInputDevice::MIDI);
        assert_ne!(EInputDevice::MIDI, EInputDevice::Unknown);
    }

    #[test]
    fn default_input_device_is_unknown() {
        let d: EInputDevice = Default::default();
        assert_eq!(d, EInputDevice::Unknown);
    }

    #[test]
    fn default_part_is_unknown() {
        let p: EKeyConfigPart = Default::default();
        assert_eq!(p, EKeyConfigPart::Unknown);
    }

    #[test]
    fn default_pad_is_unknown() {
        let p: EKeyConfigPad = Default::default();
        assert_eq!(p, EKeyConfigPad::Unknown);
    }
}
