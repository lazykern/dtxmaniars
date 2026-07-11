//! Shared spawn helpers for Customize panel content: cards with uppercase
//! micro-labels, source chips, channel color dots. Visual tokens live in
//! `chrome.rs`; this module owns only structure.
//!
//! Wired into the Controls tab (Task 5); Task 8 wires the Lanes panel.

use bevy::prelude::*;

use super::chrome;

/// A column card: `CARD_BG` fill, hairline `CARD_BORDER`, uppercase micro-label
/// title. Returns the body entity — spawn rows into it, not the card itself.
pub fn spawn_card(parent: &mut ChildSpawnerCommands, title: &str) -> Entity {
    let mut body = None;
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                margin: UiRect::bottom(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(chrome::CARD_BG),
            BorderColor::all(chrome::CARD_BORDER),
        ))
        .with_children(|card| {
            card.spawn((
                Text::new(title.to_uppercase()),
                dtx_ui::theme::Theme::font(9.0),
                TextColor(chrome::TEXT_MUTED),
            ));
            body = Some(
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .id(),
            );
        });
    body.expect("card body spawned above")
}

/// Dark source chip. `shared` appends the "⧉" marker to `label`. `bundle` is
/// attached to the chip node so callers can hang a remove-button marker (or
/// any other component) off it.
pub fn spawn_chip(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    shared: bool,
    bundle: impl Bundle,
) -> Entity {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                margin: UiRect::right(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(chrome::CHIP_BG),
            BorderColor::all(chrome::CHIP_BORDER),
            bundle,
        ))
        .with_children(|chip| {
            chip.spawn((
                Text::new(chip_label(label, shared)),
                dtx_ui::theme::Theme::font(10.0),
                TextColor(chrome::TEXT_MUTED),
            ));
        })
        .id()
}

/// Small square channel color swatch — a dot, not the whole row's tint.
pub fn spawn_channel_dot(parent: &mut ChildSpawnerCommands, color: Color) {
    parent.spawn((
        Node {
            width: Val::Px(9.0),
            height: Val::Px(9.0),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            margin: UiRect::right(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(color),
    ));
}

/// Pure part of `spawn_chip`'s text: shared sources get a "⧉" marker.
fn chip_label(label: &str, shared: bool) -> String {
    if shared {
        format!("{label} ⧉")
    } else {
        label.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::chip_label;

    #[test]
    fn shared_chip_gets_marker() {
        assert_eq!(chip_label("N42", false), "N42");
        assert_eq!(chip_label("N42", true), "N42 ⧉");
    }
}
