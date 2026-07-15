# Player Guide

Purpose: maintained instructions for installing, configuring, and playing the
drums game.

Audience: keyboard and electronic-drum players.

Status: Maintained and verified against current source and automated tests. The
latest bounded desktop launch attempts timed out during compilation, so desktop
startup, window-size layout, mouse and saved-loop interaction, audible
synchronization, and physical MIDI behavior remain manual QA items.

Neighboring guides: [roadmap](roadmap.md),
[compatibility](compatibility.md), and
[data and persistence](data-and-persistence.md).

## Install and find songs

Install and launch from the repository root as described in the
[README](../README.md). The game starts borderless fullscreen; set
`DTXMANIARS_WINDOWED=1` in the launch environment for a 1280x720 window.

The scanner recursively searches `DTX_SONG_DIR` when set. Otherwise it uses
`$XDG_CONFIG_HOME/dtxmaniars/`, falling back to
`$HOME/.config/dtxmaniars/`. A normal library has one folder per song with one
or more chart files and their relative media.

Song Select supports:

- type to search title, artist, genre, comment, chart path, or folder path;
- `Tab` to cycle sort mode and `F5` to rescan;
- `F6` to import ZIP/7z archives, or drag archives into the window;
- `F7` to toggle the highlighted chart's favorite state;
- `Ctrl+1` favorites, `Ctrl+2` unplayed, `Ctrl+3` recent, and `Ctrl+4`
  near-level filters; these filters compose;
- `Ctrl+0` to reset filters and `Ctrl+R` to choose randomly from the current
  searched and filtered set;
- arrows to move song/difficulty, `Enter` to open Song Ready in Normal mode,
  `Shift+Enter` to open it in Practice mode, and `Esc` to clear search before
  returning.

With a connected kit, the on-screen legend is authoritative. At the song
wheel, HH/CY moves, BD enters difficulty, and SD returns to Title. At the
difficulty level, HH/CY changes difficulty, BD opens Song Ready with Normal
selected, FT opens it with Practice selected, and SD returns to the wheel.
Confirm the primary action in Song Ready to start loading.

Song Ready is the checkpoint before loading. It shows the selected song and
difficulty alongside mode, fail-mode, lane-speed, and audio controls. Keyboard
and mouse players can confirm Start Song or Open Practice Setup. From a kit,
use HH/CY to move between cards, BD to enter or confirm, and SD to go back. On
the Song card, one BD opens its detail and another confirms the primary action.

## Customize controls and presentation

Press `F1` on Title to open Customize using an available song, or `F1` on Song
Select to customize the highlighted song. The eight tabs are Gameplay, Audio,
Drums, System, Accessibility, Controls, Lanes, and Widgets. `Ctrl+S` saves and
`Esc` closes; when drafts are dirty, the close flow offers save, discard, or
cancel. The footer shows current keyboard navigation.

The Controls tab has independent Keyboard and MIDI profile registries. Select,
copy, rename, and save a user profile instead of modifying a built-in. Each of
the 12 drum channels can have multiple sources, and one source may intentionally
serve multiple drum channels. A source assigned to a drum lane cannot also be
a system action.

Default keyboard lane keys are:

| Lane | Keys |
|---|---|
| Hi-hat closed | `X` |
| Snare | `C`, `D` |
| Bass drum | `Space`, `Convert` |
| High tom | `V`, `F` |
| Low tom | `B`, `G` |
| Floor tom | `N`, `H` |
| Cymbal | `M`, `J` |
| Hi-hat open | `S` |
| Ride | `,`, `K` |
| Left cymbal | `Z`, `A` |
| Left pedal | `NonConvert` |
| Left bass drum | `Left Alt` |

`Esc` toggles Pause during normal play and while Practice is Running, provided
Customize is closed. In normal-play Quick Settings, `Esc` returns to the Pause
menu. Practice Setup/Settings and Customize use `Esc` for their own back,
cancel, and close flows. Pause and Restart are also bindable system actions for
a distant kit; they are unbound by default because pad notes vary by device.
Pad retriggers are debounced.

The Lanes tab controls display-lane profiles and the Widgets tab controls HUD
placement. These change presentation, not judgment-channel identity.

## Connect a MIDI kit

The shipping desktop binary includes MIDI input by default. Open Controls,
switch to MIDI, and:

1. Use Rescan if the device list is stale.
2. Select the intended input port. The saved value is a port-name substring;
   no selection uses the first available port.
3. Adjust the velocity threshold. NoteOn values at or below the threshold are
   ignored by gameplay but remain visible and learnable during capture.
4. Click `+` on the target pad row, then strike the intended pad. Review the
   captured note and choose Add shared or Move here if another lane owns it;
   confirm with the button, `Enter`, or the same pad hit.
5. To bind Pause or Restart, click `+` on that row in the visible System card
   and strike an unused pad. A free note binds at once; a lane-owned note or a
   note assigned to the other system action is refused in the capture modal.
6. Save the user profile and verify the live status/hit display.

MIDI learn accepts a new positive-velocity NoteOn after capture starts. A stale
hit cannot bind, while a below-threshold hit remains learnable for setup.

Port availability and note output are hardware/driver behavior and must be
checked manually. If a port disconnects, reconnect it and use Rescan; keyboard
bindings remain available.

## Calibrate input

Input Offset compensates for the delay between a physical hit and the game's
judgment clock. It is separate from BGM Offset, which aligns a chart's audio
to its notes.

In Customize, open Gameplay and choose Calibrate. The guide plays a
chart-independent 120 BPM sequence, enables the metronome/timing lines, and
temporarily disables autoplay. Tap any mapped keyboard key or MIDI pad on the
beats. The report rejects outliers and includes sample count, spread, scheduler
delay, and confidence:

- high confidence: `Enter` applies the clamped suggested input offset and
  `Esc` cancels;
- low confidence: the current offset is retained; close and retry under more
  stable conditions.

A MIDI disconnect is reported without invalidating keyboard taps. After saving,
verify the feel in a familiar chart. Calibration cannot measure a display or
audio stack on behalf of the player, so that final feel check is manual.

## Normal play and pause

Normal play judges mapped inputs against the audio-owned chart clock. `Esc` or
a bound Pause action freezes chart time and chart audio. The normal pause menu
offers Resume, Restart Song, Practice This Section, Quick Settings, and Return
to Song Select. A bound Restart action reloads the current chart from either
running or paused play.

Gameplay Play Speed ranges from `0.50x` to `2.00x`. One effective rate keeps
audio, chart time, notes, visuals, seeking, and completion synchronized. The
rate changes pitch because pitch-preserving stretch is not implemented. A
normal run qualifies for an ordinary saved record only at `1.00x`.
Standard fail mode ends the stage when life reaches zero. No Fail is an
assisted mode: the stage continues, Results labels it, and no normal record is
saved.

## Practice transport

From Song Select, press `Shift+Enter` or hit FT at the kit difficulty level to
open Song Ready with Practice selected. Confirm Open Practice Setup to load the
chart. The Practice action on Results can also enter Practice. Every route
opens Practice Setup before any attempt begins. Setup uses the loaded
playfield, notes, BGA, audio, and timeline as a preview, but preview starts
stopped and drum input is not judged. Preview does not change score, combo,
gauge, attempt history, lane diagnosis, Wait, or Ramp.

Setup contains loop, tempo, snap, pre-roll, count-in, trainer, and saved-loop
controls. Trainer is exactly one of Off, Wait, or Ramp. The Progress tab shows
only completed loop attempts; an interrupted or partially edited attempt is not
added. Choose Start Practice to commit the draft and begin from the configured
pre-roll/count-in. Practice never saves a normal record and loops rather than
ending at chart end.

While Practice is running:

| Key | Action |
|---|---|
| `[` | Set loop start A at the current bar |
| `]` | Set loop end B at the current bar |
| `Backspace` | Clear the explicit loop |
| `-` / `=` | Lower / raise tempo |
| `R` | Restart the loop or current attempt |
| `T` | Toggle tempo ramp |
| `Tab` | Open Practice Settings |
| `Esc` | Open the Practice pause menu |

Practice Settings reuses the Setup and Progress surface. It starts with preview
stopped and marks the interrupted attempt ineligible. Continue Practice commits
the draft and starts a fresh attempt from the configured pre-roll/count-in.
Changing the loop or manually changing tempo disarms an active Ramp.

The Practice pause menu offers Resume, Restart Loop, Practice Settings, and Exit
to Song Select. Resume continues from the exact frozen chart/audio position;
Restart Loop and Continue Practice seek through pre-roll instead.

Saved loops live in `CONFIG_DIR/practice-presets.toml`. They are isolated by
canonical chart hash and selected difficulty. Draft edits are session-only
until Save as New or Update Saved Loop is selected; Delete is confirmed before
the stored loop is removed. Last Used updates automatically only when Start
Practice or Continue Practice commits the draft. See
[Data and Persistence](data-and-persistence.md) for backup and recovery.

## Results and recommended practice

Results first reveals score, judgments, combo, rank, and save qualification.
Any first navigation input finishes the reveal and is consumed. Then use
left/right to select Continue, Retry, or Practice; `Enter`/`Space` activates,
`Esc` continues immediately, `R` retries, and `Tab` toggles details. The kit
legend uses HH/CY to move, BD to activate, SD to continue, and FT to jump to
Practice.

For a qualifying normal run, Results compares the score with existing native
history and derives the weakest lane and section from recorded timing events.
When a weak section exists, “Practice weakest section” reopens the chart with
that loop, one-bar pre-roll, and `1.00x` tempo already selected. Practice,
modified-speed, and No Fail results still show useful run data but do not claim
a comparable personal-best delta or save a normal record.

## Accessibility

The Accessibility tab provides independent Text Scale (Standard, Large, Extra
Large), Reduce Motion, Reduce Flashes, and Background Motion controls. Text
scaling does not change gameplay geometry. Reduced flashes uses lower-contrast
outlined feedback; motion settings shorten/suppress nonessential UI motion and
can hold authored background pans at their end state or disable movies.
Selection and critical run state also use text, geometry, or icons rather than
color alone. Save the configuration after changing these settings.
