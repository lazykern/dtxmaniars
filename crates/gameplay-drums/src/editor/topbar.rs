//! Full-width Customize topbar chrome: `CUSTOMIZE ▸ song · BPM` on the left,
//! entry hints + an autoplay/loops chip on the right. Window-space, tagged
//! `EditorChrome` (hidden during hold-Tab peek), spawned on surface open.

use bevy::prelude::*;

use super::EditorOpen;
use crate::resources::ActiveChart;

#[derive(Component)]
struct TopbarRoot;

#[derive(Component)]
struct TopbarTitle;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_topbar_on_open.run_if(resource_changed::<EditorOpen>),
            update_topbar_title.run_if(super::editor_open),
        )
            .run_if(in_state(game_shell::AppState::Performance)),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_topbar);
}

/// Compose the left-hand title line from the active chart's metadata,
/// mirroring hud.rs's title/bpm access + fallbacks.
fn title_text(chart: &ActiveChart) -> String {
    let title = chart
        .chart
        .metadata
        .title
        .as_deref()
        .unwrap_or("— no chart —");
    let bpm = chart.chart.metadata.bpm.unwrap_or(0.0);
    format!("CUSTOMIZE ▸ {title} · BPM {bpm:.0}")
}

/// Rebuild the topbar when the editor opens/closes.
fn spawn_topbar_on_open(
    mut commands: Commands,
    open: Res<EditorOpen>,
    chart: Res<ActiveChart>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<TopbarRoot>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    let t = theme.0;
    let root = commands
        .spawn((
            TopbarRoot,
            super::picking::EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(40.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(16.0)),
                column_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(t.panel_bg),
            GlobalZIndex(2000),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        p.spawn((
            TopbarTitle,
            Text::new(title_text(&chart)),
            dtx_ui::theme::Theme::font(14.0),
            TextColor(t.text_primary),
        ));

        p.spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new("F1 @ Gameplay   F2 @ Widgets"),
                dtx_ui::theme::Theme::font(12.0),
                TextColor(t.text_secondary),
            ));
            r.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(t.stage_panel_border),
                children![(
                    Text::new("AUTOPLAY · CHART LOOPS"),
                    dtx_ui::theme::Theme::font(11.0),
                    TextColor(t.text_secondary),
                )],
            ));
        });
    });
}

/// Refresh the title line when the active chart changes.
fn update_topbar_title(chart: Res<ActiveChart>, mut q: Query<&mut Text, With<TopbarTitle>>) {
    if !chart.is_changed() {
        return;
    }
    for mut text in &mut q {
        text.0 = title_text(&chart);
    }
}

/// Despawn the topbar when leaving Performance (song-ended-mid-edit path).
fn despawn_topbar(mut commands: Commands, existing: Query<Entity, With<TopbarRoot>>) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}
