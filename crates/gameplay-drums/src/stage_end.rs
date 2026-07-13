//! Stage Clear / Stage Failed banners between Performance and Result.
//!
//! Mechanics: a survived performance clears; a drained gauge (`StageGauge::failed`,
//! fail threshold −0.1 per `CActPerfCommonGauge.cs`) fails. Both paths funnel
//! into `AppState::Result` after a short banner. UX (banner + auto-advance) is
//! redesigned per ADR-0014; loosely mirrors `dtxpt` clear/fail handling.
//!
//! Ref: `CStagePerfDrumsScreen.cs:270-279` (clear/fail branch),
//! `CActPerfStageFailure.cs` (failure banner).

use bevy::prelude::*;
use dtx_ui::theme::Theme;
use game_shell::{request_transition, AppState, TransitionRequest};

use crate::gauge::StageGauge;
use crate::orchestrator::DrumsStageCompletion;
use crate::resources::EffectivePlaybackRate;

/// How long the clear/fail banner stays up before auto-advancing to Result.
const BANNER_MS: f32 = 1600.0;

/// Root marker for the clear/fail banner UI.
#[derive(Component)]
struct StageBanner;

/// Countdown until the banner auto-advances to Result.
#[derive(Resource, Default)]
struct BannerTimer(f32);

/// Last performance outcome, consumed by result persistence.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LastStageOutcome {
    pub cleared: bool,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BannerTimer>()
        .init_resource::<LastStageOutcome>()
        .add_systems(OnEnter(AppState::Performance), reset_stage_outcome)
        .add_systems(
            Update,
            detect_stage_failure.run_if(in_state(AppState::Performance)),
        )
        .add_systems(OnEnter(AppState::StageClear), spawn_clear_banner)
        .add_systems(OnEnter(AppState::StageFailed), spawn_failed_banner)
        .add_systems(OnExit(AppState::StageClear), despawn_banner)
        .add_systems(OnExit(AppState::StageFailed), despawn_banner)
        .add_systems(
            Update,
            advance_banner
                .run_if(in_state(AppState::StageClear).or_else(in_state(AppState::StageFailed))),
        );
}

fn reset_stage_outcome(mut outcome: ResMut<LastStageOutcome>) {
    outcome.cleared = false;
}

/// While playing, a drained gauge sends us to the failure banner immediately.
/// Practice runs freeze the gauge every FixedUpdate tick
/// (`practice::freeze_gauge_in_practice`), but this system reads it in
/// `Update`, ungated — a damage system could set `failed` within the same
/// tick before the freeze runs. Gate on practice explicitly so that race
/// can never fail a practice run.
fn detect_stage_failure(
    gauge: Res<StageGauge>,
    mut completion: ResMut<DrumsStageCompletion>,
    mut requests: MessageWriter<TransitionRequest>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    session: Res<game_shell::EditorSession>,
    rate: Res<EffectivePlaybackRate>,
    no_fail: Option<Res<crate::resources::NoFailEnabled>>,
    mut completed_run: ResMut<game_shell::CompletedRunContext>,
) {
    if practice.is_some() {
        return;
    }
    if session.0 {
        return;
    }
    if completion.end_requested {
        return;
    }
    let no_fail = no_fail.is_some_and(|modifier| modifier.0);
    if no_fail {
        return;
    }
    if gauge.failed {
        completion.end_requested = true;
        completion.gauge_failed = true;
        info!("DrumsStage: gauge failed, routing to StageFailed");
        *completed_run = game_shell::CompletedRunContext::normal(
            rate.value,
            game_shell::RunModifiers { no_fail },
        );
        request_transition(&mut requests, AppState::StageFailed);
    }
}

fn spawn_clear_banner(
    commands: Commands,
    timer: ResMut<BannerTimer>,
    mut outcome: ResMut<LastStageOutcome>,
) {
    outcome.cleared = true;
    let theme = Theme::default();
    spawn_banner(commands, timer, "STAGE CLEAR", theme.accent);
}

fn spawn_failed_banner(
    commands: Commands,
    timer: ResMut<BannerTimer>,
    mut outcome: ResMut<LastStageOutcome>,
) {
    outcome.cleared = false;
    let theme = Theme::default();
    spawn_banner(commands, timer, "STAGE FAILED", theme.judgment_miss);
}

fn spawn_banner(mut commands: Commands, mut timer: ResMut<BannerTimer>, label: &str, color: Color) {
    timer.0 = BANNER_MS;
    let theme = Theme::default();
    commands
        .spawn((
            StageBanner,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            GlobalZIndex(crate::ui_z::STAGE_END),
        ))
        .with_children(|root| {
            root.spawn((Text::new(label), Theme::title_font(), TextColor(color)));
            root.spawn((
                Text::new("Press Enter to continue"),
                Theme::label_font(),
                TextColor(theme.text_secondary),
            ));
        });
}

fn despawn_banner(mut commands: Commands, banners: Query<Entity, With<StageBanner>>) {
    for entity in &banners {
        commands.entity(entity).despawn();
    }
}

fn advance_banner(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut timer: ResMut<BannerTimer>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    timer.0 -= time.delta_secs() * 1000.0;
    let skip = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
    if timer.0 <= 0.0 || skip {
        request_transition(&mut requests, AppState::Result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_failure_snapshots_modified_rate() {
        let mut app = App::new();
        app.init_resource::<StageGauge>()
            .init_resource::<DrumsStageCompletion>()
            .init_resource::<game_shell::EditorSession>()
            .init_resource::<EffectivePlaybackRate>()
            .init_resource::<crate::resources::NoFailEnabled>()
            .init_resource::<game_shell::CompletedRunContext>()
            .add_message::<TransitionRequest>()
            .add_systems(Update, detect_stage_failure);
        app.world_mut().resource_mut::<StageGauge>().failed = true;
        *app.world_mut().resource_mut::<EffectivePlaybackRate>() =
            EffectivePlaybackRate::normal(0.75);

        app.update();

        let run = app.world().resource::<game_shell::CompletedRunContext>();
        assert_eq!(run.kind, game_shell::RunKind::Normal);
        assert!((run.playback_rate - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn no_fail_prevents_gauge_failure_transition() {
        let mut app = App::new();
        app.init_resource::<StageGauge>()
            .init_resource::<DrumsStageCompletion>()
            .init_resource::<game_shell::EditorSession>()
            .init_resource::<EffectivePlaybackRate>()
            .insert_resource(crate::resources::NoFailEnabled(true))
            .init_resource::<game_shell::CompletedRunContext>()
            .add_message::<TransitionRequest>()
            .add_systems(Update, detect_stage_failure);
        app.world_mut().resource_mut::<StageGauge>().failed = true;

        app.update();

        assert!(!app.world().resource::<DrumsStageCompletion>().end_requested);
        assert_eq!(
            app.world()
                .resource::<game_shell::CompletedRunContext>()
                .kind,
            game_shell::RunKind::Practice
        );
    }
}
