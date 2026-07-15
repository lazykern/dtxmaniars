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
    let summary = session.attempt_history.last().map_or_else(
        || "No completed attempts yet".to_owned(),
        |attempt| {
            format!(
                "Latest: {:.1}% at {:.2}×, timing {:+.0} ms",
                attempt.accuracy_pct, attempt.tempo, attempt.mean_error_ms
            )
        },
    );
    super::setup::spawn_text(
        parent,
        summary,
        dtx_ui::TypographyRole::Body,
        theme.text_primary,
    );
    super::setup::spawn_text(
        parent,
        crate::practice::diagnosis::diagnosis_text(&session.lane_diag),
        dtx_ui::TypographyRole::Body,
        theme.text_secondary,
    );
}
