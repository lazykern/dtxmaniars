//! Quick-tier status chip (top-right): rate, ramp step, loop bars, last accuracy.

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use game_shell::AppState;

use super::timeline_ui::bar_number;
use crate::practice::session::PracticeSession;
use crate::timeline::ChipTimeline;

#[derive(Component)]
pub struct StatusChip;

/// Pure: chip contents from session state. `bar_ms` from `ChipTimeline`.
pub fn chip_text(session: &PracticeSession, bar_ms: &[i64]) -> String {
    let mut parts = vec![format!("{:.2}×", session.rate)];
    if session.ramp.armed {
        let (cur, total) =
            crate::practice::ramp::ramp_step_index(&session.ramp_config, session.rate);
        parts.push(format!("RAMP {cur}/{total}"));
    }
    if let Some(r) = session.loop_region.filter(|r| r.end_ms != i64::MAX) {
        parts.push(format!(
            "loop {}–{}",
            bar_number(bar_ms, r.start_ms),
            bar_number(bar_ms, r.end_ms)
        ));
    }
    if let Some(last) = session.attempt_history.last() {
        parts.push(format!("{:.0}%", last.accuracy_pct));
    }
    parts.join(" · ")
}

pub fn spawn_chip(mut commands: Commands, session: Res<PracticeSession>, timeline: Res<ChipTimeline>) {
    let theme = Theme::default();
    commands.spawn((
        StatusChip,
        Text::new(chip_text(&session, &timeline.bar_ms)),
        Theme::label_font(),
        TextColor(theme.text_primary),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(12.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        GlobalZIndex(900),
    ));
}

pub fn despawn_chip(mut commands: Commands, chips: Query<Entity, With<StatusChip>>) {
    for e in &chips {
        commands.entity(e).despawn();
    }
}

pub fn update_chip(
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut chips: Query<&mut Text, With<StatusChip>>,
) {
    if let Ok(mut t) = chips.single_mut() {
        t.0 = chip_text(&session, &timeline.bar_ms);
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(AppState::Performance),
        spawn_chip
            .after(crate::timeline::build_chip_timeline)
            .run_if(resource_exists::<PracticeSession>),
    )
    .add_systems(OnExit(AppState::Performance), despawn_chip)
    .add_systems(
        Update,
        update_chip
            .run_if(in_state(AppState::Performance))
            .run_if(in_state(game_shell::PauseState::Running))
            .run_if(resource_exists::<PracticeSession>),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::practice::session::{AttemptRecord, LoopRegion};

    #[test]
    fn chip_text_shows_rate_loop_and_last_accuracy() {
        let mut s = PracticeSession::default();
        let bar_ms = vec![0, 2_000, 4_000, 6_000, 8_000];
        assert_eq!(chip_text(&s, &bar_ms), "1.00×");

        s.rate = 0.85;
        s.loop_region = Some(LoopRegion {
            start_ms: 2_000,
            end_ms: 6_000,
        });
        s.attempt_history.push(AttemptRecord {
            start_ms: 2_000,
            end_ms: 6_000,
            rate: 0.85,
            counts: Default::default(),
            max_combo: 12,
            accuracy_pct: 94.2,
            mean_error_ms: -3.0,
        });
        assert_eq!(chip_text(&s, &bar_ms), "0.85× · loop 2–4 · 94%");
    }

    #[test]
    fn chip_text_shows_ramp_segment_when_armed() {
        let mut s = PracticeSession::default();
        s.rate = 0.85;
        s.ramp.armed = true;
        let bar_ms = vec![0, 2_000];
        assert_eq!(chip_text(&s, &bar_ms), "0.85× · RAMP 3/6");
    }
}
