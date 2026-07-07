//! Practice-local toast queue (spec: generalize only when a second
//! consumer appears). Newest at the bottom, cap 4, ~1.5 s life.

use bevy::prelude::*;
use dtx_ui::theme::Theme;

pub const TOAST_CAP: usize = 4;
pub const TOAST_SECS: f32 = 1.5;

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub age: f32,
}

#[derive(Resource, Debug, Default)]
pub struct ToastQueue(pub Vec<Toast>);

impl ToastQueue {
    pub fn push(&mut self, text: impl Into<String>) {
        self.0.push(Toast {
            text: text.into(),
            age: 0.0,
        });
        while self.0.len() > TOAST_CAP {
            self.0.remove(0);
        }
    }

    /// Age all toasts by `dt` seconds and drop expired ones.
    pub fn tick(&mut self, dt: f32) {
        for t in &mut self.0 {
            t.age += dt;
        }
        self.0.retain(|t| t.age < TOAST_SECS);
    }
}

#[derive(Component)]
pub struct ToastRoot;

/// Rebuild the top-center toast column each frame (≤4 small texts, so a
/// rebuild is cheaper than diffing).
pub fn toast_ui(
    time: Res<Time>,
    mut queue: ResMut<ToastQueue>,
    mut commands: Commands,
    roots: Query<Entity, With<ToastRoot>>,
) {
    queue.tick(time.delta_secs());
    for e in &roots {
        commands.entity(e).despawn();
    }
    if queue.0.is_empty() {
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
            GlobalZIndex(1100),
        ))
        .with_children(|col| {
            for t in &queue.0 {
                col.spawn((
                    Text::new(t.text.clone()),
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
        assert_eq!(q.0.len(), TOAST_CAP);
        assert_eq!(q.0[0].text, "t2", "oldest dropped first");
        assert_eq!(q.0[3].text, "t5");
    }

    #[test]
    fn tick_expires_old_toasts() {
        let mut q = ToastQueue::default();
        q.push("a");
        q.tick(1.0);
        q.push("b");
        q.tick(0.6); // a: 1.6s (dead), b: 0.6s (alive)
        assert_eq!(q.0.len(), 1);
        assert_eq!(q.0[0].text, "b");
    }
}
