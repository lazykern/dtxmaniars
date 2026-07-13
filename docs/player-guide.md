# Player Guide

Purpose: maintained instructions for installing, configuring, and playing the
drums game.

Audience: keyboard and electronic-drum players.

Status: Maintained and verified against the current desktop implementation.
Hardware enumeration, audio output, and feel still require a manual check on
the player's machine.

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
- arrows to move song/difficulty, `Enter` for normal play,
  `Shift+Enter` for Practice, and `Esc` to clear search before returning.

With a connected kit, the on-screen legend is authoritative. At the song
wheel, HH/CY moves, BD enters difficulty, and SD returns to Title. At the
difficulty level, HH/CY changes difficulty, BD starts normal play, FT starts
Practice, and SD returns to the wheel.

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

`Esc` always toggles pause during a performance. Pause and Restart are also
bindable system actions for a distant kit; they are deliberately unbound by
default because pad notes vary by device. Pad retriggers are debounced.

The Lanes tab controls display-lane profiles and the Widgets tab controls HUD
placement. These change presentation, not judgment-channel identity.

## Connect a MIDI kit

The shipping desktop binary includes MIDI input by default. Open Controls,
switch to MIDI, and:

1. Use Rescan if the device list is stale.
2. Select the intended input port. The saved value is a port-name substring;
   no selection uses the first available port.
3. Adjust the velocity threshold. NoteOn values at or below the threshold are
   ignored.
4. Select a lane or Pause/Restart row, start capture, then strike the intended
   pad. MIDI learn accepts a newly arriving NoteOn rather than a stale hit.
5. Save the user profile and verify the live status/hit display.

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
offers Resume, Retry, and Quit to Song Select. A bound Restart action reloads
the current chart from either running or paused play.

Gameplay Play Speed ranges from `0.50x` to `2.00x` and changes both notes and
audio; pitch changes because pitch-preserving stretch is not implemented.
Standard fail mode ends the stage when life reaches zero. No Fail is an
assisted mode: the stage continues, Results labels it, and no normal record is
saved.

## Practice transport

Start Practice with `Shift+Enter`, FT at the kit difficulty level, or the
Practice action on Results. Practice never saves a normal record and loops
rather than ending at chart end.

While Practice is running:

| Key | Action |
|---|---|
| `[` | Set loop start A at the current bar |
| `]` | Set loop end B at the current bar |
| `Backspace` | Clear the explicit loop |
| `-` / `=` | Lower / raise tempo |
| `R` | Restart the loop or current attempt |
| `T` | Toggle tempo ramp |
| `Tab` | Pause and open the full Practice rail |
| `Esc` | Open the compact Practice pause menu |

The full rail exposes loop, tempo, pre-roll, wait, ramp, attempt, and diagnosis
controls. Changing the loop or manually changing tempo disarms an active ramp.
The compact Practice pause menu offers Resume, Restart loop, and Exit Practice.

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
