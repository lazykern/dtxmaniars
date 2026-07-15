# Difficulty Vertical-Order Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Up select the visually higher difficulty and Down select the visually lower difficulty in Song Select and Song Ready.

**Architecture:** Keep ascending `Selection.difficulty` ordinals and descending displayed rails. Translate vertical input at menu boundaries so visible movement and ordinal movement have opposite signs.

**Tech Stack:** Rust 1.95+, Bevy 0.19, `game-menu` inline unit tests.

## Global Constraints

- Preserve `Selection.difficulty` as the only difficulty source of truth.
- Preserve direct row selection, bounds clamping, shared navigation types, and pad card traversal.
- Add the failing regression tests before production-code changes.
- No `unwrap()` in production crate code.

---

### Task 1: Align difficulty input with the descending visual rail

**Files:**
- Modify: `crates/game-menu/src/song_select.rs`
- Modify: `crates/game-menu/src/song_ready.rs`
- Test: inline test modules in both files

**Interfaces:**
- Consumes: `NavVerb`, `Selection.difficulty`, and descending rendered difficulty rails.
- Produces: spatially consistent keyboard and mouse-wheel navigation.

- [ ] **Step 1: Write failing regression tests**

Add a Ready reducer assertion that Song-focused `Up` returns
`AdjustDifficulty(1)` and `Down` returns `AdjustDifficulty(-1)`. Add helper
tests with three available difficulties showing positive deltas advance from
ordinal 1 to 2 and negative deltas decrease from ordinal 1 to 0.

- [ ] **Step 2: Run the focused tests to verify RED**

Run:

```sh
cargo test -p game-menu --lib song_ready::tests::keyboard_song_difficulty_follows_visual_vertical_order
```

Expected: failure because the reducer currently emits the opposite deltas.

- [ ] **Step 3: Implement the minimal input-direction changes**

Reverse only vertical difficulty deltas in Song Select and Song Ready. Keep
`step_ready_difficulty` and direct click behavior unchanged because its
positive delta already means a higher ordinal.

- [ ] **Step 4: Verify the focused and package tests**

Run:

```sh
cargo test -p game-menu --lib song_ready::tests
cargo test -p game-menu --lib song_select::tests
cargo check -p game-menu
cargo fmt --all -- --check
git diff --check
```

Expected: all commands exit 0.

- [ ] **Step 5: Commit and push**

```sh
git add crates/game-menu/src/song_select.rs crates/game-menu/src/song_ready.rs docs/superpowers/specs/2026-07-15-difficulty-vertical-order-design.md docs/superpowers/plans/2026-07-15-difficulty-vertical-order.md
git commit -m "fix(menu): align difficulty navigation with visual order"
git push origin main
```
