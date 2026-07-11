# DTXManiaRS Player Manual: Current Behavior

Date: 2026-07-11
Purpose: factual player-facing baseline for later behavioral research and UX evaluation

## 1. Scope and evidence

This manual documents what a player can currently see and do in DTXManiaRS. It
does not evaluate the design, propose improvements, or describe planned features
as if they already exist.

Behavior labels used in this manual:

- **Available**: reachable in the current game.
- **Limited**: reachable, with an important player-visible restriction.
- **Experimental**: present in the game but incomplete or not ready as a normal
  player workflow.
- **Unavailable**: not supported in the current player experience.
- **Unverified visually**: confirmed from runtime wiring and tests, but not
  inspected in a live graphical session during this documentation pass.

The current behavior was checked against screen wiring, input handling,
settings, persistence, tests, and recent changes. The desktop game was not run
in a graphical environment during this pass, so visual appearance and physical
MIDI hardware behavior remain **unverified visually**.

## 2. Supported player setups

### Keyboard and mouse

**Available.** A player can launch the game, choose songs, play drums, enter
practice, pause, view results, and customize settings with the keyboard.

The mouse is used mainly in settings, layout editing, binding controls, and the
practice timeline. The title screen, song list, pause menu, and results screen
are primarily keyboard-operated; normal mouse selection is **unavailable** on
those screens.

### Keyboard, mouse, and a nearby MIDI drum kit

**Available.** The kit can play drum notes and perform the main song-menu
navigation actions. The keyboard and mouse remain available for actions the kit
cannot perform, including opening settings, pausing from active play, detailed
practice control, searching, importing, and layout editing.

### MIDI drum kit with the computer out of reach

**Limited.** The kit can start from the title screen, move through songs and
difficulties, start normal play or practice, operate an already-open normal
pause menu, and leave results. A kit cannot currently open the pause menu while
a song is running. It also cannot open Customize, operate advanced practice, or
edit Controls, Lanes, and Widgets without keyboard or mouse access. If a nearby
person opens Customize, the kit can operate the Gameplay, Audio, Drums, and
System settings tabs.

## 3. Starting the game

The desktop game starts in borderless fullscreen by default. A windowed launch
mode exists, but it is not exposed as an in-game player control.

The game loads the saved configuration, HUD/lane layout, input bindings,
profiles, and score history at startup. Missing or unreadable basic settings and
layout files fall back to defaults. A damaged input or lane profile registry can
leave only the built-in profiles usable for that session.

The player first sees a short `DTXManiaRS / Loading...` startup splash. It
advances automatically to the title screen after about half a second.

## 4. Title screen

### Available actions

| Input | Action |
|---|---|
| `Enter` | Continue to song selection |
| Bass drum pad | Continue to song selection |
| `F1` | Open the Settings customize surface |
| `F2` | Open the HUD/lane Layout customize surface |
| `Esc` | Begin quitting the game |

Opening Customize from the title screen uses a playable chart as its live
background when one is available. The game uses the remembered or another
available chart and runs it in an editor/autoplay context.

**Limited:** Customize cannot open until the library contains at least one
playable chart. `F1` and `F2` do not open an empty editor when no chart is
available.

**Unavailable:** clicking the title screen with the mouse to continue.

### Screen changes and quitting

Normal screen changes use a fullscreen black fade: the current screen fades
out, the destination replaces it, and the destination fades in. This transition
also applies when entering and leaving the main player workflows.

After `Esc` is pressed on the title screen, the game changes to a final
`Thanks for playing` screen. That screen remains visible for about one second,
then the application exits automatically.

## 5. Song library and song selection

The song library recursively finds `.dtx` charts in the configured song
library. Charts that cannot be parsed are skipped. Multiple chart files in the
same song folder are presented as difficulties of one song. A `set.def` file,
when present, can define difficulty order; otherwise the charts are ordered by
their difficulty values.

When returning to the song screen, the game remembers the last selected chart
and difficulty when possible.

### Browsing with keyboard

| Input | Action |
|---|---|
| `Up` / `Down` | Select previous or next song |
| `Left` / `Right` | Select another difficulty |
| `Enter` | Start normal play |
| `Shift+Enter` | Start practice mode |
| `Esc` | Return to the title screen |
| `Tab` | Cycle the sort mode |
| `F1` | Open Customize using the selected chart |
| `F5` | Rescan the song library |
| `F6` | Open the song-archive file picker |
| Letter/number typing | Filter songs by title or artist |
| `Backspace` | Remove the last search character |

Search is live and is cleared when the song-selection screen is entered again.
The search text is limited to 64 characters.

### Sorting

`Tab` cycles through:

- Default library order
- Title
- Artist

### Browsing with a MIDI kit

MIDI song selection has two levels. When a MIDI device is connected and its
notes are mapped, the song-wheel level uses:

| Pad | Action |
|---|---|
| Closed/open hi-hat | Previous song |
| Cymbal/ride | Next song |
| Bass drum | Enter the selected song's difficulty level |
| Snare | Return to the title screen |

The difficulty level then uses:

| Pad | Action |
|---|---|
| Closed/open hi-hat | Previous difficulty |
| Cymbal/ride | Next difficulty |
| Bass drum | Start normal play |
| Floor tom | Start practice mode |
| Snare | Return to the song wheel |

Pad menu actions have a short debounce, and newly entered screens ignore pad
navigation briefly to prevent the hit that entered a screen from immediately
triggering another action.

**Limited:** the kit does not provide song search, sort cycling, library rescan,
archive import, or settings access from this screen.

### Selected-song information

The song screen can show:

- selected title and artist;
- album art;
- preview audio;
- BPM;
- skill value for the song;
- a per-lane note-density graph and total note count;
- the available difficulty slots and their displayed levels;
- saved achievement and rank for each difficulty; and
- recent play history.

Preview audio uses the chart's declared preview when present and otherwise
falls back to available song audio. Album art uses the chart's preimage when
available and otherwise shows a placeholder.

Recent history shows up to eight results for the selected chart. Results are
ordered by score, with the newer result first when scores are tied. Each row can
show rank, score, perfect rate, and play date.

### Empty library

When no songs are available, the screen offers the player-facing choices to
rescan with `F5`, import with `F6`, or drop a supported archive into the game
window.

## 6. Importing song archives

**Available.** A player can import a chart archive by:

1. Pressing `F6` on song selection and choosing one or more files.
2. Dragging a supported archive onto the game window.

Supported archive types:

- ZIP
- 7z

**Unavailable:** RAR extraction. RAR is deliberately shown in the picker, but
selecting it produces an unsupported-format notification asking the player to
extract it manually.

Files dropped onto the window are sent to the importer even when they are not
supported archives. The resulting success or error is shown as a notification.

The importer:

- rejects unsafe archive paths;
- rejects an archive containing no DTX chart;
- avoids importing an already-imported archive as another duplicate;
- handles archives with a single unnecessary wrapper folder;
- rescans the library after a successful import;
- moves selection to the imported song folder when possible; and
- reports the result with an on-screen notification.

**Limited:** an active search can filter the imported folder out of the visible
list. The import still succeeds, but the automatic cursor jump eventually gives
up without another message; clearing the search reveals the imported song.

## 7. Song loading

After normal play or practice is requested, the loading screen parses the chart
and prepares required audio. It displays loading status and progress.

If the chart defines a loading sound and BGM audio is enabled, that sound is
played when its file is available.

| Input | Action |
|---|---|
| `Esc` | Cancel loading and return to song selection |

If loading fails, the game returns to song selection rather than starting an
incomplete performance.

## 8. Normal drum performance

### Drum lanes

The drum game supports these logical channels:

- Closed hi-hat
- Snare
- Bass drum
- High tom
- Low tom
- Floor tom
- Cymbal
- Open hi-hat
- Ride
- Left cymbal
- Left pedal
- Left bass drum

The visible layout can merge more than one logical channel into one displayed
lane. The current lane arrangement determines the lane order, width, and merged
channels.

### Playing notes

Hit the configured keyboard key or MIDI note when a chip reaches the judgment
line. Keyboard input can be shared across multiple channels. A MIDI note is
assigned exclusively to one channel in the current profile model.

Hits outside the configured MIDI velocity threshold are ignored. MIDI devices
are detected automatically and the game attempts to reconnect after a
disconnect.

### Judgment windows

Default absolute timing windows are:

| Judgment | Window |
|---|---:|
| Perfect | up to 34 ms |
| Great | up to 67 ms |
| Good | up to 84 ms |
| Poor | up to 117 ms |
| Miss | outside 117 ms or an unplayed chip |

The judgment popup can show the signed timing error for a hit, indicating how
early or late it was.

### Score, combo, gauge, and failure

Normal play tracks score, current and maximum combo, judgment totals, timing
feedback, skill/achievement information, and the stage gauge.

Score follows the XG-style scoring rules, including a combo contribution that
ramps up to 50 combo. Full combo and all-perfect play can receive end bonuses.

The gauge starts partially filled. Misses reduce it according to the selected
damage level. If stage failure is enabled and the gauge falls below its failure
point, the stage fails. Damage choices are None, Small, Normal, and High.

### Audio behavior

Charts can play background music, automatic sound-effect chips, and drum
keysounds. Drum sound grouping, priority, and polyphony settings affect how
simultaneous or related pad sounds are selected and played. Empty pad hits can
use chart-derived fallback sounds.

The game uses the audio position as the main performance clock and corrects
timing drift while playing.

### Performance hotkeys

| Input | Action |
|---|---|
| `Esc` | Open the pause menu |
| `Up` / `Down` | Increase/decrease scroll speed |
| `Left` / `Right` | Decrease/increase input offset |
| `Ctrl+Left` / `Ctrl+Right` | Fine input-offset adjustment |
| `Shift+Up` / `Shift+Down` | Adjust the selected song's BGM offset |
| `Ctrl+Shift+Up` / `Ctrl+Shift+Down` | Fine BGM-offset adjustment |
| `F11` | Toggle the performance-information overlay |

Changes made through performance hotkeys are shown on screen. Scroll speed,
input offset, and the performance-information toggle are saved shortly after
the last change. The selected song's BGM adjustment is written immediately
when its hotkey changes the value.

**Unavailable:** opening pause from a drum pad while the performance is
running.

## 9. Performance HUD

The normal HUD can contain:

- Playfield and falling chips
- Frame chrome
- Score panel and detailed judgment totals
- Combo
- Judgment popup with early/late error
- Phrase meter
- Song progress
- Now Playing title, artist, and maker
- Live performance graph
- Scroll-speed readout
- Keyboard input visualization

The phrase meter, progress, score panel, and live graph are hidden in practice
by default. Widget visibility can be changed separately for normal play and
practice in the Widgets customize tab.

Lane lines can be shown as All On, Half, Line Off, or All Off.

## 10. Pausing normal play

Press `Esc` during normal performance to pause. Pausing freezes the gameplay
clock and pauses the chart's background and drum audio.

The normal pause menu contains:

1. Resume
2. Retry
3. Quit to Song Select

### Pause controls

| Input | Action |
|---|---|
| `Up` / `Down` | Move selection |
| `Enter` or `Space` | Activate selection |
| `Esc` | Resume directly |
| Hi-hat | Move up |
| Cymbal/ride | Move down |
| Bass drum | Activate selection |
| Snare | Resume/back |

**Limited:** pad controls work after the pause menu is already open; a pad
cannot open it from active play.

## 11. Stage clear, failure, and results

At the end of normal play, the game displays a stage-clear or stage-failed
banner. The banner advances automatically after about 1.6 seconds.

| Input | Action |
|---|---|
| `Enter` or `Space` | Skip the clear/fail banner |

The results screen shows:

- Song title, artist, and level
- Final score
- Maximum combo
- Rank
- Perfect, Great, Good, Poor, and Miss counts and percentages
- Total judged notes

Ranks use SS, S, A, B, C, D, and E. Rank includes the Perfect rate, Great rate,
and maximum-combo rate; it is not based on score alone.

### Leaving results

| Input | Action |
|---|---|
| `Enter` or `Esc` | Return to song selection |
| Bass drum or snare pad | Return to song selection |

### Saved results

Normal results are saved when the player leaves the Results screen, not when the
screen first appears. The game then keeps individual play history and the best
score for a chart. A compatible per-chart score record is also written next to
the chart when that location is writable.

Failed normal plays are recorded. Practice attempts are never added to normal
score history.

**Limited:** closing or killing the application while still on the Results
screen can lose that play because the save has not run yet. A failure while
saving either score record has no player-visible error or retry prompt.

**Unavailable on the current results screen:** retry, direct practice handoff,
personal-best delta, timing histogram, weakest-section analysis, and per-lane
miss analysis.

## 12. Practice mode

Start practice by selecting a song and pressing `Shift+Enter`, or by striking
the floor tom on song selection.

Practice uses the normal drum playfield but changes the session rules:

- the song or selected A/B section loops;
- the player can seek and restart;
- tempo can be changed;
- attempts and timing tendencies are tracked;
- the stage cannot fail;
- the practice session does not finish as a normal scored play; and
- practice results are not saved to normal score history.

### Quick practice controls while playing

| Input | Action |
|---|---|
| `[` | Set loop point A, snapped to a bar |
| `]` | Set loop point B, snapped to a bar |
| `Backspace` | Clear the A/B loop |
| `-` | Reduce tempo by 0.05x |
| `=` | Increase tempo by 0.05x |
| `R` | Restart the current section |
| `T` | Arm or disarm the accuracy ramp |
| `Tab` | Open the full practice HUD |
| `Esc` | Open the full practice HUD through pause |

Tempo ranges from 0.50x to 1.50x and defaults to 1.00x.

### Loop behavior

Without an A/B region, the whole song is the practice span. Set A and B to
repeat a smaller section. If B is placed before A, the points are reordered.
Mouse-dragging a region on the full timeline snaps it to bars and enforces a
minimum one-bar region.

Changing or clearing a loop disarms an active tempo ramp because the ramp is
tied to the practiced section.

### Full practice HUD

The full practice HUD pauses playback and provides a bottom density timeline,
mouse seek/region selection, transport buttons, attempt history, lane
diagnosis, and an 18-row control rail:

1. Resume
2. Scrub
3. Restart section
4. Tempo
5. Snap
6. Pre-roll
7. Count-in
8. Set A
9. Set B
10. Clear loop
11. Ramp on/off
12. Ramp start
13. Ramp target
14. Ramp step
15. Ramp pass threshold
16. Ramp streak
17. Wait mode
18. Exit practice

Use `Up`/`Down` to select a row, `Left`/`Right` to change a row's value, and
`Enter` or `Space` to activate it. Exit Practice requires a second confirmation
press.

The mouse can:

- click the timeline to seek;
- drag the timeline to create an A/B loop; and
- press previous-bar, resume, and next-bar transport buttons.

### Snap and pre-roll

Seek snap cycles through:

- Bar
- Beat
- Half-beat

Pre-roll cycles through:

- One bar
- Two seconds
- Off

Count-in is on by default and produces metronome clicks during pre-roll.

### Accuracy ramp

The ramp begins at a slower tempo and raises the tempo after successful passes.
Defaults are:

| Setting | Default | Available range |
|---|---:|---:|
| Start tempo | 0.70x | 0.50x to below target |
| Target tempo | 1.00x | above start to 1.50x |
| Step | 0.05x | 0.05x to 0.25x |
| Pass threshold | 90% | 50% to 100% |
| Required successful passes | 1 | 1 to 3 |

Two consecutive failed passes step the ramp down, no lower than its start
tempo. Completing the target promotes the player's selected tempo to the target
and ends the ramp. Manually changing tempo disarms the ramp.

### Wait mode

Wait mode stops at unhit notes until the player clears them. Notes cleared
while waiting are tracked separately from timing judgments, and the HUD reports
flow percentage. Wait mode and the accuracy ramp are mutually exclusive.

### Attempt feedback

Practice retains up to 20 attempt records. The full HUD shows up to eight recent
attempts for the current section, including accuracy, mean timing error, and
tempo. It also aggregates lane tendencies such as rushing, dragging, or on-time
play. Small reports and notifications appear when loops wrap or settings change.

### Practice limitations

- **Limited:** changing playback rate changes pitch; pitch-preserving tempo is
  unavailable.
- **Limited:** advanced practice navigation is keyboard/mouse-oriented. A kit
  can start practice but cannot fully operate the practice rail.
- **Unavailable:** saving a practice result as a normal score.
- **Unavailable:** ending practice automatically at song completion; the song
  is treated as a repeating practice span.

## 13. Customize surfaces

Customize is a live editor shown over an autoplay performance. Open it with:

- `F1` on the title or song-selection screen for settings and controls.
- `F2` on the title screen for layout-oriented editing.

Tabs are:

1. Gameplay
2. Audio
3. Drums
4. System
5. Controls
6. Lanes
7. Widgets

### General keyboard controls

| Input | Action |
|---|---|
| `PageDown` / `PageUp` | Next/previous tab |
| `Up` / `Down` | Move between rows |
| `Left` / `Right` | Decrease/increase a setting |
| `Enter` | Activate/toggle selected row |
| Hold `Shift` | Coarser setting adjustment or larger widget nudge |
| Hold `Tab` | Temporarily peek at the playfield without editor chrome |
| `Ctrl+S` | Save layout changes |
| `Ctrl+Z` | Undo layout edit |
| `Ctrl+Y` or `Ctrl+Shift+Z` | Redo layout edit |
| `Esc` | Context-dependent close, cancel, or deselect action |

Settings changes use a draft and are saved when Customize closes. Layout edits
can be explicitly saved. Dirty keyboard, MIDI, or lane-profile changes open a
`Cancel | Discard | Save` dialog when closing:

- `Enter` chooses Save.
- `Esc` cancels closing and returns to Customize.
- Discard requires clicking its visible button with the mouse.
- Saving edits made to an immutable built-in creates a new automatically named
  profile copy.

If saving one or more dirty profile types fails, Customize remains open and the
dialog keeps only the failed types pending so the player can retry. The failure
is not otherwise explained through a dedicated recovery screen.

Customize has an `Esc` hierarchy:

- During keyboard binding capture, `Esc` cancels capture without closing.
- During calibration, `Esc` cancels calibration without closing.
- On Widgets with a widget selected, the first `Esc` deselects it.
- A later `Esc`, or `Esc` elsewhere, requests closing.
- While the dirty-change dialog is open, `Esc` cancels the close request and
  returns to Customize.

Settings and widget-layout saves can fail without an on-screen error or retry
prompt. The current session can still show the edited values even when they
were not written successfully.

### General mouse controls

| Mouse action | Result |
|---|---|
| Click a top tab | Open that Customize tab |
| Click a setting row's left/right control | Decrease/increase the setting |
| Drag a settings slider | Change and snap the value to its allowed step |
| Click `RESET TAB` on a settings tab | Restore every row in that tab to defaults |
| Click `Calibrate` on Gameplay | Start the input-offset tap test |
| Click a dirty-dialog action | Cancel, discard, or save the pending changes |

### MIDI pad controls in Customize

Pads can operate the Gameplay, Audio, Drums, and System settings tabs after
Customize has been opened with a keyboard:

| Navigation level | Hi-hat | Cymbal/ride | Bass drum | Snare |
|---|---|---|---|---|
| Tab bar | Previous tab | Next tab | Enter tab rows | Close Customize |
| Setting rows | Previous row | Next row | Begin adjusting row | Return to tab bar |
| Adjusting a row | Decrease value | Increase value | Keep value | Revert value |

Pad navigation is suspended while binding capture or input calibration owns raw
drum hits.

**Limited:** pads cannot enter the Controls, Lanes, or Widgets content. Those
tabs require keyboard and/or mouse interaction.

## 14. Gameplay settings

| Setting | Range/options | Default |
|---|---|---|
| Scroll Speed | 0.5x to 9.0x, 0.5 steps | 1.0x |
| Input Offset | -300 ms to +300 ms | 0 ms |
| BGM Offset | -300 ms to +300 ms | 0 ms |
| Play Speed | 0.5x to 2.0x | 1.0x |
| Damage Level | None, Small, Normal, High | Small |
| Lane Display | All On, Half, Line Off, All Off | All On |

### Input-offset calibration

The Gameplay tab includes an input-offset tap test:

1. Activate Calibrate.
2. Tap any configured keyboard or MIDI drum input to the beat.
3. Continue until 12 samples have been collected.
4. Review the suggested input offset.
5. Press `Enter` to apply it, or `Esc` to cancel.

During collection, the game forces the metronome and timing lines on and turns
editor autoplay off. Those temporary states are restored afterward.

**Limited:** this is a median tap test, not a guided audio/video/device
diagnostic. It does not show sample spread or confidence and does not measure
audio output latency with hardware loopback.

### Play-speed warning

**Limited:** normal Play Speed currently compresses chart time without matching
audio time-scaling. Values other than 1.0x can desynchronize the chart from the
song. This control should not be treated as equivalent to practice tempo.

### Gameplay options not exposed

Stage failure is enabled by default, but there is no current settings row for
turning the stage-failure rule off. Tight judgment mode, reverse scroll, dark
mode, and fill-in behavior also exist in saved configuration but are
**unavailable** as controls in the current Customize surface.

## 15. Audio settings

| Setting | Range/options | Default |
|---|---|---|
| BGM Sound | On/Off | On |
| Drum Hit Sound | On/Off | On |
| Master Volume | 0% to 100%, 5% steps | 80% |
| BGM Volume | 0% to 100%, 5% steps | 70% |
| Drum Volume | 0% to 100%, 5% steps | 80% |

## 16. Drum sound settings

The Drums tab controls how related channels select and share sounds:

| Setting | Options | Default |
|---|---|---|
| CY/RD Group | Separate, Common | Separate |
| HH Group | All Separate, HH vs LC, HH vs HO, All Common | All Separate |
| FT Group | Separate, Common | Separate |
| BD Group | All Separate, BD+LBD, Pedals Only, All BD | All Separate |
| Cymbal Free | On/Off | Off |
| HH Priority | Chip > Pad, Pad > Chip | Chip > Pad |
| FT Priority | Chip > Pad, Pad > Chip | Chip > Pad |
| CY Priority | Chip > Pad, Pad > Chip | Chip > Pad |
| LP Priority | Chip > Pad, Pad > Chip | Chip > Pad |
| Polyphonic Sounds | 1 to 8 | 4 |

Sound priority choices determine whether a chart chip's sound or a pad's
current fallback sound wins for the affected group.

## 17. System settings

| Setting | Options | Default |
|---|---|---|
| VSync | On/Off | On |
| Performance Info | On/Off | Off |
| Metronome | On/Off | Off |

Performance Info can also be toggled with `F11` during play.

## 18. Controls and MIDI setup

The Controls tab shows MIDI device controls and channel bindings.

### MIDI device actions

- View the selected/connected MIDI port.
- Cycle the selected port when devices are available.
- Rescan MIDI devices.
- Set the MIDI velocity threshold from 0 to 127. The default is 0.
- Observe incoming MIDI activity.

The game attempts automatic connection and reconnection. A saved MIDI device is
restored when available; port matching can fall back to a matching or available
device.

The Controls tab includes a velocity meter. Its fill briefly shows the latest
MIDI velocity, a marker shows the current threshold, and a below-threshold hit
uses an amber indication. The fill disappears roughly 150 ms after the hit. A
mapped hardware MIDI hit also selects its assigned channel for inspection and
spatial highlighting.

### Editing bindings

Each drum channel can show keyboard keys and MIDI notes as binding chips. The
player can remove a binding, add a binding, or reset the tab.

The visible `+` action currently starts keyboard capture. Press the desired
key. `Esc`, `Tab`, function keys `F1` through `F12`, and modified key
combinations are reserved and cannot be captured as drum bindings.

The binding panel can display and remove existing MIDI-note chips, configure
the device, and show incoming activity.

**Unavailable:** starting MIDI-note capture from the current visible Controls
panel. A Keyboard/MIDI segment model and MIDI conflict confirmation behavior
exist behind the panel, but no visible control switches the `+` action from its
default keyboard-capture mode.

### Controls-tab mouse actions

| Mouse action | Result |
|---|---|
| Click previous/next beside Port | Cycle available MIDI input ports |
| Click `Rescan` | Re-enumerate MIDI ports |
| Click previous/next beside Velocity threshold | Decrease/increase threshold by 1 |
| Click a channel row | Select that channel for inspection and spatial highlighting |
| Click a binding chip's remove button | Remove that key or MIDI note |
| Click `+` on a channel | Start keyboard capture for that channel |
| Click `RESET TAB` | Open reset confirmation when bindings differ from defaults |
| Click `CONFIRM` in reset prompt | Restore default bindings |
| Click `CANCEL` in reset prompt | Keep current bindings |

Clicking a channel row selects it but does not perform the separate `+` or
remove actions.

### Default keyboard and MIDI bindings

| Drum channel | Default keyboard | Default MIDI note |
|---|---|---:|
| Closed hi-hat | `X` | 42 |
| Snare | `C`, `D` | 38, 40 |
| Bass drum | `Space`, `Convert` | 36, 35 |
| High tom | `V`, `F` | 48, 50 |
| Low tom | `B`, `G` | 45, 47 |
| Floor tom | `N`, `H` | 43, 41 |
| Cymbal | `M`, `J` | 57, 52 |
| Open hi-hat | `S` | 46 |
| Ride | `,`, `K` | 51, 59 |
| Left cymbal | `Z`, `A` | 49, 55 |
| Left pedal | `NonConvert` | 44 |
| Left bass drum | `Left Alt` | none |

### Input profiles

The player data model includes separate keyboard and MIDI profiles with
immutable built-in defaults. The built-ins are named `DTXMania default` and
`General MIDI drums`.

**Limited:** named profile operations such as Save As, Rename, Revert, Delete,
and profile switching exist in player data, but the profile bar and dialogs are
not visibly wired into the current Controls panel. The combined per-channel
binding editor is the dependable current UI.

## 19. Lane arrangement

The player data includes these built-in lane arrangements:

- Classic
- NX Type-B
- NX Type-D

The current Lanes panel displays the active arrangement's name. Reachable
editing actions include:

- reorder visible lanes;
- change lane widths;
- merge secondary logical channels into a displayed lane; and
- split a merged channel back into its own lane.

### Lanes-tab mouse actions

| Mouse action | Result |
|---|---|
| Click a lane's up/down button | Reorder that visible lane |
| Drag a lane-width slider | Change that lane's width |
| Click a lane merge button | Merge the lane into an adjacent displayed lane |
| Click a secondary channel's split button | Give that channel its own lane again |

Named lane profiles exist in player data with immutable built-ins.

**Limited:** the current Lanes workflow requires keyboard and mouse. The panel
does not visibly provide built-in preset selection or named-profile management,
so a player cannot switch to Classic, NX Type-B, or NX Type-D through the
current panel. Pad navigation does not provide lane editing.

## 20. HUD widget layout

The Widgets tab edits these widgets:

- Score Panel
- Combo
- Judgment Popup
- Phrase Meter
- Song Progress
- Now Playing
- Live Graph
- Speed Readout
- Frame Chrome
- Playfield

For a non-Playfield widget, the inspector exposes:

- a nine-position anchor grid and automatic anchoring toggle;
- horizontal and vertical offset;
- scale;
- stacking order; and
- separate visibility in normal play and practice.

Selecting an anchor sets the widget's origin to the same position internally.
The current inspector does not expose independent origin or screen/playfield
placement-space controls. The Playfield appears in the widget list but has no
right-side inspector.

### Mouse and keyboard layout actions

| Input | Action |
|---|---|
| Left-drag widget | Move it |
| Drag resize corner | Resize/scale it |
| `Alt`+click | Cycle through overlapping widgets |
| Arrow keys | Nudge selected widget by 1 pixel |
| `Shift`+arrow | Nudge selected widget by 8 pixels |
| `Ctrl+Z` / `Ctrl+Y` | Undo/redo |
| `Ctrl+S` | Save layout |

The Widgets tab also provides these direct mouse controls:

| Mouse action | Result |
|---|---|
| Click a widget in the widget list | Select it for editing |
| Click one of the nine anchor cells | Set both anchor and origin to that position |
| Click `auto` | Toggle automatic anchoring |
| Click/adjust offset, scale, or stacking controls | Change the selected widget value |
| Click play/practice visibility toggles | Change where the widget is shown |
| Click `Reset Widget` | Restore the selected widget's default layout |

Widgets use a nine-position anchor grid. Reset actions restore default widget
placement or the active settings tab's default values.

## 21. Player-visible media support

Charts can declare preview images, background images, background animation
events, and movie events.

- **Available:** song-select preimage/album art.
- **Available:** chart background event timing and layers.
- **Limited:** performance BGA currently renders colored placeholder overlays
  rather than the chart's real background images.
- **Unavailable:** actual chart video/movie playback.
- **Unavailable in Customize:** BGA/movie enable switches and background/movie
  alpha controls. These values can exist in saved configuration but have no
  current player-facing settings rows.

## 22. Other current limitations

- **Experimental:** guitar gameplay code is present, but no normal player-facing
  mode-selection workflow was found. Drums is the usable default mode.
- **Unavailable:** long-note gameplay.
- **Unavailable:** mouse operation for the main title, song, pause, and results
  workflows.
- **Unavailable:** opening pause with a drum pad during active performance.
- **Unavailable:** full pad-only operation of Customize and advanced practice.
- **Limited:** malformed charts are skipped, but the current player experience
  does not provide a full library error-management screen.
- **Limited:** after a score-history load failure, history can appear empty for
  the session and no player-facing recovery screen is provided.
- **Limited:** an interruption while score history is being saved is not
  guaranteed to preserve the player's previous history.
- **Limited:** visual behavior, physical MIDI-device compatibility, fullscreen
  scaling, and target-display performance are unverified in this manual pass.

## 23. Complete routine task recipes

### Play a song with keyboard

1. Press `Enter` on the title screen.
2. Use `Up`/`Down` to select a song.
3. Use `Left`/`Right` to select a difficulty.
4. Press `Enter`.
5. Play with the configured drum keys.
6. Press `Enter` or `Esc` on results to return.

### Play a song with a MIDI kit

1. Strike the bass drum on the title screen.
2. Use hi-hat and cymbal/ride hits to browse.
3. Strike the bass drum to enter difficulty selection.
4. Use hi-hat and cymbal/ride hits to choose a difficulty.
5. Strike the bass drum to start.
6. Play the chart with the mapped pads.
7. Strike bass drum or snare on results to return.

### Start practice with keyboard

1. Select a song and difficulty.
2. Press `Shift+Enter`.
3. Press `[` at the desired section start.
4. Press `]` at the desired section end.
5. Use `-`/`=` to change tempo.
6. Press `R` to restart the section.
7. Press `Tab` for detailed practice controls.

### Start practice with a MIDI kit

1. Select a song using pad navigation.
2. Strike the bass drum to enter difficulty selection.
3. Choose the difficulty with hi-hat and cymbal/ride hits.
4. Strike the floor tom.
5. Play the whole-song practice loop.
6. Use the keyboard or mouse for A/B selection, tempo, detailed feedback, and
   exiting the full practice workflow.

### Calibrate input offset

1. Press `F1` from title or song selection.
2. Open the Gameplay tab.
3. Activate Calibrate.
4. Tap 12 times to the beat.
5. Press `Enter` to apply the suggestion or `Esc` to cancel.

### Import songs

1. Open song selection.
2. Press `F6` and choose a ZIP or 7z archive, or drop it on the window.
3. Wait for the result notification and library rescan.
4. Start the imported chart from the updated song list.

### Change a drum binding

1. Press `F1` and open Controls.
2. Find the target drum channel.
3. Remove an unwanted binding if necessary.
4. Activate Add.
5. Press the keyboard key.
6. Close Customize.
7. Press `Enter` in the dirty-changes dialog to save, or choose another visible
   dialog action.

Adding a new MIDI-note binding is unavailable from the current visible panel.

### Change the HUD layout

1. Press `F2` on the title screen or open Widgets in Customize.
2. Select a widget.
3. Drag it, resize it, or change its placement settings.
4. Use `Alt`+click when widgets overlap.
5. Press `Ctrl+S` to save.
