//! GITADORA panel chrome: dark bordered boxes, yellow selected
//! variant with glow, and label+big-number badge rows.

use bevy::prelude::*;

use crate::theme::Theme;

/// Base panel: #0d0d0dee fill, 1px #444 border.
pub fn panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node
        },
        BackgroundColor(theme.stage_panel_bg),
        BorderColor::all(theme.stage_panel_border),
    )
}

/// Selected panel: yellow 2px border + glow.
pub fn selected_panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(2.0)),
            ..node
        },
        BackgroundColor(theme.stage_panel_bg),
        BorderColor::all(theme.select_yellow),
        BoxShadow::new(
            theme.select_yellow.with_alpha(0.45),
            Val::Px(0.0),
            Val::Px(0.0),
            Val::Px(2.0),
            Val::Px(14.0),
        ),
    )
}

/// Apply/remove selection chrome on an existing panel entity.
pub fn set_panel_selected(
    theme: &Theme,
    selected: bool,
    border: &mut BorderColor,
    shadow: &mut BoxShadow,
) {
    if selected {
        *border = BorderColor::all(theme.select_yellow);
        *shadow = BoxShadow::new(
            theme.select_yellow.with_alpha(0.45),
            Val::Px(0.0),
            Val::Px(0.0),
            Val::Px(2.0),
            Val::Px(14.0),
        );
    } else {
        *border = BorderColor::all(theme.stage_panel_border);
        *shadow = BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0));
    }
}

/// Marker + value channel for a big-number badge ("SKILL 145.14",
/// "BPM 157"). Text updated by screen systems.
#[derive(Component, Debug, Clone)]
pub struct BadgeValueText {
    /// Format with 2 decimals when true (skill), integer otherwise.
    pub decimals: bool,
}

/// Spawn "LABEL   <big number>" row inside a panel.
pub fn spawn_badge_row(
    parent: &mut ChildSpawnerCommands,
    theme: &Theme,
    label: &str,
    initial: &str,
    decimals: bool,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label.to_string()),
                Theme::font(12.0),
                TextColor(theme.clear_green),
            ));
            row.spawn((
                BadgeValueText { decimals },
                Text::new(initial.to_string()),
                Theme::font(26.0),
                TextColor(theme.text_primary),
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_value_marker_carries_format() {
        assert!(BadgeValueText { decimals: true }.decimals);
    }

    #[test]
    fn badge_row_spawns_label_and_value() {
        let mut app = bevy::app::App::new();
        let theme = Theme::default();
        let world = app.world_mut();
        {
            let mut commands = world.commands();
            commands.spawn(Node::default()).with_children(|p| {
                spawn_badge_row(p, &theme, "SKILL BY SONG", "0.00", true);
            });
        }
        world.flush();
        let badges = world.query::<&BadgeValueText>().iter(world).count();
        assert_eq!(badges, 1);
    }
}
