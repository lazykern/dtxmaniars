//! Note spawning + scrolling (UI-space).
//!
//! Notes are bevy_ui `Node` entities positioned absolutely within the HUD.
//! They scroll from top (spawn) to the hit line and are despawned past it.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::components::{Note, NoteVisual};
use crate::events::NoteMissed;
use crate::hud::HudRoot;
use crate::interp::RenderClock;
use crate::judge::{BarLengthChangeList, BpmChangeList, JudgedChips};
use crate::lane_map::lane_of;
use crate::lane_map::{lane_channel, LANE_COUNT};
use crate::lanes::Lanes;
use crate::layout::PlayfieldLayout;
use crate::resources::{ActiveChart, GameplayClock, ScrollSettings};
use dtx_timing::math::ChartTiming;
use game_shell::{AppState, EGameMode, PauseState};

const BACKFILL_MS: i64 = 500;

/// Ms from lane top to hit line at current scroll speed (osu-style `TimeRange`).
///
/// When `target_ms - now == lookahead_ms`, [`top_for_note`] places the note at
/// [`PlayfieldLayout::lane_top`].
pub(crate) fn lookahead_ms(layout: &PlayfieldLayout, scroll: &ScrollSettings) -> i64 {
    let px_per_ms = scroll.pixels_per_ms * layout.scale;
    if px_per_ms <= f32::EPSILON {
        return i64::MAX;
    }
    let scroll_length = layout.judge_y() - layout.lane_top();
    (scroll_length / px_per_ms).ceil() as i64
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (spawn_notes_system, scroll_notes_system)
            .chain()
            .in_set(crate::layout::PlayfieldLayoutConsumers)
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(PauseState::Running))
            .run_if(crate::practice::chart_clock_active),
    )
    .add_systems(
        Update,
        reposition_notes_on_layout_change
            .in_set(crate::layout::PlayfieldLayoutConsumers)
            .run_if(in_state(AppState::Performance))
            .run_if(resource_changed::<PlayfieldLayout>),
    )
    .add_systems(
        FixedUpdate,
        (
            despawn_missed_notes_system.run_if(crate::practice::gameplay_input_active),
            despawn_passed_preview_notes_system
                .run_if(resource_exists::<crate::practice::PracticeFlow>)
                .run_if(crate::practice::chart_clock_active),
        )
            .in_set(super::DrumsSets::Judge)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

fn top_for_note(target_ms: i64, now_ms: i64, judge_y: f32, pixels_per_ms: f32) -> f32 {
    top_for_note_f(target_ms, now_ms as f64, judge_y, pixels_per_ms)
}

/// Same as [`top_for_note`] but with a fractional `now_ms` for sub-frame motion.
pub(crate) fn top_for_note_f(target_ms: i64, now_ms: f64, judge_y: f32, pixels_per_ms: f32) -> f32 {
    let delta_ms = (target_ms as f64 - now_ms) as f32;
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

pub(crate) fn spawn_notes_system(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    judged: Res<JudgedChips>,
    preview_skipped: Res<crate::seek::PreviewSkippedChips>,
    practice_flow: Option<Res<crate::practice::PracticeFlow>>,
    lanes: Res<Lanes>,
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
        bevy::log::warn_once!("spawn_notes: no HudRoot entity found");
        return;
    };

    let existing_ids: std::collections::HashSet<usize> =
        existing.iter().map(|n| n.chip_id).collect();

    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    let judge_y = layout.judge_y();
    // Ref-space NX velocity scaled into the live (redesigned) playfield.
    let px_per_ms = scroll.pixels_per_ms * layout.scale;
    let spawn_window_ms = lookahead_ms(&layout, &scroll);

    let skipped = if practice_flow
        .as_ref()
        .is_some_and(|flow| flow.phase != crate::practice::PracticePhase::Running)
    {
        &preview_skipped.0
    } else {
        &judged.0
    };

    for (idx, chip) in chart.chart.chips.iter().enumerate() {
        if existing_ids.contains(&idx) || skipped.contains(&idx) {
            continue;
        }
        let Some(lane) = lane_of(chip.channel) else {
            continue;
        };
        let target_ms = crate::judge::chip_target_ms(chip, base_bpm, timing);
        if target_ms < now - BACKFILL_MS || target_ms > now + spawn_window_ms {
            continue;
        }
        let Some(col) = lanes.col_of(chip.channel) else {
            continue;
        };
        if col >= layout.col_count() {
            continue;
        }
        let top = top_for_note(target_ms, now, judge_y, px_per_ms);
        let left = layout.col_left(col) + 2.0;
        let color = lanes.chip_color(chip.channel);
        let hollow = lanes.is_hollow(chip.channel);

        let mut note_cmd = commands.spawn((
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
                width: Val::Px(layout.note_width(col)),
                height: Val::Px(layout.note_height()),
                border: if hollow {
                    UiRect::all(Val::Px(2.0 * layout.scale))
                } else {
                    UiRect::ZERO
                },
                ..default()
            },
        ));
        if hollow {
            note_cmd.insert((BackgroundColor(Color::NONE), BorderColor::all(color)));
        } else {
            note_cmd.insert(BackgroundColor(color));
        }
        let note_entity = note_cmd.id();
        commands.entity(hud).add_child(note_entity);
    }
}

fn scroll_notes_system(
    clock: Res<GameplayClock>,
    render: Res<RenderClock>,
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
    // Sub-frame interpolated visual clock — smooth note motion at any FPS.
    let now = render.now_ms();
    let judge_y = layout.judge_y();
    let px_per_ms = scroll.pixels_per_ms * layout.scale;
    for (note, mut node) in &mut notes {
        node.top = Val::Px(top_for_note_f(note.target_ms, now, judge_y, px_per_ms));
    }
}

/// Re-anchor already-spawned notes to their column when the playfield rescales
/// (window resize). `scroll_notes_system` only drives `top`; without this, notes
/// keep the `left`/`width` baked in at spawn-time scale and drift off the strip.
fn reposition_notes_on_layout_change(
    mode: Res<EGameMode>,
    layout: Res<PlayfieldLayout>,
    lanes: Res<Lanes>,
    mut notes: Query<(&Note, &mut Node), With<NoteVisual>>,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    for (note, mut node) in &mut notes {
        let Some(channel) = lane_channel(note.lane) else {
            continue;
        };
        let Some(col) = lanes.col_of(channel) else {
            continue;
        };
        node.left = Val::Px(layout.col_left(col) + 2.0);
        node.width = Val::Px(layout.note_width(col));
        node.height = Val::Px(layout.note_height());
        node.border = if lanes.is_hollow(channel) {
            UiRect::all(Val::Px(2.0 * layout.scale))
        } else {
            UiRect::ZERO
        };
    }
}

fn despawn_missed_notes_system(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    mut judged: ResMut<JudgedChips>,
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
            judged.0.insert(note.chip_id);
            if !hit_lanes[note.lane as usize] {
                missed.write(NoteMissed {
                    lane: note.lane,
                    audio_ms: now,
                    chip_idx: note.chip_id,
                });
                hit_lanes[note.lane as usize] = true;
            }
            commands.entity(entity).despawn();
        } else if judged.0.contains(&note.chip_id) {
            commands.entity(entity).despawn();
        }
    }
}

fn despawn_passed_preview_notes_system(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    flow: Res<crate::practice::PracticeFlow>,
    mut skipped: ResMut<crate::seek::PreviewSkippedChips>,
    notes: Query<(Entity, &Note), With<NoteVisual>>,
    mut commands: Commands,
) {
    if *mode != EGameMode::Drums
        || flow.phase == crate::practice::PracticePhase::Running
        || !clock.is_ready()
    {
        return;
    }
    let now = clock.current_ms;
    for (entity, note) in &notes {
        if should_emit_miss_for_note(note.chip_id, now, note.target_ms, &skipped.0)
            || skipped.0.contains(&note.chip_id)
        {
            skipped.0.insert(note.chip_id);
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

    #[test]
    fn lookahead_fills_lane_at_default_scroll() {
        use crate::layout::PlayfieldLayout;
        use crate::resources::ScrollSettings;

        let layout = PlayfieldLayout::default();
        let scroll = ScrollSettings::from_scroll_speed(1.0);
        let window = lookahead_ms(&layout, &scroll);
        let px = scroll.pixels_per_ms * layout.scale;
        let top = top_for_note(window, 0, layout.judge_y(), px);
        assert!(
            (top - layout.lane_top()).abs() < 1.0,
            "note at lookahead boundary should sit at lane top (got {top}, expected {})",
            layout.lane_top()
        );
    }

    #[test]
    fn faster_scroll_shortens_lookahead() {
        use crate::layout::PlayfieldLayout;
        use crate::resources::ScrollSettings;

        let layout = PlayfieldLayout::default();
        let slow = lookahead_ms(&layout, &ScrollSettings::from_scroll_speed(1.0));
        let fast = lookahead_ms(&layout, &ScrollSettings::from_scroll_speed(2.0));
        assert!(fast < slow);
    }
}
