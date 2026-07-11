//! Count-in metronome: click schedule computed at seek time, fired as
//! the clock crosses each beat, plus the quick-tier countdown number.
//! Spec: docs/superpowers/specs/2026-07-11-practice-count-in-metronome-design.md

use bevy::prelude::*;
use bevy_kira_audio::prelude::{Audio, Frame, StaticSoundData, StaticSoundSettings};
use bevy_kira_audio::AudioSource as KiraAudioSource;
use dtx_ui::theme::Theme;
use game_shell::{AppState, PauseState};

use super::session::{preroll_target, PracticeSession, PrerollSetting};
use crate::resources::{DrumAudioSettings, GameplayClock};
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// One scheduled count-in click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    /// Chart time the click fires at (a `ChipTimeline.beat_ms` line).
    pub at_ms: i64,
    /// First click of the schedule is accented.
    pub accent: bool,
    /// Number shown by the countdown UI ("4 3 2 1"): clicks left
    /// including this one.
    pub beats_remaining: u8,
}

/// Click times for one pre-roll window, sorted ascending.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClickSchedule {
    pub clicks: Vec<Click>,
}

/// Clicks on beat-grid lines in `[preroll_target, intent_ms)`. No click
/// at `intent_ms` itself — the music entry is the implicit "1".
pub fn build_preroll_schedule(
    timeline: &ChipTimeline,
    preroll: PrerollSetting,
    intent_ms: i64,
) -> ClickSchedule {
    let window_start = preroll_target(timeline, preroll, intent_ms);
    if window_start >= intent_ms {
        return ClickSchedule::default();
    }
    let beats: Vec<i64> = timeline
        .beat_ms
        .iter()
        .copied()
        .filter(|&ms| ms >= window_start && ms < intent_ms)
        .collect();
    let n = beats.len();
    ClickSchedule {
        clicks: beats
            .into_iter()
            .enumerate()
            .map(|(i, at_ms)| Click {
                at_ms,
                accent: i == 0,
                beats_remaining: (n - i) as u8,
            })
            .collect(),
    }
}

const CLICK_SAMPLE_RATE: u32 = 44_100;
const CLICK_LEN_S: f32 = 0.03;
const ACCENT_HZ: f32 = 2_000.0;
const TICK_HZ: f32 = 1_000.0;

/// ~30ms sine burst with exponential decay (pure; unit-tested).
pub fn synth_click_frames(freq_hz: f32, sample_rate: u32) -> Vec<Frame> {
    let n = (CLICK_LEN_S * sample_rate as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let env = (-t * 150.0).exp();
            Frame::from_mono((t * freq_hz * std::f32::consts::TAU).sin() * env * 0.8)
        })
        .collect()
}

fn click_source(freq_hz: f32) -> KiraAudioSource {
    KiraAudioSource {
        sound: StaticSoundData {
            sample_rate: CLICK_SAMPLE_RATE,
            frames: synth_click_frames(freq_hz, CLICK_SAMPLE_RATE).into(),
            settings: StaticSoundSettings::default(),
            slice: None,
        },
    }
}

/// Handles to the two synthesized click samples.
#[derive(Resource, Default)]
pub struct MetronomeSounds {
    pub accent: Handle<KiraAudioSource>,
    pub tick: Handle<KiraAudioSource>,
}

/// Build the click samples once per Performance enter (practice only —
/// the plugin gates on `PracticeSession`).
pub fn build_metronome_sounds(
    mut sounds: ResMut<MetronomeSounds>,
    mut sources: ResMut<Assets<KiraAudioSource>>,
) {
    sounds.accent = sources.add(click_source(ACCENT_HZ));
    sounds.tick = sources.add(click_source(TICK_HZ));
}

/// The schedule for the current pre-roll, plus how far it has fired.
#[derive(Resource, Debug, Default)]
pub struct ActiveClickSchedule {
    pub schedule: ClickSchedule,
    pub cursor: usize,
}

/// What the countdown UI shows right now.
#[derive(Resource, Debug, Default)]
pub struct CountdownDisplay {
    /// `Some((beats_remaining, accent, wall_seconds_shown_at))`.
    pub current: Option<(u8, bool, f64)>,
}

/// Clicks whose time has come at `clock_ms`, advancing `cursor` past them.
pub fn due_clicks<'a>(
    schedule: &'a ClickSchedule,
    cursor: &'a mut usize,
    clock_ms: i64,
) -> impl Iterator<Item = Click> + 'a {
    std::iter::from_fn(move || {
        let click = schedule.clicks.get(*cursor)?;
        if click.at_ms <= clock_ms {
            *cursor += 1;
            Some(*click)
        } else {
            None
        }
    })
}

/// Rebuild the schedule whenever a practice seek lands. Runs after
/// `apply_seek_system` so it sees the same coalesced last-seek.
pub fn rebuild_click_schedule(
    mut seeks: MessageReader<SeekToChartTime>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut active: ResMut<ActiveClickSchedule>,
    mut display: ResMut<CountdownDisplay>,
) {
    let Some(seek) = seeks.read().last().copied() else {
        return;
    };
    display.current = None;
    if !session.transport.metronome {
        *active = ActiveClickSchedule::default();
        return;
    }
    let intent = seek.attempt_start_ms.unwrap_or(seek.target_ms);
    active.schedule = build_preroll_schedule(&timeline, session.transport.preroll, intent);
    active.cursor = 0;
}

/// Fire due clicks: play the sample, update the countdown display.
pub fn fire_clicks(
    clock: Res<GameplayClock>,
    sounds: Res<MetronomeSounds>,
    audio: Res<Audio>,
    settings: Res<DrumAudioSettings>,
    time: Res<Time>,
    mut active: ResMut<ActiveClickSchedule>,
    mut display: ResMut<CountdownDisplay>,
) {
    if !clock.is_ready() {
        return;
    }
    let ActiveClickSchedule { schedule, cursor } = &mut *active;
    let mut last = None;
    for click in due_clicks(schedule, cursor, clock.current_ms) {
        let source = if click.accent {
            sounds.accent.clone()
        } else {
            sounds.tick.clone()
        };
        dtx_audio::play_sfx_handle(&audio, source, 100, 0, settings.master_volume, 1.0);
        last = Some(click);
    }
    if let Some(click) = last {
        display.current = Some((click.beats_remaining, click.accent, time.elapsed_secs_f64()));
    }
}

#[derive(Component)]
pub struct CountdownText;

const COUNTDOWN_FADE_S: f64 = 0.4;

pub fn spawn_countdown(mut commands: Commands) {
    let theme = Theme::default();
    commands.spawn((
        CountdownText,
        Text::new(""),
        Theme::title_font(),
        TextColor(theme.text_primary),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(24.0),
            left: Val::Percent(50.0),
            ..default()
        },
        GlobalZIndex(crate::ui_z::PRACTICE),
        Visibility::Hidden,
    ));
}

pub fn despawn_countdown(mut commands: Commands, q: Query<Entity, With<CountdownText>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

pub fn update_countdown(
    display: Res<CountdownDisplay>,
    time: Res<Time>,
    mut q: Query<(&mut Text, &mut TextColor, &mut Visibility), With<CountdownText>>,
) {
    let Ok((mut text, mut color, mut vis)) = q.single_mut() else {
        return;
    };
    match display.current {
        Some((n, accent, shown_at)) => {
            let age = time.elapsed_secs_f64() - shown_at;
            if age > COUNTDOWN_FADE_S {
                *vis = Visibility::Hidden;
                return;
            }
            text.0 = n.to_string();
            let theme = Theme::default();
            let base = if accent {
                theme.accent
            } else {
                theme.text_primary
            };
            color.0 = base.with_alpha(1.0 - (age / COUNTDOWN_FADE_S) as f32);
            *vis = Visibility::Visible;
        }
        None => *vis = Visibility::Hidden,
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<MetronomeSounds>()
        .init_resource::<ActiveClickSchedule>()
        .init_resource::<CountdownDisplay>()
        .add_systems(
            OnEnter(AppState::Performance),
            (build_metronome_sounds, spawn_countdown).run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(OnExit(AppState::Performance), despawn_countdown)
        .add_systems(
            FixedUpdate,
            (
                rebuild_click_schedule.after(crate::seek::apply_seek_system),
                fire_clicks.after(crate::DrumsSets::ClockSync),
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(resource_exists::<PracticeSession>),
        )
        .add_systems(
            Update,
            update_countdown
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(resource_exists::<PracticeSession>),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms, beat = 500ms. Chart spans 4 bars.
    fn timeline() -> ChipTimeline {
        let chart = Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: vec![
                Chip::new(0, EChannel::BassDrum, 0.0),
                Chip::new(3, EChannel::Snare, 0.75), // keeps 4 bars of lines
            ],
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 8_000)
    }

    #[test]
    fn one_bar_preroll_yields_four_clicks_counting_down() {
        let tl = timeline();
        // Intent at bar 2 start (4000ms): window = [2000, 4000).
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        let times: Vec<i64> = s.clicks.iter().map(|c| c.at_ms).collect();
        assert_eq!(times, vec![2_000, 2_500, 3_000, 3_500]);
        let remaining: Vec<u8> = s.clicks.iter().map(|c| c.beats_remaining).collect();
        assert_eq!(remaining, vec![4, 3, 2, 1]);
    }

    #[test]
    fn only_first_click_is_accented() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        assert!(s.clicks[0].accent);
        assert!(s.clicks[1..].iter().all(|c| !c.accent));
    }

    #[test]
    fn no_click_at_intent_itself() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 4_000);
        assert!(s.clicks.iter().all(|c| c.at_ms < 4_000));
    }

    #[test]
    fn seconds_preroll_takes_beats_inside_window() {
        let tl = timeline();
        // 1.2s window before 4000ms: [2800, 4000) → beats 3000, 3500.
        let s = build_preroll_schedule(&tl, PrerollSetting::Seconds(1.2), 4_000);
        let times: Vec<i64> = s.clicks.iter().map(|c| c.at_ms).collect();
        assert_eq!(times, vec![3_000, 3_500]);
        assert_eq!(s.clicks[0].beats_remaining, 2);
    }

    #[test]
    fn off_preroll_yields_empty_schedule() {
        let tl = timeline();
        let s = build_preroll_schedule(&tl, PrerollSetting::Off, 4_000);
        assert!(s.clicks.is_empty());
    }

    #[test]
    fn intent_at_chart_start_yields_empty_schedule() {
        let tl = timeline();
        // preroll_target clamps to 0 == intent → empty window.
        let s = build_preroll_schedule(&tl, PrerollSetting::OneBar, 0);
        assert!(s.clicks.is_empty());
    }

    #[test]
    fn due_clicks_advance_cursor_and_stop_at_clock() {
        let s = ClickSchedule {
            clicks: vec![
                Click {
                    at_ms: 2_000,
                    accent: true,
                    beats_remaining: 2,
                },
                Click {
                    at_ms: 2_500,
                    accent: false,
                    beats_remaining: 1,
                },
            ],
        };
        let mut cursor = 0usize;
        let first: Vec<Click> = due_clicks(&s, &mut cursor, 2_010).collect();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].at_ms, 2_000);
        assert_eq!(cursor, 1);
        // Nothing new until the clock reaches the next click.
        assert_eq!(due_clicks(&s, &mut cursor, 2_499).count(), 0);
        assert_eq!(due_clicks(&s, &mut cursor, 2_500).count(), 1);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn synth_click_is_short_and_non_silent() {
        let frames = synth_click_frames(2_000.0, 44_100);
        // ~30ms at 44.1kHz ≈ 1323 frames.
        assert!((1_300..=1_350).contains(&frames.len()));
        assert!(frames.iter().any(|f| f.left.abs() > 0.05));
        // Exponential decay: tail quieter than head.
        assert!(frames[frames.len() - 1].left.abs() < frames[10].left.abs());
    }
}
