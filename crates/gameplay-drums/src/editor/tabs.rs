//! Customize-surface tab state + settings draft lifecycle.

use bevy::prelude::*;
use game_shell::{CustomizeTab, PendingCustomizeTab};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ActiveTab>()
        .init_resource::<ConfigDraft>()
        .add_systems(
            Update,
            (sync_active_tab_on_open, save_draft_on_close)
                .run_if(in_state(game_shell::AppState::Performance)),
        );
}

/// Which Customize tab is currently shown. Defaults to Widgets (F2 landing).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ActiveTab(pub CustomizeTab);

impl Default for ActiveTab {
    fn default() -> Self {
        Self(CustomizeTab::Widgets)
    }
}

/// In-memory editable copy of `config.toml`, loaded when the surface opens,
/// saved when it closes. Same persistence contract as the old config screen.
#[derive(Resource, Default, Debug, Clone)]
pub struct ConfigDraft(pub dtx_config::Config);

/// On the frame the surface opens, load the config draft and adopt the pending
/// tab (defaulting to Widgets when none was requested).
fn sync_active_tab_on_open(
    open: Res<super::EditorOpen>,
    mut pending: ResMut<PendingCustomizeTab>,
    mut active: ResMut<ActiveTab>,
    mut draft: ResMut<ConfigDraft>,
) {
    if !open.is_changed() || !open.0 {
        return;
    }
    draft.0 = dtx_config::load(&dtx_config::default_path());
    if let Some(tab) = pending.0.take() {
        active.0 = tab;
    } else {
        active.0 = CustomizeTab::Widgets;
    }
}

/// When the surface closes, persist the draft (settings tabs auto-save on exit).
fn save_draft_on_close(open: Res<super::EditorOpen>, draft: Res<ConfigDraft>) {
    if !open.is_changed() || open.0 {
        return;
    }
    let path = dtx_config::default_path();
    if let Err(e) = dtx_config::save(&path, &draft.0) {
        error!("customize: failed to save config {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_tab_defaults_to_widgets() {
        assert_eq!(ActiveTab::default().0, CustomizeTab::Widgets);
    }

    #[test]
    fn config_draft_defaults_to_config_default() {
        assert_eq!(ConfigDraft::default().0, dtx_config::Config::default());
    }
}
