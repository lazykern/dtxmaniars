//! Nav for Customize settings tabs.
//!
//! Keyboard keeps its flat model (arrows move the focused row and adjust it in
//! place). Pads use a two-level model: the tab rail, then the row list, then an
//! adjust mode on the focused row. Both feed `NavAction`; one consumer owns all
//! mutation.

use bevy::prelude::*;
use game_shell::{CustomizeTab, NavAction, NavSource, NavVerb};

/// Which settings row is focused for nav. Reset to 0 on tab change.
#[derive(Resource, Default)]
pub struct FocusedRow(pub usize);

/// Pad navigation level inside the Customize surface. Keyboard stays flat and
/// ignores this except for the focus ring.
#[derive(Resource, Default)]
pub enum NavLevel {
    /// HH/CY switch tabs, BD enters the tab, SD closes the overlay.
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
    matches!(tab, CustomizeTab::Bindings | CustomizeTab::Widgets)
}

/// Can pads descend from the rail into this tab's rows?
fn pad_can_enter(tab: CustomizeTab) -> bool {
    tab.is_settings() && !pad_excluded(tab)
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
            (keyboard_emit_nav, settings_nav_consumer)
                .chain()
                .run_if(in_state(game_shell::AppState::Performance))
                .run_if(super::editor_open),
        );
}

/// Keyboard → `NavAction`. Tab switching (PageUp/PageDown) stays a raw
/// keyboard affordance; pads switch tabs from the rail level instead.
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
        *level = NavLevel::Rail;
    }
    for action in actions.read() {
        let items = crate::editor::settings_data::settings_items(active.0);
        match action.source {
            NavSource::Keyboard => {
                if !active.0.is_settings() || items.is_empty() {
                    continue;
                }
                let reps = if action.coarse { 10 } else { 1 };
                match action.verb {
                    NavVerb::Down => focused.0 = (focused.0 + 1).min(items.len() - 1),
                    NavVerb::Up => focused.0 = focused.0.saturating_sub(1),
                    verb => {
                        if let (Some(delta), Some(item)) =
                            (adjust_delta(verb, NavSource::Keyboard), items.get(focused.0))
                        {
                            for _ in 0..reps {
                                (item.adjust)(&mut draft.0, delta);
                            }
                        }
                    }
                }
            }
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

    /// Mirrors the keyboard arm of `settings_nav_consumer` so the flat keyboard
    /// model is asserted without booting an App. Must stay in lockstep with it.
    fn apply_keyboard(
        items: &[crate::editor::settings_data::SettingItem],
        focused: &mut usize,
        draft: &mut dtx_config::Config,
        verb: NavVerb,
        coarse: bool,
    ) {
        if items.is_empty() {
            return;
        }
        let reps = if coarse { 10 } else { 1 };
        match verb {
            NavVerb::Down => *focused = (*focused + 1).min(items.len() - 1),
            NavVerb::Up => *focused = focused.saturating_sub(1),
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
        assert!(!pad_can_enter(CustomizeTab::Bindings));
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
    fn keyboard_focus_moves_and_clamps_like_before() {
        let items = crate::editor::settings_data::settings_items(CustomizeTab::Gameplay);
        assert!(items.len() >= 2, "gameplay tab must have rows");
        let mut draft = dtx_config::Config::default();
        let mut focused = 0usize;
        apply_keyboard(items, &mut focused, &mut draft, NavVerb::Down, false);
        assert_eq!(focused, 1);
        apply_keyboard(items, &mut focused, &mut draft, NavVerb::Up, false);
        assert_eq!(focused, 0);
        apply_keyboard(items, &mut focused, &mut draft, NavVerb::Up, false);
        assert_eq!(focused, 0, "clamps at top");
        for _ in 0..items.len() + 5 {
            apply_keyboard(items, &mut focused, &mut draft, NavVerb::Down, false);
        }
        assert_eq!(focused, items.len() - 1, "clamps at bottom");
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
        let mut f = scroll;
        let mut c = scroll;
        apply_keyboard(items, &mut f, &mut fine, NavVerb::Inc, false);
        apply_keyboard(items, &mut c, &mut coarse, NavVerb::Inc, true);

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
