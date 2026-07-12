# Results Screen Rebuild Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the results screen presentation and input (`crates/game-results`) per spec `docs/superpowers/specs/2026-07-12-results-screen-design.md`: two-panel card with a dominant rank-colored rank letter, node-based judgment table (no space-padding), OutQuint staggered slide+fade reveal with input-to-skip, and three verbs (Continue / Retry / Practice) reachable by keyboard and pads. Save-on-entry (`save_result`, `SaveStatus`) behavior stays byte-for-byte identical.

**Architecture:** `game-results` splits into `lib.rs` (plugin + save path, untouched behavior), `ui.rs` (spawn/despawn, reveal, pure display helpers), `input.rs` (verb cursor, pure reducer, driver). Reveal uses the existing `dtx_ui::motion::EnterChoreo` (OutQuint slide, system already registered by dtx-ui) plus a retimed alpha fade driven by `StatRow { reveal_at_ms, target_alpha }` and a `RevealState` resource whose `done` flag gates skip-vs-act in the input driver. A tiny `SelectedDifficulty(u8)` resource is added to game-shell (PracticeIntent precedent) so results can color the Lv chip with the song wheel's difficulty index without a game-menu dependency.

**Tech Stack:** Rust, Bevy 0.19 (messages: `Message` derive, `MessageReader`/`MessageWriter`, `world.write_message` / `Messages::drain` in tests), bevy_ui nodes, existing `dtx-ui` theme/motion/easing, `dtx-scoring::Rank`.

---

## Spec-conflict resolutions (verified against source, plan has final say)

1. **Retry/Practice guard.** Spec says guard on `SelectedSong.0.is_none()`, but `SelectedSong` is `game_menu::song_select` (game-menu-private) and the spec also forbids new dependency edges. Resolution: guard on `ActiveChart.source_path.is_none()` (same defensive intent — a pathless chart has nothing SongLoading could reload; SongLoading itself already degrades with a `warn!` if `SelectedSong` were empty, `song_loading.rs:157`).
2. **Difficulty color source.** The wheel's difficulty index is `game_menu::song_select::Selection.difficulty` — unreachable from game-results. Resolution: new `SelectedDifficulty(pub u8)` resource in `game-shell/src/states.rs` (exact precedent: `PracticeIntent`, which lives there "so game-menu doesn't need gameplay internals"), written in `spawn_loading` (game-menu) which already computes `difficulty_index` on every SongLoading enter (including Retry — `Selection` persists). The spec non-goal forbids changes to game-shell *states or transition plumbing*; a plain resource is neither.
3. **Rank `--`.** `Rank::Unknown` Displays as `"UNKNOWN"` (`dtx-scoring/src/lib.rs:142`), not `--`. UI maps it via a `rank_label` helper.
4. **Easing API.** Spec's `EaseFunction::QuintOut.sample_clamped(t)` doesn't exist here; the real API is `dtx_ui::easing::EaseFunction::OutQuint.ease(t)` (clamps internally, `easing.rs:18-22`).
5. **Skip finishing EnterChoreo.** Spec allows "set elapsed >= delay+duration or remove them". We set `elapsed_ms = delay_ms + duration_ms` and let `enter_choreo_system` snap the transform to zero and remove the component — no deferred-command race.
6. **Legend alpha fade.** `spawn_nav_legend` spawns its own internal text (no `StatRow` hook). The legend block slides in via `EnterChoreo` at the last stagger slot; the keyboard hint line (our own text) gets both slide and fade. Rewriting the shared widget for one alpha fade is not worth it.

## File structure

| File | Status | Contents after this plan |
|---|---|---|
| `crates/game-results/src/lib.rs` | modified | crate doc + lints, `mod ui; mod input;`, `ResultEntity`, `SaveStatus` (`pub(crate)`), `GameResultsPlugin`/`plugin`, `result_rank` (`pub(crate)`), `chart_identity`, `native_score_entry`, `save_result`, existing save tests **untouched** |
| `crates/game-results/src/ui.rs` | new | `spawn_result`, `despawn_result`, `StatRow`, `RevealState`, `reveal_alpha`, `animate_staggered_reveal`, `sync_verb_row`, `VerbLabel`, `rank_color`, `rank_label`, `format_thousands`, `pct`, layout constants + spawn helpers |
| `crates/game-results/src/input.rs` | new | `ResultVerb`, `ResultAction`, `reduce_result_nav`, `result_nav` driver + `apply` |
| `crates/game-shell/src/states.rs` | modified | `SelectedDifficulty(pub u8)` resource |
| `crates/game-shell/src/lib.rs` | modified | re-export + `init_resource::<SelectedDifficulty>()` |
| `crates/game-menu/src/song_loading.rs` | modified | one `insert_resource` line in `spawn_loading` |

Repo rules in force for every task: no `unwrap()` in `crates/*` (`expect` allowed in tests only), Bevy 0.19 message API (`MessageReader`/`MessageWriter`), conventional commit messages, **no co-author trailers**.

---

## Task 1: Module split — lib.rs → ui.rs + input.rs (pure move)

**Files:**
- `crates/game-results/src/ui.rs` (new)
- `crates/game-results/src/input.rs` (new)
- `crates/game-results/src/lib.rs` (modified)

Pure code move, zero behavior change. **Save-status behavior and all `save_result` tests remain untouched in `lib.rs`** — `save_result`, `SaveStatus`, `chart_identity`, `native_score_entry`, and the tests `result_rank_uses_bocud_xg_formula`, `native_score_entry_uses_chart_identity_and_poor_counts`, `save_status_defaults_to_practice`, `save_result_persists_entry_and_sets_saved` stay byte-for-byte (only `SaveStatus` and `result_rank` gain `pub(crate)` so `ui.rs` can see them). The `pct_zero_total_is_zero` test moves with `pct` into `ui.rs`.

- [ ] Create `crates/game-results/src/ui.rs` with the moved presentation code (bodies verbatim from current `lib.rs`, only paths/visibility adjusted):

```rust
//! Results screen presentation: layout spawn/despawn + staggered reveal.

use bevy::prelude::*;
use dtx_ui::{theme::Theme, ThemeResource};
use game_shell::despawn_stage;
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};

use crate::{ResultEntity, SaveStatus};

#[derive(Component)]
struct ResultPanel;

/// Marks a stat row for staggered reveal.
#[derive(Component)]
pub(crate) struct StatRow {
    pub reveal_at_ms: f32,
}

#[derive(Resource)]
pub(crate) struct ResultReveal {
    pub elapsed_ms: f32,
}

const STAGGER_MS: f32 = 120.0;
const FADE_DURATION_MS: f32 = 300.0;

pub(crate) fn pct(count: u32, total: u32) -> f32 {
    if total == 0 {
        0.0
    } else {
        count as f32 / total as f32 * 100.0
    }
}

pub(crate) fn spawn_result(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    midi: Option<Res<game_shell::MidiConnected>>,
    status: Res<SaveStatus>,
) {
    commands.insert_resource(ResultReveal { elapsed_ms: 0.0 });

    let title = chart
        .chart
        .metadata
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let artist = chart
        .chart
        .metadata
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let difficulty = chart
        .chart
        .metadata
        .dlevel
        .map(|v| format!("{:.2}", dtx_core::display_dlevel(v)))
        .unwrap_or_else(|| "--".into());
    let total = scoring.total_notes;
    let rank = crate::result_rank(&counts, combo.max, total);
    let t = theme.0;

    let stat_rows: Vec<(String, f32)> = vec![
        (title.to_string(), 0.0),
        (format!("{artist}  Lv.{difficulty}"), STAGGER_MS),
        (String::new(), STAGGER_MS * 2.0),
        (format!("Score     {}", score.0), STAGGER_MS * 3.0),
        (format!("Max Combo {}", combo.max), STAGGER_MS * 4.0),
        (format!("Rank      {rank}"), STAGGER_MS * 5.0),
        (String::new(), STAGGER_MS * 6.0),
        (
            format!(
                "Perfect   {} ({:.1}%)",
                counts.perfect,
                pct(counts.perfect, total)
            ),
            STAGGER_MS * 7.0,
        ),
        (
            format!(
                "Great     {} ({:.1}%)",
                counts.great,
                pct(counts.great, total)
            ),
            STAGGER_MS * 8.0,
        ),
        (
            format!(
                "Good      {} ({:.1}%)",
                counts.good,
                pct(counts.good, total)
            ),
            STAGGER_MS * 9.0,
        ),
        (
            format!("Poor      {} ({:.1}%)", counts.ok, pct(counts.ok, total)),
            STAGGER_MS * 10.0,
        ),
        (
            format!(
                "Miss      {} ({:.1}%)",
                counts.miss,
                pct(counts.miss, total)
            ),
            STAGGER_MS * 11.0,
        ),
        (format!("Total     {total}"), STAGGER_MS * 12.0),
        (String::new(), STAGGER_MS * 13.0),
        ("ESC / ENTER → Song Select".to_string(), STAGGER_MS * 14.0),
    ];

    let panel = commands
        .spawn((
            ResultEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.bg_bottom),
        ))
        .id();

    let inner = commands
        .spawn((
            ResultPanel,
            Node {
                padding: UiRect::all(Val::Px(48.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                min_width: Val::Px(400.0),
                ..default()
            },
            BackgroundColor(t.panel_bg),
        ))
        .id();

    commands.entity(panel).add_child(inner);

    for (text, delay) in stat_rows {
        if text.is_empty() {
            let spacer = commands
                .spawn((
                    StatRow {
                        reveal_at_ms: delay,
                    },
                    Node {
                        height: Val::Px(16.0),
                        ..default()
                    },
                ))
                .id();
            commands.entity(inner).add_child(spacer);
        } else {
            let row = commands
                .spawn((
                    StatRow {
                        reveal_at_ms: delay,
                    },
                    Text::new(text),
                    Theme::label_font(),
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                ))
                .id();
            commands.entity(inner).add_child(row);
        }
    }

    let (label, color) = match *status {
        SaveStatus::Saved => ("saved ✓", t.clear_green),
        SaveStatus::Failed => ("save failed — score kept this session only", t.judgment_miss),
        SaveStatus::Practice => ("", Color::NONE),
    };
    if !label.is_empty() {
        let row = commands
            .spawn((
                StatRow {
                    reveal_at_ms: STAGGER_MS * 15.0,
                },
                Text::new(label),
                Theme::label_font(),
                TextColor(color.with_alpha(0.0)),
            ))
            .id();
        commands.entity(inner).add_child(row);
    }

    if midi.is_some_and(|m| m.0) {
        commands.entity(inner).with_children(|p| {
            dtx_ui::widget::nav_legend::spawn_nav_legend(p, &t, &[("BD", "continue")]);
        });
    }
}

pub(crate) fn animate_staggered_reveal(
    time: Res<Time>,
    mut reveal: ResMut<ResultReveal>,
    mut q: Query<(&StatRow, &mut TextColor)>,
) {
    reveal.elapsed_ms += time.delta_secs() * 1000.0;
    for (stat, mut color) in &mut q {
        let since = reveal.elapsed_ms - stat.reveal_at_ms;
        if since < 0.0 {
            continue;
        }
        let alpha = (since / FADE_DURATION_MS).clamp(0.0, 1.0);
        color.0 = color.0.with_alpha(alpha);
    }
}

pub(crate) fn despawn_result(commands: Commands, query: Query<Entity, With<ResultEntity>>) {
    despawn_stage::<ResultEntity>(commands, query);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_zero_total_is_zero() {
        assert_eq!(pct(1, 0), 0.0);
    }
}
```

- [ ] Create `crates/game-results/src/input.rs` with `result_input` moved verbatim:

```rust
//! Results screen input.

use bevy::prelude::*;
use game_shell::{request_transition, AppState, TransitionRequest};

pub(crate) fn result_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<game_shell::NavAction>,
    mut requests: MessageWriter<TransitionRequest>,
) {
    use game_shell::NavVerb;
    // Either pad verb continues; the mapper's screen-enter grace keeps the
    // song's last note from skipping this screen.
    let pad = actions
        .read()
        .any(|a| matches!(a.verb, NavVerb::Confirm | NavVerb::Back));
    if pad || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Enter) {
        request_transition(&mut requests, AppState::SongSelect);
    }
}
```

- [ ] Rewrite `crates/game-results/src/lib.rs` header and plugin (everything from `result_rank` down through `save_result` and the remaining four tests stays byte-for-byte, except `result_rank` becomes `pub(crate) fn result_rank`):

```rust
//! CStageResult — animated stat reveals (ADR-0014).

// Bevy systems take many params and queries use deeply nested generic tuples;
// both trip these lints across this crate's systems. Bevy-idiomatic
// false-positives, allowed crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod input;
mod ui;

use bevy::prelude::*;
use dtx_scoring::identity::{canonical_chart_hash, raw_file_sha256, ChartIdentity};
use dtx_scoring::{JudgmentTotals, Rank, ScoreEntry, ScoreSource};
use game_shell::{AppState, ScoreStoreResource};
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};
use gameplay_drums::stage_end::LastStageOutcome;

#[derive(Component)]
pub struct ResultEntity;

/// Outcome of the on-entry persistence attempt, shown as the last stat row.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SaveStatus {
    #[default]
    Practice, // nothing to save
    Saved,
    Failed,
}

pub struct GameResultsPlugin;

impl Plugin for GameResultsPlugin {
    fn build(&self, app: &mut App) {
        plugin(app);
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SaveStatus>()
        .add_systems(
            OnEnter(AppState::Result),
            (save_result, ui::spawn_result).chain(),
        )
        .add_systems(OnExit(AppState::Result), ui::despawn_result)
        .add_systems(
            Update,
            (input::result_input, ui::animate_staggered_reveal)
                .run_if(in_state(AppState::Result)),
        );
}
```

  Deletions from `lib.rs`: `ResultPanel`, `StatRow`, `ResultReveal`, `STAGGER_MS`, `FADE_DURATION_MS`, `pct`, `spawn_result`, `animate_staggered_reveal`, `result_input`, `despawn_result`, the `pct_zero_total_is_zero` test, and the now-unused imports (`dtx_ui::{theme::Theme, ThemeResource}`, `game_shell::{despawn_stage, request_transition, TransitionRequest}`).
- [ ] Run gates: `cargo test -p game-results && cargo clippy -p game-results --all-targets -- -D warnings`. Expect: all 5 existing tests pass (`pct_zero_total_is_zero` now under `ui::tests`), clippy clean.
- [ ] Commit: `refactor(results): split ui and input modules`

## Task 2: Pure display helpers — rank_color + format_thousands (TDD)

**Files:**
- `crates/game-results/src/ui.rs`

- [ ] Add failing tests to `ui.rs` `mod tests`:

```rust
    #[test]
    fn rank_color_total_mapping() {
        let t = Theme::default();
        assert_eq!(rank_color(Rank::SS, &t), t.judgment_perfect);
        assert_eq!(rank_color(Rank::S, &t), t.judgment_perfect);
        assert_eq!(rank_color(Rank::A, &t), t.judgment_great);
        assert_eq!(rank_color(Rank::B, &t), t.judgment_good);
        assert_eq!(rank_color(Rank::C, &t), t.judgment_ok);
        assert_eq!(rank_color(Rank::D, &t), t.judgment_miss);
        assert_eq!(rank_color(Rank::E, &t), t.judgment_miss);
        assert_eq!(rank_color(Rank::Unknown, &t), t.text_secondary);
    }

    #[test]
    fn rank_label_unknown_is_dashes() {
        assert_eq!(rank_label(Rank::Unknown), "--");
        assert_eq!(rank_label(Rank::SS), "SS");
        assert_eq!(rank_label(Rank::A), "A");
    }

    #[test]
    fn format_thousands_boundaries() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1_000), "1,000");
        assert_eq!(format_thousands(912_340), "912,340");
        assert_eq!(format_thousands(u64::MAX), "18,446,744,073,709,551,615");
    }
```

- [ ] Run `cargo test -p game-results rank_color` — expect compile failure: ``error[E0425]: cannot find function `rank_color` in this scope`` (and same for `rank_label`/`format_thousands`).
- [ ] Implement in `ui.rs` (add `use dtx_scoring::Rank;` to the imports):

```rust
/// Rank → theme color: SS/S gold, A green, B blue, C purple, D/E red,
/// Unknown secondary (spec §Left panel).
#[allow(dead_code)] // wired into spawn_result in Task 6
pub(crate) fn rank_color(rank: Rank, theme: &Theme) -> Color {
    match rank {
        Rank::SS | Rank::S => theme.judgment_perfect,
        Rank::A => theme.judgment_great,
        Rank::B => theme.judgment_good,
        Rank::C => theme.judgment_ok,
        Rank::D | Rank::E => theme.judgment_miss,
        Rank::Unknown => theme.text_secondary,
    }
}

/// Rank headline text: `Display` string, except Unknown renders `--`.
#[allow(dead_code)] // wired into spawn_result in Task 6
pub(crate) fn rank_label(rank: Rank) -> String {
    if rank == Rank::Unknown {
        "--".into()
    } else {
        rank.to_string()
    }
}

/// `912340` → `"912,340"` (comma thousands separator).
#[allow(dead_code)] // wired into spawn_result in Task 6
pub(crate) fn format_thousands(v: u64) -> String {
    let digits = v.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
```

- [ ] Run `cargo test -p game-results rank` and `cargo test -p game-results format_thousands` — expect pass. Then `cargo clippy -p game-results --all-targets -- -D warnings` clean.
- [ ] Commit: `feat(results): rank color and thousands separators`

## Task 3: SelectedDifficulty shared resource

**Files:**
- `crates/game-shell/src/states.rs`
- `crates/game-shell/src/lib.rs`
- `crates/game-menu/src/song_loading.rs`

- [ ] Add failing test to `crates/game-shell/src/states.rs` `mod tests`:

```rust
    #[test]
    fn selected_difficulty_defaults_to_basic() {
        assert_eq!(SelectedDifficulty::default().0, 0);
    }
```

- [ ] Run `cargo test -p game-shell selected_difficulty` — expect compile failure: ``error[E0425]: cannot find struct, variant or union type `SelectedDifficulty` ``.
- [ ] Implement in `states.rs`, directly below `PracticeIntent`:

```rust
/// Difficulty index (0 = BASIC) of the chart being played — the same value
/// the song wheel uses. Written by song loading on every SongLoading enter;
/// read by game-results to color the Lv chip. Lives in game-shell so
/// game-results doesn't need game-menu (same precedent as [`PracticeIntent`]).
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectedDifficulty(pub u8);
```

- [ ] In `crates/game-shell/src/lib.rs`, add `SelectedDifficulty` to the `pub use states::{...}` list and add `.init_resource::<states::SelectedDifficulty>()` next to the existing `.init_resource::<states::PracticeIntent>()` line.
- [ ] In `crates/game-menu/src/song_loading.rs` `spawn_loading` (already has `mut commands: Commands`), immediately after `let difficulty_index = selection.difficulty;` (line ~313), add:

```rust
    commands.insert_resource(game_shell::SelectedDifficulty(difficulty_index));
```

  (No unit test for the write — `spawn_loading` needs `AssetServer`/`SongDb`; covered by the controller's BRP smoke.)
- [ ] Run gates: `cargo test -p game-shell selected_difficulty && cargo check -p game-menu && cargo clippy -p game-shell -p game-menu --all-targets -- -D warnings`. Expect pass/clean.
- [ ] Commit: `feat(shell): share selected difficulty index with results`

## Task 4: ResultVerb + reduce_result_nav reducer (TDD)

**Files:**
- `crates/game-results/src/input.rs`

- [ ] Add failing tests to a new `mod tests` in `input.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_result_nav_moves_and_clamps() {
        use ResultVerb::{Continue, Practice, Retry};
        // Clamped at both ends, no wrap.
        assert_eq!(reduce_result_nav(Continue, NavVerb::Up), ResultAction::None);
        assert_eq!(
            reduce_result_nav(Practice, NavVerb::Down),
            ResultAction::None
        );
        // Moves along Continue ↔ Retry ↔ Practice.
        assert_eq!(
            reduce_result_nav(Continue, NavVerb::Down),
            ResultAction::Moved(Retry)
        );
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Down),
            ResultAction::Moved(Practice)
        );
        assert_eq!(
            reduce_result_nav(Practice, NavVerb::Up),
            ResultAction::Moved(Retry)
        );
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Up),
            ResultAction::Moved(Continue)
        );
        // Dec/Inc alias the same axis (keyboard adjust verbs).
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Dec),
            ResultAction::Moved(Continue)
        );
        assert_eq!(
            reduce_result_nav(Retry, NavVerb::Inc),
            ResultAction::Moved(Practice)
        );
    }

    #[test]
    fn reduce_result_nav_confirm_activates_cursor() {
        assert_eq!(
            reduce_result_nav(ResultVerb::Retry, NavVerb::Confirm),
            ResultAction::Activate(ResultVerb::Retry)
        );
    }

    #[test]
    fn reduce_result_nav_back_and_practice_shortcuts() {
        assert_eq!(
            reduce_result_nav(ResultVerb::Retry, NavVerb::Back),
            ResultAction::ContinueNow
        );
        assert_eq!(
            reduce_result_nav(ResultVerb::Continue, NavVerb::Practice),
            ResultAction::PracticeNow
        );
    }
}
```

- [ ] Run `cargo test -p game-results reduce_result_nav` — expect compile failure: ``error[E0425]: cannot find function `reduce_result_nav` in this scope``.
- [ ] Implement in `input.rs` (add `NavVerb` to the `game_shell` import list: `use game_shell::{request_transition, AppState, NavVerb, TransitionRequest};`):

```rust
/// The verb the cursor sits on. Resets to Continue on every Result enter.
#[allow(dead_code)] // wired into the driver + spawn in Tasks 6-7
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ResultVerb {
    #[default]
    Continue,
    Retry,
    Practice,
}

#[allow(dead_code)] // wired into the driver in Task 7
impl ResultVerb {
    fn prev(self) -> Self {
        match self {
            ResultVerb::Continue | ResultVerb::Retry => ResultVerb::Continue,
            ResultVerb::Practice => ResultVerb::Retry,
        }
    }

    fn next(self) -> Self {
        match self {
            ResultVerb::Continue => ResultVerb::Retry,
            ResultVerb::Retry | ResultVerb::Practice => ResultVerb::Practice,
        }
    }
}

/// What one nav verb means given the current cursor. Pure.
#[allow(dead_code)] // wired into the driver in Task 7
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultAction {
    Moved(ResultVerb),
    Activate(ResultVerb),
    ContinueNow,
    PracticeNow,
    None,
}

/// HH/CY (Up/Down) and keyboard ←/→ (mapped to Up/Down by the driver) move
/// the cursor, clamped at the ends. BD/Enter activates, SD/Esc continues,
/// FT jumps to practice.
#[allow(dead_code)] // wired into the driver in Task 7
pub(crate) fn reduce_result_nav(cursor: ResultVerb, verb: NavVerb) -> ResultAction {
    match verb {
        NavVerb::Up | NavVerb::Dec => {
            let moved = cursor.prev();
            if moved == cursor {
                ResultAction::None
            } else {
                ResultAction::Moved(moved)
            }
        }
        NavVerb::Down | NavVerb::Inc => {
            let moved = cursor.next();
            if moved == cursor {
                ResultAction::None
            } else {
                ResultAction::Moved(moved)
            }
        }
        NavVerb::Confirm => ResultAction::Activate(cursor),
        NavVerb::Back => ResultAction::ContinueNow,
        NavVerb::Practice => ResultAction::PracticeNow,
    }
}
```

- [ ] Run `cargo test -p game-results reduce_result_nav` — expect 3 tests pass. `cargo clippy -p game-results --all-targets -- -D warnings` clean.
- [ ] Commit: `feat(results): result verb reducer`

## Task 5: Reveal retime — RevealState, OutQuint fade, target alpha (TDD)

**Files:**
- `crates/game-results/src/ui.rs`

Replaces `ResultReveal` with `RevealState { elapsed_ms, total_ms, done }`, retimes `STAGGER_MS`/`FADE_DURATION_MS` to 60/350, eases the alpha with OutQuint, adds `target_alpha` so `text_secondary`-colored spans and the 0.25-alpha divider fade to their own alpha (not 1.0), and adds `BackgroundColor` fading for node dividers. The old (task-1) layout keeps working through a mechanical patch; Task 6 replaces it.

- [ ] Add failing tests to `ui.rs` `mod tests`:

```rust
    #[test]
    fn reveal_alpha_is_zero_before_slot() {
        assert_eq!(reveal_alpha(59.0, 60.0, 1.0), 0.0);
    }

    #[test]
    fn reveal_alpha_is_outquint_front_loaded() {
        // OutQuint at t=0.5 is 1 - 0.5^5 = 0.96875 — well past linear.
        let a = reveal_alpha(FADE_DURATION_MS * 0.5, 0.0, 1.0);
        assert!(a > 0.9, "expected front-loaded ease, got {a}");
    }

    #[test]
    fn reveal_alpha_caps_at_target() {
        assert_eq!(reveal_alpha(10_000.0, 0.0, 0.5), 0.5);
    }

    #[test]
    fn reveal_state_new_totals_last_slot_plus_fade() {
        let s = RevealState::new(13.0);
        assert_eq!(s.total_ms, 13.0 * STAGGER_MS + FADE_DURATION_MS);
        assert!(!s.done);
    }

    #[test]
    fn animate_marks_done_at_timeout() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(Time::default());
        world.insert_resource(RevealState {
            elapsed_ms: 2_000.0,
            total_ms: 1_130.0,
            done: false,
        });
        world
            .run_system_once(animate_staggered_reveal)
            .expect("system runs");
        assert!(world.resource::<RevealState>().done);
    }
```

- [ ] Run `cargo test -p game-results reveal_` — expect compile failure: ``error[E0425]: cannot find function `reveal_alpha` in this scope`` / ``cannot find struct ... `RevealState` ``.
- [ ] In `ui.rs`: add `use dtx_ui::easing::EaseFunction;` to the imports, change the constants, extend `StatRow`, and replace `ResultReveal` + `animate_staggered_reveal`:

```rust
pub(crate) const STAGGER_MS: f32 = 60.0;
pub(crate) const FADE_DURATION_MS: f32 = 350.0;

/// Marks a revealed element: fade starts at `reveal_at_ms`, rises to
/// `target_alpha` (the element's authored alpha, e.g. 0.5 for
/// `text_secondary`) over `FADE_DURATION_MS` with OutQuint.
#[derive(Component)]
pub(crate) struct StatRow {
    pub reveal_at_ms: f32,
    pub target_alpha: f32,
}

/// Reveal progress for the whole screen. `done` flips on timeout or on the
/// first input (skip); while `!done` the input driver consumes everything.
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct RevealState {
    pub elapsed_ms: f32,
    pub total_ms: f32,
    pub done: bool,
}

impl RevealState {
    pub(crate) fn new(last_slot: f32) -> Self {
        Self {
            elapsed_ms: 0.0,
            total_ms: last_slot * STAGGER_MS + FADE_DURATION_MS,
            done: false,
        }
    }
}

/// Eased alpha for one element at `elapsed_ms`. Pure.
pub(crate) fn reveal_alpha(elapsed_ms: f32, reveal_at_ms: f32, target_alpha: f32) -> f32 {
    let since = elapsed_ms - reveal_at_ms;
    if since < 0.0 {
        return 0.0;
    }
    EaseFunction::OutQuint.ease((since / FADE_DURATION_MS).clamp(0.0, 1.0)) * target_alpha
}

pub(crate) fn animate_staggered_reveal(
    time: Res<Time>,
    mut reveal: ResMut<RevealState>,
    mut q: Query<(&StatRow, Option<&mut TextColor>, Option<&mut BackgroundColor>)>,
) {
    if reveal.done {
        return;
    }
    reveal.elapsed_ms += time.delta_secs() * 1000.0;
    for (stat, text, bg) in &mut q {
        let alpha = reveal_alpha(reveal.elapsed_ms, stat.reveal_at_ms, stat.target_alpha);
        if let Some(mut c) = text {
            c.0 = c.0.with_alpha(alpha);
        }
        if let Some(mut b) = bg {
            b.0 = b.0.with_alpha(alpha);
        }
    }
    if reveal.elapsed_ms >= reveal.total_ms {
        reveal.done = true;
    }
}
```

- [ ] Mechanical patch of the old (task-1) `spawn_result` so it compiles against the new types — three edits:
  - `commands.insert_resource(ResultReveal { elapsed_ms: 0.0 });` → `commands.insert_resource(RevealState::new(15.0));`
  - Both `StatRow { reveal_at_ms: delay, }` literals (spacer and text row) gain `target_alpha: 1.0,` (replace_all).
  - The save-status row's `StatRow { reveal_at_ms: STAGGER_MS * 15.0, }` gains `target_alpha: 1.0,`.
- [ ] Run `cargo test -p game-results` — expect all tests pass (5 new + existing). `cargo clippy -p game-results --all-targets -- -D warnings` clean.
- [ ] Commit: `feat(results): outquint staggered reveal with skip state`

## Task 6: New spawn_result layout (TDD)

**Files:**
- `crates/game-results/src/ui.rs`

Full layout rewrite per spec §Layout: two-panel card (max 900px, 48px padding) on `bg_bottom`; header band (title 28 + artist/Lv 16 with `difficulty_color`); left rank panel (160px rank letter, conditional `STAGE FAILED`); right stats table (node columns 120/80px, judgment colors, divider, MAX COMBO, thousands-separated SCORE, save line); verb row + legends. Every element carries `EnterChoreo` slide + `StatRow` fade at its stagger slot. **Save-status line strings and colors stay byte-for-byte** (`"saved ✓"` `clear_green` / `"save failed — score kept this session only"` `judgment_miss` / nothing for Practice), only the font size becomes 14 per spec.

- [ ] Add failing tests to `ui.rs` `mod tests`:

```rust
    fn spawn_world() -> World {
        let mut world = World::new();
        world.insert_resource(ThemeResource::default());
        world.insert_resource(Score(912_340));
        world.insert_resource(Combo {
            current: 0,
            max: 214,
        });
        world.insert_resource(JudgmentCounts {
            perfect: 412,
            great: 61,
            good: 12,
            ok: 6,
            miss: 9,
        });
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: None,
        });
        world.insert_resource(DrumScoring {
            total_notes: 500,
            ..Default::default()
        });
        world.insert_resource(game_shell::SelectedDifficulty(2));
        world.insert_resource(SaveStatus::Saved);
        world
    }

    fn all_texts(world: &mut World) -> Vec<String> {
        let mut q = world.query::<&Text>();
        q.iter(world).map(|t| t.0.clone()).collect()
    }

    #[test]
    fn spawn_result_builds_verbs_columns_and_score() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = spawn_world();
        world
            .run_system_once(spawn_result)
            .expect("spawn_result runs");

        let mut verb_q = world.query::<&VerbLabel>();
        assert_eq!(
            verb_q.iter(&world).count(),
            3,
            "Continue / Retry / Practice"
        );

        let texts = all_texts(&mut world);
        assert!(texts.iter().any(|s| s == "912,340"), "score separated: {texts:?}");
        assert!(texts.iter().any(|s| s == "saved ✓"), "save line kept");
        // 412/500 perfect, 61 great, combo 214 → XG rate 80.73 → S.
        assert!(texts.iter().any(|s| s == "S"), "rank letter: {texts:?}");
        assert!(
            !texts.iter().any(|s| s == "STAGE FAILED"),
            "no failed tag without LastStageOutcome"
        );
        // Column layout, not space padding: count is its own text node.
        assert!(texts.iter().any(|s| s == "412"));
        assert!(texts.iter().any(|s| s == "82.4%"));
    }

    #[test]
    fn spawn_result_colors_judgment_labels() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = spawn_world();
        world
            .run_system_once(spawn_result)
            .expect("spawn_result runs");
        let t = Theme::default();
        let mut q = world.query::<(&Text, &TextColor)>();
        let (_, color) = q
            .iter(&world)
            .find(|(text, _)| text.0 == "PERFECT")
            .expect("PERFECT label exists");
        assert_eq!(color.0, t.judgment_perfect.with_alpha(0.0));
    }

    #[test]
    fn spawn_result_failed_tag_and_unknown_rank() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = spawn_world();
        world.insert_resource(LastStageOutcome { cleared: false });
        world.insert_resource(DrumScoring {
            total_notes: 0,
            ..Default::default()
        });
        world
            .run_system_once(spawn_result)
            .expect("spawn_result runs");
        let texts = all_texts(&mut world);
        assert!(texts.iter().any(|s| s == "STAGE FAILED"));
        assert!(texts.iter().any(|s| s == "--"), "Unknown rank renders --");
        assert!(texts.iter().any(|s| s == "0.0%"), "zero total → 0.0%");
    }
```

- [ ] Run `cargo test -p game-results spawn_result` — expect compile failure (``cannot find struct ... `VerbLabel` ``; `spawn_result` signature mismatch once rewritten).
- [ ] Rewrite the presentation half of `ui.rs`. New imports block:

```rust
use bevy::prelude::*;
use dtx_scoring::Rank;
use dtx_ui::easing::EaseFunction;
use dtx_ui::motion::EnterChoreo;
use dtx_ui::{theme::Theme, ThemeResource};
use game_shell::{despawn_stage, SelectedDifficulty};
use gameplay_drums::resources::{ActiveChart, Combo, DrumScoring, JudgmentCounts, Score};
use gameplay_drums::stage_end::LastStageOutcome;

use crate::input::ResultVerb;
use crate::{ResultEntity, SaveStatus};
```

  Delete the old `spawn_result`, the `stat_rows` vec, the spacer/text spawn loop, and the `#[allow(dead_code)]` attributes on `rank_color`, `rank_label`, `format_thousands`. Add the layout constants and helpers:

```rust
// Layout (spec §Layout).
const CARD_MAX_WIDTH: f32 = 900.0;
const CARD_PADDING: f32 = 48.0;
const LABEL_COL: f32 = 120.0;
const COUNT_COL: f32 = 80.0;

// Motion (spec §Motion): 24px upward slide, 350ms OutQuint per element.
const SLIDE_OFFSET: Vec2 = Vec2::new(0.0, 24.0);
const SLIDE_DURATION_MS: f32 = 350.0;

// Stagger slots × STAGGER_MS: header → rank → failed tag → judgments →
// divider → combo → score → save → verbs → legends. Last fade ends at
// 13 × 60 + 350 = 1130ms (~1.1s).
const SLOT_HEADER: f32 = 0.0;
const SLOT_RANK: f32 = 1.0;
const SLOT_FAILED: f32 = 2.0;
const SLOT_JUDGE_FIRST: f32 = 3.0; // five rows: slots 3..=7
const SLOT_TABLE_DIVIDER: f32 = 8.0;
const SLOT_COMBO: f32 = 9.0;
const SLOT_SCORE: f32 = 10.0;
const SLOT_SAVE: f32 = 11.0;
const SLOT_VERBS: f32 = 12.0;
const SLOT_LEGEND: f32 = 13.0;
pub(crate) const LAST_SLOT: f32 = SLOT_LEGEND;

/// Marks one verb-row label; `sync_verb_row` renders the cursor onto it.
#[derive(Component)]
pub(crate) struct VerbLabel(pub ResultVerb);

/// Verb label text with a width-stable selection prefix.
pub(crate) fn verb_text(verb: ResultVerb, selected: bool) -> String {
    let name = match verb {
        ResultVerb::Continue => "Continue",
        ResultVerb::Retry => "Retry",
        ResultVerb::Practice => "Practice",
    };
    if selected {
        format!("▸ {name}")
    } else {
        format!("  {name}")
    }
}

/// Text element bundle with fade (to the color's own alpha) + slide at `slot`.
fn reveal_text(
    text: impl Into<String>,
    font: TextFont,
    color: Color,
    slot: f32,
) -> (Text, TextFont, TextColor, StatRow, EnterChoreo, UiTransform) {
    (
        Text::new(text),
        font,
        TextColor(color.with_alpha(0.0)),
        StatRow {
            reveal_at_ms: slot * STAGGER_MS,
            target_alpha: color.alpha(),
        },
        EnterChoreo::slide(SLIDE_OFFSET, slot * STAGGER_MS, SLIDE_DURATION_MS),
        UiTransform::default(),
    )
}

/// 1px horizontal rule fading to a quarter-alpha `text_secondary`.
fn divider(parent: &mut ChildSpawnerCommands, t: &Theme, slot: f32) {
    let color = t.text_secondary.with_alpha(0.25);
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(color.with_alpha(0.0)),
        StatRow {
            reveal_at_ms: slot * STAGGER_MS,
            target_alpha: color.alpha(),
        },
        EnterChoreo::slide(SLIDE_OFFSET, slot * STAGGER_MS, SLIDE_DURATION_MS),
        UiTransform::default(),
    ));
}
```

- [ ] Add the new `spawn_result` and section helpers to `ui.rs`:

```rust
pub(crate) fn spawn_result(
    mut commands: Commands,
    theme: Res<ThemeResource>,
    score: Res<Score>,
    combo: Res<Combo>,
    counts: Res<JudgmentCounts>,
    chart: Res<ActiveChart>,
    scoring: Res<DrumScoring>,
    difficulty: Res<SelectedDifficulty>,
    outcome: Option<Res<LastStageOutcome>>,
    midi: Option<Res<game_shell::MidiConnected>>,
    status: Res<SaveStatus>,
) {
    commands.insert_resource(RevealState::new(LAST_SLOT));
    commands.insert_resource(ResultVerb::default());

    let t = theme.0;
    let title = chart
        .metadata()
        .title
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let artist = chart
        .metadata()
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown".into());
    let dlevel = chart
        .metadata()
        .dlevel
        .map(|v| format!("{:.2}", dtx_core::display_dlevel(v)))
        .unwrap_or_else(|| "--".into());
    let total = scoring.total_notes;
    let rank = crate::result_rank(&counts, combo.max, total);
    let failed = outcome.is_some_and(|o| !o.cleared);
    let midi_connected = midi.is_some_and(|m| m.0);

    commands
        .spawn((
            ResultEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.bg_bottom),
        ))
        .with_children(|root| {
            root.spawn((
                ResultPanel,
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(CARD_MAX_WIDTH),
                    padding: UiRect::all(Val::Px(CARD_PADDING)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
                BackgroundColor(t.panel_bg),
            ))
            .with_children(|card| {
                spawn_header(card, &t, &title, &artist, &dlevel, difficulty.0);
                divider(card, &t, SLOT_HEADER);
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(32.0),
                    ..default()
                })
                .with_children(|body| {
                    spawn_rank_panel(body, &t, rank, failed);
                    spawn_stats_panel(body, &t, &counts, total, combo.max, score.0, *status);
                });
                divider(card, &t, SLOT_VERBS);
                spawn_verb_row(card, &t);
                spawn_legends(card, &t, midi_connected);
            });
        });
}

fn spawn_header(
    card: &mut ChildSpawnerCommands,
    t: &Theme,
    title: &str,
    artist: &str,
    dlevel: &str,
    difficulty: u8,
) {
    card.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        ..default()
    })
    .with_children(|head| {
        head.spawn(reveal_text(
            title,
            Theme::font(28.0),
            t.text_primary,
            SLOT_HEADER,
        ));
        head.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|meta| {
            meta.spawn(reveal_text(
                format!("{artist} ·"),
                Theme::font(16.0),
                t.text_secondary,
                SLOT_HEADER,
            ));
            meta.spawn(reveal_text(
                format!("Lv {dlevel}"),
                Theme::font(16.0),
                t.difficulty_color(difficulty),
                SLOT_HEADER,
            ));
        });
    });
}

fn spawn_rank_panel(body: &mut ChildSpawnerCommands, t: &Theme, rank: Rank, failed: bool) {
    body.spawn(Node {
        width: Val::Percent(40.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|left| {
        left.spawn(reveal_text(
            rank_label(rank),
            Theme::font(160.0),
            rank_color(rank, t),
            SLOT_RANK,
        ));
        if failed {
            left.spawn(reveal_text(
                "STAGE FAILED",
                Theme::font(16.0),
                t.judgment_miss,
                SLOT_FAILED,
            ));
        }
    });
}

fn spawn_stats_panel(
    body: &mut ChildSpawnerCommands,
    t: &Theme,
    counts: &JudgmentCounts,
    total: u32,
    max_combo: u32,
    score: u64,
    status: SaveStatus,
) {
    body.spawn(Node {
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(6.0),
        ..default()
    })
    .with_children(|right| {
        let rows = [
            ("PERFECT", counts.perfect),
            ("GREAT", counts.great),
            ("GOOD", counts.good),
            ("POOR", counts.ok),
            ("MISS", counts.miss),
        ];
        for (i, (label, count)) in rows.into_iter().enumerate() {
            judgment_row(right, t, label, count, total, SLOT_JUDGE_FIRST + i as f32);
        }
        divider(right, t, SLOT_TABLE_DIVIDER);
        value_row(
            right,
            t,
            "MAX COMBO",
            &max_combo.to_string(),
            Theme::font(18.0),
            SLOT_COMBO,
        );
        value_row(
            right,
            t,
            "SCORE",
            &format_thousands(score),
            Theme::font(28.0),
            SLOT_SCORE,
        );
        match status {
            SaveStatus::Saved => {
                right.spawn(reveal_text(
                    "saved ✓",
                    Theme::font(14.0),
                    t.clear_green,
                    SLOT_SAVE,
                ));
            }
            SaveStatus::Failed => {
                right.spawn(reveal_text(
                    "save failed — score kept this session only",
                    Theme::font(14.0),
                    t.judgment_miss,
                    SLOT_SAVE,
                ));
            }
            SaveStatus::Practice => {}
        }
    });
}

fn judgment_row(
    parent: &mut ChildSpawnerCommands,
    t: &Theme,
    label: &str,
    count: u32,
    total: u32,
    slot: f32,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(LABEL_COL),
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(reveal_text(
                    label,
                    Theme::font(18.0),
                    t.judgment_color(label),
                    slot,
                ));
            });
            row.spawn(Node {
                width: Val::Px(COUNT_COL),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(reveal_text(
                    count.to_string(),
                    Theme::font(18.0),
                    t.text_primary,
                    slot,
                ));
            });
            row.spawn(reveal_text(
                format!("{:.1}%", pct(count, total)),
                Theme::font(14.0),
                t.text_secondary,
                slot,
            ));
        });
}

fn value_row(
    parent: &mut ChildSpawnerCommands,
    t: &Theme,
    label: &str,
    value: &str,
    value_font: TextFont,
    slot: f32,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Baseline,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(LABEL_COL),
                ..default()
            })
            .with_children(|cell| {
                cell.spawn(reveal_text(label, Theme::font(14.0), t.text_secondary, slot));
            });
            row.spawn(reveal_text(value, value_font, t.text_primary, slot));
        });
}

fn spawn_verb_row(card: &mut ChildSpawnerCommands, t: &Theme) {
    card.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::Center,
        column_gap: Val::Px(32.0),
        ..default()
    })
    .with_children(|row| {
        for verb in [ResultVerb::Continue, ResultVerb::Retry, ResultVerb::Practice] {
            let selected = verb == ResultVerb::default();
            let color = if selected { t.accent } else { t.text_secondary };
            row.spawn((
                VerbLabel(verb),
                reveal_text(verb_text(verb, selected), Theme::font(20.0), color, SLOT_VERBS),
            ));
        }
    });
}

fn spawn_legends(card: &mut ChildSpawnerCommands, t: &Theme, midi_connected: bool) {
    card.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            ..default()
        },
        EnterChoreo::slide(SLIDE_OFFSET, SLOT_LEGEND * STAGGER_MS, SLIDE_DURATION_MS),
        UiTransform::default(),
    ))
    .with_children(|legends| {
        if midi_connected {
            dtx_ui::widget::nav_legend::spawn_nav_legend(
                legends,
                t,
                &[
                    ("HH/CY", "move"),
                    ("BD", "select"),
                    ("SD", "continue"),
                    ("FT", "practice"),
                ],
            );
        }
        legends.spawn(reveal_text(
            "←/→ move · Enter select · R retry · Esc continue",
            Theme::font(12.0),
            t.text_secondary,
            SLOT_LEGEND,
        ));
    });
}
```

  Also remove the `#[allow(dead_code)]` from `ResultVerb` in `input.rs` (now constructed here); keep the allows on `impl ResultVerb`, `ResultAction`, and `reduce_result_nav` until Task 7.
- [ ] Run `cargo test -p game-results` — expect all tests pass (3 new spawn tests + prior). Note the old `result_input` still drives Continue-only until Task 7 — the screen stays usable at this commit.
- [ ] `cargo clippy -p game-results --all-targets -- -D warnings` clean.
- [ ] Commit: `feat(results): two-panel layout with rank headline`

## Task 7: Input driver — skip, cursor sync, verb effects (TDD)

**Files:**
- `crates/game-results/src/input.rs`
- `crates/game-results/src/ui.rs`
- `crates/game-results/src/lib.rs`

- [ ] Add failing tests to `input.rs` `mod tests` (append after the reducer tests):

```rust
    use bevy::ecs::message::Messages;
    use bevy::ecs::system::RunSystemOnce;
    use dtx_ui::motion::EnterChoreo;
    use game_shell::{NavAction, NavSource, PracticeIntent};
    use gameplay_drums::resources::ActiveChart;

    use crate::ui::{RevealState, StatRow};

    fn driver_world() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<NavAction>>();
        world.init_resource::<Messages<game_shell::TransitionRequest>>();
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ResultVerb::default());
        world.insert_resource(PracticeIntent::default());
        world.insert_resource(RevealState {
            elapsed_ms: 2_000.0,
            total_ms: 1_130.0,
            done: true,
        });
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: Some(std::path::PathBuf::from("song.dtx")),
        });
        world
    }

    fn pad(verb: NavVerb) -> NavAction {
        NavAction {
            verb,
            source: NavSource::Pad,
            coarse: false,
        }
    }

    fn drain_requests(world: &mut World) -> Vec<AppState> {
        world
            .resource_mut::<Messages<game_shell::TransitionRequest>>()
            .drain()
            .map(|r| r.0)
            .collect()
    }

    #[test]
    fn result_nav_back_continues_to_song_select() {
        let mut world = driver_world();
        world.write_message(pad(NavVerb::Back));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongSelect]);
    }

    #[test]
    fn result_nav_moves_cursor_then_confirm_retries() {
        let mut world = driver_world();
        world.write_message(pad(NavVerb::Down));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(*world.resource::<ResultVerb>(), ResultVerb::Retry);
        assert!(drain_requests(&mut world).is_empty());

        world.resource_mut::<Messages<NavAction>>().clear();
        world.write_message(pad(NavVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
        assert!(!world.resource::<PracticeIntent>().0, "plain retry keeps intent");
    }

    #[test]
    fn result_nav_ft_jumps_to_practice() {
        let mut world = driver_world();
        world.write_message(pad(NavVerb::Practice));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
        assert!(world.resource::<PracticeIntent>().0);
    }

    #[test]
    fn result_nav_r_key_retries() {
        let mut world = driver_world();
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongLoading]);
    }

    #[test]
    fn result_nav_retry_without_source_falls_back_to_continue() {
        let mut world = driver_world();
        world.insert_resource(ActiveChart {
            chart: dtx_core::Chart::default(),
            source_path: None,
        });
        world.insert_resource(ResultVerb::Retry);
        world.write_message(pad(NavVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongSelect]);
        assert!(!world.resource::<PracticeIntent>().0);
    }

    #[test]
    fn result_nav_first_input_skips_reveal_without_acting() {
        let mut world = driver_world();
        world.insert_resource(RevealState {
            elapsed_ms: 100.0,
            total_ms: 1_130.0,
            done: false,
        });
        let text = world
            .spawn((
                StatRow {
                    reveal_at_ms: 600.0,
                    target_alpha: 0.5,
                },
                TextColor(Color::WHITE.with_alpha(0.0)),
            ))
            .id();
        let slid = world
            .spawn(EnterChoreo::slide(Vec2::new(0.0, 24.0), 600.0, 350.0))
            .id();

        // First input: consumed, finishes the reveal, no verb action.
        world.write_message(pad(NavVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert!(world.resource::<RevealState>().done);
        assert!(drain_requests(&mut world).is_empty(), "skip consumes input");
        let color = world.get::<TextColor>(text).expect("text kept");
        assert_eq!(color.0.alpha(), 0.5, "alpha snapped to target");
        let choreo = world.get::<EnterChoreo>(slid).expect("choreo kept");
        assert!(choreo.finished(), "choreo fast-forwarded");

        // Second input acts normally.
        world.resource_mut::<Messages<NavAction>>().clear();
        world.write_message(pad(NavVerb::Confirm));
        world.run_system_once(result_nav).expect("driver runs");
        assert_eq!(drain_requests(&mut world), vec![AppState::SongSelect]);
    }
```

- [ ] Run `cargo test -p game-results result_nav` — expect compile failure: ``error[E0425]: cannot find function `result_nav` in this scope``.
- [ ] In `input.rs`: delete `result_input`, delete the remaining `#[allow(dead_code)]` attributes (on `impl ResultVerb`, `ResultAction`, `reduce_result_nav`), and add the driver. Final non-test imports:

```rust
use bevy::prelude::*;
use dtx_ui::motion::EnterChoreo;
use game_shell::{
    request_transition, AppState, NavAction, NavVerb, PracticeIntent, TransitionRequest,
};
use gameplay_drums::resources::ActiveChart;

use crate::ui::{RevealState, StatRow};
```

  Driver:

```rust
/// Results input driver. While the reveal is running, the first input of any
/// kind finishes it and is consumed; afterwards pads and keys drive the verb
/// row through `reduce_result_nav`.
pub(crate) fn result_nav(
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: MessageReader<NavAction>,
    mut cursor: ResMut<ResultVerb>,
    mut reveal: ResMut<RevealState>,
    mut practice_intent: ResMut<PracticeIntent>,
    chart: Res<ActiveChart>,
    mut requests: MessageWriter<TransitionRequest>,
    mut fades: Query<(&StatRow, Option<&mut TextColor>, Option<&mut BackgroundColor>)>,
    mut sliding: Query<&mut EnterChoreo>,
) {
    // Pads (mapper's screen-enter grace already filters the song's last
    // notes) + keyboard, folded onto the same verbs. ←/→ are the natural
    // axis for a horizontal row; pads reuse Up/Down.
    let mut verbs: Vec<NavVerb> = actions.read().map(|a| a.verb).collect();
    if keys.just_pressed(KeyCode::ArrowLeft) {
        verbs.push(NavVerb::Up);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        verbs.push(NavVerb::Down);
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        verbs.push(NavVerb::Confirm);
    }
    if keys.just_pressed(KeyCode::Escape) {
        verbs.push(NavVerb::Back);
    }
    let retry_key = keys.just_pressed(KeyCode::KeyR);
    if verbs.is_empty() && !retry_key {
        return;
    }

    if !reveal.done {
        // Skip: snap every fade to its target, fast-forward every slide
        // (enter_choreo_system zeroes the transform and removes it), and
        // consume the input.
        reveal.done = true;
        for (stat, text, bg) in &mut fades {
            if let Some(mut c) = text {
                c.0 = c.0.with_alpha(stat.target_alpha);
            }
            if let Some(mut b) = bg {
                b.0 = b.0.with_alpha(stat.target_alpha);
            }
        }
        for mut choreo in &mut sliding {
            choreo.elapsed_ms = choreo.delay_ms + choreo.duration_ms;
        }
        return;
    }

    for verb in verbs {
        let action = reduce_result_nav(*cursor, verb);
        apply(action, &mut cursor, &mut practice_intent, &chart, &mut requests);
    }
    if retry_key {
        apply(
            ResultAction::Activate(ResultVerb::Retry),
            &mut cursor,
            &mut practice_intent,
            &chart,
            &mut requests,
        );
    }
}

/// Applies one reduced action. Retry/Practice fall back to Continue when the
/// chart has no source path (nothing SongLoading could reload — defensive,
/// stands in for the spec's missing-SelectedSong guard without a game-menu
/// dependency edge).
fn apply(
    action: ResultAction,
    cursor: &mut ResultVerb,
    practice_intent: &mut PracticeIntent,
    chart: &ActiveChart,
    requests: &mut MessageWriter<TransitionRequest>,
) {
    match action {
        ResultAction::Moved(v) => *cursor = v,
        ResultAction::ContinueNow | ResultAction::Activate(ResultVerb::Continue) => {
            request_transition(requests, AppState::SongSelect);
        }
        ResultAction::Activate(ResultVerb::Retry) => {
            if chart.source_path.is_some() {
                // SelectedSong + PracticeIntent are untouched: SongLoading
                // relaunches the same chart; a practice run retries as practice.
                request_transition(requests, AppState::SongLoading);
            } else {
                request_transition(requests, AppState::SongSelect);
            }
        }
        ResultAction::PracticeNow | ResultAction::Activate(ResultVerb::Practice) => {
            if chart.source_path.is_some() {
                practice_intent.0 = true;
                request_transition(requests, AppState::SongLoading);
            } else {
                request_transition(requests, AppState::SongSelect);
            }
        }
        ResultAction::None => {}
    }
}
```

- [ ] Add `sync_verb_row` to `ui.rs` (cursor render sync; preserves fade alpha while revealing, full theme colors once done) plus its test:

```rust
/// Renders the verb cursor: selected = accent + `▸ ` prefix, others =
/// secondary + two-space prefix (row width stays stable). While the reveal
/// runs, the fade's current alpha is preserved.
pub(crate) fn sync_verb_row(
    theme: Res<ThemeResource>,
    cursor: Res<ResultVerb>,
    reveal: Res<RevealState>,
    mut q: Query<(&VerbLabel, &mut Text, &mut TextColor)>,
) {
    let t = theme.0;
    for (label, mut text, mut color) in &mut q {
        let selected = label.0 == *cursor;
        let next = verb_text(label.0, selected);
        if text.0 != next {
            text.0 = next;
        }
        let target = if selected { t.accent } else { t.text_secondary };
        color.0 = if reveal.done {
            target
        } else {
            target.with_alpha(color.0.alpha())
        };
    }
}
```

  Test in `ui.rs` `mod tests`:

```rust
    #[test]
    fn sync_verb_row_renders_cursor() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(ThemeResource::default());
        world.insert_resource(ResultVerb::Retry);
        world.insert_resource(RevealState {
            elapsed_ms: 2_000.0,
            total_ms: 1_130.0,
            done: true,
        });
        let t = Theme::default();
        let retry = world
            .spawn((
                VerbLabel(ResultVerb::Retry),
                Text::new(verb_text(ResultVerb::Retry, false)),
                TextColor(t.text_secondary),
            ))
            .id();
        let cont = world
            .spawn((
                VerbLabel(ResultVerb::Continue),
                Text::new(verb_text(ResultVerb::Continue, true)),
                TextColor(t.accent),
            ))
            .id();
        world.run_system_once(sync_verb_row).expect("sync runs");
        assert_eq!(world.get::<Text>(retry).expect("text").0, "▸ Retry");
        assert_eq!(world.get::<TextColor>(retry).expect("color").0, t.accent);
        assert_eq!(world.get::<Text>(cont).expect("text").0, "  Continue");
        assert_eq!(
            world.get::<TextColor>(cont).expect("color").0,
            t.text_secondary
        );
    }
```

- [ ] Update the plugin in `lib.rs` (driver first, then cursor render, then fade — deterministic order):

```rust
pub fn plugin(app: &mut App) {
    app.init_resource::<SaveStatus>()
        .init_resource::<input::ResultVerb>()
        .add_systems(
            OnEnter(AppState::Result),
            (save_result, ui::spawn_result).chain(),
        )
        .add_systems(OnExit(AppState::Result), ui::despawn_result)
        .add_systems(
            Update,
            (input::result_nav, ui::sync_verb_row, ui::animate_staggered_reveal)
                .chain()
                .run_if(in_state(AppState::Result)),
        );
}
```

- [ ] Run `cargo test -p game-results` — expect all pass (6 driver tests + sync test + all prior). `cargo clippy -p game-results --all-targets -- -D warnings` clean.
- [ ] Commit: `feat(results): three-verb input driver with reveal skip`

## Task 8: Final gates

**Files:** none (verification only; commit only if a fix is needed)

- [ ] Run the full gate exactly as the spec requires:

```
cargo check --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p game-results
```

  Expect: check clean, clippy clean, all game-results tests green.
- [ ] Also run `cargo test -p game-shell -p game-menu` (both crates were touched in Task 3). Expect green.
- [ ] Confirm the save path is untouched: `git diff main -- crates/game-results/src/lib.rs` shows `save_result`, `chart_identity`, `native_score_entry`, and the four save/rank tests unchanged apart from the module split (visibility on `SaveStatus`/`result_rank`, moved imports).
- [ ] If anything fails, fix minimally and commit as `fix(results): <what>`.

---

## Acceptance criteria → task map

| Spec criterion | Tasks |
|---|---|
| 1. Rank letter dominant + rank-colored; judgment rows judgment-colored | 2, 6 |
| 2. No space-padding; node columns align at any widths | 6 |
| 3. Staggered OutQuint slide+fade, ≤ ~1.1s, first input skips without side effects | 5, 6, 7 |
| 4. Continue/Retry/Practice via keyboard + pads; legends (pad legend MIDI-gated) | 3, 4, 6, 7 |
| 5. Save-on-entry + status line byte-for-byte equivalent | 1 (explicit), 6 (strings/colors), 8 (diff check) |
| 6. Workspace check + clippy -D warnings clean; game-results tests green | every task's gate, 8 |

## Verification

- Gates (executor, per task and finally in Task 8): `cargo check --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p game-results`.
- Runtime BRP smoke is done by the **controller after implementation**, not by task executors (driving notes: `/home/lazykern/.claude/projects/-home-lazykern-lab-dtxmaniars/memory/brp-smoke-driving.md`). Outline: launch, play a chart to Result; screenshot → two-panel card, colored judgment rows, big rank letter, thousands-separated score, Lv chip in difficulty color; first key press mid-reveal snaps everything visible with no transition; → → then Enter reaches Practice and lands in Performance with the practice HUD; from a second run press R → SongLoading reloads the same chart; Esc → SongSelect; with a MIDI device the HH/CY·BD·SD·FT legend shows.
