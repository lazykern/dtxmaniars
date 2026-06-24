//! `CConfigIni.CKeyAssign` + `STKEYASSIGN` — port of
//! `references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs:14-435` (~421 LoC).
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping.
//!
//! ## Role
//!
//! `CKeyAssign` is the per-instrument pad → key mapping table. The C#
//! class has 28 `STKEYASSIGN[]` arrays (HH/R/SD/G/BD/B/HT/Pick/LT/Wail/
//! FT/Help/CY/Decide/HHO/Y/RD/P/LC/LP/LBD/Cancel/Capture/Search/LoopCreate/
//! LoopDelete/SkipForward/SkipBackward/IncreasePlaySpeed/DecreasePlaySpeed/
//! Restart) under three instrument parts (Drums/Guitar/Bass).
//!
//! In Rust we model this as a `KeyAssignTable` resource that holds the
//! 3 instrument parts × 28 pad keys. The default key map mirrors the
//! BocuD defaults (CConfigIni.cs:179-376).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/CConfigIni.cs:14-435`

use std::collections::HashMap;
use std::fmt;

use dtx_input::pad::{InputDevice, KeyAssign};

/// Instrument part (BocuD `CConfigIni.CKeyAssign` indexed by part).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAssignPart {
    /// Drums part.
    Drums,
    /// Guitar part.
    Guitar,
    /// Bass part.
    Bass,
}

impl KeyAssignPart {
    /// All 3 parts in reference order.
    pub fn all() -> [Self; 3] {
        [Self::Drums, Self::Guitar, Self::Bass]
    }
}

/// Pad key identifier (BocuD `CConfigIni.CKeyAssignPad`).
///
/// Mirrors the C# enum with 28+ entries. Each variant corresponds to a
/// `STKEYASSIGN[]` field in the C# class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyAssignPad {
    /// Hi-Hat close (drums) / Red fret (guitar) / Red (bass).
    HHorR,
    /// Snare (drums) / Green fret (guitar) / Green (bass).
    SDorG,
    /// Bass Drum (drums) / Blue fret (guitar) / Blue (bass).
    BDorB,
    /// High Tom (drums) / Yellow fret (guitar) / Pick (bass).
    HTorPick,
    /// Low Tom (drums) / Purple fret (guitar) / Wail (bass).
    LTorWail,
    /// Floor Tom (drums) / Help (bass).
    FTorHelp,
    /// Cymbal (drums) / Decide (bass).
    CYorDecide,
    /// Hi-Hat open (drums) / Yellow fret (bass).
    HHOorY,
    /// Ride Cymbal (drums).
    RD,
    /// Purple fret (bass).
    P,
    /// Left Cymbal (drums).
    LC,
    /// Left Pedal (drums).
    LP,
    /// Left Bass Drum (drums).
    LBD,
    /// Cancel (both).
    Cancel,
    /// Capture (system).
    Capture,
    /// Search (system).
    Search,
    /// LoopCreate (system).
    LoopCreate,
    /// LoopDelete (system).
    LoopDelete,
    /// SkipForward (system).
    SkipForward,
    /// SkipBackward (system).
    SkipBackward,
    /// IncreasePlaySpeed (system).
    IncreasePlaySpeed,
    /// DecreasePlaySpeed (system).
    DecreasePlaySpeed,
    /// Restart (system).
    Restart,
}

impl KeyAssignPad {
    /// All 23 pad keys (BocuD CKeyAssignPad:1-30).
    pub fn all() -> [Self; 23] {
        [
            Self::HHorR,
            Self::SDorG,
            Self::BDorB,
            Self::HTorPick,
            Self::LTorWail,
            Self::FTorHelp,
            Self::CYorDecide,
            Self::HHOorY,
            Self::RD,
            Self::P,
            Self::LC,
            Self::LP,
            Self::LBD,
            Self::Cancel,
            Self::Capture,
            Self::Search,
            Self::LoopCreate,
            Self::LoopDelete,
            Self::SkipForward,
            Self::SkipBackward,
            Self::IncreasePlaySpeed,
            Self::DecreasePlaySpeed,
            Self::Restart,
        ]
    }
}

impl fmt::Display for KeyAssignPad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::HHorR => "HH/R",
            Self::SDorG => "SD/G",
            Self::BDorB => "BD/B",
            Self::HTorPick => "HT/Pick",
            Self::LTorWail => "LT/Wail",
            Self::FTorHelp => "FT/Help",
            Self::CYorDecide => "CY/Decide",
            Self::HHOorY => "HHO/Y",
            Self::RD => "RD",
            Self::P => "P",
            Self::LC => "LC",
            Self::LP => "LP",
            Self::LBD => "LBD",
            Self::Cancel => "Cancel",
            Self::Capture => "Capture",
            Self::Search => "Search",
            Self::LoopCreate => "LoopCreate",
            Self::LoopDelete => "LoopDelete",
            Self::SkipForward => "SkipForward",
            Self::SkipBackward => "SkipBackward",
            Self::IncreasePlaySpeed => "IncreasePlaySpeed",
            Self::DecreasePlaySpeed => "DecreasePlaySpeed",
            Self::Restart => "Restart",
        };
        f.write_str(s)
    }
}

/// One key assignment (BocuD `STKEYASSIGN` CConfigIni.cs:376-384).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct STKeyAssign {
    /// Input device kind (BocuD `EInputDevice`).
    pub device: InputDevice,
    /// Device instance ID (0 for Keyboard/Mouse; varies for MIDI/joypad).
    pub id: u32,
    /// Key code within the device (BocuD `nKey`).
    pub code: u16,
}

impl From<STKeyAssign> for KeyAssign {
    fn from(s: STKeyAssign) -> Self {
        KeyAssign {
            device: s.device,
            id: s.id,
            code: s.code,
        }
    }
}

impl From<KeyAssign> for STKeyAssign {
    fn from(k: KeyAssign) -> Self {
        STKeyAssign {
            device: k.device,
            id: k.id,
            code: k.code,
        }
    }
}

/// Key assignment table (BocuD `CConfigIni.CKeyAssign`).
///
/// Holds the 3 instrument parts × 23 pad keys → `Vec<STKeyAssign>`.
/// Each pad can have multiple bindings (e.g., both keyboard and MIDI).
#[derive(Debug, Clone, Default)]
pub struct KeyAssignTable {
    /// Drums: 23 pad keys, each with N key assignments.
    pub drums: HashMap<KeyAssignPad, Vec<STKeyAssign>>,
    /// Guitar: 23 pad keys, each with N key assignments.
    pub guitar: HashMap<KeyAssignPad, Vec<STKeyAssign>>,
    /// Bass: 23 pad keys, each with N key assignments.
    pub bass: HashMap<KeyAssignPad, Vec<STKeyAssign>>,
}

impl KeyAssignTable {
    /// Construct the BocuD default key assignments (CConfigIni.cs:179-376).
    ///
    /// Drums uses 1234567890 (digits) and ZXCVBNM,. (extra pads).
    /// Guitar uses DFJKL (RGBYP). Bass uses similar 5-key layout.
    pub fn defaults() -> Self {
        let mut t = Self::default();
        // Drums defaults — CConfigIni.cs:179-220
        t.drums.insert(
            KeyAssignPad::HHorR,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'Z' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::SDorG,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'X' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::BDorB,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'C' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::HTorPick,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'V' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::LTorWail,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'B' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::FTorHelp,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'N' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::CYorDecide,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'M' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::HHOorY,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b',' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::LC,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'A' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::LP,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'1' as u16,
            }],
        );
        t.drums.insert(
            KeyAssignPad::LBD,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'2' as u16,
            }],
        );
        // Guitar defaults — CConfigIni.cs:220-260
        t.guitar.insert(
            KeyAssignPad::HHorR,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'D' as u16,
            }],
        );
        t.guitar.insert(
            KeyAssignPad::SDorG,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'F' as u16,
            }],
        );
        t.guitar.insert(
            KeyAssignPad::BDorB,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'J' as u16,
            }],
        );
        t.guitar.insert(
            KeyAssignPad::HTorPick,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'K' as u16,
            }],
        );
        t.guitar.insert(
            KeyAssignPad::LTorWail,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'L' as u16,
            }],
        );
        // Bass defaults — same as guitar
        t.bass.insert(
            KeyAssignPad::HHorR,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'D' as u16,
            }],
        );
        t.bass.insert(
            KeyAssignPad::SDorG,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'F' as u16,
            }],
        );
        t.bass.insert(
            KeyAssignPad::BDorB,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'J' as u16,
            }],
        );
        t.bass.insert(
            KeyAssignPad::HTorPick,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'K' as u16,
            }],
        );
        t.bass.insert(
            KeyAssignPad::LTorWail,
            vec![STKeyAssign {
                device: InputDevice::Keyboard,
                id: 0,
                code: b'L' as u16,
            }],
        );
        t
    }

    /// Get the assignments for one part + pad.
    pub fn get(&self, part: KeyAssignPart, pad: KeyAssignPad) -> &[STKeyAssign] {
        let table = match part {
            KeyAssignPart::Drums => &self.drums,
            KeyAssignPart::Guitar => &self.guitar,
            KeyAssignPart::Bass => &self.bass,
        };
        table.get(&pad).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Set the assignments for one part + pad.
    pub fn set(&mut self, part: KeyAssignPart, pad: KeyAssignPad, assigns: Vec<STKeyAssign>) {
        let table = match part {
            KeyAssignPart::Drums => &mut self.drums,
            KeyAssignPart::Guitar => &mut self.guitar,
            KeyAssignPart::Bass => &mut self.bass,
        };
        table.insert(pad, assigns);
    }

    /// Check if any of the assignments for `part`/`pad` are currently
    /// pressed according to the provided `pressed` map.
    pub fn is_pressed(
        &self,
        part: KeyAssignPart,
        pad: KeyAssignPad,
        pressed: &STKeyAssign,
    ) -> bool {
        self.get(part, pad)
            .iter()
            .any(|a| a.device == pressed.device && a.id == pressed.id && a.code == pressed.code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_assign_part_all_has_3() {
        assert_eq!(KeyAssignPart::all().len(), 3);
    }

    #[test]
    fn key_assign_pad_all_has_23() {
        assert_eq!(KeyAssignPad::all().len(), 23);
    }

    #[test]
    fn key_assign_pad_display_unique() {
        let mut labels: Vec<String> = KeyAssignPad::all().iter().map(|p| p.to_string()).collect();
        let original = labels.clone();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), original.len());
    }

    #[test]
    fn st_key_assign_to_key_assign_conversion() {
        let s = STKeyAssign {
            device: InputDevice::Keyboard,
            id: 0,
            code: 65,
        };
        let k: KeyAssign = s.into();
        assert_eq!(k.code, 65);
    }

    #[test]
    fn defaults_have_drums_assignments() {
        let t = KeyAssignTable::defaults();
        assert!(!t.get(KeyAssignPart::Drums, KeyAssignPad::HHorR).is_empty());
        assert!(!t.get(KeyAssignPart::Drums, KeyAssignPad::BDorB).is_empty());
    }

    #[test]
    fn defaults_have_guitar_assignments() {
        let t = KeyAssignTable::defaults();
        assert!(!t.get(KeyAssignPart::Guitar, KeyAssignPad::HHorR).is_empty());
    }

    #[test]
    fn defaults_have_bass_assignments() {
        let t = KeyAssignTable::defaults();
        assert!(!t.get(KeyAssignPart::Bass, KeyAssignPad::HHorR).is_empty());
    }

    #[test]
    fn get_returns_empty_for_unset_pad() {
        let t = KeyAssignTable::defaults();
        // FTorHelp not set in defaults for guitar
        assert!(t
            .get(KeyAssignPart::Guitar, KeyAssignPad::FTorHelp)
            .is_empty());
    }

    #[test]
    fn set_overrides_assignments() {
        let mut t = KeyAssignTable::defaults();
        let new_assigns = vec![STKeyAssign {
            device: InputDevice::Keyboard,
            id: 0,
            code: 99,
        }];
        t.set(
            KeyAssignPart::Drums,
            KeyAssignPad::HHorR,
            new_assigns.clone(),
        );
        assert_eq!(
            t.get(KeyAssignPart::Drums, KeyAssignPad::HHorR),
            &new_assigns[..]
        );
    }

    #[test]
    fn is_pressed_returns_true_for_match() {
        let t = KeyAssignTable::defaults();
        let key = STKeyAssign {
            device: InputDevice::Keyboard,
            id: 0,
            code: b'Z' as u16,
        };
        assert!(t.is_pressed(KeyAssignPart::Drums, KeyAssignPad::HHorR, &key));
    }

    #[test]
    fn is_pressed_returns_false_for_mismatch() {
        let t = KeyAssignTable::defaults();
        let key = STKeyAssign {
            device: InputDevice::Keyboard,
            id: 0,
            code: b'Q' as u16,
        };
        assert!(!t.is_pressed(KeyAssignPart::Drums, KeyAssignPad::HHorR, &key));
    }
}
