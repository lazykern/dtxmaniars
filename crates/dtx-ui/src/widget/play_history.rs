//! Song-select play-history panel: header + fixed row slots.
//!
//! Screen systems fill [`PlayHistoryData`] on selection change and a
//! render system writes rows into the `HistoryRowText` entities
//! (same data-resource pattern as `difficulty_grid`).

use bevy::prelude::*;
use chrono::{DateTime, Local};

use crate::theme::Theme;

/// Maximum rows shown in the panel.
pub const HISTORY_MAX_ROWS: usize = 8;

/// One display row: a single past play of the selected chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistoryRow {
    /// Rank label ("SS".."E", "--" for unknown).
    pub rank: String,
    /// Score value.
    pub score: i64,
    /// Weighted achievement percentage (0..100).
    pub achievement_pct: f32,
    /// Pre-formatted local play time, `YYYY-MM-DD HH:MM`.
    pub played_at: String,
    /// Played with No Fail: shown as an `NF` tag, never a record.
    pub no_fail: bool,
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
/// A No Fail play carries a trailing `NF` tag.
pub fn history_row_line(row: &HistoryRow) -> String {
    format!(
        "{:<2} {:>7}  {:>5.1}%  {}{}",
        row.rank,
        row.score,
        row.achievement_pct,
        row.played_at,
        if row.no_fail { "  NF" } else { "" }
    )
}

/// Format unix seconds as a local `YYYY-MM-DD HH:MM` play time string.
pub fn format_unix_played_at(secs: u64) -> String {
    i64::try_from(secs)
        .ok()
        .and_then(|secs| DateTime::from_timestamp(secs, 0))
        .map(|time| {
            time.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_uses_local_timezone() {
        let secs = 1_783_728_000;
        let expected = DateTime::from_timestamp(secs, 0)
            .expect("valid timestamp")
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M")
            .to_string();
        assert_eq!(format_unix_played_at(secs as u64), expected);
    }

    #[test]
    fn invalid_timestamp_is_blank() {
        assert!(format_unix_played_at(u64::MAX).is_empty());
    }

    #[test]
    fn row_line_layout() {
        let row = HistoryRow {
            rank: "S".into(),
            score: 982_340,
            achievement_pct: 95.234,
            played_at: "2026-07-10 14:35".into(),
            no_fail: false,
        };
        assert_eq!(
            history_row_line(&row),
            "S   982340   95.2%  2026-07-10 14:35"
        );
    }

    #[test]
    fn no_fail_row_is_tagged() {
        let row = HistoryRow {
            rank: "S".into(),
            score: 982_340,
            achievement_pct: 95.234,
            played_at: "2026-07-10 14:35".into(),
            no_fail: true,
        };
        assert_eq!(
            history_row_line(&row),
            "S   982340   95.2%  2026-07-10 14:35  NF"
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
