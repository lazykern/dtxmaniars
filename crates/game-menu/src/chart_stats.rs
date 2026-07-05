//! Selected-chart statistics for song select: per-lane density,
//! total notes (async parse, off the main thread) and the GITADORA
//! display skill formula.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use dtx_core::channel::EChannel;
use dtx_ui::widget::density_graph::{DensityData, LANE_COUNT};
use game_shell::AppState;
use std::path::PathBuf;

use crate::song_select::{Selection, SongSelectSelection};

/// Wheel-row skill number, always two decimals (e.g. "79.17").
pub fn row_skill_text(skill: f32) -> String {
    format!("{skill:.2}")
}

/// Achievement percent clamped to a 0..=100 progress-bar fill.
pub fn bar_fill_pct(achievement: f32) -> f32 {
    achievement.clamp(0.0, 100.0)
}

/// Display skill: level × achievement% / 100 × 20.
/// 100% on a 9.80 chart = 196.0 skill points. Display-only.
pub fn skill_points(dlevel: Option<u32>, achievement_pct: f32) -> f32 {
    let level = dlevel.map(dtx_core::display_dlevel).unwrap_or(0.0);
    level * (achievement_pct / 100.0) * 20.0
}

/// Map a drum channel to its density-graph display lane
/// (LC HH LP SD HT BD LT FT CY — matches Theme::lane_colors order).
pub fn display_lane(ch: EChannel) -> Option<usize> {
    Some(match ch {
        EChannel::LeftCymbal => 0,
        EChannel::HiHatClose | EChannel::HiHatOpen => 1,
        EChannel::LeftPedal | EChannel::LeftBassDrum => 2,
        EChannel::Snare => 3,
        EChannel::HighTom => 4,
        EChannel::BassDrum => 5,
        EChannel::LowTom => 6,
        EChannel::FloorTom => 7,
        EChannel::Cymbal | EChannel::RideCymbal => 8,
        _ => return None,
    })
}

/// Compute per-lane counts from a parsed chart.
pub fn lane_counts(chart: &dtx_core::Chart) -> ([u32; LANE_COUNT], u32) {
    let mut lanes = [0u32; LANE_COUNT];
    let mut total = 0u32;
    for chip in &chart.chips {
        if let Some(lane) = display_lane(chip.channel) {
            lanes[lane] += 1;
            total += 1;
        }
    }
    (lanes, total)
}

/// In-flight stats parse for the currently selected chart path.
#[derive(Resource, Default)]
pub struct ChartStatsTask {
    pub task: Option<Task<Option<(PathBuf, [u32; LANE_COUNT], u32)>>>,
    pub for_path: Option<PathBuf>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<ChartStatsTask>().add_systems(
        Update,
        (start_stats_task, poll_stats_task).run_if(in_state(AppState::SongSelect)),
    );
}

/// Kick a background parse when the selected chart path changes.
fn start_stats_task(
    selection: Res<Selection>,
    selection_state: Res<SongSelectSelection>,
    db: Res<dtx_library::SongDb>,
    mut task: ResMut<ChartStatsTask>,
    mut data: ResMut<DensityData>,
) {
    if !selection.is_changed() && task.for_path.is_some() {
        return;
    }
    let path = selection
        .chart_index(&selection_state)
        .and_then(|i| db.songs.get(i))
        .map(|s| s.path.clone());
    if path == task.for_path {
        return;
    }
    task.for_path = path.clone();
    let Some(path) = path else {
        *data = DensityData::default();
        task.task = None;
        return;
    };
    let pool = AsyncComputeTaskPool::get();
    task.task = Some(pool.spawn(async move {
        let bytes = std::fs::read(&path).ok()?;
        let chart = dtx_core::parse(bytes.as_slice()).ok()?;
        let (lanes, total) = lane_counts(&chart);
        Some((path, lanes, total))
    }));
}

/// Publish finished stats (discard if the selection moved on).
fn poll_stats_task(mut task: ResMut<ChartStatsTask>, mut data: ResMut<DensityData>) {
    let Some(active) = task.task.as_mut() else {
        return;
    };
    let Some(result) = block_on(future::poll_once(active)) else {
        return;
    };
    task.task = None;
    if let Some((path, lanes, total)) = result {
        if task.for_path.as_ref() == Some(&path) {
            *data = DensityData { lanes, total };
        }
    } else {
        *data = DensityData::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_skill_text_two_decimals() {
        assert_eq!(row_skill_text(79.17), "79.17");
        assert_eq!(row_skill_text(0.0), "0.00");
        assert_eq!(row_skill_text(2.9), "2.90");
    }

    #[test]
    fn bar_fill_pct_clamps_0_to_100() {
        assert_eq!(bar_fill_pct(64.8), 64.8);
        assert_eq!(bar_fill_pct(0.0), 0.0);
        assert_eq!(bar_fill_pct(100.0), 100.0);
        assert_eq!(bar_fill_pct(-5.0), 0.0);
        assert_eq!(bar_fill_pct(123.4), 100.0);
    }

    #[test]
    fn skill_formula_matches_gitadora_shape() {
        assert!((skill_points(Some(98), 100.0) - 196.0).abs() < 0.01);
        assert!((skill_points(Some(355), 100.0) - 71.0).abs() < 0.01);
        assert!((skill_points(Some(78), 93.04) - (7.8 * 0.9304 * 20.0)).abs() < 0.01);
        assert_eq!(skill_points(None, 100.0), 0.0);
        assert_eq!(skill_points(Some(50), 0.0), 0.0);
    }

    #[test]
    fn display_lane_groups_hh_and_cy() {
        assert_eq!(
            display_lane(EChannel::HiHatClose),
            display_lane(EChannel::HiHatOpen)
        );
        assert_eq!(
            display_lane(EChannel::Cymbal),
            display_lane(EChannel::RideCymbal)
        );
        assert_eq!(display_lane(EChannel::HighTom), Some(4));
        assert_eq!(display_lane(EChannel::BassDrum), Some(5));
        assert_eq!(display_lane(EChannel::Snare), Some(3));
    }

    #[test]
    fn display_lane_ignores_non_drum() {
        // A non-drum channel (BGM) maps to no display lane.
        assert_eq!(display_lane(EChannel::BGM), None);
    }
}
