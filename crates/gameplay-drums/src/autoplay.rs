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
use crate::judge::{chip_target_ms, BpmChangeList, JudgedChips};
use crate::lane_map::{lane_of, LaneId};
use crate::resources::ActiveChart;
use dtx_timing::AudioClock;

/// Resource flag — when true, the autoplay system emits LaneHit for each
/// chip at its target ms. Toggleable from config (M5+).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct AutoplayEnabled(pub bool);

/// Plugin. Adds the `AutoplayEnabled` resource + system.
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AutoplayEnabled>()
        .add_systems(Update, autoplay_system.run_if(autoplay_active));
}

fn autoplay_active(flag: Res<AutoplayEnabled>) -> bool {
    flag.0
}

/// System: for each un-judged chip whose target_ms <= AudioClock.current_ms,
/// emit a LaneHit event with audio_ms = target_ms (delta = 0 → Perfect).
///
/// Runs at the chart's intrinsic time. If BGM is paused, the bot waits.
pub fn autoplay_system(
    clock: Res<AudioClock>,
    chart: Res<ActiveChart>,
    bpm_changes: Res<BpmChangeList>,
    mut judged: ResMut<JudgedChips>,
    mut lane_hits: MessageWriter<LaneHit>,
) {
    let Some(current_ms) = clock.current_ms else {
        return;
    };

    let base_bpm = chart.chart.metadata.bpm.unwrap_or(120.0);

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
        let target_ms = chip_target_ms(chip, base_bpm, &bpm_changes.changes);
        if target_ms <= current_ms {
            lane_hits.write(LaneHit {
                lane: lane as LaneId,
                audio_ms: target_ms,
            });
            judged.0.insert(idx);
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
            .init_resource::<AudioClock>()
            .init_resource::<JudgedChips>()
            .init_resource::<BpmChangeList>()
            .init_resource::<AutoplayEnabled>()
            .add_message::<LaneHit>()
            .add_systems(Update, autoplay_system.run_if(autoplay_active));
        app
    }

    #[test]
    fn autoplay_disabled_emits_nothing() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = false;
        app.world_mut().resource_mut::<AudioClock>().current_ms = Some(2000);

        // No lane_hits should be emitted when disabled.
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
        // One chip at measure 0, fraction 0.0 → target_ms = 0
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        // Audio at 100ms — past the chip target (0).
        app.world_mut().resource_mut::<AudioClock>().current_ms = Some(100);

        app.update();

        // The chip should be marked as judged.
        let judged = app.world().resource::<JudgedChips>();
        assert!(judged.0.contains(&0), "chip 0 should be marked judged");
    }

    #[test]
    fn autoplay_no_emit_when_audio_before_target() {
        let mut app = build_app();
        // Chip at measure 1, fraction 0.0 → target_ms = 2000 (at 120 BPM)
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(1, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        // Audio at 500ms — before target (2000).
        app.world_mut().resource_mut::<AudioClock>().current_ms = Some(500);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(
            !judged.0.contains(&0),
            "chip should NOT be marked judged yet"
        );
    }

    #[test]
    fn autoplay_emits_multiple_chips_in_sequence() {
        let mut app = build_app();
        // 5 chips at measures 0, 1, 2, 3, 4 → targets 0, 2000, 4000, 6000, 8000
        let chips: Vec<dtx_core::Chip> = (0..5)
            .map(|m| dtx_core::Chip::new(m, EChannel::BassDrum, 0.0))
            .collect();
        let chart = make_chart_with_chips(chips);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        // Audio at 10000ms — past all 5 chips.
        app.world_mut().resource_mut::<AudioClock>().current_ms = Some(10000);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        for i in 0..5 {
            assert!(judged.0.contains(&i), "chip {i} should be judged");
        }
    }

    #[test]
    fn autoplay_skips_non_playable_chips() {
        let mut app = build_app();
        // BGA chip — not playable but should be marked judged.
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BGALayer1, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        app.world_mut().resource_mut::<AudioClock>().current_ms = Some(1000);

        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(judged.0.contains(&0), "BGA chip should be marked judged");
    }

    #[test]
    fn autoplay_no_emit_when_audio_clock_none() {
        let mut app = build_app();
        let chart = make_chart_with_chips(vec![dtx_core::Chip::new(0, EChannel::BassDrum, 0.0)]);
        app.world_mut().resource_mut::<ActiveChart>().chart = chart;
        app.world_mut().resource_mut::<AutoplayEnabled>().0 = true;
        // AudioClock.current_ms = None
        app.update();

        let judged = app.world().resource::<JudgedChips>();
        assert!(!judged.0.contains(&0));
    }

    #[test]
    fn autoplay_default_disabled() {
        let flag = AutoplayEnabled::default();
        assert!(!flag.0, "autoplay should default to off");
    }
}
