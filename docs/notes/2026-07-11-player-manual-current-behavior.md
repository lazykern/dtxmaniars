# DTXManiaRS Player Manual: Current Behavior

Updated: 2026-07-15
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
navigation actions. Bound system Pause/Restart actions and Practice Setup menu
navigation are wired for a kit, while keyboard and mouse remain available for
searching, importing, Customize, and layout editing. Physical MIDI behavior was
not verified during this documentation pass.

### MIDI drum kit with the computer out of reach

**Limited.** The kit can start from the title screen, move through songs and
difficulties, start normal play or practice, navigate Practice Setup and open
pause when a system Pause note has been configured, operate pause, and leave
results. It cannot open Customize or edit Controls, Lanes, and Widgets without
keyboard or mouse access. If a nearby person opens Customize, the kit can
operate the Gameplay, Audio, Drums, and System settings tabs. This reachability
is confirmed from wiring and tests, not a physical kit check.

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

Opening pause from a drum pad requires assigning a system Pause note in the
active MIDI profile; it is deliberately unbound by default.

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

The normal pause menu contains Resume, Restart Song, Practice This Section,
Quick Settings, and Return to Song Select. Quick Settings contains scroll speed,
lane visibility, BGM volume, and input offset.

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

**Limited:** pad controls work after the pause menu is open. Opening it from
active play requires a configured system Pause binding; no pad is bound by
default.

## 11. Stage clear, failure, and results

At the end of normal play, the game displays a stage-clear or stage-failed
banner. The banner advances automatically after about 1.6 seconds.

| Input | Action |
|---|---|
| `Enter` or `Space` | Skip the clear/fail banner |

The Results screen shows:

- Song title, artist, and level
- Final score
- Maximum combo
- Rank
- Perfect, Great, Good, Poor, and Miss counts and percentages
- Total judged notes
- Save qualification and save status
- Personal-best delta for comparable runs
- Timing bias/spread, weakest lanes, and weakest section when the run supplies
  enough timing evidence

Ranks use SS, S, A, B, C, D, and E. Rank includes the Perfect rate, Great rate,
and maximum-combo rate; it is not based on score alone.

### Results actions

| Input | Action |
|---|---|
| `Left` / `Right` | Select Continue, Retry, or Practice |
| `Enter` or `Space` | Activate the selected action |
| `Esc` | Continue to song selection |
| `R` | Retry the same chart directly |
| `Tab` | Show or hide timing, lane, and section details |
| Hi-hat / cymbal or ride | Move among Continue, Retry, and Practice |
| Bass drum | Activate the selected action |
| Snare | Continue to song selection |
| Floor tom | Open Practice directly |

The first navigation input during the animated reveal completes the reveal and
is consumed. Retry reloads the same chart without returning to song selection.
Practice opens stopped Setup. For a qualifying normal run with enough timing
evidence, the Practice action names the weakest section and seeds that loop,
one-bar pre-roll, and `1.00x` tempo. Otherwise it opens manual Setup.

### Saved results

Results attempts persistence when the screen opens. The save-status line shows
success, failure, Practice, modified-speed, or No Fail qualification. A
qualifying normal run enters native history and updates a compatible per-chart
score record when that location is writable. No Fail enters native history as
an assisted run but cannot update the ordinary best or compatible score.

Failed normal plays are recorded. Practice and modified-speed runs do not enter
normal score history. A failed save is visible, but Results has no retry-save
action or recovery workflow.

## 12. Practice mode

Start practice by selecting a song and pressing `Shift+Enter`, or by striking
the floor tom on song selection. The Practice action on Results can also seed a
recommended section. Every route opens Practice Setup before the first attempt.

Setup starts with preview stopped. The preview uses the real playfield, notes,
BGA, audio, and timeline, but input is not judged and cannot create misses,
score, combo, gauge changes, attempt records, lane diagnosis, Wait halts, or
Ramp evaluation. Start Practice commits the draft and begins from its configured
pre-roll/count-in.

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
| `Tab` | Open Practice Settings |
| `Esc` | Open the Practice pause menu |

Tempo ranges from 0.50x to 1.50x and defaults to 1.00x.

### Loop behavior

Without an A/B region, the whole song is the practice span. Set A and B to
repeat a smaller section. If B is placed before A, the points are reordered.
Mouse-dragging a region on the Setup timeline snaps it to bars and enforces a
minimum one-bar region.

Changing or clearing a loop disarms an active tempo ramp because the ramp is
tied to the practiced section.

### Setup, Settings, and Progress

Setup and Practice Settings share loop, transport, trainer, saved-loop,
Progress, preview, and timeline controls. Use `Up`/`Down` to select a visible
setting, `Left`/`Right` to adjust it, and `Enter` or `Space` to activate it.
Trainer mode is one mutually exclusive value: Off, Wait, or Ramp. Ramp detail
rows appear only in Ramp mode.

Practice Settings opens with preview stopped and invalidates the interrupted
attempt. Continue Practice commits its draft and begins a fresh attempt from
pre-roll/count-in. `Esc` during a run instead opens the small Practice pause
menu. Pause Resume restores the exact frozen audio/chart position; its other
choices are Restart Loop, Practice Settings, and Exit to Song Select.

The mouse can:

- click the timeline to seek;
- drag the timeline to create an A/B loop; and
- press previous-bar, preview play/pause, and next-bar transport buttons.

### Saved loops

Saved loops are stored in `CONFIG_DIR/practice-presets.toml` and looked up by
the chart's canonical hash plus selected difficulty index. Save as New creates
a preset; Update Saved Loop changes the selected saved source; Delete requires
confirmation. Merely editing a draft never changes a named saved loop. Last
Used updates only when Start Practice or Continue Practice commits the draft.

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
flow percentage. Selecting Wait replaces Off or Ramp; selecting Ramp replaces
Off or Wait.

### Attempt feedback

Practice retains up to 20 completed attempt records. Progress shows completed
attempts for the current section, using accuracy for Off/Ramp and flow for Wait,
along with mean timing error, tempo, and lane diagnosis. Interrupted, partial,
preview, and settings-invalidated attempts do not enter Progress or Ramp
evaluation. Small reports and notifications appear when loops wrap or settings
change.

### Practice limitations

- **Limited:** changing playback rate changes pitch; pitch-preserving tempo is
  unavailable.
- **Limited:** source and automated tests cover keyboard, mouse, and pad
  navigation, but physical MIDI operation and room-distance readability were
  not checked during this documentation pass.
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

- During Keyboard or MIDI binding listening/confirmation, `Esc` cancels capture
  without closing. The modal's `Cancel` button does the same.
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

**Limited:** pads cannot enter the Controls, Lanes, or Widgets content. Controls
and Lanes are currently mouse-led despite having keyboard/pad navigation logic
in their internal model; that navigation is not wired to the visible panels.

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

### Play-speed behavior

Normal Play Speed applies one effective rate to audio, chart time, notes,
visuals, seeking, and completion, so those systems stay synchronized. Values
other than `1.0x` change pitch and make the run ineligible for an ordinary
saved record. Practice tempo uses the same synchronized rate path under
Practice rules.

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

The redesigned Controls tab has separate `Keyboard` and `MIDI` segments. Each
segment has its own active profile, draft, dirty indicator, and binding list.
Click the segment name to switch which source type is being edited.

The selected channel row and the corresponding lane in the live playfield
preview highlight together. Clicking a row selects it. Striking an already
mapped MIDI pad also selects one of its assigned channels. Hovering a shared
binding chip highlights all of that source's owning lanes.

### MIDI device actions

- View the selected/connected MIDI port.
- Cycle the selected port when devices are available.
- Rescan MIDI devices.
- Set the MIDI velocity threshold from 0 to 127. The default is 0.
- Observe incoming MIDI activity.

The game attempts automatic connection and reconnection. A saved MIDI device is
restored when available; port matching can fall back to a matching or available
device.

The MIDI segment includes a velocity meter. Its fill briefly shows the latest
MIDI velocity, a marker shows the current threshold, and a below-threshold hit
uses an amber indication. The fill disappears roughly 150 ms after the hit. A
mapped hardware MIDI hit also selects its assigned channel for inspection and
spatial highlighting.

### Editing bindings

Each channel row shows only the active segment's key or note chips. A shared
marker indicates that one source is assigned to more than one channel. A row
with no source in the active segment is warning-tinted and says `no binding`.

To add a keyboard binding:

1. Click `Keyboard`.
2. Click `+` on the target channel.
3. Press the desired unmodified key.
4. Review the arrived key in the modal.
5. If the key is already used, click `Add shared` to keep both assignments or
   `Move here` to remove the other assignments. `Left`/`Right` also switches
   this choice.
6. Click `Confirm` or press `Enter`. Click `Cancel` or press `Esc` to discard.

`Esc`, `Tab`, function keys `F1` through `F12`, and keys pressed with
`Ctrl`, `Alt`, or `Super` are reserved and remain ignored while capture waits.

To add a MIDI binding:

1. Click `MIDI`.
2. Click `+` on the target channel.
3. Hit the desired pad. Only a new positive-velocity NoteOn after capture was
   armed is accepted; a stale earlier hit is ignored.
4. Review the arrived note number and velocity.
5. Choose `Add shared` or `Move here` if another channel owns the note.
6. Click `Confirm`, press `Enter`, or hit the same MIDI note again. `Esc` or
   the modal's `Cancel` discards it. Hitting a different note replaces the
   arrived candidate so the player can retry without reopening capture.

The listening modal shows live MIDI note and velocity information. A hit at or
below the configured threshold is shown as below threshold for diagnosis, but
a new positive-velocity note can still be learned as a binding; the threshold
blocks gameplay dispatch, not capture. Capture temporarily owns raw pad input,
so pad navigation is suspended until it closes.

Click a chip's `x` to remove only that channel's claim. Other owners of a
shared source remain bound. At runtime, pressing one shared keyboard key or
hitting one shared MIDI note generates a hit for every assigned channel.

To bind a MIDI system action, click `+` on Pause or Restart in the visible
System card, then hit the intended pad. A free note binds immediately without
the lane-sharing confirmation step. The modal refuses a note already owned by
a drum lane or the other system action and remains open so another pad can be
tried. Pause and Restart have no default MIDI notes.

### Controls-tab mouse actions

| Mouse action | Result |
|---|---|
| Click previous/next beside Port | Cycle available MIDI input ports |
| Click `Rescan` | Re-enumerate MIDI ports |
| Click previous/next beside Velocity threshold | Decrease/increase threshold by 1, clamped from 0 to 127 |
| Click `Keyboard` / `MIDI` | Switch the visible binding source and profile |
| Click a channel row | Select that channel for inspection and spatial highlighting |
| Click a binding chip's remove button | Remove that key or MIDI note |
| Click `+` on a channel | Start capture for the active Keyboard or MIDI segment |
| Click `+` on Pause or Restart | Start system-action capture for the active segment |
| Click `Reset tab` | Open reset confirmation |
| Click `Confirm reset` | Restore the full composed defaults, including both source types and MIDI device fields |
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

Keyboard and MIDI profiles are managed separately. Their immutable built-ins
are `DTXMania default` and `General MIDI drums`.

The profile bar provides:

- click the profile name to select a built-in or user profile;
- `Save` to overwrite a dirty user profile;
- `Save As` to enter a new profile name;
- the overflow menu for `Save As` on built-ins, or `Rename`, `Revert`, and
  `Delete` on user profiles; and
- an amber dot when the current draft differs from its saved value.

Built-ins cannot be overwritten, renamed, reverted, or deleted. Editing one
requires Save As; closing with dirty built-in edits and choosing Save creates
an automatically named copy. Name dialogs accept typed text and Backspace,
`Enter`/`OK` submits, and `Esc`/`Cancel` closes without applying. Names are
trimmed and limited to 48 characters. Blank names, control characters,
reserved built-in names, and case-insensitive duplicates are rejected.

When a captured key or MIDI note is confirmed while the live editor clock is
ready, the game also emits one immediate hit on the target lane. The player may
therefore see and hear that lane respond at the moment the binding is committed.

Selecting another profile with unsaved edits asks whether to cancel, discard,
or save before switching. Revert uses the same dirty-decision dialog. Deleting
a user profile requires confirmation and falls back to the built-in default.
Registry transaction failures appear below the profile bar and leave the
current editor session available for another action.

**Limited:** despite its label and placement inside one segment, `Reset tab`
currently resets the complete combined binding/device state, not only the
visible Keyboard or MIDI segment. This can discard changes outside the segment
the player is looking at.

## 19. Lane arrangement

The player data includes these built-in lane arrangements:

- Classic
- NX Type-B
- NX Type-D

The Lanes tab now exposes the active lane profile in the same profile bar used
by Controls. The selector can switch among built-ins and saved user profiles;
Save, Save As, Rename, Revert, Delete, dirty switching, and close protection
follow the profile behavior described above.

The panel contains slim visible-lane rows, a detail card for the selected lane,
and a `Hidden` strip when any channels are unassigned. Reachable actions include:

- reorder visible lanes;
- change lane widths;
- merge secondary logical channels into a displayed lane; and
- split a merged channel back into its own lane.

### Lanes-tab mouse actions

| Mouse action | Result |
|---|---|
| Click a lane row | Select it and open/update its detail card |
| Click a pad in the playfield preview | Select its lane |
| Drag the middle of a preview pad horizontally | Reorder that visible lane at drop |
| Drag the left or right edge of a preview pad | Resize that lane continuously |
| Drag the detail-card Width slider | Set width between the enforced minimum and maximum |
| Click `+ add`, then a channel | Merge an unassigned or secondary channel into this lane |
| Click `x` on a secondary-channel chip | Split it into its own visible lane |
| Click `Hide lane` | Remove the displayed lane and place its channels in Hidden |
| Click a channel in `Hidden` | Restore that channel as its own visible lane |

The primary channel is shown as fixed text in the detail card; only secondary
chips can be split there. The add chooser does not offer another lane's primary
channel, because moving it would leave that lane empty. Width edits clamp to a
non-zero range. Drag operations and detail edits participate in layout
undo/redo, and the preview updates immediately before the profile is saved.
The final visible lane cannot be hidden. Hiding affects visual lane assignment,
not logical judgment: a hidden channel can remain bound and its notes can still
be hit and scored even though it has no displayed lane.

**Limited:** lane rows themselves are selection-only, not drag handles. Lane
reorder and edge resize require dragging the playfield preview, while other
detail actions require clicking panel controls. The visible Lanes workflow is
therefore mouse-dependent; keyboard/pad reorder and resize reducers exist but
are not connected to player input. `Ctrl+Z`/`Ctrl+Y` can undo/redo edits, but
profile Save/Save As still requires clicking the visible profile controls.

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
- **Limited:** opening Pause from a drum pad requires a configured system Pause
  binding because the action is unbound by default.
- **Limited:** pads cannot open Customize or operate its Controls, Lanes, and
  Widgets panels. Practice Setup, Progress, preview transport, Start Practice,
  Continue Practice, and Pause actions are wired to shared pad navigation;
  physical-kit behavior remains unverified.
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
3. In the stopped Setup surface, configure the loop, tempo, trainer, pre-roll,
   and count-in. Use Preview if needed; preview input is not judged.
4. Choose Start Practice to begin from pre-roll/count-in.
5. During the run, press `Tab` to open Settings and choose Continue Practice to
   restart from pre-roll, or press `Esc` to open Pause.
6. Choose Resume in Pause to continue from the frozen position, or Exit to Song
   Select to leave Practice.

### Start practice with a MIDI kit

1. Select a song using pad navigation.
2. Strike the bass drum to enter difficulty selection.
3. Choose the difficulty with hi-hat and cymbal/ride hits.
4. Strike the floor tom.
5. Use the shared pad-navigation verbs to configure stopped Setup, inspect
   Progress, or operate preview transport.
6. Activate Start Practice to begin from pre-roll/count-in.
7. During the run, use a configured system Pause note to open Pause, then
   choose Practice Settings, Resume, Restart Loop, or Exit to Song Select. The
   Pause note is unbound by default and must be configured before kit-only use.

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
4. Click `+` on the target row.
5. Press a keyboard key in the Keyboard segment, or hit a pad in the MIDI
   segment.
6. For a lane source, review the candidate, choose Add shared or Move here if
   needed, and confirm. A free Pause/Restart system source binds immediately.
7. Save the user profile, or close Customize and choose Save in the dirty guard.

### Change the HUD layout

1. Press `F2` on the title screen or open Widgets in Customize.
2. Select a widget.
3. Drag it, resize it, or change its placement settings.
4. Use `Alt`+click when widgets overlap.
5. Press `Ctrl+S` to save.
