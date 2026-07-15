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
    Seek {
        target_ms: i64,
    },
    /// Drag in progress: preview/commit `drag_region(anchor, cursor)`.
    LoopPreview {
        anchor_ms: i64,
    },
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
                let effect = if i.inside_strip {
                    GestureEffect::Seek {
                        target_ms: press_ms,
                    }
                } else {
                    GestureEffect::None
                };
                (TimelineGesture::Idle, effect)
            } else if (i.cursor_x - press_x).abs() > CLICK_SLOP_PX {
                (
                    TimelineGesture::DragLoop {
                        anchor_ms: press_ms,
                    },
                    GestureEffect::LoopPreview {
                        anchor_ms: press_ms,
                    },
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

#[derive(Component)]
pub struct PracticeTimelineRoot;

#[derive(Component)]
pub struct PracticeTimelineStrip;

#[derive(Component)]
pub struct PracticePlayhead;

#[derive(Component)]
pub struct PracticeLoopFill;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticeLoopHandle {
    Start,
    End,
}

#[derive(Component)]
pub struct PracticeTimeText;

#[derive(Component)]
pub(super) struct PreviewTransportText;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewTransportButton {
    PrevBar,
    PlayPause,
    NextBar,
}

pub(super) fn spawn_timeline(
    root: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    flow: &crate::practice::PracticeFlow,
    draft: &crate::practice::PracticeDraft,
    timeline: &crate::timeline::ChipTimeline,
    height: f32,
    row_mode: super::setup::PracticeTransportRowMode,
) {
    root.spawn((
        PracticeTimelineRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(height),
            min_height: Val::Px(height),
            flex_direction: if row_mode == super::setup::PracticeTransportRowMode::Single {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            },
            flex_wrap: FlexWrap::NoWrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(super::setup::TRANSPORT_CONTROL_GAP),
            row_gap: Val::Px(dtx_ui::SpacingRole::Sm.px()),
            padding: UiRect::axes(
                Val::Px(super::setup::TIMELINE_HORIZONTAL_PADDING),
                Val::Px(10.0),
            ),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.88)),
    ))
    .with_children(|timeline_root| {
        timeline_root
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(super::setup::TRANSPORT_CONTROL_GAP),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|transport_row| {
                for button in [
                    PreviewTransportButton::PrevBar,
                    PreviewTransportButton::PlayPause,
                    PreviewTransportButton::NextBar,
                ] {
                    transport_row
                        .spawn((
                            button,
                            Button,
                            Node {
                                min_width: Val::Px(super::setup::TRANSPORT_BUTTON_MIN_WIDTH),
                                min_height: Val::Px(40.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                        ))
                        .with_children(|button_root| {
                            let label = match button {
                                PreviewTransportButton::PrevBar => "Previous bar",
                                PreviewTransportButton::PlayPause
                                    if flow.preview == crate::practice::PreviewState::Playing =>
                                {
                                    "Pause Preview"
                                }
                                PreviewTransportButton::PlayPause => "Play Preview",
                                PreviewTransportButton::NextBar => "Next bar",
                            };
                            let text = super::setup::spawn_text(
                                button_root,
                                label,
                                dtx_ui::TypographyRole::Label,
                                theme.text_primary,
                            );
                            if button == PreviewTransportButton::PlayPause {
                                button_root
                                    .commands()
                                    .entity(text)
                                    .insert(PreviewTransportText);
                            }
                        });
                }
                transport_row.spawn((
                    PracticeTimeText,
                    Text::new(super::format_chart_time(0)),
                    dtx_ui::Theme::font(dtx_ui::Typography.base_px(dtx_ui::TypographyRole::Label)),
                    dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
                    TextColor(theme.text_primary),
                    Node {
                        min_width: Val::Px(super::setup::TRANSPORT_TIME_MIN_WIDTH),
                        ..default()
                    },
                ));
            });
        let strip = dtx_ui::widget::density_strip::spawn_density_strip(
            timeline_root,
            &timeline.density,
            theme,
        );
        timeline_root.commands().entity(strip).insert((
            PracticeTimelineStrip,
            Node {
                width: if row_mode == super::setup::PracticeTransportRowMode::Stacked {
                    Val::Percent(100.0)
                } else {
                    Val::Auto
                },
                min_width: Val::Px(super::setup::TIMELINE_STRIP_MIN_WIDTH),
                height: Val::Px(44.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(1.0),
                ..default()
            },
        ));
        timeline_root
            .commands()
            .entity(strip)
            .with_children(|markers| {
                for &bar_ms in &timeline.bar_ms {
                    markers.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(dtx_ui::widget::density_strip::time_to_pct(
                                bar_ms,
                                timeline.end_ms,
                            )),
                            top: Val::Px(0.0),
                            width: Val::Px(1.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        BackgroundColor(theme.text_secondary.with_alpha(0.7)),
                    ));
                }
                markers.spawn((
                    PracticeLoopFill,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(theme.selection_highlight),
                    if draft.loop_region.is_some() {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ));
                markers.spawn((
                    PracticePlayhead,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(2.0),
                        ..default()
                    },
                    BackgroundColor(theme.accent),
                ));
                for (handle, label) in [
                    (PracticeLoopHandle::Start, "A"),
                    (PracticeLoopHandle::End, "B"),
                ] {
                    markers
                        .spawn((
                            handle,
                            Node {
                                position_type: PositionType::Absolute,
                                left: if handle == PracticeLoopHandle::Start {
                                    Val::Percent(0.0)
                                } else {
                                    Val::Auto
                                },
                                right: if handle == PracticeLoopHandle::End {
                                    Val::Percent(0.0)
                                } else {
                                    Val::Auto
                                },
                                top: Val::Px(0.0),
                                bottom: Val::Px(0.0),
                                min_width: Val::Px(20.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(theme.stage_bg),
                            BorderColor::all(theme.accent),
                            if draft.loop_region.is_some() {
                                Visibility::Visible
                            } else {
                                Visibility::Hidden
                            },
                        ))
                        .with_children(|marker| {
                            super::setup::spawn_text(
                                marker,
                                label,
                                dtx_ui::TypographyRole::Hint,
                                theme.text_primary,
                            );
                        });
                }
            });
    });
}

pub(super) fn preview_transport_buttons(
    interactions: Query<(&Interaction, &PreviewTransportButton), Changed<Interaction>>,
    flow: Res<crate::practice::PracticeFlow>,
    mut actions: MessageWriter<super::setup_controls::PracticeUiAction>,
) {
    for (interaction, button) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let preview = match button {
            PreviewTransportButton::PrevBar => crate::practice::PreviewAction::PrevBar,
            PreviewTransportButton::PlayPause
                if flow.preview == crate::practice::PreviewState::Playing =>
            {
                crate::practice::PreviewAction::Pause
            }
            PreviewTransportButton::PlayPause => crate::practice::PreviewAction::Play,
            PreviewTransportButton::NextBar => crate::practice::PreviewAction::NextBar,
        };
        actions.write(super::setup_controls::PracticeUiAction::Preview(preview));
    }
}

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

/// Mouse on the setup timeline: click seeks the preview and drag edits the draft loop.
pub fn timeline_mouse(
    windows: Query<&Window>,
    buttons: Res<ButtonInput<MouseButton>>,
    strips: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<PracticeTimelineStrip>>,
    mut gesture: ResMut<TimelineGesture>,
    mut draft: ResMut<crate::practice::PracticeDraft>,
    timeline: Res<ChipTimeline>,
    mut actions: MessageWriter<super::setup_controls::PracticeUiAction>,
) {
    let Ok(window) = windows.single() else {
        *gesture = TimelineGesture::Idle;
        return;
    };
    let Some(pos) = window.cursor_position() else {
        *gesture = TimelineGesture::Idle;
        return;
    };
    let Ok((node, gt)) = strips.single() else {
        *gesture = TimelineGesture::Idle;
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
            let snapped = timeline.resolve_snap(target_ms, draft.snap);
            actions.write(super::setup_controls::PracticeUiAction::Preview(
                crate::practice::PreviewAction::Seek(snapped),
            ));
        }
        GestureEffect::LoopPreview { anchor_ms } => {
            draft.loop_region = Some(drag_region(&timeline, anchor_ms, cursor_ms));
            draft.source = crate::practice::PracticeDraftSource::Custom;
        }
    }
}

pub(super) fn reset_timeline_gesture(
    flow: Option<Res<crate::practice::PracticeFlow>>,
    mut gesture: ResMut<TimelineGesture>,
) {
    if !flow.is_some_and(|flow| {
        matches!(
            flow.phase,
            crate::practice::PracticePhase::Setup | crate::practice::PracticePhase::Editing
        )
    }) {
        *gesture = TimelineGesture::Idle;
    }
}

pub(super) fn clear_timeline_gesture(mut gesture: ResMut<TimelineGesture>) {
    *gesture = TimelineGesture::Idle;
}

pub(super) fn update_timeline_markers(
    clock: Res<crate::resources::GameplayClock>,
    draft: Res<crate::practice::PracticeDraft>,
    timeline: Res<ChipTimeline>,
    mut time_text: Query<&mut Text, With<PracticeTimeText>>,
    mut markers: ParamSet<(
        Query<&mut Node, With<PracticePlayhead>>,
        Query<(&mut Node, &mut Visibility), With<PracticeLoopFill>>,
        Query<(&PracticeLoopHandle, &mut Node, &mut Visibility)>,
    )>,
) {
    if let Ok(mut text) = time_text.single_mut() {
        text.0 = super::format_chart_time(clock.current_ms);
    }
    if let Ok(mut playhead) = markers.p0().single_mut() {
        playhead.left = Val::Percent(dtx_ui::widget::density_strip::time_to_pct(
            clock.current_ms,
            timeline.end_ms,
        ));
    }
    let region = draft.loop_region.filter(|region| region.end_ms != i64::MAX);
    if let Ok((mut fill, mut visibility)) = markers.p1().single_mut() {
        match region {
            Some(region) => {
                let start =
                    dtx_ui::widget::density_strip::time_to_pct(region.start_ms, timeline.end_ms);
                let end =
                    dtx_ui::widget::density_strip::time_to_pct(region.end_ms, timeline.end_ms);
                fill.left = Val::Percent(start);
                fill.width = Val::Percent((end - start).max(0.0));
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
    for (handle, mut node, mut visibility) in &mut markers.p2() {
        match region {
            Some(region) => {
                let percent = dtx_ui::widget::density_strip::time_to_pct(
                    match handle {
                        PracticeLoopHandle::Start => region.start_ms,
                        PracticeLoopHandle::End => region.end_ms,
                    },
                    timeline.end_ms,
                );
                match handle {
                    PracticeLoopHandle::Start => {
                        node.left = Val::Percent(percent);
                        node.right = Val::Auto;
                    }
                    PracticeLoopHandle::End => {
                        node.left = Val::Auto;
                        node.right = Val::Percent(100.0 - percent);
                    }
                }
                *visibility = Visibility::Visible;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

pub(super) fn update_transport_label(
    flow: Res<crate::practice::PracticeFlow>,
    mut labels: Query<&mut Text, With<PreviewTransportText>>,
) {
    let label = if flow.preview == crate::practice::PreviewState::Playing {
        "Pause Preview"
    } else {
        "Play Preview"
    };
    for mut text in &mut labels {
        text.0 = label.to_owned();
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
