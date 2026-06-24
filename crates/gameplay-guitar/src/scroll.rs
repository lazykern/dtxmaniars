//! Guitar note spawning + scrolling.
//!
//! Gated on `EGameMode::Guitar` so only the active mode's pipeline runs.
//! Mirrors `gameplay-drums/src/scroll.rs` shape.

use bevy::prelude::*;
use dtx_timing::AudioClock;
use game_shell::EGameMode;

use crate::components::{Note, NoteVisual};
use crate::events::NoteMissed;
use crate::lane_map::lane_of;
use crate::resources::ActiveChart;

const LOOKAHEAD_MS: i64 = 2000;
const HIT_LINE_Y: f32 = -300.0;
const SCROLL_PIXELS_PER_MS: f32 = 0.4;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_notes_system,
            scroll_notes_system,
            despawn_missed_notes_system,
        )
            .chain(),
    );
}

fn y_for_target(target_ms: i64, now_ms: i64) -> f32 {
    let delta_ms = (target_ms - now_ms) as f32;
    HIT_LINE_Y + delta_ms * SCROLL_PIXELS_PER_MS
}

fn spawn_notes_system(
    mut commands: Commands,
    clock: Res<AudioClock>,
    mode: Res<EGameMode>,
    chart: Res<ActiveChart>,
    existing: Query<&Note>,
) {
    if *mode != EGameMode::Guitar {
        return;
    }
    let Some(now) = clock.current_ms else {
        return;
    };
    let existing_ids: std::collections::HashSet<usize> =
        existing.iter().map(|n| n.chip_id).collect();

    for (idx, chip) in chart.chart.chips.iter().enumerate() {
        if existing_ids.contains(&idx) {
            continue;
        }
        let Some(lane) = lane_of(chip.channel) else {
            continue;
        };
        let target_ms = dtx_timing::math::chip_time_ms(
            chip.measure,
            chip.value,
            chart.chart.metadata.bpm.unwrap_or(120.0),
        );
        if target_ms < now || target_ms > now + LOOKAHEAD_MS {
            continue;
        }
        commands.spawn((
            Note {
                chip_id: idx,
                lane,
                target_ms,
            },
            NoteVisual,
            Transform::from_translation(Vec3::new(
                lane as f32 * 80.0 - 160.0,
                y_for_target(target_ms, now),
                0.0,
            )),
        ));
    }
}

fn scroll_notes_system(
    clock: Res<AudioClock>,
    mode: Res<EGameMode>,
    mut notes: Query<(&Note, &mut Transform), With<NoteVisual>>,
) {
    if *mode != EGameMode::Guitar {
        return;
    }
    let Some(now) = clock.current_ms else {
        return;
    };
    for (note, mut transform) in &mut notes {
        transform.translation.y = y_for_target(note.target_ms, now);
    }
}

fn despawn_missed_notes_system(
    clock: Res<AudioClock>,
    mode: Res<EGameMode>,
    mut notes: Query<(Entity, &Note), With<NoteVisual>>,
    mut missed: MessageWriter<NoteMissed>,
    mut commands: Commands,
) {
    if *mode != EGameMode::Guitar {
        return;
    }
    let Some(now) = clock.current_ms else {
        return;
    };
    let mut hit_lanes: Vec<bool> = vec![false; 5];
    for (entity, note) in &notes {
        if now - note.target_ms > 200 {
            if !hit_lanes[note.lane as usize] {
                missed.write(NoteMissed {
                    lane: note.lane,
                    audio_ms: now,
                });
                hit_lanes[note.lane as usize] = true;
            }
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_decreases_as_target_approaches() {
        let y_far = y_for_target(2000, 0);
        let y_near = y_for_target(1000, 0);
        assert!(y_far > y_near);
    }

    #[test]
    fn y_zero_when_target_is_now() {
        let y = y_for_target(1000, 1000);
        assert!((y - HIT_LINE_Y).abs() < 0.01);
    }
}
