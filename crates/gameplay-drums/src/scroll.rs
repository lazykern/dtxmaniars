//! Note spawning + scrolling (UI-space).
//!
//! Notes are bevy_ui `Node` entities positioned absolutely within the HUD.
//! They scroll from top (spawn) to the hit line and are despawned past it.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::{Note, NoteVisual};
use crate::events::NoteMissed;
use crate::hud::HudRoot;
use crate::judge::{BpmChangeList, JudgedChips};
use crate::lane_map::lane_of;
use crate::lane_map::LANE_COUNT;
use crate::layout::PlayfieldLayout;
use crate::resources::{ActiveChart, GameplayClock, ScrollSettings};
use game_shell::EGameMode;

const LOOKAHEAD_MS: i64 = 2000;
const BACKFILL_MS: i64 = 500;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (spawn_notes_system, scroll_notes_system)
            .chain()
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(
        FixedUpdate,
        despawn_missed_notes_system
            .in_set(super::DrumsSets::Judge)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

fn top_for_note(target_ms: i64, now_ms: i64, judge_y: f32, pixels_per_ms: f32) -> f32 {
    let delta_ms = (target_ms - now_ms) as f32;
    judge_y - delta_ms * pixels_per_ms
}

fn should_emit_miss_for_note(
    chip_id: usize,
    now_ms: i64,
    target_ms: i64,
    judged: &HashSet<usize>,
) -> bool {
    !judged.contains(&chip_id) && now_ms - target_ms > crate::drum_groups::MAX_JUDGE_WINDOW_MS
}

/// Per-lane color from the theme palette (gives visual variety).
pub fn lane_color(lane: u8) -> Color {
    match lane {
        0 => Color::srgb(0.0, 0.65, 0.85),
        1 => Color::srgb(0.95, 0.85, 0.2),
        2 => Color::srgb(0.85, 0.2, 0.85),
        3 => Color::srgb(0.2, 0.85, 0.2),
        4 => Color::srgb(0.2, 0.6, 0.2),
        5 => Color::srgb(0.75, 0.35, 0.15),
        6 => Color::srgb(0.9, 0.75, 0.0),
        7 => Color::srgb(0.3, 0.75, 0.9),
        8 => Color::srgb(0.85, 0.55, 0.15),
        9 => Color::srgb(0.55, 0.85, 0.35),
        10 => Color::srgb(0.65, 0.25, 0.55),
        11 => Color::srgb(0.45, 0.25, 0.75),
        _ => Color::WHITE,
    }
}

fn spawn_notes_system(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    existing: Query<&Note>,
    hud_root: Query<Entity, With<HudRoot>>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    if !clock.is_ready() {
        return;
    }
    let now = clock.current_ms;
    let Ok(hud) = hud_root.single() else {
        return;
    };

    let existing_ids: std::collections::HashSet<usize> =
        existing.iter().map(|n| n.chip_id).collect();

    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let judge_y = layout.judge_y();
    let px_per_ms = scroll.pixels_per_ms;

    for (idx, chip) in chart.chart.chips.iter().enumerate() {
        if existing_ids.contains(&idx) {
            continue;
        }
        let Some(lane) = lane_of(chip.channel) else {
            continue;
        };
        let target_ms = crate::judge::chip_target_ms(chip, base_bpm, &bpm_changes.changes);
        if target_ms < now - BACKFILL_MS || target_ms > now + LOOKAHEAD_MS {
            continue;
        }
        let top = top_for_note(target_ms, now, judge_y, px_per_ms);
        let left = layout.lane_left(lane as usize) + 2.0;

        let note_entity = commands
            .spawn((
                Note {
                    chip_id: idx,
                    lane,
                    target_ms,
                },
                NoteVisual,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(top),
                    width: Val::Px(layout.note_width()),
                    height: Val::Px(layout.note_height()),
                    ..default()
                },
                BackgroundColor(lane_color(lane)),
            ))
            .id();
        commands.entity(hud).add_child(note_entity);
    }
}

fn scroll_notes_system(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    mut notes: Query<(&Note, &mut Node), With<NoteVisual>>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    if !clock.is_ready() {
        return;
    }
    let now = clock.current_ms;
    let judge_y = layout.judge_y();
    let px_per_ms = scroll.pixels_per_ms;
    for (note, mut node) in &mut notes {
        node.top = Val::Px(top_for_note(note.target_ms, now, judge_y, px_per_ms));
    }
}

fn despawn_missed_notes_system(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    judged: Res<JudgedChips>,
    notes: Query<(Entity, &Note), With<NoteVisual>>,
    mut missed: MessageWriter<NoteMissed>,
    mut commands: Commands,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    if !clock.is_ready() {
        return;
    }
    let now = clock.current_ms;
    let mut hit_lanes: Vec<bool> = vec![false; LANE_COUNT];
    for (entity, note) in &notes {
        if should_emit_miss_for_note(note.chip_id, now, note.target_ms, &judged.0) {
            if !hit_lanes[note.lane as usize] {
                missed.write(NoteMissed {
                    lane: note.lane,
                    audio_ms: now,
                });
                hit_lanes[note.lane as usize] = true;
            }
            commands.entity(entity).despawn();
        } else if judged.0.contains(&note.chip_id) {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane_map::LaneId;
    use std::collections::HashSet;

    #[test]
    fn top_at_hit_line_when_target_is_now() {
        let top = top_for_note(1000, 1000, 620.0, 0.5);
        assert!((top - 620.0).abs() < 0.01);
    }

    #[test]
    fn future_notes_above_hit_line() {
        let top_far = top_for_note(2000, 0, 620.0, 0.5);
        let top_near = top_for_note(1000, 0, 620.0, 0.5);
        assert!(
            top_far < top_near,
            "farther notes should have lower top (higher on screen)"
        );
    }

    #[test]
    fn lane_id_zero() {
        let _: LaneId = 0;
    }

    #[test]
    fn judged_note_is_not_a_late_miss() {
        let judged = HashSet::from([7usize]);

        assert!(!should_emit_miss_for_note(7, 1200, 1000, &judged));
    }

    #[test]
    fn miss_window_matches_nx_poor_boundary() {
        let judged = HashSet::new();

        assert!(!should_emit_miss_for_note(7, 1117, 1000, &judged));
        assert!(should_emit_miss_for_note(7, 1118, 1000, &judged));
    }
}
