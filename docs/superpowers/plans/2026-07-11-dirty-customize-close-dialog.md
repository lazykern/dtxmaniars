# Dirty Customize Close Dialog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a usable Cancel/Discard/Save modal when dirty Customize profiles block exit.

**Architecture:** Add one focused `close_dialog` editor plugin that renders `PendingCloseState` and emits existing `CloseDecision` values. Extend existing resolver to consume those decisions; keep all persistence and reducer behavior unchanged.

**Tech Stack:** Rust 2024, Bevy 0.19 UI/ECS messages, existing `dtx-ui` theme.

## Global Constraints

- No new dependency or generic dialog framework.
- Preserve existing dirty-profile reducers and atomic save behavior.
- Modal blocks pointer input beneath it.
- Escape cancels; Enter saves; Discard requires explicit click.

---

### Task 1: Close-guard modal and decision transport

**Files:**
- Create: `crates/gameplay-drums/src/editor/close_dialog.rs`
- Modify: `crates/gameplay-drums/src/editor/mod.rs`
- Modify: `crates/gameplay-drums/src/ui_z.rs`
- Test: `crates/gameplay-drums/src/editor/close_dialog.rs`

**Interfaces:**
- Consumes: `profile_state::PendingCloseState`, `profile_state::CustomizeSession`, `profile_state::dirty_dialog_layout`.
- Produces: `CloseDecisionRequest(pub profile_state::CloseDecision)` Bevy message; modal button components emit it.

- [ ] **Step 1: Write failing modal spawn test**

Create a Bevy `App`, insert a pending dirty MIDI close, `CustomizeSession`, and `ThemeResource`, add `sync_dialog`, update once, then assert one `CloseDialogRoot` exists and button decisions equal:

```rust
assert_eq!(
    decisions,
    vec![CloseDecision::Cancel, CloseDecision::DiscardAll, CloseDecision::SaveAll]
);
```

- [ ] **Step 2: Verify RED**

Run:

```sh
cargo test -p gameplay-drums editor::close_dialog::tests --lib
```

Expected: compile failure because `close_dialog` UI types/systems do not exist.

- [ ] **Step 3: Implement minimal modal plugin**

Implement:

```rust
#[derive(Debug, Clone, Copy, Message)]
pub struct CloseDecisionRequest(pub CloseDecision);

#[derive(Component)]
struct CloseDialogRoot;

#[derive(Component, Clone, Copy)]
struct CloseDialogButton(CloseDecision);
```

`sync_dialog` despawns old roots, returns for `PendingCloseState::None`, otherwise spawns a full-window absolute scrim, centered panel, title/body, and three buttons from `dirty_dialog_layout`. Use `GlobalZIndex(crate::ui_z::EDITOR_MODAL)` and default pickability to block lower UI. `handle_buttons` emits `CloseDecisionRequest` on `Interaction::Pressed`. Add `OnExit(AppState::Performance)` cleanup.

- [ ] **Step 4: Route click decisions through existing resolver**

Register `close_dialog::plugin` from `editor::plugin`. Extend `resolve_pending_close` with:

```rust
mut requested: MessageReader<close_dialog::CloseDecisionRequest>,
```

Choose first click decision, falling back to existing Escape/Enter keyboard mapping. Keep `reduce_close_decision`, save calls, and final close logic unchanged.

- [ ] **Step 5: Verify GREEN**

Run:

```sh
cargo test -p gameplay-drums editor::close_dialog::tests --lib
cargo test -p gameplay-drums editor::profile_state::tests --lib
```

Expected: all selected tests pass.

- [ ] **Step 6: Verify package and formatting**

Run:

```sh
cargo fmt --all -- --check
cargo check -p gameplay-drums
cargo test -p gameplay-drums --lib
```

Expected: format/check succeed; all gameplay-drums library tests pass.

- [ ] **Step 7: Commit and push**

```sh
git add crates/gameplay-drums/src/editor/close_dialog.rs crates/gameplay-drums/src/editor/mod.rs crates/gameplay-drums/src/ui_z.rs docs/superpowers/plans/2026-07-11-dirty-customize-close-dialog.md
git commit -m "fix(customize): show dirty close dialog"
git push
```
