//! Customize-surface tab state + settings draft lifecycle.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::{CustomizeTab, PendingCustomizeTab};

pub(super) fn plugin(app: &mut App) {
    app.add_message::<ConfigDraftAction>()
        .init_resource::<ActiveTab>()
        .init_resource::<ConfigDraft>()
        .init_resource::<SavedConfigDraft>()
        .add_systems(
            Update,
            handle_config_draft_actions
                .run_if(super::editor_open)
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(
            Update,
            (
                apply_accessibility_policy
                    .run_if(super::editor_open)
                    .run_if(resource_changed::<ConfigDraft>),
                sync_active_tab_on_open,
                apply_draft_live
                    .run_if(super::editor_open)
                    .run_if(resource_changed::<ConfigDraft>),
            )
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

/// Last loaded or successfully saved configuration snapshot.
#[derive(Resource, Default, Debug, Clone)]
pub struct SavedConfigDraft(pub dtx_config::Config);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub enum ConfigDraftAction {
    Save,
    Discard,
}

pub fn config_draft_is_dirty(saved: &SavedConfigDraft, draft: &ConfigDraft) -> bool {
    saved.0 != draft.0
}

pub fn discard_config_draft(
    saved: &SavedConfigDraft,
    draft: &mut ConfigDraft,
    policy: &mut dtx_ui::AccessibilityPolicy,
) {
    draft.0 = saved.0.clone();
    *policy = dtx_ui::AccessibilityPolicy::from(&draft.0.accessibility);
}

pub fn save_config_draft_to(
    path: &std::path::Path,
    saved: &mut SavedConfigDraft,
    draft: &ConfigDraft,
) -> Result<(), dtx_config::ConfigError> {
    dtx_config::save(path, &draft.0)?;
    saved.0 = draft.0.clone();
    Ok(())
}

/// On the frame the surface opens, load the config draft and adopt the pending
/// tab (defaulting to Widgets when none was requested).
fn sync_active_tab_on_open(
    open: Res<super::EditorOpen>,
    mut pending: ResMut<PendingCustomizeTab>,
    mut active: ResMut<ActiveTab>,
    mut draft: ResMut<ConfigDraft>,
    mut saved: ResMut<SavedConfigDraft>,
) {
    if !open.is_changed() || !open.0 {
        return;
    }
    draft.0 = dtx_config::load(&dtx_config::default_path());
    saved.0 = draft.0.clone();
    if let Some(tab) = pending.0.take() {
        active.0 = tab;
    } else {
        active.0 = CustomizeTab::Widgets;
    }
}

fn handle_config_draft_actions(
    mut actions: MessageReader<ConfigDraftAction>,
    mut draft: ResMut<ConfigDraft>,
    mut saved: ResMut<SavedConfigDraft>,
    mut policy: ResMut<dtx_ui::AccessibilityPolicy>,
    time: Res<Time>,
    mut err: ResMut<super::footer::EditorSaveError>,
    mut notifications: Option<ResMut<dtx_ui::NotificationQueue>>,
) {
    for action in actions.read() {
        match action {
            ConfigDraftAction::Save => {
                let path = dtx_config::default_path();
                if let Err(error) = save_config_draft_to(&path, &mut saved, &draft) {
                    error!(
                        "customize: failed to save config {}: {error}",
                        path.display()
                    );
                    err.set(time.elapsed_secs_f64(), format!("save failed: {error}"));
                    if let Some(notifications) = &mut notifications {
                        notifications.push(dtx_ui::Notification::error(format!(
                            "Settings were not saved: {error}"
                        )));
                    }
                }
            }
            ConfigDraftAction::Discard => {
                // The live-apply system observes this draft change on the same
                // update; set the policy immediately so text/motion preview
                // never lingers for a frame.
                discard_config_draft(&saved, &mut draft, &mut policy);
            }
        }
    }
}

/// Draft edits with a live runtime resource apply immediately while open.
fn apply_draft_live(
    draft: Res<ConfigDraft>,
    audio: Res<Audio>,
    chart: Res<crate::resources::ActiveChart>,
    mut scroll: ResMut<crate::resources::ScrollSettings>,
    mut playback_rate: ResMut<crate::resources::EffectivePlaybackRate>,
    mut input_offset: ResMut<crate::resources::InputOffsetMs>,
    mut bgm_adjust: ResMut<crate::resources::BgmAdjustState>,
    mut audio_settings: ResMut<crate::resources::DrumAudioSettings>,
    mut gauge: ResMut<crate::gauge::StageGauge>,
    mut toggles: (
        ResMut<crate::resources::ShowPerfInfo>,
        ResMut<crate::resources::MetronomeEnabled>,
        ResMut<crate::resources::ShowTimingLines>,
        ResMut<crate::resources::NoFailEnabled>,
    ),
    mut drum_cfg: ResMut<crate::resources::DrumGameplaySettings>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
    mut bga_settings: ResMut<dtx_bga::BgaSettings>,
) {
    let next_bga = dtx_bga::BgaSettings::from(&draft.0.system);
    if *bga_settings != next_bga {
        *bga_settings = next_bga;
    }
    let g = &draft.0.gameplay;
    *scroll = crate::resources::ScrollSettings::from_scroll_speed(g.scroll_speed);
    let next_rate = crate::resources::EffectivePlaybackRate::normal(f64::from(
        dtx_config::play_speed_multiplier(g.play_speed),
    ));
    if *playback_rate != next_rate {
        crate::playback_rate::apply_playback_rate(
            next_rate,
            &mut playback_rate,
            &audio,
            &bgm,
            &mut instances,
        );
    }
    input_offset.0 = g.input_offset_ms;
    bgm_adjust.common_ms = g.bgm_adjust_ms;
    gauge.damage_level = crate::map_damage_level(g.damage_level);
    toggles.2 .0 = g.lane_display.shows_timing_lines();
    toggles.3 .0 = g.fail_mode() == dtx_config::FailMode::NoFail;
    toggles.0 .0 = draft.0.system.show_perf_info;
    toggles.1 .0 = draft.0.system.metronome;

    if drum_cfg.config != draft.0.drums {
        drum_cfg.config = draft.0.drums.clone();
        drum_cfg.rebuild_from_chart(&chart.chart);
    }
    polyphony.set_voices(draft.0.drums.polyphonic_sounds);

    if let Ok(mut window) = windows.single_mut() {
        let want = if draft.0.system.vsync {
            bevy::window::PresentMode::AutoVsync
        } else {
            bevy::window::PresentMode::AutoNoVsync
        };
        if window.present_mode != want {
            window.present_mode = want;
        }
    }

    let next_audio = crate::resources::DrumAudioSettings {
        bgm_enabled: draft.0.audio.bgm_enabled,
        drum_enabled: draft.0.audio.drum_sound_enabled,
        master_volume: draft.0.audio.master_volume,
        bgm_volume: draft.0.audio.bgm_volume,
        drum_volume: draft.0.audio.drum_volume,
    };
    if *audio_settings != next_audio {
        *audio_settings = next_audio;
        if audio_settings.bgm_enabled {
            dtx_audio::set_bgm_volume(&bgm, &mut instances, audio_settings.bgm_gain());
        } else {
            dtx_audio::stop_bgm(&audio, &mut bgm, &mut instances);
        }
    }
}

fn apply_accessibility_policy(
    draft: Res<ConfigDraft>,
    mut policy: ResMut<dtx_ui::AccessibilityPolicy>,
) {
    let next = dtx_ui::AccessibilityPolicy::from(&draft.0.accessibility);
    if *policy != next {
        *policy = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discard_restores_saved_config_and_policy() {
        let saved = SavedConfigDraft(dtx_config::Config::default());
        let mut draft = ConfigDraft(saved.0.clone());
        draft.0.accessibility.text_scale = dtx_config::TextScale::XLarge;
        let mut policy = dtx_ui::AccessibilityPolicy::from(&draft.0.accessibility);

        discard_config_draft(&saved, &mut draft, &mut policy);

        assert_eq!(draft.0, saved.0);
        assert_eq!(
            policy,
            dtx_ui::AccessibilityPolicy::from(&saved.0.accessibility)
        );
    }

    #[test]
    fn saved_snapshot_identifies_dirty_draft() {
        let saved = SavedConfigDraft(dtx_config::Config::default());
        let mut draft = ConfigDraft(saved.0.clone());
        assert!(!config_draft_is_dirty(&saved, &draft));
        draft.0.accessibility.reduce_motion = true;
        assert!(config_draft_is_dirty(&saved, &draft));
    }

    #[test]
    fn successful_save_updates_snapshot() {
        let tmp = std::env::temp_dir().join("dtxmaniars_config_draft_save_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let path = tmp.join("config.toml");
        let mut saved = SavedConfigDraft(dtx_config::Config::default());
        let mut draft = ConfigDraft(saved.0.clone());
        draft.0.accessibility.reduce_flashes = true;

        save_config_draft_to(&path, &mut saved, &draft).unwrap();

        assert_eq!(saved.0, draft.0);
        assert_eq!(dtx_config::load(&path), draft.0);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn active_tab_defaults_to_widgets() {
        assert_eq!(ActiveTab::default().0, CustomizeTab::Widgets);
    }

    #[test]
    fn config_draft_defaults_to_config_default() {
        assert_eq!(ConfigDraft::default().0, dtx_config::Config::default());
    }
}
