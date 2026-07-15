# Practice Setup Directional Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure Practice Setup Up/Down moves the selected row without changing its value, while Left/Right remains the only directional value adjustment.

**Architecture:** Keep the existing `PracticeUiAction` split unchanged: its reducer already sends Up/Down to `MoveSelection` and Left/Right to `Adjust`. Gate the independent performance-hotkey system while the Practice Setup or Editing surface owns the keyboard, so its ArrowUp/ArrowDown scroll-speed handling cannot also run.

**Tech Stack:** Rust, Bevy messages/resources, `gameplay-drums` practice HUD integration tests.

## Global Constraints

- Preserve the approved Practice Setup input contract: Up/Down changes visible selection; Left/Right adjusts values.
- Do not change judgment, transport, preview, or MIDI binding mechanics.
- Add a test before production-code changes and observe it fail first.
- No `unwrap()` in production crate code; no AI co-author trailers.

---

### Task 1: Isolate vertical Setup navigation from value adjustment

**Files:**
- Modify: `crates/gameplay-drums/src/perf_hotkeys.rs:plugin`
- Test: `crates/gameplay-drums/src/perf_hotkeys.rs`

**Interfaces:**
- Consumes: `PracticeFlow` and its `PracticePhase`.
- Produces: a run condition that permits performance hotkeys only for normal play and Practice Running, never Setup or Editing.

- [x] **Step 1: Write the failing integration test**

```rust
#[test]
fn performance_hotkeys_yield_to_practice_setup_and_editing() {
    assert!(!performance_hotkeys_active(Some(&PracticeFlow::default())));

    let mut editing = PracticeFlow::default();
    editing.phase = PracticePhase::Editing;
    assert!(!performance_hotkeys_active(Some(&editing)));

    assert!(performance_hotkeys_active(Some(&PracticeFlow::running())));
    assert!(performance_hotkeys_active(None));
}
```

- [x] **Step 2: Run the test to verify it fails**

Run: `cargo test -p gameplay-drums --lib perf_hotkeys::tests::performance_hotkeys_yield_to_practice_setup_and_editing`

Expected: FAIL because no Practice-surface hotkey run condition exists.

- [x] **Step 3: Write the minimal implementation**

```rust
fn performance_hotkeys_active_value(flow: Option<&PracticeFlow>) -> bool {
    !practice_surface_open_value(flow)
}

(handle_perf_hotkeys, debounced_persist_perf_hotkeys)
    .chain()
    .run_if(performance_hotkeys_active)
```

Keep arrow handling active in normal play and Practice Running. The Setup HUD remains the only consumer of arrows while Setup or Editing is open.

- [x] **Step 4: Run the focused HUD suite to verify it passes**

Run: `cargo test -p gameplay-drums --lib perf_hotkeys::tests::performance_hotkeys_yield_to_practice_setup_and_editing && cargo test -p gameplay-drums --test practice_hud`

Expected: PASS, including the new vertical-navigation regression.

- [x] **Step 5: Run crate checks and commit**

Run: `cargo fmt --all -- --check && cargo clippy -p gameplay-drums --all-targets -- -D warnings && git diff --check`

Expected: all commands exit 0.

```bash
git add crates/gameplay-drums/src/perf_hotkeys.rs docs/superpowers/plans/2026-07-15-practice-setup-directional-nav.md
git commit -m "fix(practice): separate setup navigation and adjustment"
```
