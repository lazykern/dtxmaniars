//! `CAVI` (108 LOC) — AVI video clip handle.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CAVI.cs:1-108`
//!
//! v1 strict-port: register + lookup for AVI video clips used by
//! Movie/MovieFull BGA channels. We don't decode (FFmpeg deferred).

use std::collections::HashMap;

/// Maximum cached AVIs (BocuD CAVI.cs:30).
pub const MAX_CACHED_AVIS: usize = 8;

/// One registered AVI clip (BocuD CAVI.cs:30-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaviClip {
    /// Filename without extension.
    pub name: String,
    /// Path on disk.
    pub path: std::path::PathBuf,
    /// Whether the file exists at load time.
    pub exists: bool,
}

/// AVI registry (BocuD CAVI.cs:50-100).
#[derive(Debug, Clone, Default)]
pub struct CAVI {
    clips: HashMap<String, CaviClip>,
}

impl CAVI {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an AVI clip. Returns false if cache is full.
    pub fn register(&mut self, name: String, path: std::path::PathBuf) -> bool {
        if self.clips.len() >= MAX_CACHED_AVIS && !self.clips.contains_key(&name) {
            return false;
        }
        let exists = path.exists();
        self.clips
            .insert(name.clone(), CaviClip { name, path, exists });
        true
    }

    /// Lookup a clip by name.
    pub fn get(&self, name: &str) -> Option<&CaviClip> {
        self.clips.get(name)
    }

    /// Number of registered clips.
    pub fn len(&self) -> usize {
        self.clips.len()
    }

    /// True if no clips registered.
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    /// Drop all clips and return their paths.
    pub fn clear(&mut self) -> Vec<std::path::PathBuf> {
        let paths: Vec<_> = self.clips.values().map(|c| c.path.clone()).collect();
        self.clips.clear();
        paths
    }

    /// Iterate all registered clips.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &CaviClip)> {
        self.clips.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cavi_register_and_get() {
        let mut r = CAVI::new();
        assert!(r.register("intro".into(), std::path::PathBuf::from("/x.avi")));
        assert_eq!(r.len(), 1);
        let c = r.get("intro").unwrap();
        assert_eq!(c.name, "intro");
    }

    #[test]
    fn cavi_register_replaces_existing() {
        let mut r = CAVI::new();
        r.register("k".into(), std::path::PathBuf::from("/a.avi"));
        r.register("k".into(), std::path::PathBuf::from("/b.avi"));
        assert_eq!(r.len(), 1);
        assert_eq!(r.get("k").unwrap().path, std::path::PathBuf::from("/b.avi"));
    }

    #[test]
    fn cavi_max_cached_avis() {
        assert_eq!(MAX_CACHED_AVIS, 8);
    }

    #[test]
    fn cavi_cache_full_returns_false() {
        let mut r = CAVI::new();
        for i in 0..MAX_CACHED_AVIS {
            assert!(r.register(format!("k{i}"), std::path::PathBuf::from("/x.avi")));
        }
        // Adding a new key when full should fail
        assert!(!r.register("k_new".into(), std::path::PathBuf::from("/y.avi")));
        // Replacing an existing key still works
        assert!(r.register("k0".into(), std::path::PathBuf::from("/z.avi")));
    }

    #[test]
    fn cavi_clear_returns_paths() {
        let mut r = CAVI::new();
        r.register("a".into(), std::path::PathBuf::from("/a.avi"));
        r.register("b".into(), std::path::PathBuf::from("/b.avi"));
        let paths = r.clear();
        assert_eq!(paths.len(), 2);
        assert!(r.is_empty());
    }

    #[test]
    fn cavi_iter() {
        let mut r = CAVI::new();
        r.register("x".into(), std::path::PathBuf::from("/x.avi"));
        r.register("y".into(), std::path::PathBuf::from("/y.avi"));
        let names: Vec<_> = r.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn cavi_get_missing_returns_none() {
        let r = CAVI::new();
        assert!(r.get("missing").is_none());
    }
}
