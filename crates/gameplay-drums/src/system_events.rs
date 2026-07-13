//! Timed, non-judgeable chart events.
//!
//! Hidden drums and MIDI chorus are consumed without entering input,
//! judgment, score, gauge, or analysis routing. Click and first-sound chips
//! share the same deterministic cursor while their audio is scheduled by the
//! auto-SE system.

use bevy::prelude::*;
use dtx_core::{Chart, EChannel};

use crate::resources::{ActiveChart, GameplayClock};
use crate::timeline::ChipTimeline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEventKind {
    Hidden { sound_lane: EChannel },
    MidiChorus,
    FillIn,
    Click,
    FirstSound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedSystemEvent {
    pub chip_idx: usize,
    pub at_ms: i64,
    pub wav_slot: u32,
    pub kind: SystemEventKind,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SystemEventSchedule {
    pub events: Vec<TimedSystemEvent>,
}

impl SystemEventSchedule {
    pub fn from_chart(chart: &Chart, timeline: &ChipTimeline) -> Self {
        let mut events = chart
            .chips
            .iter()
            .enumerate()
            .filter_map(|(chip_idx, chip)| {
                let kind = if let Some(sound_lane) = chip.channel.hidden_sound_lane() {
                    SystemEventKind::Hidden { sound_lane }
                } else {
                    match chip.channel {
                        EChannel::MIDIChorus => SystemEventKind::MidiChorus,
                        EChannel::FillIn => SystemEventKind::FillIn,
                        EChannel::Click => SystemEventKind::Click,
                        EChannel::FirstSoundChip => SystemEventKind::FirstSound,
                        _ => return None,
                    }
                };
                Some(TimedSystemEvent {
                    chip_idx,
                    at_ms: timeline
                        .judge_ms_by_idx
                        .get(chip_idx)
                        .copied()
                        .unwrap_or_default(),
                    wav_slot: chip.wav_slot,
                    kind,
                })
            })
            .collect::<Vec<_>>();
        events.sort_by_key(|event| (event.at_ms, event.chip_idx));
        Self { events }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct SystemEventCursor {
    next: usize,
    last_ms: i64,
}

impl Default for SystemEventCursor {
    fn default() -> Self {
        Self {
            next: 0,
            last_ms: i64::MIN,
        }
    }
}

impl SystemEventCursor {
    /// Advances once through events at or before `now_ms`. A backward clock
    /// move reconstructs the cursor at the destination and emits nothing.
    pub fn advance_to(
        &mut self,
        schedule: &SystemEventSchedule,
        now_ms: i64,
    ) -> Vec<TimedSystemEvent> {
        if now_ms < self.last_ms {
            self.next = schedule
                .events
                .partition_point(|event| event.at_ms <= now_ms);
            self.last_ms = now_ms;
            return Vec::new();
        }

        let start = self.next;
        while self.next < schedule.events.len() && schedule.events[self.next].at_ms <= now_ms {
            self.next += 1;
        }
        self.last_ms = now_ms;
        schedule.events[start..self.next].to_vec()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn will_rebuild(&self, now_ms: i64) -> bool {
        now_ms < self.last_ms
    }
}

/// Reconstruct every observable system-event effect at `now_ms`. This is the
/// seek/restart path and is also useful for deterministic headless tests.
pub fn rebuild_effects_at(
    schedule: &SystemEventSchedule,
    chart: &Chart,
    now_ms: i64,
    fillin_enabled: bool,
    templates: &mut crate::resources::CurrentEmptyHitTemplates,
    filling: &mut crate::drums_perf::DrumsFillingEffect,
) {
    templates.reset();
    filling.end();
    for event in schedule.events.iter().filter(|event| event.at_ms <= now_ms) {
        apply_event_effect(*event, chart, fillin_enabled, templates, filling);
    }
}

fn apply_event_effect(
    event: TimedSystemEvent,
    chart: &Chart,
    fillin_enabled: bool,
    templates: &mut crate::resources::CurrentEmptyHitTemplates,
    filling: &mut crate::drums_perf::DrumsFillingEffect,
) {
    match event.kind {
        SystemEventKind::Hidden { sound_lane } => {
            let Some(lane) = crate::lane_map::lane_of(sound_lane) else {
                return;
            };
            let Some(chip) = chart.chips.get(event.chip_idx) else {
                return;
            };
            templates.set_at(
                lane,
                dtx_core::EmptyHitEvent {
                    lane,
                    measure: chip.measure,
                    value: chip.value,
                    wav_slot: event.wav_slot,
                },
                event.at_ms,
            );
        }
        SystemEventKind::FillIn if fillin_enabled => match event.wav_slot {
            1 => filling.start(),
            2 => filling.end(),
            _ => {}
        },
        SystemEventKind::MidiChorus
        | SystemEventKind::FillIn
        | SystemEventKind::Click
        | SystemEventKind::FirstSound => {}
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SystemEventSchedule>()
        .init_resource::<SystemEventCursor>()
        .add_systems(
            OnEnter(game_shell::AppState::Performance),
            build_schedule.after(crate::timeline::build_chip_timeline),
        )
        .add_systems(
            FixedUpdate,
            consume_system_events
                .in_set(crate::DrumsSets::NoteSpawn)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running)),
        );
}

fn build_schedule(
    chart: Res<ActiveChart>,
    timeline: Res<ChipTimeline>,
    mut schedule: ResMut<SystemEventSchedule>,
    mut cursor: ResMut<SystemEventCursor>,
    mut filling: ResMut<crate::drums_perf::DrumsFillingEffect>,
) {
    *schedule = SystemEventSchedule::from_chart(&chart.chart, &timeline);
    cursor.reset();
    filling.end();
}

fn consume_system_events(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    schedule: Res<SystemEventSchedule>,
    mut cursor: ResMut<SystemEventCursor>,
    drum_settings: Res<crate::resources::DrumGameplaySettings>,
    mut templates: ResMut<crate::resources::CurrentEmptyHitTemplates>,
    mut filling: ResMut<crate::drums_perf::DrumsFillingEffect>,
) {
    if clock.is_ready() {
        let rebuilding = cursor.will_rebuild(clock.current_ms);
        let events = cursor.advance_to(&schedule, clock.current_ms);
        if rebuilding {
            rebuild_effects_at(
                &schedule,
                &chart.chart,
                clock.current_ms,
                drum_settings.fillin_enabled,
                &mut templates,
                &mut filling,
            );
        } else {
            for event in events {
                apply_event_effect(
                    event,
                    &chart.chart,
                    drum_settings.fillin_enabled,
                    &mut templates,
                    &mut filling,
                );
            }
        }
    }
}
