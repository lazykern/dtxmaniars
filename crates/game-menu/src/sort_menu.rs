#![allow(missing_docs)]
//! `SortMenuContainer` + `SortMenuElement` — port of
//! `Stage/04.SongSelectionNew/SortMenuContainer.cs` (205 LOC) + `SortMenuElement.cs` (48 LOC).
//!
//! Strict-port-first.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/SortMenuContainer.cs:1-205`
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/04.SongSelectionNew/SortMenuElement.cs:1-48`

use super::song_select_new_stage::ESortMode;

/// Container size (SortMenuContainer.cs:25).
pub const SORT_MENU_W: f32 = 662.0;
pub const SORT_MENU_H: f32 = 92.0;
/// Element spacing (SortMenuContainer.cs:55).
pub const SORT_ELEMENT_SPACING: f32 = 90.0;
/// Initial selection index (SortMenuContainer.cs:30).
pub const SORT_INITIAL_SELECTION: usize = 2;
/// Animation offset (SortMenuContainer.cs:60).
pub const SORT_OFFSET_RANGE: f32 = 90.0;
pub const SORT_OFFSET_DISTANCE: f32 = 18.0;
/// Element container offset (SortMenuContainer.cs:38).
pub const SORT_ELEMENTS_Y: f32 = 40.0;

/// One sort menu element (one sorter).
#[derive(Debug, Clone)]
pub struct SortMenuElement {
    pub sort_mode: ESortMode,
    /// Element position relative to the container.
    pub x: f32,
}

impl SortMenuElement {
    pub fn new(sort_mode: ESortMode, x: f32) -> Self {
        Self { sort_mode, x }
    }
}

/// State for the sort menu (ring buffer of elements).
#[derive(Debug, Clone)]
pub struct SortMenuContainer {
    pub elements: Vec<SortMenuElement>,
    pub selection_index: usize,
    /// Animated x offset for smooth scroll.
    pub target_x: f32,
}

impl SortMenuContainer {
    /// Build a container with one element per sorter.
    pub fn new() -> Self {
        let sorters = ESortMode::all();
        let elements = sorters
            .iter()
            .enumerate()
            .map(|(i, s)| SortMenuElement::new(*s, i as f32 * SORT_ELEMENT_SPACING))
            .collect();
        Self {
            elements,
            selection_index: SORT_INITIAL_SELECTION,
            target_x: 0.0,
        }
    }

    /// Move selection right (wraps at end).
    pub fn move_next(&mut self) {
        if self.elements.is_empty() {
            return;
        }
        self.selection_index = (self.selection_index + 1) % self.elements.len();
        self.target_x = self.selection_index as f32 * SORT_ELEMENT_SPACING;
    }

    /// Move selection left (wraps at start).
    pub fn move_previous(&mut self) {
        if self.elements.is_empty() {
            return;
        }
        let max = self.elements.len() - 1;
        self.selection_index = if self.selection_index == 0 {
            max
        } else {
            self.selection_index - 1
        };
        self.target_x = self.selection_index as f32 * SORT_ELEMENT_SPACING;
    }

    /// Get the currently selected element.
    pub fn current(&self) -> Option<&SortMenuElement> {
        self.elements.get(self.selection_index)
    }
}

impl Default for SortMenuContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_menu_size_matches_reference() {
        // SortMenuContainer.cs:25
        assert_eq!(SORT_MENU_W, 662.0);
        assert_eq!(SORT_MENU_H, 92.0);
    }

    #[test]
    fn sort_element_spacing_matches_reference() {
        // SortMenuContainer.cs:55
        assert_eq!(SORT_ELEMENT_SPACING, 90.0);
    }

    #[test]
    fn sort_initial_selection_matches_reference() {
        // SortMenuContainer.cs:30 — selectionIndex = 2
        assert_eq!(SORT_INITIAL_SELECTION, 2);
    }

    #[test]
    fn sort_menu_container_has_9_elements() {
        // SortMenuContainer.cs:34-44 — one per sorter
        let c = SortMenuContainer::new();
        assert_eq!(c.elements.len(), 9);
    }

    #[test]
    fn sort_menu_default_selection_is_initial() {
        let c = SortMenuContainer::new();
        assert_eq!(c.selection_index, SORT_INITIAL_SELECTION);
    }

    #[test]
    fn sort_menu_move_next_wraps() {
        let mut c = SortMenuContainer::new();
        // 9 elements, current = 2. After 7 next → wraps to 0.
        for _ in 0..7 {
            c.move_next();
        }
        assert_eq!(c.selection_index, 0);
    }

    #[test]
    fn sort_menu_move_previous_wraps() {
        let mut c = SortMenuContainer::new();
        c.move_previous();
        // 2 → 1
        assert_eq!(c.selection_index, 1);
        c.move_previous();
        // 1 → 0
        assert_eq!(c.selection_index, 0);
        c.move_previous();
        // 0 → 8 (wrap)
        assert_eq!(c.selection_index, 8);
    }

    #[test]
    fn sort_menu_current_returns_some() {
        let c = SortMenuContainer::new();
        assert!(c.current().is_some());
    }
}
