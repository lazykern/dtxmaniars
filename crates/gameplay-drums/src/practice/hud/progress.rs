use bevy::prelude::*;

pub(super) fn spawn_progress(
    parent: &mut ChildSpawnerCommands,
    theme: &dtx_ui::Theme,
    session: &crate::practice::PracticeSession,
    timeline: &crate::timeline::ChipTimeline,
) {
    super::setup::spawn_text(
        parent,
        "Completed attempts",
        dtx_ui::TypographyRole::Heading,
        theme.text_primary,
    );
    let summary = progress_summary(session, timeline.end_ms);
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

fn progress_summary(session: &crate::practice::PracticeSession, chart_end_ms: i64) -> String {
    let rows = progress_rows(session, chart_end_ms);
    if rows.is_empty() {
        "No completed attempts yet".to_owned()
    } else {
        rows.join("\n")
    }
}

pub fn progress_rows(session: &crate::practice::PracticeSession, span_end_ms: i64) -> Vec<String> {
    let (span_start_ms, span_end_ms) = session
        .transport
        .loop_region
        .map_or((0, span_end_ms), |region| (region.start_ms, region.end_ms));
    session
        .attempt_history
        .iter()
        .filter(|attempt| attempt.start_ms == span_start_ms && attempt.end_ms == span_end_ms)
        .map(|attempt| {
            let metric = if attempt.trainer_mode == crate::practice::PracticeTrainerMode::Wait {
                format!("Latest flow: {:.1}%", attempt.flow_pct)
            } else {
                format!("Latest: {:.1}%", attempt.accuracy_pct)
            };
            format!(
                "{metric} at {:.2}×, timing {:+.0} ms",
                attempt.tempo, attempt.mean_error_ms
            )
        })
        .collect()
}

pub(super) fn refresh_progress_copy(
    session: Res<crate::practice::PracticeSession>,
    timeline: Res<crate::timeline::ChipTimeline>,
    mut summary: Query<&mut Text, (With<ProgressSummaryText>, Without<ProgressDiagnosisText>)>,
    mut diagnosis: Query<&mut Text, (With<ProgressDiagnosisText>, Without<ProgressSummaryText>)>,
) {
    for mut text in &mut summary {
        text.0 = progress_summary(&session, timeline.end_ms);
    }
    for mut text in &mut diagnosis {
        text.0 = crate::practice::diagnosis::diagnosis_text(&session.lane_diag);
    }
}
