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
) {
    *schedule = SystemEventSchedule::from_chart(&chart.chart, &timeline);
    cursor.reset();
}

fn consume_system_events(
    clock: Res<GameplayClock>,
    schedule: Res<SystemEventSchedule>,
    mut cursor: ResMut<SystemEventCursor>,
) {
    if clock.is_ready() {
        // Consumption is intentionally side-effect free. The cursor keeps
        // system events deterministic across normal play and seek/restart.
        drop(cursor.advance_to(&schedule, clock.current_ms));
    }
}
