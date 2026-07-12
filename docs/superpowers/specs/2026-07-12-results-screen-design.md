# Results Screen Rebuild — Design

Stream 1 of the post-audit UX work. Rebuilds the presentation and input of the
results screen (`crates/game-results`). Fixes audit finding U3 (flat single-font
column, space-padded alignment, no judgment colors, plain rank, linear fade,
single exit verb). Research base: `docs/notes/2026-07-12-streams-research.md`
§Stream 1.

## Goals

- Typographic hierarchy: rank is the headline, score secondary, judgments a
  colored table.
- Real column layout (nodes), no space-padding under the proportional font.
- OutQuint staggered reveal with input-to-skip.
- Three verbs: Continue, Retry, Practice — cursor + shortcuts, keyboard and pad.
- Preserve save-on-entry (`save_result`, `SaveStatus`) behavior exactly.

## Non-goals

- No new theme tokens (design-token stream handles consolidation; this screen
  uses existing `Theme` fields).
- No changes to scoring, rank formula, or `.score.ini` writing.
- No changes to `game-shell` states or transition plumbing.

## Layout

Two-panel card, centered on `bg_bottom`, card `panel_bg`, max content width
~900px, padding 48px.

```
┌──────────────────────────────────────────────┐
│  Song Title — Artist            Lv 5.20      │  header band
│──────────────────────────────────────────────│
│                     │  PERFECT   412  82.4%  │
│         A           │  GREAT      61  12.2%  │
│   (rank letter,     │  GOOD       12   2.4%  │
│    ~160px, rank-    │  POOR        6   1.2%  │
│    colored)         │  MISS        9   1.8%  │
│                     │  ──────────────────    │
│   STAGE FAILED      │  MAX COMBO        214  │
│   (only if failed)  │  SCORE        912,340  │
│                     │  saved ✓               │
│──────────────────────────────────────────────│
│    ▸ Continue      Retry      Practice       │  verb row
│     BD select · HH/CY move · SD back  (MIDI) │  legend
│     Enter select · R retry · Esc continue    │  kbd hint
└──────────────────────────────────────────────┘
```

### Header band

- Title: `Theme::font(28.0)`, `text_primary`.
- `{artist} · Lv {dlevel:.2}`: `Theme::font(16.0)`, artist in `text_secondary`,
  the `Lv X.XX` span colored with `theme.difficulty_color(difficulty)` where
  difficulty is the chart's difficulty index (same value the song wheel uses).
  `dlevel` via `dtx_core::display_dlevel`.

### Left panel — rank

- Rank letter (`Rank` Display string: SS/S/A/B/C/D/E, or `--` for Unknown):
  `Theme::font(160.0)`, colored by a pure helper:

  ```rust
  fn rank_color(rank: Rank, theme: &Theme) -> Color {
      match rank {
          Rank::SS | Rank::S => theme.judgment_perfect, // gold
          Rank::A => theme.judgment_great,              // green
          Rank::B => theme.judgment_good,               // blue
          Rank::C => theme.judgment_ok,                 // purple
          Rank::D | Rank::E => theme.judgment_miss,     // red
          Rank::Unknown => theme.text_secondary,
      }
  }
  ```

- `STAGE FAILED` tag under the letter, `Theme::font(16.0)`, `judgment_miss`,
  shown only when `Option<Res<LastStageOutcome>>` is present and
  `cleared == false`. Absent resource ⇒ no tag.

### Right panel — stats table

Each judgment row is a horizontal `Node` with three children (no space
padding):

- label node, fixed width 120px, `Theme::font(18.0)`, colored
  `theme.judgment_color(label)` (PERFECT/GREAT/GOOD/POOR/MISS);
- count node, fixed width 80px, right-aligned (`justify_content: FlexEnd`),
  `Theme::font(18.0)`, `text_primary`;
- percent node, `Theme::font(14.0)`, `text_secondary`, `{pct:.1}%` where
  `pct = count / DrumScoring.total_notes * 100` (existing `pct()` helper),
  0 total ⇒ `0.0%`.

Divider: 1px `Node` with `text_secondary`-alpha background.

Below the divider:

- `MAX COMBO` row: label `Theme::font(14.0)` `text_secondary`, value
  `Theme::font(18.0)` `text_primary`.
- `SCORE` row: label `Theme::font(14.0)` `text_secondary`, value
  `Theme::font(28.0)` `text_primary`, thousands-separated
  (pure helper `format_thousands(u64) -> String`, comma separator).
- Save-status line: unchanged logic and colors from today — `"saved ✓"` in
  `clear_green` / `"save failed — score kept this session only"` in
  `judgment_miss` / nothing for `SaveStatus::Practice`. `Theme::font(14.0)`.

### Verb row

Three text labels: `Continue`, `Retry`, `Practice`, `Theme::font(20.0)`.
Selected verb: `theme.accent` color + `▸ ` prefix; unselected:
`text_secondary`, two-space prefix (keeps row width stable). Row centered,
`column_gap: 32px`.

Under it:

- Pad legend via `spawn_nav_legend(p, &t, &[("HH/CY","move"), ("BD","select"),
  ("SD","continue"), ("FT","practice")])`, spawned only when
  `MidiConnected(true)` (existing pattern).
- Keyboard hint line, `Theme::font(12.0)`, `text_secondary`:
  `←/→ move · Enter select · R retry · Esc continue`. Always shown.

## Motion

Every visible element carries:

- `EnterChoreo::slide(Vec2::new(0.0, 24.0), delay_ms, 350.0)` — existing
  OutQuint system (`dtx-ui/src/motion.rs`, `enter_choreo_system`), and
- the existing alpha reveal (`StatRow { reveal_at_ms }` +
  `animate_staggered_reveal`), retimed: `STAGGER_MS = 60.0`,
  `FADE_DURATION_MS = 350.0`, and eased with OutQuint instead of linear
  (`EaseFunction::QuintOut.sample_clamped(t)` on the alpha fraction).

Stagger order: header → rank letter → failed tag → judgment rows top-down →
divider → combo → score → save line → verb row → legends. Last element starts
at roughly 60 × 12 = 720ms; full reveal completes under ~1.1s.

**Skip:** a `RevealState` resource tracks `done: bool`. The first input of any
kind (any NavAction, or any relevant key press) while `!done` sets all alphas
to 1, finishes all `EnterChoreo` components (set `elapsed >= delay+duration`
or remove them), sets `done = true`, and is consumed — it does not move the
cursor or activate a verb. Once `done` (either by timeout when the last
element finishes, or by skip), input acts normally. Reveal-completion by
timeout also sets `done = true`.

## Input

New resource:

```rust
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum ResultVerb { #[default] Continue, Retry, Practice }
```

Pure reducer, unit-tested:

```rust
fn reduce_result_nav(cursor: ResultVerb, verb: NavVerb) -> ResultAction
// ResultAction = Moved(ResultVerb) | Activate(ResultVerb) | ContinueNow | PracticeNow | None
```

- `NavVerb::Up` / `Down` (HH / CY pads) and keyboard ←/→: move cursor
  Continue↔Retry↔Practice (clamped at ends, no wrap).
- `NavVerb::Confirm` (BD) / Enter / Space: `Activate(cursor)`.
- `NavVerb::Back` (SD) / Esc: `ContinueNow`.
- `NavVerb::Practice` (FT): `PracticeNow`.
- Keyboard `R`: `Activate(Retry)` (handled by the driver alongside the
  reducer since `R` has no NavVerb).

Keyboard mapping detail: ←/→ are the natural axis for a horizontal row, but
pads only have Up/Down verbs — the driver maps both keyboard Left/Right and
pad Up/Down onto the same prev/next moves.

Verb effects (driver system, replaces `result_input`):

- **Continue** → `request_transition(AppState::SongSelect)` (unchanged).
- **Retry** → `request_transition(AppState::SongLoading)`. `SelectedSong` and
  `PracticeIntent` are untouched — SongLoading relaunches the same chart; a
  practice run retries as practice. Guard: if `SelectedSong.0.is_none()`
  (defensive; should not happen), fall back to Continue.
- **Practice** → set `PracticeIntent.0 = true`, then
  `request_transition(AppState::SongLoading)`. Same `SelectedSong` guard.

All input systems run `in_state(AppState::Result)`; transition dedup relies on
the existing `request_transition` semantics.

## Architecture

`crates/game-results/src/` splits:

- `lib.rs` — plugin wiring + `save_result` + `SaveStatus` (existing save code
  moves nowhere; only spawn/input move out).
- `ui.rs` — `spawn_result`, `despawn_result`, reveal
  (`animate_staggered_reveal`, `RevealState`), `rank_color`,
  `format_thousands`, layout constants.
- `input.rs` — `ResultVerb`, `reduce_result_nav`, driver system, verb effects.

Plugin registration order: `OnEnter(Result)`: `(save_result, spawn_result)`
chained (unchanged); `Update`: `(skip_or_input, animate_staggered_reveal)`.
`enter_choreo_system` is already registered by dtx-ui.

New dependency edges: none (game-results already depends on game-shell,
gameplay-drums, dtx-ui, dtx-core, dtx-scoring).

## Error handling

- Zero `total_notes`: percents render `0.0%`; rank is `Unknown` → `--` in
  `text_secondary`.
- Missing `LastStageOutcome`: no failed tag.
- Missing `SelectedSong` on Retry/Practice: fall back to Continue (no panic,
  no unwrap — repo rule).

## Testing

Unit (in-crate):

- `reduce_result_nav`: move clamps at both ends; Confirm activates cursor;
  Back → ContinueNow; Practice verb → PracticeNow.
- `rank_color`: total mapping (each Rank variant).
- `format_thousands`: 0, 999, 1000, 912340, u64::MAX boundary sanity.
- Reveal skip: first input with `!done` marks done and does not produce a verb
  action; second input does.
- Keep all existing `save_result` tests unchanged.

Runtime (BRP smoke): play a chart to results; screenshot — verify two-panel
layout, colored judgments, rank letter; press ← ← to reach Practice, Enter →
verify SongLoading→Performance with practice HUD; from a second run press R →
verify reload. (Shift+Enter unreachable via BRP is irrelevant here — Practice
is reachable by cursor.)

## Acceptance criteria

1. Rank letter dominant and rank-colored; judgment rows use judgment colors.
2. No space-padding alignment anywhere; columns align at any count widths.
3. Reveal is staggered OutQuint (slide + fade), completes ≤ ~1.1s, and any
   first input skips it without side effects.
4. Continue / Retry / Practice all work via keyboard and pads per the mapping
   above; legends shown (pad legend MIDI-gated).
5. Save-on-entry behavior and status line byte-for-byte equivalent to today.
6. `cargo check --workspace && cargo clippy --workspace --all-targets -- -D
   warnings` clean; `cargo test -p game-results` green.
