//! Customize-surface tab state + settings draft lifecycle.

use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use game_shell::{CustomizeTab, PendingCustomizeTab};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ActiveTab>()
        .init_resource::<ConfigDraft>()
        .add_systems(
            Update,
            (
                sync_active_tab_on_open,
                save_draft_on_close,
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

/// Draft edits with a live runtime resource apply immediately while open.
fn apply_draft_live(
    draft: Res<ConfigDraft>,
    audio: Res<Audio>,
    chart: Res<crate::resources::ActiveChart>,
    mut scroll: ResMut<crate::resources::ScrollSettings>,
    mut input_offset: ResMut<crate::resources::InputOffsetMs>,
    mut bgm_adjust: ResMut<crate::resources::BgmAdjustState>,
    mut audio_settings: ResMut<crate::resources::DrumAudioSettings>,
    mut gauge: ResMut<crate::gauge::StageGauge>,
    mut show_perf_info: ResMut<crate::resources::ShowPerfInfo>,
    mut metronome_on: ResMut<crate::resources::MetronomeEnabled>,
    mut show_timing_lines: ResMut<crate::resources::ShowTimingLines>,
    mut drum_cfg: ResMut<crate::resources::DrumGameplaySettings>,
    mut polyphony: ResMut<dtx_audio::DrumPolyphony>,
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut bgm: ResMut<dtx_audio::BgmHandle>,
    mut instances: ResMut<Assets<AudioInstance>>,
) {
    let g = &draft.0.gameplay;
    *scroll = crate::resources::ScrollSettings::from_scroll_speed(g.scroll_speed);
    scroll.play_speed = dtx_config::play_speed_multiplier(g.play_speed);
    input_offset.0 = g.input_offset_ms;
    bgm_adjust.common_ms = g.bgm_adjust_ms;
    gauge.damage_level = crate::map_damage_level(g.damage_level);
    show_timing_lines.0 = g.lane_display.shows_timing_lines();
    show_perf_info.0 = draft.0.system.show_perf_info;
    metronome_on.0 = draft.0.system.metronome;

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
