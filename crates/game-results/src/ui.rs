//! Results screen presentation: layout spawn/despawn + staggered reveal.

use bevy::prelude::*;
use dtx_scoring::Rank;
use dtx_ui::easing::EaseFunction;
use dtx_ui::motion::EnterChoreo;
use dtx_ui::{theme::Theme, ThemeResource};
use game_shell::{despawn_stage, SelectedDifficulty};
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};
use gameplay_drums::stage_end::LastStageOutcome;

use crate::input::ResultVerb;
use crate::{ResultEntity, SaveStatus};

/// Marks a revealed element: fade starts at `reveal_at_ms`, rises to
/// `target_alpha` (the element's authored alpha, e.g. 0.5 for
/// `text_secondary`) over `FADE_DURATION_MS` with OutQuint.
#[derive(Component)]
pub(crate) struct StatRow {
    pub reveal_at_ms: f32,
    pub target_alpha: f32,
}

/// Reveal progress for the whole screen. `done` flips on timeout or on the
/// first input (skip); while `!done` the input driver consumes everything.
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct RevealState {
    pub elapsed_ms: f32,
    pub total_ms: f32,
    pub done: bool,
}

impl RevealState {
    pub(crate) fn new(last_slot: f32) -> Self {
        Self {
            elapsed_ms: 0.0,
            total_ms: last_slot * STAGGER_MS + FADE_DURATION_MS,
            done: false,
        }
    }
}

/// Eased alpha for one element at `elapsed_ms`. Pure.
pub(crate) fn reveal_alpha(elapsed_ms: f32, reveal_at_ms: f32, target_alpha: f32) -> f32 {
    let since = elapsed_ms - reveal_at_ms;
    if since < 0.0 {
        return 0.0;
    }
    EaseFunction::OutQuint.ease((since / FADE_DURATION_MS).clamp(0.0, 1.0)) * target_alpha
}

pub(crate) const STAGGER_MS: f32 = 60.0;
pub(crate) const FADE_DURATION_MS: f32 = 350.0;

// Layout (spec §Layout).
const CARD_MAX_WIDTH: f32 = 900.0;
const CARD_PADDING: f32 = 48.0;
const LABEL_COL: f32 = 120.0;
const COUNT_COL: f32 = 80.0;

// Motion (spec §Motion): 24px upward slide, 350ms OutQuint per element.
const SLIDE_OFFSET: Vec2 = Vec2::new(0.0, 24.0);
const SLIDE_DURATION_MS: f32 = 350.0;

// Stagger slots × STAGGER_MS: header → rank → failed tag → judgments →
// divider → combo → score → save → verbs → legends. Last fade ends at
// 13 × 60 + 350 = 1130ms (~1.1s).
const SLOT_HEADER: f32 = 0.0;
const SLOT_RANK: f32 = 1.0;
const SLOT_FAILED: f32 = 2.0;
const SLOT_JUDGE_FIRST: f32 = 3.0; // five rows: slots 3..=7
const SLOT_TABLE_DIVIDER: f32 = 8.0;
const SLOT_COMBO: f32 = 9.0;
const SLOT_SCORE: f32 = 10.0;
const SLOT_SAVE: f32 = 11.0;
const SLOT_VERBS: f32 = 12.0;
const SLOT_LEGEND: f32 = 13.0;
pub(crate) const LAST_SLOT: f32 = SLOT_LEGEND;

/// Marks one verb-row label; `sync_verb_row` renders the cursor onto it.
#[derive(Component)]
pub(crate) struct VerbLabel(pub ResultVerb);

/// Verb label text with a width-stable selection prefix.
pub(crate) fn verb_text(verb: ResultVerb, selected: bool) -> String {
    let name = match verb {
        ResultVerb::Continue => "Continue",
        ResultVerb::Retry => "Retry",
        ResultVerb::Practice => "Practice",
    };
    if selected {
        format!("▸ {name}")
    } else {
        format!("  {name}")
    }
}

/// Text element bundle with fade (to the color's own alpha) + slide at `slot`.
fn reveal_text(
    text: impl Into<String>,
    font: TextFont,
    color: Color,
    slot: f32,
) -> (Text, TextFont, TextColor, StatRow, EnterChoreo, UiTransform) {
    (
        Text::new(text),
        font,
        TextColor(color.with_alpha(0.0)),
        StatRow {
            reveal_at_ms: slot * STAGGER_MS,
            target_alpha: color.alpha(),
        },
        EnterChoreo::slide(SLIDE_OFFSET, slot * STAGGER_MS, SLIDE_DURATION_MS),
        UiTransform::default(),
    )
}

/// 1px horizontal rule fading to a quarter-alpha `text_secondary`.
fn divider(parent: &mut ChildSpawnerCommands, t: &Theme, slot: f32) {
    let color = t.text_secondary.with_alpha(0.25);
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(color.with_alpha(0.0)),
        StatRow {
            reveal_at_ms: slot * STAGGER_MS,
            target_alpha: color.alpha(),
        },
        EnterChoreo::slide(SLIDE_OFFSET, slot * STAGGER_MS, SLIDE_DURATION_MS),
        UiTransform::default(),
    ));
}

/// Rank → theme color: SS/S gold, A green, B blue, C purple, D/E red,
/// Unknown secondary (spec §Left panel).
pub(crate) fn rank_color(rank: Rank, theme: &Theme) -> Color {
    match rank {
        Rank::SS | Rank::S => theme.judgment_perfect,
        Rank::A => theme.judgment_great,
        Rank::B => theme.judgment_good,
        Rank::C => theme.judgment_ok,
        Rank::D | Rank::E => theme.judgment_miss,
        Rank::Unknown => theme.text_secondary,
    }
}

/// Rank headline text: `Display` string, except Unknown renders `--`.
pub(crate) fn rank_label(rank: Rank) -> String {
    if rank == Rank::Unknown {
        "--".into()
    } else {
        rank.to_string()
    }
}

/// `912340` → `"912,340"` (comma thousands separator).
pub(crate) fn format_thousands(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + usize::from(v < 0));
    if v < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub(crate) fn pct(count: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        count as f32 / total as f32 * 100.0
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_result(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    difficulty: Res<SelectedDifficulty>,
    outcome: Option<Res<LastStageOutcome>>,
    midi: Option<Res<game_shell::MidiConnected>>,
    status: Res<SaveStatus>,
) {
    commands.insert_resource(RevealState::new(LAST_SLOT));
    commands.insert_resource(ResultVerb::default());

    let t = theme.0;
    let title = chart
        .metadata()
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let artist = chart
        .metadata()
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let dlevel = chart
        .metadata()
        .dlevel
        .map(|v| format!("{:.2}", dtx_core::display_dlevel(v)))
        .unwrap_or_else(|| "--".into());
    let total = scoring.total_notes;
    let rank = crate::result_rank(&counts, combo.max, total);
    let failed = outcome.is_some_and(|o| !o.cleared);
    let midi_connected = midi.is_some_and(|m| m.0);

    commands
        .spawn((
            ResultEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.bg_bottom),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(CARD_MAX_WIDTH),
                    padding: UiRect::all(Val::Px(CARD_PADDING)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
                BackgroundColor(t.panel_bg),
            ))
            .with_children(|card| {
                spawn_header(card, &t, &title, &artist, &dlevel, difficulty.0);
                divider(card, &t, SLOT_HEADER);
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(32.0),
                    ..default()
                })
                .with_children(|body| {
                    spawn_rank_panel(body, &t, rank, failed);
                    spawn_stats_panel(body, &t, &counts, total, combo.max, score.0, *status);
                });
                divider(card, &t, SLOT_VERBS);
                spawn_verb_row(card, &t);
                spawn_legends(card, &t, midi_connected);
            });
        });
}

fn spawn_header(
    card: &mut ChildSpawnerCommands,
    t: &Theme,
    title: &str,
    artist: &str,
    dlevel: &str,
    difficulty: u8,
) {
    card.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        ..default()
    })
    .with_children(|head| {
        head.spawn(reveal_text(
            title,
            Theme::font(28.0),
            t.text_primary,
            SLOT_HEADER,
        ));
        head.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|meta| {
            meta.spawn(reveal_text(
                format!("{artist} ·"),
                Theme::font(16.0),
                t.text_secondary,
                SLOT_HEADER,
            ));
            meta.spawn(reveal_text(
                format!("Lv {dlevel}"),
                Theme::font(16.0),
                t.difficulty_color(difficulty),
                SLOT_HEADER,
            ));
        });
    });
}

fn spawn_rank_panel(body: &mut ChildSpawnerCommands, t: &Theme, rank: Rank, failed: bool) {
    body.spawn(Node {
        width: Val::Percent(40.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|left| {
        left.spawn(reveal_text(
            rank_label(rank),
            Theme::font(160.0),
            rank_color(rank, t),
            SLOT_RANK,
        ));
        if failed {
            left.spawn(reveal_text(
                "STAGE FAILED",
                Theme::font(16.0),
                t.judgment_miss,
                SLOT_FAILED,
            ));
        }
    });
}

fn spawn_stats_panel(
    body: &mut ChildSpawnerCommands,
    t: &Theme,
    counts: &JudgmentCounts,
    total: u32,
    max_combo: u32,
    score: i64,
    status: SaveStatus,
) {
    body.spawn(Node {
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|right| {
        let rows = [
            ("PERFECT", counts.perfect),
            ("GREAT", counts.great),
            ("GOOD", counts.good),
            ("POOR", counts.ok),
            ("MISS", counts.miss),
        ];
        for (i, (label, count)) in rows.into_iter().enumerate() {
            judgment_row(right, t, label, count, total, SLOT_JUDGE_FIRST + i as f32);
        }
        divider(right, t, SLOT_TABLE_DIVIDER);
        value_row(
            right,
            t,
            "MAX COMBO",
            &max_combo.to_string(),
            Theme::font(18.0),
            SLOT_COMBO,
        );
        value_row(
            right,
            t,
            "SCORE",
            &format_thousands(score),
            Theme::font(28.0),
            SLOT_SCORE,
        );
        match status {
            SaveStatus::Saved => {
                right.spawn(reveal_text(
                    "saved ✓",
                    Theme::font(14.0),
                    t.clear_green,
                    SLOT_SAVE,
                ));
            }
            SaveStatus::Failed => {
                right.spawn(reveal_text(
                    "save failed — score kept this session only",
                    Theme::font(14.0),
                    t.judgment_miss,
                    SLOT_SAVE,
                ));
            }
            SaveStatus::Practice => {}
        }
    });
}

fn judgment_row(
    parent: &mut ChildSpawnerCommands,
    t: &Theme,
    label: &str,
    count: u32,
    total: u32,
    slot: f32,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(LABEL_COL),
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(reveal_text(
                    label,
                    Theme::font(18.0),
                    t.judgment_color(label),
                    slot,
                ));
            });
            row.spawn(Node {
                width: Val::Px(COUNT_COL),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(reveal_text(
                    count.to_string(),
                    Theme::font(18.0),
                    t.text_primary,
                    slot,
                ));
            });
            row.spawn(reveal_text(
                format!("{:.1}%", pct(count, total)),
                Theme::font(14.0),
                t.text_secondary,
                slot,
            ));
        });
}

fn value_row(
    parent: &mut ChildSpawnerCommands,
    t: &Theme,
    label: &str,
    value: &str,
    value_font: TextFont,
    slot: f32,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Baseline,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(LABEL_COL),
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(reveal_text(
                    label,
                    Theme::font(14.0),
                    t.text_secondary,
                    slot,
                ));
            });
            row.spawn(reveal_text(value, value_font, t.text_primary, slot));
        });
}

fn spawn_verb_row(card: &mut ChildSpawnerCommands, t: &Theme) {
    card.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::Center,
        column_gap: Val::Px(32.0),
        ..default()
    })
    .with_children(|row| {
        for verb in [
            ResultVerb::Continue,
            ResultVerb::Retry,
            ResultVerb::Practice,
        ] {
            let selected = verb == ResultVerb::default();
            let color = if selected { t.accent } else { t.text_secondary };
            row.spawn((
                VerbLabel(verb),
                reveal_text(
                    verb_text(verb, selected),
                    Theme::font(20.0),
                    color,
                    SLOT_VERBS,
                ),
            ));
        }
    });
}

fn spawn_legends(card: &mut ChildSpawnerCommands, t: &Theme, midi_connected: bool) {
    card.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        },
        EnterChoreo::slide(SLIDE_OFFSET, SLOT_LEGEND * STAGGER_MS, SLIDE_DURATION_MS),
        UiTransform::default(),
    ))
    .with_children(|legends| {
        if midi_connected {
            dtx_ui::widget::nav_legend::spawn_nav_legend(
                legends,
                t,
                &[
                    ("HH/CY", "move"),
                    ("BD", "select"),
                    ("SD", "continue"),
                    ("FT", "practice"),
                ],
            );
        }
        legends.spawn(reveal_text(
            "←/→ move · Enter select · R retry · Esc continue",
            Theme::font(12.0),
            t.text_secondary,
            SLOT_LEGEND,
        ));
    });
}

/// Renders the verb cursor: selected = accent + `▸ ` prefix, others =
/// secondary + two-space prefix (row width stays stable). While the reveal
/// runs, the fade's current alpha is preserved.
pub(crate) fn sync_verb_row(
    theme: Res<ThemeResource>,
    cursor: Res<ResultVerb>,
    reveal: Res<RevealState>,
    mut q: Query<(&VerbLabel, &mut Text, &mut TextColor)>,
) {
    let t = theme.0;
    for (label, mut text, mut color) in &mut q {
        let selected = label.0 == *cursor;
        let next = verb_text(label.0, selected);
        if text.0 != next {
            text.0 = next;
        }
        let target = if selected { t.accent } else { t.text_secondary };
        color.0 = if reveal.done {
            target
        } else {
            target.with_alpha(color.0.alpha())
        };
    }
}

pub(crate) fn animate_staggered_reveal(
    time: Res<Time>,
    mut reveal: ResMut<RevealState>,
    mut q: Query<(
        &StatRow,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
    )>,
) {
    if reveal.done {
        return;
    }
    reveal.elapsed_ms += time.delta_secs() * 1000.0;
    for (stat, text, bg) in &mut q {
        let alpha = reveal_alpha(reveal.elapsed_ms, stat.reveal_at_ms, stat.target_alpha);
        if let Some(mut c) = text {
            c.0 = c.0.with_alpha(alpha);
        } else if let Some(mut b) = bg {
            // Node-only elements (dividers). Text entities also carry an
            // auto-inserted default BackgroundColor (transparent black);
            // fading that would paint a black chip behind every glyph.
            b.0 = b.0.with_alpha(alpha);
        }
    }
    if reveal.elapsed_ms >= reveal.total_ms {
        reveal.done = true;
    }
}

pub(crate) fn despawn_result(commands: Commands, query: Query<Entity, With<ResultEntity>>) {
    despawn_stage::<ResultEntity>(commands, query);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_world() -> World {
        let mut world = World::new();
        world.insert_resource(ThemeResource::default());
        world.insert_resource(Score(912_340));
        world.insert_resource(Combo {
            current: 0,
            max: 214,
        });
        world.insert_resource(JudgmentCounts {
            perfect: 412,
            great: 61,
            good: 12,
            ok: 6,
            miss: 9,
        });
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: None,
        });
        world.insert_resource(DrumScoring {
            total_notes: 500,
            ..Default::default()
        });
        world.insert_resource(game_shell::SelectedDifficulty(2));
        world.insert_resource(SaveStatus::Saved);
        world
    }

    fn all_texts(world: &mut World) -> Vec<String> {
        let mut q = world.query::<&Text>();
        q.iter(world).map(|t| t.0.clone()).collect()
    }

    #[test]
    fn spawn_result_builds_verbs_columns_and_score() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = spawn_world();
        world
            .run_system_once(spawn_result)
            .expect("spawn_result runs");

        let mut verb_q = world.query::<&VerbLabel>();
        assert_eq!(
            verb_q.iter(&world).count(),
            3,
            "Continue / Retry / Practice"
        );

        let texts = all_texts(&mut world);
        assert!(
            texts.iter().any(|s| s == "912,340"),
            "score separated: {texts:?}"
        );
        assert!(texts.iter().any(|s| s == "saved ✓"), "save line kept");
        // 412/500 perfect, 61 great, combo 214 → XG rate 80.73 → S.
        assert!(texts.iter().any(|s| s == "S"), "rank letter: {texts:?}");
        assert!(
            !texts.iter().any(|s| s == "STAGE FAILED"),
            "no failed tag without LastStageOutcome"
        );
        // Column layout, not space padding: count is its own text node.
        assert!(texts.iter().any(|s| s == "412"));
        assert!(texts.iter().any(|s| s == "82.4%"));
    }

    #[test]
    fn spawn_result_colors_judgment_labels() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = spawn_world();
        world
            .run_system_once(spawn_result)
            .expect("spawn_result runs");
        let t = Theme::default();
        let mut q = world.query::<(&Text, &TextColor)>();
        let (_, color) = q
            .iter(&world)
            .find(|(text, _)| text.0 == "PERFECT")
            .expect("PERFECT label exists");
        assert_eq!(color.0, t.judgment_perfect.with_alpha(0.0));
    }

    #[test]
    fn spawn_result_failed_tag_and_unknown_rank() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = spawn_world();
        world.insert_resource(LastStageOutcome { cleared: false });
        world.insert_resource(DrumScoring {
            total_notes: 0,
            ..Default::default()
        });
        world
            .run_system_once(spawn_result)
            .expect("spawn_result runs");
        let texts = all_texts(&mut world);
        assert!(texts.iter().any(|s| s == "STAGE FAILED"));
        assert!(texts.iter().any(|s| s == "--"), "Unknown rank renders --");
        assert!(texts.iter().any(|s| s == "0.0%"), "zero total → 0.0%");
    }

    #[test]
    fn pct_zero_total_is_zero() {
        assert_eq!(pct(1, 0), 0.0);
    }

    #[test]
    fn rank_color_total_mapping() {
        let t = Theme::default();
        assert_eq!(rank_color(Rank::SS, &t), t.judgment_perfect);
        assert_eq!(rank_color(Rank::S, &t), t.judgment_perfect);
        assert_eq!(rank_color(Rank::A, &t), t.judgment_great);
        assert_eq!(rank_color(Rank::B, &t), t.judgment_good);
        assert_eq!(rank_color(Rank::C, &t), t.judgment_ok);
        assert_eq!(rank_color(Rank::D, &t), t.judgment_miss);
        assert_eq!(rank_color(Rank::E, &t), t.judgment_miss);
        assert_eq!(rank_color(Rank::Unknown, &t), t.text_secondary);
    }

    #[test]
    fn rank_label_unknown_is_dashes() {
        assert_eq!(rank_label(Rank::Unknown), "--");
        assert_eq!(rank_label(Rank::SS), "SS");
        assert_eq!(rank_label(Rank::A), "A");
    }

    #[test]
    fn format_thousands_boundaries() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1_000), "1,000");
        assert_eq!(format_thousands(912_340), "912,340");
        assert_eq!(format_thousands(i64::MAX), "9,223,372,036,854,775,807");
        assert_eq!(format_thousands(-1_000), "-1,000");
    }

    #[test]
    fn reveal_alpha_is_zero_before_slot() {
        assert_eq!(reveal_alpha(59.0, 60.0, 1.0), 0.0);
    }

    #[test]
    fn reveal_alpha_is_outquint_front_loaded() {
        // OutQuint at t=0.5 is 1 - 0.5^5 = 0.96875 — well past linear.
        let a = reveal_alpha(FADE_DURATION_MS * 0.5, 0.0, 1.0);
        assert!(a > 0.9, "expected front-loaded ease, got {a}");
    }

    #[test]
    fn reveal_alpha_caps_at_target() {
        assert_eq!(reveal_alpha(10_000.0, 0.0, 0.5), 0.5);
    }

    #[test]
    fn reveal_state_new_totals_last_slot_plus_fade() {
        let s = RevealState::new(13.0);
        assert_eq!(s.total_ms, 13.0 * STAGGER_MS + FADE_DURATION_MS);
        assert!(!s.done);
    }

    #[test]
    fn sync_verb_row_renders_cursor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(ThemeResource::default());
        world.insert_resource(ResultVerb::Retry);
        world.insert_resource(RevealState {
            elapsed_ms: 2_000.0,
            total_ms: 1_130.0,
            done: true,
        });
        let t = Theme::default();
        let retry = world
            .spawn((
                VerbLabel(ResultVerb::Retry),
                Text::new(verb_text(ResultVerb::Retry, false)),
                TextColor(t.text_secondary),
            ))
            .id();
        let cont = world
            .spawn((
                VerbLabel(ResultVerb::Continue),
                Text::new(verb_text(ResultVerb::Continue, true)),
                TextColor(t.accent),
            ))
            .id();
        world.run_system_once(sync_verb_row).expect("sync runs");
        assert_eq!(world.get::<Text>(retry).expect("text").0, "▸ Retry");
        assert_eq!(world.get::<TextColor>(retry).expect("color").0, t.accent);
        assert_eq!(world.get::<Text>(cont).expect("text").0, "  Continue");
        assert_eq!(
            world.get::<TextColor>(cont).expect("color").0,
            t.text_secondary
        );
    }

    #[test]
    fn animate_marks_done_at_timeout() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(RevealState {
            elapsed_ms: 2_000.0,
            total_ms: 1_130.0,
            done: false,
        });
        world
            .run_system_once(animate_staggered_reveal)
            .expect("system runs");
        assert!(world.resource::<RevealState>().done);
    }
}
