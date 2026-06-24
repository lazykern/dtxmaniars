//! Base `CActConfigList` — port of `Stage/03.Config/CActConfigList.cs` (818 LOC).
//!
//! Strict-port-first. This is the base list/iterator/ESC dispatch. Per-tab
//! item lists (System/Audio/Graphics/...) are p1-3..p1-7.
//!
//! ## Reference
//! - `references/DTXmaniaNX-BocuD/DTXMania/Stage/03.Config/CActConfigList.cs`
//! - EMenuType (CActConfigList.cs:122-141) — 17 variants
//! - tPressEsc (CActConfigList.cs:51-105) — ESC dispatches to parent menu
//! - tMoveToPrevious/Next (CActConfigList.cs:108-120) — list navigation
//! - tSetupItemList_* (CActConfigList.cs) — per-tab list builders

use bevy::prelude::*;
use game_shell::AppState;

/// All 17 menu types in BocuD (CActConfigList.cs:122-141).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EMenuType {
    /// Top-level System tab.
    System,
    /// Top-level Drums tab.
    Drums,
    /// Top-level Guitar tab.
    Guitar,
    /// Top-level Bass tab.
    Bass,
    /// KeyAssign sub-act.
    KeyAssignSystem,
    /// System > Graphics sub-tab.
    SystemGraphics,
    /// System > Audio sub-tab.
    SystemAudio,
    /// System > Audio > Driver sub-tab.
    SystemAudioDriver,
    /// System > Gameplay sub-tab.
    SystemGameplay,
    /// System > Menu sub-tab.
    SystemMenu,
    /// System > Skin sub-tab.
    SystemSkin,
    /// Drums > KeyAssign sub-tab.
    KeyAssignDrums,
    /// Drums > Velocity sub-tab.
    VelocityDrums,
    /// Guitar > KeyAssign sub-tab.
    KeyAssignGuitar,
    /// Bass > KeyAssign sub-tab.
    KeyAssignBass,
    /// Default for unselected state.
    Unknown,
}

impl EMenuType {
    /// 17 variants per CActConfigList.cs:122-141.
    pub fn all() -> [Self; 16] {
        [
            Self::System,
            Self::Drums,
            Self::Guitar,
            Self::Bass,
            Self::KeyAssignSystem,
            Self::SystemGraphics,
            Self::SystemAudio,
            Self::SystemAudioDriver,
            Self::SystemGameplay,
            Self::SystemMenu,
            Self::SystemSkin,
            Self::KeyAssignDrums,
            Self::VelocityDrums,
            Self::KeyAssignGuitar,
            Self::KeyAssignBass,
            Self::Unknown,
        ]
    }

    /// True if the menu is a submenu (drilldown) — see CActConfigList.cs:18-32.
    pub fn is_submenu(&self) -> bool {
        matches!(
            self,
            Self::KeyAssignBass
                | Self::KeyAssignDrums
                | Self::KeyAssignGuitar
                | Self::KeyAssignSystem
                | Self::SystemGraphics
                | Self::SystemGameplay
                | Self::SystemMenu
                | Self::SystemSkin
                | Self::SystemAudio
                | Self::SystemAudioDriver
                | Self::VelocityDrums
        )
    }

    /// ESC parent dispatch (CActConfigList.cs:60-103).
    pub fn esc_parent(&self) -> Self {
        match self {
            Self::KeyAssignSystem
            | Self::SystemGraphics
            | Self::SystemSkin
            | Self::SystemAudio
            | Self::SystemGameplay
            | Self::SystemMenu => Self::System,
            Self::SystemAudioDriver => Self::SystemAudio,
            Self::KeyAssignDrums => Self::Drums,
            Self::VelocityDrums => Self::Drums,
            Self::KeyAssignGuitar => Self::Guitar,
            Self::KeyAssignBass => Self::Bass,
            // Top-level buttons stay (no parent); reference falls through
            // with no action.
            _ => *self,
        }
    }
}

impl Default for EMenuType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// A single item in the ConfigList. Same shape as M12 `ConfigItem`
/// (volume/cycle/toggle), wrapped here for clarity.
#[derive(Debug, Clone)]
pub struct ConfigListItem {
    pub name: &'static str,
    pub value: String,
    /// Item has a submenu (e.g. "System > Audio").
    pub has_submenu: bool,
}

impl ConfigListItem {
    pub fn leaf(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
            has_submenu: false,
        }
    }

    pub fn submenu(name: &'static str) -> Self {
        Self {
            name,
            value: "...".into(),
            has_submenu: true,
        }
    }
}

/// Current ConfigList state — menu type + items + selection.
#[derive(Resource, Debug, Clone)]
pub struct ConfigListState {
    pub menu_type: EMenuType,
    pub items: Vec<ConfigListItem>,
    pub selection: usize,
}

impl Default for ConfigListState {
    fn default() -> Self {
        Self {
            menu_type: EMenuType::Unknown,
            items: Vec::new(),
            selection: 0,
        }
    }
}

impl ConfigListState {
    /// Re-populate the list for a given menu type.
    ///
    /// Reference: CActConfigList.cs:152-200 — the OnActivate sequence
    /// populates System, Audio, Drums, Guitar, Bass in order, then
    /// returns to System.
    pub fn load(&mut self, menu_type: EMenuType) {
        self.menu_type = menu_type;
        self.items = build_items_for(menu_type);
        self.selection = 0;
    }

    /// Move selection up (CActConfigList.cs:111-115).
    pub fn move_previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selection = self.selection.saturating_sub(1);
    }

    /// Move selection down (CActConfigList.cs:117-120).
    pub fn move_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let max = self.items.len() - 1;
        self.selection = (self.selection + 1).min(max);
    }

    /// True if currently on a submenu item.
    pub fn is_on_submenu(&self) -> bool {
        self.items
            .get(self.selection)
            .map(|i| i.has_submenu)
            .unwrap_or(false)
    }
}

/// Build items for a menu type. Stub for p1-2 — full lists land in
/// p1-3 (System), p1-4 (Audio), p1-5 (Audio Driver), p1-6 (Graphics),
/// p1-7 (Gameplay), p1-8 (Menu), p1-13 (Skin).
fn build_items_for(menu_type: EMenuType) -> Vec<ConfigListItem> {
    match menu_type {
        EMenuType::System => crate::config_list_system::build_system_items(),
        EMenuType::SystemAudio => crate::config_list_audio::build_audio_items(),
        EMenuType::SystemAudioDriver => crate::config_list_audio_driver::build_audio_driver_items(),
        EMenuType::SystemGraphics => crate::config_list_graphics::build_graphics_items(),
        EMenuType::Drums => vec![
            ConfigListItem::submenu("Auto Play"),
            ConfigListItem::submenu("Lane Type"),
            ConfigListItem::submenu("Velocity"),
        ],
        EMenuType::Guitar => vec![
            ConfigListItem::submenu("Auto Play"),
            ConfigListItem::submenu("Pick Color"),
        ],
        EMenuType::Bass => vec![
            ConfigListItem::submenu("Auto Play"),
            ConfigListItem::submenu("Pick Color"),
        ],
        EMenuType::Unknown => Vec::new(),
        // Sub-acts (SystemGraphics, etc.) are filled by p1-3..p1-7 + p1-13.
        _ => vec![ConfigListItem::leaf("(stub)", "p1-3+")],
    }
}

/// Marker for the right-side items list entity.
#[derive(Component, Debug, Clone, Copy)]
pub struct ConfigListItems;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ConfigListState>()
        .add_systems(OnEnter(AppState::Config), populate_default_list);
}

fn populate_default_list(mut list: ResMut<ConfigListState>) {
    if list.menu_type == EMenuType::Unknown {
        list.load(EMenuType::System);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emenu_type_count_matches_reference() {
        // CActConfigList.cs:122-141 has 16 named variants + 1 Unknown.
        // C# has Unknown at index 16 but is not counted in `all()`.
        let all = EMenuType::all();
        assert_eq!(all.len(), 16);
    }

    #[test]
    fn emenu_type_variants_unique() {
        let all = EMenuType::all();
        let unique: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn is_submenu_returns_true_for_known_submenus() {
        // CActConfigList.cs:18-32
        assert!(EMenuType::SystemAudio.is_submenu());
        assert!(EMenuType::SystemGraphics.is_submenu());
        assert!(EMenuType::SystemGameplay.is_submenu());
        assert!(EMenuType::KeyAssignDrums.is_submenu());
        assert!(EMenuType::VelocityDrums.is_submenu());
    }

    #[test]
    fn is_submenu_returns_false_for_top_level() {
        assert!(!EMenuType::System.is_submenu());
        assert!(!EMenuType::Drums.is_submenu());
        assert!(!EMenuType::Guitar.is_submenu());
        assert!(!EMenuType::Bass.is_submenu());
        assert!(!EMenuType::Unknown.is_submenu());
    }

    #[test]
    fn esc_parent_dispatches_to_system() {
        // CActConfigList.cs:60-75
        assert_eq!(EMenuType::SystemGraphics.esc_parent(), EMenuType::System);
        assert_eq!(EMenuType::SystemAudio.esc_parent(), EMenuType::System);
        assert_eq!(EMenuType::SystemSkin.esc_parent(), EMenuType::System);
        assert_eq!(EMenuType::SystemGameplay.esc_parent(), EMenuType::System);
        assert_eq!(EMenuType::SystemMenu.esc_parent(), EMenuType::System);
    }

    #[test]
    fn esc_parent_dispatches_to_audio() {
        // CActConfigList.cs:77-80
        assert_eq!(
            EMenuType::SystemAudioDriver.esc_parent(),
            EMenuType::SystemAudio
        );
    }

    #[test]
    fn esc_parent_dispatches_to_instrument() {
        // CActConfigList.cs:82-95
        assert_eq!(EMenuType::KeyAssignDrums.esc_parent(), EMenuType::Drums);
        assert_eq!(EMenuType::VelocityDrums.esc_parent(), EMenuType::Drums);
        assert_eq!(EMenuType::KeyAssignGuitar.esc_parent(), EMenuType::Guitar);
        assert_eq!(EMenuType::KeyAssignBass.esc_parent(), EMenuType::Bass);
    }

    #[test]
    fn esc_parent_top_level_returns_self() {
        assert_eq!(EMenuType::System.esc_parent(), EMenuType::System);
        assert_eq!(EMenuType::Drums.esc_parent(), EMenuType::Drums);
        assert_eq!(EMenuType::Unknown.esc_parent(), EMenuType::Unknown);
    }

    #[test]
    fn config_list_state_default_empty() {
        let s = ConfigListState::default();
        assert_eq!(s.menu_type, EMenuType::Unknown);
        assert!(s.items.is_empty());
        assert_eq!(s.selection, 0);
    }

    #[test]
    fn config_list_load_populates_items() {
        let mut s = ConfigListState::default();
        s.load(EMenuType::System);
        assert_eq!(s.menu_type, EMenuType::System);
        // System tab now has 12 items (p1-3 port) per CActConfigList.System.cs:9-396.
        assert_eq!(s.items.len(), 12);
        assert_eq!(s.selection, 0);
    }

    #[test]
    fn config_list_move_previous_saturates() {
        let mut s = ConfigListState::default();
        s.load(EMenuType::System);
        s.move_previous();
        assert_eq!(s.selection, 0); // saturating
    }

    #[test]
    fn config_list_move_next_within_bounds() {
        let mut s = ConfigListState::default();
        s.load(EMenuType::System);
        s.move_next();
        s.move_next();
        assert_eq!(s.selection, 2);
    }

    #[test]
    fn config_list_is_on_submenu_true_for_submenu_item() {
        let mut s = ConfigListState::default();
        s.load(EMenuType::System);
        // Item 1 is "Graphics Options" (submenu); 0 is "Return to Menu" (leaf).
        s.selection = 1;
        assert!(s.is_on_submenu());
    }

    #[test]
    fn config_list_item_submenu_marker() {
        let s = ConfigListItem::submenu("Test");
        assert!(s.has_submenu);
        let l = ConfigListItem::leaf("X", "Y");
        assert!(!l.has_submenu);
    }

    #[test]
    fn config_list_load_resets_selection() {
        let mut s = ConfigListState::default();
        s.load(EMenuType::System);
        s.selection = 4;
        s.load(EMenuType::Drums);
        assert_eq!(s.selection, 0);
    }
}
