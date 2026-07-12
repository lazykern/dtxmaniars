# Wait Chord Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Explain halted wait-mode chords without changing judgment rules.

**Architecture:** A focused practice HUD module derives prompt text from `WaitState` and `JudgedChips`, then owns one state-scoped Bevy text entity. The formatter is pure and tested independently.

**Tech Stack:** Rust, Bevy UI, gameplay-drums.

## Global Constraints

- Preserve the 50 ms wait-mode chord requirement.
- Do not change chart judgment or input bindings.
- Keep the module scoped to practice mode.

---

### Task 1: Persistent wait prompt

**Files:**
- Create: `crates/gameplay-drums/src/practice/hud/wait_prompt.rs`
- Modify: `crates/gameplay-drums/src/practice/hud/mod.rs`

- [ ] Write a failing unit test for `wait_prompt_text` with SD and FT, where SD is judged and FT is pending.
- [ ] Run `cargo test -p gameplay-drums wait_prompt_text` and confirm failure.
- [ ] Add the pure formatter and a state-scoped top-centre text prompt.
- [ ] Run the focused test and `cargo test -p gameplay-drums`.
