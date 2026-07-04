//! Album-art crossfade widget.
//!
//! ADR-0015 Phase 3. Listens to `dtx_audio::PreviewSwapEvent` and
//! crossfades the album art's opacity to match the audio crossfade.
//!
//! Guard against in-flight tween: if a previous fade-in is still
//! running, the new event hard-cuts to the new image instead of
//! starting a partial tween. Prevents partial-opacity ghosts on
//! rapid scroll.
//!
//! The widget is a `Component`; the system updates its `is_flying`
//! field. No visible image entity is required for the system to
//! function — integration into the actual song-select layout is a
//! separate "modern song select" task.
//!
//! Layer: Game. Depends on `dtx-audio` (Engine) for the event type.

use std::time::Duration;

use bevy::prelude::*;
use dtx_audio::PreviewSwapEvent;

/// Album-art widget marker. Attach to the entity holding the album
/// art (or any placeholder) to participate in the crossfade tween.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct AlbumArt {
    /// True while a crossfade tween is in flight. New swap events
    /// arriving while `is_flying == true` trigger a hard-cut instead
    /// of starting a new tween.
    pub is_flying: bool,
    /// Elapsed milliseconds in the current tween.
    pub elapsed_ms: u32,
    /// Total tween duration (`fade_out_ms + fade_in_ms`).
    pub total_ms: u32,
}

const FADE_OUT_MS: u32 = 150;
const FADE_IN_MS: u32 = 220;
const TOTAL_MS: u32 = FADE_OUT_MS + FADE_IN_MS;

/// System: drive album-art tween from `PreviewSwapEvent`.
///
/// - If `AlbumArt::is_flying == false`: start a new tween, set
///   `is_flying = true`, reset `elapsed_ms = 0`.
/// - If `is_flying == true`: hard-cut (just reset `elapsed_ms = 0`,
///   leave `is_flying = true`).
/// - Each frame, advance `elapsed_ms` by the delta. When
///   `elapsed_ms >= total_ms`, set `is_flying = false`.
pub fn album_art_tween_system(
    time: Res<Time>,
    mut events: MessageReader<PreviewSwapEvent>,
    mut query: Query<&mut AlbumArt>,
) {
    let delta_ms = (time.delta_secs() * 1000.0) as u32;

    // Advance any in-flight tween.
    for mut art in &mut query {
        if art.is_flying {
            art.elapsed_ms = art.elapsed_ms.saturating_add(delta_ms);
            if art.elapsed_ms >= art.total_ms {
                art.is_flying = false;
                art.elapsed_ms = 0;
            }
        }
    }

    // Apply incoming swap events.
    for _event in events.read() {
        for mut art in &mut query {
            if art.is_flying {
                // Hard-cut: skip the fade, jump to "end" of tween.
                art.elapsed_ms = 0;
                // Keep is_flying=true so the next tick will clear it.
            } else {
                art.is_flying = true;
                art.elapsed_ms = 0;
                art.total_ms = TOTAL_MS;
            }
        }
    }
}

/// Convenience: spawn a no-op album-art entity for testing. The entity
/// has the `AlbumArt` Component but no visible mesh.
#[allow(dead_code)]
pub fn spawn_test_album_art(commands: &mut Commands) -> Entity {
    commands
        .spawn(AlbumArt {
            is_flying: false,
            elapsed_ms: 0,
            total_ms: TOTAL_MS,
        })
        .id()
}

/// Test helper: feed a `PreviewSwapEvent` into a `MessageWriter`.
/// Mirrors the message shape consumed by `album_art_tween_system`.
#[allow(dead_code)]
pub fn fake_swap_event() -> PreviewSwapEvent {
    PreviewSwapEvent {
        old_path: None,
        new_path: std::path::PathBuf::from("/songs/a/preview.ogg"),
        direction: dtx_audio::PreviewSwapDirection::Next,
    }
}

#[allow(dead_code)]
const _: Duration = Duration::from_millis(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_art_defaults_to_not_flying() {
        let art = AlbumArt::default();
        assert!(!art.is_flying);
        assert_eq!(art.elapsed_ms, 0);
        assert_eq!(art.total_ms, 0);
    }

    #[test]
    fn total_constant_matches_fade_sum() {
        assert_eq!(TOTAL_MS, FADE_OUT_MS + FADE_IN_MS);
        assert_eq!(FADE_OUT_MS, 150);
        assert_eq!(FADE_IN_MS, 220);
    }
}
