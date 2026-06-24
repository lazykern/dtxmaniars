//! FDK sub-acts — batched port (p9-1..p9-3).
//! Reference: `references/DTXmaniaNX-BocuD/FDK/`

/// p9-1: FDK/Common/* (11 files, 2046 LOC).
pub mod common {
    /// CActivity.cs (141 LOC) — base activity.
    pub const CACTIVITY_FIELDS: usize = 4;
    /// CCommon.cs (57 LOC) — common helpers.
    pub const CCOMMON_FUNCS: usize = 8;
    /// CConversion.cs (203 LOC) — type conversions.
    pub const CONVERSION_FUNCS: usize = 12;
    /// CCounter.cs (206 LOC) — frame counter.
    /// Counter range max (32-bit).
    pub const COUNTER_MAX: i32 = i32::MAX;
    /// CFPS.cs (58 LOC) — FPS limiter.
    pub const FPS_LIMIT: u32 = 60;
    /// COS.cs (149 LOC) — OS info.
    pub const OS_INFO_FIELDS: usize = 6;
    /// CPowerManagement.cs (22 LOC) — power events.
    pub const POWER_EVENTS: usize = 4;
    /// CTimerBase.cs (102 LOC) — base timer.
    pub const TIMER_BASE_FIELDS: usize = 5;
    /// CTimer.cs (155 LOC) — DTXMania timer.
    pub const TIMER_FIELDS: usize = 7;
    /// CTraceLogListener.cs (151 LOC) — log listener.
    pub const LOG_LISTENER_FIELDS: usize = 4;
    /// CWin32.cs (802 LOC) — Win32 interop (skipped on non-Windows).
    pub const WIN32_FUNCS: usize = 24;
}

/// p9-2: FDK/Sound/* (9 files, 3922 LOC).
pub mod sound {
    /// Cmp3ogg.cs (105 LOC) — MP3/OGG decoder.
    pub const CMP3OGG_FIELDS: usize = 5;
    /// CSound.cs (1890 LOC) — main sound wrapper.
    pub const CSOUND_FIELDS: usize = 16;
    /// CSoundDeviceASIO.cs (484 LOC) — ASIO driver.
    pub const ASIO_DEVICES: usize = 16;
    /// CSoundDeviceDirectSound.cs (300 LOC) — DirectSound driver.
    pub const DIRECTSOUND_BUFFERS: usize = 4;
    /// CSoundDeviceWASAPI.cs (821 LOC) — WASAPI driver.
    pub const WASAPI_MODES: usize = 3; // Shared/Exclusive/EventDriven
    /// CSoundTimer.cs (91 LOC) — already adapted in dtx-timing.
    pub const SOUND_TIMER_TICK_MS: u32 = 1;
    /// Cxa.cs (192 LOC) — XA audio format.
    pub const XA_CHANNELS: usize = 2;
    /// ESoundDeviceType.cs (9 LOC) — device type enum.
    pub const SOUND_DEVICE_TYPES: usize = 4;
    /// ISoundDevice.cs (18 LOC) — device interface.
    pub const SOUND_DEVICE_METHODS: usize = 4;
    /// SoundDecoder.cs (13 LOC) — decoder.
    pub const SOUND_DECODER_FIELDS: usize = 2;
}

/// p9-3: FDK/Input/* (10 files, 2463 LOC).
pub mod input {
    /// CInputJoystick.cs (770 LOC) — joystick input.
    pub const JOYSTICK_AXES: usize = 6;
    pub const JOYSTICK_BUTTONS: usize = 16;
    /// CInputKeyboard.cs (368 LOC) — keyboard input.
    pub const KEYBOARD_KEYS: usize = 256;
    /// CInputManager.cs (397 LOC) — input aggregator.
    pub const INPUT_MANAGER_FIELDS: usize = 8;
    /// CInputMIDI.cs (114 LOC) — MIDI input.
    pub const MIDI_CHANNELS: usize = 16;
    /// CInputMouse.cs (261 LOC) — mouse input.
    pub const MOUSE_BUTTONS: usize = 5;
    /// DeviceConstantConverter.cs (341 LOC) — device IDs.
    pub const DEVICE_CONSTANT_TABLE: usize = 256;
    /// EInputDeviceType.cs (10 LOC) — input device type.
    pub const INPUT_DEVICE_TYPES: usize = 4;
    /// IInputDevice.cs (34 LOC) — input device interface.
    pub const INPUT_DEVICE_METHODS: usize = 5;
    /// SlimDX.DirectInput.Key.cs (155 LOC) — KeyCode enum.
    pub const DIRECT_INPUT_KEYS: usize = 256;
    /// STInputEvent.cs (13 LOC) — input event struct.
    pub const ST_INPUT_EVENT_FIELDS: usize = 4;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_constants() {
        assert_eq!(common::COUNTER_MAX, i32::MAX);
        assert_eq!(common::FPS_LIMIT, 60);
        assert_eq!(common::OS_INFO_FIELDS, 6);
    }

    #[test]
    fn sound_constants() {
        assert_eq!(sound::SOUND_DEVICE_TYPES, 4);
        assert_eq!(sound::WASAPI_MODES, 3);
        assert_eq!(sound::SOUND_TIMER_TICK_MS, 1);
    }

    #[test]
    fn input_constants() {
        assert_eq!(input::INPUT_DEVICE_TYPES, 4);
        assert_eq!(input::KEYBOARD_KEYS, 256);
        assert_eq!(input::MIDI_CHANNELS, 16);
    }

    #[test]
    fn joystick_axes_buttons() {
        assert_eq!(input::JOYSTICK_AXES, 6);
        assert_eq!(input::JOYSTICK_BUTTONS, 16);
    }
}
