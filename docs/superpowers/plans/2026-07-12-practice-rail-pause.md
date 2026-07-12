# Practice Rail + Pause Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Esc in practice opens the standard pause overlay (extended with Resume / Restart loop / Exit Practice); Tab keeps opening the full rail. The rail becomes ref-px scaled, typographically hierarchical, and mouse-operable. `ExitArmed` double-Enter exit dies. Normal-play pause overlay behavior is unchanged.

**Architecture:** A `PracticePauseSurface` resource (`Overlay` | `Rail`) discriminates which surface owns `PauseState::Paused` during practice; the Esc opener (`toggle_pause`) and Tab opener (`apply_practice_actions` → `OpenFullHud`) set it before pausing, and it resets to `Overlay` on every `OnEnter(PauseState::Running)`. The pause overlay's row set becomes context-dependent via a pure `pause_items(practice)` helper; its practice rows dispatch by `PauseItemKind` (Restart loop reuses `PracticeAction::RestartLoop` via the message stream — `apply_practice_actions` is gated `Running` and reads it the frame after resume applies; messages live two frames). The rail is respawned in ref-px (1280×720 reference space × `PlayfieldLayout::scale`) with `HudRefRect` dual-write like `now_playing.rs`; the existing unscoped `apply_hud_ref_layout` (hud.rs:393-411) re-applies rects on resize for free. Keyboard adjust/activate logic is extracted into `adjust_rail_item` / `activate_rail_item` helpers shared by a new `rail_mouse` Interaction system (same pattern as the in-file `TransportButton`s), with `RailSelection` as the single shared cursor.

**Tech Stack:** Rust, Bevy 0.19 (`Message` / `MessageReader` / `MessageWriter`, `NextState`, `States`), existing crates only (`gameplay-drums`, `dtx-ui`). No new dependencies. Repo rules: no `unwrap()` in `crates/*` src (tests may), conventional commits, **no co-author trailers**.

---

## Verified source facts (read before implementing)

- `apply_hud_ref_layout` (`crates/gameplay-drums/src/hud.rs:393-411`) is **unscoped**: it queries ALL `(&HudRefRect, &mut Node)` entities, excluding only entities carrying `PlayfieldBackboard`, `HitLine`, `SongProgressFill`, `PlayfieldSpeedText`, `PhrasePlayhead`, `PhraseSection`, or `LiveGraphBar` (each has its own layout system). It runs in `Update` under `in_state(AppState::Performance)` + `resource_changed::<PlayfieldLayout>` — **not** gated on `PauseState`, so full-HUD entities tagged with `HudRefRect` get live-resize re-application with **no new system needed**. Initial `Node` values must still be dual-written at spawn with the current scale (the system only fires on layout *change*), exactly like `now_playing.rs`.
- `PracticeAction::RestartLoop` is applied in `apply_practice_actions` (`crates/gameplay-drums/src/practice/actions.rs:127-139`): computes intent = loop start (or current attempt start), writes `SeekToChartTime` with `preroll_target`, pushes a "restart" toast. The system is registered (`practice/mod.rs:29-39`) gated `Performance` + `PauseState::Running` + `PracticeSession` exists. Writing the message from the paused overlay and setting `next_pause` → `Running` in the same frame works: the state transition applies before the next frame's `Update`, and the message (two-frame lifetime) is read then.
- `PlayfieldLayout` (`layout.rs`) is `init_resource`'d app-wide and rebuilt on resize; `scale = (w/1280).min(h/720)`, `origin = (0,0)` for the full-window `StageRect::full`. Headless tests do **not** register it → `spawn_full_hud` takes `Option<Res<PlayfieldLayout>>` with fallback `(scale, origin) = (1.0, Vec2::ZERO)` — never panic.
- Repo Bevy-0.19 test idioms (verified in-tree): `bevy::ecs::system::RunSystemOnce` + `world.run_system_once(sys).expect(..)` (gauge.rs:219-231), `world.init_resource::<bevy::ecs::message::Messages<T>>()`, `world.write_message(..)` / `.resource_mut::<Messages<T>>().write(..)`, `.iter_current_update_messages()` for read-back (play_chart.rs:80), `ChildSpawnerCommands` as the `with_children` builder type, `Overflow::clip_y()`.
- `RailItem`/`ExitArmed`/`RailSelection` external users: only `practice/hud/mod.rs` and `tests/practice_hud.rs` (grep-verified). `rail_label` is used only inside `full_hud.rs`.
- Header injection indices 0/7/10 (TRANSPORT/LOOP/TRAINER) stay valid after deleting `ExitPractice` (it is the last `ORDER` entry).

## File structure

```
crates/gameplay-drums/src/
  pause.rs                      # Tasks 1,2: PracticePauseSurface, PauseItemKind, pause_items, guards, dispatch
  practice/actions.rs           # Task 1: OpenFullHud sets Rail surface
  practice/hud/mod.rs           # Tasks 3,5,6: rail_surface_active gating, ExitArmed removal, new systems in chain
  practice/hud/full_hud.rs      # Tasks 3,4,5,6: exit removal, helpers, ref-px rebuild, mouse system
  ui_z.rs                       # (read-only; PRACTICE_FULL_HUD already exists)
crates/gameplay-drums/tests/
  practice_hud.rs               # Tasks 2,3,5,6: surface gating + overlay + rail anatomy + mouse tests
docs/superpowers/plans/2026-07-12-practice-rail-pause.md   # this file
```

No other file is touched. `dtx-ui` is consumed as-is (`HudRefRect`, `scaled_font`, `Theme`, `spawn_density_strip`, `spawn_nav_legend`).

---

## Task 1 — `PracticePauseSurface` resource + openers + reset

**Files:**
- `crates/gameplay-drums/src/pause.rs`
- `crates/gameplay-drums/src/practice/actions.rs`

- [ ] **Write failing tests.** In `pause.rs`, add a tests module at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn esc_opener_sets_overlay_surface() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Running));
        world.init_resource::<NextState<PauseState>>();
        // Stale value from a previous Tab-opened rail must be overwritten.
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Overlay
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }

    #[test]
    fn esc_while_paused_resumes_and_leaves_surface_alone() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(State::new(PauseState::Paused));
        world.init_resource::<NextState<PauseState>>();
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(toggle_pause)
            .expect("toggle_pause runs");
        // The OnEnter(Running) reset handles hygiene; the toggle itself only closes.
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Rail
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
    }

    #[test]
    fn surface_resets_to_overlay_on_running() {
        let mut world = World::new();
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(reset_pause_surface)
            .expect("reset runs");
        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Overlay
        );
    }
}
```

  In `practice/actions.rs` tests module, add:

```rust
    #[test]
    fn tab_opener_sets_rail_surface_and_pauses() {
        use crate::pause::PracticePauseSurface;
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use bevy::prelude::*;

        let mut world = World::new();
        world.init_resource::<Messages<PracticeAction>>();
        world.init_resource::<Messages<crate::seek::SeekToChartTime>>();
        world.insert_resource(crate::practice::session::PracticeSession::default());
        world.init_resource::<crate::timeline::ChipTimeline>();
        world.init_resource::<crate::resources::GameplayClock>();
        world.init_resource::<super::toast::ToastQueue>();
        world.init_resource::<NextState<PauseState>>();
        world.init_resource::<PracticePauseSurface>();
        world.write_message(PracticeAction::OpenFullHud);

        world
            .run_system_once(apply_practice_actions)
            .expect("apply_practice_actions runs");

        assert_eq!(
            *world.resource::<PracticePauseSurface>(),
            PracticePauseSurface::Rail
        );
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Paused)
        ));
    }
```

- [ ] **Run:** `cargo test -p gameplay-drums pause_surface` and `cargo test -p gameplay-drums tab_opener` — expected failure: compile error `cannot find type 'PracticePauseSurface'` (E0412/E0433).

- [ ] **Implement.** In `pause.rs`, below the `PauseSelection` definition, add:

```rust
/// Which surface owns `PauseState::Paused` during practice. Esc opens the
/// standard pause overlay; Tab opens the full practice rail. Irrelevant
/// outside practice (the overlay always spawns); reset to `Overlay` on
/// every return to `Running` for hygiene.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PracticePauseSurface {
    #[default]
    Overlay,
    Rail,
}
```

  Replace `toggle_pause` with:

```rust
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<PauseState>>,
    mut next: ResMut<NextState<PauseState>>,
    mut surface: ResMut<PracticePauseSurface>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        match state.get() {
            PauseState::Running => {
                *surface = PracticePauseSurface::Overlay;
                next.set(PauseState::Paused);
            }
            PauseState::Paused => next.set(PauseState::Running),
        }
    }
}
```

  Add the reset system:

```rust
fn reset_pause_surface(mut surface: ResMut<PracticePauseSurface>) {
    *surface = PracticePauseSurface::Overlay;
}
```

  In `pause::plugin`, register both (add to the existing builder chain):

```rust
    app.init_resource::<PauseSelection>()
        .init_resource::<PracticePauseSurface>()
        .add_systems(OnEnter(PauseState::Running), reset_pause_surface)
        // ... existing registrations unchanged ...
```

  In `practice/actions.rs`, change `apply_practice_actions`'s signature and `OpenFullHud` arm (the function gains an 8th param, so add the clippy allow):

```rust
#[allow(clippy::too_many_arguments)]
pub fn apply_practice_actions(
    mut actions: MessageReader<PracticeAction>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut toasts: ResMut<ToastQueue>,
    mut surface: ResMut<crate::pause::PracticePauseSurface>,
) {
```

  and:

```rust
            PracticeAction::OpenFullHud => {
                *surface = crate::pause::PracticePauseSurface::Rail;
                next_pause.set(PauseState::Paused);
            }
```

  Everything else in the match stays byte-identical.

- [ ] **Run:** `cargo test -p gameplay-drums` — all green (existing `practice_mode.rs` integration tests that drive Tab via the real plugin get the resource from `pause::plugin`; if any hand-wired test app lacks `PracticePauseSurface`, add `.init_resource::<gameplay_drums::pause::PracticePauseSurface>()` to that test's setup).
- [ ] **Commit:** `feat(pause): add PracticePauseSurface discriminator set by Esc/Tab openers`

---

## Task 2 — Pause overlay practice rows + un-suppression

**Files:**
- `crates/gameplay-drums/src/pause.rs`
- `crates/gameplay-drums/tests/practice_hud.rs`

- [ ] **Write failing tests.** In `pause.rs` tests module:

```rust
    #[test]
    fn pause_items_normal_vs_practice() {
        assert_eq!(
            pause_items(false),
            &[
                PauseItemKind::Resume,
                PauseItemKind::Retry,
                PauseItemKind::Quit
            ]
        );
        assert_eq!(
            pause_items(true),
            &[
                PauseItemKind::Resume,
                PauseItemKind::RestartLoop,
                PauseItemKind::ExitPractice
            ]
        );
    }

    fn dispatch_world(selection: usize) -> World {
        use bevy::ecs::message::Messages;
        let mut world = World::new();
        world.init_resource::<Messages<game_shell::NavAction>>();
        world.init_resource::<Messages<TransitionRequest>>();
        world.init_resource::<Messages<crate::practice::actions::PracticeAction>>();
        world.insert_resource(PauseSelection(selection));
        world.init_resource::<NextState<PauseState>>();
        world.init_resource::<PracticePauseSurface>(); // Overlay
        world.insert_resource(crate::practice::PracticeSession::default());
        world.write_message(game_shell::NavAction {
            verb: game_shell::NavVerb::Confirm,
            source: game_shell::NavSource::Keyboard,
            coarse: false,
        });
        world
    }

    #[test]
    fn practice_confirm_exit_goes_to_song_select() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(2); // Exit Practice row
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let targets: Vec<AppState> = world
            .resource::<Messages<TransitionRequest>>()
            .iter_current_update_messages()
            .map(|r| r.0)
            .collect();
        assert_eq!(targets, vec![AppState::SongSelect]);
    }

    #[test]
    fn practice_confirm_restart_loop_emits_action_and_resumes() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use crate::practice::actions::PracticeAction;
        let mut world = dispatch_world(1); // Restart loop row
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let actions: Vec<PracticeAction> = world
            .resource::<Messages<PracticeAction>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(actions, vec![PracticeAction::RestartLoop]);
    }

    #[test]
    fn rail_surface_clears_actions_and_does_nothing() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        let mut world = dispatch_world(0);
        world.insert_resource(PracticePauseSurface::Rail);
        world
            .run_system_once(pause_menu_input)
            .expect("pause_menu_input runs");
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Unchanged
        ));
        assert_eq!(
            world
                .resource::<Messages<TransitionRequest>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }
```

  In `tests/practice_hud.rs`, **replace** `normal_pause_overlay_suppressed_in_practice` with:

```rust
use gameplay_drums::pause::PracticePauseSurface;

#[test]
fn overlay_spawns_in_practice_on_overlay_surface() {
    let mut app = build_app();
    app.init_resource::<gameplay_drums::pause::PauseSelection>()
        .init_resource::<PracticePauseSurface>() // defaults to Overlay
        .add_systems(
            OnEnter(PauseState::Paused),
            gameplay_drums::pause::spawn_overlay,
        );
    app.world_mut().insert_resource(PracticeSession::default());
    set_paused(&mut app, true);
    let overlays = app
        .world_mut()
        .query::<&gameplay_drums::pause::PauseOverlay>()
        .iter(app.world())
        .count();
    assert_eq!(overlays, 1, "Esc surface shows the pause overlay in practice");
}

#[test]
fn overlay_suppressed_on_rail_surface() {
    let mut app = build_app();
    app.init_resource::<gameplay_drums::pause::PauseSelection>()
        .init_resource::<PracticePauseSurface>()
        .add_systems(
            OnEnter(PauseState::Paused),
            gameplay_drums::pause::spawn_overlay,
        );
    app.world_mut().insert_resource(PracticeSession::default());
    app.world_mut()
        .insert_resource(PracticePauseSurface::Rail);
    set_paused(&mut app, true);
    let overlays = app
        .world_mut()
        .query::<&gameplay_drums::pause::PauseOverlay>()
        .iter(app.world())
        .count();
    assert_eq!(overlays, 0, "Tab surface suppresses the overlay; the rail owns it");
}
```

- [ ] **Run:** `cargo test -p gameplay-drums pause_items` — expected failure: compile error `cannot find function 'pause_items'` / `cannot find type 'PauseItemKind'`.

- [ ] **Implement.** In `pause.rs`, replace the `PauseItem` enum + impl (lines 24-42) with:

```rust
/// One selectable pause-menu row. The set differs between normal play and
/// practice — see [`pause_items`].
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseItemKind {
    Resume,
    Retry,
    Quit,
    RestartLoop,
    ExitPractice,
}

impl PauseItemKind {
    fn label(self) -> &'static str {
        match self {
            PauseItemKind::Resume => "Resume",
            PauseItemKind::Retry => "Retry",
            PauseItemKind::Quit => "Quit to Song Select",
            PauseItemKind::RestartLoop => "Restart loop",
            PauseItemKind::ExitPractice => "Exit Practice",
        }
    }
}

const NORMAL_ITEMS: &[PauseItemKind] = &[
    PauseItemKind::Resume,
    PauseItemKind::Retry,
    PauseItemKind::Quit,
];
const PRACTICE_ITEMS: &[PauseItemKind] = &[
    PauseItemKind::Resume,
    PauseItemKind::RestartLoop,
    PauseItemKind::ExitPractice,
];

/// Rows for the pause overlay: practice gets Resume / Restart loop /
/// Exit Practice; normal play keeps Resume / Retry / Quit exactly as-is.
pub fn pause_items(practice: bool) -> &'static [PauseItemKind] {
    if practice {
        PRACTICE_ITEMS
    } else {
        NORMAL_ITEMS
    }
}
```

  Update the `PauseOverlay` doc comment (lines 19-20) to:

```rust
/// Root marker for the pause overlay. In practice this spawns for the Esc
/// surface; Tab opens the full rail instead (see PracticePauseSurface).
#[derive(Component)]
pub struct PauseOverlay;
```

  Replace `spawn_overlay` (the only changes: signature gains `surface`, the early return becomes surface-conditional, the row loop uses `pause_items`):

```rust
pub fn spawn_overlay(
    mut commands: Commands,
    mut selection: ResMut<PauseSelection>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    surface: Res<PracticePauseSurface>,
    midi: Option<Res<game_shell::MidiConnected>>,
) {
    if practice.is_some() && *surface == PracticePauseSurface::Rail {
        return; // Tab-opened pause: the practice rail owns this surface
    }
    selection.0 = 0;
    let theme = Theme::default();
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(crate::ui_z::PAUSE),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("PAUSED"),
                Theme::title_font(),
                TextColor(theme.text_primary),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
            for item in pause_items(practice.is_some()) {
                root.spawn((
                    *item,
                    Text::new(item.label()),
                    Theme::hud_font(),
                    TextColor(theme.text_secondary),
                ));
            }
            if midi.is_some_and(|m| m.0) {
                dtx_ui::widget::nav_legend::spawn_nav_legend(
                    root,
                    &theme,
                    &[
                        ("HH", "up"),
                        ("CY", "down"),
                        ("BD", "select"),
                        ("SD", "resume"),
                    ],
                );
            }
        });
}
```

  Pad grammar is identical in both variants (HH/CY move, BD confirm, SD resume); the legend is unchanged.

  Replace `pause_menu_input`:

```rust
#[allow(clippy::too_many_arguments)]
fn pause_menu_input(
    mut actions: MessageReader<game_shell::NavAction>,
    mut selection: ResMut<PauseSelection>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut requests: MessageWriter<TransitionRequest>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
    mut rows: Query<(&PauseItemKind, &mut TextColor)>,
    practice: Option<Res<crate::practice::PracticeSession>>,
    surface: Res<PracticePauseSurface>,
) {
    use game_shell::NavVerb;
    if practice.is_some() && *surface == PracticePauseSurface::Rail {
        actions.clear(); // rail owns this pause; don't double-handle keys/pads
        return;
    }
    let items = pause_items(practice.is_some());
    let count = items.len();
    let mut confirm = false;
    let mut resume = false;
    for action in actions.read() {
        match action.verb {
            NavVerb::Down => selection.0 = (selection.0 + 1) % count,
            NavVerb::Up => selection.0 = (selection.0 + count - 1) % count,
            NavVerb::Confirm => confirm = true,
            // SD resumes: the pad equivalent of Esc.
            NavVerb::Back => resume = true,
            _ => {}
        }
    }

    let theme = Theme::default();
    let selected = items[selection.0 % count];
    for (item, mut color) in &mut rows {
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }

    if resume {
        next_pause.set(PauseState::Running);
        return;
    }
    if confirm {
        match selected {
            PauseItemKind::Resume => next_pause.set(PauseState::Running),
            PauseItemKind::Retry => {
                // Reload the chart from the top via the loading screen.
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongLoading);
            }
            PauseItemKind::Quit | PauseItemKind::ExitPractice => {
                next_pause.set(PauseState::Running);
                request_transition(&mut requests, AppState::SongSelect);
            }
            PauseItemKind::RestartLoop => {
                // Reuse the quick-tier effect verbatim: apply_practice_actions
                // (gated Running) reads this message the frame after the resume
                // transition applies — messages live two update cycles.
                practice_actions
                    .write(crate::practice::actions::PracticeAction::RestartLoop);
                next_pause.set(PauseState::Running);
            }
        }
    }
}
```

  Notes: `ExitPractice` needs no session teardown here — `remove_practice_session` already runs `OnEnter(AppState::SongSelect)` (practice/mod.rs:51). Normal play never renders `RestartLoop`/`ExitPractice` rows, and its three rows + dispatch are byte-identical to today — `pause_items_normal_vs_practice` pins that.

  **Transient state note (until Task 3 lands):** Esc in practice now spawns the overlay while the rail also still spawns (its gate arrives in Task 3). Acceptable between commits; no test exercises both surfaces at once.

- [ ] **Run:** `cargo test -p gameplay-drums` — all green.
- [ ] **Commit:** `feat(pause): practice pause rows — Resume / Restart loop / Exit Practice`

---

## Task 3 — Gate the rail on `surface == Rail`; delete `ExitPractice` + `ExitArmed`

**Files:**
- `crates/gameplay-drums/src/practice/hud/mod.rs`
- `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- `crates/gameplay-drums/tests/practice_hud.rs`

- [ ] **Write failing test.** In `tests/practice_hud.rs`, add (uses the real plugin, like `real_hud_plugin_schedule_builds_headlessly`):

```rust
#[test]
fn hud_plugin_overlay_surface_spawns_no_rail() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
    ))
    .init_state::<AppState>()
    .init_state::<PauseState>()
    .add_message::<game_shell::TransitionRequest>()
    .add_message::<gameplay_drums::seek::SeekToChartTime>()
    .add_message::<gameplay_drums::practice::actions::PracticeAction>()
    .init_resource::<GameplayClock>()
    .init_resource::<ChipTimeline>()
    .world_mut()
    .insert_resource(PracticeSession::default());

    gameplay_drums::practice::hud::plugin(&mut app);

    // Esc path: surface stays at its Overlay default.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Performance);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<PauseState>>()
        .set(PauseState::Paused);
    app.update();

    let huds = app
        .world_mut()
        .query::<&FullHudRoot>()
        .iter(app.world())
        .count();
    assert_eq!(huds, 0, "Esc surface must not spawn the rail");
}
```

- [ ] **Run:** `cargo test -p gameplay-drums hud_plugin_overlay_surface` — expected failure: `assertion 'left == right' failed: Esc surface must not spawn the rail; left: 1, right: 0`.

- [ ] **Implement gating.** Rewrite `practice/hud/mod.rs::plugin` (and add the run condition):

```rust
/// Run condition: the practice rail owns the current pause (Tab opener).
pub fn rail_surface_active(surface: Res<crate::pause::PracticePauseSurface>) -> bool {
    *surface == crate::pause::PracticePauseSurface::Rail
}

/// Exposed `pub` (not `pub(super)`) so integration tests can build the real
/// HUD plugin schedule headlessly; see `tests/practice_hud.rs`.
pub fn plugin(app: &mut App) {
    use game_shell::{AppState, PauseState};
    mini_strip::plugin(app);
    chip::plugin(app);
    app.init_resource::<full_hud::RailSelection>()
        .init_resource::<crate::pause::PracticePauseSurface>()
        .init_resource::<timeline_ui::TimelineGesture>()
        .init_resource::<crate::practice::toast::ToastQueue>()
        .add_systems(
            OnEnter(PauseState::Paused),
            full_hud::spawn_full_hud
                .run_if(resource_exists::<crate::practice::PracticeSession>)
                .run_if(rail_surface_active),
        )
        .add_systems(OnExit(PauseState::Paused), full_hud::despawn_full_hud)
        .add_systems(
            Update,
            (
                timeline_ui::timeline_mouse,
                full_hud::full_hud_input,
                full_hud::transport_buttons,
                full_hud::update_full_hud_markers,
            )
                .chain()
                .run_if(in_state(AppState::Performance))
                .run_if(in_state(PauseState::Paused))
                .run_if(resource_exists::<crate::practice::PracticeSession>)
                .run_if(rail_surface_active),
        );
}
```

  (`init_resource::<PracticePauseSurface>` here is idempotent with `pause::plugin`'s and lets tests build `hud::plugin` standalone. `despawn_full_hud` stays unconditional — empty query is a no-op. Gating the Update chain on `rail_surface_active` is what prevents Enter/arrow double-handling when the overlay surface is up.) Also update the module doc header (lines 1-3) to say the full HUD is the **Tab** pause tier.

- [ ] **Implement exit removal.** In `full_hud.rs`:
  - Delete `RailItem::ExitPractice` from the enum; `ORDER` becomes 17 entries (drop the last):

```rust
    pub const ORDER: [RailItem; 17] = [
        RailItem::Resume,
        RailItem::Scrub,
        RailItem::RestartSection,
        RailItem::Rate,
        RailItem::Snap,
        RailItem::Preroll,
        RailItem::Metronome,
        RailItem::SetA,
        RailItem::SetB,
        RailItem::ClearLoop,
        RailItem::RampArm,
        RailItem::RampStart,
        RailItem::RampTarget,
        RailItem::RampStep,
        RailItem::RampThreshold,
        RailItem::RampStreak,
        RailItem::WaitMode,
    ];
```

    Header injection indices 0/7/10 in `spawn_full_hud` are unchanged (ExitPractice was last).
  - Delete the `ExitArmed` resource (lines 88-90).
  - `rail_label` drops the `exit_armed` parameter and the `ExitPractice` arm: signature becomes `pub fn rail_label(item: RailItem, session: &PracticeSession) -> String`; update both call sites (`spawn_full_hud`: `rail_label(*item, &session)`; `full_hud_input` tail: `rail_label(*item, &session)`) and the three `rail_label(...)` calls in the tests module (drop the `false` argument).
  - `spawn_full_hud`: remove the `mut exit_armed: ResMut<ExitArmed>` param and `exit_armed.0 = false;` line.
  - `full_hud_input`: remove `mut exit_armed: ResMut<ExitArmed>`, `mut requests: MessageWriter<TransitionRequest>`, both `exit_armed.0 = false;` lines in the Up/Down arms, and the whole `RailItem::ExitPractice => { ... }` Enter arm. Fix the import line to `use game_shell::PauseState;` (drop `request_transition`, `AppState`, `TransitionRequest` — now unused).
  - Update the file doc header (line 3): drop ", exit".
- [ ] **Update tests.** In `tests/practice_hud.rs`:
  - `build_app()`: delete `.init_resource::<gameplay_drums::practice::hud::full_hud::ExitArmed>()`, add `.init_resource::<PracticePauseSurface>()`, and mirror the plugin's gate on the hand-wired spawn:

```rust
        .add_systems(
            OnEnter(PauseState::Paused),
            spawn_full_hud
                .run_if(resource_exists::<PracticeSession>)
                .run_if(gameplay_drums::practice::hud::rail_surface_active),
        )
```

  - Add a helper and use it in every test that expects the rail to spawn (`full_hud_spawns_on_pause_and_despawns_on_resume`, `next_bar_button_moves_scrub_cursor`, `rail_clear_loop_disarms_the_ramp` — the latter doesn't spawn UI, it only needs `ExitArmed` init removed):

```rust
fn set_rail_surface(app: &mut App) {
    app.world_mut()
        .insert_resource(PracticePauseSurface::Rail);
}
```

    Call `set_rail_surface(&mut app);` before `set_paused(&mut app, true)` in `full_hud_spawns_on_pause_and_despawns_on_resume`.
  - `real_hud_plugin_schedule_builds_headlessly`: insert `app.world_mut().insert_resource(gameplay_drums::pause::PracticePauseSurface::Rail);` before setting `PauseState::Paused` (simulates the Tab opener).
  - Line 192 import: drop `ExitArmed` → `use gameplay_drums::practice::hud::full_hud::{full_hud_input, RailItem};` and delete `.init_resource::<ExitArmed>()` at line 211.
- [ ] **Run:** `cargo test -p gameplay-drums` — all green (grep the crate for `ExitArmed`: zero hits).
- [ ] **Commit:** `feat(practice): gate full rail on Tab surface; kill double-Enter exit`

---

## Task 4 — Extract `adjust_rail_item` / `activate_rail_item` + row label/value/kind helpers

**Files:**
- `crates/gameplay-drums/src/practice/hud/full_hud.rs`

- [ ] **Write failing tests.** In `full_hud.rs` tests module, add (and port the two `rail_label` toggle tests to `rail_row_value` — delete `wait_rail_label_reflects_toggle` and `metronome_rail_label_reflects_toggle`):

```rust
    #[test]
    fn rail_row_value_reflects_toggles() {
        let mut s = PracticeSession::default();
        assert_eq!(rail_row_value(RailItem::WaitMode, &s), "off");
        s.trainer.wait_enabled = true;
        assert_eq!(rail_row_value(RailItem::WaitMode, &s), "ON");
        assert_eq!(rail_row_value(RailItem::Metronome, &s), "on");
        s.transport.metronome = false;
        assert_eq!(rail_row_value(RailItem::Metronome, &s), "off");
    }

    #[test]
    fn rail_row_kind_classifies_every_row() {
        use RailItem::*;
        for item in RailItem::ORDER {
            let kind = rail_row_kind(item);
            match item {
                Scrub | Rate | Snap | Preroll | RampStart | RampTarget | RampStep
                | RampThreshold | RampStreak => assert_eq!(kind, RowKind::Value),
                Resume | RestartSection | SetA | SetB | ClearLoop | RampArm => {
                    assert_eq!(kind, RowKind::Action)
                }
                Metronome | WaitMode => assert_eq!(kind, RowKind::Toggle),
            }
        }
    }

    #[test]
    fn adjust_rate_steps_and_streak_clamps() {
        let timeline = ChipTimeline::default();
        let mut s = PracticeSession::default();
        adjust_rail_item(RailItem::Rate, 1, &mut s, &timeline, 0);
        assert!((s.transport.user_tempo - 1.05).abs() < 1e-6);
        for _ in 0..10 {
            adjust_rail_item(RailItem::RampStreak, 1, &mut s, &timeline, 0);
        }
        assert_eq!(s.trainer.ramp_config.required_successes, 3, "clamped at 3");
        adjust_rail_item(RailItem::Snap, 1, &mut s, &timeline, 0);
        assert_eq!(s.transport.snap, crate::timeline::SnapDivisor::Beat);
    }

    #[test]
    fn activate_clear_loop_disarms_ramp_and_resume_sets_running() {
        use bevy::ecs::message::Messages;
        use bevy::ecs::system::RunSystemOnce;
        use crate::practice::actions::PracticeAction;
        use crate::seek::SeekToChartTime;
        use game_shell::PauseState;

        let mut world = World::new();
        world.init_resource::<Messages<SeekToChartTime>>();
        world.init_resource::<Messages<PracticeAction>>();
        world.init_resource::<NextState<PauseState>>();
        world.init_resource::<ChipTimeline>();
        let mut session = PracticeSession::default();
        session.set_loop_start(2_000);
        session.set_loop_end(6_000);
        session.trainer.ramp.armed = true;
        world.insert_resource(session);

        world
            .run_system_once(
                |mut session: ResMut<PracticeSession>,
                 timeline: Res<ChipTimeline>,
                 mut next: ResMut<NextState<PauseState>>,
                 mut seeks: MessageWriter<SeekToChartTime>,
                 mut pa: MessageWriter<PracticeAction>| {
                    activate_rail_item(
                        RailItem::ClearLoop,
                        &mut session,
                        &timeline,
                        0,
                        None,
                        None,
                        &mut next,
                        &mut seeks,
                        &mut pa,
                    );
                    activate_rail_item(
                        RailItem::Resume,
                        &mut session,
                        &timeline,
                        0,
                        None,
                        None,
                        &mut next,
                        &mut seeks,
                        &mut pa,
                    );
                    activate_rail_item(
                        RailItem::RampArm,
                        &mut session,
                        &timeline,
                        0,
                        None,
                        None,
                        &mut next,
                        &mut seeks,
                        &mut pa,
                    );
                },
            )
            .expect("helpers run");

        let session = world.resource::<PracticeSession>();
        assert!(session.transport.loop_region.is_none());
        assert!(!session.trainer.ramp.armed);
        assert!(matches!(
            world.resource::<NextState<PauseState>>(),
            NextState::Pending(PauseState::Running)
        ));
        let toggles: Vec<PracticeAction> = world
            .resource::<Messages<PracticeAction>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(toggles, vec![PracticeAction::ToggleRamp]);
    }
```

- [ ] **Run:** `cargo test -p gameplay-drums rail_row_kind` — expected failure: compile error `cannot find function 'rail_row_kind'` (and `adjust_rail_item`, `activate_rail_item`, `rail_row_value`).

- [ ] **Implement.** In `full_hud.rs`, add below the `RailSelection` definition:

```rust
/// How a rail row reacts to input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowKind {
    /// ◂ value ▸: Left/Right (or the glyph buttons) adjust; row click selects only.
    Value,
    /// Row click / Enter runs the action.
    Action,
    /// Row click / Enter flips the switch.
    Toggle,
}

pub fn rail_row_kind(item: RailItem) -> RowKind {
    use RailItem::*;
    match item {
        Scrub | Rate | Snap | Preroll | RampStart | RampTarget | RampStep | RampThreshold
        | RampStreak => RowKind::Value,
        Resume | RestartSection | SetA | SetB | ClearLoop | RampArm => RowKind::Action,
        Metronome | WaitMode => RowKind::Toggle,
    }
}

/// Static left-column label for a rail row.
pub fn rail_row_label(item: RailItem) -> &'static str {
    match item {
        RailItem::Resume => "Resume",
        RailItem::Scrub => "Scrub",
        RailItem::RestartSection => "Restart section",
        RailItem::SetA => "Set A here",
        RailItem::SetB => "Set B here",
        RailItem::ClearLoop => "Clear loop",
        RailItem::Rate => "Tempo",
        RailItem::Snap => "Snap",
        RailItem::Preroll => "Pre-roll",
        RailItem::Metronome => "Count-in",
        RailItem::RampArm => "Ramp",
        RailItem::RampStart => "Ramp start",
        RailItem::RampTarget => "Ramp target",
        RailItem::RampStep => "Ramp step",
        RailItem::RampThreshold => "Ramp pass",
        RailItem::RampStreak => "Ramp streak",
        RailItem::WaitMode => "Wait",
    }
}

/// Right-column value text for a rail row; empty for pure action rows.
pub fn rail_row_value(item: RailItem, session: &PracticeSession) -> String {
    match item {
        RailItem::Resume
        | RailItem::RestartSection
        | RailItem::SetA
        | RailItem::SetB
        | RailItem::ClearLoop => String::new(),
        RailItem::Scrub => match session.transport.scrub_cursor_ms {
            Some(ms) => format_chart_time(ms),
            None => "—".into(),
        },
        RailItem::Rate => {
            if session.trainer.ramp.armed {
                format!(
                    "x{:.2} (ramp x{:.2})",
                    session.transport.user_tempo, session.trainer.ramp.step_tempo
                )
            } else {
                format!("x{:.2}", session.transport.user_tempo)
            }
        }
        RailItem::Snap => session.transport.snap.label().into(),
        RailItem::Preroll => session.transport.preroll.label(),
        RailItem::Metronome => if session.transport.metronome { "on" } else { "off" }.into(),
        RailItem::RampArm => {
            if session.trainer.ramp.armed {
                let (cur, total) = crate::practice::ramp::ramp_step_index(
                    &session.trainer.ramp_config,
                    session.transport.user_tempo,
                );
                format!("ON {cur}/{total}")
            } else {
                "off".into()
            }
        }
        RailItem::RampStart => format!("x{:.2}", session.trainer.ramp_config.start_tempo),
        RailItem::RampTarget => format!("x{:.2}", session.trainer.ramp_config.target_tempo),
        RailItem::RampStep => format!("+{:.2}", session.trainer.ramp_config.step),
        RailItem::RampThreshold => {
            format!("≥{:.0}%", session.trainer.ramp_config.threshold_pct)
        }
        RailItem::RampStreak => format!("×{}", session.trainer.ramp_config.required_successes),
    }
}

/// Left/Right adjustment for `item` (`dir` = ±1). Shared by keyboard
/// arrows and the ◂/▸ mouse buttons — one code path for both.
pub fn adjust_rail_item(
    item: RailItem,
    dir: i8,
    session: &mut PracticeSession,
    timeline: &ChipTimeline,
    current_ms: i64,
) {
    match item {
        RailItem::Scrub => {
            let cur = session
                .transport
                .scrub_cursor_ms
                .unwrap_or(current_ms);
            session.transport.scrub_cursor_ms =
                Some(timeline.snap_neighbor(cur, session.transport.snap, dir));
        }
        RailItem::Rate => session.step_user_tempo(dir),
        RailItem::Snap => session.transport.snap = session.transport.snap.next(),
        RailItem::Preroll => session.transport.preroll = session.transport.preroll.next(),
        RailItem::RampStart => {
            let c = &mut session.trainer.ramp_config;
            c.start_tempo = (c.start_tempo + dir as f32 * 0.05).clamp(0.5, c.target_tempo - 0.05);
            let cfg = session.trainer.ramp_config;
            crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
        }
        RailItem::RampTarget => {
            let c = &mut session.trainer.ramp_config;
            c.target_tempo = (c.target_tempo + dir as f32 * 0.05).clamp(c.start_tempo + 0.05, 1.5);
            let cfg = session.trainer.ramp_config;
            crate::practice::ramp::clamp_to_config(&cfg, &mut session.trainer.ramp);
        }
        RailItem::RampStep => {
            let c = &mut session.trainer.ramp_config;
            c.step = (c.step + dir as f32 * 0.05).clamp(0.05, 0.25);
        }
        RailItem::RampThreshold => {
            let c = &mut session.trainer.ramp_config;
            c.threshold_pct = (c.threshold_pct + dir as f32 * 5.0).clamp(50.0, 100.0);
        }
        RailItem::RampStreak => {
            let c = &mut session.trainer.ramp_config;
            c.required_successes = (c.required_successes as i8 + dir).clamp(1, 3) as u8;
        }
        _ => {}
    }
}

/// Enter/Space (or row-click) activation for `item`. Shared by keyboard
/// and mouse. Row semantics are unchanged from the v1 rail.
#[allow(clippy::too_many_arguments)]
pub fn activate_rail_item(
    item: RailItem,
    session: &mut PracticeSession,
    timeline: &ChipTimeline,
    current_ms: i64,
    wait_state: Option<&mut crate::practice::wait::WaitState>,
    chord_hits: Option<&mut crate::practice::wait::ChordHitTimes>,
    next_pause: &mut NextState<PauseState>,
    seeks: &mut MessageWriter<SeekToChartTime>,
    practice_actions: &mut MessageWriter<crate::practice::actions::PracticeAction>,
) {
    match item {
        RailItem::Resume => next_pause.set(PauseState::Running),
        RailItem::Scrub => {
            let intent = session
                .transport
                .scrub_cursor_ms
                .unwrap_or(current_ms);
            seeks.write(SeekToChartTime {
                target_ms: preroll_target(timeline, session.transport.preroll, intent),
                snap: None,
                attempt_start_ms: Some(intent),
            });
            next_pause.set(PauseState::Running);
        }
        RailItem::RestartSection => {
            let intent = session
                .transport
                .loop_region
                .map(|r| r.start_ms)
                .unwrap_or(session.current_attempt.start_ms);
            seeks.write(SeekToChartTime {
                target_ms: preroll_target(timeline, session.transport.preroll, intent),
                snap: None,
                attempt_start_ms: Some(intent),
            });
            next_pause.set(PauseState::Running);
        }
        RailItem::SetA => {
            let ms = timeline.bar_start_before(
                session
                    .transport
                    .scrub_cursor_ms
                    .unwrap_or(current_ms),
            );
            session.set_loop_start(ms);
        }
        RailItem::SetB => {
            let cursor = session
                .transport
                .scrub_cursor_ms
                .unwrap_or(current_ms);
            let mut ms = timeline.bar_start_before(cursor);
            if let Some(r) = session.transport.loop_region {
                if ms <= r.start_ms {
                    ms = timeline.snap_neighbor(r.start_ms, crate::timeline::SnapDivisor::Bar, 1);
                }
            }
            session.set_loop_end(ms);
        }
        RailItem::ClearLoop => session.clear_loop(),
        RailItem::Metronome => {
            session.transport.metronome = !session.transport.metronome;
        }
        RailItem::RampArm => {
            practice_actions.write(crate::practice::actions::PracticeAction::ToggleRamp);
        }
        RailItem::WaitMode => {
            session.trainer.wait_enabled = !session.trainer.wait_enabled;
            if session.trainer.wait_enabled && session.trainer.ramp.armed {
                session.trainer.ramp.armed = false;
            }
            if session.trainer.wait_enabled {
                if let (Some(wait_state), Some(chord_hits)) = (wait_state, chord_hits) {
                    wait_state.begin(current_ms);
                    chord_hits.0.clear();
                }
            }
        }
        RailItem::Rate
        | RailItem::Snap
        | RailItem::Preroll
        | RailItem::RampStart
        | RailItem::RampTarget
        | RailItem::RampStep
        | RailItem::RampThreshold
        | RailItem::RampStreak => {}
    }
}
```

  Then rewrite `full_hud_input` to delegate (keyboard semantics unchanged; the per-frame label/selection render tail stays here until Task 5 replaces it with `refresh_rail`):

```rust
/// Keyboard nav for the rail: Up/Down select, Left/Right adjust,
/// Enter/Space activate. Mouse shares the same helpers (Task 6).
#[allow(clippy::too_many_arguments)]
pub fn full_hud_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<RailSelection>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut wait_state: Option<ResMut<crate::practice::wait::WaitState>>,
    mut chord_hits: Option<ResMut<crate::practice::wait::ChordHitTimes>>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
    mut rows: Query<(&RailItem, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<RailItem>)>,
    mut diag_text: Query<
        &mut Text,
        (
            With<LaneDiagnosisText>,
            Without<RailItem>,
            Without<AttemptHistoryText>,
        ),
    >,
) {
    let count = RailItem::ORDER.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
    }
    let selected = RailItem::ORDER[selection.0];

    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let right = keys.just_pressed(KeyCode::ArrowRight);
    if left || right {
        let dir: i8 = if right { 1 } else { -1 };
        adjust_rail_item(selected, dir, &mut session, &timeline, clock.current_ms);
    }

    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        activate_rail_item(
            selected,
            &mut session,
            &timeline,
            clock.current_ms,
            wait_state.as_deref_mut(),
            chord_hits.as_deref_mut(),
            &mut next_pause,
            &mut seeks,
            &mut practice_actions,
        );
    }

    // Render tail (replaced by refresh_rail in the ref-px rebuild task).
    let theme = Theme::default();
    for (item, mut text, mut color) in &mut rows {
        text.0 = rail_label(*item, &session);
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_secondary
        };
    }
    if let Ok(mut t) = history.single_mut() {
        t.0 = attempt_history_text(&session, timeline.end_ms);
    }
    if let Ok(mut t) = diag_text.single_mut() {
        t.0 = crate::practice::diagnosis::diagnosis_text(&session.lane_diag);
    }
}
```

- [ ] **Run:** `cargo test -p gameplay-drums` — all green (`rail_clear_loop_disarms_the_ramp` in `tests/practice_hud.rs` now exercises the Enter → `activate_rail_item` path and must still pass).
- [ ] **Commit:** `refactor(practice): extract rail adjust/activate helpers and row label/value fns`

---

## Task 5 — Ref-px rail + timeline rebuild (geometry, typography, row anatomy, z)

**Files:**
- `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- `crates/gameplay-drums/src/practice/hud/mod.rs`
- `crates/gameplay-drums/tests/practice_hud.rs`

- [ ] **Write failing tests.** In `full_hud.rs` tests module:

```rust
    #[test]
    fn rail_fixed_content_fits_720_reference_height() {
        // Spec fit check: headers + rows + gaps + padding at scale 1.0 must
        // leave room inside the 648 ref-px band above the timeline row.
        let h = rail_fixed_content_height(1.0);
        assert!(
            h < RAIL_REF_HEIGHT,
            "rail fixed content {h} ref-px must fit {RAIL_REF_HEIGHT}"
        );
    }
```

  In `tests/practice_hud.rs`:

```rust
use gameplay_drums::practice::hud::full_hud::{RailAdjustButton, RailRowButton};

#[test]
fn rail_spawns_17_rows_with_adjust_buttons_at_practice_z() {
    let mut app = build_app();
    app.world_mut().insert_resource(PracticeSession::default());
    set_rail_surface(&mut app);
    set_paused(&mut app, true);

    let rows = app
        .world_mut()
        .query::<&RailRowButton>()
        .iter(app.world())
        .count();
    assert_eq!(rows, 17, "one clickable row per RailItem");

    let adjusts = app
        .world_mut()
        .query::<&RailAdjustButton>()
        .iter(app.world())
        .count();
    assert_eq!(adjusts, 18, "9 value rows x (◂ + ▸)");

    let z = app
        .world_mut()
        .query::<(&FullHudRoot, &GlobalZIndex)>()
        .iter(app.world())
        .map(|(_, z)| z.0)
        .next()
        .expect("full HUD root has a GlobalZIndex");
    assert_eq!(z, 1000, "ui_z::PRACTICE_FULL_HUD");
}
```

- [ ] **Run:** `cargo test -p gameplay-drums rail_spawns_17` — expected failure: compile error `cannot find type 'RailRowButton'` (and `rail_fixed_content_height` unresolved).

- [ ] **Implement.** In `full_hud.rs`:

  **Imports and constants** — extend the imports and add geometry constants near the top:

```rust
use dtx_ui::theme::{Theme, REF_HEIGHT, REF_WIDTH};
use dtx_ui::widget::hud_ref::{scaled_font, HudRefRect};

/// Rail geometry in ref-px (1280x720 reference space, scaled by
/// `PlayfieldLayout::scale`). The rail sits flush with the ref right edge
/// (identical to `right: 0` at 16:9) so it scales with the Now-Playing
/// card by construction — no collision at 1080p, no overflow at 720p.
pub const RAIL_REF_WIDTH: f32 = 300.0;
pub const TIMELINE_REF_HEIGHT: f32 = 72.0;
pub const RAIL_REF_LEFT: f32 = REF_WIDTH - RAIL_REF_WIDTH;
pub const RAIL_REF_HEIGHT: f32 = REF_HEIGHT - TIMELINE_REF_HEIGHT;
pub const RAIL_REF_PAD: f32 = 12.0;
pub const ROW_REF_HEIGHT: f32 = 22.0;
pub const ROW_REF_GAP: f32 = 4.0;
pub const HEADER_REF_FONT: f32 = 11.0;
pub const HEADER_REF_TOP_MARGIN: f32 = 8.0;
pub const ROW_REF_FONT: f32 = 16.0;
pub const SMALL_REF_FONT: f32 = 12.0;

/// Fixed rail content height (headers + rows + gaps + padding) in px at
/// `scale`. Attempt history + lane diagnosis render in the leftover band
/// and are clipped by the rail container when they run long.
pub fn rail_fixed_content_height(scale: f32) -> f32 {
    let headers = 3.0 * (HEADER_REF_FONT * 1.2 + HEADER_REF_TOP_MARGIN);
    let rows = RailItem::ORDER.len() as f32 * ROW_REF_HEIGHT;
    let gaps = (3 + RailItem::ORDER.len() - 1) as f32 * ROW_REF_GAP;
    (headers + rows + gaps + 2.0 * RAIL_REF_PAD) * scale
}
```

  (Sanity: 3×(13.2+8) + 17×22 + 19×4 + 24 = 63.6 + 374 + 76 + 24 = 537.6 < 648.)

  **New components** — add next to the marker components:

```rust
/// Whole-row click target: click selects (and activates non-value rows).
#[derive(Component)]
pub struct RailRowButton(pub RailItem);

/// ◂ / ▸ adjust glyph: `1` field is the direction (−1 / +1).
#[derive(Component)]
pub struct RailAdjustButton(pub RailItem, pub i8);

/// Right-column value text of a row (rewritten every frame by `refresh_rail`).
#[derive(Component)]
pub struct RailValueText(pub RailItem);
```

  Drop `Component` from `RailItem`'s derive (it is no longer inserted as a component anywhere after this task): `#[derive(Clone, Copy, PartialEq, Eq, Debug)]`.

  **Replace `spawn_full_hud` and add the two spawn helpers** (dual-write convention from `now_playing.rs`: `HudRefRect` + initial `Node` values at current scale; `apply_hud_ref_layout` re-applies on resize; fonts are spawn-scaled like every other HUD widget — they refresh on the next rail open after a resize):

```rust
pub fn spawn_full_hud(
    mut commands: Commands,
    mut selection: ResMut<RailSelection>,
    mut session: ResMut<PracticeSession>,
    clock: Res<GameplayClock>,
    timeline: Res<ChipTimeline>,
    layout: Option<Res<crate::layout::PlayfieldLayout>>,
) {
    selection.0 = 0;
    session.transport.scrub_cursor_ms = Some(clock.current_ms);
    // Missing layout (headless tests) falls back to identity — never panic.
    let (scale, origin) = layout
        .map(|l| (l.scale, l.origin))
        .unwrap_or((1.0, Vec2::ZERO));
    let theme = Theme::default();
    commands
        .spawn((
            FullHudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(crate::ui_z::PRACTICE_FULL_HUD),
        ))
        .with_children(|root| {
            spawn_rail(root, &theme, scale, origin, &session, &timeline);
            spawn_timeline_row(root, &theme, scale, origin, &clock, &timeline);
        });
}

fn spawn_rail(
    root: &mut ChildSpawnerCommands,
    theme: &Theme,
    scale: f32,
    origin: Vec2,
    session: &PracticeSession,
    timeline: &ChipTimeline,
) {
    let rail_rect = HudRefRect::new(RAIL_REF_LEFT, 0.0, RAIL_REF_WIDTH, RAIL_REF_HEIGHT);
    let mut rail_node = Node {
        position_type: PositionType::Absolute,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(ROW_REF_GAP * scale),
        padding: UiRect::all(Val::Px(RAIL_REF_PAD * scale)),
        // ponytail: worst-case history/diag overflow clips at the rail
        // bottom; add a scroll view only if players actually hit it.
        overflow: Overflow::clip_y(),
        ..default()
    };
    rail_rect.apply(scale, origin, &mut rail_node);
    root.spawn((
        rail_rect,
        rail_node,
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
    ))
    .with_children(|rail| {
        for (idx, item) in RailItem::ORDER.iter().enumerate() {
            let header = match idx {
                0 => Some("TRANSPORT"),
                7 => Some("LOOP"),
                10 => Some("TRAINER"),
                _ => None,
            };
            if let Some(h) = header {
                rail.spawn((
                    Text::new(h),
                    scaled_font(scale, HEADER_REF_FONT),
                    TextColor(theme.text_secondary),
                    Node {
                        margin: UiRect::top(Val::Px(HEADER_REF_TOP_MARGIN * scale)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            }
            spawn_rail_row(rail, theme, scale, *item, session);
        }
        rail.spawn((
            AttemptHistoryText,
            Text::new(attempt_history_text(session, timeline.end_ms)),
            scaled_font(scale, SMALL_REF_FONT),
            TextColor(theme.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(12.0 * scale)),
                max_width: Val::Px((RAIL_REF_WIDTH - 2.0 * RAIL_REF_PAD) * scale),
                flex_shrink: 0.0,
                ..default()
            },
        ));
        rail.spawn((
            LaneDiagnosisText,
            Text::new(crate::practice::diagnosis::diagnosis_text(
                &session.lane_diag,
            )),
            scaled_font(scale, SMALL_REF_FONT),
            TextColor(theme.text_secondary),
            Node {
                margin: UiRect::top(Val::Px(12.0 * scale)),
                max_width: Val::Px((RAIL_REF_WIDTH - 2.0 * RAIL_REF_PAD) * scale),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    });
}

fn spawn_rail_row(
    rail: &mut ChildSpawnerCommands,
    theme: &Theme,
    scale: f32,
    item: RailItem,
    session: &PracticeSession,
) {
    rail.spawn((
        RailRowButton(item),
        Button,
        Node {
            height: Val::Px(ROW_REF_HEIGHT * scale),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0 * scale),
            padding: UiRect::horizontal(Val::Px(4.0 * scale)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(Color::NONE),
    ))
    .with_children(|row| {
        row.spawn((
            Text::new(rail_row_label(item)),
            scaled_font(scale, ROW_REF_FONT),
            TextColor(theme.text_primary),
        ));
        if rail_row_kind(item) == RowKind::Value {
            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0 * scale),
                ..default()
            })
            .with_children(|value| {
                value
                    .spawn((
                        RailAdjustButton(item, -1),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(4.0 * scale), Val::Px(1.0 * scale)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("◂"),
                            scaled_font(scale, ROW_REF_FONT),
                            TextColor(theme.text_secondary),
                        ));
                    });
                value.spawn((
                    RailValueText(item),
                    Text::new(rail_row_value(item, session)),
                    scaled_font(scale, ROW_REF_FONT),
                    TextColor(theme.text_primary),
                ));
                value
                    .spawn((
                        RailAdjustButton(item, 1),
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(4.0 * scale), Val::Px(1.0 * scale)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("▸"),
                            scaled_font(scale, ROW_REF_FONT),
                            TextColor(theme.text_secondary),
                        ));
                    });
            });
        } else {
            row.spawn((
                RailValueText(item),
                Text::new(rail_row_value(item, session)),
                scaled_font(scale, ROW_REF_FONT),
                TextColor(theme.text_primary),
            ));
        }
    });
}
```

```rust
fn spawn_timeline_row(
    root: &mut ChildSpawnerCommands,
    theme: &Theme,
    scale: f32,
    origin: Vec2,
    clock: &GameplayClock,
    timeline: &ChipTimeline,
) {
    // Width 0 in the ref rect = "don't write width": the node stretches
    // window-wide via left+right. Top-anchored at ref 648 so the rail's
    // bottom edge and the timeline's top edge coincide at every scale.
    let row_rect = HudRefRect::new(0.0, RAIL_REF_HEIGHT, 0.0, TIMELINE_REF_HEIGHT);
    let mut row_node = Node {
        position_type: PositionType::Absolute,
        right: Val::Px(0.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(12.0 * scale),
        padding: UiRect::horizontal(Val::Px(12.0 * scale)),
        ..default()
    };
    row_rect.apply(scale, origin, &mut row_node);
    root.spawn((
        row_rect,
        row_node,
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
    ))
    .with_children(|row| {
        for button in [
            TransportButton::PrevBar,
            TransportButton::Resume,
            TransportButton::NextBar,
        ] {
            row.spawn((
                button,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(10.0 * scale), Val::Px(4.0 * scale)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(button.label()),
                    scaled_font(scale, SMALL_REF_FONT),
                    TextColor(theme.text_primary),
                ));
            });
        }
        row.spawn((
            HudTimeText,
            Text::new(format_chart_time(clock.current_ms)),
            scaled_font(scale, ROW_REF_FONT),
            TextColor(theme.text_primary),
        ));
        let strip = spawn_density_strip(row, &timeline.density, theme);
        row.commands().entity(strip).insert(FullHudTimelineStrip);
        row.commands().entity(strip).with_children(|markers| {
            // Bar ticks along the top edge (1px hairline stays device-px).
            for &bar in &timeline.bar_ms {
                markers.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(time_to_pct(bar, timeline.end_ms)),
                        top: Val::Px(0.0),
                        width: Val::Px(1.0),
                        height: Val::Px(8.0 * scale),
                        ..default()
                    },
                    BackgroundColor(theme.text_secondary.with_alpha(0.6)),
                ));
            }
            markers.spawn((
                HudLoopFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 0.9, 0.5, 0.25)),
                Visibility::Hidden,
            ));
            markers.spawn((
                HudPlayhead,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(theme.accent),
            ));
            markers.spawn((
                HudScrubCursor,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                Visibility::Hidden,
            ));
        });
    });
}
```

  Deliberate deletions/changes vs. the old spawn (call out in the commit body):
  - `GlobalZIndex(1000)` literal → `crate::ui_z::PRACTICE_FULL_HUD` (same value, now registered).
  - The 340-screen-px rail, `justify_content: Center`, and the "PRACTICE" title row are gone — the spec's typography/fit model has 3 headers + 17 rows only.
  - `Theme::hud_font()` (32px) rows → `scaled_font(scale, 16.0)`; headers → 11; history/diag/buttons → 12; time text 32px → `scaled_font(scale, 16.0)`.
  - Timeline row is top-anchored at ref 648 instead of `bottom: 0` — identical at 16:9; on non-16:9 it stays glued to the rail's bottom edge (ref-space convention).

  **Add `refresh_rail`** (replaces `full_hud_input`'s render tail):

```rust
/// Re-render selection highlight + row values each frame while the rail is
/// open. Selected row: `selection_highlight` background + accent value.
#[allow(clippy::type_complexity)]
pub fn refresh_rail(
    selection: Res<RailSelection>,
    session: Res<PracticeSession>,
    timeline: Res<ChipTimeline>,
    mut rows: Query<(&RailRowButton, &mut BackgroundColor)>,
    mut values: Query<(&RailValueText, &mut Text, &mut TextColor)>,
    mut history: Query<&mut Text, (With<AttemptHistoryText>, Without<RailValueText>)>,
    mut diag_text: Query<
        &mut Text,
        (
            With<LaneDiagnosisText>,
            Without<RailValueText>,
            Without<AttemptHistoryText>,
        ),
    >,
) {
    let theme = Theme::default();
    let selected = RailItem::ORDER[selection.0 % RailItem::ORDER.len()];
    for (RailRowButton(item), mut bg) in &mut rows {
        bg.0 = if *item == selected {
            theme.selection_highlight
        } else {
            Color::NONE
        };
    }
    for (RailValueText(item), mut text, mut color) in &mut values {
        text.0 = rail_row_value(*item, &session);
        color.0 = if *item == selected {
            theme.accent
        } else {
            theme.text_primary
        };
    }
    if let Ok(mut t) = history.single_mut() {
        t.0 = attempt_history_text(&session, timeline.end_ms);
    }
    if let Ok(mut t) = diag_text.single_mut() {
        t.0 = crate::practice::diagnosis::diagnosis_text(&session.lane_diag);
    }
}
```

  **Slim `full_hud_input`**: delete the `rows` / `history` / `diag_text` params and the whole render tail from the Task-4 version (everything after the Enter/Space block). Delete `rail_label` and its remaining callers/tests (its behavior now lives in `rail_row_label` + `rail_row_value`).

  **Register** in `practice/hud/mod.rs` — the Update chain becomes:

```rust
            (
                timeline_ui::timeline_mouse,
                full_hud::full_hud_input,
                full_hud::transport_buttons,
                full_hud::refresh_rail,
                full_hud::update_full_hud_markers,
            )
```

  (same `.chain()` + run conditions as Task 3).

- [ ] **Run:** `cargo test -p gameplay-drums` — all green. Also `cargo clippy -p gameplay-drums --all-targets -- -D warnings` (the rewrite touches many query types).
- [ ] **Commit:** `feat(practice): ref-px scaled rail with typographic hierarchy and registered z`

---

## Task 6 — Mouse-operable rows: `rail_mouse` system

**Files:**
- `crates/gameplay-drums/src/practice/hud/full_hud.rs`
- `crates/gameplay-drums/src/practice/hud/mod.rs`
- `crates/gameplay-drums/tests/practice_hud.rs`

- [ ] **Write failing tests.** In `tests/practice_hud.rs` (reuse the 2-bar chart recipe from `next_bar_button_moves_scrub_cursor`):

```rust
use gameplay_drums::practice::hud::full_hud::rail_mouse;

fn two_bar_timeline() -> ChipTimeline {
    // 2 bars @ 120 BPM: bar starts at 0 and 2000.
    let chart = dtx_core::chart::Chart {
        metadata: dtx_core::chart::Metadata {
            bpm: Some(120.0),
            ..Default::default()
        },
        chips: vec![dtx_core::chart::Chip::new(
            0,
            dtx_core::channel::EChannel::BassDrum,
            0.0,
        )],
        ..Default::default()
    };
    let bpm = gameplay_drums::judge::BpmChangeList::from_chart(&chart);
    let bar = gameplay_drums::judge::BarLengthChangeList::from_chart(&chart);
    ChipTimeline::from_chart(&chart, &bpm, &bar, 0, 4_000)
}

#[test]
fn adjust_button_click_steps_tempo_and_moves_selection() {
    let mut app = build_app();
    app.add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .add_systems(Update, rail_mouse);
    app.world_mut().insert_resource(PracticeSession::default());
    app.world_mut()
        .spawn((Interaction::Pressed, RailAdjustButton(RailItem::Rate, 1)));
    app.update();

    let session = app.world().resource::<PracticeSession>();
    assert!(
        (session.transport.user_tempo - 1.05).abs() < 1e-6,
        "▸ on Tempo steps +0.05 like ArrowRight"
    );
    let rate_idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::Rate)
        .expect("Rate is a rail row");
    assert_eq!(
        app.world().resource::<RailSelection>().0,
        rate_idx,
        "mouse click moves the shared selection cursor"
    );
}

#[test]
fn row_click_selects_and_activates_set_a() {
    let mut app = build_app();
    app.add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .add_systems(Update, rail_mouse);
    app.world_mut().insert_resource(two_bar_timeline());
    app.world_mut().insert_resource(PracticeSession {
        transport: PracticeTransport {
            scrub_cursor_ms: Some(2_500),
            ..Default::default()
        },
        ..Default::default()
    });
    app.world_mut()
        .spawn((Interaction::Pressed, RailRowButton(RailItem::SetA)));
    app.update();

    let session = app.world().resource::<PracticeSession>();
    assert_eq!(
        session.transport.loop_region.map(|r| r.start_ms),
        Some(2_000),
        "row click on Set A snaps the loop start to the bar"
    );
    let a_idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::SetA)
        .expect("SetA is a rail row");
    assert_eq!(app.world().resource::<RailSelection>().0, a_idx);
}

#[test]
fn value_row_click_selects_without_acting() {
    let mut app = build_app();
    app.add_message::<gameplay_drums::seek::SeekToChartTime>()
        .add_message::<gameplay_drums::practice::actions::PracticeAction>()
        .add_systems(Update, rail_mouse);
    app.world_mut().insert_resource(PracticeSession::default());
    app.world_mut()
        .spawn((Interaction::Pressed, RailRowButton(RailItem::Scrub)));
    app.update();

    // Selection moved, but no seek was written (Scrub activation = "play here").
    let scrub_idx = RailItem::ORDER
        .iter()
        .position(|i| *i == RailItem::Scrub)
        .expect("Scrub is a rail row");
    assert_eq!(app.world().resource::<RailSelection>().0, scrub_idx);
    let seeks = app
        .world()
        .resource::<bevy::ecs::message::Messages<gameplay_drums::seek::SeekToChartTime>>()
        .iter_current_update_messages()
        .count();
    assert_eq!(seeks, 0, "value-row click must not trigger play-here");
}
```

  (`build_app` registers states/resources only, not the seek/action messages — hence the explicit `add_message` calls above; `add_message` is idempotent if `build_app` later grows them.)

- [ ] **Run:** `cargo test -p gameplay-drums rail_mouse` (name filter `adjust_button_click`) — expected failure: compile error `cannot find function 'rail_mouse'`.

- [ ] **Implement.** In `full_hud.rs`, add:

```rust
/// Mouse path for the rail: row click selects (and activates action/toggle
/// rows); ◂/▸ click adjusts. Same helpers as the keyboard path, and every
/// click moves `RailSelection` so both inputs share one cursor.
#[allow(clippy::too_many_arguments)]
pub fn rail_mouse(
    row_clicks: Query<(&Interaction, &RailRowButton), Changed<Interaction>>,
    adjust_clicks: Query<(&Interaction, &RailAdjustButton), Changed<Interaction>>,
    mut selection: ResMut<RailSelection>,
    mut session: ResMut<PracticeSession>,
    timeline: Res<ChipTimeline>,
    clock: Res<GameplayClock>,
    mut wait_state: Option<ResMut<crate::practice::wait::WaitState>>,
    mut chord_hits: Option<ResMut<crate::practice::wait::ChordHitTimes>>,
    mut next_pause: ResMut<NextState<PauseState>>,
    mut seeks: MessageWriter<SeekToChartTime>,
    mut practice_actions: MessageWriter<crate::practice::actions::PracticeAction>,
) {
    for (interaction, RailRowButton(item)) in &row_clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(idx) = RailItem::ORDER.iter().position(|i| i == item) {
            selection.0 = idx;
        }
        // Action rows: click = select + act. Toggle rows: click = select +
        // flip. Value rows (incl. Scrub): click = select only — adjusting
        // is the ◂/▸ buttons' job, and Scrub's activation is "play here".
        if rail_row_kind(*item) != RowKind::Value {
            activate_rail_item(
                *item,
                &mut session,
                &timeline,
                clock.current_ms,
                wait_state.as_deref_mut(),
                chord_hits.as_deref_mut(),
                &mut next_pause,
                &mut seeks,
                &mut practice_actions,
            );
        }
    }
    for (interaction, RailAdjustButton(item, dir)) in &adjust_clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(idx) = RailItem::ORDER.iter().position(|i| i == item) {
            selection.0 = idx;
        }
        adjust_rail_item(*item, *dir, &mut session, &timeline, clock.current_ms);
    }
}
```

  Register it in `practice/hud/mod.rs`'s chain, after keyboard input and before `transport_buttons`:

```rust
            (
                timeline_ui::timeline_mouse,
                full_hud::full_hud_input,
                full_hud::rail_mouse,
                full_hud::transport_buttons,
                full_hud::refresh_rail,
                full_hud::update_full_hud_markers,
            )
```

- [ ] **Run:** `cargo test -p gameplay-drums` — all green.
- [ ] **Commit:** `feat(practice): mouse-operable rail rows and adjust buttons sharing RailSelection`

---

## Task 7 — Final gates + acceptance sweep

**Files:** none (verification only; fixups if gates fail)

- [ ] **Leftover scan** (all must return nothing):

```
grep -rn "ExitArmed" crates/
grep -rn "GlobalZIndex(1000)" crates/gameplay-drums/src/practice/
grep -rn "rail_label" crates/
grep -rn "Val::Px(340" crates/gameplay-drums/src/practice/
```

- [ ] **Gates:**

```
cargo fmt --all
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p gameplay-drums
```

- [ ] Fix anything the gates surface; if fixes were needed, commit as `fix(practice): gate fixups for rail/pause unification` (no co-author trailers).
- [ ] Runtime smoke (BRP) is done by the controller afterward, per the spec's Testing section: enter practice → Esc → overlay rows work (SD resumes) → Exit Practice lands in song select; re-enter → Tab → rail renders scaled, `▸` on Tempo steps the value, Set A row click sets the loop.

---

## Acceptance criteria → task map

| # | Criterion (spec) | Covered by |
|---|---|---|
| 1 | Esc in practice opens pause overlay (Resume / Restart loop / Exit Practice), keyboard + pad nav, SD resumes | Tasks 1–2 (`pause_items_*`, dispatch tests, `overlay_spawns_in_practice_on_overlay_surface`; pad path is the untouched `NavAction` consumer) |
| 2 | Normal-play pause overlay unchanged | Task 2: `pause_items(false)` pins Resume/Retry/Quit; dispatch arms byte-identical; existing pause-path tests keep passing (`cargo test -p gameplay-drums` gate every task) |
| 3 | Tab opens the rail; rail scales with window (no Now-Playing spill at 1080p, no overflow at 720p) | Tasks 1, 3, 5 (`tab_opener_sets_rail_surface_and_pauses`, `hud_plugin_overlay_surface_spawns_no_rail`, `rail_fixed_content_fits_720_reference_height`; ref-px + unscoped `apply_hud_ref_layout` give resize for free) |
| 4 | Every rail row mouse- and keyboard-operable with one shared selection | Tasks 4–6 (shared `adjust_rail_item`/`activate_rail_item`, `rail_mouse` tests assert `RailSelection` moves) |
| 5 | `ExitArmed` gone; no double-Enter exit anywhere | Task 3 (variant + resource + arming logic deleted; grep gate in Task 7) |
| 6 | Full gates green | Task 7 |

Spec error-handling: `PlayfieldLayout` missing → scale 1.0 fallback (Task 5, `Option<Res<...>>`); loop/ramp placeholder labels → `rail_row_value` renders "—"/"off" (Task 4). Mini-strip quick-key legend: untouched (Tab still opens the rail).

## Verification

```
cargo fmt --all && cargo check --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p gameplay-drums
```

All four must pass. Runtime BRP smoke (overlay verbs, rail scaling, mouse clicks, exit-to-song-select) is executed by the controller after the plan completes (BRP driving note: `move_mouse` before `click_mouse` is mandatory).
