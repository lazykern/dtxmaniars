//! Full practice HUD (paused tier), layout B "L-shape": bottom density
//! timeline (mouse scrub + drag loop; keyboard scrub kept) + right rail
//! (rate, snap, pre-roll, ramp config, attempt history, restart).
//! Fixed overlay — not a dtx-layout widget.

use bevy::prelude::*;
use dtx_ui::theme::{Theme, REF_HEIGHT, REF_WIDTH};
use dtx_ui::widget::density_strip::{spawn_density_strip, time_to_pct};
use dtx_ui::widget::hud_ref::{scaled_font, HudRefRect};
use game_shell::PauseState;

use super::format_chart_time;
use crate::practice::session::{preroll_target, PracticeSession};
use crate::resources::GameplayClock;
use crate::seek::SeekToChartTime;
use crate::timeline::ChipTimeline;

/// Rail geometry in ref-px (1280x720 reference space, scaled by
/// `PlayfieldLayout::scale`). The rail sits flush with the ref right edge
/// (identical to `right: 0` at 16:9) so it scales with the Now-Playing
/// card by construction — no collision at 1080p, no overflow at 720p.
pub const RAIL_REF_WIDTH: f32 = 300.0;
pub const TIMELINE_REF_HEIGHT: f32 = 72.0;
pub const RAIL_REF_LEFT: f32 = REF_WIDTH - RAIL_REF_WIDTH;
pub const RAIL_REF_HEIGHT: f32 = REF_HEIGHT - TIMELINE_REF_HEIGHT;
pub const RAIL_REF_PAD: f32 = 12.0;
pub const ROW_REF_HEIGHT: f32 = 22.0;
pub const ROW_REF_GAP: f32 = 4.0;
pub const HEADER_REF_FONT: f32 = 11.0;
pub const HEADER_REF_TOP_MARGIN: f32 = 8.0;
pub const ROW_REF_FONT: f32 = 16.0;
pub const SMALL_REF_FONT: f32 = 12.0;
pub const BLOCK_REF_MARGIN: f32 = 6.0;
/// Line budget for the two trailing blocks, header line included — so each
/// block renders its own header plus two entries. The rail's leftover band
/// below the fixed rows only fits six 12-ref-px lines at scale 1.0, and an
/// uncapped history would push lane diagnosis out of the clipped rail
/// entirely; each block keeps its most useful entries (newest attempts,
/// worst lanes) and drops the tail.
pub const HISTORY_MAX_LINES: usize = 3;
pub const DIAG_MAX_LINES: usize = 3;

/// Fixed rail content height (headers + rows + gaps + padding) in px at
/// `scale`. Attempt history + lane diagnosis render in the leftover band;
/// both are line-capped so they always fit (see `rail_content_height`).
pub fn rail_fixed_content_height(scale: f32) -> f32 {
    let headers = 3.0 * (HEADER_REF_FONT * 1.2 + HEADER_REF_TOP_MARGIN);
    let rows = RailItem::ORDER.len() as f32 * ROW_REF_HEIGHT;
    let gaps = (3 + RailItem::ORDER.len() - 1) as f32 * ROW_REF_GAP;
    (headers + rows + gaps + 2.0 * RAIL_REF_PAD) * scale
}

/// Worst-case rail height including both fully-populated trailing blocks.
pub fn rail_content_height(scale: f32) -> f32 {
    rail_content_height_for(scale, 1.0)
}

pub fn rail_content_height_for(scale: f32, text_multiplier: f32) -> f32 {
    let line = SMALL_REF_FONT * 1.2;
    let blocks =
        (HISTORY_MAX_LINES + DIAG_MAX_LINES) as f32 * line + 2.0 * (BLOCK_REF_MARGIN + ROW_REF_GAP);
    let headers = 3.0 * (HEADER_REF_FONT * 1.2 + HEADER_REF_TOP_MARGIN);
    let rows = RailItem::ORDER.len() as f32 * ROW_REF_HEIGHT;
    let gaps = (3 + RailItem::ORDER.len() - 1) as f32 * ROW_REF_GAP;
    (headers + rows + gaps + blocks) * scale * text_multiplier + 2.0 * RAIL_REF_PAD * scale
}

/// Keeps the first `max` lines of `text` — the blocks order their most
/// useful lines first (header, then newest attempts / worst lanes).
pub fn clamp_lines(text: &str, max: usize) -> String {
    text.lines().take(max).collect::<Vec<_>>().join("\n")
}

/// Root marker for the full practice HUD.
#[derive(Component)]
pub struct FullHudRoot;

#[derive(Component)]
pub(super) struct PracticeRail {
    row_height: f32,
    row_gap: f32,
    header_height: f32,
    viewport_height: f32,
    padding: f32,
}

/// The bottom timeline strip (mouse hit-target; markers are children).
#[derive(Component)]
pub struct FullHudTimelineStrip;

#[derive(Component)]
pub struct HudPlayhead;
#[derive(Component)]
pub struct HudScrubCursor;
#[derive(Component)]
pub struct HudLoopFill;
#[derive(Component)]
pub struct HudTimeText;
#[derive(Component)]
pub struct AttemptHistoryText;
#[derive(Component)]
pub struct LaneDiagnosisText;

/// Whole-row click target: click selects (and activates non-value rows).
#[derive(Component)]
pub struct RailRowButton(pub RailItem);

/// ◂ / ▸ adjust glyph: `1` field is the direction (−1 / +1).
#[derive(Component)]
pub struct RailAdjustButton(pub RailItem, pub i8);

/// Right-column value text of a row (rewritten every frame by `refresh_rail`).
#[derive(Component)]
pub struct RailValueText(pub RailItem);

/// One selectable right-rail row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RailItem {
    Resume,
    Scrub,
    RestartSection,
    SetA,
    SetB,
    ClearLoop,
    Rate,
    Snap,
    Preroll,
    Metronome,
    RampArm,
    RampStart,
    RampTarget,
    RampStep,
    RampThreshold,
    RampStreak,
    WaitMode,
}

impl RailItem {
    pub const ORDER: [RailItem; 17] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::Rate,
        RailItem::Snap,
        RailItem::Preroll,
        RailItem::Metronome,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::RampArm,
        RailItem::RampStart,
        RailItem::RampTarget,
        RailItem::RampStep,
        RailItem::RampThreshold,
        RailItem::RampStreak,
        RailItem::WaitMode,
    ];
}

/// Currently highlighted rail row.
#[derive(Resource, Default)]
pub struct RailSelection(pub usize);

/// How a rail row reacts to input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowKind {
    /// ◂ value ▸: Left/Right (or the glyph buttons) adjust; row click selects only.
    Value,
    /// Row click / Enter runs the action.
    Action,
    /// Row click / Enter flips the switch.
    Toggle,
}

pub fn rail_row_kind(item: RailItem) -> RowKind {
    use RailItem::*;
    match item {
        Scrub | Rate | Snap | Preroll | RampStart | RampTarget | RampStep | RampThreshold
        | RampStreak => RowKind::Value,
        Resume | RestartSection | SetA | SetB | ClearLoop | RampArm => RowKind::Action,
        Metronome | WaitMode => RowKind::Toggle,
    }
}

/// Static left-column label for a rail row.
pub fn rail_row_label(item: RailItem) -> &'static str {
    match item {
        RailItem::Resume => "Resume",
        RailItem::Scrub => "Scrub",
        RailItem::RestartSection => "Restart section",
        RailItem::SetA => "Set A here",
        RailItem::SetB => "Set B here",
        RailItem::ClearLoop => "Clear loop",
        RailItem::Rate => "Tempo",
        RailItem::Snap => "Snap",
        RailItem::Preroll => "Pre-roll",
        RailItem::Metronome => "Count-in",
        RailItem::RampArm => "Ramp",
        RailItem::RampStart => "Ramp start",
        RailItem::RampTarget => "Ramp target",
        RailItem::RampStep => "Ramp step",
        RailItem::RampThreshold => "Ramp pass",
        RailItem::RampStreak => "Ramp streak",
        RailItem::WaitMode => "Wait",
    }
}

/// Right-column value text for a rail row; empty for pure action rows.
pub fn rail_row_value(item: RailItem, session: &PracticeSession) -> String {
    match item {
        RailItem::Resume
        | RailItem::RestartSection
        | RailItem::SetA
        | RailItem::SetB
        | RailItem::ClearLoop => String::new(),
        RailItem::Scrub => match session.transport.scrub_cursor_ms {
            Some(ms) => format_chart_time(ms),
            None => "—".into(),
        },
        RailItem::Rate => {
            if session.trainer.ramp.armed {
                format!(
                    "x{:.2} (ramp x{:.2})",
                    session.transport.user_tempo, session.trainer.ramp.step_tempo
                )
            } else {
                format!("x{:.2}", session.transport.user_tempo)
            }
        }
        RailItem::Snap => session.transport.snap.label().into(),
        RailItem::Preroll => session.transport.preroll.label(),
        RailItem::Metronome => if session.transport.metronome {
            "on"
        } else {
            "off"
        }
        .into(),
        RailItem::RampArm => {
            if session.trainer.ramp.armed {
                let (cur, total) = crate::practice::ramp::ramp_step_index(
                    &session.trainer.ramp_config,
                    session.transport.user_tempo,
                );
                format!("ON {cur}/{total}")
            } else {
                "off".into()
            }
        }
        RailItem::RampStart => format!("x{:.2}", session.trainer.ramp_config.start_tempo),
        RailItem::RampTarget => format!("x{:.2}", session.trainer.ramp_config.target_tempo),
        RailItem::RampStep => format!("+{:.2}", session.trainer.ramp_config.step),
        RailItem::RampThreshold => {
            format!("≥{:.0}%", session.trainer.ramp_config.threshold_pct)
        }
        RailItem::RampStreak => format!("×{}", session.trainer.ramp_config.required_successes),
        RailItem::WaitMode => if session.trainer.wait_enabled {
            "ON"
        } else {
            "off"
        }
        .into(),
    }
}

/// Left/Right adjustment for `item` (`dir` = ±1). Shared by keyboard
/// arrows and the ◂/▸ mouse buttons — one code path for both.
pub fn adjust_rail_item(
    item: RailItem,
    dir: i8,
    session: &mut PracticeSession,
    timeline: &ChipTimeline,
    current_ms: i64,
) {
    match item {
        RailItem::Scrub => {
            let cur = session.transport.scrub_cursor_ms.unwrap_or(current_ms);
            session.transport.scrub_cursor_ms =
                Some(timeline.snap_neighbor(cur, session.transport.snap, dir));
        }
        RailItem::Rate => session.step_user_tempo(dir),
        RailItem::Snap => session.transport.snap = session.transport.snap.next(),
        RailItem::Preroll => session.transport.preroll = session.transport.preroll.next(),
        RailItem::RampStart => {
            let c = &mut session.trainer.ramp_config;
            c.start_tempo = (c.start_tempo + dir as f32 * 0.05).clamp(0.5, c.target_tempo - 0.05);
            let cfg = session.trainer.ramp_config;
            crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
        }
        RailItem::RampTarget => {
            let c = &mut session.trainer.ramp_config;
            c.target_tempo = (c.target_tempo + dir as f32 * 0.05).clamp(c.start_tempo + 0.05, 1.5);
            let cfg = session.trainer.ramp_config;
            crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
        }
        RailItem::RampStep => {
            let c = &mut session.trainer.ramp_config;
            c.step = (c.step + dir as f32 * 0.05).clamp(0.05, 0.25);
        }
        RailItem::RampThreshold => {
            let c = &mut session.trainer.ramp_config;
            c.threshold_pct = (c.threshold_pct + dir as f32 * 5.0).clamp(50.0, 100.0);
        }
        RailItem::RampStreak => {
            let c = &mut session.trainer.ramp_config;
            c.required_successes = (c.required_successes as i8 + dir).clamp(1, 3) as u8;
        }
        _ => {}
    }
}

/// Enter/Space (or row-click) activation for `item`. Shared by keyboard
/// and mouse. Row semantics are unchanged from the v1 rail.
#[allow(clippy::too_many_arguments)]
pub fn activate_rail_item(
    item: RailItem,
    session: &mut PracticeSession,
    timeline: &ChipTimeline,
    current_ms: i64,
    wait_state: Option<&mut crate::practice::wait::WaitState>,
    chord_hits: Option<&mut crate::practice::wait::ChordHitTimes>,
    next_pause: &mut NextState<PauseState>,
    seeks: &mut MessageWriter<SeekToChartTime>,
    practice_actions: &mut MessageWriter<crate::practice::actions::PracticeAction>,
) {
    match item {
        RailItem::Resume => next_pause.set(PauseState::Running),
        RailItem::Scrub => {
            let intent = session.transport.scrub_cursor_ms.unwrap_or(current_ms);
            seeks.write(SeekToChartTime {
                target_ms: preroll_target(timeline, session.transport.preroll, intent),
                snap: None,
                attempt_start_ms: Some(intent),
            });
            next_pause.set(PauseState::Running);
        }
        RailItem::RestartSection => {
            let intent = session
                .transport
                .loop_region
                .map(|r| r.start_ms)
                .unwrap_or(session.current_attempt.start_ms);
            seeks.write(SeekToChartTime {
                target_ms: preroll_target(timeline, session.transport.preroll, intent),
                snap: None,
                attempt_start_ms: Some(intent),
            });
            next_pause.set(PauseState::Running);
        }
        RailItem::SetA => {
            let ms =
                timeline.bar_start_before(session.transport.scrub_cursor_ms.unwrap_or(current_ms));
            session.set_loop_start(ms);
        }
        RailItem::SetB => {
            let cursor = session.transport.scrub_cursor_ms.unwrap_or(current_ms);
            let mut ms = timeline.bar_start_before(cursor);
            if let Some(r) = session.transport.loop_region {
                if ms <= r.start_ms {
                    ms = timeline.snap_neighbor(r.start_ms, crate::timeline::SnapDivisor::Bar, 1);
                }
            }
            session.set_loop_end(ms);
        }
        RailItem::ClearLoop => session.clear_loop(),
        RailItem::Metronome => {
            session.transport.metronome = !session.transport.metronome;
        }
        RailItem::RampArm => {
            practice_actions.write(crate::practice::actions::PracticeAction::ToggleRamp);
        }
        RailItem::WaitMode => {
            session.trainer.wait_enabled = !session.trainer.wait_enabled;
            if session.trainer.wait_enabled && session.trainer.ramp.armed {
                session.trainer.ramp.armed = false;
            }
            if session.trainer.wait_enabled {
                if let (Some(wait_state), Some(chord_hits)) = (wait_state, chord_hits) {
                    wait_state.begin(current_ms);
                    chord_hits.0.clear();
                }
            }
        }
        RailItem::Rate
        | RailItem::Snap
        | RailItem::Preroll
        | RailItem::RampStart
        | RailItem::RampTarget
        | RailItem::RampStep
        | RailItem::RampThreshold
        | RailItem::RampStreak => {}
    }
}

/// Attempts for the current span only (armed A/B region, or the
/// implicit whole-song span when none). `end_ms` = chart end (reserved
/// for future span-end display; span identity is start-keyed).
pub fn attempt_history_text(session: &PracticeSession, end_ms: i64) -> String {
    let span_start = session
        .transport
        .loop_region
        .filter(|r| r.end_ms != i64::MAX)
        .map(|r| r.start_ms)
        .unwrap_or(0);
    let _ = end_ms;
    let span_attempts: Vec<_> = session
        .attempt_history
        .iter()
        .filter(|a| a.start_ms == span_start)
        .collect();
    let mut lines = vec!["Attempts:".to_string()];
    for (i, a) in span_attempts.iter().enumerate().rev().take(8) {
        lines.push(format!(
            "#{}  {:.1}%  {:+.0}ms  x{:.2}",
            i + 1,
            a.accuracy_pct,
            a.mean_error_ms,
            a.tempo
        ));
    }
    lines.join("\n")
}

/// Clickable transport-row button.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum TransportButton {
    PrevBar,
    Resume,
    NextBar,
}

impl TransportButton {
    fn label(self) -> &'static str {
        match self {
            TransportButton::PrevBar => "|◀ bar",
            TransportButton::Resume => "▶ resume",
            TransportButton::NextBar => "bar ▶|",
        }
    }
}

pub fn transport_buttons(
    interactions: Query<(&Interaction, &TransportButton), Changed<Interaction>>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut next_pause: ResMut<NextState<PauseState>>,
) {
    for (interaction, button) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            TransportButton::PrevBar | TransportButton::NextBar => {
                let dir: i8 = if *button == TransportButton::NextBar {
                    1
                } else {
                    -1
                };
                let cur = session
                    .transport
                    .scrub_cursor_ms
                    .unwrap_or(clock.current_ms);
                session.transport.scrub_cursor_ms =
                    Some(timeline.snap_neighbor(cur, crate::timeline::SnapDivisor::Bar, dir));
            }
            TransportButton::Resume => next_pause.set(PauseState::Running),
        }
    }
}

pub fn spawn_full_hud(
    mut commands: Commands,
    mut selection: ResMut<RailSelection>,
    mut session: ResMut<PracticeSession>,
    clock: Res<GameplayClock>,
    timeline: Res<ChipTimeline>,
    layout: Option<Res<crate::layout::PlayfieldLayout>>,
    accessibility: Option<Res<dtx_ui::AccessibilityPolicy>>,
) {
    selection.0 = 0;
    session.transport.scrub_cursor_ms = Some(clock.current_ms);
    // Missing layout (headless tests) falls back to identity — never panic.
    let (scale, origin) = layout
        .map(|l| (l.scale, l.origin))
        .unwrap_or((1.0, Vec2::ZERO));
    let text_multiplier = accessibility.map_or(1.0, |policy| policy.text_multiplier());
    let theme = Theme::default();
    commands
        .spawn((
            FullHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(crate::ui_z::PRACTICE_FULL_HUD),
        ))
        .with_children(|root| {
            spawn_rail(
                root,
                &theme,
                scale,
                text_multiplier,
                origin,
                &session,
                &timeline,
            );
            spawn_timeline_row(
                root,
                &theme,
                scale,
                text_multiplier,
                origin,
                &clock,
                &timeline,
            );
        });
}

fn spawn_rail(
    root: &mut ChildSpawnerCommands,
    theme: &Theme,
    scale: f32,
    text_multiplier: f32,
    origin: Vec2,
    session: &PracticeSession,
    timeline: &ChipTimeline,
) {
    let rail_rect = HudRefRect::new(RAIL_REF_LEFT, 0.0, RAIL_REF_WIDTH, RAIL_REF_HEIGHT);
    let mut rail_node = Node {
        position_type: PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(ROW_REF_GAP * scale * text_multiplier),
        padding: UiRect::all(Val::Px(RAIL_REF_PAD * scale)),
        overflow: Overflow::scroll_y(),
        ..default()
    };
    rail_rect.apply(scale, origin, &mut rail_node);
    root.spawn((
        PracticeRail {
            row_height: ROW_REF_HEIGHT * scale * text_multiplier,
            row_gap: ROW_REF_GAP * scale * text_multiplier,
            header_height: (HEADER_REF_FONT * 1.2 + HEADER_REF_TOP_MARGIN)
                * scale
                * text_multiplier,
            viewport_height: RAIL_REF_HEIGHT * scale,
            padding: RAIL_REF_PAD * scale,
        },
        ScrollPosition::default(),
        rail_rect,
        rail_node,
        // Opaque: the rail now sits over the Now-Playing card and the skill
        // graph, and any translucency ghosts their text through the rows.
        BackgroundColor(theme.stage_bg),
    ))
    .with_children(|rail| {
        for (idx, item) in RailItem::ORDER.iter().enumerate() {
            let header = match idx {
                0 => Some("TRANSPORT"),
                7 => Some("LOOP"),
                10 => Some("TRAINER"),
                _ => None,
            };
            if let Some(h) = header {
                rail.spawn((
                    Text::new(h),
                    scaled_font(scale * text_multiplier, HEADER_REF_FONT),
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(
                            HEADER_REF_TOP_MARGIN * scale * text_multiplier,
                        )),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            }
            spawn_rail_row(rail, theme, scale, text_multiplier, *item, session);
        }
        rail.spawn((
            AttemptHistoryText,
            Text::new(clamp_lines(
                &attempt_history_text(session, timeline.end_ms),
                HISTORY_MAX_LINES,
            )),
            scaled_font(scale * text_multiplier, SMALL_REF_FONT),
            TextColor(theme.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(BLOCK_REF_MARGIN * scale)),
                max_width: Val::Px((RAIL_REF_WIDTH - 2.0 * RAIL_REF_PAD) * scale),
                flex_shrink: 0.0,
                ..default()
            },
        ));
        rail.spawn((
            LaneDiagnosisText,
            Text::new(clamp_lines(
                &crate::practice::diagnosis::diagnosis_text(&session.lane_diag),
                DIAG_MAX_LINES,
            )),
            scaled_font(scale * text_multiplier, SMALL_REF_FONT),
            TextColor(theme.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(BLOCK_REF_MARGIN * scale)),
                max_width: Val::Px((RAIL_REF_WIDTH - 2.0 * RAIL_REF_PAD) * scale),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    });
}

fn spawn_rail_row(
    rail: &mut ChildSpawnerCommands,
    theme: &Theme,
    scale: f32,
    text_multiplier: f32,
    item: RailItem,
    session: &PracticeSession,
) {
    rail.spawn((
        RailRowButton(item),
        Button,
        Node {
            height: Val::Px(ROW_REF_HEIGHT * scale * text_multiplier),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0 * scale),
            padding: UiRect::horizontal(Val::Px(4.0 * scale)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(Color::NONE),
    ))
    .with_children(|row| {
        row.spawn((
            Text::new(rail_row_label(item)),
            scaled_font(scale * text_multiplier, ROW_REF_FONT),
            TextColor(theme.text_primary),
        ));
        if rail_row_kind(item) == RowKind::Value {
            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0 * scale),
                ..default()
            })
            .with_children(|value| {
                value
                    .spawn((
                        RailAdjustButton(item, -1),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(4.0 * scale), Val::Px(1.0 * scale)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("◂"),
                            scaled_font(scale * text_multiplier, ROW_REF_FONT),
                            TextColor(theme.text_secondary),
                        ));
                    });
                value.spawn((
                    RailValueText(item),
                    Text::new(rail_row_value(item, session)),
                    scaled_font(scale * text_multiplier, ROW_REF_FONT),
                    TextColor(theme.text_primary),
                ));
                value
                    .spawn((
                        RailAdjustButton(item, 1),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(4.0 * scale), Val::Px(1.0 * scale)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("▸"),
                            scaled_font(scale * text_multiplier, ROW_REF_FONT),
                            TextColor(theme.text_secondary),
                        ));
                    });
            });
        } else {
            row.spawn((
                RailValueText(item),
                Text::new(rail_row_value(item, session)),
                scaled_font(scale * text_multiplier, ROW_REF_FONT),
                TextColor(theme.text_primary),
            ));
        }
    });
}

fn spawn_timeline_row(
    root: &mut ChildSpawnerCommands,
    theme: &Theme,
    scale: f32,
    text_multiplier: f32,
    origin: Vec2,
    clock: &GameplayClock,
    timeline: &ChipTimeline,
) {
    // Width 0 in the ref rect = "don't write width": the node stretches
    // window-wide via left+right. Top-anchored at ref 648 so the rail's
    // bottom edge and the timeline's top edge coincide at every scale.
    let row_rect = HudRefRect::new(0.0, RAIL_REF_HEIGHT, 0.0, TIMELINE_REF_HEIGHT);
    let mut row_node = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(0.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(12.0 * scale),
        padding: UiRect::horizontal(Val::Px(12.0 * scale)),
        ..default()
    };
    row_rect.apply(scale, origin, &mut row_node);
    root.spawn((
        row_rect,
        row_node,
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
    ))
    .with_children(|row| {
        for button in [
            TransportButton::PrevBar,
            TransportButton::Resume,
            TransportButton::NextBar,
        ] {
            row.spawn((
                button,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(10.0 * scale), Val::Px(4.0 * scale)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(button.label()),
                    scaled_font(scale * text_multiplier, SMALL_REF_FONT),
                    TextColor(theme.text_primary),
                ));
            });
        }
        row.spawn((
            HudTimeText,
            Text::new(format_chart_time(clock.current_ms)),
            scaled_font(scale * text_multiplier, ROW_REF_FONT),
            TextColor(theme.text_primary),
        ));
        let strip = spawn_density_strip(row, &timeline.density, theme);
        row.commands().entity(strip).insert(FullHudTimelineStrip);
        row.commands().entity(strip).with_children(|markers| {
            // Bar ticks along the top edge (1px hairline stays device-px).
            for &bar in &timeline.bar_ms {
                markers.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(time_to_pct(bar, timeline.end_ms)),
                        top: Val::Px(0.0),
                        width: Val::Px(1.0),
                        height: Val::Px(8.0 * scale),
                        ..default()
                    },
                    BackgroundColor(theme.text_secondary.with_alpha(0.6)),
                ));
            }
            markers.spawn((
                HudLoopFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 0.9, 0.5, 0.25)),
                Visibility::Hidden,
            ));
            markers.spawn((
                HudPlayhead,
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
            markers.spawn((
                HudScrubCursor,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                Visibility::Hidden,
            ));
        });
    });
}

pub fn despawn_full_hud(
    mut commands: Commands,
    roots: Query<Entity, With<FullHudRoot>>,
    mut session: Option<ResMut<PracticeSession>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if let Some(session) = session.as_mut() {
        session.transport.scrub_cursor_ms = None;
    }
}

/// Keyboard nav for the rail: Up/Down select, Left/Right adjust,
/// Enter/Space activate. `rail_mouse` shares the same helpers and cursor.
#[allow(clippy::too_many_arguments)]
pub fn full_hud_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<RailSelection>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut wait_state: Option<ResMut<crate::practice::wait::WaitState>>,
    mut chord_hits: Option<ResMut<crate::practice::wait::ChordHitTimes>>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
) {
    let count = RailItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
    }
    let selected = RailItem::ORDER[selection.0];

    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight);
    if left || right {
        let dir: i8 = if right { 1 } else { -1 };
        adjust_rail_item(selected, dir, &mut session, &timeline, clock.current_ms);
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        activate_rail_item(
            selected,
            &mut session,
            &timeline,
            clock.current_ms,
            wait_state.as_deref_mut(),
            chord_hits.as_deref_mut(),
            &mut next_pause,
            &mut seeks,
            &mut practice_actions,
        );
    }
}

/// Mouse path for the rail: row click selects (and activates action/toggle
/// rows); ◂/▸ click adjusts. Same helpers as the keyboard path, and every
/// click moves `RailSelection` so both inputs share one cursor.
#[allow(clippy::too_many_arguments)]
pub fn rail_mouse(
    row_clicks: Query<(&Interaction, &RailRowButton), Changed<Interaction>>,
    adjust_clicks: Query<(&Interaction, &RailAdjustButton), Changed<Interaction>>,
    mut selection: ResMut<RailSelection>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut wait_state: Option<ResMut<crate::practice::wait::WaitState>>,
    mut chord_hits: Option<ResMut<crate::practice::wait::ChordHitTimes>>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
) {
    for (interaction, RailRowButton(item)) in &row_clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(idx) = RailItem::ORDER.iter().position(|i| i == item) {
            selection.0 = idx;
        }
        // Action rows: click = select + act. Toggle rows: click = select +
        // flip. Value rows (incl. Scrub): click = select only — adjusting
        // is the ◂/▸ buttons' job, and Scrub's activation is "play here".
        if rail_row_kind(*item) != RowKind::Value {
            activate_rail_item(
                *item,
                &mut session,
                &timeline,
                clock.current_ms,
                wait_state.as_deref_mut(),
                chord_hits.as_deref_mut(),
                &mut next_pause,
                &mut seeks,
                &mut practice_actions,
            );
        }
    }
    for (interaction, RailAdjustButton(item, dir)) in &adjust_clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(idx) = RailItem::ORDER.iter().position(|i| i == item) {
            selection.0 = idx;
        }
        adjust_rail_item(*item, *dir, &mut session, &timeline, clock.current_ms);
    }
}

/// Re-render selection highlight + row values each frame while the rail is
/// open. Selected row: `selection_highlight` background + accent value.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_rail(
    selection: Res<RailSelection>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut rows: Query<(&RailRowButton, &mut BackgroundColor)>,
    mut values: Query<(&RailValueText, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<RailValueText>)>,
    mut diag_text: Query<
        &mut Text,
        (
            With<LaneDiagnosisText>,
            Without<RailValueText>,
            Without<AttemptHistoryText>,
        ),
    >,
    mut rail: Query<(&PracticeRail, &mut ScrollPosition)>,
) {
    let theme = Theme::default();
    let selected = RailItem::ORDER[selection.0 % RailItem::ORDER.len()];
    if let Ok((metrics, mut scroll)) = rail.single_mut() {
        let index = selection.0 % RailItem::ORDER.len();
        let preceding_headers = 1 + usize::from(index >= 7) + usize::from(index >= 10);
        let top = metrics.padding
            + index as f32 * (metrics.row_height + metrics.row_gap)
            + preceding_headers as f32 * (metrics.header_height + metrics.row_gap);
        let bottom = top + metrics.row_height;
        let visible_height = (metrics.viewport_height - 2.0 * metrics.padding).max(1.0);
        if top < scroll.0.y {
            scroll.0.y = top;
        } else if bottom > scroll.0.y + visible_height {
            scroll.0.y = bottom - visible_height;
        }
    }
    for (RailRowButton(item), mut bg) in &mut rows {
        bg.0 = if *item == selected {
            theme.selection_highlight
        } else {
            Color::NONE
        };
    }
    for (RailValueText(item), mut text, mut color) in &mut values {
        text.0 = rail_row_value(*item, &session);
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_primary
        };
    }
    if let Ok(mut t) = history.single_mut() {
        t.0 = clamp_lines(
            &attempt_history_text(&session, timeline.end_ms),
            HISTORY_MAX_LINES,
        );
    }
    if let Ok(mut t) = diag_text.single_mut() {
        t.0 = clamp_lines(
            &crate::practice::diagnosis::diagnosis_text(&session.lane_diag),
            DIAG_MAX_LINES,
        );
    }
}

/// Reposition playhead / scrub cursor / loop fill each frame while open.
#[allow(clippy::type_complexity)]
pub fn update_full_hud_markers(
    clock: Res<GameplayClock>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut time_text: Query<&mut Text, With<HudTimeText>>,
    mut markers: ParamSet<(
        Query<&mut Node, With<HudPlayhead>>,
        Query<(&mut Node, &mut Visibility), With<HudScrubCursor>>,
        Query<(&mut Node, &mut Visibility), With<HudLoopFill>>,
    )>,
) {
    let end = timeline.end_ms;
    if let Ok(mut t) = time_text.single_mut() {
        t.0 = format_chart_time(
            session
                .transport
                .scrub_cursor_ms
                .unwrap_or(clock.current_ms),
        );
    }
    if let Ok(mut node) = markers.p0().single_mut() {
        node.left = Val::Percent(time_to_pct(clock.current_ms, end));
    }
    if let Ok((mut node, mut vis)) = markers.p1().single_mut() {
        match session.transport.scrub_cursor_ms {
            Some(ms) => {
                node.left = Val::Percent(time_to_pct(ms, end));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
    if let Ok((mut node, mut vis)) = markers.p2().single_mut() {
        match session
            .transport
            .loop_region
            .filter(|r| r.end_ms != i64::MAX)
        {
            Some(r) => {
                let a = time_to_pct(r.start_ms, end);
                let b = time_to_pct(r.end_ms, end);
                node.left = Val::Percent(a);
                node.width = Val::Percent((b - a).max(0.0));
                *vis = Visibility::Visible;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::{AttemptRecord, LoopRegion};

    fn record(start_ms: i64, acc: f32) -> AttemptRecord {
        AttemptRecord {
            start_ms,
            end_ms: start_ms + 4_000,
            tempo: 1.0,
            counts: Default::default(),
            overhits: 0,
            max_combo: 0,
            accuracy_pct: acc,
            mean_error_ms: 0.0,
            waited: 0,
            flow_pct: 0.0,
        }
    }

    #[test]
    fn rail_fixed_content_fits_720_reference_height() {
        // Spec fit check: headers + rows + gaps + padding at scale 1.0 must
        // leave room inside the 648 ref-px band above the timeline row.
        let h = rail_fixed_content_height(1.0);
        assert!(
            h < RAIL_REF_HEIGHT,
            "rail fixed content {h} ref-px must fit {RAIL_REF_HEIGHT}"
        );
    }

    #[test]
    fn rail_worst_case_content_fits_720_reference_height() {
        // Both trailing blocks full at their line caps must still fit, so a
        // long attempt history can never push lane diagnosis out of the rail.
        let h = rail_content_height(1.0);
        assert!(
            h <= RAIL_REF_HEIGHT,
            "rail worst-case content {h} ref-px must fit {RAIL_REF_HEIGHT}"
        );
    }

    #[test]
    fn xlarge_rail_overflow_uses_the_scrollable_layout() {
        assert!(rail_content_height_for(1.0, 1.5) > RAIL_REF_HEIGHT);
        assert_eq!(
            dtx_ui::fit_overlay(
                dtx_ui::Size::new(RAIL_REF_WIDTH, rail_content_height(1.0)),
                dtx_ui::SafeArea::reference_720p(),
                1.5,
            ),
            dtx_ui::FitDecision::CompactScrollable
        );
    }

    #[test]
    fn clamp_lines_keeps_leading_lines() {
        assert_eq!(clamp_lines("a\nb\nc\nd", 3), "a\nb\nc");
        assert_eq!(clamp_lines("a\nb", 3), "a\nb");
    }

    #[test]
    fn history_and_diag_render_within_line_caps() {
        let mut s = PracticeSession::default();
        for i in 0..8 {
            s.attempt_history.push(record(0, 50.0 + i as f32));
        }
        let history = clamp_lines(&attempt_history_text(&s, 16_000), HISTORY_MAX_LINES);
        assert_eq!(history.lines().count(), HISTORY_MAX_LINES);
        let diag = clamp_lines(
            &crate::practice::diagnosis::diagnosis_text(&s.lane_diag),
            DIAG_MAX_LINES,
        );
        assert!(diag.lines().count() <= DIAG_MAX_LINES);
    }

    #[test]
    fn rail_row_value_reflects_toggles() {
        let mut s = PracticeSession::default();
        assert_eq!(rail_row_value(RailItem::WaitMode, &s), "off");
        s.trainer.wait_enabled = true;
        assert_eq!(rail_row_value(RailItem::WaitMode, &s), "ON");
        assert_eq!(rail_row_value(RailItem::Metronome, &s), "on");
        s.transport.metronome = false;
        assert_eq!(rail_row_value(RailItem::Metronome, &s), "off");
    }

    #[test]
    fn rail_row_kind_classifies_every_row() {
        use RailItem::*;
        for item in RailItem::ORDER {
            let kind = rail_row_kind(item);
            match item {
                Scrub | Rate | Snap | Preroll | RampStart | RampTarget | RampStep
                | RampThreshold | RampStreak => assert_eq!(kind, RowKind::Value),
                Resume | RestartSection | SetA | SetB | ClearLoop | RampArm => {
                    assert_eq!(kind, RowKind::Action)
                }
                Metronome | WaitMode => assert_eq!(kind, RowKind::Toggle),
            }
        }
    }

    #[test]
    fn adjust_rate_steps_and_streak_clamps() {
        let timeline = ChipTimeline::default();
        let mut s = PracticeSession::default();
        adjust_rail_item(RailItem::Rate, 1, &mut s, &timeline, 0);
        assert!((s.transport.user_tempo - 1.05).abs() < 1e-6);
        for _ in 0..10 {
            adjust_rail_item(RailItem::RampStreak, 1, &mut s, &timeline, 0);
        }
        assert_eq!(s.trainer.ramp_config.required_successes, 3, "clamped at 3");
        adjust_rail_item(RailItem::Snap, 1, &mut s, &timeline, 0);
        assert_eq!(s.transport.snap, crate::timeline::SnapDivisor::Beat);
    }

    #[test]
    fn activate_clear_loop_disarms_ramp_and_resume_sets_running() {
        use crate::practice::actions::PracticeAction;
        use crate::seek::SeekToChartTime;
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use game_shell::PauseState;

        let mut world = World::new();
        world.init_resource::<Messages<SeekToChartTime>>();
        world.init_resource::<Messages<PracticeAction>>();
        world.init_resource::<NextState<PauseState>>();
        world.init_resource::<ChipTimeline>();
        let mut session = PracticeSession::default();
        session.set_loop_start(2_000);
        session.set_loop_end(6_000);
        session.trainer.ramp.armed = true;
        world.insert_resource(session);

        world
            .run_system_once(
                |mut session: ResMut<PracticeSession>,
                 timeline: Res<ChipTimeline>,
                 mut next: ResMut<NextState<PauseState>>,
                 mut seeks: MessageWriter<SeekToChartTime>,
                 mut pa: MessageWriter<PracticeAction>| {
                    activate_rail_item(
                        RailItem::ClearLoop,
                        &mut session,
                        &timeline,
                        0,
                        None,
                        None,
                        &mut next,
                        &mut seeks,
                        &mut pa,
                    );
                    activate_rail_item(
                        RailItem::Resume,
                        &mut session,
                        &timeline,
                        0,
                        None,
                        None,
                        &mut next,
                        &mut seeks,
                        &mut pa,
                    );
                    activate_rail_item(
                        RailItem::RampArm,
                        &mut session,
                        &timeline,
                        0,
                        None,
                        None,
                        &mut next,
                        &mut seeks,
                        &mut pa,
                    );
                },
            )
            .expect("helpers run");

        let session = world.resource::<PracticeSession>();
        assert!(session.transport.loop_region.is_none());
        assert!(!session.trainer.ramp.armed);
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let toggles: Vec<PracticeAction> = world
            .resource::<Messages<PracticeAction>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(toggles, vec![PracticeAction::ToggleRamp]);
    }

    #[test]
    fn attempt_history_filters_to_current_span() {
        let mut s = PracticeSession::default();
        s.transport.loop_region = Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
        s.attempt_history.push(record(0, 50.0)); // old free-play span
        s.attempt_history.push(record(2_000, 91.0)); // this loop
        s.attempt_history.push(record(8_000, 60.0)); // scrub junk
        s.attempt_history.push(record(2_000, 95.0)); // this loop
        let text = attempt_history_text(&s, 16_000);
        assert!(text.contains("91.0%") && text.contains("95.0%"));
        assert!(!text.contains("50.0%") && !text.contains("60.0%"));
    }

    #[test]
    fn attempt_history_no_loop_uses_implicit_whole_song_span() {
        let mut s = PracticeSession::default();
        s.attempt_history.push(record(0, 88.0)); // implicit span
        s.attempt_history.push(record(4_000, 70.0)); // partial
        let text = attempt_history_text(&s, 16_000);
        assert!(text.contains("88.0%"));
        assert!(!text.contains("70.0%"));
    }
}
