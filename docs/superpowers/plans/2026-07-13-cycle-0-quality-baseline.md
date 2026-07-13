# Cycle 0 Quality Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore a clean, truthful local formatting baseline before feature work begins.

**Architecture:** This plan is a mechanical-only cleanup. Run the repository's pinned Rust formatter, verify that only the twelve already-known files change, and prove that the resulting workspace still compiles and passes its library tests.

**Tech Stack:** Rust 1.95, Cargo, rustfmt, Git.

## Global Constraints

- Do not change CI/CD configuration or workflows.
- Do not change runtime behavior in this plan.
- Keep the formatting-only change in one isolated commit.
- Preserve all existing user changes and abort if files outside the known baseline are modified.

---

## File map

- Modify mechanically: `crates/dtx-layout/src/file.rs`
- Modify mechanically: `crates/dtx-layout/src/lane_edit.rs`
- Modify mechanically: `crates/dtx-scoring/src/lib.rs`
- Modify mechanically: `crates/dtx-scoring/src/nx_import.rs`
- Modify mechanically: `crates/dtx-scoring/src/store.rs`
- Modify mechanically: `crates/dtx-scoring/tests/edge_cases.rs`
- Modify mechanically: `crates/dtx-scoring/tests/nx_import.rs`
- Modify mechanically: `crates/dtx-scoring/tests/store_v2.rs`
- Modify mechanically: `crates/game-results/src/lib.rs`
- Modify mechanically: `crates/game-results/src/ui.rs`
- Modify mechanically: `crates/game-menu/src/song_loading.rs`
- Modify mechanically: `crates/game-menu/src/title.rs`

### Task 1: Apply and verify the formatting baseline

**Files:**

- Modify: the twelve files in the file map above
- Test: entire workspace

**Interfaces:**

- Consumes: the current unformatted but compiling source tree
- Produces: a workspace where `cargo fmt --all -- --check` exits successfully

- [ ] **Step 1: Reconfirm the expected failing baseline**

Run:

```bash
cargo fmt --all -- --check
```

Expected: exit code 1 with diffs limited to the twelve files listed in the file map.

- [ ] **Step 2: Apply rustfmt**

Run:

```bash
cargo fmt --all
```

Expected: success with no terminal output.

- [ ] **Step 3: Prove the change is mechanical and scoped**

Run:

```bash
git diff --check
git diff --stat
git diff -- crates/dtx-layout/src/file.rs crates/dtx-layout/src/lane_edit.rs crates/dtx-scoring/src/lib.rs crates/dtx-scoring/src/nx_import.rs crates/dtx-scoring/src/store.rs crates/dtx-scoring/tests/edge_cases.rs crates/dtx-scoring/tests/nx_import.rs crates/dtx-scoring/tests/store_v2.rs crates/game-results/src/lib.rs crates/game-results/src/ui.rs crates/game-menu/src/song_loading.rs crates/game-menu/src/title.rs
```

Expected: `git diff --check` is silent; the stat names only those twelve files; inspection shows whitespace, wrapping, and indentation changes only.

- [ ] **Step 4: Verify formatting and behavior**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace --lib
```

Expected: all three commands exit 0; the library test command reports all tests passing.

- [ ] **Step 5: Commit the baseline**

```bash
git add crates/dtx-layout/src/file.rs crates/dtx-layout/src/lane_edit.rs crates/dtx-scoring/src/lib.rs crates/dtx-scoring/src/nx_import.rs crates/dtx-scoring/src/store.rs crates/dtx-scoring/tests/edge_cases.rs crates/dtx-scoring/tests/nx_import.rs crates/dtx-scoring/tests/store_v2.rs crates/game-results/src/lib.rs crates/game-results/src/ui.rs crates/game-menu/src/song_loading.rs crates/game-menu/src/title.rs
git commit -m "style: restore workspace formatting baseline"
```

Expected: one commit containing only formatting changes.
