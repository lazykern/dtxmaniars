//! CStageChangeSkin — skin selection.
//!
//! Merged from `change_skin.rs` (M3 stub) + `change_skin_full.rs` (state +
//! reload). Single plugin, no double-spawn.
//!
//! Reference: `references/DTXmaniaNX-BocuD/DTXMania/Stage/09.ChangeSkin/CStageChangeSkin.cs` (95 LOC)

use bevy::prelude::*;

use game_shell::fade::start_fade;
use game_shell::{AppState, despawn_stage};

/// Stage ID (CStageChangeSkin.cs:11) — `EStage.ChangeSkin_9`.
pub const CHANGE_SKIN_STAGE_ID: u32 = 9;

/// Skin state — current subfolder + reload tracking.
#[derive(Resource, Debug, Default, Clone)]
pub struct ChangeSkinState {
    /// Current skin subfolder name (e.g. "Default").
    pub current_skin: String,
    /// True if a reload is currently in progress.
    pub reloading: bool,
    /// Number of reloads performed (for diagnostic logging).
    pub reload_count: u32,
}

impl ChangeSkinState {
    pub fn new() -> Self {
        Self {
            current_skin: "Default".into(),
            reloading: false,
            reload_count: 0,
        }
    }

    /// Trigger skin reload (CStageChangeSkin.cs:78-86 — tChangeSkinMain).
    pub fn reload(&mut self, new_skin: &str) {
        self.reloading = true;
        self.current_skin = new_skin.to_string();
        self.reload_count += 1;
        self.reloading = false;
    }
}

#[derive(Component)]
pub struct ChangeSkinEntity;

pub fn plugin(app: &mut App) {
    app.init_resource::<ChangeSkinState>()
        .add_systems(
            OnEnter(AppState::ChangeSkin),
            (spawn_change_skin, start_fade),
        )
        .add_systems(
            OnExit(AppState::ChangeSkin),
            despawn_stage::<ChangeSkinEntity>,
        )
        .add_systems(
            Update,
            change_skin_input.run_if(in_state(AppState::ChangeSkin)),
        );
}

fn spawn_change_skin(mut commands: Commands) {
    commands.spawn((
        ChangeSkinEntity,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.05)),
        children![(
            Text::new("Change Skin — M6+"),
            TextFont {
                font_size: FontSize::Px(28.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.6)),
        )],
    ));
}

fn change_skin_input(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_id_matches_e_stage_change_skin_9() {
        // CStageChangeSkin.cs:11
        assert_eq!(CHANGE_SKIN_STAGE_ID, 9);
    }

    #[test]
    fn default_skin_is_default() {
        let s = ChangeSkinState::new();
        assert_eq!(s.current_skin, "Default");
        assert!(!s.reloading);
    }

    #[test]
    fn default_reload_count_zero() {
        let s = ChangeSkinState::new();
        assert_eq!(s.reload_count, 0);
    }

    #[test]
    fn reload_updates_skin_name() {
        let mut s = ChangeSkinState::new();
        s.reload("MySkin");
        assert_eq!(s.current_skin, "MySkin");
        assert_eq!(s.reload_count, 1);
    }

    #[test]
    fn reload_increments_count() {
        let mut s = ChangeSkinState::new();
        s.reload("Skin1");
        s.reload("Skin2");
        s.reload("Skin3");
        assert_eq!(s.reload_count, 3);
        assert_eq!(s.current_skin, "Skin3");
    }

    #[test]
    fn reload_resets_reloading_flag() {
        let mut s = ChangeSkinState::new();
        s.reload("X");
        assert!(!s.reloading);
    }
}
