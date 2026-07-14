# Practice Mode Overhaul Design

Status: approved in design review on 2026-07-14.

## Context

Practice already supports whole-song and A/B loops, tempo changes, pre-roll,
count-in, wait mode, accuracy ramping, attempt history, lane diagnosis, and a
recommended weak-section handoff. The current interaction model exposes those
features through a quick HUD while running and a Tab-opened rail implemented as
a special paused surface.

The overhaul keeps those mechanics and replaces the rail-centered interaction
model. Every practice request first opens a setup surface with a real playfield
preview. Pause remains a fast interruption menu. Practice Settings becomes the
place to edit the loop, transport, trainer, and saved presets.

This is product UX under ADR-0010. It does not change reference-derived chart,
judgment, scoring, lane, or timing mechanics.

## Goals

- Open Practice Setup before every practice run, including recommended and
  saved-loop requests.
- Show the real chart and audio position while editing without producing fake
  gameplay results.
- Give Pause and Practice Settings distinct, predictable behavior.
- Let a player save chart-specific loop, transport, and trainer presets.
- Preserve expert keyboard shortcuts and kit-driven navigation.
- Keep completed loop attempts comparable in Progress and ramp evaluation.
- Reuse the loaded Performance playfield, chart, BGA, and audio instead of
  building a second preview renderer.

## Non-goals

- Changing judgment windows, score formulas, lane order, scroll math, or audio
  clock authority.
- Adding autoplay, automatic hits, preview judgments, or preview statistics.
- Copying a reference game's practice layout.
- Replacing full profile or device configuration from the pause menu.
- Building a general preset browser outside Practice Setup.

## Selected approach

Practice gains an explicit flow inside `AppState::Performance`. The existing
`PracticeSession` remains the runtime mechanics owner. A separate draft and
flow state distinguish setup, non-judged preview, and a practice run.

Alternatives considered:

1. Expanding the current paused rail would cost less, but setup would remain a
   paused run and optional preview playback would conflict with
   `PauseState::Paused`.
2. A separate application screen would make initial setup clear, but it would
   require duplicate chart, playfield, and audio-preview ownership. Reopening
   settings during practice would still require an in-Performance surface.

The explicit in-Performance flow gives Setup its own semantics while reusing
the authoritative gameplay presentation.

## State and navigation model

```text
Song Select / Results
        | Practice request
        v
+-------------------- PRACTICE SETUP --------------------+
| Preview: stopped by default                            |
| Esc: cancel practice                                   |
| Start Practice: commit draft and seek to pre-roll      |
+--------------------------+------------------------------+
                           v
                    PRACTICE RUNNING
                    |              |
               Esc  |              | Tab
                    v              v
             PRACTICE PAUSED   PRACTICE SETTINGS
             Resume exact      Mark pass ineligible
             Restart loop      Keep editing cursor
             Settings -------> Preview stopped
             Exit              Continue from pre-roll
```

The practice flow has three durable phases:

- `Setup`: initial configuration before the first attempt;
- `Running`: gameplay input and practice mechanics are active; and
- `Editing`: the shared setup surface reopened from a running session.

Preview stopped/playing is a child state of Setup or Editing, not another
practice phase. `PauseState` remains orthogonal and owns only the pause menu.

Entry and exit rules:

- Song Select and Results practice requests carry their origin and seed into
  Practice Setup.
- Esc from Song Select-origin Setup returns to Song Select.
- Esc from Results-origin Setup returns to Results while the completed-run
  context remains available. If that context is unavailable, it returns to
  Song Select and reports the fallback.
- `Practice This Section` from a normal-play pause ends the scored run and
  opens Setup with a provisional loop. Cancelling this Setup returns to Song
  Select; it cannot restore the ended scored run.
- Esc from Editing restores the frozen chart/audio position and opens Practice
  Paused. It does not resume playback.
- `Start Practice` and `Continue Practice` commit the draft, seek to pre-roll,
  run the configured count-in, and begin a fresh eligible attempt.

Ordinary Pause and Practice Settings differ by contract:

- Pause freezes audio and chart time. Resume continues from that exact
  position with the current session configuration.
- Settings freezes the run, creates a draft, and preserves the playhead as the
  editing cursor. Continue commits the draft and restarts from pre-roll.
- Opening Settings marks the interrupted attempt ineligible for history and
  ramp evaluation. If the player cancels Settings and resumes, the run
  continues at the frozen position, but no attempt becomes eligible until the
  next loop boundary.

## Setup and Settings interface

Setup and Settings share one interface. The primary action reads `Start
Practice` on initial entry and `Continue Practice` when reopened.

```text
+----------------------------+-------------------------------+
| PRACTICE                   | PREVIEW: INPUT IS NOT JUDGED  |
| [SETUP] [PROGRESS]         |                               |
|                            |       actual playfield        |
| LOOP                       |       actual chart notes      |
| Source  Saved: Chorus      |       no judgment effects     |
| A       Bar 24 / 0:43.2    |       no score or combo       |
| B       Bar 28 / 0:51.4    |                               |
|                            |                               |
| TRANSPORT                  |                               |
| Tempo   0.80x              |                               |
| Snap    Bar                |                               |
| Pre-roll 1 bar             |                               |
| Count-in On                |                               |
|                            |                               |
| TRAINER                    |                               |
| Mode    Ramp               |                               |
|   Start / Target / Step    |                               |
|   Pass / Required passes   |                               |
|                            |                               |
| [START PRACTICE]           |                               |
+----------------------------+-------------------------------+
| Back  Play Preview  0:46.8   A=======<>=======B  bar ticks |
+------------------------------------------------------------+
```

The left panel owns two tabs:

- `Setup` contains loop, transport, trainer, and preset controls.
- `Progress` contains completed-attempt history, accuracy or flow, timing bias,
  and per-lane diagnosis.

The right side renders the actual playfield and chart notes at the preview
position. It omits score, combo, gauge, hit/miss effects, and judgment popups.
The persistent `PREVIEW: INPUT IS NOT JUDGED` label communicates the input
contract from a distance.

The bottom timeline owns preview play/pause, seek, A/B handles, bar ticks, note
density, and current time. Mouse users can click to seek and drag either loop
handle. Keyboard and pad users can reach equivalent focused controls.

At the 1280x720 reference size and normal text scale, the interface uses the
split layout. At narrow aspect ratios or when larger text cannot fit without
overlap, it switches to `Settings | Preview` tabs. It does not shrink text or
place the settings panel over the playfield. All geometry follows the existing
reference-pixel and safe-area system.

## Loop sources and draft behavior

The Loop Source row offers:

- Whole Song;
- Last Used;
- Recommended Section, when supplied by Results or normal-play pause;
- saved presets for the loaded chart; and
- Custom.

Selecting a source fills `PracticeDraft` and moves the editing cursor to the
source start. It does not start preview playback. Editing a loaded source
changes only the draft.

`PracticeDraft` contains:

- optional A/B bounds;
- snap divisor;
- user tempo;
- pre-roll;
- count-in state; and
- one trainer mode with its applicable configuration.

Trainer mode is an enum with `Off`, `Wait`, and `Ramp`. This makes wait and ramp
mutually exclusive by construction. Ramp reveals start tempo, target tempo,
step, pass threshold, and required successful passes. Off and Wait hide those
rows.

Draft validation clamps tempo and chart positions to supported ranges. Reversed
A/B bounds are normalized. Bounds that cannot form a positive loop fall back
to Whole Song and produce a visible warning.

## Preview contract

Preview begins stopped whenever Setup or Settings opens. The player must invoke
`Play Preview` to start it.

Preview:

- uses the loaded chart, playfield, BGA, audio, and authoritative chart clock;
- loops the current A/B region or the whole song when A/B is unset;
- uses the tempo at which the next attempt will begin;
- moves chart notes and the timeline playhead;
- accepts menu navigation and timeline editing; and
- produces no judgments, misses, score, combo, gauge changes, attempt data,
  lane diagnosis, wait halts, or ramp evaluation.

For Ramp, preview uses the configured start tempo. For Wait, preview plays
without halting. Count-in and trainer progression run only after Start or
Continue Practice.

Opening Settings records a frozen runtime snapshot before preview can seek.
Esc from Settings stops preview, restores the frozen audio and chart position,
and opens Practice Paused.

## Saved practice presets

Saved presets use `CONFIG_DIR/practice-presets.toml`, a version 1 registry
written through `dtx-persistence` atomic replacement. The registry stays
separate from general settings and score history.

`dtx-config` owns the pure schema and file operations. Gameplay computes a
`PracticeChartKey` from the canonical chart hash and selected difficulty index.
Both fields participate in lookup, so two difficulty selections cannot share
presets by accident. The registry also keeps source-path metadata as a display
and recovery hint; the path does not participate in lookup.

A saved preset contains:

- stable preset id and optional player name;
- practice chart key and display metadata;
- A/B bounds as chart milliseconds;
- snap, tempo, pre-roll, and count-in; and
- trainer mode and ramp configuration.

The UI resolves the stored bounds against the loaded timeline and derives an
automatic label such as `Bars 24-28 / 0:43.2-0:51.4` when a player leaves the
name blank. Player names are trimmed, limited to 48 characters, reject control
characters, and must be unique within one practice chart key without regard to
case. `Save as New` creates a preset. `Update Saved Loop` appears only for an
existing saved source. Delete requires confirmation. Preset edits remain
session-only until the player invokes one of these actions.

Last Used is one automatic snapshot per practice chart key. It updates when
the player starts or continues practice, not while editing. Recommended and
Custom drafts never become named presets without an explicit save action.

## Pause menus and input ownership

Normal and practice pause menus stay small:

```text
NORMAL PLAY PAUSED              PRACTICE PAUSED
------------------              ---------------
Resume                          Resume
Restart Song                    Restart Loop
Practice This Section           Practice Settings
Quick Settings                  Exit to Song Select
Return to Song Select
```

`Practice This Section` builds a provisional bar-aligned loop around the
current playhead and opens Practice Setup. `Quick Settings` contains only
scroll speed, lane visibility, BGM volume, and input offset. Full MIDI device,
profile, and layout editing remain in Customize.

Input ownership follows the active phase:

```text
Practice Run       drum inputs -> gameplay
Pause              drum inputs -> menu navigation
Setup/Settings     drum inputs -> menu navigation
Preview playing    drum inputs -> menu navigation
```

- Esc opens Pause during a run.
- Tab opens Practice Settings directly during Practice Running.
- A visible Practice Settings action supports mouse users.
- The bound system Pause input opens the same pause menu from a kit.
- HH/CY navigate, BD confirms, and SD performs the surface's back or resume
  action.
- Existing A/B, tempo, and restart shortcuts remain active during Practice
  Running. A loop or tempo change makes the current pass ineligible and keeps
  the existing ramp-disarm rules.

The interface shows the current input legend for each phase. Focus and
selection use shape or text markers in addition to color.

## Component ownership

```text
dtx-config
  `- PracticePresetRegistry schema and file operations
       `- atomic replacement through dtx-persistence

game-shell
  `- cross-screen PracticeIntent, seed, and origin

gameplay-drums/practice
  |- PracticeFlow             Setup / Running / Editing
  |- PracticeDraft            uncommitted settings
  |- PracticeEditSnapshot     frozen runtime position
  |- PreviewController        non-judged playback
  |- PracticeSession          committed mechanics
  `- setup UI                 Setup / Progress / timeline
```

The current quick practice HUD remains available during Practice Running. The
current full rail becomes the shared Setup/Settings surface. The
`PracticePauseSurface::Rail` distinction is removed because Editing no longer
borrows `PauseState::Paused`; the pause overlay remains the only paused
surface.

UI systems send typed preset requests and consume typed results. They do not
perform filesystem writes. Preview and runtime systems consume the same loaded
chart resources but use disjoint judgment and statistics gates.

## Data flow

```text
PracticeIntent
      |
      v
prefill PracticeDraft <--- Whole / Last / Recommended / Saved
      |
      |- PreviewController reads draft
      |- preset requests persist explicit changes
      `- Start / Continue
                 |
                 v
          validate and commit
                 |
                 v
          PracticeSession
                 |
                 `- seek pre-roll -> count-in -> eligible attempt
```

Practice Settings clones the committed session into a draft. Changes do not
affect the session until Continue. Returning to Pause restores the frozen
runtime configuration and position. Starting a new attempt clears the
ineligible interrupted pass without adding it to Progress.

## Errors and recovery

- A missing preset file creates an empty in-memory registry.
- A corrupt or unsupported newer registry is preserved, reported, and treated
  as empty for the session. The game does not overwrite it.
- A failed save retains the draft and offers `Retry Save`.
- Missing or invalid recommended bounds fall back to Whole Song.
- Preview audio failure leaves visual seeking available and reports `Preview
  audio unavailable`.
- A chart unload or application transition stops preview and clears transient
  drafts and snapshots.
- Failure to restore a frozen audio position returns to loop pre-roll and
  reports the recovery instead of resuming at an unknown position.

## Motion and accessibility

Setup and Settings use a short OutQuint panel transition and a restrained tab
crossfade. Preview play/pause and the playhead communicate transport state.
Reduce Motion removes the panel motion and shortens the crossfade. Application
screen transitions keep the accepted 300 ms OutQuint behavior.

The interface supports Standard, Large, and Extra Large text scales. Critical
labels and controls remain readable at electronic-kit distance. The preview
warning, current mode, selected row, save status, and disabled state never rely
on color alone. Focus order follows the visual order and cannot enter hidden
Ramp rows or the hidden half of the narrow-layout tab pair.

## Testing

Pure tests:

- flow transitions for Setup, Running, Editing, Preview, cancel, and commit;
- draft clamping, A/B normalization, trainer exclusivity, and fallback;
- preset round-trip, version rejection, corruption preservation, naming,
  Last Used behavior, and failed saves; and
- interrupted-attempt eligibility and loop-boundary recovery.

System and integration tests:

- every practice intent opens Setup before Running;
- preview cannot produce judgments, misses, combo, score, attempts, lane
  diagnosis, wait halts, or ramp evaluation;
- preview loop, tempo, seek, BGA, and audio positions stay synchronized;
- Esc from Editing restores the frozen position and opens Practice Paused;
- Start, Continue, and Restart Loop seek to pre-roll, run count-in, and create
  fresh eligible attempts;
- normal and practice pause menus dispatch the documented actions; and
- keyboard, mouse, and pad navigation reach both tabs and timeline controls.

Layout checks cover 1280x720, 1920x1080, narrow aspect ratios, and all text
scales. Manual checks cover audible synchronization, real MIDI navigation,
distance readability, preview labeling, and A/B dragging.

Package verification must include the changed Pure persistence package,
`game-shell`, and `gameplay-drums`, followed by workspace check and Clippy with
warnings denied before merge.

## Acceptance criteria

1. Every Practice action opens Setup with preview stopped, including saved and
   recommended requests.
2. Preview shows and plays the real chart without producing any judged or
   statistical gameplay output.
3. Pause resumes at the exact frozen position. Practice Settings continues
   from loop pre-roll as a fresh attempt.
4. Setup and Settings expose Loop, Transport, Trainer, and Progress without
   overlap at supported sizes and text scales.
5. Saved presets belong to one canonical chart, require explicit writes, and
   restore all loop, transport, and trainer fields.
6. Off, Wait, and Ramp are mutually exclusive trainer modes. Preview never
   executes trainer behavior.
7. Only complete eligible loop attempts enter Progress or ramp evaluation.
8. Keyboard, mouse, and configured kit controls can pause, configure, preview,
   start, restart, and exit practice through visible actions.
9. Normal-play Pause and Practice Pause retain their separate, concise action
   sets.
10. Practice and preview remain excluded from qualifying score persistence.
