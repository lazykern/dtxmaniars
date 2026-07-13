# DTXManiaRS End-to-End Improvement Program

Date: 2026-07-13
Status: Program direction approved; individual written designs require review
Owner: Codex, executing autonomously between written-spec gates
Product scope: drums and electronic drums
Explicit exclusion: CI/CD configuration and workflows

## Objective

Implement every player-facing and maintainability improvement accepted in the
2026-07-13 project review. Work proceeds in small, independently verifiable
cycles so correctness, references, tests, and commits remain reviewable. This
ledger is the durable inventory; completing one cycle never removes later work.

## Governing rules

- Mechanics and DTX format behavior are ported from `references/DTXmaniaNX/`.
- UX follows `PRODUCT.md` and the existing osu-inspired design direction.
- `references/` remains read-only.
- Each cycle receives its own design, implementation plan, tests, and logical
  commits.
- New behavior is tested before implementation where practical.
- Package tests, workspace check, workspace Clippy, and formatting are local
  release gates. CI/CD files are out of scope.
- Unsupported structures that prevent faithful play reject the chart;
  unsupported optional assets are surfaced as explicit load warnings.
- Modified or assisted play never silently competes with an unmodified score.

## Program order and status

| Cycle | Outcome | Status |
|---|---|---|
| 0 | Restore a truthful local quality baseline | Implemented |
| 1 | Playback-rate and score integrity | Implemented |
| 2 | Core DTX and audio compatibility | Implemented |
| 3 | Reliable guided calibration | Implemented |
| 4 | Results analysis and weakest-section practice handoff | Implemented |
| 5 | Large-library discovery and measured scan performance | Implemented |
| 6 | Accessibility and design-system consolidation | Queued |
| 7 | Extended format/media compatibility | Queued |
| 8 | Documentation and repository-maintenance repair | Queued |

## Cycle 0 — Truthful local quality baseline

Repair the pre-existing workspace formatting failures before feature changes so
the full local gate can distinguish new regressions from inherited noise. Keep
the cleanup mechanical and isolated in its own commit. CI/CD remains excluded.

Baseline observed on 2026-07-13:

- `cargo check --workspace` passed;
- `cargo clippy --workspace --all-targets -- -D warnings` passed;
- `cargo test --workspace --lib` passed all 1,460 library tests;
- `cargo fmt --all -- --check` failed in twelve existing source/test files
  across `dtx-layout`, `dtx-scoring`, `game-results`, and `game-menu`;
- `crates/dtx-core/tests/comprehensive.rs` contains at least one tautological
  assertion that does not prove behavior and must be replaced in Cycle 8.

## Cycle 1 — Playback-rate and score integrity

Unify normal Play Speed and practice tempo around one chart-time clock. Notes,
BGM, chart sounds, BGA, seeking, pause, restart, and stage completion must use
the same effective rate. Practice tempo overrides the normal Play Speed setting
rather than multiplying it.

Modified-speed normal results remain visible but do not write ordinary history,
PB, rank, or compatible `score.ini` records. Results state why the run was not
saved. A future modifier-aware score schema may retain assisted history, but it
must not be smuggled into the current unqualified schema.

## Cycle 2 — Core DTX and audio compatibility

Deliver the compatibility defects already proven by source comparison:

- case-insensitive `.dtx` discovery;
- `#RANDOM` / `#IF` / `#ENDIF` conditional parsing;
- SE01 through SE32 channel support;
- MP3 chart audio and fallback-BGM discovery;
- structured scan/load diagnostics;
- a real compatibility fixture matrix covering filename case, UTF-8,
  Shift-JIS, UTF-16, conditional branches, BPM/bar changes, MP3, OGG, WAV,
  high-numbered SE channels, missing assets, and malformed input.

## Cycle 3 — Reliable guided calibration

Replace selected-song calibration with a synthetic, constant and known timing
sequence. Collect enough samples to show median offset, spread, rejected
outliers, and confidence. Low-confidence results are never auto-applied.
Explain input offset separately from BGM adjustment, preserve manual controls,
and support keyboard and MIDI taps through the same timestamped input path.

The design must cover audio output latency, visual timing, refresh/frame-time
observations, device disconnect, cancellation, and restoration of the prior
runtime state.

## Cycle 4 — Results analysis and practice handoff

Record a bounded normal-play event stream containing lane, judgment, signed
timing error, chip index, and chart time. Derive:

- early/late bias;
- timing spread;
- weakest lane;
- weakest chart section;
- personal-best delta when comparable history exists.

Keep the main results hierarchy compact. Put detailed distribution and lane /
section analysis behind a focused details surface. Replace the boolean practice
intent with a structured intent carrying loop bounds, pre-roll, initial tempo,
and recommendation reason. “Practice weakest section” opens the existing
practice mode with that loop already selected. Practice results remain excluded
from normal score persistence.

Completed 2026-07-13:

- Bounded, in-memory normal-play telemetry records lane, judgment, signed
  timing error, chip index, and chart time; practice telemetry remains
  excluded.
- Results derives median early/late bias, MAD timing spread, weighted weakest
  lane, and a bar-aligned weakest-section loop.
- Results snapshots comparable PB deltas before persistence and keeps
  diagnostics in a focused details surface.
- Recommended practice uses a typed game-shell request and applies the loop,
  one-bar pre-roll, and 1.0 tempo to the existing transport.

## Cycle 5 — Large-library discovery

Add focused, combinable discovery tools:

- Favorites;
- Unplayed;
- Recent;
- Near My Level;
- Random Within Results.

Preserve active filters and selected chart where possible. Empty results must
name the active constraint and offer a clear reset. Measure startup scan time,
rescan time, parsed chart count, skipped count, and on-demand chart-stat cost
before adding a cache or database. A cache is allowed only after representative
libraries demonstrate a player-visible need.

Completed 2026-07-13:

- A separate versioned `library-preferences.json` persists favorites by
  normalized chart path and safely falls back to empty favorites when it cannot
  be read.
- Song Select now combines Favorites, Unplayed, Recent, and Near My Level with
  search and sorting. Recent uses score history; Near My Level uses the median
  completed chart level (±1 display level); imported history still counts as a
  play.
- F7 toggles the highlighted favorite; Ctrl+1 through Ctrl+4 toggle discovery
  filters; Ctrl+R chooses deterministically from visible filtered rows; Ctrl+0
  resets filters. Recompute restores the selected chart by path when possible.
- Empty results state their active filters and provide the reset command.
- Startup and F5 rescan report elapsed time, directories, parsed charts, and
  skips. The existing asynchronous selected-chart stat parse now publishes its
  completion cost. No cache or database was introduced: measurement comes
  first.

## Cycle 6 — Accessibility and design-system consolidation

Add scalable HUD text, reduced flashes/background motion, stronger
color-plus-shape selection cues, and an explicit No Fail presentation for the
existing no-damage mode. Promote critical text that remains at desk-only sizes.
Consolidate accent/focus/error meanings, the type scale, shared dialogs,
buttons, and toast behavior. Clamp or recover off-screen widget placement and
finish remaining drag/selection affordances.

This cycle protects existing strengths: the spring song wheel, BPM glow,
OutQuint entrances, velocity meter, profile transaction model, lane direct
manipulation, and widget anchor/origin visualization.

## Cycle 7 — Extended format/media compatibility

Complete compatibility work that should not inflate the core DTX fix:

- evaluate and implement GDA/BMS/BME only with dedicated fixtures and explicit
  product support statements;
- evaluate XA decoding or provide a precise unsupported-media recovery path;
- evaluate pitch-preserving time stretch as an opt-in alternative to the
  NX-compatible pitch-changing rate mode;
- port remaining relevant NX channels, including hidden drum lanes, BGA swaps,
  BGA/AVI pan semantics, MIDI chorus, mixer add/remove, and SE extensions not
  covered by the first compatibility slice;
- support metadata aliases and decimal level directives such as `PLAYLEVEL`
  and `DLVDEC`;
- add box/folder organization only if it improves the approved discovery flow.

## Cycle 8 — Documentation and repository maintenance

Repair all stale `DTXmaniaNX-BocuD` links to the actual reference root, restore
or replace the missing roadmap link, reconcile conflicting transition rules,
and reconstruct binding ADRs that are currently described as lost. Expand the
README with installation dependencies, song location/import, supported media,
controls, MIDI setup, troubleshooting, data locations, and score-modifier
behavior.

Remove tautological or placeholder tests when replacing them with
behavior-bearing tests. No CI/CD workflow work is included.

## Cross-cycle acceptance

The program is complete only when:

- normal speed changes cannot desynchronize the chart and audio;
- modified runs cannot overwrite or masquerade as ordinary records;
- supported DTX charts are discovered independent of filename case;
- conditional charts select exactly one valid branch;
- supported media either plays or produces a player-visible diagnosis;
- calibration reports confidence and never applies weak evidence;
- results explain a weakness and can open practice at that section;
- large libraries can be filtered and explored without hidden state;
- routine actions communicate focus/state without relying on color alone;
- player and contributor documentation matches the executable behavior;
- local format, check, Clippy, and relevant package/integration tests pass.
