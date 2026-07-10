# Bindings and Lane Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make MIDI capture, binding recovery, lane-preset presentation, and the persisted-input-to-display path reliable and covered end to end.

**Architecture:** Preserve `EChannel` as the boundary between logical input/judgment and visual lane arrangement. Harden each side independently, then add a headless integration test proving visual presets never change mechanics.

**Tech Stack:** Rust 2024 workspace, Bevy 0.19 ECS/messages/UI, serde/TOML, Cargo tests.

## Global Constraints

- Preserve the two-axis model: bindings target channels/logical pads; layouts map channels to display columns.
- Do not change BocuD default bindings, judgment groups, or named-preset channel order.
- `references/` is read-only.
- No `unwrap()` in `crates/*` production code.
- Use red-green TDD for every behavior change.
- Do not modify or stage the user's unrelated editor changes.

---

### Task 1: Reject duplicate custom lane IDs

**Files:**
- Modify: `crates/dtx-layout/src/file.rs:42-79`
- Test: `crates/dtx-layout/src/file.rs` unit-test module

**Interfaces:**
- Consumes: `LanesSection::order: Option<Vec<String>>`
- Produces: `LanesSection::resolve() -> LaneArrangement` with unique lane IDs, preserving the first occurrence

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn duplicate_custom_lane_ids_keep_first_occurrence() {
    let section = LanesSection {
        preset: crate::presets::LanePreset::Custom,
        order: Some(vec!["HH".into(), "SD".into(), "HH".into(), "BD".into()]),
        ..Default::default()
    };

    let arr = section.resolve();
    let ids: Vec<&str> = arr.lanes.iter().map(|lane| lane.id.as_str()).collect();

    assert_eq!(ids, ["HH", "SD", "BD"]);
    assert_eq!(arr.lane_index_of(EChannel::HiHatClose), Some(0));
}
```

- [ ] **Step 2: Run test to verify RED**

Run: `cargo test -p dtx-layout duplicate_custom_lane_ids_keep_first_occurrence`

Expected: FAIL because the resolved IDs contain the second `HH`.

- [ ] **Step 3: Deduplicate during resolution**

Import `HashSet` beside `HashMap`, then validate first-seen IDs:

```rust
let mut seen = HashSet::new();
let order: Vec<String> = self
    .order
    .clone()
    .unwrap_or_else(|| base.lanes.iter().map(|lane| lane.id.clone()).collect())
    .into_iter()
    .filter(|id| {
        if channel_from_short(id).is_none() {
            eprintln!("dtx-layout: unknown lane id {id:?} dropped");
            return false;
        }
        if !seen.insert(id.clone()) {
            eprintln!("dtx-layout: duplicate lane id {id:?} dropped");
            return false;
        }
        true
    })
    .collect();
```

- [ ] **Step 4: Verify GREEN and the crate**

Run: `cargo test -p dtx-layout duplicate_custom_lane_ids_keep_first_occurrence`

Expected: PASS.

Run: `cargo test -p dtx-layout`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/dtx-layout/src/file.rs
git commit -m "fix(layout): drop duplicate custom lane ids"
```

### Task 2: Drain MIDI independently from gameplay readiness

**Files:**
- Modify: `crates/gameplay-drums/src/lib.rs:405-457`
- Test: `crates/gameplay-drums/src/lib.rs` inside `midi_consumer`

**Interfaces:**
- Consumes: queued `VirtualSource` events, `BindResolver`, `ActiveChart`, `GameplayClock`
- Produces: `LastMidiHit` for every NoteOn; `LaneHit` only when gameplay is ready

- [ ] **Step 1: Add a headless system fixture and failing tests**

Inside `midi_consumer`, add a `#[cfg(test)] mod tests` that builds a real `App`, initializes the five resources above plus `Messages<LaneHit>`, registers `poll_midi`, pushes NoteOn, and updates once:

```rust
#[test]
fn midi_updates_last_hit_and_drains_when_chart_is_empty() {
    let mut app = midi_test_app();
    push_note_on(&mut app, 38, 90);

    app.update();

    let last = *app.world().resource::<LastMidiHit>();
    assert_eq!((last.note, last.velocity), (38, 90));
    assert!(app.world().resource::<VirtualSource>().is_empty());
    assert_eq!(lane_hit_count(&app), 0);
}

#[test]
fn gated_midi_event_is_not_replayed_after_clock_becomes_ready() {
    let mut app = midi_test_app();
    push_note_on(&mut app, 38, 90);
    app.update();
    make_chart_and_clock_ready(&mut app);

    app.update();

    assert_eq!(lane_hit_count(&app), 0);
}
```

The fixture must use real resources/messages rather than mocking `VirtualSource`.

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p gameplay-drums midi_updates_last_hit_and_drains_when_chart_is_empty`

Expected: FAIL because `LastMidiHit` remains default and the source stays non-empty.

- [ ] **Step 3: Move readiness gates after the drain**

```rust
let gameplay_ready = !chart.chart.chips.is_empty() && clock.is_ready();
let mut buf = Vec::new();
(*source).poll(&mut buf);
for ev in buf {
    let dtx_input::midi::MidiEvent::NoteOn {
        note,
        velocity,
        audio_ms,
    } = ev
    else {
        continue;
    };
    *last = LastMidiHit {
        note,
        velocity,
        below_threshold: velocity <= resolver.velocity_threshold,
        at: Some(std::time::Instant::now()),
    };
    if !gameplay_ready || velocity == 0 || velocity <= resolver.velocity_threshold {
        continue;
    }
    let Some(lane) = resolver.lane_for_note(note) else {
        continue;
    };
    hits.write(LaneHit {
        lane,
        audio_ms: if audio_ms != 0 {
            audio_ms
        } else {
            clock.current_ms
        },
    });
}
```

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p gameplay-drums midi_consumer`

Expected: both new tests pass.

```bash
git add crates/gameplay-drums/src/lib.rs
git commit -m "fix(input): drain midi before gameplay gates"
```

### Task 3: Order binding rows by the active arrangement

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs:318-418`
- Test: its unit-test module

**Interfaces:**
- Consumes: `&Lanes`
- Produces: `channels_in_display_order(&Lanes) -> Vec<EChannel>` with primary-first merged groups and exactly-once coverage

- [ ] **Step 1: Write failing named-preset tests**

```rust
#[test]
fn binding_rows_follow_classic_display_order() {
    let lanes = Lanes(dtx_layout::classic());
    assert_eq!(
        channels_in_display_order(&lanes),
        [
            EChannel::LeftCymbal,
            EChannel::HiHatClose,
            EChannel::HiHatOpen,
            EChannel::LeftPedal,
            EChannel::Snare,
            EChannel::HighTom,
            EChannel::BassDrum,
            EChannel::LeftBassDrum,
            EChannel::LowTom,
            EChannel::FloorTom,
            EChannel::Cymbal,
            EChannel::RideCymbal,
        ]
    );
}

#[test]
fn binding_rows_cover_type_b_channels_once() {
    let lanes = Lanes(dtx_layout::nx_type_b());
    let rows = channels_in_display_order(&lanes);
    assert_eq!(rows.len(), dtx_config::BINDABLE_CHANNELS.len());
    for channel in dtx_config::BINDABLE_CHANNELS {
        assert_eq!(rows.iter().filter(|&&row| row == channel).count(), 1);
    }
    let lp = rows.iter().position(|&row| row == EChannel::LeftPedal);
    let lbd = rows.iter().position(|&row| row == EChannel::LeftBassDrum);
    assert_eq!(lbd, lp.map(|index| index + 1));
}
```

Add an NX Type-D test asserting primary order `LC, HH, SD, HT, LP, BD, LT, FT, CY, RD`, with HHO after HH and LBD after LP.

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p gameplay-drums binding_rows_`

Expected: compile failure because `channels_in_display_order` does not exist.

- [ ] **Step 3: Implement ordering**

Walk `lanes.0.lanes` left-to-right with `HashSet<EChannel>`. Emit each display lane's bindable primary first, then remaining `BINDABLE_CHANNELS` mapped to that column. Append unseen bindable channels defensively. Change the panel loop to:

```rust
for ch in channels_in_display_order(lanes) {
```

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p gameplay-drums binding_rows_`

Expected: all three preset tests pass.

```bash
git add crates/gameplay-drums/src/editor/bindings_panel.rs
git commit -m "fix(editor): order bindings by display lanes"
```

### Task 4: Add binding reset recovery

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_panel.rs`
- Test: its unit-test module

**Interfaces:**
- Produces: `BindingsResetState::{Idle, Confirming}`
- Produces: `bindings_modified(&InputBindings) -> bool`
- Produces: `reset_bindings(&mut LiveBindings, &mut BindingsRev)`

- [ ] **Step 1: Write failing reset tests**

```rust
#[test]
fn reset_confirmation_restores_all_binding_defaults() {
    let mut live = LiveBindings(InputBindings::default());
    live.0.midi.port = Some("test-port".into());
    live.0.midi.velocity_threshold = 64;
    live.0.bind(EChannel::Snare, BindSource::Key(KeyCode::KeyQ));
    let mut rev = BindingsRev(7);

    assert!(bindings_modified(&live.0));
    reset_bindings(&mut live, &mut rev);

    assert_eq!(live.0, InputBindings::default());
    assert_eq!(rev.0, 8);
    assert!(!bindings_modified(&live.0));
}

#[test]
fn cancel_reset_leaves_bindings_unchanged() {
    let before = modified_bindings_fixture();
    let live = LiveBindings(before.clone());
    let mut state = BindingsResetState::Confirming;

    cancel_bindings_reset(&mut state);

    assert_eq!(state, BindingsResetState::Idle);
    assert_eq!(live.0, before);
}
```

- [ ] **Step 2: Run tests to verify RED**

Run: `cargo test -p gameplay-drums reset_confirmation_restores_all_binding_defaults`

Expected: compile failure because the reset API does not exist.

- [ ] **Step 3: Implement state and UI**

Initialize `BindingsResetState` in the plugin. Add a binding-panel header with amber `MODIFIED` when live bindings differ from defaults. Idle renders `RESET TAB`; first click enters `Confirming` and bumps `BindingsRev`. Confirming renders `CONFIRM RESET` and `CANCEL`; confirmation replaces all live bindings with defaults, cancellation leaves them untouched, and both return to idle/rebuild.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p gameplay-drums reset_ bindings_modified cancel_reset_`

Expected: all reset tests pass.

```bash
git add crates/gameplay-drums/src/editor/bindings_panel.rs
git commit -m "feat(editor): add binding reset recovery"
```

### Task 5: Emit feedback for the newly captured channel

**Files:**
- Modify: `crates/gameplay-drums/src/editor/bindings_capture.rs:114-198`
- Modify: `crates/gameplay-drums/src/input.rs:54-69`
- Test: `bindings_capture.rs` unit-test module

**Interfaces:**
- Produces: `captured_lane_hit(EChannel, i64) -> Option<LaneHit>`
- Prevents: old resolver input while capture state is active

- [ ] **Step 1: Write the failing helper test**

```rust
#[test]
fn capture_feedback_targets_newly_bound_channel() {
    let hit = captured_lane_hit(EChannel::Snare, 1234);
    assert_eq!(hit.map(|value| (value.lane, value.audio_ms)), Some((1, 1234)));
}
```

- [ ] **Step 2: Run test to verify RED**

Run: `cargo test -p gameplay-drums capture_feedback_targets_newly_bound_channel`

Expected: compile failure because `captured_lane_hit` does not exist.

- [ ] **Step 3: Implement post-capture feedback**

Add `GameplayClock` and `MessageWriter<LaneHit>` to `capture_binding`. After direct bind and confirmed steal, when the clock is ready, convert the target channel with `lane_map::lane_of` and write `LaneHit { lane, audio_ms: clock.current_ms }`.

In `capture_key_to_lane_input`, add `Res<CaptureState>` and return before resolver lookup whenever capture is not `Idle`. This makes the explicit post-capture message the sole feedback for the interaction.

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test -p gameplay-drums capture_feedback_targets_newly_bound_channel input::tests`

Expected: all selected tests pass.

```bash
git add crates/gameplay-drums/src/editor/bindings_capture.rs crates/gameplay-drums/src/input.rs
git commit -m "fix(editor): flash newly captured binding"
```

### Task 6: Add persisted binding-to-display integration coverage

**Files:**
- Create: `crates/gameplay-drums/tests/bindings_lane_pipeline.rs`

**Interfaces:**
- Consumes: `save_bindings`, `load_bindings`, `BindResolver`, `lane_channel`, `Lanes`, and named presets
- Verifies: persistence and resolution preserve logical channel identity across every named visual preset

- [ ] **Step 1: Write the integration test**

Create a temporary bindings file, bind KeyQ and MIDI note 99 to Snare, save/load it, build `BindResolver`, and assert both sources resolve to the logical Snare lane. For Classic, NX Type-B, and NX Type-D, convert that lane through `lane_channel`, then assert `Lanes::col_of` equals the preset's `lane_index_of(EChannel::Snare)`.

Add a Type-B case proving LP and LBD remain distinct logical lanes but share one display column. Clean the temporary directory after assertions.

- [ ] **Step 2: Run the integration test**

Run: `cargo test -p gameplay-drums --test bindings_lane_pipeline`

Expected: PASS with Tasks 1-5 present. Confirm the test exercises save/load plus both logical and display assertions.

- [ ] **Step 3: Commit**

```bash
git add crates/gameplay-drums/tests/bindings_lane_pipeline.rs
git commit -m "test(drums): cover bindings and lane presets end to end"
```

### Task 7: Final verification and terminology

**Files:**
- Modify only stale comments found by the exact search

- [ ] **Step 1: Correct stale logical/display order comments**

Run:

```bash
rg -n "BocuD lane order|visual lane index|leftmost \(HH|lane order" crates/dtx-config crates/gameplay-drums docs -g '*.rs' -g '*.md'
```

Call `BINDABLE_CHANNELS` and `LANE_ORDER` the canonical logical-pad order. Keep accurate display-arrangement references.

- [ ] **Step 2: Format and inspect**

Run:

```bash
cargo fmt --all -- --check
git diff --check
git status --short
```

Expected: checks succeed; unrelated user files remain unstaged.

- [ ] **Step 3: Run focused verification**

Run: `cargo test -p dtx-layout -p dtx-config -p gameplay-drums`

Expected: all tests pass, including `bindings_lane_pipeline`.

- [ ] **Step 4: Type-check the workspace**

Run: `cargo check --workspace`

Expected: exit 0.

- [ ] **Step 5: Review scoped history/diff**

Run:

```bash
git log --oneline -8
git show --stat --oneline HEAD
git status --short
```

Confirm no reference edits, secrets, local configuration, generated assets, or unrelated editor changes entered the commits.

- [ ] **Step 6: Commit comment-only cleanup when needed**

```bash
git add crates/dtx-config crates/gameplay-drums
git commit -m "docs(input): clarify logical and display lane order"
```

