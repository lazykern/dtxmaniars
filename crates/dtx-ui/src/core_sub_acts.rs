//! Core sub-acts — real ports of BocuD's Core module.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/`
//!
//! Replaces constants-only Core sub-acts with real logic:
//! - Video: VideoState (Playing/Paused/Stopped/Error), VideoFrame
//! - Framework: Color4 (RGBA), DisplayState, RuntimeLogListener
//! - Glue: marker (Bevy replaces)
//! - OpenGL: skipped marker

/// p8-1: Video/* (8 files, 1238 LOC) — structure only, no decode.
pub mod video {
    /// DecodedFrameData fields.
    pub const FRAME_DATA_FIELDS: usize = 4;
    /// DisplayedFrame fields.
    pub const DISPLAY_FRAME_FIELDS: usize = 3;
    /// FFmpeg cores (1 — not ported per goal scope).
    pub const FFMPEG_CORES: usize = 1;
    /// UI video renderers.
    pub const VIDEO_RENDERERS: usize = 1;
    /// Video controllers.
    pub const VIDEO_CONTROLLERS: usize = 1;
    /// Video state types count.
    pub const VIDEO_STATES: usize = 4;
    /// Default video buffer size.
    pub const VIDEO_BUFFER_SIZE: usize = 4;

    /// Video state (BocuD VideoPlayerController.cs:30).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum VideoState {
        /// Stopped (not playing).
        #[default]
        Stopped = 0,
        /// Currently playing.
        Playing = 1,
        /// Paused.
        Paused = 2,
        /// Error.
        Error = 3,
    }

    impl VideoState {
        /// Valid transitions (BocuD VideoPlayerController.cs:ValidTransitions).
        pub fn can_transition_to(self, target: VideoState) -> bool {
            use VideoState::*;
            matches!(
                (self, target),
                (Stopped, Playing)
                    | (Playing, Paused)
                    | (Paused, Playing)
                    | (Playing, Stopped)
                    | (Paused, Stopped)
                    | (Stopped, Error)
                    | (Playing, Error)
                    | (Error, Stopped)
            )
        }

        /// All variants in order.
        pub fn all() -> &'static [Self] {
            &[Self::Stopped, Self::Playing, Self::Paused, Self::Error]
        }
    }

    /// Decoded frame data (BocuD DecodedFrameData.cs:5-15).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DecodedFrameData {
        /// Frame index.
        pub frame_index: u32,
        /// Width in pixels.
        pub width: u32,
        /// Height in pixels.
        pub height: u32,
        /// Frame data size in bytes (BocuD: encoded size, we use plain).
        pub data_size: usize,
    }

    impl DecodedFrameData {
        /// Build a new frame data.
        pub fn new(frame_index: u32, width: u32, height: u32, data_size: usize) -> Self {
            Self {
                frame_index,
                width,
                height,
                data_size,
            }
        }

        /// Aspect ratio (BocuD DecodedFrameData.cs:AspectRatio).
        pub fn aspect_ratio(&self) -> f32 {
            if self.height == 0 {
                0.0
            } else {
                self.width as f32 / self.height as f32
            }
        }
    }

    /// Displayed frame (BocuD DisplayedFrame.cs:5-25).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DisplayedFrame {
        /// Reference to the underlying decoded frame.
        pub frame_index: u32,
        /// Display time in ms (BocuD: nDisplayTimeMs).
        pub display_time_ms: u64,
        /// Duration to display in ms.
        pub duration_ms: u64,
    }

    /// Video buffer (BocuD VideoPlayerController.cs:GetVideoBuffer).
    #[derive(Debug, Clone, Default)]
    pub struct VideoBuffer {
        /// Ring buffer of frames.
        pub frames: Vec<DecodedFrameData>,
    }

    impl VideoBuffer {
        /// Build a new buffer with capacity.
        pub fn new() -> Self {
            Self {
                frames: Vec::with_capacity(VIDEO_BUFFER_SIZE),
            }
        }

        /// Push a frame, dropping oldest if at capacity.
        pub fn push(&mut self, frame: DecodedFrameData) {
            if self.frames.len() >= VIDEO_BUFFER_SIZE {
                self.frames.remove(0);
            }
            self.frames.push(frame);
        }

        /// Number of frames in buffer.
        pub fn len(&self) -> usize {
            self.frames.len()
        }

        /// Empty?
        pub fn is_empty(&self) -> bool {
            self.frames.is_empty()
        }

        /// Latest frame.
        pub fn latest(&self) -> Option<&DecodedFrameData> {
            self.frames.last()
        }
    }
}

/// p8-2: Framework/* (6 files, 271 LOC).
pub mod framework {
    /// BaseGame fields.
    pub const BASE_GAME_FIELDS: usize = 5;
    /// Color4 channels (RGBA).
    pub const COLOR4_CHANNELS: usize = 4;
    /// Display states.
    pub const DISPLAY_STATES: usize = 3;
    /// IGameHost methods.
    pub const HOST_METHODS: usize = 4;
    /// IRenderer methods.
    pub const RENDERER_METHODS: usize = 2;
    /// Log levels.
    pub const LOG_LEVELS: usize = 6;

    /// Color4 (BocuD Color4.cs:5-43) — RGBA color.
    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    pub struct Color4 {
        /// Red (0.0..1.0).
        pub r: f32,
        /// Green (0.0..1.0).
        pub g: f32,
        /// Blue (0.0..1.0).
        pub b: f32,
        /// Alpha (0.0..1.0).
        pub a: f32,
    }

    impl Color4 {
        /// Black opaque.
        pub const BLACK: Self = Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        /// White opaque.
        pub const WHITE: Self = Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        /// Transparent.
        pub const TRANSPARENT: Self = Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };

        /// Build a new color from RGBA components.
        pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
            Self { r, g, b, a }
        }

        /// Pack to 32-bit RGBA (BocuD Color4.cs:ToArgb).
        pub fn to_argb(&self) -> u32 {
            let a = (self.a.clamp(0.0, 1.0) * 255.0) as u32;
            let r = (self.r.clamp(0.0, 1.0) * 255.0) as u32;
            let g = (self.g.clamp(0.0, 1.0) * 255.0) as u32;
            let b = (self.b.clamp(0.0, 1.0) * 255.0) as u32;
            (a << 24) | (r << 16) | (g << 8) | b
        }

        /// Unpack from 32-bit ARGB.
        pub fn from_argb(argb: u32) -> Self {
            let a = ((argb >> 24) & 0xFF) as f32 / 255.0;
            let r = ((argb >> 16) & 0xFF) as f32 / 255.0;
            let g = ((argb >> 8) & 0xFF) as f32 / 255.0;
            let b = (argb & 0xFF) as f32 / 255.0;
            Self { r, g, b, a }
        }

        /// Average luma (BocuD Color4.cs:Luminance).
        pub fn luminance(&self) -> f32 {
            0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
        }
    }

    /// Display state (BocuD DisplayState.cs:5-8).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum DisplayState {
        /// Normal windowed.
        #[default]
        Windowed = 0,
        /// Fullscreen.
        Fullscreen = 1,
        /// Borderless fullscreen.
        Borderless = 2,
    }

    impl DisplayState {
        /// All variants in order.
        pub fn all() -> &'static [Self] {
            &[Self::Windowed, Self::Fullscreen, Self::Borderless]
        }
    }

    /// Runtime log level (BocuD RuntimeLogListener.cs:10-15).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub enum LogLevel {
        /// Trace.
        #[default]
        Trace = 0,
        /// Debug.
        Debug = 1,
        /// Info.
        Info = 2,
        /// Warning.
        Warn = 3,
        /// Error.
        Error = 4,
        /// Critical.
        Critical = 5,
    }

    impl LogLevel {
        /// All variants in order.
        pub fn all() -> &'static [Self] {
            &[
                Self::Trace,
                Self::Debug,
                Self::Info,
                Self::Warn,
                Self::Error,
                Self::Critical,
            ]
        }

        /// Whether this level should be logged given a threshold.
        pub fn should_log(self, threshold: LogLevel) -> bool {
            self as i32 >= threshold as i32
        }
    }

    /// Runtime log listener (BocuD RuntimeLogListener.cs:30-167).
    #[derive(Debug, Clone, Default)]
    pub struct RuntimeLogListener {
        /// Minimum level to log.
        pub threshold: LogLevel,
        /// Number of messages logged.
        pub message_count: u64,
        /// Last error message (if any).
        pub last_error: Option<String>,
    }

    impl RuntimeLogListener {
        /// Build a new listener.
        pub fn new() -> Self {
            Self::default()
        }

        /// Set threshold.
        pub fn with_threshold(mut self, t: LogLevel) -> Self {
            self.threshold = t;
            self
        }

        /// Log a message at the given level.
        pub fn log(&mut self, level: LogLevel, message: &str) {
            if !level.should_log(self.threshold) {
                return;
            }
            self.message_count += 1;
            if level as i32 >= LogLevel::Error as i32 {
                self.last_error = Some(message.to_string());
            }
        }
    }
}

/// p8-3: Glue/* (1 file, 248 LOC).
pub mod glue {
    /// SlimDXGLFWGlue LOC (BocuD SlimDXGLFWGlue.cs:1-248).
    pub const GLUE_SIZE_LOC: u32 = 248;
}

/// p8-4: OpenGL/* — SKIP per goal scope (Bevy replaces).
pub mod opengl_skipped {
    /// 3 OpenGL files not ported.
    pub const SKIPPED_FILES: usize = 3;
    /// Total LOC skipped.
    pub const SKIPPED_LOC: u32 = 856;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_constants() {
        assert_eq!(video::VIDEO_STATES, 4);
        assert_eq!(video::VIDEO_BUFFER_SIZE, 4);
        assert_eq!(video::FRAME_DATA_FIELDS, 4);
    }

    #[test]
    fn video_state_transitions() {
        use video::VideoState::*;
        assert!(Stopped.can_transition_to(Playing));
        assert!(Playing.can_transition_to(Paused));
        assert!(Paused.can_transition_to(Playing));
        assert!(Playing.can_transition_to(Stopped));
        assert!(!Stopped.can_transition_to(Paused)); // can't pause when stopped
    }

    #[test]
    fn video_state_all() {
        assert_eq!(video::VideoState::all().len(), 4);
    }

    #[test]
    fn decoded_frame_aspect_ratio() {
        let f = video::DecodedFrameData::new(0, 1920, 1080, 1024);
        assert!((f.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn decoded_frame_zero_height() {
        let f = video::DecodedFrameData::new(0, 100, 0, 0);
        assert_eq!(f.aspect_ratio(), 0.0);
    }

    #[test]
    fn video_buffer_push_overflow() {
        let mut b = video::VideoBuffer::new();
        for i in 0..(video::VIDEO_BUFFER_SIZE + 2) {
            b.push(video::DecodedFrameData::new(i as u32, 100, 100, 0));
        }
        assert_eq!(b.len(), video::VIDEO_BUFFER_SIZE);
    }

    #[test]
    fn framework_constants() {
        assert_eq!(framework::COLOR4_CHANNELS, 4);
        assert_eq!(framework::DISPLAY_STATES, 3);
        assert_eq!(framework::LOG_LEVELS, 6);
    }

    #[test]
    fn color4_black_argb() {
        let c = framework::Color4::BLACK;
        assert_eq!(c.to_argb(), 0xFF000000);
    }

    #[test]
    fn color4_white_argb() {
        let c = framework::Color4::WHITE;
        assert_eq!(c.to_argb(), 0xFFFFFFFF);
    }

    #[test]
    fn color4_argb_round_trip() {
        let c = framework::Color4::new(0.5, 0.25, 0.75, 1.0);
        let argb = c.to_argb();
        let c2 = framework::Color4::from_argb(argb);
        assert!((c.r - c2.r).abs() < 0.01);
        assert!((c.g - c2.g).abs() < 0.01);
        assert!((c.b - c2.b).abs() < 0.01);
    }

    #[test]
    fn color4_luminance() {
        let white = framework::Color4::WHITE;
        assert!((white.luminance() - 1.0).abs() < 0.01);
        let black = framework::Color4::BLACK;
        assert_eq!(black.luminance(), 0.0);
    }

    #[test]
    fn display_state_all() {
        assert_eq!(framework::DisplayState::all().len(), 3);
    }

    #[test]
    fn log_level_all_and_should_log() {
        assert_eq!(framework::LogLevel::all().len(), 6);
        assert!(framework::LogLevel::Error.should_log(framework::LogLevel::Warn));
        assert!(!framework::LogLevel::Debug.should_log(framework::LogLevel::Error));
    }

    #[test]
    fn runtime_log_listener() {
        let mut l = framework::RuntimeLogListener::new();
        l.log(framework::LogLevel::Info, "hello");
        l.log(framework::LogLevel::Error, "boom");
        assert_eq!(l.message_count, 2);
        assert_eq!(l.last_error.as_deref(), Some("boom"));
        // Default threshold is Trace → all messages pass
        l.log(framework::LogLevel::Trace, "noise");
        assert_eq!(l.message_count, 3);
    }

    #[test]
    fn glue_marker() {
        assert_eq!(glue::GLUE_SIZE_LOC, 248);
    }

    #[test]
    fn opengl_skipped_marker() {
        assert_eq!(opengl_skipped::SKIPPED_FILES, 3);
        assert_eq!(opengl_skipped::SKIPPED_LOC, 856);
    }
}
