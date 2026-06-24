//! DTXManiaNX fade transition — verbatim from StageManager.cs:29.
//!
//! ## Reference
//!
//! `references/DTXmaniaNX-BocuD/DTXMania/Core/StageManager.cs:29`
//! `private float FadeDurationMs = 1500f;`
//!
//! StageManager.cs:670-699 — `DrawFadeOverlay`:
//!   `_fadeAlpha = Math.Clamp(1f - (float)elapsed / FadeDurationMs, 0f, 1f);`
//!
//! ## Behavior
//!
//! - **Linear 1500ms fade-out** of an overlay over the new stage.
//! - **Direction**: out only — overlay alpha goes 1.0 → 0.0.
//! - **Curve**: linear (NOT OutQuint).  `(1 - elapsed/1500).clamp(0, 1)`
//! - **Snapshot approximation**: StageManager captures `rt.ReadPixels()` of
//!   the OLD stage and fades that. We use a fullscreen black overlay (alpha
//!   animation is identical from the user's perspective). True framebuffer
//!   snapshot is deferred to M3.1 (ADR-0011).
//! - **Spike handling**: StageManager waits for the new stage's first frame
//!   to complete before starting the fade. Our overlay at full alpha on
//!   frame 1 covers any spike naturally.
//!
//! ponytail: black-overlay approximation, NOT true rt.ReadPixels() snapshot.
//! Swap to texture capture in M3.1 when framebuffer→texture is needed.

use bevy::prelude::*;

/// DTXManiaNX `StageManager.cs:29 FadeDurationMs = 1500f`.
pub const FADE_DURATION_MS: u64 = 1500;

/// Tracks current fade. `Some(_)` = active; `None` = idle.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct FadeState {
    /// ms elapsed since fade began. `None` means no fade in progress.
    pub elapsed_ms: Option<u64>,
}

impl FadeState {
    pub fn begin(&mut self) {
        self.elapsed_ms = Some(0);
    }

    /// Current overlay alpha. Linear: `1 - elapsed/duration`.
    pub fn alpha(&self) -> f32 {
        match self.elapsed_ms {
            Some(e) => (1.0 - e as f32 / FADE_DURATION_MS as f32).clamp(0.0, 1.0),
            None => 0.0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.elapsed_ms.is_some_and(|e| e < FADE_DURATION_MS)
    }
}

/// Marker for the fullscreen fade overlay entity.
#[derive(Component)]
pub struct FadeOverlay;

pub struct FadePlugin;

impl Plugin for FadePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FadeState>()
            .add_systems(Update, update_fade_system);
    }
}

fn update_fade_system(
    time: Res<Time>,
    mut fade: ResMut<FadeState>,
    mut overlay_query: Query<(Entity, &mut BackgroundColor), With<FadeOverlay>>,
    mut commands: Commands,
) {
    if !fade.is_active() {
        // Cleanup any leftover overlay entities (shouldn't normally exist).
        for (entity, _) in &overlay_query {
            commands.entity(entity).despawn();
        }
        return;
    }

    let dt_ms = time.delta().as_millis() as u64;
    fade.elapsed_ms = Some((fade.elapsed_ms.unwrap_or(0) + dt_ms).min(FADE_DURATION_MS));

    let alpha = fade.alpha();

    for (_, mut bg) in &mut overlay_query {
        // Linear alpha decay over 1500ms (StageManager.cs:687 verbatim).
        bg.0 = Color::srgba(0.0, 0.0, 0.0, alpha);
    }

    if !fade.is_active() {
        for (entity, _) in &overlay_query {
            commands.entity(entity).despawn();
        }
        fade.elapsed_ms = None;
    }
}

/// Spawn the fade overlay at full alpha and start the fade. Call from OnEnter of any state.
pub fn start_fade(mut commands: Commands, mut fade: ResMut<FadeState>) {
    fade.begin();
    commands.spawn((
        FadeOverlay,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 1.0)),
        GlobalZIndex(i32::MAX - 1), // under topmost debug UI
    ));
}

#[cfg(test)]
mod tests {
    //! Verifies the fade math matches DTXManiaNX StageManager.cs:687 verbatim:
    //!   `_fadeAlpha = Math.Clamp(1f - (float)elapsed / FadeDurationMs, 0f, 1f);`
    use super::*;

    #[test]
    fn alpha_is_1_at_start() {
        let fade = FadeState {
            elapsed_ms: Some(0),
        };
        assert!(
            (fade.alpha() - 1.0).abs() < 1e-6,
            "alpha at t=0 must be 1.0"
        );
    }

    #[test]
    fn alpha_is_half_at_halfway() {
        let fade = FadeState {
            elapsed_ms: Some(FADE_DURATION_MS / 2),
        };
        assert!(
            (fade.alpha() - 0.5).abs() < 1e-3,
            "alpha at t=dur/2 must be 0.5 (linear)"
        );
    }

    #[test]
    fn alpha_is_0_at_end() {
        let fade = FadeState {
            elapsed_ms: Some(FADE_DURATION_MS),
        };
        assert!(fade.alpha().abs() < 1e-6, "alpha at t=duration must be 0.0");
    }

    #[test]
    fn alpha_clamps_beyond_end() {
        let fade = FadeState {
            elapsed_ms: Some(FADE_DURATION_MS + 1000),
        };
        assert!(
            fade.alpha().abs() < 1e-6,
            "alpha beyond duration must be clamped to 0.0"
        );
    }

    #[test]
    fn is_active_only_while_elapsed_under_duration() {
        assert!(!FadeState::default().is_active());
        assert!(
            FadeState {
                elapsed_ms: Some(0)
            }
            .is_active()
        );
        assert!(
            FadeState {
                elapsed_ms: Some(FADE_DURATION_MS - 1)
            }
            .is_active()
        );
        assert!(
            !FadeState {
                elapsed_ms: Some(FADE_DURATION_MS)
            }
            .is_active()
        );
        assert!(
            !FadeState {
                elapsed_ms: Some(FADE_DURATION_MS + 1)
            }
            .is_active()
        );
    }

    #[test]
    fn duration_constant_matches_dtxmania() {
        // StageManager.cs:29: `private float FadeDurationMs = 1500f;`
        assert_eq!(
            FADE_DURATION_MS, 1500,
            "must match StageManager.cs:29 verbatim"
        );
    }
}
