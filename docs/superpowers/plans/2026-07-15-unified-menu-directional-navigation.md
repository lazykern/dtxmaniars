# Unified Menu Directional Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Song Select and Song Ready consistently use Left/Right for horizontal focus and Up/Down for the focused selection or value, while preserving current pad behavior and fixing Ready's visual overlap.

**Architecture:** Keep the shared `NavAction` API unchanged. Add small pure reducers local to `game-menu` for input decisions, let the Bevy systems apply their effects to `Selection` and `ReadyConfigDraft`, and reuse the existing per-card config merge/save path. Retain `SongReadyLayer::Edit` and `PrimaryDetail` only for pad transactions.

**Tech Stack:** Rust 1.95+, Bevy 0.19 ECS/UI, existing `game-shell` navigation messages, `dtx-config`, `dtx-ui`, package-local Rust tests.

## Global Constraints

- Modify only `crates/game-menu/src/song_select.rs` and `crates/game-menu/src/song_ready.rs`.
- Do not modify `NavAction`, `NavVerb`, `NavSource`, shared input architecture, MIDI mappings, Practice Setup, or practice gameplay.
- Preserve `Selection.difficulty` as the only difficulty source of truth.
- Preserve current pad Browse/Edit/PrimaryDetail behavior.
- Preserve the existing silenced entity-command cleanup added in commit `8527184`.
- Preserve the user's uncommitted `crates/dtx-ui/src/typography.rs` change; execute from an isolated worktree created at implementation time.
- Keep the five Ready cards on one horizontal strip at 1280×720 and 1920×1080.

---

### Task 1: Lock Song Select's directional contract

**Files:**
- Modify: `crates/game-menu/src/song_select.rs:1756-1786`
- Modify: `crates/game-menu/src/song_select.rs:2197-2293`
- Test: `crates/game-menu/src/song_select.rs` inline `tests` module

**Interfaces:**
- Consumes: `SongSelectFocus`, `NavSource`, `NavVerb`, `ReadyMode`.
- Produces: `fn ready_mode_for_action(source: NavSource, focus: SongSelectFocus, verb: NavVerb) -> Option<ReadyMode>` used by `song_select_nav_consumer`.

- [ ] **Step 1: Add failing tests for Ready entry from either keyboard region**

Add these assertions beside `song_select_focus_regions_follow_keyboard_geometry`:

```rust
#[test]
fn keyboard_ready_entry_is_available_from_both_focus_regions() {
    for focus in [SongSelectFocus::Songs, SongSelectFocus::Difficulty] {
        assert_eq!(
            ready_mode_for_action(NavSource::Keyboard, focus, NavVerb::Confirm),
            Some(crate::song_ready::ReadyMode::Normal)
        );
        assert_eq!(
            ready_mode_for_action(NavSource::Keyboard, focus, NavVerb::Practice),
            Some(crate::song_ready::ReadyMode::Practice)
        );
    }
}

#[test]
fn pad_ready_entry_still_requires_difficulty_focus() {
    assert_eq!(
        ready_mode_for_action(
            NavSource::Pad,
            SongSelectFocus::Songs,
            NavVerb::Confirm,
        ),
        None
    );
    assert_eq!(
        ready_mode_for_action(
            NavSource::Pad,
            SongSelectFocus::Difficulty,
            NavVerb::Confirm,
        ),
        Some(crate::song_ready::ReadyMode::Normal)
    );
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cargo test -p game-menu --lib keyboard_ready_entry_is_available_from_both_focus_regions
```

Expected: compilation fails because `ready_mode_for_action` does not exist.

- [ ] **Step 3: Extract the existing entry decision into the pure helper**

Add `NavSource` and `NavVerb` to the module-level `game_shell` import, then add
this helper immediately before `song_select_nav_consumer`:

```rust
fn ready_mode_for_action(
    source: NavSource,
    focus: SongSelectFocus,
    verb: NavVerb,
) -> Option<crate::song_ready::ReadyMode> {
    match (source, focus, verb) {
        (NavSource::Keyboard, _, NavVerb::Confirm) => {
            Some(crate::song_ready::ReadyMode::Normal)
        }
        (NavSource::Keyboard, _, NavVerb::Practice) => {
            Some(crate::song_ready::ReadyMode::Practice)
        }
        (NavSource::Pad, SongSelectFocus::Difficulty, NavVerb::Confirm) => {
            Some(crate::song_ready::ReadyMode::Normal)
        }
        (NavSource::Pad, SongSelectFocus::Difficulty, NavVerb::Practice) => {
            Some(crate::song_ready::ReadyMode::Practice)
        }
        _ => None,
    }
}
```

Replace the inline `open_mode` match with:

```rust
let open_mode = ready_mode_for_action(action.source, *focus, action.verb);
```

Do not change `SongSelectFocus::on_keyboard_verb`: Dec/Inc remain region movement and Up/Down remain selection movement.

- [ ] **Step 4: Run both contract tests and the existing focus tests**

Run:

```bash
cargo test -p game-menu --lib song_select::tests::keyboard_ready_entry_is_available_from_both_focus_regions
cargo test -p game-menu --lib song_select::tests::pad_ready_entry_still_requires_difficulty_focus
cargo test -p game-menu --lib song_select::tests::song_select_focus
```

Expected: all tests pass; existing pad two-level behavior remains green.

- [ ] **Step 5: Commit Task 1**

```bash
git add crates/game-menu/src/song_select.rs
git commit -m "test: lock song select directional navigation"
```

---

### Task 2: Make Ready keyboard Browse adjust values immediately

**Files:**
- Modify: `crates/game-menu/src/song_ready.rs:22-162`
- Modify: `crates/game-menu/src/song_ready.rs:706-864`
- Test: `crates/game-menu/src/song_ready.rs` inline `tests` module

**Interfaces:**
- Consumes: `SongReadyState`, `ReadyCard`, `AudioField`, `NavVerb`, `ReadyConfigDraft`, `NotificationQueue`.
- Produces: `ReadyKeyboardEffect`, `reduce_ready_keyboard_browse`, and `persist_ready_card_value`.

- [ ] **Step 1: Add failing reducer tests for horizontal focus and vertical changes**

Add to the Song Ready tests:

```rust
#[test]
fn keyboard_browse_uses_horizontal_focus_and_vertical_change_effects() {
    let mut state = SongReadyState::default();
    state.open(ReadyMode::Normal);

    assert_eq!(
        reduce_ready_keyboard_browse(&mut state, NavVerb::Dec),
        ReadyKeyboardEffect::None
    );
    assert_eq!(state.focus, ReadyCard::Mode);
    assert_eq!(
        reduce_ready_keyboard_browse(&mut state, NavVerb::Up),
        ReadyKeyboardEffect::AdjustValue(-1)
    );
    assert_eq!(state.layer, SongReadyLayer::Browse);
}

#[test]
fn keyboard_audio_confirm_toggles_field_without_entering_edit() {
    let mut state = SongReadyState::default();
    state.open(ReadyMode::Normal);
    state.focus = ReadyCard::Audio;
    state.audio_field = AudioField::Bgm;

    assert_eq!(
        reduce_ready_keyboard_browse(&mut state, NavVerb::Confirm),
        ReadyKeyboardEffect::None
    );
    assert_eq!(state.audio_field, AudioField::Drums);
    assert_eq!(state.layer, SongReadyLayer::Browse);
}

#[test]
fn keyboard_song_confirm_launches_and_other_cards_do_not_enter_edit() {
    let mut state = SongReadyState::default();
    state.open(ReadyMode::Normal);
    state.focus = ReadyCard::LaneSpeed;
    assert_eq!(
        reduce_ready_keyboard_browse(&mut state, NavVerb::Confirm),
        ReadyKeyboardEffect::None
    );
    assert_eq!(state.layer, SongReadyLayer::Browse);

    state.focus = ReadyCard::Song;
    assert_eq!(
        reduce_ready_keyboard_browse(&mut state, NavVerb::Confirm),
        ReadyKeyboardEffect::Launch
    );
}
```

- [ ] **Step 2: Run the reducer test and verify RED**

Run:

```bash
cargo test -p game-menu --lib keyboard_browse_uses_horizontal_focus_and_vertical_change_effects
```

Expected: compilation fails because `ReadyKeyboardEffect` and `reduce_ready_keyboard_browse` do not exist.

- [ ] **Step 3: Add the pure keyboard Browse reducer**

Define near `SongReadyState`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadyKeyboardEffect {
    None,
    AdjustValue(i32),
    AdjustDifficulty(i32),
    Launch,
    Close,
}

fn reduce_ready_keyboard_browse(
    state: &mut SongReadyState,
    verb: NavVerb,
) -> ReadyKeyboardEffect {
    match verb {
        NavVerb::Dec => {
            state.step_card(-1);
            ReadyKeyboardEffect::None
        }
        NavVerb::Inc => {
            state.step_card(1);
            ReadyKeyboardEffect::None
        }
        NavVerb::Up if state.focus == ReadyCard::Song => {
            ReadyKeyboardEffect::AdjustDifficulty(-1)
        }
        NavVerb::Down if state.focus == ReadyCard::Song => {
            ReadyKeyboardEffect::AdjustDifficulty(1)
        }
        NavVerb::Up => ReadyKeyboardEffect::AdjustValue(-1),
        NavVerb::Down => ReadyKeyboardEffect::AdjustValue(1),
        NavVerb::Confirm if state.focus == ReadyCard::Song => ReadyKeyboardEffect::Launch,
        NavVerb::Confirm if state.focus == ReadyCard::Audio => {
            state.audio_field = match state.audio_field {
                AudioField::Bgm => AudioField::Drums,
                AudioField::Drums => AudioField::Bgm,
            };
            ReadyKeyboardEffect::None
        }
        NavVerb::Confirm => ReadyKeyboardEffect::None,
        NavVerb::Back => ReadyKeyboardEffect::Close,
        _ => ReadyKeyboardEffect::None,
    }
}
```

- [ ] **Step 4: Refactor config persistence so Browse can save without leaving its layer**

Extract the body of `finish_ready_edit` into:

```rust
fn persist_ready_card_value(
    card: ReadyCard,
    draft: &mut ReadyConfigDraft,
    notifications: &mut NotificationQueue,
) -> bool {
    if card == ReadyCard::Mode || card == ReadyCard::Song {
        return true;
    }
    let path = dtx_config::default_path();
    let mut current = dtx_config::load(&path);
    merge_ready_card_config(card, &draft.config, &mut current);
    match dtx_config::save(&path, &current) {
        Ok(()) => {
            draft.config = current;
            true
        }
        Err(error) => {
            notifications.push(Notification::error(format!(
                "Could not save Ready settings: {error}"
            )));
            false
        }
    }
}
```

Then make `finish_ready_edit` retain pad transaction semantics:

```rust
fn finish_ready_edit(
    state: &mut SongReadyState,
    draft: &mut ReadyConfigDraft,
    notifications: &mut NotificationQueue,
) {
    if persist_ready_card_value(state.focus, draft, notifications) {
        state.apply_edit();
    }
}
```

- [ ] **Step 5: Wire the reducer into only the keyboard Browse branch**

Replace the current keyboard Browse match with:

```rust
NavSource::Keyboard => {
    match reduce_ready_keyboard_browse(&mut state, action.verb) {
        ReadyKeyboardEffect::None => {}
        ReadyKeyboardEffect::AdjustValue(delta) => {
            let card = state.focus;
            adjust_ready_value(&mut state, &mut draft, delta);
            if card != ReadyCard::Mode {
                persist_ready_card_value(card, &mut draft, &mut notifications);
            }
        }
        ReadyKeyboardEffect::AdjustDifficulty(delta) => {
            step_ready_difficulty(&mut selection, &selection_state, delta);
        }
        ReadyKeyboardEffect::Launch => {
            launches.write(ReadyLaunch::Current);
        }
        ReadyKeyboardEffect::Close => state.close(),
    }
}
```

Leave the pad Browse, Edit, and PrimaryDetail branches byte-for-byte behaviorally equivalent to their current implementations.

- [ ] **Step 6: Run focused and complete Song Ready tests**

Run:

```bash
cargo test -p game-menu --lib keyboard_browse
cargo test -p game-menu --lib keyboard_audio_confirm_toggles_field_without_entering_edit
cargo test -p game-menu --lib song_ready::tests
```

Expected: all tests pass, including existing `option_edit_cancel_restores_snapshot` and `practice_card_navigation_skips_disabled_modifiers`.

- [ ] **Step 7: Commit Task 2**

```bash
git add crates/game-menu/src/song_ready.rs
git commit -m "feat: simplify song ready keyboard navigation"
```

---

### Task 3: Align mouse controls, legends, scrim, and responsive central layout

**Files:**
- Modify: `crates/game-menu/src/song_ready.rs:199-690`
- Modify: `crates/game-menu/src/song_ready.rs:865-1279`
- Modify: `crates/game-menu/src/song_select.rs:1180-1235`
- Test: `crates/game-menu/src/song_ready.rs` inline `tests` module

**Interfaces:**
- Consumes: `ReadyStepButton`, `ReadyCardValue`, `ReadyLayout`, `AudioField`, `persist_ready_card_value`.
- Produces: `ReadyAudioFieldButton`, `ReadyDifficultyRail`, `ReadyMetadataColumn`, and responsive child widths in `ReadyLayout`.

- [ ] **Step 1: Add failing responsive-layout assertions**

Extend `adaptive_layout_keeps_five_cards_on_one_line_and_center_largest`:

```rust
for width in [1280.0, 1920.0] {
    let layout = ready_layout(width);
    assert_eq!(layout.rows, 1);
    assert!(layout.widths[2] > layout.widths[0]);
    assert!(layout.widths[2] > layout.widths[4]);
    let central_inner = layout.widths[2] - 36.0;
    let occupied = layout.jacket_width
        + layout.content_gap * 2.0
        + layout.difficulty_width
        + layout.metadata_min_width;
    assert!(occupied <= central_inner);
}
```

Add these fields to the expected API:

```rust
pub struct ReadyLayout {
    pub widths: [f32; 5],
    pub gap: f32,
    pub edge_peek_px: f32,
    pub rows: u8,
    pub jacket_width: f32,
    pub difficulty_width: f32,
    pub metadata_min_width: f32,
    pub content_gap: f32,
}
```

- [ ] **Step 2: Run the layout test and verify RED**

Run:

```bash
cargo test -p game-menu --lib adaptive_layout_keeps_five_cards_on_one_line_and_center_largest
```

Expected: compilation fails because the responsive child-width fields are missing.

- [ ] **Step 3: Supply child widths that fit both target resolutions**

Use these values in `ready_layout`:

```rust
if viewport_width <= 1280.0 {
    ReadyLayout {
        widths: [142.0, 128.0, 610.0, 158.0, 190.0],
        gap: 10.0,
        edge_peek_px: 24.0,
        rows: 1,
        jacket_width: 150.0,
        difficulty_width: 180.0,
        metadata_min_width: 204.0,
        content_gap: 10.0,
    }
} else {
    ReadyLayout {
        widths: [210.0, 180.0, 820.0, 220.0, 250.0],
        gap: 16.0,
        edge_peek_px: 0.0,
        rows: 1,
        jacket_width: 190.0,
        difficulty_width: 230.0,
        metadata_min_width: 344.0,
        content_gap: 10.0,
    }
}
```

Tag the difficulty and metadata columns when spawning:

```rust
#[derive(Component)]
struct ReadyDifficultyRail;

#[derive(Component)]
struct ReadyMetadataColumn;
```

Add `ReadyDifficultyRail` to the rail Node and `ReadyMetadataColumn` to the metadata Node. Extend `layout_song_ready` with disjoint queries and set the jacket, rail, metadata minimum width, and both content gaps from `ReadyLayout`. Keep `flex_shrink = 0.0` for jacket/rail and allow metadata to clip/truncate.

- [ ] **Step 4: Make mouse option controls direct and add Audio row selection**

Add:

```rust
#[derive(Component)]
struct ReadyAudioFieldButton(AudioField);
```

Spawn two compact Audio field buttons labelled `BGM` and `DRUMS`; retain `ReadyCardValue(ReadyCard::Audio)` for the percentages but move the active `▶` marker onto the corresponding field button. Change the generic value step labels from `◀`/`▶` to `▲`/`▼`, and stack them vertically with `FlexDirection::Column`.

Extend `song_ready_pointer_input` with:

```rust
audio_fields: Query<(&Interaction, &ReadyAudioFieldButton), Changed<Interaction>>,
mut notifications: ResMut<NotificationQueue>,
```

Apply these rules:

```rust
for (interaction, field) in &audio_fields {
    if *interaction == Interaction::Pressed && state.layer == SongReadyLayer::Browse {
        state.focus = ReadyCard::Audio;
        state.audio_field = field.0;
    }
}

// In Browse, a step is immediate and persisted. In pad Edit, it remains
// part of the existing snapshot transaction.
if state.layer == SongReadyLayer::Browse {
    state.focus = step.card;
    if let Some(field) = step.field {
        state.audio_field = field;
    }
    adjust_ready_value(&mut state, &mut draft, step.delta);
    if step.card != ReadyCard::Mode {
        persist_ready_card_value(step.card, &mut draft, &mut notifications);
    }
} else if state.layer == SongReadyLayer::Edit && state.focus == step.card {
    adjust_ready_value(&mut state, &mut draft, step.delta);
}
```

Clicking an already-focused option card must no longer call `begin_edit`; only pad Confirm enters Edit.

- [ ] **Step 5: Update visual hierarchy and legends**

Apply these exact copy and styling changes:

```text
Song Select keyboard: ←→ SELECT · ↑↓ CHANGE · ENTER READY · SHIFT+ENTER PRACTICE · ESC BACK
Song Ready keyboard:  ←→ SELECT · ↑↓ CHANGE · ENTER ACTION/AUDIO ROW · ESC BACK
```

- Replace the Ready overlay alpha `0.74` with `0.88`.
- Keep the focused border at 4px and the existing non-color focus scale, respecting Reduced Motion.
- Preserve all five card widths and `FlexWrap::NoWrap`.
- Remove the duplicate old hint tokens that say `CARD/VALUE` or imply Left/Right changes values.

- [ ] **Step 6: Run layout and runtime UI tests**

Run:

```bash
cargo test -p game-menu --lib adaptive_layout_keeps_five_cards_on_one_line_and_center_largest
cargo test -p game-menu --lib ready_plugin_spawns_exactly_five_cards_without_system_conflicts
cargo test -p game-menu --lib song_select_plugin_registers_without_query_conflicts
```

Expected: all tests pass; runtime tests do not report query conflicts or stale entity commands.

- [ ] **Step 7: Commit Task 3**

```bash
git add crates/game-menu/src/song_ready.rs crates/game-menu/src/song_select.rs
git commit -m "fix: align ready controls and responsive layout"
```

---

### Task 4: Verify lifecycle safety and desktop integration

**Files:**
- Modify: `crates/game-menu/src/song_ready.rs` inline `tests` module
- Test: `crates/game-menu/src/song_select.rs` existing inline runtime tests

**Interfaces:**
- Consumes: completed Song Select and Song Ready plugins.
- Produces: verified package and desktop application with no new public API.

- [ ] **Step 1: Add a repeated Ready lifecycle regression test**

Extend the existing Song Ready runtime test to perform three open/close cycles:

```rust
for _ in 0..3 {
    app.world_mut()
        .resource_mut::<SongReadyState>()
        .open(ReadyMode::Normal);
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&ReadyCardNode>()
            .iter(app.world())
            .count(),
        5
    );

    app.world_mut().resource_mut::<SongReadyState>().close();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&SongReadyEntity>()
            .iter(app.world())
            .count(),
        0
    );
}
```

- [ ] **Step 2: Run the lifecycle test before any production adjustment**

Run:

```bash
cargo test -p game-menu --lib ready_plugin_spawns_exactly_five_cards_without_system_conflicts
```

Expected: PASS for all three cycles with zero stale-entity failures.

- [ ] **Step 3: Run the full changed-package test gate**

Run:

```bash
cargo test -p game-menu --lib
```

Expected: all game-menu tests pass with zero failures.

- [ ] **Step 4: Run formatting and warnings-denied lint gates**

Run:

```bash
cargo fmt --all -- --check
cargo clippy -p game-menu --all-targets -- -D warnings
```

Expected: both commands exit 0 with no formatting diff and no warnings.

- [ ] **Step 5: Check the desktop integration graph**

Run:

```bash
cargo check -p dtxmaniars-desktop
```

Expected: `Finished dev profile` with no errors.

- [ ] **Step 6: Audit scope and diff cleanliness**

Run:

```bash
git diff --check
git status --short
git diff --stat
```

Expected: production changes are limited to the two `game-menu` source files; no Practice, shared navigation, MIDI, or typography file appears in the feature diff.

- [ ] **Step 7: Commit any test-only lifecycle addition**

If Step 1 was not already included in Task 3's commit:

```bash
git add crates/game-menu/src/song_ready.rs crates/game-menu/src/song_select.rs
git commit -m "test: cover repeated song ready lifecycle"
```

If there is no remaining diff, skip this commit.
