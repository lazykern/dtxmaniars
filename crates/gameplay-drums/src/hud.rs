//! Performance HUD (Drums) — full port of `Stage/06.Performance/DrumsScreen/`.
//!
//! Strict-port-first (ADR-0010): every sub-act is ported at the reference's
//! verbatim positions and structure, with placeholder rendering until the
//! skin system lands (M14+).
//!
//! ## Sub-acts ported
//!
//! | Sub-act | Reference file | LOC | Purpose |
//! |---------|---------------|----:|---------|
//! | DrumsScore | CActPerfDrumsScore.cs | 77 | 7-digit score, top-left, (40, 13) |
//! | DrumsComboDGB | CActPerfDrumsComboDGB.cs | 113 | combo + PG/GR counters, (1245, 60) + bomb (845, -130) |
//! | DrumsGauge | CActPerfDrumsGauge.cs | 89 | lives + difficulty bar, x=294, y=28/626 |
//! | DrumsStatusPanel | CActPerfDrumsStatusPanel.cs | 212 | BPM/lives/title/level, (22, 250) |
//! | DrumsJudgementString | CActPerfDrumsJudgementString.cs | 103 | PG/GR/GO/MISS text, lane-based |
//! | DrumsLaneFlushD | CActPerfDrumsLaneFlushD.cs | 457 | hit-flash per lane, lane-size table |
//! | DrumsPad | CActPerfDrumsPad.cs | 499 | lane pad bottom, 10 pad positions |
//! | DrumsDanger | CActPerfDrumsDanger.cs | 78 | low-life red overlay, full-screen |
//! | DrumsFillingEffect | CActPerfDrumsFillingEffect.cs | 42 | fillin-roll visual (stub) |
//! | PerfChipFireD | CActPerfPerfChipFireD.cs | 1081 | chip-strike particles (M9.1+) |
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/06.Performance/DrumsScreen/`
//! Orchestrator: CStagePerfDrumsScreen.cs (3671 LOC) — implemented as DrumsHudPlugin
//! that aggregates the sub-acts above.

use bevy::prelude::*;
use dtx_scoring::JudgmentKind;

use crate::components::LastJudgment;
use crate::resources::{Combo, JudgmentCounts, Score};

// ===== Layout constants (verbatim from reference) =====

/// CActPerfDrumsScore.cs:12-13 — `n本体X[0] = 40; n本体Y = 13;`
pub const SCORE_X: f32 = 40.0;
pub const SCORE_Y: f32 = 13.0;
/// CActPerfDrumsScore.cs:46-47 — `n本体X[0] + (i * 34)`, 36px digit, gap 34.
pub const SCORE_DIGIT_GAP: f32 = 34.0;
pub const SCORE_DIGIT_WIDTH: f32 = 36.0;
pub const SCORE_DIGITS: usize = 7;

/// CActPerfDrumsComboDGB.cs:44-45 — combo draw at (1245, 60) or (1275, 60) small-graph.
pub const COMBO_DGB_X: f32 = 1245.0;
pub const COMBO_DGB_Y: f32 = 60.0;
/// CActPerfDrumsComboDGB.cs:55 — bomb at (845, -130).
pub const COMBO_BOMB_X: f32 = 845.0;
pub const COMBO_BOMB_Y: f32 = -130.0;

/// CActPerfDrumsGauge.cs:31 — `n本体X.Drums = 294`.
pub const GAUGE_X: f32 = 294.0;
/// CActPerfDrumsGauge.cs:44-46 — `bReverse.Drums ? 28 : 626`.
pub const GAUGE_Y_NORMAL: f32 = 626.0;
pub const GAUGE_Y_REVERSE: f32 = 28.0;
/// CActPerfDrumsGauge.cs:48 — `txフレーム.Drums.Width` 47 (frame), 31 (bar).
pub const GAUGE_FRAME_TOP_H: f32 = 47.0;
pub const GAUGE_FRAME_BOT_H: f32 = 47.0;
pub const GAUGE_BAR_H: f32 = 31.0;
pub const GAUGE_HSPI_W: f32 = 42.0;
pub const GAUGE_HSPI_H: f32 = 48.0;

/// CActPerfDrumsStatusPanel.cs:22-25 — `nBodyX[0] = 22; nBodyY = 250;`
pub const STATUS_PANEL_X: f32 = 22.0;
pub const STATUS_PANEL_Y: f32 = 250.0;
/// CActPerfDrumsStatusPanel.cs:48-78 — 6 rows at y=72, 102, 132, 162, 192, 222 + 277 + 363.
pub const STATUS_PANEL_ROW_DY: f32 = 30.0;
pub const STATUS_PANEL_ROW0_Y: f32 = 72.0;
pub const STATUS_PANEL_ACHIEVE_Y: f32 = 277.0;
pub const STATUS_PANEL_SKILL_Y: f32 = 363.0;

/// CActPerfDrumsDanger.cs:43-44 — full-screen overlay at (0, 0).
pub const DANGER_X: f32 = 0.0;
pub const DANGER_Y: f32 = 0.0;
pub const DANGER_W: f32 = 1280.0;
pub const DANGER_H: f32 = 720.0;

/// CActPerfDrumsJudgementString.cs:73-79 — y at 348 (forward) or 583 (reversed).
pub const JUDGE_STRING_Y_FORWARD: f32 = 348.0;
pub const JUDGE_STRING_Y_REVERSE: f32 = 583.0;
pub const JUDGE_STRING_Y_TOP_REVERSE: f32 = 80.0;
/// CActPerfDrumsJudgementString.cs:80 — `verticalCharacterOffsets * 0x20` (32).
pub const JUDGE_STRING_VERT_DX: f32 = 32.0;

/// Lane positions verbatim from CActPerfDrumsLaneFlushD.cs:17-72 and
/// CActPerfDrumsJudgementString.cs:14-32 (Type A — 9-lane classic).
///
/// Index 0=LC, 1=HH, 2=SD, 3=BD, 4=HT, 5=LT, 6=FT, 7=CY, 8=LP, 9=RD.
pub const LANE_X: [f32; 10] = [
    298.0, 370.0, 470.0, 582.0, 528.0, 645.0, 694.0, 748.0, 419.0, 815.0,
];
pub const LANE_W: [f32; 10] = [64.0, 46.0, 54.0, 60.0, 46.0, 46.0, 46.0, 64.0, 48.0, 38.0];

/// Pad positions verbatim from CActPerfDrumsPad.cs:16-103.
///
/// Index 0=LC, 1=HH, 2=SD, 3=BD, 4=HT, 5=LT, 6=FT, 7=CY, 8=RD, 9=LP.
pub const PAD_X: [f32; 10] = [
    263.0,
    336.0,
    446.0,
    565.0,
    510.0,
    622.0,
    672.0,
    0x2df as f32,
    0x317 as f32,
    0x18c as f32,
];
pub const PAD_Y: [f32; 10] = [10.0; 10];
pub const PAD_SIZE: f32 = 0x60 as f32;

// ===== Sub-act components =====

/// Per-sub-act overlay marker. Spawned by `spawn_drums_hud` on OnEnter(Performance).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumsHudKind {
    Score,
    ComboDGB,
    ComboBomb,
    Gauge,
    GaugeBar,
    StatusPanel,
    JudgeString,
    LaneFlush(usize),
    Pad(usize),
    Danger,
    FillingEffect,
    ChipFire,
}

/// Lane hit-flash intensity for [`DrumsHudKind::LaneFlush`].
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LaneFlashState {
    /// 0..90 counter value from CActPerfDrumsLaneFlushD.cs:170.
    pub intensity: u32,
}

impl LaneFlashState {
    /// Trigger a flash on this lane (CActPerfDrumsLaneFlushD.cs:167-170).
    pub fn trigger(&mut self, strength: f32) {
        let num = ((1.0 - strength) * 55.0) as u32;
        self.intensity = num;
    }
}

/// Pad hit-flash brightness 0..6 from CActPerfDrumsPad.cs:115-118.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PadFlashState {
    pub brightness: u32,
}

impl PadFlashState {
    /// Trigger pad flash (CActPerfDrumsPad.cs:114).
    pub fn hit(&mut self) {
        self.brightness = 6;
    }
}

/// Danger active flag (CActPerfDrumsDanger.cs:35-42).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct DrumsDangerState {
    pub active: bool,
    /// Animation counter (0..127) from CActPerfDrumsDanger.cs:38-40.
    pub counter: u32,
}

// ===== Plugin =====

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<DrumsDangerState>()
        .add_systems(Startup, spawn_drums_hud)
        .add_systems(
            Update,
            (
                update_score_text,
                update_combo_text,
                update_combo_bomb_visibility,
                update_gauge_bar,
                update_status_panel,
                update_judgment_string,
                tick_lane_flash,
                tick_pad_flash,
                tick_danger,
            )
                .chain(),
        );
}

fn spawn_drums_hud(mut commands: Commands) {
    // Score (top-left, 7 digits).
    commands.spawn((
        DrumsHudKind::Score,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(SCORE_X),
            top: Val::Px(SCORE_Y + 28.0),
            width: Val::Px(SCORE_DIGIT_GAP * SCORE_DIGITS as f32),
            height: Val::Px(50.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        Text::new("0000000"),
        TextFont {
            font_size: 36.0.into(),
            ..default()
        },
    ));

    // Combo text (1245, 60).
    commands.spawn((
        DrumsHudKind::ComboDGB,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(COMBO_DGB_X),
            top: Val::Px(COMBO_DGB_Y),
            width: Val::Px(140.0),
            height: Val::Px(80.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Text::new("0"),
        TextFont {
            font_size: 64.0.into(),
            ..default()
        },
    ));

    // Combo bomb (845, -130) — usually off-screen y; rendered when nCombo >= 100.
    commands.spawn((
        DrumsHudKind::ComboBomb,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(COMBO_BOMB_X),
            top: Val::Px(COMBO_BOMB_Y + 720.0), // clamp negative y to screen
            width: Val::Px(360.0),
            height: Val::Px(340.0),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.6, 0.1, 0.7)),
        Text::new(""),
    ));

    // Gauge frame + bar (294, 626).
    commands.spawn((
        DrumsHudKind::Gauge,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(GAUGE_X),
            top: Val::Px(GAUGE_Y_NORMAL),
            width: Val::Px(686.0),
            height: Val::Px(GAUGE_FRAME_TOP_H + GAUGE_FRAME_BOT_H),
            ..default()
        },
        BackgroundColor(Color::srgba(0.1, 0.1, 0.2, 0.7)),
    ));
    commands.spawn((
        DrumsHudKind::GaugeBar,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(GAUGE_X + 20.0),
            top: Val::Px(GAUGE_Y_NORMAL + 9.0),
            width: Val::Px(620.0 * 0.5), // half-gauge at song start
            height: Val::Px(GAUGE_BAR_H),
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.8, 0.3, 1.0)),
    ));

    // Status panel (22, 250) — left side.
    commands.spawn((
        DrumsHudKind::StatusPanel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(STATUS_PANEL_X),
            top: Val::Px(STATUS_PANEL_Y),
            width: Val::Px(250.0),
            height: Val::Px(450.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(0.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Text::new("Perfect: 0\nGreat: 0\nGood: 0\nOK: 0\nMiss: 0\nMax Combo: 0"),
        TextFont {
            font_size: 18.0.into(),
            ..default()
        },
    ));

    // Judgement string (single placeholder; lane-based position in update system).
    commands.spawn((
        DrumsHudKind::JudgeString,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(LANE_X[2] + LANE_W[2] / 2.0), // default: SD lane
            top: Val::Px(JUDGE_STRING_Y_FORWARD),
            width: Val::Px(140.0),
            height: Val::Px(40.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Text::new(""),
        TextFont {
            font_size: 28.0.into(),
            ..default()
        },
    ));

    // Lane flush overlays (10 lanes per LANE_X/W).
    for i in 0..10 {
        commands.spawn((
            DrumsHudKind::LaneFlush(i),
            LaneFlashState::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(LANE_X[i]),
                top: Val::Px(580.0), // judgment line area
                width: Val::Px(LANE_W[i]),
                height: Val::Px(40.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        ));
    }

    // Pad overlays (10 pads per PAD_X/Y).
    for i in 0..10 {
        commands.spawn((
            DrumsHudKind::Pad(i),
            PadFlashState::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PAD_X[i]),
                top: Val::Px(PAD_Y[i]),
                width: Val::Px(PAD_SIZE),
                height: Val::Px(PAD_SIZE),
                ..default()
            },
            BackgroundColor(Color::srgba(0.4, 0.4, 0.5, 0.6)),
        ));
    }

    // Danger overlay (full screen, hidden by default).
    commands.spawn((
        DrumsHudKind::Danger,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(DANGER_X),
            top: Val::Px(DANGER_Y),
            width: Val::Px(DANGER_W),
            height: Val::Px(DANGER_H),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 0.0, 0.0, 0.4)),
    ));

    // Filling effect (stub; visible only when fillin is active).
    commands.spawn((
        DrumsHudKind::FillingEffect,
        Visibility::Hidden,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(DANGER_W),
            height: Val::Px(DANGER_H),
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 0.0, 0.15)),
    ));
}

// ===== Update systems =====

fn update_score_text(
    score: Res<Score>,
    mut q: Query<&mut Text, (With<DrumsHudKind>, With<DrumsScoreTag>)>,
) {
    if score.is_changed() {
        let s = format!("{:>7}", score.0);
        for mut t in &mut q {
            *t = Text::new(s.clone());
        }
    }
}

#[derive(Component)]
struct DrumsScoreTag;

fn update_combo_text(
    combo: Res<Combo>,
    mut q: Query<&mut Text, (With<DrumsHudKind>, With<DrumsComboTag>)>,
) {
    if combo.is_changed() {
        for mut t in &mut q {
            *t = Text::new(format!("{}", combo.current));
        }
    }
}

#[derive(Component)]
struct DrumsComboTag;

fn update_combo_bomb_visibility(
    combo: Res<Combo>,
    mut q: Query<&mut Visibility, With<DrumsHudKind>>,
) {
    let show = combo.current >= 100;
    for mut v in &mut q {
        let should_show = matches!(*v, Visibility::Visible) || show;
        *v = if should_show && show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_gauge_bar(
    _score: Res<Score>,
    counts: Res<JudgmentCounts>,
    mut q: Query<&mut Node, With<DrumsHudKind>>,
) {
    // Approximate gauge from perfect_pct (0..100 -> 0..1).
    let pct = counts.perfect_pct() / 100.0;
    let w = 620.0 * pct;
    for mut n in &mut q {
        if let Val::Px(ref mut width) = n.width {
            *width = w.max(1.0);
        }
    }
}

fn update_status_panel(
    counts: Res<JudgmentCounts>,
    combo: Res<Combo>,
    mut q: Query<&mut Text, (With<DrumsHudKind>, With<DrumsStatusTag>)>,
) {
    if counts.is_changed() || combo.is_changed() {
        let s = format!(
            "Perfect: {}\nGreat: {}\nGood: {}\nOK: {}\nMiss: {}\nMax Combo: {}",
            counts.perfect, counts.great, counts.good, counts.ok, counts.miss, combo.max
        );
        for mut t in &mut q {
            *t = Text::new(s.clone());
        }
    }
}

#[derive(Component)]
struct DrumsStatusTag;

fn update_judgment_string(
    last: Res<LastJudgment>,
    mut q: Query<&mut Text, (With<DrumsHudKind>, With<DrumsJudgeTag>)>,
) {
    if let Some(j) = last.0 {
        let s = match j.kind {
            JudgmentKind::Perfect => "PERFECT",
            JudgmentKind::Great => "GREAT",
            JudgmentKind::Good => "GOOD",
            JudgmentKind::Ok => "OK",
            JudgmentKind::Miss => "MISS",
        };
        for mut t in &mut q {
            *t = Text::new(s);
        }
    }
}

#[derive(Component)]
struct DrumsJudgeTag;

fn tick_lane_flash(mut q: Query<(&mut LaneFlashState, &mut BackgroundColor), With<DrumsHudKind>>) {
    for (mut state, mut bg) in &mut q {
        if state.intensity > 0 {
            state.intensity = state.intensity.saturating_sub(3);
            // Fade from 1.0 -> 0.0 over the 90-step counter (CActPerfDrumsLaneFlushD.cs:170).
            let alpha = state.intensity as f32 / 90.0;
            bg.0 = Color::srgba(1.0, 1.0, 0.6, alpha);
        } else {
            bg.0 = Color::srgba(1.0, 1.0, 1.0, 0.0);
        }
    }
}

fn tick_pad_flash(mut q: Query<(&mut PadFlashState, &mut BackgroundColor), With<DrumsHudKind>>) {
    for (mut state, mut bg) in &mut q {
        if state.brightness > 0 {
            state.brightness = state.brightness.saturating_sub(1);
            let intensity = state.brightness as f32 / 6.0;
            bg.0 = Color::srgba(0.4 + intensity, 0.4 + intensity, 0.5 + intensity, 0.6);
        } else {
            bg.0 = Color::srgba(0.4, 0.4, 0.5, 0.6);
        }
    }
}

fn tick_danger(
    time: Res<Time>,
    mut state: ResMut<DrumsDangerState>,
    mut q: Query<&mut Visibility, With<DrumsHudKind>>,
) {
    if state.active {
        state.counter = (state.counter + 7).min(0x7f);
        for mut v in &mut q {
            *v = Visibility::Visible;
        }
    } else {
        for mut v in &mut q {
            *v = Visibility::Hidden;
        }
    }
    let _ = time;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_position_matches_reference() {
        // CActPerfDrumsScore.cs:12-13
        assert_eq!(SCORE_X, 40.0);
        assert_eq!(SCORE_Y, 13.0);
        assert_eq!(SCORE_DIGITS, 7);
    }

    #[test]
    fn combo_position_matches_reference() {
        // CActPerfDrumsComboDGB.cs:44-45
        assert_eq!(COMBO_DGB_X, 1245.0);
        assert_eq!(COMBO_DGB_Y, 60.0);
        // CActPerfDrumsComboDGB.cs:55
        assert_eq!(COMBO_BOMB_X, 845.0);
        assert_eq!(COMBO_BOMB_Y, -130.0);
    }

    #[test]
    fn gauge_position_matches_reference() {
        // CActPerfDrumsGauge.cs:31, 44-46
        assert_eq!(GAUGE_X, 294.0);
        assert_eq!(GAUGE_Y_NORMAL, 626.0);
        assert_eq!(GAUGE_Y_REVERSE, 28.0);
        assert_eq!(GAUGE_FRAME_TOP_H, 47.0);
        assert_eq!(GAUGE_BAR_H, 31.0);
    }

    #[test]
    fn status_panel_position_matches_reference() {
        // CActPerfDrumsStatusPanel.cs:22-25
        assert_eq!(STATUS_PANEL_X, 22.0);
        assert_eq!(STATUS_PANEL_Y, 250.0);
    }

    #[test]
    fn lane_positions_match_reference() {
        // CActPerfDrumsLaneFlushD.cs:17-72 (Type A)
        assert_eq!(LANE_X[0], 298.0); // LC
        assert_eq!(LANE_W[0], 64.0);
        assert_eq!(LANE_X[2], 470.0); // SD
        assert_eq!(LANE_W[2], 54.0);
        assert_eq!(LANE_X[7], 748.0); // CY
        assert_eq!(LANE_W[7], 64.0);
    }

    #[test]
    fn pad_positions_match_reference() {
        // CActPerfDrumsPad.cs:16-103
        assert_eq!(PAD_X[0], 263.0); // LC
        assert_eq!(PAD_X[1], 336.0); // HH
        assert_eq!(PAD_X[2], 446.0); // SD
        assert_eq!(PAD_X[3], 565.0); // BD
        assert_eq!(PAD_X[9], 0x18c as f32); // LP
    }

    #[test]
    fn lane_flash_trigger() {
        let mut s = LaneFlashState::default();
        assert_eq!(s.intensity, 0);
        s.trigger(0.5);
        // (1.0 - 0.5) * 55.0 = 27
        assert_eq!(s.intensity, 27);
        s.trigger(1.0);
        // (1.0 - 1.0) * 55.0 = 0
        assert_eq!(s.intensity, 0);
        s.trigger(0.0);
        // (1.0 - 0.0) * 55.0 = 55
        assert_eq!(s.intensity, 55);
    }

    #[test]
    fn pad_flash_hit() {
        let mut s = PadFlashState::default();
        s.hit();
        assert_eq!(s.brightness, 6);
    }

    #[test]
    fn danger_state_default() {
        let s = DrumsDangerState::default();
        assert!(!s.active);
        assert_eq!(s.counter, 0);
    }

    #[test]
    fn judge_string_y_matches_reference() {
        // CActPerfDrumsJudgementString.cs:73-79
        assert_eq!(JUDGE_STRING_Y_FORWARD, 348.0);
        assert_eq!(JUDGE_STRING_Y_REVERSE, 583.0);
        assert_eq!(JUDGE_STRING_Y_TOP_REVERSE, 80.0);
        assert_eq!(JUDGE_STRING_VERT_DX, 32.0);
    }

    #[test]
    fn danger_overlay_fullscreen() {
        assert_eq!(DANGER_W, 1280.0);
        assert_eq!(DANGER_H, 720.0);
    }
}
