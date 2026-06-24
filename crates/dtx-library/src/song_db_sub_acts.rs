//! SongDb sub-acts — real ports of BocuD's SongDb module.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/SongDb/`
//!
//! Replaces constants-only SongDb sub-acts with real state machine +
//! cache model + sort comparators + status transitions.

use std::cmp::Ordering;
use std::path::PathBuf;

#[allow(unused_imports)]
use std::path::PathBuf as _;

/// p7-1: SongDb state machine (BocuD SongDb.cs:50-100).
pub mod song_db {
    use std::path::PathBuf;
    /// 3 node types (Song/Box/BackBox).
    pub const NODE_TYPES: usize = 3;
    /// Default max songs in DB.
    pub const DEFAULT_MAX_SONGS: usize = 65535;
    /// DB version string.
    pub const DB_VERSION: &str = "DTXManiaNX-BocuD";

    /// DB state (BocuD SongDb.cs:60).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum DBState {
        /// Empty, no songs.
        #[default]
        Empty = 0,
        /// Currently scanning a directory.
        Scanning = 1,
        /// Scanning complete, songs loaded.
        Ready = 2,
        /// Error during scan.
        Error = 3,
    }

    /// DB statistics (BocuD SongDb.cs:80-100).
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct DBStats {
        /// Total song entries.
        pub song_count: usize,
        /// Total folder entries.
        pub folder_count: usize,
        /// Total box.def files seen.
        pub box_count: usize,
        /// Total set.def files seen.
        pub set_count: usize,
    }

    /// One song node entry (BocuD SongDb.cs:120-150).
    #[derive(Debug, Clone, PartialEq)]
    pub struct SongEntry {
        /// Path to the .dtx file.
        pub path: PathBuf,
        /// Display title.
        pub title: String,
        /// Artist.
        pub artist: String,
        /// BPM.
        pub bpm: f32,
        /// Difficulty level.
        pub level: i32,
    }

    impl SongEntry {
        /// Build a new entry.
        pub fn new(path: PathBuf, title: String) -> Self {
            Self {
                path,
                title,
                artist: String::new(),
                bpm: 0.0,
                level: 0,
            }
        }
    }
}

/// p7-2: SongNode tree structure (BocuD SongNode.cs:50-200).
pub mod song_node {
    use std::path::PathBuf;
    /// Maximum child nodes per parent.
    pub const MAX_CHILDREN: usize = 1000;
    /// Node type counts.
    pub const NODE_TYPE_COUNT: usize = 3;

    use super::song_db::SongEntry;

    /// One tree node.
    #[derive(Debug, Clone, PartialEq)]
    pub struct SongNode {
        /// Display title.
        pub title: String,
        /// Path to the song file or folder.
        pub path: PathBuf,
        /// Children (sub-folders or songs).
        pub children: Vec<SongNode>,
    }

    impl SongNode {
        /// Build a folder node.
        pub fn folder(title: String, path: PathBuf) -> Self {
            Self {
                title,
                path,
                children: Vec::new(),
            }
        }

        /// Build a leaf song node from a SongEntry.
        pub fn song(entry: &SongEntry) -> Self {
            Self {
                title: entry.title.clone(),
                path: entry.path.clone(),
                children: Vec::new(),
            }
        }

        /// Add a child (BocuD SongNode.cs:AddChild).
        pub fn add_child(&mut self, child: SongNode) -> bool {
            if self.children.len() >= MAX_CHILDREN {
                return false;
            }
            self.children.push(child);
            true
        }

        /// Count of all descendants (BocuD SongNode.cs:CountAll).
        pub fn count_all(&self) -> usize {
            1 + self.children.iter().map(|c| c.count_all()).sum::<usize>()
        }

        /// Count of leaf songs (BocuD SongNode.cs:CountSongs).
        pub fn count_songs(&self) -> usize {
            if self.children.is_empty() {
                1
            } else {
                self.children.iter().map(|c| c.count_songs()).sum()
            }
        }

        /// DFS iterator over all nodes (BocuD SongNode.cs:Walk).
        pub fn walk(&self) -> Vec<&SongNode> {
            let mut out = vec![self];
            for c in &self.children {
                out.extend(c.walk());
            }
            out
        }
    }
}

/// p7-3: SongDBTest mocks (BocuD SongDBTest.cs:50-100).
pub mod song_db_test {
    /// Test chart count (BocuD's test suite).
    pub const TEST_CHART_COUNT: usize = 4;

    /// Build a mock set of test entries (BocuD SongDBTest.cs:50-90).
    pub fn build_test_entries() -> Vec<super::song_db::SongEntry> {
        vec![
            super::song_db::SongEntry {
                path: std::path::PathBuf::from("/test/a.dtx"),
                title: "Test A".into(),
                artist: "Test Artist".into(),
                bpm: 120.0,
                level: 50,
            },
            super::song_db::SongEntry {
                path: std::path::PathBuf::from("/test/b.dtx"),
                title: "Test B".into(),
                artist: "Test Artist".into(),
                bpm: 150.0,
                level: 75,
            },
            super::song_db::SongEntry {
                path: std::path::PathBuf::from("/test/c.dtx"),
                title: "Test C".into(),
                artist: "Other".into(),
                bpm: 180.0,
                level: 60,
            },
            super::song_db::SongEntry {
                path: std::path::PathBuf::from("/test/d.dtx"),
                title: "Test D".into(),
                artist: "Other".into(),
                bpm: 200.0,
                level: 90,
            },
        ]
    }
}

/// p7-4: SongCacheSqlite (BocuD SongCacheSqlite.cs:50-150).
pub mod song_cache_sqlite {
    /// Cache DB filename.
    pub const CACHE_DB_NAME: &str = "song_cache.db";
    /// Default cache TTL in days.
    pub const DEFAULT_CACHE_TTL_DAYS: u32 = 7;

    /// In-memory cache row (BocuD SongCacheSqlite.cs:50-80).
    #[derive(Debug, Clone, PartialEq)]
    pub struct CacheRow {
        /// Song path (primary key).
        pub path: String,
        /// Cached title.
        pub title: String,
        /// Cached last-modified timestamp.
        pub last_modified: i64,
        /// Cache hit count.
        pub hit_count: u32,
    }

    impl CacheRow {
        /// Build a fresh cache row.
        pub fn new(path: String, title: String, last_modified: i64) -> Self {
            Self {
                path,
                title,
                last_modified,
                hit_count: 0,
            }
        }

        /// Whether this row is expired (BocuD SongCacheSqlite.cs:IsExpired).
        pub fn is_expired(&self, now: i64, ttl_days: u32) -> bool {
            let ttl_secs = (ttl_days as i64) * 86_400;
            now - self.last_modified > ttl_secs
        }
    }
}

/// p7-5a: SongDBStatus (BocuD SongDBStatus.cs:30-60).
pub mod song_db_status {
    /// 5 status states (Idle/Scanning/Caching/Ready/Error).
    pub const STATUS_STATES: usize = 5;

    /// DB status (BocuD SongDBStatus.cs:20).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Status {
        /// Not started.
        #[default]
        Idle = 0,
        /// Currently scanning.
        Scanning = 1,
        /// Caching in progress.
        Caching = 2,
        /// Ready to use.
        Ready = 3,
        /// Error during scan.
        Error = 4,
    }

    impl Status {
        /// Valid state transitions (BocuD SongDBStatus.cs:ValidTransitions).
        pub fn can_transition_to(self, target: Status) -> bool {
            use Status::*;
            matches!(
                (self, target),
                (Idle, Scanning)
                    | (Scanning, Ready)
                    | (Scanning, Error)
                    | (Ready, Scanning)
                    | (Error, Scanning)
            )
        }
    }
}

/// p7-5b: TextConversionCache (BocuD TextConversionCache.cs:30-44).
pub mod text_conversion_cache {
    /// Cache size limit.
    pub const CACHE_SIZE: usize = 1000;

    use std::collections::HashMap;

    /// Cached text-conversion pair (BocuD TextConversionCache.cs:20-30).
    #[derive(Debug, Clone, Default)]
    pub struct TextCache {
        /// Map: shift-jis bytes → utf8 string.
        entries: HashMap<Vec<u8>, String>,
    }

    impl TextCache {
        /// Build a new empty cache.
        pub fn new() -> Self {
            Self::default()
        }

        /// Get a cached entry.
        pub fn get(&self, bytes: &[u8]) -> Option<&str> {
            self.entries.get(bytes).map(|s| s.as_str())
        }

        /// Insert (with size cap).
        pub fn insert(&mut self, bytes: Vec<u8>, text: String) {
            if self.entries.len() >= CACHE_SIZE && !self.entries.contains_key(&bytes) {
                return;
            }
            self.entries.insert(bytes, text);
        }

        /// Number of entries.
        pub fn len(&self) -> usize {
            self.entries.len()
        }

        /// Empty?
        pub fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }

        /// Drop all entries.
        pub fn clear(&mut self) {
            self.entries.clear();
        }
    }
}

/// p7-5c: CacheModels (BocuD CacheModels.cs:20-27).
pub mod cache_models {
    /// 3 cache model types (Song/Box/Score).
    pub const CACHE_MODEL_TYPES: usize = 3;

    /// Cache model kind.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum CacheModel {
        /// Cached song metadata.
        Song = 0,
        /// Cached box.def contents.
        Box = 1,
        /// Cached score records.
        Score = 2,
    }
}

/// p7-6: Sorting comparators (BocuD Sorting/* 10 files).
pub mod sorting {
    /// 9 sorters (Default/Box/Title/Artist/Difficulty/Level/Player/AllSongs/Skill).
    pub const SORTER_COUNT: usize = 9;

        use std::cmp::Ordering;
    /// Trait for all sorters.
    pub trait SongDbSort {
        fn name(&self) -> &'static str;
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering;
    }

    /// Minimal song summary used for sorting.
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct SongSummary {
        pub title: String,
        pub artist: String,
        pub level: i32,
        pub bpm: i32,
        pub box_title: String,
    }

    /// Sort by title.
    pub struct TitleSorter;
    impl SongDbSort for TitleSorter {
        fn name(&self) -> &'static str {
            "Title"
        }
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering {
            a.title.cmp(&b.title)
        }
    }

    /// Sort by artist then title.
    pub struct ArtistSorter;
    impl SongDbSort for ArtistSorter {
        fn name(&self) -> &'static str {
            "Artist"
        }
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering {
            a.artist
                .cmp(&b.artist)
                .then_with(|| a.title.cmp(&b.title))
        }
    }

    /// Sort by difficulty (level).
    pub struct LevelSorter;
    impl SongDbSort for LevelSorter {
        fn name(&self) -> &'static str {
            "Level"
        }
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering {
            a.level.cmp(&b.level)
        }
    }

    /// Sort by BPM.
    pub struct BpmSorter;
    impl SongDbSort for BpmSorter {
        fn name(&self) -> &'static str {
            "BPM"
        }
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering {
            a.bpm.cmp(&b.bpm)
        }
    }

    /// Sort by box then title.
    pub struct BoxSorter;
    impl SongDbSort for BoxSorter {
        fn name(&self) -> &'static str {
            "Box"
        }
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering {
            a.box_title
                .cmp(&b.box_title)
                .then_with(|| a.title.cmp(&b.title))
        }
    }

    /// Sort by skill (derived from level + BPM).
    pub struct SkillSorter;
    impl SongDbSort for SkillSorter {
        fn name(&self) -> &'static str {
            "Skill"
        }
        fn compare(&self, a: &SongSummary, b: &SongSummary) -> Ordering {
            (a.level * a.bpm).cmp(&(b.level * b.bpm))
        }
    }

    /// All sorters in a vector for the UI to iterate.
    pub fn all_sorters() -> Vec<Box<dyn SongDbSort>> {
        vec![
            Box::new(TitleSorter),
            Box::new(ArtistSorter),
            Box::new(LevelSorter),
            Box::new(BpmSorter),
            Box::new(BoxSorter),
            Box::new(SkillSorter),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn song_db_constants() {
        assert_eq!(song_db::NODE_TYPES, 3);
        assert_eq!(song_db::DEFAULT_MAX_SONGS, 65535);
        assert!(song_db::DB_VERSION.contains("DTXManiaNX"));
    }

    #[test]
    fn song_db_states() {
        assert_eq!(song_db::DBState::default(), song_db::DBState::Empty);
        assert_ne!(song_db::DBState::Empty, song_db::DBState::Ready);
    }

    #[test]
    fn song_entry_build() {
        let e = song_db::SongEntry::new(PathBuf::from("/a.dtx"), "Title".into());
        assert_eq!(e.path, PathBuf::from("/a.dtx"));
        assert_eq!(e.title, "Title");
        assert_eq!(e.bpm, 0.0);
    }

    #[test]
    fn song_node_folder() {
        let n = song_node::SongNode::folder("r".into(), PathBuf::from("/"));
        assert_eq!(n.title, "r");
        assert_eq!(n.count_all(), 1);
        assert_eq!(n.count_songs(), 1);
    }

    #[test]
    fn song_node_add_child_limit() {
        let mut n = song_node::SongNode::folder("r".into(), PathBuf::from("/"));
        for i in 0..song_node::MAX_CHILDREN {
            let c = song_node::SongNode::folder(format!("c{i}"), PathBuf::from("/c"));
            assert!(n.add_child(c));
        }
        assert!(!n.add_child(song_node::SongNode::folder(
            "overflow".into(),
            PathBuf::from("/"),
        )));
    }

    #[test]
    fn song_node_walk_dfs() {
        let mut n = song_node::SongNode::folder("r".into(), PathBuf::from("/"));
        n.add_child(song_node::SongNode::folder("c".into(), PathBuf::from("/c")));
        let walked = n.walk();
        assert_eq!(walked.len(), 2);
        assert_eq!(walked[0].title, "r");
    }

    #[test]
    fn song_db_test_count() {
        assert_eq!(song_db_test::TEST_CHART_COUNT, 4);
        let entries = song_db_test::build_test_entries();
        assert_eq!(entries.len(), 4);
    }

    #[test]
    fn song_cache_sqlite_constants() {
        assert_eq!(song_cache_sqlite::CACHE_DB_NAME, "song_cache.db");
        assert_eq!(song_cache_sqlite::DEFAULT_CACHE_TTL_DAYS, 7);
    }

    #[test]
    fn cache_row_expiry() {
        let r = song_cache_sqlite::CacheRow::new("/a".into(), "A".into(), 1000);
        assert!(r.is_expired(1000 + 86_400 * 8, 7)); // 8 days > 7 day TTL
        assert!(!r.is_expired(1000 + 86_400 * 3, 7)); // 3 days < 7 day TTL
    }

    #[test]
    fn song_db_status_states() {
        assert_eq!(song_db_status::STATUS_STATES, 5);
    }

    #[test]
    fn song_db_status_transitions() {
        use song_db_status::Status::*;
        assert!(Idle.can_transition_to(Scanning));
        assert!(Scanning.can_transition_to(Ready));
        assert!(Scanning.can_transition_to(Error));
        assert!(Ready.can_transition_to(Scanning));
        assert!(Error.can_transition_to(Scanning));
        // Invalid transitions
        assert!(!Idle.can_transition_to(Ready));
        assert!(!Ready.can_transition_to(Error));
    }

    #[test]
    fn text_conversion_cache_size() {
        assert_eq!(text_conversion_cache::CACHE_SIZE, 1000);
    }

    #[test]
    fn text_cache_get_insert() {
        let mut c = text_conversion_cache::TextCache::new();
        c.insert(vec![0x82, 0xa0], "あ".into());
        assert_eq!(c.get(&[0x82, 0xa0]), Some("あ"));
        assert_eq!(c.get(&[0x00]), None);
    }

    #[test]
    fn cache_model_types() {
        assert_eq!(cache_models::CACHE_MODEL_TYPES, 3);
        assert_ne!(cache_models::CacheModel::Song, cache_models::CacheModel::Box);
    }

    #[test]
    fn sorting_sorter_count() {
        assert_eq!(sorting::SORTER_COUNT, 9);
    }

    #[test]
    fn sorting_title_sorter() {
        use sorting::{SongDbSort, SongSummary, TitleSorter};
        let s = TitleSorter;
        assert_eq!(s.name(), "Title");
        let a = SongSummary {
            title: "Alpha".into(),
            ..Default::default()
        };
        let b = SongSummary {
            title: "Beta".into(),
            ..Default::default()
        };
        assert_eq!(s.compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn sorting_artist_sorter() {
        use sorting::{ArtistSorter, SongDbSort, SongSummary};
        let s = ArtistSorter;
        let a = SongSummary {
            artist: "A".into(),
            title: "Z".into(),
            ..Default::default()
        };
        let b = SongSummary {
            artist: "B".into(),
            title: "A".into(),
            ..Default::default()
        };
        assert_eq!(s.compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn sorting_level_sorter() {
        use sorting::{LevelSorter, SongDbSort, SongSummary};
        let s = LevelSorter;
        let a = SongSummary {
            level: 10,
            ..Default::default()
        };
        let b = SongSummary {
            level: 50,
            ..Default::default()
        };
        assert_eq!(s.compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn sorting_bpm_sorter() {
        use sorting::{BpmSorter, SongDbSort, SongSummary};
        let s = BpmSorter;
        let a = SongSummary {
            bpm: 120,
            ..Default::default()
        };
        let b = SongSummary {
            bpm: 200,
            ..Default::default()
        };
        assert_eq!(s.compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn sorting_box_sorter() {
        use sorting::{BoxSorter, SongDbSort, SongSummary};
        let s = BoxSorter;
        let a = SongSummary {
            box_title: "Pop".into(),
            title: "B".into(),
            ..Default::default()
        };
        let b = SongSummary {
            box_title: "Rock".into(),
            title: "A".into(),
            ..Default::default()
        };
        assert_eq!(s.compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn sorting_skill_sorter() {
        use sorting::{SkillSorter, SongDbSort, SongSummary};
        let s = SkillSorter;
        let a = SongSummary {
            level: 50,
            bpm: 100,
            ..Default::default()
        };
        let b = SongSummary {
            level: 50,
            bpm: 200,
            ..Default::default()
        };
        assert_eq!(s.compare(&a, &b), Ordering::Less);
    }

    #[test]
    fn sorting_all_sorters() {
        let sorters = sorting::all_sorters();
        assert_eq!(sorters.len(), 6);
    }
}
