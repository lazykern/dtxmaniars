//! Pure math for the full-HUD timeline: cursor x ↔ chart ms, drag→loop
//! region snapping, bar numbering, and the press/drag gesture machine.

use bevy::prelude::*;

use crate::practice::session::LoopRegion;
use crate::timeline::{ChipTimeline, SnapDivisor};

/// Cursor movement below this (logical px) between press and release is a
/// click (seek); above it the press becomes a loop drag.
pub const CLICK_SLOP_PX: f32 = 4.0;

/// Map a cursor x (logical px) on a strip starting at `strip_min_x` with
/// `strip_width` px to chart ms over `[0, end_ms]`. Clamps to the strip.
pub fn cursor_to_ms(cursor_x: f32, strip_min_x: f32, strip_width: f32, end_ms: i64) -> i64 {
    if strip_width <= 0.0 || end_ms <= 0 {
        return 0;
    }
    let t = ((cursor_x - strip_min_x) / strip_width).clamp(0.0, 1.0) as f64;
    (t * end_ms as f64).round() as i64
}

/// Bar-snapped loop region for a drag between two chart times, in either
/// direction. Regions shorter than one bar snap up to exactly one bar.
pub fn drag_region(timeline: &ChipTimeline, anchor_ms: i64, cursor_ms: i64) -> LoopRegion {
    let (lo, hi) = if cursor_ms < anchor_ms {
        (cursor_ms, anchor_ms)
    } else {
        (anchor_ms, cursor_ms)
    };
    let start_ms = timeline.bar_start_before(lo);
    let mut end_ms = timeline.bar_start_before(hi);
    if end_ms <= start_ms {
        end_ms = timeline.snap_neighbor(start_ms, SnapDivisor::Bar, 1);
    }
    LoopRegion { start_ms, end_ms }
}

/// 1-based bar number containing `ms` (against `ChipTimeline::bar_ms`).
pub fn bar_number(bar_ms: &[i64], ms: i64) -> usize {
    match bar_ms.binary_search(&ms) {
        Ok(i) => i + 1,
        Err(0) => 1,
        Err(i) => i,
    }
}

/// Mouse gesture on the full-HUD timeline strip.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub enum TimelineGesture {
    #[default]
    Idle,
    /// Pressed inside the strip; not yet decided click vs drag.
    Pending { press_x: f32, press_ms: i64 },
    /// Dragging out a loop region anchored at the press point.
    DragLoop { anchor_ms: i64 },
}

/// One frame of mouse state, pre-resolved against the strip rect.
#[derive(Debug, Clone, Copy)]
pub struct GestureInput {
    pub just_pressed: bool,
    pub pressed: bool,
    pub inside_strip: bool,
    pub cursor_x: f32,
    pub cursor_ms: i64,
}

/// What the system should do this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEffect {
    None,
    /// Click released: seek (snap applied by the consumer via the seek
    /// message's `snap` field — SeekToChartTime shape is frozen).
    Seek { target_ms: i64 },
    /// Drag in progress: preview/commit `drag_region(anchor, cursor)`.
    LoopPreview { anchor_ms: i64 },
}

/// Pure gesture step: previous state + frame input → next state + effect.
pub fn advance_gesture(g: TimelineGesture, i: GestureInput) -> (TimelineGesture, GestureEffect) {
    match g {
        TimelineGesture::Idle => {
            if i.just_pressed && i.inside_strip {
                (
                    TimelineGesture::Pending {
                        press_x: i.cursor_x,
                        press_ms: i.cursor_ms,
                    },
                    GestureEffect::None,
                )
            } else {
                (TimelineGesture::Idle, GestureEffect::None)
            }
        }
        TimelineGesture::Pending { press_x, press_ms } => {
            if !i.pressed {
                (TimelineGesture::Idle, GestureEffect::Seek { target_ms: press_ms })
            } else if (i.cursor_x - press_x).abs() > CLICK_SLOP_PX {
                (
                    TimelineGesture::DragLoop { anchor_ms: press_ms },
                    GestureEffect::LoopPreview { anchor_ms: press_ms },
                )
            } else {
                (g, GestureEffect::None)
            }
        }
        TimelineGesture::DragLoop { anchor_ms } => {
            if i.pressed {
                (g, GestureEffect::LoopPreview { anchor_ms })
            } else {
                (TimelineGesture::Idle, GestureEffect::None)
            }
        }
    }
}

use crate::practice::session::PracticeSession;
use crate::seek::SeekToChartTime;

/// Logical-px rect of the timeline strip node (same math as
/// editor/picking.rs `node_rect`; duplicated to avoid coupling the
/// practice pillar to editor files). UI nodes carry `UiGlobalTransform`
/// (an `Affine2`, not the 3D `GlobalTransform`); its `translation` is the
/// node center in physical px. Querying `&GlobalTransform` on a UI node
/// silently matches nothing — no panic, green tests, dead mouse.
fn strip_rect(node: &ComputedNode, gt: &bevy::ui::UiGlobalTransform) -> Rect {
    let inv = node.inverse_scale_factor();
    let center = gt.translation * inv;
    let size = node.size() * inv;
    Rect::from_center_size(center, size)
}

/// Mouse on the full-HUD timeline: press+release = seek (snapped via the
/// seek message's `snap` field), press+drag = select A/B region
/// (bar-snapped, min one bar, committed live while dragging).
pub fn timeline_mouse(
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    strips: Query<
        (&ComputedNode, &bevy::ui::UiGlobalTransform),
        With<super::full_hud::FullHudTimelineStrip>,
    >,
    mut gesture: ResMut<TimelineGesture>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut seeks: MessageWriter<SeekToChartTime>,
) {
    let Ok(window) = windows.single() else { return };
    let Some(pos) = window.cursor_position() else {
        return;
    };
    let Ok((node, gt)) = strips.single() else {
        return;
    };
    let rect = strip_rect(node, gt);
    let cursor_ms = cursor_to_ms(pos.x, rect.min.x, rect.width(), timeline.end_ms);
    let input = GestureInput {
        just_pressed: buttons.just_pressed(MouseButton::Left),
        pressed: buttons.pressed(MouseButton::Left),
        inside_strip: rect.contains(pos),
        cursor_x: pos.x,
        cursor_ms,
    };
    let (next, effect) = advance_gesture(*gesture, input);
    *gesture = next;
    match effect {
        GestureEffect::None => {}
        GestureEffect::Seek { target_ms } => {
            let snapped = timeline.resolve_snap(target_ms, session.snap);
            session.scrub_cursor_ms = Some(snapped);
            seeks.write(SeekToChartTime {
                target_ms,
                snap: Some(session.snap),
                attempt_start_ms: None,
            });
        }
        GestureEffect::LoopPreview { anchor_ms } => {
            session.loop_region = Some(drag_region(&timeline, anchor_ms, cursor_ms));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::{BarLengthChangeList, BpmChangeList};
    use dtx_core::chart::{Chart, Chip, Metadata};
    use dtx_core::EChannel;

    // 120 BPM 4/4: bar = 2000ms. 8 bars → end 16000ms.
    fn timeline() -> ChipTimeline {
        let chart = Chart {
            metadata: Metadata {
                bpm: Some(120.0),
                ..Default::default()
            },
            chips: (0..8)
                .map(|i| Chip::new(i, EChannel::BassDrum, 0.0))
                .collect(),
            ..Default::default()
        };
        let bpm = BpmChangeList::from_chart(&chart);
        let bar = BarLengthChangeList::from_chart(&chart);
        ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 16_000)
    }

    #[test]
    fn cursor_to_ms_maps_strip_extent() {
        assert_eq!(cursor_to_ms(100.0, 100.0, 400.0, 16_000), 0);
        assert_eq!(cursor_to_ms(500.0, 100.0, 400.0, 16_000), 16_000);
        assert_eq!(cursor_to_ms(300.0, 100.0, 400.0, 16_000), 8_000);
        // Clamped outside the strip.
        assert_eq!(cursor_to_ms(0.0, 100.0, 400.0, 16_000), 0);
        assert_eq!(cursor_to_ms(900.0, 100.0, 400.0, 16_000), 16_000);
        // Degenerate inputs.
        assert_eq!(cursor_to_ms(300.0, 100.0, 0.0, 16_000), 0);
        assert_eq!(cursor_to_ms(300.0, 100.0, 400.0, 0), 0);
    }

    #[test]
    fn cursor_to_ms_round_trips_time_to_pct() {
        // dtx_ui::density_strip::time_to_pct is the inverse mapping used
        // to place markers; verify both directions agree.
        let ms = 6_000;
        let pct = dtx_ui::widget::density_strip::time_to_pct(ms, 16_000);
        let x = 100.0 + pct / 100.0 * 400.0;
        assert_eq!(cursor_to_ms(x, 100.0, 400.0, 16_000), ms);
    }

    #[test]
    fn drag_region_snaps_to_bars_and_orders_endpoints() {
        let tl = timeline();
        // Drag right→left across bars 2..4 (ms 4700 → 2100).
        let r = drag_region(&tl, 4_700, 2_100);
        assert_eq!(r.start_ms, 2_000);
        assert_eq!(r.end_ms, 4_000);
    }

    #[test]
    fn drag_region_shorter_than_one_bar_snaps_up() {
        let tl = timeline();
        let r = drag_region(&tl, 2_100, 2_300);
        assert_eq!(r.start_ms, 2_000);
        assert_eq!(r.end_ms, 4_000, "min region is one bar");
    }

    #[test]
    fn bar_number_is_one_based() {
        let tl = timeline();
        assert_eq!(bar_number(&tl.bar_ms, 0), 1);
        assert_eq!(bar_number(&tl.bar_ms, 1_999), 1);
        assert_eq!(bar_number(&tl.bar_ms, 2_000), 2);
        assert_eq!(bar_number(&tl.bar_ms, 5_000), 3);
    }

    #[test]
    fn gesture_click_seeks_drag_loops() {
        let idle = TimelineGesture::Idle;
        let press = GestureInput {
            just_pressed: true,
            pressed: true,
            inside_strip: true,
            cursor_x: 200.0,
            cursor_ms: 4_000,
        };
        let (g, fx) = advance_gesture(idle, press);
        assert_eq!(
            g,
            TimelineGesture::Pending {
                press_x: 200.0,
                press_ms: 4_000
            }
        );
        assert_eq!(fx, GestureEffect::None);

        // Release without movement → click seek.
        let release = GestureInput {
            just_pressed: false,
            pressed: false,
            inside_strip: true,
            cursor_x: 201.0,
            cursor_ms: 4_050,
        };
        let (g2, fx2) = advance_gesture(g, release);
        assert_eq!(g2, TimelineGesture::Idle);
        assert_eq!(fx2, GestureEffect::Seek { target_ms: 4_000 });

        // Move past the slop while held → loop drag.
        let drag = GestureInput {
            just_pressed: false,
            pressed: true,
            inside_strip: true,
            cursor_x: 240.0,
            cursor_ms: 6_500,
        };
        let (g3, fx3) = advance_gesture(g, drag);
        assert_eq!(g3, TimelineGesture::DragLoop { anchor_ms: 4_000 });
        assert_eq!(fx3, GestureEffect::LoopPreview { anchor_ms: 4_000 });

        // Release ends the drag (region was committed live).
        let (g4, fx4) = advance_gesture(g3, release);
        assert_eq!(g4, TimelineGesture::Idle);
        assert_eq!(fx4, GestureEffect::None);
    }

    #[test]
    fn gesture_press_outside_strip_is_ignored() {
        let press = GestureInput {
            just_pressed: true,
            pressed: true,
            inside_strip: false,
            cursor_x: 5.0,
            cursor_ms: 0,
        };
        let (g, fx) = advance_gesture(TimelineGesture::Idle, press);
        assert_eq!(g, TimelineGesture::Idle);
        assert_eq!(fx, GestureEffect::None);
    }
}
