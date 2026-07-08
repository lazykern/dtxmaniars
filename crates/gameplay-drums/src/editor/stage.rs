//! Customize stage-transform presets: map ActiveTab → target StageRect.

use crate::stage_rect::{StageRect, StageTarget};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shell::CustomizeTab;

/// Tabs-only rail width (editor/ui.rs).
const RAIL_WIDTH: f32 = 132.0;
/// Left content panel width, docked flush right of the rail (editor/panel.rs).
const LEFT_PANEL_WIDTH: f32 = 348.0;
/// Right inspector panel width (editor/panel.rs).
const PANEL_WIDTH: f32 = 240.0;
const TOP_MARGIN: f32 = 24.0;
/// Breathing room between the chrome and the shrunk stage.
const GAP: f32 = 16.0;
/// The left content panel is present on ALL tabs now, so the left chrome is
/// always rail + left panel; the inspector reserves the right chrome.
const LEFT_CHROME: f32 = RAIL_WIDTH + LEFT_PANEL_WIDTH;

/// Preset rect for a tab given the window size. Both groups Fit identically: the
/// screen shrinks into the gap between the left chrome (rail + left panel) and
/// the right chrome (inspector), centered with a `GAP` margin on each side.
pub fn preset_rect(_tab: CustomizeTab, window: Vec2) -> StageRect {
    StageRect {
        origin: Vec2::new(LEFT_CHROME + GAP, TOP_MARGIN),
        size: Vec2::new(
            (window.x - LEFT_CHROME - PANEL_WIDTH - 2.0 * GAP).max(1.0),
            (window.y - 2.0 * TOP_MARGIN).max(1.0),
        ),
    }
}

/// Thin screen-bounds outline drawn at the current `StageRect` while the
/// surface is open and not peeking, so the user sees the true (shrunk) screen
/// edges for WYSIWYG anchor placement. Window-space, positioned directly from
/// `StageRect` (no self-transform). Not tagged `EditorChrome`: it owns its own
/// visibility so peek does not double-touch it.
#[derive(Component)]
struct StageOutline;

/// Just below chrome (`GlobalZIndex(2000)`) so the outline reads under the rail.
const OUTLINE_Z: i32 = 1900;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            peek_stage.run_if(super::editor_open),
            spawn_outline_on_open.run_if(in_state(game_shell::AppState::Performance)),
            sync_stage_outline.run_if(super::editor_open),
        ),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_outline);
}

/// Spawn one `StageOutline` node when the surface opens; despawn when it closes.
/// Because the node is despawned on close, the outline is guaranteed invisible
/// whenever the surface is closed (the `editor_open`-gated sync never runs then).
fn spawn_outline_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<StageOutline>>,
) {
    if !open.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    commands.spawn((
        StageOutline,
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor::all(theme.0.stage_panel_border),
        Visibility::Hidden,
        GlobalZIndex(OUTLINE_Z),
        Pickable::IGNORE,
    ));
}

fn despawn_outline(mut commands: Commands, existing: Query<Entity, With<StageOutline>>) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}

/// Track the outline node to the current `StageRect` and show it while not
/// peeking; hidden during peek (and despawned whenever the surface is closed).
fn sync_stage_outline(
    rect: Res<crate::stage_rect::StageRect>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&mut Node, &mut Visibility), With<StageOutline>>,
) {
    let Ok((mut node, mut vis)) = q.single_mut() else {
        return;
    };
    node.left = Val::Px(rect.origin.x);
    node.top = Val::Px(rect.origin.y);
    node.width = Val::Px(rect.size.x);
    node.height = Val::Px(rect.size.y);
    let show = !keys.pressed(KeyCode::Tab);
    *vis = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

/// While the surface is open, drive the target rect from the active tab.
/// Holding `Tab` peeks: forces Identity (full window) and hides chrome for the
/// exact play view; releasing restores the preset + chrome.
fn peek_stage(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    active: Res<super::tabs::ActiveTab>,
    mut target: ResMut<StageTarget>,
    mut chrome: Query<&mut Visibility, With<super::picking::EditorChrome>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let peeking = keys.pressed(KeyCode::Tab);
    let want = if peeking {
        StageRect::full(Vec2::new(win.width(), win.height()))
    } else {
        preset_rect(active.0, Vec2::new(win.width(), win.height()))
    };
    if target.0 != want {
        target.0 = want;
    }
    let vis = if peeking {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut chrome {
        if *v != vis {
            *v = vis;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Left chrome = rail(132) + left panel(348) = 480; right chrome = inspector(240).
    #[test]
    fn settings_tab_fits_beside_left_panel() {
        let r = preset_rect(CustomizeTab::Gameplay, Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::new(480.0 + 16.0, 24.0));
        assert_eq!(
            r.size,
            Vec2::new(1600.0 - 480.0 - 240.0 - 32.0, 900.0 - 48.0)
        );
    }

    #[test]
    fn kit_tab_is_fit_between_chrome() {
        let r = preset_rect(CustomizeTab::Widgets, Vec2::new(1600.0, 900.0));
        assert_eq!(r.origin, Vec2::new(480.0 + 16.0, 24.0));
        assert_eq!(
            r.size,
            Vec2::new(1600.0 - 480.0 - 240.0 - 32.0, 900.0 - 48.0)
        );
    }
}
