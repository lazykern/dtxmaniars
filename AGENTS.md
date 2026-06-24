# AGENTS.md — DTXManiaRS agent handbook

> Single source of truth for AI agents (Claude Code, Codex, pi, etc.).
> Humans also read this. Keep tight.

## Quickstart

```sh
cargo check --workspace      # type-check everything
cargo test -p dtx-core       # parser tests
cargo run -p dtx-cli -- validate tests/fixtures/dtx-core/minimal.dtx
cargo run -p dtxmaniars-desktop   # launches the bevy window (M2+)
```

Bevy 0.19 requires Rust 1.95+. CI on `stable`.

## Before writing any implementation code (mandatory)

**Read the relevant reference files first.** See `docs/decisions/0008-reference-first-workflow.md`.

1. Read `crates/<your-crate>/AGENTS.md` — it lists the specific reference files for your crate.
2. Read those files (`ctx_execute_file` for excerpts, `ctx_index` for whole small files).
3. Cite `references/<path>:L<line>` in your commit for any non-trivial behavior ported.
4. **If unsure, port from the reference** rather than guessing from memory.

## Port-first rule (ADR-0010) — applies to ALL UX/UI work

**This is a port of DTXManiaNX, not a new game.** v1 of every screen, lane,
fade, judgment window, HUD layout, transition must match `references/DTXmaniaNX-BocuD/`
verbatim. Improvements (osu-lazer fluidity, custom skins) are M6+ work.

- Source of truth: `references/DTXmaniaNX-BocuD/`. NOT osu-lazer. NOT your instinct.
- Match the reference verbatim for: lane order/positions, timing windows,
  fade durations, HUD position/size, hit line position, scroll direction,
  combo/score animations, transition style, default input bindings.
- Cite the reference file:line in the commit (same as ADR-0008).
- Improvements during port = scope creep. Reject or defer to M6+.
- Exception: correctness fixes for crashes/data corruption are always OK.

**Existing contradictions already fixed:** `SCREEN_FADE_MS = 300 → 1500`,
M3 ROADMAP `osu-style → DTXManiaNX fades`. Do not regress.

Tool order:
- Bevy API → `npx ctx7@latest docs /websites/rs_bevy "<exact question>"`
- Quick excerpt → `ctx_execute_file path=...`
- Whole-file index → `ctx_index path=...` (files <50KB)
- Cross-file search → `ctx_search queries=[...]`

## Where to look

| Need | File |
|---|---|
| What are we building? Phase status | `docs/ROADMAP.md` |
| Crate map, layer rules, data flow | `docs/ARCHITECTURE.md` |
| Project-specific Bevy patterns | `docs/BEVY_PATTERNS.md` |
| Why we chose X over Y | `docs/decisions/` |
| Scratch / session logs / research | `docs/notes/` |
| Per-crate scope, tests, ref files | `crates/<name>/AGENTS.md` |
| Reference implementations | `references/` (read-only, never edit) |

## Crate layers (no violations)

```
Pure    (no bevy)         dtx-core, dtx-scoring, dtx-config
Engine  (bevy allowed)    dtx-timing, dtx-audio, dtx-input, dtx-assets, dtx-library
Game    (bevy + plugins)  dtx-ui, gameplay-drums, game-shell, game-menu, game-results, dev-tools
Bin     (main only)       app/dtxmaniars-desktop, tools/dtx-cli
```

A Pure crate **must not** depend on any Engine or Game crate. Engine crates may depend on Pure. Game crates may depend on Pure + Engine.

## Bevy conventions (see `docs/BEVY_PATTERNS.md` for details)

- One plugin fn per file: `pub(super) fn plugin(app: &mut App)`
- Screens as States; use `StateScoped(Screen::X)` for cleanup (bevy 0.14+)
- Events for cross-plugin communication; explicit ordering via `SystemSet`
- Update systems bounded by `.run_if(in_state(...))` + `SystemSet`
- Asset preload via `Resource` with `#[dependency]` (bevy_asset_loader pattern)
- Animation: `bevy_tween` + cubic-bezier easing via `CubicSegment`
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
- Merge order: `dtx-core` → `dtx-timing` → `gameplay-drums` → `game-shell` → `game-menu`.

## When stuck

1. Read the relevant `AGENTS.md` in `crates/<name>/`.
2. Search `docs/decisions/` for prior art (especially ADR-0010 port-first rule).
3. `ctx_search` for indexed notes.
4. `npx ctx7@latest docs /websites/rs_bevy "<exact question>"`.
5. Read the reference file cited in the issue. **Do not** guess.

## Continuing work in a new session

1. Read this `AGENTS.md` first.
2. Read `docs/ROADMAP.md` for current milestone status.
3. Read `docs/decisions/` for accepted constraints.
4. Read the `AGENTS.md` of the crate you'll touch.
5. (Re)index reference files via `ctx_index` (cheap, pays back fast).
6. Then start coding.