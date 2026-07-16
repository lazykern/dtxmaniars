//! Input-offset tap-test calibration overlay for the Customize surface.

use std::time::{Duration, Instant};

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

/// Suggested input offset from the median tap error. The judge computes
/// `delta = (audio_ms - input_offset) - target`, and the beat grid the taps are
/// measured against is the chip-target grid, so the measured median error
/// (`audio_ms - target`) IS the offset that zeroes delta — apply it directly.
pub fn suggested_offset(median_err: i32, clamp: i32) -> i32 {
    median_err.clamp(-clamp, clamp)
}

/// Replace a manual setting only when the collected evidence is stable.
pub fn apply_report(current: &mut i32, report: &CalibrationReport) {
    if report.can_apply() {
        *current = suggested_offset(report.proposed_offset_ms, dtx_config::INPUT_OFFSET_CLAMP_MS);
    }
}

fn midi_disconnect_detected(was_connected: bool, is_connected: bool) -> bool {
    was_connected && !is_connected
}

fn restore_toggles(
    metronome: &mut bool,
    timing_lines: &mut bool,
    autoplay: &mut bool,
    prev_metronome: bool,
    prev_timing_lines: bool,
    prev_autoplay: bool,
) {
    *metronome = prev_metronome;
    *timing_lines = prev_timing_lines;
    *autoplay = prev_autoplay;
}

/// Number of stable taps required before calibration may change the setting.
pub const TARGET_ACCEPTED_SAMPLES: usize = 12;
/// Samples beyond this distance from the first median are accidental taps.
pub const OUTLIER_DISTANCE_MS: i32 = 100;
/// Largest accepted median absolute deviation that is safe to apply.
pub const MAX_MAD_MS: i32 = 20;
/// One 60 Hz display frame: larger click-scheduling delay weakens confidence.
pub const MAX_SCHEDULER_DELAY_MS: i32 = 34;

/// Whether calibration evidence is sufficiently stable to replace manual input
/// timing adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Low,
}

/// Robust summary of raw calibration tap errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationReport {
    pub proposed_offset_ms: i32,
    pub accepted_samples: usize,
    pub rejected_samples: usize,
    pub spread_mad_ms: i32,
    pub max_scheduler_delay_ms: i32,
    pub confidence: Confidence,
}

impl CalibrationReport {
    /// Reject remote samples around the raw median, then calculate the proposal
    /// and MAD from the remaining evidence.
    pub fn from_errors(errors: &[i32], scheduler_delays: &[i32]) -> Self {
        let center = median(errors);
        let accepted: Vec<_> = errors
            .iter()
            .copied()
            .filter(|error| {
                (i64::from(*error) - i64::from(center)).abs() <= i64::from(OUTLIER_DISTANCE_MS)
            })
            .collect();
        let proposed_offset_ms = median(&accepted);
        let deviations: Vec<_> = accepted
            .iter()
            .map(|value| (i64::from(*value) - i64::from(proposed_offset_ms)).abs() as i32)
            .collect();
        let spread_mad_ms = median(&deviations);
        let max_scheduler_delay_ms = scheduler_delays.iter().copied().max().unwrap_or(0).max(0);
        let rejected_samples = errors.len().saturating_sub(accepted.len());
        let confidence = if accepted.len() >= TARGET_ACCEPTED_SAMPLES
            && rejected_samples.saturating_mul(4) <= errors.len()
            && spread_mad_ms <= MAX_MAD_MS
            && max_scheduler_delay_ms <= MAX_SCHEDULER_DELAY_MS
        {
            Confidence::High
        } else {
            Confidence::Low
        };

        Self {
            proposed_offset_ms,
            accepted_samples: accepted.len(),
            rejected_samples,
            spread_mad_ms,
            max_scheduler_delay_ms,
            confidence,
        }
    }

    pub fn can_apply(&self) -> bool {
        self.confidence == Confidence::High
    }
}

/// Chart-independent 120 BPM sequence used by the guided calibration overlay.
#[derive(Debug, Clone, Copy)]
pub struct CalibrationSchedule {
    started_at: Instant,
    first_beat_at: Instant,
}

impl CalibrationSchedule {
    const LEAD_IN: Duration = Duration::from_secs(1);
    const BEAT_INTERVAL: Duration = Duration::from_millis(500);
    const PREVIEW_HIT_HOLD: Duration = Duration::from_millis(80);
    pub const BEAT_COUNT: usize = 16;

    pub fn new(started_at: Instant) -> Self {
        Self {
            started_at,
            first_beat_at: started_at + Self::LEAD_IN,
        }
    }

    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    pub fn first_beat_at(&self) -> Instant {
        self.first_beat_at
    }

    pub fn beat_interval(&self) -> Duration {
        Self::BEAT_INTERVAL
    }

    /// Number of scheduled clicks that should have fired by `now`.
    pub fn due_beat_count(&self, now: Instant) -> usize {
        if now < self.first_beat_at {
            return 0;
        }
        let elapsed = now.duration_since(self.first_beat_at);
        let due = elapsed.as_millis() / Self::BEAT_INTERVAL.as_millis() + 1;
        (due as usize).min(Self::BEAT_COUNT)
    }

    /// Signed distance from `tap` to the nearest scheduled beat.
    pub fn error_ms(&self, tap: Instant) -> i32 {
        let interval_ms = Self::BEAT_INTERVAL.as_millis() as i128;
        let elapsed_ms = if tap >= self.first_beat_at {
            tap.duration_since(self.first_beat_at).as_millis() as i128
        } else {
            -(self.first_beat_at.duration_since(tap).as_millis() as i128)
        };
        let nearest_ms = (elapsed_ms + interval_ms / 2).div_euclid(interval_ms) * interval_ms;
        (elapsed_ms - nearest_ms) as i32
    }

    /// Note travel from lane top (0) to strike line (1), held briefly on each beat.
    fn preview_progress(&self, now: Instant) -> Option<f32> {
        let interval_ms = Self::BEAT_INTERVAL.as_millis() as u64;
        if now < self.first_beat_at {
            let remaining_ms = self.first_beat_at.duration_since(now).as_millis() as u64;
            return Some(1.0 - remaining_ms.min(interval_ms) as f32 / interval_ms as f32);
        }

        let elapsed = now.duration_since(self.first_beat_at);
        let final_beat = Self::BEAT_INTERVAL.mul_f32((Self::BEAT_COUNT - 1) as f32);
        if elapsed > final_beat + Self::PREVIEW_HIT_HOLD {
            return None;
        }

        let phase_ms = elapsed.as_millis() as u64 % interval_ms;
        let hold_ms = Self::PREVIEW_HIT_HOLD.as_millis() as u64;
        Some(if phase_ms < hold_ms {
            1.0
        } else {
            (phase_ms - hold_ms) as f32 / (interval_ms - hold_ms) as f32
        })
    }
}

/// Tap-test lifecycle. Idle by default.
#[derive(Resource, Default)]
pub enum CalibrationState {
    #[default]
    Idle,
    Collecting {
        schedule: CalibrationSchedule,
        samples: Vec<i32>,
        scheduler_delays: Vec<i32>,
        fired_beats: usize,
        midi_was_connected: bool,
        midi_disconnected: bool,
        prev_metronome: bool,
        prev_timing_lines: bool,
        prev_autoplay: bool,
    },
    Done {
        report: CalibrationReport,
        midi_disconnected: bool,
        prev_metronome: bool,
        prev_timing_lines: bool,
        prev_autoplay: bool,
    },
}

#[derive(Component)]
struct CalibrationOverlay;

#[derive(Component)]
struct CalibrationPreviewNote;

#[derive(Resource, Default)]
struct CalibrationBgmDuck(bool);

/// Reuses the chart-independent metronome sample for calibration clicks.
#[derive(Resource, Default)]
struct CalibrationClickSound(Option<Handle<bevy_kira_audio::prelude::AudioSource>>);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CalibrationState>()
        .init_resource::<CalibrationClickSound>()
        .init_resource::<CalibrationBgmDuck>()
        .add_systems(
            Update,
            (
                preload_click_sound,
                fire_synthetic_clicks,
                collect_taps,
                confirm_or_cancel,
                render_overlay,
                animate_preview_note,
            )
                .chain()
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        )
        .add_systems(
            Update,
            (restore_when_editor_closes, sync_calibration_bgm_volume)
                .chain()
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(
            OnExit(game_shell::AppState::Performance),
            (restore_on_performance_exit, despawn_overlay),
        );
}

/// Called by the panel Calibrate button: enter the chart-independent sequence
/// and disable autoplay so only physical keyboard or MIDI input is sampled.
pub fn start_calibration(
    state: &mut CalibrationState,
    metronome_on: &mut crate::resources::MetronomeEnabled,
    timing_lines: &mut crate::resources::ShowTimingLines,
    autoplay: &mut crate::autoplay::AutoplayEnabled,
    midi: Option<&game_shell::MidiConnected>,
) {
    if !matches!(state, CalibrationState::Idle) {
        return;
    }
    *state = CalibrationState::Collecting {
        schedule: CalibrationSchedule::new(Instant::now()),
        samples: Vec::new(),
        scheduler_delays: Vec::new(),
        fired_beats: 0,
        midi_was_connected: midi.is_some_and(|state| state.0),
        midi_disconnected: false,
        prev_metronome: metronome_on.0,
        prev_timing_lines: timing_lines.0,
        prev_autoplay: autoplay.0,
    };
    metronome_on.0 = true;
    timing_lines.0 = true;
    autoplay.0 = false;
}

fn preload_click_sound(
    mut sources: ResMut<Assets<bevy_kira_audio::prelude::AudioSource>>,
    mut click: ResMut<CalibrationClickSound>,
) {
    if click.0.is_none() {
        click.0 = Some(sources.add(crate::practice::metronome::click_source(
            crate::practice::metronome::TICK_HZ,
        )));
    }
}

/// Fire every missed beat individually and retain the scheduler delay as an
/// observation. A laggy frame lowers confidence but cannot change the tap
/// proposal itself.
fn fire_synthetic_clicks(
    mut state: ResMut<CalibrationState>,
    click: Res<CalibrationClickSound>,
    audio: Res<bevy_kira_audio::prelude::Audio>,
    settings: Res<crate::resources::DrumAudioSettings>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    let CalibrationState::Collecting {
        schedule,
        scheduler_delays,
        fired_beats,
        midi_was_connected,
        midi_disconnected,
        ..
    } = &mut *state
    else {
        return;
    };
    let now = Instant::now();
    if let Some(midi) = midi {
        *midi_disconnected |= midi_disconnect_detected(*midi_was_connected, midi.0);
    }
    let due = schedule.due_beat_count(now);
    for beat in *fired_beats..due {
        let scheduled = schedule.first_beat_at()
            + Duration::from_millis(schedule.beat_interval().as_millis() as u64 * beat as u64);
        let delay = now
            .duration_since(scheduled)
            .as_millis()
            .min(i32::MAX as u128) as i32;
        scheduler_delays.push(delay);
        if let Some(source) = &click.0 {
            dtx_audio::play_sfx_handle(&audio, source.clone(), 100, 0, settings.master_volume, 1.0);
        }
    }
    *fired_beats = due;
}

fn collect_taps(
    mut state: ResMut<CalibrationState>,
    mut hits: MessageReader<crate::events::InputHit>,
) {
    let CalibrationState::Collecting {
        schedule,
        samples,
        scheduler_delays,
        prev_metronome,
        prev_timing_lines,
        prev_autoplay,
        midi_disconnected,
        ..
    } = &mut *state
    else {
        return;
    };

    // `InputHit` is written by both keyboard and MIDI paths before judgement;
    // use its monotonic physical-input stamp rather than chart/audio time.
    for hit in hits.read() {
        if hit.captured_at >= schedule.first_beat_at()
            && samples.len() < CalibrationSchedule::BEAT_COUNT
        {
            samples.push(schedule.error_ms(hit.captured_at));
        }
    }

    let last_beat = schedule.first_beat_at()
        + Duration::from_millis(
            schedule.beat_interval().as_millis() as u64
                * (CalibrationSchedule::BEAT_COUNT - 1) as u64,
        );
    let finished = samples.len() == CalibrationSchedule::BEAT_COUNT
        || Instant::now() >= last_beat + Duration::from_secs(2);
    if finished {
        let report = CalibrationReport::from_errors(samples, scheduler_delays);
        *state = CalibrationState::Done {
            report,
            midi_disconnected: *midi_disconnected,
            prev_metronome: *prev_metronome,
            prev_timing_lines: *prev_timing_lines,
            prev_autoplay: *prev_autoplay,
        };
    }
}

fn restore_runtime_state(
    state: &mut CalibrationState,
    metronome: &mut crate::resources::MetronomeEnabled,
    timing_lines: &mut crate::resources::ShowTimingLines,
    autoplay: &mut crate::autoplay::AutoplayEnabled,
) {
    let snapshot = match state {
        CalibrationState::Idle => None,
        CalibrationState::Collecting {
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
            ..
        }
        | CalibrationState::Done {
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
            ..
        } => Some((*prev_metronome, *prev_timing_lines, *prev_autoplay)),
    };
    if let Some((prev_metronome, prev_timing_lines, prev_autoplay)) = snapshot {
        restore_toggles(
            &mut metronome.0,
            &mut timing_lines.0,
            &mut autoplay.0,
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
        );
        *state = CalibrationState::Idle;
    }
}

fn restore_when_editor_closes(
    open: Res<super::EditorOpen>,
    mut state: ResMut<CalibrationState>,
    mut metronome: ResMut<crate::resources::MetronomeEnabled>,
    mut timing_lines: ResMut<crate::resources::ShowTimingLines>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    if open.is_changed() && !open.0 {
        restore_runtime_state(&mut state, &mut metronome, &mut timing_lines, &mut autoplay);
    }
}

fn restore_on_performance_exit(
    mut state: ResMut<CalibrationState>,
    mut metronome: ResMut<crate::resources::MetronomeEnabled>,
    mut timing_lines: ResMut<crate::resources::ShowTimingLines>,
    mut autoplay: ResMut<crate::autoplay::AutoplayEnabled>,
) {
    restore_runtime_state(&mut state, &mut metronome, &mut timing_lines, &mut autoplay);
}

fn sync_calibration_bgm_volume(
    state: Res<CalibrationState>,
    settings: Res<crate::resources::DrumAudioSettings>,
    bgm: Res<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<bevy_kira_audio::prelude::AudioInstance>>,
    mut ducked: ResMut<CalibrationBgmDuck>,
) {
    let active = !matches!(*state, CalibrationState::Idle);
    if active == ducked.0 {
        return;
    }
    let gain = settings.bgm_gain() * if active { 0.15 } else { 1.0 };
    dtx_audio::set_bgm_volume(&bgm, &mut instances, gain);
    ducked.0 = active;
}

pub(super) fn confirm_or_cancel(
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
            report,
            prev_metronome,
            prev_timing_lines,
            prev_autoplay,
            ..
        } => {
            let apply = keys.just_pressed(KeyCode::Enter);
            let cancel = keys.just_pressed(KeyCode::Escape);
            if apply || cancel {
                if apply {
                    apply_report(&mut draft.0.gameplay.input_offset_ms, report);
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
    let (msg, pulse, show_preview) = match &*state {
        CalibrationState::Idle => return,
        CalibrationState::Collecting {
            samples,
            fired_beats,
            ..
        } => (
            format!(
                "120 BPM · strike when note reaches line\n{}/{} taps · beat {}/{}\nAny mapped key or pad · Esc cancel",
                samples.len(),
                CalibrationSchedule::BEAT_COUNT,
                fired_beats,
                CalibrationSchedule::BEAT_COUNT,
            ),
            fired_beats % 2 == 1,
            true,
        ),
        CalibrationState::Done {
            report,
            midi_disconnected,
            ..
        } => {
            let off =
                suggested_offset(report.proposed_offset_ms, dtx_config::INPUT_OFFSET_CLAMP_MS);
            let action = if report.can_apply() {
                "High confidence · Enter apply · Esc cancel"
            } else {
                "Low confidence · current offset retained · Enter/Esc close"
            };
            (
                format!(
                    "Suggested {off:+} ms · {} accepted, {} rejected · ±{} ms spread\nScheduler observation ≤ {} ms · {action}\n{}BGM adjustment is separate chart-audio alignment.",
                    report.accepted_samples,
                    report.rejected_samples,
                    report.spread_mad_ms,
                    report.max_scheduler_delay_ms,
                    if *midi_disconnected {
                        "MIDI disconnected; keyboard taps remain valid.\n"
                    } else {
                        ""
                    },
                ),
                false,
                false,
            )
        }
    };
    let overlay = commands
        .spawn((
            CalibrationOverlay,
            super::picking::EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(32.0),
                left: Val::Percent(30.0),
                padding: UiRect::all(Val::Px(16.0)),
                column_gap: Val::Px(18.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(if pulse {
                Color::srgba(0.10, 0.14, 0.20, 0.97)
            } else {
                Color::srgba(0.05, 0.05, 0.07, 0.95)
            }),
            GlobalZIndex(crate::ui_z::EDITOR_CHROME + 1),
        ))
        .id();
    commands.entity(overlay).with_children(|parent| {
        if show_preview {
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(72.0),
                        height: Val::Px(180.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                    BorderColor::all(theme.0.stage_panel_border),
                ))
                .with_children(|lane| {
                    lane.spawn((
                        CalibrationPreviewNote,
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            left: Val::Px(7.0),
                            width: Val::Px(56.0),
                            height: Val::Px(14.0),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(theme.0.accent),
                    ));
                    lane.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(16.0),
                            left: Val::Px(4.0),
                            width: Val::Px(62.0),
                            height: Val::Px(3.0),
                            ..default()
                        },
                        BackgroundColor(theme.0.select_yellow),
                    ));
                });
        }
        parent.spawn((
            Text::new(msg),
            dtx_ui::theme::Theme::font(16.0),
            TextColor(theme.0.text_primary),
        ));
    });
}

fn animate_preview_note(
    state: Res<CalibrationState>,
    mut note: Query<(&mut Node, &mut Visibility), With<CalibrationPreviewNote>>,
) {
    let CalibrationState::Collecting { schedule, .. } = &*state else {
        return;
    };
    let Some(progress) = schedule.preview_progress(Instant::now()) else {
        for (_, mut visibility) in &mut note {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    for (mut node, mut visibility) in &mut note {
        node.top = Val::Px(progress * 147.0);
        *visibility = Visibility::Visible;
    }
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
        // A consistently-late player (+40ms vs the beat) needs a +40ms offset,
        // because the judge subtracts input_offset from the hit time.
        assert_eq!(suggested_offset(40, 300), 40);
        assert_eq!(suggested_offset(-500, 300), -300);
    }

    #[test]
    fn report_uses_median_and_rejects_distant_outlier() {
        let report = CalibrationReport::from_errors(&[39, 40, 41, 40, 400], &[2, 3]);
        assert_eq!(report.proposed_offset_ms, 40);
        assert_eq!(report.accepted_samples, 4);
        assert_eq!(report.rejected_samples, 1);
    }

    #[test]
    fn unstable_or_sparse_evidence_cannot_apply() {
        assert!(!CalibrationReport::from_errors(&[10; 11], &[0]).can_apply());
        assert!(!CalibrationReport::from_errors(&[10; 12], &[35]).can_apply());
    }

    #[test]
    fn synthetic_schedule_has_a_lead_in_and_half_second_beats() {
        let schedule = CalibrationSchedule::new(std::time::Instant::now());
        assert_eq!(
            schedule.beat_interval(),
            std::time::Duration::from_millis(500)
        );
        assert!(schedule.first_beat_at() > schedule.started_at());
    }

    #[test]
    fn tap_error_uses_the_physical_input_timestamp() {
        let started = std::time::Instant::now();
        let schedule = CalibrationSchedule::new(started);
        assert_eq!(
            schedule.error_ms(schedule.first_beat_at() + std::time::Duration::from_millis(37)),
            37
        );
    }

    #[test]
    fn schedule_reports_every_beat_that_a_slow_frame_missed() {
        let schedule = CalibrationSchedule::new(std::time::Instant::now());
        let after_three_beats = schedule.first_beat_at() + std::time::Duration::from_millis(1_020);
        assert_eq!(schedule.due_beat_count(after_three_beats), 3);
    }

    #[test]
    fn preview_note_reaches_and_briefly_holds_the_strike_line() {
        let schedule = CalibrationSchedule::new(std::time::Instant::now());
        assert_eq!(
            schedule.preview_progress(schedule.first_beat_at() - Duration::from_millis(250)),
            Some(0.5)
        );
        assert_eq!(
            schedule.preview_progress(schedule.first_beat_at() + Duration::from_millis(40)),
            Some(1.0)
        );
    }

    #[test]
    fn weak_report_does_not_replace_manual_offset() {
        let mut value = 17;
        apply_report(&mut value, &CalibrationReport::from_errors(&[7; 11], &[0]));
        assert_eq!(value, 17);
    }

    #[test]
    fn strong_report_replaces_manual_offset_within_config_clamp() {
        let mut value = 17;
        apply_report(
            &mut value,
            &CalibrationReport::from_errors(&[450; 12], &[0]),
        );
        assert_eq!(value, 300);
    }

    #[test]
    fn only_a_disconnect_after_a_midi_start_is_reported() {
        assert!(!midi_disconnect_detected(false, false));
        assert!(!midi_disconnect_detected(true, true));
        assert!(midi_disconnect_detected(true, false));
    }

    #[test]
    fn restoration_returns_every_runtime_toggle_to_its_snapshot() {
        let mut metronome = true;
        let mut timing_lines = true;
        let mut autoplay = false;
        restore_toggles(
            &mut metronome,
            &mut timing_lines,
            &mut autoplay,
            false,
            false,
            true,
        );
        assert_eq!((metronome, timing_lines, autoplay), (false, false, true));
    }
}
