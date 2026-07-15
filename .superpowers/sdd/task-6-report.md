# Task 6 report

## Result

- Added one `PracticeSetupRoot` for Setup and Editing.
- Uses a settings/playfield split at roomy sizes and Setup/Progress/Preview tabs at narrow sizes or larger text scales.
- Keeps the existing gameplay scene as the preview region and shows `PREVIEW: INPUT IS NOT JUDGED` wherever the preview contract is visible.
- Pins `Start Practice` or `Continue Practice` below the scrollable settings content.
- Replaced the paused full rail with a full-width draft-backed timeline containing Task 5 preview transport, density, bar ticks, playhead, loop fill, and labeled A/B handles.
- Removed `full_hud.rs` and the legacy `PracticePauseSurface::Rail` path.
- Keeps the mini strip and status chip for Running while hiding them during Setup/Editing.

## TDD evidence

- Initial shell RED: unresolved `hud::setup` import.
- Expanded shell RED: missing stable-region and timeline marker components.
- Responsive RED: 900x720 Standard incorrectly selected Split before minimum preview width was made structural.
- Lifecycle RED: setup root remained after leaving Performance.
- Compact-HUD RED: mini strip and chip stayed visible during Setup.
- A/B marker RED: timeline had no labeled draft handles.

## Verification

- `cargo test -p gameplay-drums --test practice_hud`
- `cargo test -p gameplay-drums --lib practice::hud`
- `cargo test -p gameplay-drums --test practice_mode`
- `cargo test -p gameplay-drums --all-targets`
- `cargo check -p gameplay-drums --all-targets`
- `cargo clippy -p gameplay-drums --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Product-register decisions

- Preserved the existing restrained dark theme and semantic accent roles.
- Used familiar tabs, settings rows, and a single obvious primary action.
- Used structural split-to-tabbed responsiveness rather than shrinking typography.
- Used semantic typography and visible selected/A/B markers so state is not color-only.
- Kept the content as flat regions with one scroll boundary instead of nested cards.

## Concern

- Task 6 intentionally stops at shell placeholders. Task 7 still owns keyboard/pad focus, typed setup actions, preset persistence, and detailed Progress controls.

## Review fixes

- Tabbed mode now collapses its inactive pane with `Display::None`; computed layout verifies that the visible pane fills the content width and the inactive pane resolves to zero width.
- Split pane widths now come from the same settings and preview minima used by `practice_layout_mode`, including live Split-to-Split resize at 1920x1080 Extra Large text scale.
- Setup values, Progress metrics, lane diagnosis, and the primary action refresh from live draft/session/flow resources. Timeline drag mutations refresh Setup copy later in the same ordered update chain.
- Resizing Tabbed Preview into Split normalizes selection to Setup and preserves the visible check marker; returning to Tabbed remains coherent.
- The B handle is right-anchored so its minimum width stays inside the strip at chart end.
- Integration coverage now runs Bevy UI layout against a camera/window target, asserts `ComputedNode` geometry and overflow, drives tab-button interaction, performs real mouse timeline seek/drag gestures, and verifies Running-phase shell cleanup.

## Review TDD evidence

- RED: Tabbed settings and preview each resolved to half of the available width because hidden panes remained in flex layout.
- RED: 1920x1080 Extra Large resolved the settings pane to 730 px instead of its required 900 px.
- RED: a live resize that remained in Split mode retained the old 400 px settings width instead of recomputing to 900 px.
- RED: draft/session mutation and a real timeline drag left Setup and Progress copy stale.
- RED: Tabbed Preview remained selected after a Split resize.
- RED: the chart-end B handle extended 20 px beyond the computed strip rectangle.
- GREEN: all corrected behavior and boundary tests pass while preserving the exact `PREVIEW: INPUT IS NOT JUDGED` label.

## Review verification

- `cargo test -p gameplay-drums --test practice_hud`
- `cargo test -p gameplay-drums --lib practice::hud`
- `cargo test -p gameplay-drums --test practice_mode`
- `cargo test -p gameplay-drums --all-targets`
- `cargo check -p gameplay-drums --all-targets`
- `cargo clippy -p gameplay-drums --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Review UI rationale

- Kept the existing restrained product vocabulary and exact preview contract copy.
- Made responsiveness structural instead of shrinking text or allowing hidden content to influence layout.
- Preserved non-color selection markers through responsive transitions.
- Added no Task 7 controls, persistence, focus model, or preset behavior.

## Re-review fixes

- Moved the shared Practice title, preview contract, and tab buttons above the
  mutually exclusive settings and preview pane bodies. Tabbed Preview now
  retains visible, clickable Setup, Progress, and Preview navigation.
- Made `PlayfieldLayout` consume the computed `PracticePreviewRegion` rect
  during Setup and Editing. Existing backboard, lane strip, notes, key caps,
  and playfield-anchored widget placement therefore share one fitted geometry;
  no preview notes or second renderer are spawned.
- Kept responsive shell sizing based on the primary window rather than the
  fitted playfield resource, avoiding a layout feedback loop. Running and
  non-practice phases restore full-window playfield geometry.
- Reset `PracticeTab` to Setup on each fresh Performance lifecycle while an
  in-place Editing session retains its selected tab.
- Reset `TimelineGesture` when the cursor, window, strip, practice surface, or
  Performance lifecycle is lost. Releasing outside the strip cancels instead
  of emitting a stale seek; a later click starts from Idle.

## Re-review TDD evidence

- RED: Tabbed Preview despawned the only tab buttons, so Preview could not
  click back to Setup.
- RED: `PlayfieldLayout` remained full-window while the computed preview pane
  only reserved transparent space.
- RED: the global Practice tab survived a fresh Performance entry.
- RED: timeline early returns preserved Pending or DragLoop, and an outside
  release could emit the old press seek.
- GREEN: real Bevy computed-node coverage verifies Preview-to-Setup navigation,
  pane visibility, and fitted playfield/backboard/drum-strip bounds at
  1280x720 Standard and 1920x1080 Extra Large.
- GREEN: lifecycle coverage verifies full-window Running restoration, fresh
  Setup tab reset, Editing retention, and release-outside/cursor-loss re-entry.

## Re-review verification

- `cargo test -p gameplay-drums --test practice_hud`
- `cargo test -p gameplay-drums --lib practice::hud`
- `cargo test -p gameplay-drums --test practice_mode`
- `cargo test -p gameplay-drums --all-targets`
- `cargo check -p gameplay-drums --all-targets`
- `cargo clippy -p gameplay-drums --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Release-frame fixes

- Tab presses now run before shell reconciliation. Deferred despawns are visible
  to the reconciler, which respawns the selected pane and persistent chrome in
  the same update. Layout-mode changes use the same no-gap reconciliation.
- The setup shell now owns a deterministic `PracticePreviewGeometry` handoff.
  It derives the fitted stage rect from the window, accessibility multiplier,
  selected tab, and the shell's shared chrome/timeline dimensions. The
  playfield sync and its render consumers are explicitly ordered after that
  handoff in `Update`, before Bevy computes and renders UI layout. This avoids
  both prior-frame `ComputedNode` reads and a playfield/shell feedback loop;
  computed-node tests remain the steady-state geometry oracle.
- Performance exit now uses an unconditional timeline-gesture clear, separate
  from the phase-conditional update reset.

## Release-frame TDD evidence

- RED: one scheduled Preview-tab press left zero setup roots for the frame.
- RED: the initial shell frame retained full-window origin `(0, 0)` while the
  computed preview began at `(400, 48)`.
- RED: the first 1920x1080 resize frame retained origin `(400, 48)` while the
  resized preview began at `(600, 48)`.
- RED: both `Pending` and `DragLoop` gestures survived Performance exit.
- GREEN: initial, resize, tab, responsive-reconciliation, consumer-ordering,
  and both gesture lifecycle regressions pass after exactly one update each.

## Release-frame verification

- `cargo test -p gameplay-drums --test practice_hud`
- `cargo test -p gameplay-drums --lib practice::hud`
- `cargo test -p gameplay-drums --test practice_mode`
- `cargo test -p gameplay-drums --all-targets`
- `cargo check -p gameplay-drums --all-targets`
- `cargo clippy -p gameplay-drums --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Accessibility geometry fix

- Replaced the fixed 48/88 preview subtraction with one deterministic chrome
  geometry calculation shared by the setup handoff and explicit header/timeline
  node heights.
- Semantic heading/label growth follows the live accessibility text multiplier.
  The timeline reserves a second row below the scaled 560 px transport
  breakpoint, preserving all transport controls and the density strip.
- The headless HUD harness now loads the real `dtx_ui::plugin`. Steady-state
  coverage verifies semantic Standard and Extra Large typography at reference,
  1080p, and wrapped narrow widths; the handoff equals the real preview node and
  every visible chrome descendant remains inside its computed bounds.
- A live resize plus Standard-to-Extra-Large policy change verifies the preview
  handoff, playfield sync, and downstream consumer see the new geometry in the
  same update.

## Accessibility geometry TDD evidence

- RED: at 480x720, both Standard and Extra Large tabbed Preview computed a
  480x560 region after the transport wrapped, while the fixed-offset handoff
  incorrectly reported 480x584.
- GREEN: shared explicit chrome geometry makes computed layout and deterministic
  handoff identical without reading prior-frame layout or adding feedback.
