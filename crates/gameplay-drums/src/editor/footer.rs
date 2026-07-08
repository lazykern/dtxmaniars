//! Full-width Customize footer chrome: a 1-2 line description of the hovered
//! settings row on the left, and a static key legend on the right. Window-space,
//! tagged `EditorChrome`, spawned on surface open and despawned on close.

use bevy::prelude::*;

use super::EditorOpen;

/// The description of the settings row currently under the cursor. Updated by
/// `panel::update_hovered_desc` on hover; rendered into the footer's left text.
#[derive(Resource, Default)]
pub struct HoveredDesc(pub String);

#[derive(Component)]
struct FooterRoot;

#[derive(Component)]
struct FooterDescText;

/// Hint shown when no settings row is hovered.
const HINT: &str = "Hover a setting for details.";

/// Static key legend for the right-hand side of the footer.
const LEGEND: &str = "↑↓ row   ←→ adjust   Tab peek   Ctrl+S save   Esc close";

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<HoveredDesc>()
        .add_systems(
            Update,
            (
                spawn_footer_on_open.run_if(resource_changed::<EditorOpen>),
                update_footer_desc.run_if(super::editor_open),
            )
                .run_if(in_state(game_shell::AppState::Performance)),
        )
        .add_systems(OnExit(game_shell::AppState::Performance), despawn_footer);
}

/// Left-hand text: the hovered row's description, or a hint if none.
fn desc_text(desc: &HoveredDesc) -> String {
    if desc.0.is_empty() {
        HINT.to_string()
    } else {
        desc.0.clone()
    }
}

/// Rebuild the footer when the editor opens/closes.
fn spawn_footer_on_open(
    mut commands: Commands,
    open: Res<EditorOpen>,
    desc: Res<HoveredDesc>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<FooterRoot>>,
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
            FooterRoot,
            super::picking::EditorChrome,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
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
            FooterDescText,
            Text::new(desc_text(&desc)),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
        ));
        p.spawn((
            Text::new(LEGEND),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_secondary),
        ));
    });
}

/// Refresh the left-hand description when the hovered row changes.
fn update_footer_desc(desc: Res<HoveredDesc>, mut q: Query<&mut Text, With<FooterDescText>>) {
    if !desc.is_changed() {
        return;
    }
    for mut text in &mut q {
        text.0 = desc_text(&desc);
    }
}

/// Despawn the footer when leaving Performance (song-ended-mid-edit path).
fn despawn_footer(mut commands: Commands, existing: Query<Entity, With<FooterRoot>>) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}
