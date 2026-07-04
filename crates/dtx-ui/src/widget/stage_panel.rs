//! GITADORA panel chrome: dark bordered boxes, yellow selected
//! variant with glow, and label+big-number badge rows.

use bevy::prelude::*;

use crate::theme::Theme;

fn selected_glow(theme: &Theme) -> BoxShadow {
    BoxShadow::new(
        theme.select_yellow.with_alpha(0.45),
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(2.0),
        Val::Px(14.0),
    )
}

fn no_shadow() -> BoxShadow {
    BoxShadow::new(Color::NONE, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0))
}

/// Base panel: #0d0d0dee fill, 1px #444 border. Carries a no-op
/// `BoxShadow` so `set_panel_selected` toggle queries match it.
pub fn panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node
        },
        BackgroundColor(theme.stage_panel_bg),
        BorderColor::all(theme.stage_panel_border),
        no_shadow(),
    )
}

/// Selected panel: yellow 1px border + glow. Selection is signaled
/// by color + glow only; border width matches the base panel.
pub fn selected_panel(theme: &Theme, node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node
        },
        BackgroundColor(theme.stage_panel_bg),
        BorderColor::all(theme.select_yellow),
        selected_glow(theme),
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
        *shadow = selected_glow(theme);
    } else {
        *border = BorderColor::all(theme.stage_panel_border);
        *shadow = no_shadow();
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
                Text::new(label),
                Theme::font(12.0),
                TextColor(theme.clear_green),
            ));
            row.spawn((
                BadgeValueText { decimals },
                Text::new(initial),
                Theme::font(26.0),
                TextColor(theme.text_primary),
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let values: Vec<String> = world
            .query::<(&BadgeValueText, &Text)>()
            .iter(world)
            .map(|(_, text)| text.0.clone())
            .collect();
        assert_eq!(values, vec!["0.00".to_string()]);

        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(
            texts.iter().any(|t| t == "SKILL BY SONG"),
            "label text missing, got {texts:?}"
        );
    }
}
