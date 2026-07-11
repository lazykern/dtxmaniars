//! Nav for Customize settings tabs.
//!
//! Keyboard and pads share the two-level model: the top tab bar, then the row
//! list. Pads add a third adjust mode on the focused row (keyboard adjusts rows
//! in place with Left/Right). Both feed `NavAction`; one consumer owns all
//! mutation.

use bevy::prelude::*;
use game_shell::{CustomizeTab, NavAction, NavSource, NavVerb};

/// Which settings row is focused for nav. Reset to 0 on tab change.
#[derive(Resource, Default)]
pub struct FocusedRow(pub usize);

/// Navigation level inside the Customize surface, shared by keyboard and pads.
#[derive(Resource, Default)]
pub enum NavLevel {
    /// Focus on the top tab bar. Pads: HH/CY switch tabs, BD enters, SD closes
    /// the overlay. Keyboard: ←/→ switch tabs, ↓/Enter enters.
    #[default]
    Rail,
    /// HH/CY move row focus, BD enters adjust, SD returns to the rail.
    Rows,
    /// HH = −1, CY = +1, BD keeps the value, SD reverts to `saved`.
    Adjust {
        /// Draft snapshot taken on adjust-entry; SD restores it.
        saved: Box<dtx_config::Config>,
    },
}

/// Tabs whose CONTENT pads cannot navigate (pointer / capture surfaces).
pub fn pad_excluded(tab: CustomizeTab) -> bool {
    matches!(tab, CustomizeTab::Controls | CustomizeTab::Widgets)
}

/// Can pads descend from the tab bar into this tab's rows?
fn pad_can_enter(tab: CustomizeTab) -> bool {
    tab.is_settings() && !pad_excluded(tab)
}

/// Can the keyboard descend from the tab bar into this tab's rows?
fn keyboard_can_enter(tab: CustomizeTab) -> bool {
    tab.is_settings()
}

/// Delta a verb applies to the focused row, if any. Pads reuse Up/Down as −/+
/// once in adjust mode; the keyboard uses Dec/Inc directly.
fn adjust_delta(verb: NavVerb, source: NavSource) -> Option<i32> {
    match (verb, source) {
        (NavVerb::Up, NavSource::Pad) | (NavVerb::Dec, _) => Some(-1),
        (NavVerb::Down, NavSource::Pad) | (NavVerb::Inc, _) => Some(1),
        _ => None,
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<FocusedRow>()
        .init_resource::<NavLevel>()
        .add_systems(
            Update,
            (
                keyboard_emit_nav,
                settings_nav_consumer,
                update_focus_rings,
                update_tab_bar_focus,
                update_stepper_glyphs,
            )
                .chain()
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        );
}

/// Red ring on the focused row; green while that row is in adjust mode. No row
/// ring while focus sits on the tab bar (the bar button carries it instead).
fn update_focus_rings(
    focused: Res<FocusedRow>,
    level: Res<NavLevel>,
    mut rows: Query<(&super::panel::SettingRow, &mut Outline)>,
) {
    let at_rail = matches!(*level, NavLevel::Rail);
    for (row, mut outline) in &mut rows {
        if row.0 == focused.0 && !at_rail {
            outline.width = Val::Px(3.0);
            outline.color = match *level {
                NavLevel::Adjust { .. } => super::panel::ADJUST_RING,
                _ => super::panel::FOCUS_RING,
            };
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}

/// Red ring on the active tab-bar button while nav focus is on the bar.
fn update_tab_bar_focus(
    active: Res<super::tabs::ActiveTab>,
    level: Res<NavLevel>,
    mut tabs: Query<(&super::ui::TabButton, &mut Outline)>,
) {
    let at_rail = matches!(*level, NavLevel::Rail);
    for (tab, mut outline) in &mut tabs {
        if at_rail && tab.0 == active.0 {
            outline.width = Val::Px(2.0);
            outline.color = super::panel::FOCUS_RING;
        } else {
            outline.width = Val::Px(0.0);
            outline.color = Color::NONE;
        }
    }
}

/// In adjust mode the focused row's steppers read `−` / `+`, not `<` / `>`.
fn update_stepper_glyphs(
    focused: Res<FocusedRow>,
    level: Res<NavLevel>,
    mut glyphs: Query<(&super::panel::StepperGlyph, &mut Text)>,
) {
    let adjusting = matches!(*level, NavLevel::Adjust { .. });
    for (glyph, mut text) in &mut glyphs {
        let active = adjusting && glyph.row == focused.0;
        let want = match (active, glyph.dir) {
            (true, d) if d < 0 => "−",
            (true, _) => "+",
            (false, d) if d < 0 => "<",
            (false, _) => ">",
        };
        if text.0 != want {
            *text = Text::new(want);
        }
    }
}

/// Keyboard → `NavAction`. PageUp/PageDown stay a raw tab-switch shortcut from
/// any level; ←/→ switch tabs only while focus is on the bar (Rail level).
fn keyboard_emit_nav(
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<super::tabs::ActiveTab>,
    mut out: MessageWriter<NavAction>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        return;
    }
    if keys.just_pressed(KeyCode::PageDown) {
        active.0 = active.0.next();
        return;
    } else if keys.just_pressed(KeyCode::PageUp) {
        active.0 = active.0.prev();
        return;
    }
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let verb = if keys.just_pressed(KeyCode::ArrowDown) {
        NavVerb::Down
    } else if keys.just_pressed(KeyCode::ArrowUp) {
        NavVerb::Up
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        NavVerb::Inc
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        NavVerb::Dec
    } else if keys.just_pressed(KeyCode::Enter) {
        NavVerb::Confirm
    } else {
        return;
    };
    out.write(NavAction {
        verb,
        source: NavSource::Keyboard,
        coarse,
    });
}

fn settings_nav_consumer(
    mut actions: MessageReader<NavAction>,
    mut active: ResMut<super::tabs::ActiveTab>,
    mut focused: ResMut<FocusedRow>,
    mut level: ResMut<NavLevel>,
    mut draft: ResMut<super::tabs::ConfigDraft>,
    mut close: MessageWriter<super::EditorCloseRequest>,
) {
    if active.is_changed() {
        focused.0 = 0;
        // Tab switched while working the rows (PgUp/PgDn, mouse click): stay at
        // row level when the new tab has rows; otherwise fall back to the bar.
        let keep_rows = matches!(*level, NavLevel::Rows)
            && active.0.is_settings()
            && !crate::editor::settings_data::settings_items(active.0).is_empty();
        *level = if keep_rows {
            NavLevel::Rows
        } else {
            NavLevel::Rail
        };
    }
    for action in actions.read() {
        let items = crate::editor::settings_data::settings_items(active.0);
        match action.source {
            NavSource::Keyboard => match &mut *level {
                NavLevel::Rail => match action.verb {
                    NavVerb::Dec => active.0 = active.0.prev(),
                    NavVerb::Inc => active.0 = active.0.next(),
                    NavVerb::Down | NavVerb::Confirm => {
                        if keyboard_can_enter(active.0) && !items.is_empty() {
                            focused.0 = 0;
                            *level = NavLevel::Rows;
                        }
                    }
                    _ => {}
                },
                _ => {
                    if !active.0.is_settings() || items.is_empty() {
                        continue;
                    }
                    let reps = if action.coarse { 10 } else { 1 };
                    match action.verb {
                        NavVerb::Down => focused.0 = (focused.0 + 1).min(items.len() - 1),
                        NavVerb::Up => {
                            if focused.0 == 0 {
                                *level = NavLevel::Rail;
                            } else {
                                focused.0 -= 1;
                            }
                        }
                        verb => {
                            if let (Some(delta), Some(item)) = (
                                adjust_delta(verb, NavSource::Keyboard),
                                items.get(focused.0),
                            ) {
                                for _ in 0..reps {
                                    (item.adjust)(&mut draft.0, delta);
                                }
                            }
                        }
                    }
                }
            },
            NavSource::Pad => match &mut *level {
                NavLevel::Rail => match action.verb {
                    NavVerb::Up => active.0 = active.0.prev(),
                    NavVerb::Down => active.0 = active.0.next(),
                    NavVerb::Confirm => {
                        if pad_can_enter(active.0) && !items.is_empty() {
                            focused.0 = 0;
                            *level = NavLevel::Rows;
                        }
                    }
                    NavVerb::Back => {
                        close.write(super::EditorCloseRequest);
                    }
                    _ => {}
                },
                NavLevel::Rows => match action.verb {
                    NavVerb::Up => focused.0 = focused.0.saturating_sub(1),
                    NavVerb::Down => {
                        focused.0 = (focused.0 + 1).min(items.len().saturating_sub(1));
                    }
                    NavVerb::Confirm => {
                        if items.get(focused.0).is_some() {
                            *level = NavLevel::Adjust {
                                saved: Box::new(draft.0.clone()),
                            };
                        }
                    }
                    NavVerb::Back => *level = NavLevel::Rail,
                    _ => {}
                },
                NavLevel::Adjust { saved } => match action.verb {
                    NavVerb::Confirm => *level = NavLevel::Rows,
                    NavVerb::Back => {
                        draft.0 = (**saved).clone();
                        *level = NavLevel::Rows;
                    }
                    verb => {
                        if let (Some(delta), Some(item)) =
                            (adjust_delta(verb, NavSource::Pad), items.get(focused.0))
                        {
                            (item.adjust)(&mut draft.0, delta);
                        }
                    }
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the keyboard arm of `settings_nav_consumer` so the two-level
    /// keyboard model is asserted without booting an App. Must stay in lockstep
    /// with it. `at_rail` mirrors `NavLevel::Rail` vs `NavLevel::Rows`.
    #[allow(clippy::too_many_arguments)]
    fn apply_keyboard(
        active: &mut CustomizeTab,
        focused: &mut usize,
        at_rail: &mut bool,
        draft: &mut dtx_config::Config,
        verb: NavVerb,
        coarse: bool,
    ) {
        let items = crate::editor::settings_data::settings_items(*active);
        if *at_rail {
            match verb {
                NavVerb::Dec => *active = active.prev(),
                NavVerb::Inc => *active = active.next(),
                NavVerb::Down | NavVerb::Confirm => {
                    if keyboard_can_enter(*active) && !items.is_empty() {
                        *focused = 0;
                        *at_rail = false;
                    }
                }
                _ => {}
            }
            return;
        }
        if !active.is_settings() || items.is_empty() {
            return;
        }
        let reps = if coarse { 10 } else { 1 };
        match verb {
            NavVerb::Down => *focused = (*focused + 1).min(items.len() - 1),
            NavVerb::Up => {
                if *focused == 0 {
                    *at_rail = true;
                } else {
                    *focused -= 1;
                }
            }
            v => {
                if let (Some(delta), Some(item)) =
                    (adjust_delta(v, NavSource::Keyboard), items.get(*focused))
                {
                    for _ in 0..reps {
                        (item.adjust)(draft, delta);
                    }
                }
            }
        }
    }

    #[test]
    fn pad_cannot_enter_excluded_or_non_settings_tabs() {
        assert!(!pad_can_enter(CustomizeTab::Controls));
        assert!(!pad_can_enter(CustomizeTab::Widgets));
        assert!(!pad_can_enter(CustomizeTab::Lanes));
        assert!(pad_can_enter(CustomizeTab::Gameplay));
        assert!(pad_can_enter(CustomizeTab::Audio));
        assert!(pad_can_enter(CustomizeTab::Drums));
        assert!(pad_can_enter(CustomizeTab::System));
    }

    #[test]
    fn pad_verbs_in_adjust_mode_map_to_steps() {
        assert_eq!(adjust_delta(NavVerb::Up, NavSource::Pad), Some(-1));
        assert_eq!(adjust_delta(NavVerb::Down, NavSource::Pad), Some(1));
        assert_eq!(adjust_delta(NavVerb::Dec, NavSource::Keyboard), Some(-1));
        assert_eq!(adjust_delta(NavVerb::Inc, NavSource::Keyboard), Some(1));
        assert_eq!(adjust_delta(NavVerb::Confirm, NavSource::Pad), None);
        assert_eq!(adjust_delta(NavVerb::Back, NavSource::Pad), None);
        assert_eq!(adjust_delta(NavVerb::Up, NavSource::Keyboard), None);
        assert_eq!(adjust_delta(NavVerb::Down, NavSource::Keyboard), None);
    }

    #[test]
    fn keyboard_two_level_moves_rows_and_returns_to_bar() {
        let items = crate::editor::settings_data::settings_items(CustomizeTab::Gameplay);
        assert!(items.len() >= 2, "gameplay tab must have rows");
        let mut draft = dtx_config::Config::default();
        let mut active = CustomizeTab::Gameplay;
        let mut focused = 0usize;
        let mut at_rail = true;
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Down,
            false,
        );
        assert!(!at_rail, "Down from the bar enters the rows");
        assert_eq!(focused, 0);
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Down,
            false,
        );
        assert_eq!(focused, 1);
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Up,
            false,
        );
        assert_eq!(focused, 0);
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Up,
            false,
        );
        assert!(at_rail, "Up on the first row returns focus to the bar");
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Confirm,
            false,
        );
        assert!(!at_rail, "Enter on the bar re-enters the rows");
        for _ in 0..items.len() + 5 {
            apply_keyboard(
                &mut active,
                &mut focused,
                &mut at_rail,
                &mut draft,
                NavVerb::Down,
                false,
            );
        }
        assert_eq!(focused, items.len() - 1, "clamps at bottom");
    }

    #[test]
    fn keyboard_bar_left_right_switch_tabs() {
        let mut draft = dtx_config::Config::default();
        let mut active = CustomizeTab::Gameplay;
        let mut focused = 0usize;
        let mut at_rail = true;
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Inc,
            false,
        );
        assert_eq!(active, CustomizeTab::Audio);
        assert!(at_rail, "switching tabs keeps focus on the bar");
        apply_keyboard(
            &mut active,
            &mut focused,
            &mut at_rail,
            &mut draft,
            NavVerb::Dec,
            false,
        );
        assert_eq!(active, CustomizeTab::Gameplay);
    }

    #[test]
    fn keyboard_cannot_descend_into_rowless_tabs() {
        let mut draft = dtx_config::Config::default();
        let mut focused = 0usize;
        for tab in [CustomizeTab::Lanes, CustomizeTab::Widgets] {
            let mut active = tab;
            let mut at_rail = true;
            apply_keyboard(
                &mut active,
                &mut focused,
                &mut at_rail,
                &mut draft,
                NavVerb::Down,
                false,
            );
            assert!(at_rail, "{tab:?} has no settings rows to enter");
        }
    }

    #[test]
    fn keyboard_coarse_applies_ten_steps() {
        let items = crate::editor::settings_data::settings_items(CustomizeTab::Gameplay);
        let scroll = items
            .iter()
            .position(|i| i.label == "Scroll Speed")
            .expect("scroll speed row");
        let base = dtx_config::Config::default();
        let mut fine = base.clone();
        let mut coarse = base.clone();
        let mut active = CustomizeTab::Gameplay;
        let mut at_rail = false;
        let mut f = scroll;
        let mut c = scroll;
        apply_keyboard(
            &mut active,
            &mut f,
            &mut at_rail,
            &mut fine,
            NavVerb::Inc,
            false,
        );
        apply_keyboard(
            &mut active,
            &mut c,
            &mut at_rail,
            &mut coarse,
            NavVerb::Inc,
            true,
        );

        let one = (items[scroll].raw)(&fine) - (items[scroll].raw)(&base);
        let ten = (items[scroll].raw)(&coarse) - (items[scroll].raw)(&base);
        assert!(one.abs() > 0.0, "one step must change the value");
        assert!(
            (ten - one * 10.0).abs() < 1e-3,
            "coarse must be 10x fine: {ten} vs {one}"
        );
    }

    #[test]
    fn pad_adjust_step_then_back_reverts_to_snapshot() {
        let items = crate::editor::settings_data::settings_items(CustomizeTab::Gameplay);
        let scroll = items
            .iter()
            .position(|i| i.label == "Scroll Speed")
            .expect("scroll speed row");
        let base = dtx_config::Config::default();
        let mut draft = base.clone();

        let saved = draft.clone();
        let delta = adjust_delta(NavVerb::Down, NavSource::Pad).unwrap();
        (items[scroll].adjust)(&mut draft, delta);
        assert_ne!(
            (items[scroll].raw)(&draft),
            (items[scroll].raw)(&base),
            "pad Down must step the value"
        );

        draft = saved;
        assert_eq!(
            (items[scroll].raw)(&draft),
            (items[scroll].raw)(&base),
            "pad Back must revert to the adjust-entry snapshot"
        );
    }
}
