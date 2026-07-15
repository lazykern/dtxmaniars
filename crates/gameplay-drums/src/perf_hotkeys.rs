//! In-song performance hotkeys — port of `CStagePerfCommonScreen.tHandleKeyInput`.
//!
//! Hardcoded keys (not key-assign table):
//! - ↑/↓ scroll speed (drums)
//! - ←/→ input timing adjust
//! - Shift+↑/↓ BGM auto-chip timing adjust (per-song `.score.ini`)
//! - F11 toggle performance debug overlay (NX Guitar Help / `bShowPerformanceInformation`)
//!
//! Reference: `references/DTXmaniaNX/DTXMania/Stage/06.Performance/CStagePerfCommonScreen.cs:2266-2437`

use std::time::{Duration, Instant};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_kira_audio::prelude::AudioInstance;
use dtx_config::{default_path, load, save, Config};
use game_shell::{AppState, EGameMode, PauseState};

use crate::resources::{BgmAdjustState, InputOffsetMs, ScrollSettings, ShowPerfInfo};

/// NX drum scroll index step ≡ multiplier step 0.5 (`CStagePerfDrumsScreen.cs:634-640`).
pub const SCROLL_SPEED_STEP: f32 = 0.5;
pub const SCROLL_SPEED_MIN: f32 = 0.5;
pub const SCROLL_SPEED_MAX: f32 = 9.0;

/// NX in-song input adjust (`ChangeInputAdjustTimeInPlaying`, ±99 clamp).
pub const INPUT_OFFSET_STEP_MS: i32 = 10;
pub const INPUT_OFFSET_FINE_STEP_MS: i32 = 1;
pub const INPUT_OFFSET_CLAMP_MS: i32 = 99;

/// NX BGM adjust (`t各自動再生音チップの再生時刻を変更する`, ±99 clamp).
pub const BGM_ADJUST_STEP_MS: i32 = 10;
pub const BGM_ADJUST_FINE_STEP_MS: i32 = 1;
pub const BGM_ADJUST_CLAMP_MS: i32 = 99;

const PERSIST_DEBOUNCE: Duration = Duration::from_millis(200);

/// Draft config mutated in-song; flushed to disk with debounce.
#[derive(Resource)]
pub struct PerfHotkeyDraft {
    pub cfg: Config,
    dirty: bool,
    last_change: Option<Instant>,
    song_bgm_dirty: bool,
}

impl Default for PerfHotkeyDraft {
    fn default() -> Self {
        Self {
            cfg: load(&default_path()),
            dirty: false,
            last_change: None,
            song_bgm_dirty: false,
        }
    }
}

impl PerfHotkeyDraft {
    pub fn reload(&mut self) {
        self.replace_config(load(&default_path()));
    }

    fn replace_config(&mut self, cfg: Config) {
        self.cfg = cfg;
        self.dirty = false;
        self.last_change = None;
        self.song_bgm_dirty = false;
    }

    pub(crate) fn sync_from_editor(&mut self, cfg: &Config, show_perf_info: bool) {
        self.replace_config(cfg.clone());
        self.cfg.system.show_perf_info = show_perf_info;
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_change = Some(Instant::now());
    }

    fn mark_song_bgm_dirty(&mut self) {
        self.song_bgm_dirty = true;
        self.last_change = Some(Instant::now());
    }
}

pub fn adjust_scroll_speed(current: f32, direction: i32) -> f32 {
    let next = current + SCROLL_SPEED_STEP * direction as f32;
    next.clamp(SCROLL_SPEED_MIN, SCROLL_SPEED_MAX)
}

pub fn adjust_input_offset_ms(current: i32, direction: i32, fine: bool) -> i32 {
    let step = if fine {
        INPUT_OFFSET_FINE_STEP_MS
    } else {
        INPUT_OFFSET_STEP_MS
    };
    (current + step * direction).clamp(-INPUT_OFFSET_CLAMP_MS, INPUT_OFFSET_CLAMP_MS)
}

pub fn adjust_bgm_offset_ms(current: i32, direction: i32, fine: bool) -> i32 {
    let step = if fine {
        BGM_ADJUST_FINE_STEP_MS
    } else {
        BGM_ADJUST_STEP_MS
    };
    (current + step * direction).clamp(-BGM_ADJUST_CLAMP_MS, BGM_ADJUST_CLAMP_MS)
}

pub fn adjust_quick_setting_config(
    cfg: &mut Config,
    setting: crate::pause::QuickSettingKind,
    direction: i32,
) {
    match setting {
        crate::pause::QuickSettingKind::ScrollSpeed => {
            cfg.gameplay.scroll_speed = adjust_scroll_speed(cfg.gameplay.scroll_speed, direction);
        }
        crate::pause::QuickSettingKind::LaneVisibility => {
            let values = dtx_config::LaneDisplay::all();
            let current = values
                .iter()
                .position(|value| *value == cfg.gameplay.lane_display)
                .unwrap_or_default() as i32;
            cfg.gameplay.lane_display =
                values[(current + direction).rem_euclid(values.len() as i32) as usize];
        }
        crate::pause::QuickSettingKind::BgmVolume => {
            let steps = (cfg.audio.bgm_volume * 20.0).round() as i32 + direction;
            cfg.audio.bgm_volume = steps.clamp(0, 20) as f32 / 20.0;
        }
        crate::pause::QuickSettingKind::InputOffset => {
            cfg.gameplay.input_offset_ms =
                adjust_input_offset_ms(cfg.gameplay.input_offset_ms, direction, false);
        }
        crate::pause::QuickSettingKind::Back => {}
    }
}

pub(crate) fn quick_setting_value(cfg: &Config, setting: crate::pause::QuickSettingKind) -> String {
    match setting {
        crate::pause::QuickSettingKind::ScrollSpeed => {
            format!("Scroll Speed  {:.1}×", cfg.gameplay.scroll_speed)
        }
        crate::pause::QuickSettingKind::LaneVisibility => {
            let value = match cfg.gameplay.lane_display {
                dtx_config::LaneDisplay::AllOn => "All On",
                dtx_config::LaneDisplay::Half => "Half",
                dtx_config::LaneDisplay::LineOff => "Lines Off",
                dtx_config::LaneDisplay::AllOff => "All Off",
            };
            format!("Lane Visibility  {value}")
        }
        crate::pause::QuickSettingKind::BgmVolume => {
            format!(
                "BGM Volume  {}%",
                (cfg.audio.bgm_volume * 100.0).round() as i32
            )
        }
        crate::pause::QuickSettingKind::InputOffset => {
            format!("Input Offset  {:+} ms", cfg.gameplay.input_offset_ms)
        }
        crate::pause::QuickSettingKind::Back => "Back".to_owned(),
    }
}

#[derive(SystemParam)]
pub(crate) struct PauseQuickSettings<'w> {
    draft: ResMut<'w, PerfHotkeyDraft>,
    scroll: ResMut<'w, ScrollSettings>,
    input_offset: ResMut<'w, InputOffsetMs>,
    bgm_adjust: ResMut<'w, BgmAdjustState>,
    show_timing_lines: ResMut<'w, crate::resources::ShowTimingLines>,
    lane_display: ResMut<'w, crate::resources::LaneDisplayState>,
    audio_settings: ResMut<'w, crate::resources::DrumAudioSettings>,
    bgm: Res<'w, dtx_audio::BgmHandle>,
    instances: ResMut<'w, Assets<AudioInstance>>,
}

impl PauseQuickSettings<'_> {
    pub fn adjust(&mut self, setting: crate::pause::QuickSettingKind, direction: i32) {
        adjust_quick_setting_config(&mut self.draft.cfg, setting, direction);
        self.draft.mark_dirty();
        apply_runtime_from_draft(
            &self.draft,
            &mut self.scroll,
            &mut self.input_offset,
            &mut self.bgm_adjust,
        );
        self.show_timing_lines.0 = self.draft.cfg.gameplay.lane_display.shows_timing_lines();
        self.lane_display.0 = self.draft.cfg.gameplay.lane_display;
        self.audio_settings.bgm_volume = self.draft.cfg.audio.bgm_volume;
        if self.audio_settings.bgm_enabled {
            dtx_audio::set_bgm_volume(
                &self.bgm,
                &mut self.instances,
                self.audio_settings.bgm_gain(),
            );
        }
    }

    pub fn value(&self, setting: crate::pause::QuickSettingKind) -> String {
        quick_setting_value(&self.draft.cfg, setting)
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PerfHotkeyDraft>()
        .add_systems(OnEnter(AppState::Performance), init_perf_hotkey_draft)
        .add_systems(OnExit(AppState::Performance), flush_perf_hotkey_persist)
        .add_systems(
            Update,
            toggle_perf_info
                .run_if(in_state(AppState::Performance))
                .run_if(drums_mode_active),
        )
        .add_systems(Update, reload_draft_on_editor_close)
        .add_systems(
            Update,
            (handle_perf_hotkeys, debounced_persist_perf_hotkeys)
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Running))
                .run_if(drums_mode_active)
                .run_if(performance_hotkeys_active)
                // Editor arrow-nudge shares these keys; don't let it mutate/persist
                // scroll speed + offsets while the layout editor is open.
                .run_if(crate::editor::editor_closed),
        );
}

fn drums_mode_active(mode: Res<EGameMode>) -> bool {
    *mode == EGameMode::Drums
}

fn performance_hotkeys_active(flow: Option<Res<crate::practice::PracticeFlow>>) -> bool {
    performance_hotkeys_active_value(flow.as_deref())
}

fn performance_hotkeys_active_value(flow: Option<&crate::practice::PracticeFlow>) -> bool {
    flow.is_none_or(|flow| flow.phase == crate::practice::PracticePhase::Running)
}

fn init_perf_hotkey_draft(mut draft: ResMut<PerfHotkeyDraft>, show: Res<ShowPerfInfo>) {
    draft.reload();
    draft.cfg.system.show_perf_info = show.0;
}

/// The Customize surface saves its own config draft on close; the perf-hotkey
/// draft (loaded at song start) is stale then — the next arrow-key press would
/// persist stale config over the Customize edits. Reload it when the surface
/// closes.
fn reload_draft_on_editor_close(
    open: Res<crate::editor::EditorOpen>,
    config_draft: Res<crate::editor::tabs::ConfigDraft>,
    mut draft: ResMut<PerfHotkeyDraft>,
    show: Res<ShowPerfInfo>,
    state: Res<State<AppState>>,
    mut was_open: Local<bool>,
) {
    if crate::editor::should_persist_close(
        open.0,
        *state.get() == AppState::Performance,
        &mut was_open,
    ) {
        draft.sync_from_editor(&config_draft.0, show.0);
    }
}

fn toggle_perf_info(
    keys: Res<ButtonInput<KeyCode>>,
    mut show: ResMut<ShowPerfInfo>,
    mut draft: ResMut<PerfHotkeyDraft>,
) {
    if keys.just_pressed(KeyCode::F11) {
        show.0 = !show.0;
        draft.cfg.system.show_perf_info = show.0;
        draft.mark_dirty();
    }
}

fn apply_runtime_from_draft(
    draft: &PerfHotkeyDraft,
    scroll: &mut ScrollSettings,
    input_offset: &mut InputOffsetMs,
    bgm_adjust: &mut BgmAdjustState,
) {
    *scroll = ScrollSettings::from_scroll_speed(draft.cfg.gameplay.scroll_speed);
    input_offset.0 = draft.cfg.gameplay.input_offset_ms;
    bgm_adjust.common_ms = draft.cfg.gameplay.bgm_adjust_ms;
}

fn handle_perf_hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    mut draft: ResMut<PerfHotkeyDraft>,
    mut scroll: ResMut<ScrollSettings>,
    mut input_offset: ResMut<InputOffsetMs>,
    mut bgm_adjust: ResMut<BgmAdjustState>,
    chart: Res<crate::resources::ActiveChart>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let mut changed = false;

    if shift && keys.just_pressed(KeyCode::ArrowUp) {
        bgm_adjust.song_ms = adjust_bgm_offset_ms(bgm_adjust.song_ms, 1, ctrl);
        draft.mark_song_bgm_dirty();
        changed = true;
    } else if shift && keys.just_pressed(KeyCode::ArrowDown) {
        bgm_adjust.song_ms = adjust_bgm_offset_ms(bgm_adjust.song_ms, -1, ctrl);
        draft.mark_song_bgm_dirty();
        changed = true;
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        draft.cfg.gameplay.scroll_speed = adjust_scroll_speed(draft.cfg.gameplay.scroll_speed, 1);
        draft.mark_dirty();
        changed = true;
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        draft.cfg.gameplay.scroll_speed = adjust_scroll_speed(draft.cfg.gameplay.scroll_speed, -1);
        draft.mark_dirty();
        changed = true;
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        draft.cfg.gameplay.input_offset_ms =
            adjust_input_offset_ms(draft.cfg.gameplay.input_offset_ms, -1, ctrl);
        draft.mark_dirty();
        changed = true;
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        draft.cfg.gameplay.input_offset_ms =
            adjust_input_offset_ms(draft.cfg.gameplay.input_offset_ms, 1, ctrl);
        draft.mark_dirty();
        changed = true;
    }

    if changed {
        apply_runtime_from_draft(&draft, &mut scroll, &mut input_offset, &mut bgm_adjust);
        if draft.song_bgm_dirty {
            if let Some(path) = chart.source_path.as_ref() {
                let ini = dtx_scoring::score_ini::score_ini_path(path);
                if let Err(e) = dtx_scoring::score_ini::write_bgm_adjust(&ini, bgm_adjust.song_ms) {
                    warn!("perf hotkeys: failed to persist BGMAdjust to {ini:?}: {e}");
                } else {
                    draft.song_bgm_dirty = false;
                }
            }
        }
    }
}

fn debounced_persist_perf_hotkeys(mut draft: ResMut<PerfHotkeyDraft>) {
    if !draft.dirty {
        return;
    }
    let Some(last) = draft.last_change else {
        return;
    };
    if last.elapsed() < PERSIST_DEBOUNCE {
        return;
    }
    flush_config_draft(&mut draft);
}

fn flush_perf_hotkey_persist(mut draft: ResMut<PerfHotkeyDraft>) {
    if draft.dirty {
        flush_config_draft(&mut draft);
    }
    if draft.song_bgm_dirty {
        // Best-effort: song BGMAdjust already written on key; flag cleared there.
        draft.song_bgm_dirty = false;
    }
}

fn flush_config_draft(draft: &mut PerfHotkeyDraft) {
    let path = default_path();
    match save(&path, &draft.cfg) {
        Ok(()) => {
            draft.dirty = false;
        }
        Err(e) => {
            warn!("perf hotkeys: failed to persist config to {path:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::{PracticeFlow, PracticePhase};

    #[test]
    fn performance_hotkeys_yield_to_practice_setup_and_editing() {
        assert!(!performance_hotkeys_active_value(Some(
            &PracticeFlow::default()
        )));

        let mut editing = PracticeFlow::default();
        editing.phase = PracticePhase::Editing;
        assert!(!performance_hotkeys_active_value(Some(&editing)));

        assert!(performance_hotkeys_active_value(Some(
            &PracticeFlow::running()
        )));
        assert!(performance_hotkeys_active_value(None));
    }

    #[test]
    fn forced_editor_close_sync_preserves_live_perf_info() {
        let mut editor_cfg = Config::default();
        editor_cfg.gameplay.scroll_speed = 7.5;

        let mut draft = PerfHotkeyDraft {
            cfg: Config::default(),
            dirty: true,
            last_change: Some(Instant::now()),
            song_bgm_dirty: true,
        };
        draft.sync_from_editor(&editor_cfg, true);

        assert_eq!(draft.cfg.gameplay.scroll_speed, 7.5);
        assert!(draft.cfg.system.show_perf_info);
        assert!(!draft.dirty);
        assert!(draft.last_change.is_none());
        assert!(!draft.song_bgm_dirty);
    }

    #[test]
    fn editor_close_syncs_perf_draft_from_memory() {
        let mut editor_cfg = Config::default();
        editor_cfg.gameplay.scroll_speed = 7.5;

        let mut app = App::new();
        app.insert_resource(State::new(AppState::Performance))
            .insert_resource(crate::editor::EditorOpen(false))
            .insert_resource(crate::editor::tabs::ConfigDraft(editor_cfg))
            .insert_resource(PerfHotkeyDraft {
                cfg: Config::default(),
                dirty: false,
                last_change: None,
                song_bgm_dirty: false,
            })
            .insert_resource(ShowPerfInfo(false))
            .add_systems(Update, reload_draft_on_editor_close);

        app.update();
        app.world_mut()
            .resource_mut::<crate::editor::EditorOpen>()
            .0 = true;
        app.update();
        app.world_mut()
            .resource_mut::<crate::editor::EditorOpen>()
            .0 = false;
        app.update();

        assert_eq!(
            app.world()
                .resource::<PerfHotkeyDraft>()
                .cfg
                .gameplay
                .scroll_speed,
            7.5
        );
    }

    #[test]
    fn scroll_speed_steps_and_clamps() {
        assert!((adjust_scroll_speed(1.0, 1) - 1.5).abs() < f32::EPSILON);
        assert!((adjust_scroll_speed(0.5, -1) - 0.5).abs() < f32::EPSILON);
        assert!((adjust_scroll_speed(9.0, 1) - 9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn input_offset_steps_and_clamps() {
        assert_eq!(adjust_input_offset_ms(0, 1, false), 10);
        assert_eq!(adjust_input_offset_ms(0, 1, true), 1);
        assert_eq!(adjust_input_offset_ms(95, 1, false), 99);
        assert_eq!(adjust_input_offset_ms(-95, -1, false), -99);
    }

    #[test]
    fn bgm_adjust_steps_and_clamps() {
        assert_eq!(adjust_bgm_offset_ms(0, 1, false), 10);
        assert_eq!(adjust_bgm_offset_ms(0, 1, true), 1);
        assert_eq!(adjust_bgm_offset_ms(95, 1, false), 99);
    }

    #[test]
    fn pause_quick_settings_adjust_only_the_compact_contract() {
        let mut cfg = Config::default();
        cfg.gameplay.scroll_speed = 1.0;
        cfg.gameplay.lane_display = dtx_config::LaneDisplay::AllOn;
        cfg.audio.bgm_volume = 0.7;
        cfg.gameplay.input_offset_ms = 0;

        adjust_quick_setting_config(&mut cfg, crate::pause::QuickSettingKind::ScrollSpeed, 1);
        adjust_quick_setting_config(&mut cfg, crate::pause::QuickSettingKind::LaneVisibility, 1);
        adjust_quick_setting_config(&mut cfg, crate::pause::QuickSettingKind::BgmVolume, -1);
        adjust_quick_setting_config(&mut cfg, crate::pause::QuickSettingKind::InputOffset, 1);

        assert_eq!(cfg.gameplay.scroll_speed, 1.5);
        assert_eq!(cfg.gameplay.lane_display, dtx_config::LaneDisplay::Half);
        assert!((cfg.audio.bgm_volume - 0.65).abs() < f32::EPSILON);
        assert_eq!(cfg.gameplay.input_offset_ms, 10);
    }

    #[test]
    fn quick_settings_use_one_bounded_reducer_in_both_directions() {
        let mut cfg = Config::default();
        for setting in [
            crate::pause::QuickSettingKind::ScrollSpeed,
            crate::pause::QuickSettingKind::LaneVisibility,
            crate::pause::QuickSettingKind::BgmVolume,
            crate::pause::QuickSettingKind::InputOffset,
        ] {
            let before = cfg.clone();
            adjust_quick_setting_config(&mut cfg, setting, 1);
            adjust_quick_setting_config(&mut cfg, setting, -1);
            assert_eq!(cfg, before, "round trip for {setting:?}");
        }
    }
}
