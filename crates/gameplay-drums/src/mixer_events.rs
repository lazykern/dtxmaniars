//! Mixer add/remove eligibility with deterministic seek reconstruction.

use std::collections::BTreeSet;

use bevy::prelude::*;
use dtx_core::{Chart, EChannel};
use dtx_timing::math::ChartTiming;

use crate::judge::{chip_target_ms, BarLengthChangeList, BpmChangeList};
use crate::resources::{ActiveChart, GameplayClock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixerEventKind {
    Add,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerEvent {
    pub at_ms: i64,
    pub slot: u32,
    pub kind: MixerEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixerAction {
    /// Registration changes never stop an already-playing voice.
    EligibilityOnly,
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct MixerEligibility {
    restricted: bool,
    eligible: BTreeSet<u32>,
}

impl MixerEligibility {
    pub fn restricted() -> Self {
        Self {
            restricted: true,
            eligible: BTreeSet::new(),
        }
    }

    pub fn is_slot_eligible(&self, slot: u32) -> bool {
        !self.restricted || self.eligible.contains(&slot)
    }
}

pub fn apply_mixer_event(eligibility: &mut MixerEligibility, event: MixerEvent) -> MixerAction {
    eligibility.restricted = true;
    match event.kind {
        MixerEventKind::Add => {
            eligibility.eligible.insert(event.slot);
        }
        MixerEventKind::Remove => {
            eligibility.eligible.remove(&event.slot);
        }
    }
    MixerAction::EligibilityOnly
}

pub fn rebuild_mixer_at(events: &[MixerEvent], target_ms: i64) -> MixerEligibility {
    if events.is_empty() {
        return MixerEligibility::default();
    }
    let mut eligibility = MixerEligibility::restricted();
    for event in events.iter().filter(|event| event.at_ms <= target_ms) {
        apply_mixer_event(&mut eligibility, *event);
    }
    eligibility
}

#[derive(Resource, Debug, Clone, Default)]
pub struct MixerEventCursor {
    events: Vec<MixerEvent>,
    next: usize,
    last_ms: i64,
}

impl MixerEventCursor {
    pub fn new(mut events: Vec<MixerEvent>) -> Self {
        events.sort_by_key(|event| (event.at_ms, event.slot));
        Self {
            events,
            next: 0,
            last_ms: i64::MIN,
        }
    }

    pub fn advance_to(&mut self, now_ms: i64, eligibility: &mut MixerEligibility) {
        if now_ms < self.last_ms {
            self.rebuild_at(now_ms, eligibility);
            return;
        }
        while self.next < self.events.len() && self.events[self.next].at_ms <= now_ms {
            apply_mixer_event(eligibility, self.events[self.next]);
            self.next += 1;
        }
        self.last_ms = now_ms;
    }

    pub fn rebuild_at(&mut self, target_ms: i64, eligibility: &mut MixerEligibility) {
        *eligibility = rebuild_mixer_at(&self.events, target_ms);
        self.next = self
            .events
            .partition_point(|event| event.at_ms <= target_ms);
        self.last_ms = target_ms;
    }

    pub fn events(&self) -> &[MixerEvent] {
        &self.events
    }
}

fn events_from_chart(
    chart: &Chart,
    bpm_changes: &BpmChangeList,
    bar_changes: &BarLengthChangeList,
) -> Vec<MixerEvent> {
    let base_bpm = chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    chart
        .chips
        .iter()
        .filter_map(|chip| {
            let kind = match chip.channel {
                EChannel::MixerAdd => MixerEventKind::Add,
                EChannel::MixerRemove => MixerEventKind::Remove,
                _ => return None,
            };
            Some(MixerEvent {
                at_ms: chip_target_ms(chip, base_bpm, timing),
                slot: chip.wav_slot,
                kind,
            })
        })
        .collect()
}

pub fn plugin(app: &mut App) {
    app.init_resource::<MixerEligibility>()
        .init_resource::<MixerEventCursor>()
        .add_systems(
            OnEnter(game_shell::AppState::Performance),
            initialize_mixer.after(crate::orchestrator::DrumsEnterSet),
        )
        .add_systems(
            FixedUpdate,
            advance_mixer_events
                .in_set(crate::DrumsSets::Mixer)
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(in_state(game_shell::PauseState::Running)),
        );
}

fn initialize_mixer(
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    mut cursor: ResMut<MixerEventCursor>,
    mut eligibility: ResMut<MixerEligibility>,
) {
    *cursor = MixerEventCursor::new(events_from_chart(&chart.chart, &bpm_changes, &bar_changes));
    cursor.rebuild_at(0, &mut eligibility);

    let missing = cursor
        .events()
        .iter()
        .map(|event| event.slot)
        .filter(|slot| chart.chart.assets.wav.get(*slot).is_none())
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        warn!("Performance: mixer references missing optional WAV slots {missing:?}");
    }
}

fn advance_mixer_events(
    clock: Res<GameplayClock>,
    mut cursor: ResMut<MixerEventCursor>,
    mut eligibility: ResMut<MixerEligibility>,
) {
    if clock.is_ready() {
        cursor.advance_to(clock.current_ms, &mut eligibility);
    }
}
