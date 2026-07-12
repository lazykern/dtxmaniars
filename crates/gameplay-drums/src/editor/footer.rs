//! Full-width Customize footer chrome: a 1-2 line description of the hovered
//! settings row on the left, and a static key legend on the right. Window-space,
//! tagged `EditorChrome`, spawned on surface open and despawned on close.

use bevy::prelude::*;

use super::EditorOpen;

/// The description of the settings row currently under the cursor. Updated by
/// `panel::update_hovered_desc` on hover; rendered into the footer's left text.
#[derive(Resource, Default)]
pub struct HoveredDesc(pub String);

/// Transient save-failure banner shown in the footer's description slot.
#[derive(Resource, Default)]
pub struct EditorSaveError {
    pub message: Option<String>,
    pub until_secs: f64,
}

impl EditorSaveError {
    pub fn set(&mut self, now: f64, message: impl Into<String>) {
        self.message = Some(message.into());
        self.until_secs = now + 4.0;
    }
}

#[derive(Component)]
struct FooterRoot;

#[derive(Component)]
struct FooterDescText;

/// Hint shown when no settings row is hovered.
const HINT: &str = "Hover a setting for details.";

/// Static key legend for the right-hand side of the footer.
const LEGEND: &str =
    "↑↓ row (↑ to tabs)   ←→ adjust / switch tab (Shift=coarse)   PgUp/Dn tab   Tab peek   Ctrl+S save   Esc close";

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<HoveredDesc>()
        .init_resource::<EditorSaveError>()
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
            GlobalZIndex(crate::ui_z::EDITOR_CHROME),
        ))
        .id();

    commands.entity(root).with_children(|p| {
        p.spawn((
            FooterDescText,
            Text::new(desc_text(&desc)),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_primary),
            Node {
                margin: UiRect::left(Val::Px(super::chrome::LEFT_PANEL_WIDTH)),
                ..default()
            },
        ));
        p.spawn((
            Text::new(LEGEND),
            dtx_ui::theme::Theme::font(12.0),
            TextColor(t.text_secondary),
        ));
    });
}

/// Footer text while a capture is armed; None outside capture.
pub fn capture_footer_text(state: &super::bindings_capture::CaptureState) -> Option<String> {
    use super::bindings_capture::CaptureState;
    match state {
        CaptureState::Idle => None,
        CaptureState::Keyboard(channel) => Some(format!(
            "Press a key for {} — Esc cancels",
            channel.short_name().unwrap_or("channel")
        )),
        CaptureState::Midi(channel) => Some(format!(
            "Hit a pad for {} — Esc cancels",
            channel.short_name().unwrap_or("channel")
        )),
        CaptureState::KeyArrived { owners, .. } | CaptureState::MidiArrived { owners, .. }
            if owners.is_empty() =>
        {
            Some("Enter confirm · Esc cancel".to_string())
        }
        CaptureState::KeyArrived { .. } | CaptureState::MidiArrived { .. } => Some(
            "Enter confirm · ←→ shared/move · Esc cancel".to_string(),
        ),
    }
}

/// Refresh the left-hand description when the hovered row, capture state or
/// save-error banner changes. Priority: armed capture > save-error banner >
/// hover description. The banner shows in `chrome::ERR` red until it expires,
/// then the normal description (and color) is restored.
fn update_footer_desc(
    desc: Res<HoveredDesc>,
    capture: Res<super::bindings_capture::CaptureState>,
    time: Res<Time>,
    theme: Res<dtx_ui::ThemeResource>,
    mut err: ResMut<EditorSaveError>,
    mut q: Query<(&mut Text, &mut TextColor), With<FooterDescText>>,
) {
    if !desc.is_changed() && !capture.is_changed() && err.message.is_none() {
        return;
    }
    if err.message.is_some() && time.elapsed_secs_f64() >= err.until_secs {
        err.message = None;
    }
    let (line, color) = if let Some(cap) = capture_footer_text(&capture) {
        (cap, theme.0.text_primary)
    } else if let Some(msg) = &err.message {
        (msg.clone(), super::chrome::ERR)
    } else {
        (desc_text(&desc), theme.0.text_primary)
    };
    for (mut text, mut text_color) in &mut q {
        if text.0 != line {
            text.0 = line.clone();
        }
        if text_color.0 != color {
            text_color.0 = color;
        }
    }
}

/// Despawn the footer when leaving Performance (song-ended-mid-edit path).
fn despawn_footer(mut commands: Commands, existing: Query<Entity, With<FooterRoot>>) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}
