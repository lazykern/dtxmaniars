use bevy::prelude::*;

pub(super) fn spawn_progress(
    parent: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    session: &crate::practice::PracticeSession,
) {
    super::setup::spawn_text(
        parent,
        "Completed attempts",
        dtx_ui::TypographyRole::Heading,
        theme.text_primary,
    );
    let summary = progress_summary(session);
    let summary_entity = super::setup::spawn_text(
        parent,
        summary,
        dtx_ui::TypographyRole::Body,
        theme.text_primary,
    );
    parent
        .commands()
        .entity(summary_entity)
        .insert(ProgressSummaryText);
    let diagnosis_entity = super::setup::spawn_text(
        parent,
        crate::practice::diagnosis::diagnosis_text(&session.lane_diag),
        dtx_ui::TypographyRole::Body,
        theme.text_secondary,
    );
    parent
        .commands()
        .entity(diagnosis_entity)
        .insert(ProgressDiagnosisText);
}

#[derive(Component)]
pub(super) struct ProgressSummaryText;

#[derive(Component)]
pub(super) struct ProgressDiagnosisText;

fn progress_summary(session: &crate::practice::PracticeSession) -> String {
    session.attempt_history.last().map_or_else(
        || "No completed attempts yet".to_owned(),
        |attempt| {
            format!(
                "Latest: {:.1}% at {:.2}×, timing {:+.0} ms",
                attempt.accuracy_pct, attempt.tempo, attempt.mean_error_ms
            )
        },
    )
}

pub(super) fn refresh_progress_copy(
    session: Res<crate::practice::PracticeSession>,
    mut summary: Query<&mut Text, (With<ProgressSummaryText>, Without<ProgressDiagnosisText>)>,
    mut diagnosis: Query<&mut Text, (With<ProgressDiagnosisText>, Without<ProgressSummaryText>)>,
) {
    for mut text in &mut summary {
        text.0 = progress_summary(&session);
    }
    for mut text in &mut diagnosis {
        text.0 = crate::practice::diagnosis::diagnosis_text(&session.lane_diag);
    }
}
