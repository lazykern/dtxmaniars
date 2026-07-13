//! Pad nav legend: a bottom bar naming the drum verb for each nav action,
//! e.g. `HH up · CY down · BD adjust · SD back`. Shown only when a MIDI device
//! is connected; the verbs change with the active nav level.

use bevy::prelude::*;

use crate::theme::Theme;

/// Marks a legend bar so surfaces can despawn and rebuild it on level change.
#[derive(Component)]
pub struct NavLegend;

/// One `(pad, verb)` cell, e.g. `("HH", "up")`.
pub type LegendItem<'a> = (&'a str, &'a str);

/// Spawn a legend bar as the last child of the current parent.
pub fn spawn_nav_legend(parent: &mut ChildSpawnerCommands, theme: &Theme, items: &[LegendItem]) {
    parent
        .spawn((
            NavLegend,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(7.0)),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
        ))
        .with_children(|bar| {
            for (pad, verb) in items {
                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|cell| {
                    cell.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.18, 0.24)),
                        children![(
                            Text::new(*pad),
                            Theme::font(10.0),
                            crate::SemanticText(crate::TypographyRole::Hint),
                            TextColor(theme.text_primary),
                        )],
                    ));
                    cell.spawn((
                        Text::new(*verb),
                        Theme::font(10.0),
                        crate::SemanticText(crate::TypographyRole::Hint),
                        TextColor(theme.text_secondary),
                    ));
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_items_are_pad_verb_pairs() {
        let items: &[LegendItem] = &[("HH", "up"), ("CY", "down")];
        assert_eq!(items[0].0, "HH");
        assert_eq!(items[1].1, "down");
    }
}
