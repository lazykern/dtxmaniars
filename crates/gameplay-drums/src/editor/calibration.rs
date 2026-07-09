//! Input-offset tap-test calibration overlay for the Customize surface.

use bevy::prelude::*;

/// Signed error (ms) of `now_ms` to the nearest beat on a grid of `beat_ms`
/// spacing starting at `first_beat_ms`. Range (-beat/2, beat/2].
pub fn error_ms(now_ms: f64, beat_ms: f64, first_beat_ms: f64) -> f64 {
    if beat_ms <= 0.0 {
        return 0.0;
    }
    let rel = now_ms - first_beat_ms;
    let phase = rel.rem_euclid(beat_ms);
    if phase > beat_ms / 2.0 {
        phase - beat_ms
    } else {
        phase
    }
}

/// Median of a sample set (integer ms). Empty → 0.
pub fn median(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    let mut v = samples.to_vec();
    v.sort_unstable();
    v[v.len() / 2]
}

/// Suggested input offset from the median tap error: cancel the latency.
pub fn suggested_offset(median_err: i32, clamp: i32) -> i32 {
    (-median_err).clamp(-clamp, clamp)
}

/// Tap-test lifecycle. Idle by default.
#[derive(Resource, Default)]
pub enum CalibrationState {
    #[default]
    Idle,
    Collecting {
        samples: Vec<i32>,
        prev_metronome: bool,
        prev_timing_lines: bool,
        prev_autoplay: bool,
    },
    Done {
        median: i32,
        prev_metronome: bool,
        prev_timing_lines: bool,
        prev_autoplay: bool,
    },
}

/// How many taps before showing a suggestion.
pub const TARGET_SAMPLES: usize = 12;

#[derive(Component)]
struct CalibrationOverlay;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CalibrationState>()
        .add_systems(
            Update,
            (collect_taps, confirm_or_cancel, render_overlay)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        )
        .add_systems(
            OnExit(game_shell::AppState::Performance),
            despawn_overlay,
        );
}

/// Called by the panel Calibrate button: enter Collecting, forcing metronome +
/// timing lines on (the tick fires only on line crossings) and autoplay OFF
/// (the editor session forces autoplay on; its perfect on-target LaneHits would
/// otherwise auto-fill the tap test instead of the player's own taps).
pub fn start_calibration(
    state: &mut CalibrationState,
    metronome_on: &mut crate::resources::MetronomeEnabled,
    timing_lines: &mut crate::resources::ShowTimingLines,
    autoplay: &mut crate::autoplay::AutoplayEnabled,
) {
    if !matches!(state, CalibrationState::Idle) {
        return;
    }
    *state = CalibrationState::Collecting {
        samples: Vec::new(),
        prev_metronome: metronome_on.0,
        prev_timing_lines: timing_lines.0,
        prev_autoplay: autoplay.0,
    };
    metronome_on.0 = true;
    timing_lines.0 = true;
    autoplay.0 = false;
}

fn collect_taps(
    mut state: ResMut<CalibrationState>,
    chart: Res<crate::resources::ActiveChart>,
    mut hits: MessageReader<crate::events::LaneHit>,
) {
    let CalibrationState::Collecting { samples, .. } = &mut *state else {
        return;
    };
    let bpm = chart.chart.metadata.bpm.unwrap_or(120.0) as f64;
    if bpm <= 0.0 {
        return;
    }
    let beat_ms = 60_000.0 / bpm;
    let mut got = false;
    // Each hit's own raw timestamp (pre input-offset) vs the nearest beat is the
    // latency to cancel; the frame clock would smear all hits in a frame together.
    for hit in hits.read() {
        let e = error_ms(hit.audio_ms as f64, beat_ms, 0.0);
        if e.abs() <= beat_ms / 2.0 {
            samples.push(e.round() as i32);
            got = true;
        }
    }
    if got && samples.len() >= TARGET_SAMPLES {
        let m = median(samples);
        if let CalibrationState::Collecting {
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
            ..
        } = *state
        {
            *state = CalibrationState::Done {
                median: m,
                prev_metronome,
                prev_timing_lines,
                prev_autoplay,
            };
        }
    }
}

fn confirm_or_cancel(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CalibrationState>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
    mut metronome_on: ResMut<crate::resources::MetronomeEnabled>,
    mut timing_lines: ResMut<crate::resources::ShowTimingLines>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    match &*state {
        CalibrationState::Idle => {}
        CalibrationState::Collecting {
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
            ..
        } => {
            if keys.just_pressed(KeyCode::Escape) {
                metronome_on.0 = *prev_metronome;
                timing_lines.0 = *prev_timing_lines;
                autoplay.0 = *prev_autoplay;
                *state = CalibrationState::Idle;
            }
        }
        CalibrationState::Done {
            median,
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
        } => {
            let apply = keys.just_pressed(KeyCode::Enter);
            let cancel = keys.just_pressed(KeyCode::Escape);
            if apply || cancel {
                if apply {
                    let off = suggested_offset(*median, dtx_config::INPUT_OFFSET_CLAMP_MS);
                    draft.0.gameplay.input_offset_ms = off;
                }
                metronome_on.0 = *prev_metronome;
                timing_lines.0 = *prev_timing_lines;
                autoplay.0 = *prev_autoplay;
                *state = CalibrationState::Idle;
            }
        }
    }
}

fn render_overlay(
    mut commands: Commands,
    state: Res<CalibrationState>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<CalibrationOverlay>>,
) {
    if !state.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    let msg = match &*state {
        CalibrationState::Idle => return,
        CalibrationState::Collecting { samples, .. } => {
            format!("Tap to the beat  ({}/{})", samples.len(), TARGET_SAMPLES)
        }
        CalibrationState::Done { median, .. } => {
            let off = suggested_offset(*median, dtx_config::INPUT_OFFSET_CLAMP_MS);
            format!("Suggested {off:+} ms   Enter apply · Esc cancel")
        }
    };
    commands.spawn((
        CalibrationOverlay,
        super::picking::EditorChrome,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(40.0),
            left: Val::Percent(35.0),
            padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.95)),
        GlobalZIndex(crate::ui_z::EDITOR_CHROME + 1),
        children![(
            Text::new(msg),
            dtx_ui::theme::Theme::font(16.0),
            TextColor(theme.0.text_primary),
        )],
    ));
}

fn despawn_overlay(mut commands: Commands, existing: Query<Entity, With<CalibrationOverlay>>) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_ms_nearest_beat_signed() {
        assert!((error_ms(40.0, 500.0, 0.0) - 40.0).abs() < 1e-6);
        assert!((error_ms(470.0, 500.0, 0.0) + 30.0).abs() < 1e-6);
        assert!((error_ms(1010.0, 500.0, 0.0) - 10.0).abs() < 1e-6);
    }
    #[test]
    fn median_odd_and_empty() {
        assert_eq!(median(&[3, 1, 2]), 2);
        assert_eq!(median(&[]), 0);
    }
    #[test]
    fn suggested_offset_cancels_and_clamps() {
        assert_eq!(suggested_offset(40, 300), -40);
        assert_eq!(suggested_offset(-500, 300), 300);
    }
}
