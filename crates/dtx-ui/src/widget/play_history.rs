//! Song-select play-history panel: header + fixed row slots.
//!
//! Screen systems fill [`PlayHistoryData`] on selection change and a
//! render system writes rows into the `HistoryRowText` entities
//! (same data-resource pattern as `difficulty_grid`).

use bevy::prelude::*;

use crate::theme::Theme;

/// Maximum rows shown in the panel.
pub const HISTORY_MAX_ROWS: usize = 8;

/// One display row: a single past play of the selected chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoryRow {
    /// Rank label ("SS".."E", "--" for unknown).
    pub rank: String,
    /// Score value.
    pub score: u32,
    /// Weighted achievement percentage (0..100).
    pub achievement_pct: f32,
    /// Pre-formatted UTC play time, `YYYY-MM-DD HH:MM`.
    pub played_at: String,
}

/// Rows for the selected chart, best score first. Filled by the
/// song-select screen, rendered by `render_play_history`.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PlayHistoryData {
    /// At most [`HISTORY_MAX_ROWS`] rows.
    pub rows: Vec<HistoryRow>,
}

/// Row line text entity (slot index `0..HISTORY_MAX_ROWS`).
#[derive(Component, Debug, Clone, Copy)]
pub struct HistoryRowText(pub usize);

/// "NO PLAYS" empty-state label.
#[derive(Component, Debug, Clone, Copy)]
pub struct HistoryEmptyText;

/// Spawn the panel contents: header, empty-state label, and
/// `HISTORY_MAX_ROWS` blank row slots.
pub fn spawn_play_history(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Text::new("PLAY HISTORY"),
                Theme::font(12.0),
                TextColor(theme.clear_green),
            ));
            col.spawn((
                HistoryEmptyText,
                Text::new("NO PLAYS"),
                Theme::font(12.0),
                TextColor(theme.text_secondary),
            ));
            for i in 0..HISTORY_MAX_ROWS {
                col.spawn((
                    HistoryRowText(i),
                    Text::new(""),
                    Theme::font(12.0),
                    TextColor(theme.text_primary),
                ));
            }
        });
}

/// Render one row as a single line: `S   982340   95.2%  2026-07-10 14:35`.
pub fn history_row_line(row: &HistoryRow) -> String {
    format!(
        "{:<2} {:>7}  {:>5.1}%  {}",
        row.rank, row.score, row.achievement_pct, row.played_at
    )
}

/// Format unix seconds as a UTC `YYYY-MM-DD HH:MM` play time string.
///
/// Uses the days-to-civil algorithm (Howard Hinnant) — the workspace
/// has no date dependency and this panel only needs minute precision.
pub fn format_unix_played_at(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let seconds_today = secs % 86_400;
    let hour = seconds_today / 3_600;
    let minute = (seconds_today % 3_600) / 60;
    format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_epoch_start() {
        assert_eq!(format_unix_played_at(0), "1970-01-01 00:00");
    }

    #[test]
    fn date_day_boundary() {
        assert_eq!(format_unix_played_at(86_399), "1970-01-01 23:59");
        assert_eq!(format_unix_played_at(86_400), "1970-01-02 00:00");
    }

    #[test]
    fn date_modern() {
        // 2026-07-11 00:00:00 UTC
        assert_eq!(format_unix_played_at(1_783_728_000), "2026-07-11 00:00");
    }

    #[test]
    fn date_leap_day() {
        // 2024-02-29 00:00:00 UTC
        assert_eq!(format_unix_played_at(1_709_164_800), "2024-02-29 00:00");
    }

    #[test]
    fn row_line_layout() {
        let row = HistoryRow {
            rank: "S".into(),
            score: 982_340,
            achievement_pct: 95.234,
            played_at: "2026-07-10 14:35".into(),
        };
        assert_eq!(
            history_row_line(&row),
            "S   982340   95.2%  2026-07-10 14:35"
        );
    }

    #[test]
    fn spawns_header_rows_and_empty_label() {
        let mut app = bevy::app::App::new();
        let theme = Theme::default();
        let world = app.world_mut();
        {
            let mut commands = world.commands();
            commands.spawn(Node::default()).with_children(|p| {
                spawn_play_history(p, &theme);
            });
        }
        world.flush();

        let row_count = world.query::<&HistoryRowText>().iter(world).count();
        assert_eq!(row_count, HISTORY_MAX_ROWS);

        let empty_count = world.query::<&HistoryEmptyText>().iter(world).count();
        assert_eq!(empty_count, 1);

        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|t| t.0.clone())
            .collect();
        assert!(texts.iter().any(|t| t == "PLAY HISTORY"));
        assert!(texts.iter().any(|t| t == "NO PLAYS"));
    }
}
