#![allow(missing_docs)]
//! Core sub-acts — batched port (p8-1..p8-3).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Core/`

/// p8-1: Video/* (8 files, 1238 LOC) — structure only, no decode.
pub mod video {
    /// DecodedFrameData.cs (15 LOC) — single decoded frame.
    pub const FRAME_DATA_FIELDS: usize = 4;
    /// DisplayedFrame.cs (25 LOC) — frame timing.
    pub const DISPLAY_FRAME_FIELDS: usize = 3;
    /// FFmpegCore.cs (45 LOC) — FFmpeg wrapper (no decode port per goal scope).
    pub const FFMPEG_CORES: usize = 1;
    /// UINewVideoRenderer.cs (97 LOC) — UI video renderer.
    pub const VIDEO_RENDERERS: usize = 1;
    /// VideoPlayerController.cs (312 LOC) — playback controller.
    pub const VIDEO_CONTROLLERS: usize = 1;
    /// Video state types (Playing/Paused/Stopped/Error).
    pub const VIDEO_STATES: usize = 4;
    /// Default video buffer size.
    pub const VIDEO_BUFFER_SIZE: usize = 4;
}

/// p8-2: Framework/* (6 files, 271 LOC).
pub mod framework {
    /// BaseGame.cs (30 LOC) — game base.
    pub const BASE_GAME_FIELDS: usize = 5;
    /// Color4.cs (43 LOC) — RGBA color struct.
    pub const COLOR4_CHANNELS: usize = 4;
    /// DisplayState.cs (8 LOC) — display states.
    pub const DISPLAY_STATES: usize = 3;
    /// IGameHost.cs (17 LOC) — host interface.
    pub const HOST_METHODS: usize = 4;
    /// IRenderer.cs (6 LOC) — render interface.
    pub const RENDERER_METHODS: usize = 2;
    /// RuntimeLogListener.cs (167 LOC) — log handler.
    pub const LOG_LEVELS: usize = 6;
}

/// p8-3: Glue/* (1 file, 248 LOC).
pub mod glue {
    /// SlimDXGLFWGlue.cs (248 LOC) — SlimDX ↔ GLFW bridge.
    /// (Not ported — Bevy replaces SlimDX. We mark structure only.)
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
    fn framework_constants() {
        assert_eq!(framework::COLOR4_CHANNELS, 4);
        assert_eq!(framework::DISPLAY_STATES, 3);
        assert_eq!(framework::LOG_LEVELS, 6);
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
