//! Difficulty grid: one slot per chart in the selected folder —
//! colored label bar, big level number, achievement + rank when
//! played, dimmed "no play" otherwise. Selected slot gets yellow
//! border + glow (applied by the song-select screen system).

use bevy::prelude::*;

use crate::theme::Theme;

pub const GRID_MAX_SLOTS: usize = 5; // BASIC..EDIT

/// Slot state pushed by the screen each selection change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DifficultySlot {
    pub present: bool,
    pub label: String,
    /// Display level, e.g. 7.80 (dlevel / 10.0).
    pub level: Option<f32>,
    /// Achievement percent 0..100 when a score exists.
    pub achievement: Option<f32>,
    pub rank: Option<String>,
}

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct DifficultyGridData {
    pub slots: [DifficultySlot; GRID_MAX_SLOTS],
    pub selected: usize,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotPanel(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotLabel(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotLevel(pub usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct DifficultySlotScore(pub usize);

/// Spawn the grid slots (all 5; absent slots render empty and dim).
pub fn spawn_difficulty_grid(parent: &mut ChildSpawnerCommands, theme: &Theme) {
    for i in (0..GRID_MAX_SLOTS).rev() {
        // MASTER on top like GITADORA (highest index first)
        parent
            .spawn((
                DifficultySlotPanel(i),
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(theme.stage_panel_bg),
                BorderColor::all(theme.stage_panel_border),
                BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)),
            ))
            .with_children(|slot| {
                slot.spawn((
                    DifficultySlotLabel(i),
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(theme.difficulty_color(i as u8)),
                    Text::new(""),
                    Theme::font(11.0),
                    TextColor(theme.text_primary),
                ));
                slot.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        DifficultySlotScore(i),
                        Text::new(""),
                        Theme::font(11.0),
                        TextColor(theme.text_secondary),
                    ));
                    row.spawn((
                        DifficultySlotLevel(i),
                        Text::new("--"),
                        Theme::font(28.0),
                        TextColor(theme.text_primary),
                    ));
                });
            });
    }
}

/// Format helpers used by the update system (kept pure for tests).
pub fn level_text(level: Option<f32>) -> String {
    match level {
        Some(v) => format!("{v:.2}"),
        None => "--".into(),
    }
}

pub fn score_text(slot: &DifficultySlot) -> String {
    if !slot.present {
        return String::new();
    }
    match (slot.achievement, slot.rank.as_deref()) {
        (Some(a), Some(r)) => format!("{r}  {a:.2}%"),
        (Some(a), None) => format!("{a:.2}%"),
        _ => "— no play".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_text_formats_two_decimals() {
        assert_eq!(level_text(Some(7.8)), "7.80");
        assert_eq!(level_text(None), "--");
    }

    #[test]
    fn score_text_states() {
        let mut s = DifficultySlot::default();
        assert_eq!(score_text(&s), "");
        s.present = true;
        assert_eq!(score_text(&s), "— no play");
        s.achievement = Some(93.04);
        s.rank = Some("S".into());
        assert_eq!(score_text(&s), "S  93.04%");
        s.rank = None;
        assert_eq!(score_text(&s), "93.04%");
    }
}
