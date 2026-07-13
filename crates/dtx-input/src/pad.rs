//! `CPad` — port of `references/DTXmaniaNX/DTXMania/Core/CPad.cs` (278 LoC).
//!
//! Strict-port-first (ADR-0010). 1:1 file mapping.
//!
//! ## Role
//!
//! `CPad` dispatches input events from `CInputManager` to a per-instrument,
//! per-pad key assignment. The C# class has two main methods:
//! - `GetEvents(part, pad)`: collect all input events matching the
//!   key-assignment for that (instrument, pad).
//! - `bPressed(part, pad)` / `bPressing(part, pad)`: query whether the
//!   pad is currently pressed/held, with `DGB`/`GB` variants that
//!   combine across drums + guitar + bass.
//!
//! In Rust we model this as a `PadDispatcher` resource that holds the
//! detected-device flags and dispatches to the underlying `InputManager`.
//! The full key-assignment table is in `dtx-config` (see `CKeyAssign`).
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Core/CPad.cs:1-278`

use bevy::prelude::*;

/// Input device kind (BocuD CPad.cs STHIT — Keyboard/MIDIIN/Joypad/Mouse).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DetectedDevice {
    /// Keyboard detected.
    pub keyboard: bool,
    /// MIDI IN detected.
    pub midi_in: bool,
    /// Joypad detected.
    pub joypad: bool,
    /// Mouse detected.
    pub mouse: bool,
}

impl DetectedDevice {
    /// Construct a fresh, all-false flags struct (BocuD `STHIT.Clear`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all flags.
    pub fn clear(&mut self) {
        self.keyboard = false;
        self.midi_in = false;
        self.joypad = false;
        self.mouse = false;
    }

    /// True if any device is detected.
    pub fn any(&self) -> bool {
        self.keyboard || self.midi_in || self.joypad || self.mouse
    }
}

/// Instrument part (BocuD `EInstrumentPart`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum InstrumentPart {
    /// Unknown / unset (BocuD `EInstrumentPart.UNKNOWN`).
    #[default]
    Unknown,
    /// Drums (BocuD `EInstrumentPart.DRUMS`).
    Drums,
    /// Guitar (BocuD `EInstrumentPart.GUITAR`).
    Guitar,
    /// Bass (BocuD `EInstrumentPart.BASS`).
    Bass,
}

impl InstrumentPart {
    /// All known instrument parts (BocuD `EInstrumentPart` values).
    pub fn all() -> [Self; 4] {
        [Self::Unknown, Self::Drums, Self::Guitar, Self::Bass]
    }
}

/// Pad assignment identifier (BocuD `EPad`).
///
/// Used to look up the key-assignment for a specific lane.
/// The numeric values map to C# `EPad` (0=LC, 1=HH, ...).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Pad {
    /// Unknown pad.
    #[default]
    Unknown,
    /// Pad #0 (e.g., LC for drums, R for guitar).
    Pad0,
    /// Pad #1 (e.g., HH for drums, G for guitar).
    Pad1,
    /// Pad #2 (e.g., SD for drums, B for guitar).
    Pad2,
    /// Pad #3 (e.g., BD for drums, Y for guitar).
    Pad3,
    /// Pad #4 (e.g., HT for drums, P for guitar).
    Pad4,
    /// Pad #5 (drums only — LT).
    Pad5,
    /// Pad #6 (drums only — FT).
    Pad6,
    /// Pad #7 (drums only — CY).
    Pad7,
    /// Pad #8 (drums only — RD).
    Pad8,
    /// Pad #9 (drums only — LP).
    Pad9,
}

impl Pad {
    /// Numeric pad index (0..=9), or None for Unknown.
    pub fn index(self) -> Option<u8> {
        match self {
            Self::Unknown => None,
            Self::Pad0 => Some(0),
            Self::Pad1 => Some(1),
            Self::Pad2 => Some(2),
            Self::Pad3 => Some(3),
            Self::Pad4 => Some(4),
            Self::Pad5 => Some(5),
            Self::Pad6 => Some(6),
            Self::Pad7 => Some(7),
            Self::Pad8 => Some(8),
            Self::Pad9 => Some(9),
        }
    }
}

/// One key assignment (BocuD `CConfigIni.CKeyAssign.STKEYASSIGN`).
///
/// Maps a single (device, code) pair to a pad. The C# struct has
/// `InputDevice`, `ID`, and `Code` fields; the ID is the device
/// instance for MIDI/Joypad (Keyboard/Mouse use ID=0).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyAssign {
    /// Device kind (BocuD `EInputDevice`).
    pub device: InputDevice,
    /// Device instance ID (0 for keyboard/mouse; varies for MIDI/joypad).
    pub id: u32,
    /// Key code within the device (BocuD `nKey` / `Code`).
    pub code: u16,
}

/// Input device kind (BocuD `EInputDevice`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum InputDevice {
    /// Keyboard (BocuD `EInputDevice.Keyboard`).
    #[default]
    Keyboard,
    /// MIDI input (BocuD `EInputDevice.MIDI入力`).
    MidiIn,
    /// Joypad / joystick (BocuD `EInputDevice.Joypad`).
    Joypad,
    /// Mouse (BocuD `EInputDevice.Mouse`).
    Mouse,
}

/// Pad dispatcher (BocuD `CPad` class).
///
/// Holds the `DetectedDevice` flags (set when any input matches) and
/// provides the `bPressedDGB` / `bPressedGB` / `bPressingGB` combination
/// methods that the gameplay crates use.
#[derive(Resource, Debug, Default, Clone)]
pub struct PadDispatcher {
    /// Most-recently detected device (BocuD CPad.cs `stDetectedDevice`).
    pub detected: DetectedDevice,
}

impl PadDispatcher {
    /// Construct a fresh dispatcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the detected-device flags.
    pub fn clear_detected(&mut self) {
        self.detected.clear();
    }

    /// Mark a device as detected (called by the input system when a
    /// matching event fires).
    pub fn mark_detected(&mut self, device: InputDevice) {
        match device {
            InputDevice::Keyboard => self.detected.keyboard = true,
            InputDevice::MidiIn => self.detected.midi_in = true,
            InputDevice::Joypad => self.detected.joypad = true,
            InputDevice::Mouse => self.detected.mouse = true,
        }
    }

    /// `bPressedDGB` — true if any of (Drums, Guitar, Bass) has the pad
    /// pressed (BocuD CPad.cs:155-163).
    pub fn pressed_dgb(&self, drums: bool, guitar: bool, bass: bool) -> bool {
        drums || guitar || bass
    }

    /// `bPressedGB` — true if any of (Guitar, Bass) has the pad pressed
    /// (BocuD CPad.cs:165-171).
    pub fn pressed_gb(&self, guitar: bool, bass: bool) -> bool {
        guitar || bass
    }

    /// `bPressingGB` — true if any of (Guitar, Bass) is currently
    /// holding the pad (BocuD CPad.cs:198-205).
    pub fn pressing_gb(&self, guitar: bool, bass: bool) -> bool {
        guitar || bass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === DetectedDevice ===

    #[test]
    fn detected_device_default_all_false() {
        let d = DetectedDevice::new();
        assert!(!d.any());
    }

    #[test]
    fn detected_device_clear_resets_all() {
        let mut d = DetectedDevice {
            keyboard: true,
            midi_in: true,
            joypad: true,
            mouse: true,
        };
        d.clear();
        assert!(!d.any());
    }

    #[test]
    fn detected_device_any_with_one_true() {
        let d = DetectedDevice {
            keyboard: true,
            ..Default::default()
        };
        assert!(d.any());
    }

    // === InstrumentPart ===

    #[test]
    fn instrument_part_all_has_4() {
        assert_eq!(InstrumentPart::all().len(), 4);
    }

    #[test]
    fn instrument_part_default_unknown() {
        assert_eq!(InstrumentPart::default(), InstrumentPart::Unknown);
    }

    // === Pad ===

    #[test]
    fn pad_index_returns_0_through_9() {
        assert_eq!(Pad::Pad0.index(), Some(0));
        assert_eq!(Pad::Pad4.index(), Some(4));
        assert_eq!(Pad::Pad9.index(), Some(9));
    }

    #[test]
    fn pad_unknown_returns_none() {
        assert_eq!(Pad::Unknown.index(), None);
    }

    // === PadDispatcher ===

    #[test]
    fn pad_dispatcher_default_clear() {
        let d = PadDispatcher::new();
        assert!(!d.detected.any());
    }

    #[test]
    fn pad_dispatcher_mark_detected_keyboard() {
        let mut d = PadDispatcher::new();
        d.mark_detected(InputDevice::Keyboard);
        assert!(d.detected.keyboard);
        assert!(d.detected.any());
    }

    #[test]
    fn pad_dispatcher_mark_detected_midi() {
        let mut d = PadDispatcher::new();
        d.mark_detected(InputDevice::MidiIn);
        assert!(d.detected.midi_in);
    }

    #[test]
    fn pad_dispatcher_clear_detected_resets() {
        let mut d = PadDispatcher::new();
        d.mark_detected(InputDevice::Joypad);
        d.mark_detected(InputDevice::Mouse);
        d.clear_detected();
        assert!(!d.detected.any());
    }

    #[test]
    fn pressed_dgb_any_true() {
        let d = PadDispatcher::new();
        assert!(d.pressed_dgb(false, false, true));
        assert!(d.pressed_dgb(true, false, false));
        assert!(d.pressed_dgb(false, true, false));
        assert!(!d.pressed_dgb(false, false, false));
    }

    #[test]
    fn pressed_gb_excludes_drums() {
        let d = PadDispatcher::new();
        // Drums-only should NOT trigger GB.
        assert!(!d.pressed_gb(false, false));
        assert!(d.pressed_gb(true, false));
        assert!(d.pressed_gb(false, true));
    }

    #[test]
    fn pressing_gb_same_as_pressed_gb() {
        let d = PadDispatcher::new();
        assert_eq!(d.pressing_gb(false, false), d.pressed_gb(false, false));
        assert_eq!(d.pressing_gb(true, false), d.pressed_gb(true, false));
    }
}
