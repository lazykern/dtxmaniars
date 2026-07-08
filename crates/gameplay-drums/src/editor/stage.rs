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
const INSPECTOR_WIDTH: f32 = 236.0;
const TOP_MARGIN: f32 = 24.0;
/// Breathing room between the chrome and the shrunk stage.
const GAP: f32 = 16.0;
/// The left content panel is present on ALL tabs now, so the left chrome is
/// always rail + left panel; the inspector reserves the right chrome.
const LEFT_CHROME: f32 = RAIL_WIDTH + LEFT_PANEL_WIDTH;

/// Preset stage rect per tab (osu SetCustomRect target; `stage_xform` derives the
/// actual scale/offset). Two modes, matching the prototype:
///   - SETTINGS tabs: NO shrink. Rect = full window shifted right by half the
///     left chrome, so the full-size playfield centers in the visible gap
///     (`stage_xform` → scale 1, translate ≈ (chrome/2, 0)). HUD stays hidden
///     (P0), so only lanes+notes show.
///   - KIT tabs (Bindings/Lanes/Widgets): the whole screen shrinks into the gap
///     between the left chrome and the right inspector, centered. The inspector
///     only reserves space on the Widgets tab with a selection.
pub fn preset_rect(tab: CustomizeTab, window: Vec2, has_inspector: bool) -> StageRect {
    if tab.is_settings() {
        return StageRect {
            origin: Vec2::new(LEFT_CHROME / 2.0, 0.0),
            size: window,
        };
    }
    let right = if has_inspector {
        INSPECTOR_WIDTH + GAP
    } else {
        0.0
    };
    StageRect {
        origin: Vec2::new(LEFT_CHROME + GAP, TOP_MARGIN),
        size: Vec2::new(
            (window.x - LEFT_CHROME - 2.0 * GAP - right).max(1.0),
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
            border: UiRect::all(Val::Px(1.0)),
            // Rounded frame around the shrunk miniature (prototype `.shrunk`).
            border_radius: BorderRadius::all(Val::Px(10.0)),
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
    active: Res<super::tabs::ActiveTab>,
    mut q: Query<(&mut Node, &mut Visibility), With<StageOutline>>,
) {
    let Ok((mut node, mut vis)) = q.single_mut() else {
        return;
    };
    node.left = Val::Px(rect.origin.x);
    node.top = Val::Px(rect.origin.y);
    node.width = Val::Px(rect.size.x);
    node.height = Val::Px(rect.size.y);
    // Bounds outline only frames the shrunk miniature on KIT tabs (matches the
    // prototype's `.shrunk` outline). Settings tabs shift the full-size
    // playfield without a box; peek hides all chrome.
    let show = !keys.pressed(KeyCode::Tab) && !active.0.is_settings();
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
    selection: Res<super::drag::Selection>,
    mut target: ResMut<StageTarget>,
    mut chrome: Query<&mut Visibility, With<super::picking::EditorChrome>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let peeking = keys.pressed(KeyCode::Tab);
    let has_inspector = active.0 == CustomizeTab::Widgets && selection.0.is_some();
    let want = if peeking {
        StageRect::full(Vec2::new(win.width(), win.height()))
    } else {
        preset_rect(
            active.0,
            Vec2::new(win.width(), win.height()),
            has_inspector,
        )
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

    // Left chrome = rail(132) + left panel(348) = 480; inspector = 236.
    #[test]
    fn settings_tab_shifts_full_screen_into_gap() {
        // Settings: no shrink — full window shifted right by half the chrome.
        let win = Vec2::new(1600.0, 900.0);
        let r = preset_rect(CustomizeTab::Gameplay, win, false);
        assert_eq!(r.origin, Vec2::new(240.0, 0.0));
        assert_eq!(r.size, win);
    }

    #[test]
    fn kit_tab_fits_between_chrome_no_inspector() {
        let r = preset_rect(CustomizeTab::Widgets, Vec2::new(1600.0, 900.0), false);
        assert_eq!(r.origin, Vec2::new(480.0 + 16.0, 24.0));
        assert_eq!(r.size, Vec2::new(1600.0 - 480.0 - 32.0, 900.0 - 48.0));
    }

    #[test]
    fn kit_tab_reserves_inspector_when_selected() {
        let r = preset_rect(CustomizeTab::Widgets, Vec2::new(1600.0, 900.0), true);
        assert_eq!(r.origin, Vec2::new(480.0 + 16.0, 24.0));
        assert_eq!(
            r.size,
            Vec2::new(1600.0 - 480.0 - 32.0 - (236.0 + 16.0), 900.0 - 48.0)
        );
    }
}
