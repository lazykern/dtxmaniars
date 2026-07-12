//! Auto-play bot (Phase F5).
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/CStagePerfDrumsScreen.cs`
//! (auto-play logic).
//!
//! The bot reads `ActiveChart` + `AudioClock` and emits a `LaneHit` event
//! for each un-judged chip whose target time has just passed. The judge
//! system then classifies the press as Perfect (delta=0). Reuses
//! `JudgedChips` from `judge.rs` to dedupe.

use bevy::prelude::*;

use crate::events::LaneHit;
use crate::judge::{chip_target_ms, BarLengthChangeList, BpmChangeList, JudgedChips};
use crate::lane_map::{lane_of, LaneId};
use crate::resources::{ActiveChart, GameplayClock};
use dtx_timing::math::ChartTiming;

/// Resource flag — when true, the autoplay system emits LaneHit for each
/// chip at its target ms. Toggleable from config (M5+).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoplayEnabled(pub bool);

/// Plugin. Adds the `AutoplayEnabled` resource + system.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AutoplayEnabled>().add_systems(
        FixedUpdate,
        autoplay_system
            .in_set(super::DrumsSets::Input)
            .run_if(autoplay_active)
            .run_if(in_state(game_shell::AppState::Performance)),
    );
}

fn autoplay_active(flag: Res<AutoplayEnabled>) -> bool {
    flag.0
}

/// System: for each un-judged chip whose target_ms <= GameplayClock.current_ms,
/// emit a LaneHit event with audio_ms = target_ms (delta = 0 → Perfect).
///
/// In normal play the judge marks the chip judged on the same tick, so each
/// chip fires exactly once. While the Customize surface is open the judge is
/// gated off (`editor_closed`), so nothing would mark the chip until the miss
/// sweep does ~118ms later — the bot would re-emit the same chip every
/// FixedUpdate tick and `hit_feedback` would stack ~8 copies of its voice. To
/// keep one hit per chip, the bot marks the chip judged itself when the
/// editor is open.
pub fn autoplay_system(
    clock: Res<GameplayClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    bar_changes: Res<BarLengthChangeList>,
    editor_open: Res<crate::editor::EditorOpen>,
    mut judged: ResMut<JudgedChips>,
    mut lane_hits: MessageWriter<LaneHit>,
) {
    if !clock.is_ready() {
        return;
    }
    let current_ms = clock.current_ms;

    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);
    let timing = ChartTiming {
        bpm_changes: &bpm_changes.changes,
        bar_changes: &bar_changes.changes,
    };

    for (idx, chip) in chart.chart.chips.iter().enumerate() {
        if judged.0.contains(&idx) {
            continue;
        }
        let Some(lane) = lane_of(chip.channel) else {
            // Not a playable lane (e.g. BGA, BPM) — auto-judge as judged so
            // it doesn't block later logic.
            judged.0.insert(idx);
            continue;
        };
        let target_ms = chip_target_ms(chip, base_bpm, timing);
        if target_ms <= current_ms {
            lane_hits.write(LaneHit {
                lane: lane as LaneId,
                audio_ms: target_ms,
            });
            if editor_open.0 {
                debug!(
                    "autoplay (editor): chip {idx} lane {lane} target {target_ms}ms marked judged"
                );
                judged.0.insert(idx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::channel::EChannel;
    use dtx_core::chart::Chart;

    fn make_chart_with_chips(chips: Vec<dtx_core::Chip>) -> Chart {
        let mut c = Chart::default();
        c.metadata.bpm = Some(120.0);
        c.chips = chips;
        c
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_resource::<ActiveChart>()
            .init_resource::<GameplayClock>()
            .init_resource::<JudgedChips>()
            .init_resource::<BpmChangeList>()
            .init_resource::<BarLengthChangeList>()
            .init_resource::<AutoplayEnabled>()
            .init_resource::<crate::editor::EditorOpen>()
            .add_message::<LaneHit>()
            .add_message::<crate::events::InputHit>()
            .add_systems(Update, autoplay_system.run_if(autoplay_active));
        app
    }

    fn build_pipeline_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_resource::<ActiveChart>()
            .init_resource::<GameplayClock>()
            .init_resource::<JudgedChips>()
            .init_resource::<BpmChangeList>()
            .init_resource::<BarLengthChangeList>()
            .init_resource::<AutoplayEnabled>()
            .init_resource::<crate::editor::EditorOpen>()
            .init_resource::<crate::resources::DrumGameplaySettings>()
            .init_resource::<crate::resources::Score>()
            .init_resource::<crate::resources::DrumScoring>()
            .init_resource::<crate::resources::Combo>()
            .init_resource::<crate::resources::JudgmentCounts>()
            .init_resource::<crate::resources::FastSlowCount>()
            .init_resource::<crate::resources::SkillValue>()
            .init_resource::<crate::derived::ChartDerived>()
            .init_resource::<crate::resources::InputOffsetMs>()
            .init_resource::<crate::components::LastJudgment>()
            .add_message::<LaneHit>()
            .add_message::<crate::events::InputHit>()
            .add_message::<crate::events::JudgmentEvent>()
            .add_message::<crate::events::EmptyHit>()
            .add_systems(
                Update,
                (
                    autoplay_system.run_if(autoplay_active),
                    crate::judge::judge_lane_hit_system,
                    crate::score::update_score_system,
                )
                    .chain(),
            );
        app
    }

    fn set_clock(app: &mut App, ms: i64) {
        let mut clock = app.world_mut().resource_mut::<GameplayClock>();
        clock.start();
        clock.sync(Some(ms));
    }

    fn lane_hit_count(app: &App) -> usize {
        app.world()
            .resource::<Messages<LaneHit>>()
            .iter_current_update_messages()
            .count()
    }

    #[test]
    fn autoplay_disabled_emits_nothing() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = false;
        set_clock(&mut app, 2000);

        app.update();
        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&0),
            "autoplay disabled: chip should NOT be judged"
        );
    }

    #[test]
    fn autoplay_emits_when_audio_passes_target() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        set_clock(&mut app, 100);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&0),
            "autoplay must not pre-mark playable chips judged"
        );
        assert_eq!(lane_hit_count(&app), 1);
    }

    #[test]
    fn autoplay_no_emit_when_audio_before_target() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(1, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        set_clock(&mut app, 500);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&0),
            "chip should NOT be marked judged yet"
        );
    }

    #[test]
    fn autoplay_no_emit_while_audio_required_clock_waits() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        app.world_mut()
            .resource_mut::<GameplayClock>()
            .start_audio_required();

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&0),
            "waiting BGM clock must not autoplay notes at stale 0ms"
        );
    }

    #[test]
    fn autoplay_emits_multiple_chips_in_sequence() {
        let mut app = build_app();
        let chips: Vec<dtx_core::Chip> = (0..5)
            .map(|m| dtx_core::Chip::new(m, EChannel::BassDrum, 0.0))
            .collect();
        let chart = make_chart_with_chips(chips);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        set_clock(&mut app, 10000);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(judged.0.is_empty());
        assert_eq!(lane_hit_count(&app), 5);
    }

    #[test]
    fn autoplay_flows_through_judge_and_score_pipeline() {
        let mut app = build_pipeline_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        // XG scoring needs the chart's note count (nComboMax).
        app.world_mut()
            .resource_mut::<crate::resources::DrumScoring>()
            .reset(1);
        set_clock(&mut app, 100);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(judged.0.contains(&0));
        // Single all-Perfect note snaps the true score to the 1e6 remainder.
        assert!(app.world().resource::<crate::resources::Score>().0 > 0);
        assert_eq!(app.world().resource::<crate::resources::Combo>().current, 1);
    }

    #[test]
    fn autoplay_skips_non_playable_chips() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BGALayer1, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        set_clock(&mut app, 1000);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(judged.0.contains(&0), "BGA chip should be marked judged");
    }

    #[test]
    fn autoplay_no_emit_when_clock_not_started() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        // GameplayClock not started
        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(!judged.0.contains(&0));
    }

    #[test]
    fn autoplay_editor_open_marks_judged_no_reemit() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        app.world_mut()
            .resource_mut::<crate::editor::EditorOpen>()
            .0 = true;
        set_clock(&mut app, 100);

        app.update();
        assert_eq!(lane_hit_count(&app), 1);
        assert!(
            app.world().resource::<JudgedChips>().0.contains(&0),
            "editor open: autoplay must self-mark the chip judged"
        );

        // Next tick: the judge is gated off while the editor is open, so only
        // the self-mark prevents this chip from firing (and sounding) again.
        app.update();
        assert_eq!(
            lane_hit_count(&app),
            0,
            "chip must not re-emit while judged"
        );
    }

    #[test]
    fn autoplay_default_disabled() {
        let flag = AutoplayEnabled::default();
        assert!(!flag.0, "autoplay should default to off");
    }
}
