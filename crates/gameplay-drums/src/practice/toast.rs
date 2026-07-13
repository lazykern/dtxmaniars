//! Practice-local toast queue (spec: generalize only when a second
//! consumer appears). Newest at the bottom, cap 4, ~1.5 s life.

use bevy::prelude::*;
use dtx_ui::theme::Theme;

pub const TOAST_CAP: usize = 4;
pub type ToastQueue = dtx_ui::NotificationQueue;

#[derive(Component)]
pub struct ToastRoot;

/// Rebuild the top-center toast column each frame (≤4 small texts, so a
/// rebuild is cheaper than diffing).
pub fn toast_ui(
    queue: Res<ToastQueue>,
    mut commands: Commands,
    roots: Query<Entity, With<ToastRoot>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    if queue.is_empty() {
        return;
    }
    let theme = Theme::default();
    commands
        .spawn((
            ToastRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(56.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            GlobalZIndex(crate::ui_z::TOAST),
        ))
        .with_children(|col| {
            for notification in queue.iter() {
                col.spawn((
                    Text::new(notification.message.clone()),
                    Theme::label_font(),
                    TextColor(theme.text_primary),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                        ..default()
                    },
                ));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_at_four_dropping_oldest() {
        let mut q = ToastQueue::default();
        for i in 0..6 {
            q.push(format!("t{i}"));
        }
        assert_eq!(q.len(), TOAST_CAP);
        let texts = q
            .iter()
            .map(|item| item.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(texts, ["t2", "t3", "t4", "t5"]);
    }

    #[test]
    fn tick_expires_old_toasts() {
        let mut q = ToastQueue::default();
        q.push("a");
        q.tick(3_000);
        q.push("b");
        q.tick(600); // a: 3.6s (dead), b: 0.6s (alive)
        assert_eq!(q.len(), 1);
        assert_eq!(q.iter().next().unwrap().message, "b");
    }
}
