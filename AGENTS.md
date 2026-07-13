# AGENTS.md — DTXManiaRS agent handbook

> Single source of truth for AI agents (Claude Code, Codex, pi, etc.).
> Humans also read this. Keep tight.

## Quickstart

```sh
cargo check -p dtx-core
cargo test -p dtx-core
cargo run -p dtx-cli -- validate crates/dtx-core/tests/fixtures/minimal.dtx
cargo run -p dtxmaniars-desktop --features bevy/dynamic_linking
```

The `dtx-cli` package builds a binary named `dtx`.

Bevy 0.19 requires Rust 1.95+. CI on `stable`. `dynamic_linking` is dev-only;
release builds must omit it.

## Fast build/test workflow

Inner loop — run only changed package and smallest relevant target:

```sh
cargo check -p <changed-package>
cargo test -p <changed-package> --lib
cargo test -p <changed-package> --test <integration-test>
cargo nextest run -p <changed-package> # multiple test binaries
```

For Bevy-heavy integration tests, reduce linking cost with:

```sh
cargo test -p gameplay-drums --features bevy/dynamic_linking --test <integration-test>
```

Before handoff, run all tests for changed packages. Before merge:

```sh
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Run `CARGO_BUILD_JOBS=1 cargo test --workspace --all-targets` only when a full
workspace gate is required; this suite can exceed available RAM, so CI currently
uses package tests instead. Use `cargo build --profile debugging` when full
debugger information is needed.

## Before writing any implementation code (mandatory)

**Read the relevant reference files first.** See `docs/decisions/README.md`.

1. Read `crates/<your-crate>/AGENTS.md` — it lists the specific reference files for your crate.
2. Read those files (`ctx_execute_file` for excerpts, `ctx_index` for whole small files).
3. Cite `references/<path>:L<line>` in your commit for any non-trivial behavior ported.
4. **If unsure, port from the reference** rather than guessing from memory.

## Port-first rule (ADR-0010) — applies to **game mechanics only**

**Mechanics port from DTXManiaNX-BocuD.** Judgment windows, scoring, lane order,
EChannel mapping, chart parsing, scroll logic, default input bindings must match
`references/DTXmaniaNX/` verbatim. Cite reference file:line in commits.

**UX/UI is redesigned** per ADR-0014 (osu-inspired fluidity). Do NOT copy BocuD
pixel layouts, GitaDora transitions, or static HUD when ADR-0014 specifies otherwise.

- Mechanics source of truth: `references/DTXmaniaNX/`
- UX/UI source of truth: ADR-0014
- Screen transitions: 300ms OutQuint fades (not GitaDora, not 1500ms snapshot)
- Exception: correctness fixes for crashes/data corruption are always OK

Tool order:
- Bevy API → `npx ctx7@latest docs /websites/rs_bevy "<exact question>"`
- Quick excerpt → `ctx_execute_file path=...`
- Whole-file index → `ctx_index path=...` (files <50KB)
- Cross-file search → `ctx_search queries=[...]`

## Where to look

| Need | File |
|---|---|
| What are we building? Phase status | `docs/roadmap.md` |
| Why we chose X over Y | `docs/decisions/` |
| How to contribute and verify | `docs/contributing.md` |
| Scratch / session logs / research | `docs/notes/` |
| Per-crate scope, tests, ref files | `crates/<name>/AGENTS.md` |
| Reference implementations | `references/` (read-only, never edit) |

## Crate layers (no violations)

```
Pure    (no bevy)         dtx-core, dtx-scoring, dtx-config, dtx-layout, xtask
Engine  (bevy allowed)    dtx-timing, dtx-audio, dtx-input, dtx-assets, dtx-library, dtx-bga, gameplay-guitar
Game    (bevy + plugins)  dtx-ui, gameplay-drums, game-shell, game-menu, game-results, dev-tools
Bin     (main only)       app/dtxmaniars-desktop, tools/dtx-cli
```

A Pure crate **must not** depend on any Engine or Game crate. Engine crates may depend on Pure. Game crates may depend on Pure + Engine.

## Bevy conventions

- One plugin fn per file: `pub(super) fn plugin(app: &mut App)`
- Screens as States; use `StateScoped(Screen::X)` for cleanup (bevy 0.14+)
- Events for cross-plugin communication; explicit ordering via `SystemSet`
- Update systems bounded by `.run_if(in_state(...))` + `SystemSet`
- Asset preload via `Resource` with `#[dependency]` (bevy_asset_loader pattern)
- Animation: `bevy_tweening` (djeedai) — `TweeningPlugin` + lenses. Pinned to git rev 5e3d0c9 until 0.16 ships.
- Audio: `bevy_kira_audio` (not raw kira) — gives proper bevy Resource/Events
- Frame pacing: `bevy_framepace` (per project rules)

## References policy (ADR-0003, ADR-0004)

- `references/` is **read-only**. Do not edit, copy, or commit.
- Use `npx ctx7@latest docs /websites/rs_bevy "<question>"` for Bevy API questions.
- Use `ctx_search` (context-mode) for in-session lookup of indexed reference files.
- If you need to cite a reference file, link it as `references/<path>:L<line>`.

## Coding rules

- No `unwrap()` in `crates/*` (binary stubs may use it sparingly).
- Errors: `thiserror` for libraries, `anyhow` for binaries/tools.
- Internal crates: `version = "0.0.0"`, `publish = false`.
- One commit per logical change. No AI co-author trailers (per project rules).
- No secrets, tokens, or local config in commits.

## AI agent parallelism

- **One agent per crate domain.** Two agents on the same crate = serialize.
- Use `git worktree add ../dtxmaniars-<domain> -b feat/<domain>-<task>` for parallel work.
- Worktrees share the configured Cargo target directory; isolated Bevy targets cost tens of GiB and require cold rebuilds.
- Parallelize editing and small package checks, but serialize Bevy-heavy builds/tests and workspace-wide gates.
- Use `sccache` as secondary reuse across cleans/config variants; set `SCCACHE_BASEDIRS` to the absolute common worktree parent.
- Keep incremental compilation enabled.
- Linker, `rustflags`, toolchain, profile, or feature changes invalidate build artifacts.
- Never clean the shared target while another Cargo process is active or without confirmation.
- Merge order: `dtx-core` → `dtx-timing` → `gameplay-drums` → `game-shell` → `game-menu`.

## When stuck

1. Read the relevant `AGENTS.md` in `crates/<name>/`.
2. Search `docs/decisions/` for prior art (especially ADR-0010 port-first rule).
3. `ctx_search` for indexed notes.
4. `npx ctx7@latest docs /websites/rs_bevy "<exact question>"`.
5. Read the reference file cited in the issue. **Do not** guess.

## Continuing work in a new session

1. Read this `AGENTS.md` first.
2. Read `docs/roadmap.md` for current program status.
3. Read `docs/decisions/` for accepted constraints.
4. Read the `AGENTS.md` of the crate you'll touch.
5. (Re)index reference files via `ctx_index` (cheap, pays back fast).
6. Then start coding.
