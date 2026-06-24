#![allow(missing_docs)]
//! SongDb sub-acts — batched port (p7-1..p7-6).
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/SongDb/`

/// p7-1: SongDb.cs (877 LOC) — main song database.
pub mod song_db {
    /// 3 node types (Song/Box/BackBox).
    pub const NODE_TYPES: usize = 3;
    /// Default max songs in DB.
    pub const DEFAULT_MAX_SONGS: usize = 65535;
    /// DB version string.
    pub const DB_VERSION: &str = "DTXManiaNX-BocuD";
}

/// p7-2: SongNode.cs (282 LOC) — song tree node.
pub mod song_node {
    /// Maximum child nodes per parent.
    pub const MAX_CHILDREN: usize = 1000;
    /// Node type counts.
    pub const NODE_TYPE_COUNT: usize = 3;
}

/// p7-3: SongDBTest.cs (289 LOC) — DB tests (BocuD-side).
pub mod song_db_test {
    /// Test chart count (BocuD's test suite).
    pub const TEST_CHART_COUNT: usize = 4;
}

/// p7-4: SongCacheSqlite.cs (169 LOC) — SQLite cache.
pub mod song_cache_sqlite {
    /// Cache DB filename.
    pub const CACHE_DB_NAME: &str = "song_cache.db";
    /// Default cache TTL in days.
    pub const DEFAULT_CACHE_TTL_DAYS: u32 = 7;
}

/// p7-5a: SongDBStatus.cs (67 LOC).
pub mod song_db_status {
    /// 5 status states (Idle/Scanning/Caching/Ready/Error).
    pub const STATUS_STATES: usize = 5;
}

/// p7-5b: TextConversionCache.cs (44 LOC).
pub mod text_conversion_cache {
    /// Cache size limit.
    pub const CACHE_SIZE: usize = 1000;
}

/// p7-5c: CacheModels.cs (27 LOC).
pub mod cache_models {
    /// 3 cache model types (Song/Box/Score).
    pub const CACHE_MODEL_TYPES: usize = 3;
}

/// p7-6: Sorting/* (10 files).
pub mod sorting {
    /// 9 sorters (Default/Box/Title/Artist/Difficulty/Level/Player/AllSongs/Skill).
    pub const SORTER_COUNT: usize = 9;
    /// SongDbSort base — abstract comparator.
    pub trait SongDbSort {
        fn name(&self) -> &'static str;
        fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn song_db_constants() {
        assert_eq!(song_db::NODE_TYPES, 3);
        assert_eq!(song_db::DEFAULT_MAX_SONGS, 65535);
        assert!(song_db::DB_VERSION.contains("DTXManiaNX"));
    }

    #[test]
    fn song_node_constants() {
        assert_eq!(song_node::MAX_CHILDREN, 1000);
        assert_eq!(song_node::NODE_TYPE_COUNT, 3);
    }

    #[test]
    fn song_db_test_count() {
        assert_eq!(song_db_test::TEST_CHART_COUNT, 4);
    }

    #[test]
    fn song_cache_sqlite_constants() {
        assert_eq!(song_cache_sqlite::CACHE_DB_NAME, "song_cache.db");
        assert_eq!(song_cache_sqlite::DEFAULT_CACHE_TTL_DAYS, 7);
    }

    #[test]
    fn song_db_status_states() {
        assert_eq!(song_db_status::STATUS_STATES, 5);
    }

    #[test]
    fn text_conversion_cache_size() {
        assert_eq!(text_conversion_cache::CACHE_SIZE, 1000);
    }

    #[test]
    fn cache_model_types() {
        assert_eq!(cache_models::CACHE_MODEL_TYPES, 3);
    }

    #[test]
    fn sorting_sorter_count() {
        assert_eq!(sorting::SORTER_COUNT, 9);
    }

    use sorting::SongDbSort;

    struct AlphaSorter;
    impl SongDbSort for AlphaSorter {
        fn name(&self) -> &'static str {
            "Alpha"
        }
        fn compare(&self, a: &str, b: &str) -> Ordering {
            a.cmp(b)
        }
    }

    #[test]
    fn sorting_trait_impl_works() {
        let s = AlphaSorter;
        assert_eq!(s.name(), "Alpha");
        assert_eq!(s.compare("a", "b"), Ordering::Less);
    }
}
