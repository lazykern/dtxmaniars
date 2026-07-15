//! Scrolling bar/beat timing lines (BocuD `CStagePerfCommonScreen.cs:3502-3537`).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use dtx_core::beat_lines::{tick_to_measure_fraction, TimingLineKind};
use dtx_timing::math::{chip_time_ms_with_bpm_and_bar_changes, ChartTiming};
use dtx_ui::{theme::Theme, ThemeResource};

use crate::hud::HudRoot;
use crate::interp::RenderClock;
use crate::judge::{BarLengthChangeList, BpmChangeList};
use crate::layout::PlayfieldLayout;
use crate::resources::{
    ActiveChart, DrumAudioSettings, GameplayClock, MetronomeEnabled, MetronomeSound,
    ScrollSettings, ShowPerfInfo, ShowTimingLines, TimingLineCrossed, TimingLineList,
};
use crate::scroll::{lookahead_ms, top_for_note_f};
use game_shell::{AppState, EGameMode, PauseState};

const BACKFILL_MS: i64 = 500;
const DESPAWN_MARGIN_MS: i64 = 200;
const METRONOME_PATH: &str = "sounds/metronome.wav";
const BEAT_METRONOME_VOLUME: f32 = 0.35;

/// NX skin atlas heights (2px at 720p ref); scaled by playfield layout.
const BAR_LINE_HEIGHT: f32 = 2.0;
const BEAT_LINE_HEIGHT: f32 = 2.0;
const MEASURE_LABEL_OFFSET_Y: f32 = 14.0;

#[derive(Component)]
struct TimingLineVisual {
    line_id: usize,
}

#[derive(Component)]
struct TimingLineEntity {
    target_ms: i64,
    height: f32,
}

#[derive(Component)]
struct BarMeasureLabel {
    line_id: usize,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TimingLineCrossed>()
        .init_resource::<MetronomeSound>()
        .add_systems(
            OnEnter(AppState::Performance),
            (preload_metronome_sound, init_timing_lines),
        )
        .add_systems(
            Update,
            (
                spawn_timing_lines,
                scroll_timing_lines,
                despawn_timing_lines,
            )
                .chain()
                .in_set(crate::layout::PlayfieldLayoutConsumers)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running)),
        )
        .add_systems(
            FixedUpdate,
            tick_metronome_on_cross
                .in_set(super::DrumsSets::NoteSpawn)
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(crate::practice::gameplay_input_active),
        );
}

fn preload_metronome_sound(asset_server: Res<AssetServer>, mut metronome: ResMut<MetronomeSound>) {
    metronome.0 = Some(asset_server.load(METRONOME_PATH));
}

fn init_timing_lines(
    chart: Res<ActiveChart>,
    mut lines: ResMut<TimingLineList>,
    mut crossed: ResMut<TimingLineCrossed>,
) {
    *lines = TimingLineList::from_chart(&chart.chart);
    crossed.0.clear();
}

fn spawn_timing_lines(
    mut commands: Commands,
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    lines: Res<TimingLineList>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    show_lines: Res<ShowTimingLines>,
    show_perf_info: Res<ShowPerfInfo>,
    theme: Res<ThemeResource>,
    existing_lines: Query<&TimingLineVisual>,
    existing_labels: Query<&BarMeasureLabel>,
    hud_root: Query<Entity, With<HudRoot>>,
) {
    if *mode != EGameMode::Drums || !clock.is_ready() || lines.lines.is_empty() || !show_lines.0 {
        return;
    }
    let Ok(hud) = hud_root.single() else {
        return;
    };

    let existing_ids: HashSet<usize> = existing_lines.iter().map(|v| v.line_id).collect();
    let existing_label_ids: HashSet<usize> = existing_labels.iter().map(|v| v.line_id).collect();
    let now = clock.current_ms;
    let base_bpm = lines.base_bpm;
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };
    let judge_y = layout.judge_y();
    let px_per_ms = scroll.pixels_per_ms * layout.scale;
    let spawn_window_ms = lookahead_ms(&layout, &scroll);

    for (idx, line) in lines.lines.iter().enumerate() {
        if !line.visible || existing_ids.contains(&idx) {
            continue;
        }
        let (measure, fraction) = tick_to_measure_fraction(line.tick);
        let target_ms = chip_time_ms_with_bpm_and_bar_changes(measure, fraction, base_bpm, timing);
        if target_ms < now - BACKFILL_MS || target_ms > now + spawn_window_ms {
            continue;
        }

        let (height, color) = line_style(line.kind, layout.scale);
        let top = top_for_note_f(target_ms, now as f64, judge_y, px_per_ms) - height * 0.5;

        let entity = commands
            .spawn((
                TimingLineVisual { line_id: idx },
                TimingLineEntity { target_ms, height },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(layout.strip_left()),
                    top: Val::Px(top),
                    width: Val::Px(layout.strip_width()),
                    height: Val::Px(height),
                    ..default()
                },
                BackgroundColor(color),
            ))
            .id();
        commands.entity(hud).add_child(entity);

        if show_perf_info.0
            && line.kind == TimingLineKind::Bar
            && !existing_label_ids.contains(&idx)
        {
            let Some(measure_no) = line.measure_number() else {
                continue;
            };
            let label_top = top - MEASURE_LABEL_OFFSET_Y * layout.scale;
            commands.entity(hud).with_children(|root| {
                root.spawn((
                    BarMeasureLabel { line_id: idx },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(layout.measure_label_left()),
                        top: Val::Px(label_top),
                        ..default()
                    },
                    Text::new(measure_no.to_string()),
                    Theme::label_font(),
                    TextColor(theme.0.text_secondary),
                ));
            });
        }
    }
}

fn scroll_timing_lines(
    render: Res<RenderClock>,
    mode: Res<EGameMode>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    mut lines: Query<
        (&TimingLineVisual, &TimingLineEntity, &mut Node),
        (With<TimingLineVisual>, Without<BarMeasureLabel>),
    >,
    mut labels: Query<
        (&BarMeasureLabel, &mut Node),
        (With<BarMeasureLabel>, Without<TimingLineVisual>),
    >,
) {
    if *mode != EGameMode::Drums {
        return;
    }
    let now = render.now_ms();
    let judge_y = layout.judge_y();
    let px_per_ms = scroll.pixels_per_ms * layout.scale;
    let mut line_tops: HashMap<usize, f32> = HashMap::new();

    for (visual, line, mut node) in &mut lines {
        let top = top_for_note_f(line.target_ms, now, judge_y, px_per_ms) - line.height * 0.5;
        node.top = Val::Px(top);
        // Re-anchor horizontally too, so lines follow the strip across resizes.
        node.left = Val::Px(layout.strip_left());
        node.width = Val::Px(layout.strip_width());
        line_tops.insert(visual.line_id, top);
    }

    for (label, mut node) in &mut labels {
        if let Some(top) = line_tops.get(&label.line_id) {
            node.top = Val::Px(top - MEASURE_LABEL_OFFSET_Y * layout.scale);
        }
    }
}

fn despawn_timing_lines(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    layout: Res<PlayfieldLayout>,
    scroll: Res<ScrollSettings>,
    lines: Query<(Entity, &TimingLineEntity), With<TimingLineVisual>>,
    labels: Query<(Entity, &BarMeasureLabel)>,
    line_visuals: Query<&TimingLineVisual>,
    mut commands: Commands,
) {
    if *mode != EGameMode::Drums || !clock.is_ready() {
        return;
    }
    let now = clock.current_ms;
    let spawn_window_ms = lookahead_ms(&layout, &scroll);
    let mut despawned: HashSet<usize> = HashSet::new();

    for (entity, line) in &lines {
        if line.target_ms < now - BACKFILL_MS - DESPAWN_MARGIN_MS
            || line.target_ms > now + spawn_window_ms + DESPAWN_MARGIN_MS
        {
            if let Ok(visual) = line_visuals.get(entity) {
                despawned.insert(visual.line_id);
            }
            commands.entity(entity).despawn();
        }
    }

    for (entity, label) in &labels {
        if despawned.contains(&label.line_id) {
            commands.entity(entity).despawn();
        }
    }
}

fn tick_metronome_on_cross(
    clock: Res<GameplayClock>,
    mode: Res<EGameMode>,
    lines: Res<TimingLineList>,
    show_lines: Res<ShowTimingLines>,
    metronome_on: Res<MetronomeEnabled>,
    metronome_sound: Res<MetronomeSound>,
    audio_settings: Res<DrumAudioSettings>,
    mut crossed: ResMut<TimingLineCrossed>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    audio: Res<Audio>,
) {
    if *mode != EGameMode::Drums
        || !clock.is_ready()
        || !show_lines.0
        || !metronome_on.0
        || lines.lines.is_empty()
    {
        return;
    }

    let Some(source) = metronome_sound.0.as_ref() else {
        return;
    };

    let now = clock.current_ms;
    let base_bpm = lines.base_bpm;
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };

    for (idx, line) in lines.lines.iter().enumerate() {
        if !line.visible || crossed.0.contains(&idx) {
            continue;
        }
        let (measure, fraction) = tick_to_measure_fraction(line.tick);
        let target_ms = chip_time_ms_with_bpm_and_bar_changes(measure, fraction, base_bpm, timing);
        if now < target_ms {
            continue;
        }

        crossed.0.insert(idx);
        let volume = match line.kind {
            TimingLineKind::Bar => 1.0,
            TimingLineKind::Beat => BEAT_METRONOME_VOLUME,
        };
        let _ = dtx_audio::play_sfx_handle(
            &audio,
            source.clone(),
            100,
            0,
            audio_settings.master_volume * volume,
            1.0,
        );
    }
}

fn line_style(kind: TimingLineKind, scale: f32) -> (f32, Color) {
    match kind {
        TimingLineKind::Bar => (BAR_LINE_HEIGHT * scale, Color::srgba(1.0, 1.0, 1.0, 0.85)),
        TimingLineKind::Beat => (
            BEAT_LINE_HEIGHT * scale,
            Color::srgba(0.55, 0.55, 0.55, 0.55),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_line_brighter_than_beat() {
        let (bar_h, bar_c) = line_style(TimingLineKind::Bar, 1.0);
        let (beat_h, beat_c) = line_style(TimingLineKind::Beat, 1.0);
        assert_eq!(bar_h, beat_h);
        assert!(bar_c.alpha() > beat_c.alpha());
    }
}
