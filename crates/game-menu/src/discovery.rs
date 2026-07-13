//! Pure eligibility rules for Song Select discovery filters.

use std::collections::HashSet;
use std::path::Path;

use bevy::prelude::*;
use dtx_library::{LibraryPreferences, SongInfo};
use dtx_scoring::ScoreStore;

/// Filters combine with search and sorting; all disabled means the full library.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryFilters {
    pub favorites_only: bool,
    pub unplayed_only: bool,
    pub recent_only: bool,
    pub near_level_only: bool,
}

impl DiscoveryFilters {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn active_description(&self) -> String {
        let mut names = Vec::new();
        if self.favorites_only {
            names.push("Favorites");
        }
        if self.unplayed_only {
            names.push("Unplayed");
        }
        if self.recent_only {
            names.push("Recent");
        }
        if self.near_level_only {
            names.push("Near My Level");
        }
        if names.is_empty() {
            "All Songs".into()
        } else {
            names.join(" + ")
        }
    }
}

/// Returns source indices that satisfy every active discovery constraint.
pub fn filtered_indices(
    songs: &[SongInfo],
    preferences: &LibraryPreferences,
    scores: &ScoreStore,
    filters: &DiscoveryFilters,
) -> Vec<usize> {
    let recent = recent_paths(scores);
    let near_target = near_level_target(scores);
    songs
        .iter()
        .enumerate()
        .filter_map(|(index, song)| {
            let played = song_played(scores, &song.path);
            let matches = (!filters.favorites_only || preferences.is_favorite(&song.path))
                && (!filters.unplayed_only || !played)
                && (!filters.recent_only || recent.contains(&path_key(&song.path)))
                && (!filters.near_level_only
                    || near_target.is_some_and(|target| {
                        song.dlevel
                            .map(dtx_core::display_dlevel)
                            .is_some_and(|level| (level - target).abs() <= 1.0)
                    }));
            matches.then_some(index)
        })
        .collect()
}

pub fn random_candidate(indices: &[usize], state: &mut u64) -> Option<usize> {
    if indices.is_empty() {
        return None;
    }
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    Some(indices[(*state as usize) % indices.len()])
}

fn song_played(scores: &ScoreStore, path: &Path) -> bool {
    let key = path_key(path);
    scores.entries.iter().any(|entry| {
        entry
            .chart
            .source_path_hint
            .as_deref()
            .is_some_and(|entry_path| path_key(entry_path) == key)
    })
}

fn recent_paths(scores: &ScoreStore) -> HashSet<String> {
    let mut entries: Vec<_> = scores
        .entries
        .iter()
        .filter_map(|entry| {
            entry
                .chart
                .source_path_hint
                .as_deref()
                .map(|path| (entry.played_at, path_key(path)))
        })
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    entries.into_iter().take(20).map(|(_, path)| path).collect()
}

fn near_level_target(scores: &ScoreStore) -> Option<f32> {
    let mut levels: Vec<f32> = scores
        .entries
        .iter()
        .filter(|entry| entry.total() > 0 && entry.chart_level > 0.0)
        .map(|entry| entry.chart_level as f32)
        .collect();
    levels.sort_by(|a, b| a.total_cmp(b));
    levels.get(levels.len() / 2).copied()
}

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_scoring::identity::ChartIdentity;
    use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource};

    #[test]
    fn random_candidate_stays_within_candidates() {
        let mut state = 7;
        let candidate = random_candidate(&[2, 5, 9], &mut state).expect("candidate");
        assert!([2, 5, 9].contains(&candidate));
    }

    #[test]
    fn active_description_names_combined_filters() {
        assert_eq!(
            DiscoveryFilters {
                favorites_only: true,
                unplayed_only: true,
                ..default()
            }
            .active_description(),
            "Favorites + Unplayed"
        );
    }

    #[test]
    fn unplayed_and_recent_filters_follow_score_history_paths() {
        let songs = vec![test_song("A", 50), test_song("B", 60)];
        let mut scores = ScoreStore::default();
        scores.add(test_score(songs[0].path.clone(), 100));
        let preferences = LibraryPreferences::with_path("unused.json".into());

        assert_eq!(
            filtered_indices(
                &songs,
                &preferences,
                &scores,
                &DiscoveryFilters {
                    unplayed_only: true,
                    ..default()
                }
            ),
            vec![1]
        );
        assert_eq!(
            filtered_indices(
                &songs,
                &preferences,
                &scores,
                &DiscoveryFilters {
                    recent_only: true,
                    ..default()
                }
            ),
            vec![0]
        );
    }

    fn test_song(title: &str, dlevel: u32) -> SongInfo {
        SongInfo {
            path: format!("/songs/{title}.dtx").into(),
            title: title.into(),
            artist: String::new(),
            bpm: None,
            dlevel: Some(dlevel),
            bgm_path: None,
            preview_path: None,
            preview_is_loopable: false,
            preimage_path: None,
        }
    }

    fn test_score(path: std::path::PathBuf, played_at: u64) -> ScoreEntry {
        ScoreEntry {
            id: "entry".into(),
            chart: ChartIdentity::new("chart".into(), None, Some(path)),
            title: String::new(),
            artist: String::new(),
            score: 0,
            chart_level: 5.0,
            performance_skill: 0.0,
            song_skill: 0.0,
            max_combo: 1,
            judgments: JudgmentTotals {
                perfect: 1,
                ..default()
            },
            rank: Rank::Unknown,
            played_at,
            source: ScoreSource::Native,
            replay_ref: None,
        }
    }
}
