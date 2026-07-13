//! Persistent explanation of the chord currently held by practice wait mode.

use std::collections::HashSet;

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use game_shell::AppState;

use crate::judge::JudgedChips;
use crate::practice::wait::{WaitPhase, WaitSet, WaitState};
use crate::resources::ActiveChart;

#[derive(Component)]
struct WaitPrompt;

pub fn wait_prompt_text(set: &WaitSet, chart: &dtx_core::Chart, judged: &HashSet<usize>) -> String {
    let mut chips: Vec<_> = set
        .chips
        .iter()
        .filter_map(|&idx| {
            let channel = chart.chips.get(idx)?.channel;
            Some((
                crate::lane_map::lane_of(channel)?,
                idx,
                channel.short_name()?,
            ))
        })
        .collect();
    chips.sort_by_key(|(lane, _, _)| *lane);
    let parts: Vec<_> = chips
        .into_iter()
        .map(|(_, idx, label)| {
            if judged.contains(&idx) {
                format!("{label} ✓")
            } else {
                label.to_string()
            }
        })
        .collect();
    format!("WAIT — HIT TOGETHER: {}", parts.join(" + "))
}

fn spawn_prompt(mut commands: Commands) {
    let theme = Theme::default();
    commands.spawn((
        WaitPrompt,
        Text::new(""),
        Theme::label_font(),
        dtx_ui::SemanticText(dtx_ui::TypographyRole::Label),
        TextColor(theme.text_primary),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(88.0),
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
        Visibility::Hidden,
        GlobalZIndex(crate::ui_z::PRACTICE),
    ));
}

fn despawn_prompt(mut commands: Commands, prompts: Query<Entity, With<WaitPrompt>>) {
    for entity in &prompts {
        commands.entity(entity).despawn();
    }
}

fn update_prompt(
    state: Option<Res<WaitState>>,
    chart: Option<Res<ActiveChart>>,
    judged: Option<Res<JudgedChips>>,
    mut prompts: Query<(&mut Text, &mut Visibility), With<WaitPrompt>>,
) {
    let Ok((mut text, mut visibility)) = prompts.single_mut() else {
        return;
    };
    let (Some(state), Some(chart), Some(judged)) = (state, chart, judged) else {
        *visibility = Visibility::Hidden;
        return;
    };
    let WaitPhase::Halted(set) = &state.phase else {
        *visibility = Visibility::Hidden;
        return;
    };
    text.0 = wait_prompt_text(set, &chart.chart, &judged.0);
    *visibility = Visibility::Visible;
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Performance), spawn_prompt)
        .add_systems(OnExit(AppState::Performance), despawn_prompt)
        .add_systems(
            Update,
            update_prompt
                .run_if(in_state(AppState::Performance))
                .run_if(resource_exists::<crate::practice::PracticeSession>),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use dtx_core::{Chart, Chip, EChannel};
    use game_shell::AppState;

    #[test]
    fn wait_prompt_marks_hit_members_and_keeps_lane_order() {
        let chart = Chart {
            chips: vec![
                Chip::new(0, EChannel::FloorTom, 0.0),
                Chip::new(0, EChannel::Snare, 0.0),
            ],
            ..default()
        };
        let set = WaitSet {
            target_ms: 1_000,
            chips: vec![0, 1],
        };
        assert_eq!(
            wait_prompt_text(&set, &chart, &HashSet::from([1])),
            "WAIT — HIT TOGETHER: SD ✓ + FT"
        );
    }

    #[test]
    fn prompt_entity_spawns_before_practice_session_is_inserted() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<AppState>();
        plugin(&mut app);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Performance);
        app.update();

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<WaitPrompt>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }
}
