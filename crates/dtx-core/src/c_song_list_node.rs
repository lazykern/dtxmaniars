#![allow(non_snake_case)]
//! `CSongListNode` (92 LOC) — a single node in the song tree.
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Score,Song/CSongListNode.cs:1-92`
//!
//! v1 strict-port: tree node with children + parent + chart metadata.

use std::path::PathBuf;

/// Maximum children per node (BocuD CSongListNode.cs:30).
pub const MAX_CHILDREN: usize = 32;

/// Node type (BocuD CSongListNode.cs:20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NodeType {
    /// Folder node (BG path).
    #[default]
    Folder = 0,
    /// Set (a set.def file referencing multiple charts).
    Set = 1,
    /// Single DTX chart.
    Chart = 2,
    /// Box (a folder representing a BGA box).
    Box = 3,
}

/// A node in the song tree (BocuD CSongListNode.cs:30-80).
#[derive(Debug, Clone, Default)]
pub struct CSongListNode {
    /// Display title.
    pub title: String,
    /// Path on disk.
    pub path: PathBuf,
    /// Subtitle (artist, level, etc.).
    pub subtitle: String,
    /// Parent node title (empty for root).
    pub parent_title: String,
    /// Node type.
    pub node_type: NodeType,
    /// Difficulty level (0-99).
    pub level: i32,
    /// BPM.
    pub bpm: f32,
    /// Children (sub-folders / charts).
    pub children: Vec<CSongListNode>,
}

impl CSongListNode {
    /// Build a new folder node.
    pub fn folder(title: impl Into<String>, path: PathBuf) -> Self {
        Self {
            title: title.into(),
            path,
            node_type: NodeType::Folder,
            ..Default::default()
        }
    }

    /// Build a new chart node.
    pub fn chart(title: impl Into<String>, path: PathBuf, level: i32, bpm: f32) -> Self {
        Self {
            title: title.into(),
            path,
            node_type: NodeType::Chart,
            level,
            bpm,
            ..Default::default()
        }
    }

    /// Add a child. Returns false if children list is full.
    pub fn add_child(&mut self, child: CSongListNode) -> bool {
        if self.children.len() >= MAX_CHILDREN {
            return false;
        }
        self.children.push(child);
        true
    }

    /// Number of children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Is this a leaf (chart with no children)?
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Depth-first iterator over all descendants.
    pub fn walk(&self) -> Vec<&CSongListNode> {
        let mut out = vec![self];
        for child in &self.children {
            out.extend(child.walk());
        }
        out
    }

    /// Find the first chart descendant by title (case-sensitive).
    pub fn find_chart(&self, title: &str) -> Option<&CSongListNode> {
        self.walk()
            .into_iter()
            .find(|n| n.node_type == NodeType::Chart && n.title == title)
    }

    /// All chart descendants in DFS order.
    pub fn all_charts(&self) -> Vec<&CSongListNode> {
        self.walk()
            .into_iter()
            .filter(|n| n.node_type == NodeType::Chart)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_node() {
        let n = CSongListNode::folder("Songs", PathBuf::from("/songs"));
        assert_eq!(n.title, "Songs");
        assert_eq!(n.node_type, NodeType::Folder);
        assert!(n.is_leaf());
    }

    #[test]
    fn chart_node() {
        let n = CSongListNode::chart("Demo", PathBuf::from("/d.dtx"), 50, 120.0);
        assert_eq!(n.level, 50);
        assert_eq!(n.bpm, 120.0);
        assert_eq!(n.node_type, NodeType::Chart);
    }

    #[test]
    fn add_child_limit_32() {
        assert_eq!(MAX_CHILDREN, 32);
        let mut n = CSongListNode::folder("p", PathBuf::from("/"));
        for i in 0..MAX_CHILDREN {
            let c = CSongListNode::folder(format!("c{i}"), PathBuf::from("/c"));
            assert!(n.add_child(c));
        }
        // 33rd add should fail
        let c = CSongListNode::folder("overflow", PathBuf::from("/"));
        assert!(!n.add_child(c));
    }

    #[test]
    fn walk_returns_self_first() {
        let root = CSongListNode::folder("r", PathBuf::from("/"));
        let walked = root.walk();
        assert_eq!(walked.len(), 1);
        assert_eq!(walked[0].title, "r");
    }

    #[test]
    fn walk_dfs_includes_descendants() {
        let mut root = CSongListNode::folder("root", PathBuf::from("/"));
        let mut child = CSongListNode::folder("child", PathBuf::from("/c"));
        child.add_child(CSongListNode::chart(
            "chart1",
            PathBuf::from("/c1.dtx"),
            1,
            120.0,
        ));
        root.add_child(child);
        let walked = root.walk();
        assert_eq!(walked.len(), 3);
    }

    #[test]
    fn find_chart_by_title() {
        let mut root = CSongListNode::folder("root", PathBuf::from("/"));
        let mut sub = CSongListNode::folder("sub", PathBuf::from("/sub"));
        sub.add_child(CSongListNode::chart(
            "Target",
            PathBuf::from("/sub/t.dtx"),
            50,
            130.0,
        ));
        root.add_child(sub);
        let c = root.find_chart("Target").expect("found");
        assert_eq!(c.level, 50);
        assert!(root.find_chart("Missing").is_none());
    }

    #[test]
    fn all_charts() {
        let mut root = CSongListNode::folder("r", PathBuf::from("/"));
        root.add_child(CSongListNode::chart("a", PathBuf::from("/a"), 1, 120.0));
        root.add_child(CSongListNode::chart("b", PathBuf::from("/b"), 2, 130.0));
        let mut sub = CSongListNode::folder("s", PathBuf::from("/s"));
        sub.add_child(CSongListNode::chart("c", PathBuf::from("/c"), 3, 140.0));
        root.add_child(sub);
        let charts = root.all_charts();
        assert_eq!(charts.len(), 3);
    }

    #[test]
    fn child_count() {
        let mut n = CSongListNode::folder("r", PathBuf::from("/"));
        n.add_child(CSongListNode::chart("a", PathBuf::from("/a"), 1, 120.0));
        assert_eq!(n.child_count(), 1);
    }

    #[test]
    fn is_leaf() {
        let mut n = CSongListNode::folder("r", PathBuf::from("/"));
        assert!(n.is_leaf());
        n.add_child(CSongListNode::chart("c", PathBuf::from("/c"), 1, 120.0));
        assert!(!n.is_leaf());
    }

    #[test]
    fn node_type_default_is_folder() {
        let n = CSongListNode::default();
        assert_eq!(n.node_type, NodeType::Folder);
    }
}
