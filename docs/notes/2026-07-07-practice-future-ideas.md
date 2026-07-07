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

## Limb layering / lane isolation

Select lanes to play; unselected lanes autoplay (keysound fires, not
judged). Kick+snare first, add hihat, add fills — how drummers actually
learn. DTX keysound model makes autoplay-per-lane natural.

## Auto-suggest practice sections

Mine miss/accuracy history for worst bars → one-button "practice this
fill" (pre-set A/B + rate). Needs persistence (phase 0) to be useful.

## Wait mode

Chart halts until correct pad hit, no clock. Best tool for learning
fills note-by-note. Already listed as trainer phase 3.

## Count-in click / metronome

Metronome click during pre-roll + visual 4-3-2-1; optional click through
loop. Fixes "silent rewind" feel of current pre-roll. Cheapest of these;
first candidate when picking practice work after v3.
