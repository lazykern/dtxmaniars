//! Results screen presentation: layout spawn/despawn + staggered reveal.

use bevy::prelude::*;
use dtx_scoring::Rank;
use dtx_ui::{theme::Theme, ThemeResource};
use game_shell::despawn_stage;
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};

use crate::{ResultEntity, SaveStatus};

#[derive(Component)]
struct ResultPanel;

/// Marks a stat row for staggered reveal.
#[derive(Component)]
pub(crate) struct StatRow {
    pub reveal_at_ms: f32,
}

#[derive(Resource)]
pub(crate) struct ResultReveal {
    pub elapsed_ms: f32,
}

const STAGGER_MS: f32 = 120.0;
const FADE_DURATION_MS: f32 = 300.0;

/// Rank → theme color: SS/S gold, A green, B blue, C purple, D/E red,
/// Unknown secondary (spec §Left panel).
#[allow(dead_code)] // wired into spawn_result in Task 6
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
#[allow(dead_code)] // wired into spawn_result in Task 6
pub(crate) fn rank_label(rank: Rank) -> String {
    if rank == Rank::Unknown {
        "--".into()
    } else {
        rank.to_string()
    }
}

/// `912340` → `"912,340"` (comma thousands separator).
#[allow(dead_code)] // wired into spawn_result in Task 6
pub(crate) fn format_thousands(v: u64) -> String {
    let digits = v.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
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
    midi: Option<Res<game_shell::MidiConnected>>,
    status: Res<SaveStatus>,
) {
    commands.insert_resource(ResultReveal { elapsed_ms: 0.0 });

    let title = chart
        .chart
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let artist = chart
        .chart
        .metadata
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let difficulty = chart
        .chart
        .metadata
        .dlevel
        .map(|v| format!("{:.2}", dtx_core::display_dlevel(v)))
        .unwrap_or_else(|| "--".into());
    let total = scoring.total_notes;
    let rank = crate::result_rank(&counts, combo.max, total);
    let t = theme.0;

    let stat_rows: Vec<(String, f32)> = vec![
        (title.to_string(), 0.0),
        (format!("{artist}  Lv.{difficulty}"), STAGGER_MS),
        (String::new(), STAGGER_MS * 2.0),
        (format!("Score     {}", score.0), STAGGER_MS * 3.0),
        (format!("Max Combo {}", combo.max), STAGGER_MS * 4.0),
        (format!("Rank      {rank}"), STAGGER_MS * 5.0),
        (String::new(), STAGGER_MS * 6.0),
        (
            format!(
                "Perfect   {} ({:.1}%)",
                counts.perfect,
                pct(counts.perfect, total)
            ),
            STAGGER_MS * 7.0,
        ),
        (
            format!(
                "Great     {} ({:.1}%)",
                counts.great,
                pct(counts.great, total)
            ),
            STAGGER_MS * 8.0,
        ),
        (
            format!(
                "Good      {} ({:.1}%)",
                counts.good,
                pct(counts.good, total)
            ),
            STAGGER_MS * 9.0,
        ),
        (
            format!("Poor      {} ({:.1}%)", counts.ok, pct(counts.ok, total)),
            STAGGER_MS * 10.0,
        ),
        (
            format!(
                "Miss      {} ({:.1}%)",
                counts.miss,
                pct(counts.miss, total)
            ),
            STAGGER_MS * 11.0,
        ),
        (format!("Total     {total}"), STAGGER_MS * 12.0),
        (String::new(), STAGGER_MS * 13.0),
        ("ESC / ENTER → Song Select".to_string(), STAGGER_MS * 14.0),
    ];

    let panel = commands
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
        .id();

    let inner = commands
        .spawn((
            ResultPanel,
            Node {
                padding: UiRect::all(Val::Px(48.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                min_width: Val::Px(400.0),
                ..default()
            },
            BackgroundColor(t.panel_bg),
        ))
        .id();

    commands.entity(panel).add_child(inner);

    for (text, delay) in stat_rows {
        if text.is_empty() {
            let spacer = commands
                .spawn((
                    StatRow {
                        reveal_at_ms: delay,
                    },
                    Node {
                        height: Val::Px(16.0),
                        ..default()
                    },
                ))
                .id();
            commands.entity(inner).add_child(spacer);
        } else {
            let row = commands
                .spawn((
                    StatRow {
                        reveal_at_ms: delay,
                    },
                    Text::new(text),
                    Theme::label_font(),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                ))
                .id();
            commands.entity(inner).add_child(row);
        }
    }

    let (label, color) = match *status {
        SaveStatus::Saved => ("saved ✓", t.clear_green),
        SaveStatus::Failed => ("save failed — score kept this session only", t.judgment_miss),
        SaveStatus::Practice => ("", Color::NONE),
    };
    if !label.is_empty() {
        let row = commands
            .spawn((
                StatRow {
                    reveal_at_ms: STAGGER_MS * 15.0,
                },
                Text::new(label),
                Theme::label_font(),
                TextColor(color.with_alpha(0.0)),
            ))
            .id();
        commands.entity(inner).add_child(row);
    }

    if midi.is_some_and(|m| m.0) {
        commands.entity(inner).with_children(|p| {
            dtx_ui::widget::nav_legend::spawn_nav_legend(p, &t, &[("BD", "continue")]);
        });
    }
}

pub(crate) fn animate_staggered_reveal(
    time: Res<Time>,
    mut reveal: ResMut<ResultReveal>,
    mut q: Query<(&StatRow, &mut TextColor)>,
) {
    reveal.elapsed_ms += time.delta_secs() * 1000.0;
    for (stat, mut color) in &mut q {
        let since = reveal.elapsed_ms - stat.reveal_at_ms;
        if since < 0.0 {
            continue;
        }
        let alpha = (since / FADE_DURATION_MS).clamp(0.0, 1.0);
        color.0 = color.0.with_alpha(alpha);
    }
}

pub(crate) fn despawn_result(commands: Commands, query: Query<Entity, With<ResultEntity>>) {
    despawn_stage::<ResultEntity>(commands, query);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(format_thousands(u64::MAX), "18,446,744,073,709,551,615");
    }
}
