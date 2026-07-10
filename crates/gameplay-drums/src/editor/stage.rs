//! Customize stage-transform presets: map ActiveTab → target StageRect.

use super::chrome::{INSPECTOR_WIDTH, LEFT_PANEL_WIDTH};
use crate::stage_rect::{StageRect, StageTarget};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_shell::CustomizeTab;

const TOP_MARGIN: f32 = 24.0;
/// Breathing room between the chrome and the shrunk stage.
const GAP: f32 = 16.0;
/// The left content panel is present on ALL tabs now, so the left chrome is
/// the left column (tab bar + panel); the inspector reserves the right chrome.
const LEFT_CHROME: f32 = LEFT_PANEL_WIDTH;

/// Whether the tab shrinks the whole screen into a framed miniature. Only the
/// Widgets tab does: it needs the true screen edges visible for WYSIWYG anchor
/// placement. Bindings/Lanes preview like the SETTINGS tabs — full-size, shifted.
fn shrinks_stage(tab: CustomizeTab) -> bool {
    matches!(tab, CustomizeTab::Widgets)
}

/// Preset stage rect per tab (osu SetCustomRect target; `stage_xform` derives the
/// actual scale/offset). Two modes, matching the prototype:
///   - SETTINGS tabs + Bindings/Lanes: NO shrink. Rect = full window shifted
///     right by half the left chrome, so the full-size playfield centers in the
///     visible gap (`stage_xform` → scale 1, translate ≈ (chrome/2, 0)).
///   - Widgets: the whole screen shrinks into the gap between the left chrome
///     and the right inspector, centered. The inspector only reserves space
///     when a widget is selected.
pub fn preset_rect(tab: CustomizeTab, window: Vec2, has_inspector: bool) -> StageRect {
    if !shrinks_stage(tab) {
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

/// Just below chrome so the outline reads under the rail; the scrim sits above
/// all HUD global z but below the outline and chrome. See `crate::ui_z`.
const OUTLINE_Z: i32 = crate::ui_z::STAGE_OUTLINE;
const SCRIM_Z: i32 = crate::ui_z::PREVIEW_SCRIM;

/// Full-window dim veil that calms the whole preview while the surface is open
/// (prototype's dim look). Own visibility so peek (full play view) drops it.
#[derive(Component)]
struct PreviewScrim;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            peek_stage.run_if(super::editor_open),
            spawn_outline_on_open.run_if(in_state(game_shell::AppState::Performance)),
            sync_stage_outline.run_if(super::editor_open),
            sync_preview_scrim.run_if(super::editor_open),
        ),
    )
    .add_systems(OnExit(game_shell::AppState::Performance), despawn_outline);
}

/// Fade the dim scrim in while the surface is open and not peeking; the
/// `spawn_outline_on_open` pass spawns/despawns it alongside the outline.
fn sync_preview_scrim(
    state: Res<super::PreviewState>,
    mut q: Query<&mut Visibility, With<PreviewScrim>>,
) {
    let Ok(mut vis) = q.single_mut() else {
        return;
    };
    let show = !state.peeking;
    *vis = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

/// Spawn one `StageOutline` node when the surface opens; despawn when it closes.
/// Because the node is despawned on close, the outline is guaranteed invisible
/// whenever the surface is closed (the `editor_open`-gated sync never runs then).
fn spawn_outline_on_open(
    mut commands: Commands,
    open: Res<super::EditorOpen>,
    theme: Res<dtx_ui::ThemeResource>,
    existing: Query<Entity, With<StageOutline>>,
    scrims: Query<Entity, With<PreviewScrim>>,
) {
    if !open.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    for e in &scrims {
        commands.entity(e).despawn();
    }
    if !open.0 {
        return;
    }
    // Full-window dim veil under the chrome, above all HUD (see SCRIM_Z).
    commands.spawn((
        PreviewScrim,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.024, 0.035, 0.72)),
        GlobalZIndex(SCRIM_Z),
        Visibility::Hidden,
        Pickable::IGNORE,
    ));
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

fn despawn_outline(
    mut commands: Commands,
    existing: Query<Entity, Or<(With<StageOutline>, With<PreviewScrim>)>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
}

/// Track the outline node to the current `StageRect` and show it while not
/// peeking; hidden during peek (and despawned whenever the surface is closed).
fn sync_stage_outline(
    rect: Res<crate::stage_rect::StageRect>,
    state: Res<super::PreviewState>,
    mut q: Query<(&mut Node, &mut Visibility), With<StageOutline>>,
) {
    let Ok((mut node, mut vis)) = q.single_mut() else {
        return;
    };
    node.left = Val::Px(rect.origin.x);
    node.top = Val::Px(rect.origin.y);
    node.width = Val::Px(rect.size.x);
    node.height = Val::Px(rect.size.y);
    // Bounds outline only frames the shrunk miniature (Widgets). Other tabs
    // shift the full-size playfield without a box; peek hides all chrome.
    let show = !state.peeking && shrinks_stage(state.tab);
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
    state: Res<super::PreviewState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut target: ResMut<StageTarget>,
    mut chrome: Query<&mut Visibility, With<super::picking::EditorChrome>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let peeking = state.peeking;
    let want = if peeking {
        StageRect::full(Vec2::new(win.width(), win.height()))
    } else {
        preset_rect(
            state.tab,
            Vec2::new(win.width(), win.height()),
            state.has_inspector,
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

    // Left chrome = left column (tab bar + panel) = 480; inspector = 236.
    #[test]
    fn settings_tab_shifts_full_screen_into_gap() {
        // Settings: no shrink — full window shifted right by half the chrome.
        let win = Vec2::new(1600.0, 900.0);
        let r = preset_rect(CustomizeTab::Gameplay, win, false);
        assert_eq!(r.origin, Vec2::new(240.0, 0.0));
        assert_eq!(r.size, win);
    }

    #[test]
    fn bindings_and_lanes_preview_like_settings() {
        let win = Vec2::new(1600.0, 900.0);
        for tab in [CustomizeTab::Bindings, CustomizeTab::Lanes] {
            let r = preset_rect(tab, win, false);
            assert_eq!(r.origin, Vec2::new(240.0, 0.0));
            assert_eq!(r.size, win);
        }
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
            Vec2::new(1600.0 - 480.0 - 32.0 - (240.0 + 16.0), 900.0 - 48.0)
        );
    }
}
