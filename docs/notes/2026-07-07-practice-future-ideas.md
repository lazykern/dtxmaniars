# Practice mode — future feature ideas (parked)

Noted 2026-07-07 during practice-scoring design discussion. Deliberately
NOT in scope for practice v3 (scoring semantics). Revisit after
foundation phase 0 (chart hash / section identity / persistence).

## Per-lane diagnosis widget

Per-lane accuracy + signed timing bias ("snare fine, hihat −18ms
rushing, kick 60% on doubles"). `delta_ms` already collected per
judgment; lane is discarded today. Candidate for a **normal-game widget
too** (dtx-layout WidgetKind), not practice-only — post-song per-lane
breakdown on results, live version in practice full HUD.

## ~~Limb layering / lane isolation~~ (dropped 2026-07-11)

Decided not to build. Select-lanes-to-play with autoplay for the rest —
cut from the backlog.

## ~~Auto-suggest practice sections~~ (dropped 2026-07-11)

Decided not to build. Mine-miss-history → one-button section suggestion —
cut from the backlog.

## Wait mode

Chart halts until correct pad hit, no clock. Best tool for learning
fills note-by-note. Already listed as trainer phase 3.

## Layout: per-kind visibility defaults vs saved entries

v3 changed `default_instance` so score widgets (ScorePanel/PhraseMeter/
LiveGraph/SongProgress) default hidden in practice. Gap: `WidgetEntry`'s
`visible_play`/`visible_practice` use serde `default = default_true` (a
constant), and `resolve()` replaces wholesale — so an *explicit* saved
layout entry for a score widget (written because its position differed
from default) keeps the OLD `visible_practice = true`, silently defeating
the new default for that one saved layout. Fresh/default layouts are
fine. Proper fix: make `visible_*` `Option<bool>`, fall back to
`default_instance(kind)` in `to_instance` when `None`, and
`skip_serializing_if` when equal to the kind default. Deferred — it's a
layout-format change, orthogonal to the practice training model.

## Count-in click / metronome

Metronome click during pre-roll + visual 4-3-2-1; optional click through
loop. Fixes "silent rewind" feel of current pre-roll. Cheapest of these;
first candidate when picking practice work after v3.
