# DTXManiaRS Player User Stories

Updated: 2026-07-15
Status: Behavioral-research baseline
Source: `docs/notes/2026-07-11-player-manual-current-behavior.md`

## 1. Purpose

This document describes how three player types attempt to use the current game.
It covers first-time setup and returning play. It is an input for later UX
evaluation, not a redesign specification.

The three player types are defined by equipment and physical reach rather than
age, skill, or demographic assumptions:

1. Keyboard and mouse player.
2. MIDI drum player with keyboard, mouse, and computer within reach.
3. MIDI drum player with the computer out of reach during play.

## 2. Scope boundary

The merged Controls and Lanes redesign is now in scope. These stories cover
discovering profiles, choosing Keyboard or MIDI, binding and sharing sources,
resolving conflicts, selecting a MIDI port and threshold, arranging lanes,
and saving or recovering drafts. They distinguish implemented player actions
from keyboard/pad navigation described by the design but not wired to the
visible panels.

This document does evaluate whether a player can reach routine actions after
setup. For example, the requirement to configure a system Pause binding before
a distant-kit session remains in scope because it affects active play, not only
input configuration.

Also excluded:

- Guitar mode
- Chart authoring
- Developer and command-line workflows
- Planned features presented as current behavior
- Visual-quality conclusions without a live graphical review
- Physical MIDI reliability conclusions without hardware testing

## 3. Evidence language

- **Current fact**: confirmed in the current player manual and source review.
- **Hypothesis**: expected player behavior or reaction that requires research.
- **Blocker**: the player cannot complete the intended action in the stated
  physical context.
- **Workaround**: an alternate path using another device, person, or changed
  physical setup.
- **Research question**: something that must be observed or asked, not inferred.

In journey tables, `Current action/path`, `Current support`, `Current outcome`,
and `Reachability` columns state current facts. Columns explicitly named
`Research focus` or `Friction hypothesis`, plus all `Possible` motivations and
concerns, are hypotheses rather than findings.

## 4. Shared player jobs

All three player types share these high-level jobs:

| Job | Desired outcome |
|---|---|
| Become ready | The game recognizes the intended play inputs and feels synchronized |
| Find music | The player can locate a suitable song and difficulty |
| Start quickly | The player moves from launch to performance without unnecessary setup |
| Perform | Hits produce understandable sound, judgment, score, combo, and gauge feedback |
| Recover | The player can pause, resume, retry, quit, or recover from failure |
| Learn | The player can isolate a section, adjust tempo, repeat it, and understand progress |
| Preserve progress | Completed normal plays appear in results and later history |
| Add content | The player can import supported song archives and find the result |
| End safely | The player can leave without uncertainty about saved progress |

### Shared startup and degraded-data stories

These are current facts shared by all three player types:

- Startup loads saved settings, layout, input readiness, and score history.
- Missing or unreadable basic settings/layout data falls back to defaults.
- Damaged profile data can leave only built-in profiles usable for the session.
  A back-up-and-reset dialog exists in the implementation but is not currently
  wired from startup corruption detection, so players should not rely on that
  recovery path being reachable.
- If score history cannot load, history can appear empty with no player-facing
  recovery screen.
- Malformed charts are skipped without a full library error-management screen.
- An interrupted history save is not guaranteed to preserve the prior history.

> As a returning player, I want degraded or reset player data explained before
> I begin playing so I can distinguish a recovery fallback from lost progress.

Research must test whether players notice missing songs/history/settings, what
cause they infer, and whether they continue, restart, or abandon the session.

### Shared import outcome stories

Import is not one happy path. Current player-visible outcomes include:

| Import condition | Current outcome |
|---|---|
| Valid ZIP or 7z | Extract, rescan, notify, and attempt to select imported song |
| Multiple picker selections | Each selected archive starts an import |
| RAR | Shown in picker but rejected as unsupported |
| Unsupported dropped file | Error notification |
| Unsafe archive path | Archive rejected |
| Archive with no charts | No-charts notification |
| Already imported content | Duplicate avoided; existing content is reported |
| Active search hides destination | Import succeeds, but automatic selection can silently give up |

> As a player adding content, I want success, rejection, duplication, and
> filtered-result states to be distinguishable so I know whether to retry,
> extract manually, clear search, or use the existing song.

### Shared loading stories

After play or practice is requested, a loading screen prepares the chart and
audio and shows progress/status.

| Player type | Cancel loading | Loading failure |
|---|---|---|
| Keyboard/mouse | `Esc` cancels and returns to song selection | Automatically returns to song selection |
| Nearby MIDI | Keyboard `Esc` fallback | Automatically returns; kit cannot cancel |
| Distant MIDI | No kit cancel action | Automatically returns, but manual cancel is a blocker |

- **LOAD-1:** As a player, I want progress to distinguish normal preparation
  from a stalled or failed load.
- **LOAD-2:** As a player who selected the wrong chart, I want to cancel before
  performance begins.
- **LOAD-3:** As a MIDI-only distant player, I want loading cancellation on my
  reachable device so a mistaken selection does not force me to wait.
- **LOAD-4:** As a player returned after failure, I want an understandable error
  and recovery path rather than an unexplained return to the song wheel.

### Shared non-Control/Lane settings stories

The following settings remain in scope because they affect routine behavior.

| Domain | Player-level choices or effect |
|---|---|
| Gameplay | Scroll speed, input/BGM offsets, Play Speed, damage, lane-line display, calibration |
| Audio | BGM/drum sound toggles and master/BGM/drum volumes |
| Drums | Sound grouping, cymbal behavior, chip/pad sound priority, polyphony |
| System | VSync, performance overlay, metronome |
| Widgets | Normal/practice visibility and HUD placement for non-Playfield widgets |

Behavioral constraints:

- Play Speed outside 1.0x can desynchronize chart and audio; it is not practice
  tempo.
- Closing Customize triggers a settings-save attempt, but failure has no
  on-screen error or retry prompt.
- `Ctrl+S` or closing triggers a HUD-layout save attempt; failure is also
  silent.
- Normal and practice widget visibility can differ.
- Stage failure defaults on and has no exposed setting row.

Reachability after Customize has already been opened:

| Player type | Gameplay/Audio/Drums/System | Widgets |
|---|---|---|
| Keyboard/mouse | Keyboard and mouse | Keyboard and mouse |
| Nearby MIDI | Pads can operate four settings tabs; computer remains fallback | Keyboard/mouse fallback |
| Distant MIDI | Setup-time only because opening Customize requires computer input | Setup-time only |

- **SET-1:** As a player, I want audio and gameplay changes to produce an
  understandable immediate effect without risking silent loss on close.
- **SET-2:** As a drummer, I want grouping, priority, and polyphony choices to
  match what I hear from physical hits and chart sounds.
- **SET-3:** As a player, I want performance and metronome feedback appropriate
  to normal play without confusing it with practice count-in.
- **SET-4:** As a practice user, I want HUD visibility choices to preserve the
  feedback I need while hiding score-oriented information when appropriate.

### Shared Controls and Lanes stories

Controls and lane setup are three separate profile domains: Keyboard, MIDI,
and Lanes. Each has built-ins, user profiles, a draft, an amber dirty marker,
selection, Save, Save As, and protected switching/closing. User profiles also
offer Rename, Revert, and Delete. Built-ins are immutable.

| Player job | What the player must physically do | Current outcome |
|---|---|---|
| Choose input source | Click `Keyboard` or `MIDI` | The segment, profile, and visible chips switch |
| Inspect a channel | Click its row; for MIDI, optionally hit an already mapped pad | Row and playfield lane highlight together |
| Add a key | Click `+`, press an allowed key, choose sharing behavior if needed, then click Confirm or press `Enter` | Key is added to the draft; confirmation can immediately fire the target lane |
| Add a MIDI note | Click `+`, hit a new pad, review note/velocity, choose sharing behavior if needed, then click Confirm, press `Enter`, or hit the same note again | Note is added to the draft; confirmation can immediately fire the target lane |
| Resolve a collision | Click `Add shared` or `Move here`, or press `Left`/`Right`, then confirm | Source either fans out or moves exclusively |
| Remove one claim | Click the chip's `x` | Only that channel loses the source |
| Check soft hits | Hit a pad while viewing the MIDI device card/capture modal | Meter and below-threshold feedback expose velocity |
| Choose device | Click port previous/next or `Rescan`; click threshold previous/next to adjust one unit | MIDI draft updates and connection state remains visible |
| Select/create a profile | Click profile name and a choice, or click `Save As`, type a name, then press `Enter`/click OK | Active registry/profile changes if persistence succeeds |
| Manage a user profile | Open `...`, then click Rename, Revert, or Delete and complete its dialog | Named profile is changed, reset, or removed |
| Select a lane | Click its row or its preview pad | Detail card and preview selection synchronize |
| Reorder/resize | Mouse-drag a preview pad or its edge; alternatively drag the Width slider | Draft preview changes immediately and clamps width |
| Merge/split/hide/restore | Click `+ add` and a channel, a secondary chip's `x`, `Hide lane`, or a Hidden chip | Visible lane/channel composition changes |
| Preserve edits | Click Save/Save As, or close/switch and choose Save | User profile is written; dirty built-in becomes a copy |

- **CTRL-1:** As a keyboard player, I want to test an arrived key before
  committing so an accidental press does not silently replace my mapping.
- **CTRL-2:** As a MIDI drummer, I want note number, velocity, threshold, port,
  and connection state visible so I can separate device problems from mapping
  problems.
- **CTRL-3:** As a player using one physical source for two logical channels,
  I want an explicit Add shared versus Move here choice and visible shared
  markers so fan-out is intentional.
- **CTRL-4:** As a profile user, I want switching, closing, revert, deletion,
  and write failure to protect or clearly account for unsaved work.
- **LANE-1:** As a player, I want list selection and playfield manipulation to
  stay synchronized so I know which lane I am changing.
- **LANE-2:** As a player, I want hide, restore, merge, split, width, and order
  actions to remain reversible before I save.
- **LANE-3:** As a keyboard/pad-only operator, I want every lane-edit command
  reachable without a mouse. This is currently unsupported because the visible
  Lanes panel does not drive its internal keyboard/pad reducer.

Research must observe whether players understand that Controls has two
independent profile contexts, whether `Add shared` predicts runtime fan-out,
whether the preview pad edges look draggable, whether `Hide lane` is mistaken
for disabling judgment, and whether dirty/profile states remain understandable
when switching tabs or closing. Capture studies should also observe whether the
immediate lane response on confirmation reads as useful verification or as an
unexpected extra hit.

### Shared performance, media, and result contracts

During normal performance, current feedback can include playfield/chips, score,
combo, judgment and signed timing, gauge, detailed counts, phrase/progress,
now-playing data, graph, speed, and keyboard visualization.

The player must press a configured drum key or hit a mapped MIDI pad as a chip
reaches the judgment line. Default timing tiers are Perfect through 34 ms,
Great through 67 ms, Good through 84 ms, Poor through 117 ms, then Miss. Score
and combo respond to judgments; misses reduce the gauge according to Damage
Level, and the default-enabled stage-failure rule can end the performance.
Chart music, automatic sounds, keysounds, grouping, priority, polyphony, and
empty-hit fallback determine what the player hears.

Current hotkey behavior:

- `F11` toggles performance information.
- Scroll speed, input offset, and performance-overlay changes persist shortly
  after the last change.
- Per-song BGM offset is written immediately when changed.

Media limitations apply to all personas: song-select album art is available,
but performance BGA uses colored placeholders and actual video playback is
unavailable.

The clear/fail banner auto-advances after about 1.6 seconds. Keyboard
`Enter`/`Space` can skip it; no equivalent pad skip is documented.

Leaving Results triggers the native-history and compatible per-chart save
attempts. Closing while still on Results can lose the play, and write failure
has no player-visible error or retry.

- **RESULT-1:** As a player, I want to know when persistence is attempted and
  whether it succeeded.
- **RESULT-2:** As a player, I want a failed save to preserve prior history and
  provide a recovery action.
- **HUD-1:** As a player, I want performance feedback prioritized for my viewing
  distance and current goal.
- **MEDIA-1:** As a chart player, I want unsupported background media represented
  honestly rather than mistaking placeholders for intended chart visuals.

### Shared practice contract

Practice behavior is richer than starting a loop:

- Every Practice request first opens Setup with preview stopped. Preview uses
  the real chart presentation, but input is not judged and creates no gameplay
  or Progress data.
- Without A/B points, the entire song loops and does not end automatically.
- `[` and `]` set bar-snapped A/B points; `Backspace` clears the loop.
- Snap cycles Bar, Beat, and half-beat.
- Pre-roll cycles one bar, two seconds, and off; count-in defaults on.
- Tempo ranges from 0.50x to 1.50x and changes pitch.
- Manual tempo or loop changes disarm an active accuracy ramp.
- Ramp defaults to 0.70x start, 1.00x target, 0.05x step, 90% pass, and one
  required successful pass. Two failed passes step down; reaching the target
  completes and disarms the ramp.
- Trainer is exactly one of Off, Wait, or Ramp. Wait stops at unhit notes; Ramp
  owns accuracy-based tempo progression.
- Only completed attempts enter Progress and Ramp evaluation. Progress exposes
  accuracy or Wait flow, timing bias, and per-lane diagnosis.
- `Esc` opens Pause during a run; Resume continues from the exact frozen
  position. `Tab` opens Practice Settings; Continue Practice starts a fresh
  attempt from pre-roll/count-in.
- Saved loops require explicit Save as New, Update Saved Loop, or confirmed
  Delete. They are isolated by canonical chart hash and selected difficulty.

- **PRACTICE-1:** As a learner, I want whole-song versus A/B looping to be
  explicit so I understand why practice does not finish.
- **PRACTICE-2:** As a learner using ramp, I want promotion, step-down, disarm,
  and completion states to be predictable.
- **PRACTICE-3:** As a learner using wait mode, I want to understand that waited
  notes measure flow rather than normal timing accuracy.
- **PRACTICE-4:** As a player changing tempo, I want to know that current tempo
  changes pitch before choosing it as a practice method.
- **PRACTICE-5:** As a player exiting practice, I want Pause to provide a clear
  Exit to Song Select action without confusing it with Practice Settings.

## 5. Player Type 1: Keyboard and Mouse

### Context

The player sits at the computer. Keyboard and mouse are always within reach.
The keyboard is both the navigation device and the drum-performance device.

Possible motivations:

- Try DTX charts without owning an electronic drum kit.
- Preview songs or learn chart structure.
- Play casually with low setup cost.
- Customize the HUD and practice with precise computer controls.

Possible concerns:

- Keyboard key rollover and physical comfort.
- Remembering a large drum-key layout.
- Understanding that primary menus are keyboard-driven rather than clickable.
- Distinguishing normal Play Speed from practice tempo.

These motivations and concerns are hypotheses until validated with players.

### Core story

> As a keyboard and mouse player, I want every routine action to remain within
> normal desktop reach so I can browse, play, practice, recover, and leave
> without another controller.

### First-time setup journey

| Stage | Player intent | Current action/path | Outcome | Research focus |
|---|---|---|---|---|
| Launch | See whether the game started correctly | Launch and wait through the short loading splash | Title appears automatically | Does the splash communicate healthy startup? |
| Find songs | Understand why the library is empty | Press `Enter`; follow the empty-library hints | Player sees rescan, import, and archive-drop options | Which option does a new player understand first? |
| Import | Add playable content | Press `F6` and mouse-select files, or mouse-drag files onto the window | Valid ZIP/7z imports rescan; invalid, unsafe, duplicate, unsupported, and filtered outcomes differ | Does the player choose the correct recovery for each notification? |
| Learn navigation | Move through songs and difficulties | Press `Up`/`Down` for songs, `Left`/`Right` for difficulty, type to search, `Tab` to sort, `Enter` to play, `Shift+Enter` for practice, and `Esc` to go back | Entire song-selection workflow is reachable | Do players try to click the wheel before reading keyboard hints? |
| Verify timing | Make keyboard hits feel synchronized | Press `F1`, click `Calibrate`, press configured drum keys 12 times, then press `Enter` to apply or `Esc` to cancel; alternatively click/drag the Input Offset slider | Input offset can be changed | Can the player understand input offset versus BGM offset? |
| Learn and edit keys | Identify drum mappings | Press `F1`, click Controls and Keyboard, inspect rows; click `+`, press a key, review it, choose Add shared/Move here if needed, then confirm | Defaults and custom keyboard profiles are visible and editable | Do source chips and lane highlights create a usable mental map? |
| Arrange lanes | Make the playfield match preference | Click Lanes, select a profile, then click rows/pads; drag preview pads/edges and use detail-card add/split/hide/restore actions; Save As if editing a built-in | Lane profiles and live draft preview are reachable with mouse | Do players discover preview dragging without instruction? |
| Load chart | Wait, cancel a mistake, or recover from failure | Observe progress; use `Esc` to cancel | Cancel/failure returns to songs | Does the player understand why loading ended? |
| First play | Complete a chart | Select difficulty and press `Enter` | Normal performance, clear/fail banner, and results are reachable | Is the HUD understandable without prior DTX knowledge? |
| Save result | Preserve the first play | Leave Results with `Enter` or `Esc` | Leaving Results triggers a save attempt | Does the player remain on Results and close the app before the attempt occurs? |

### First-time acceptance stories

- **KM-F1:** As a new keyboard player, I want empty-library guidance so I can
  add a song without developer knowledge.
- **KM-F2:** As a new keyboard player, I want visible navigation cues so I do
  not assume the song wheel is mouse-operated.
- **KM-F3:** As a new keyboard player, I want a discoverable drum-key reference
  so I can associate keys with lanes before the chart begins.
- **KM-F4:** As a timing-sensitive player, I want calibration and offsets to
  explain their effect so I do not compensate in the wrong direction.
- **KM-F5:** As a first-time player, I want to know when result persistence is
  attempted and whether it succeeded so I do not unknowingly lose a play.
- **KM-F6:** As a new keyboard player, I want binding capture, sharing, and
  profile saving to make the resulting gameplay keys unambiguous.
- **KM-F7:** As a mouse user, I want lane reorder and resize targets to look
  draggable and remain reversible before saving.

### Returning normal-play journey

| Stage | Typical behavior | Current support | Friction hypothesis |
|---|---|---|---|
| Launch | Press through title immediately | `Enter` reaches remembered song selection | Low friction after controls are learned |
| Resume browsing | Expect prior song/difficulty | Last selection and difficulty are remembered when available | Strong continuity if library paths remain stable |
| Search | Type a title or artist | Live search is available | Efficient for known-item retrieval |
| Sort | Change browsing order | `Tab` cycles Default, Title, Artist | Hidden shortcut may be forgotten |
| Choose difficulty | Compare levels and past performance | Difficulty slots, level, achievement, rank, density, BPM, skill, and history are visible | Information density may exceed what casual keyboard players need |
| Start | Press `Enter` | Normal play starts | Direct and repeatable |
| Adjust | Correct speed or offsets during play | Press `Up`/`Down` for scroll speed, `Left`/`Right` for input offset, add `Ctrl` for fine input adjustment, or press `Shift+Up`/`Shift+Down` for song BGM offset | Accidental adjustment is possible because arrows are active during play |
| Pause | Interrupt safely | Press `Esc`; select Resume, Restart Song, Practice This Section, Quick Settings, or Return to Song Select; `Esc` resumes | Fully reachable |
| Finish | Inspect score and judgments | Results presents score, combo, rank, counts, and percentages | Diagnostic depth may not answer why performance was weak |
| Return | Press `Enter` or `Esc` | Song selection returns and a save is attempted | Navigation is reachable; save failure is silent |

### Returning practice journey

| Stage | Player intent | Current action/path | Outcome |
|---|---|---|---|
| Enter practice | Configure practice instead of recording a score | `Shift+Enter` on song selection | Stopped, non-judged Setup opens before any attempt |
| Preview | Inspect the exercise before starting | Use preview transport/timeline | Audio, notes, BGA, and timeline move without judgments or Progress data |
| Mark section | Repeat a difficult span | `[` sets A; `]` sets B | Bar-snapped A/B loop |
| Clear section | Return to whole-song practice | Press `Backspace` while running, or clear A/B in Setup/Settings | A/B clears and active Ramp disarms |
| Change tempo | Reduce or increase difficulty | `-` / `=` | 0.50x to 1.50x tempo |
| Restart | Repeat immediately | `R` | Current span restarts with pre-roll |
| Automate progression | Raise tempo after accurate passes | Press `T` while running, or choose Ramp in Setup/Settings | Ramp evaluates only completed eligible attempts |
| Change attempt entry | Control seek precision and readiness time | Press `Tab`, then edit Snap/Pre-roll/Count-in and choose Continue Practice | Continue starts a fresh attempt from pre-roll/count-in |
| Wait for notes | Practice note recognition without timing pressure | Choose Wait as the trainer in Setup/Settings | Playback stops at unhit notes; Ramp is not simultaneously active |
| Inspect attempts | Compare accuracy and timing | Open the Progress tab in Setup/Settings | Only completed attempts, timing, and diagnosis appear |
| Seek precisely | Choose another location | Mouse timeline click/drag and transport buttons | Seek and loop editing are reachable |
| Pause/resume | Interrupt without restarting the exercise | Press `Esc`, then choose Resume | Continues from the exact frozen position |
| Exit | Leave without writing a score | Press `Esc`, then choose Exit to Song Select | Returns without normal score persistence |

Practice tempo currently changes pitch, and whole-song practice loops rather
than ending automatically. These are current facts to explain before measuring
whether the player understands or accepts the behavior.

### Keyboard/mouse blockers and workarounds

| Issue | Classification | Workaround |
|---|---|---|
| Main title, song, pause, and results workflows are not mouse-clickable | Limitation, not blocker | Use keyboard |
| Normal Play Speed can desynchronize audio and chart | Behavioral hazard | Keep Play Speed at 1.0x; use practice tempo for training |
| Result persistence is attempted only when Results is exited | Data-loss risk | Return to song selection before closing; no visible confirmation proves success |
| Actual BGA images and video are unavailable during play | Presentation limitation | Use gameplay/HUD feedback without relying on chart media |

### Keyboard/mouse success criteria

- A new player imports a song and starts play without leaving the game for
  instructions.
- A returning player reaches a remembered or searched song quickly.
- The player can pause, retry, quit, and practice without changing posture or
  input device.
- The player understands when result persistence is attempted and that success
  is not visibly confirmed.
- The player does not confuse normal Play Speed with practice tempo.

## 6. Player Type 2: Nearby MIDI Kit With Computer Access

### Context

The player primarily wants to perform on an electronic drum kit. The computer,
keyboard, and mouse are near enough to reach before or during the session. The
player may use the kit for common navigation and the computer for detailed
actions.

Possible motivations:

- Use DTXManiaRS as a rhythm game with realistic limb movement.
- Practice timing and difficult drum phrases.
- Prefer pad navigation but accept occasional keyboard/mouse use.
- Tune offsets for the audio interface, drum module, and display.

Possible concerns:

- Whether the intended MIDI device is connected.
- Whether velocity threshold hides soft hits.
- Whether pad sounds and chart sounds match physical expectations.
- How often hands must leave the kit for computer controls.

### Core story

> As a MIDI drummer with the computer nearby, I want the kit to handle frequent
> play actions while keyboard and mouse remain an efficient fallback for setup
> and advanced control.

### First-time setup journey

| Stage | Player intent | Current action/path | Outcome | Research focus |
|---|---|---|---|---|
| Readiness | Begin with a connected, mapped kit | Press `F1`, click Controls then MIDI; select/rescan a port, strike pads to inspect activity, click threshold arrows, and inspect note chips | Connection, port, velocity, threshold, mapping, and profiles are visible | Can the player distinguish disconnected, below-threshold, and unbound states? |
| Map a pad | Assign or correct a note | Click `+` on a channel, hit the pad, review note/velocity, choose Add shared or Move here if needed, then hit it again, press `Enter`, or click Confirm | MIDI draft updates without stealing unless Move here is chosen | Does the two-hit shortcut feel intentional or like a double trigger? |
| Save the setup | Reuse it next session | Click Save As for a built-in, type a name, and submit; later click Save after edits | User MIDI profile becomes active if the registry write succeeds | Is the separation between MIDI and Keyboard profiles understood? |
| Verify play input | Confirm prepared pads produce gameplay input | Strike mapped pads during a readiness check or chart | Hits above the prepared threshold produce gameplay input | Can the player recognize loss of readiness from routine game feedback? |
| Calibrate | Align physical strike and judgment | Press keyboard `F1`, click `Calibrate`, hit configured pads 12 times, then press `Enter` to apply or `Esc` to cancel | Suggested input offset can be applied | Does a pad drummer produce stable enough samples? |
| Learn menu grammar | Navigate without immediately reaching for keyboard | Hit HH/CY to move, BD to enter/confirm, SD to go back, and FT to start practice at difficulty level | Title, song wheel, difficulty, play, practice, results, and open pause are partially navigable | Are lane-to-command associations memorable? |
| Load chart | Wait or correct a mistaken selection | Observe progress; reach keyboard `Esc` to cancel | Failure returns automatically; kit has no cancel action | Does the fallback occur before the player settles into play posture? |
| First play | Play primarily from the kit | Hit HH/CY to choose song, BD to enter difficulty, HH/CY to choose difficulty, BD to start, then hit mapped pads at the judgment line | Normal drum gameplay is reachable | Are pad/audio/judgment responses perceived as simultaneous? |
| Recover | Handle first failure or interruption | Open Pause; press `Up`/`Down` and `Enter`, or hit HH/CY and BD, to choose Restart Song or Return to Song Select; hit SD to resume directly | Restart and return are available | Does reaching away feel acceptable or disruptive? |

### First-time acceptance stories

- **NM-F1:** As a new MIDI player, I want a clear ready/not-ready checkpoint so
  I do not begin the evaluated play journey with an unusable kit.
- **NM-F2:** As a new drummer, I want pad-menu commands shown in context so I
  can learn them without memorizing a manual.
- **NM-F3:** As a drummer, I want calibration to accept real pad hits and show a
  trustworthy result before my first scored play.
- **NM-F4:** As a player with computer access, I want keyboard/mouse fallback to
  remain available whenever pad navigation does not cover an action.
- **NM-F5:** As a MIDI player, I want below-threshold hits and source conflicts
  explained before commit so setup mistakes do not appear as gameplay misses.

### Returning normal-play journey

| Stage | Typical behavior | Current support | Friction hypothesis |
|---|---|---|---|
| Launch | Sit at kit and start | Bass drum advances title | Direct if MIDI reconnects before input |
| Browse songs | Use kit for repeated navigation | HH/CY move the song wheel | Efficient after convention is learned |
| Enter difficulty | Commit to selected song | BD changes from wheel to difficulty level | Two-level structure prevents accidental start but adds a step |
| Choose difficulty | Compare chart choices | HH/CY move difficulty; SD returns | Kit-accessible |
| Start | Play or practice | BD starts play; FT starts practice | Kit-accessible |
| Perform | Stay focused on kit | Mapped pads drive lanes and sounds | Primary job is supported |
| Pause | Interrupt play | Reach keyboard and press `Esc` | Computer is nearby, but kit-only flow breaks |
| Operate pause | Resume/retry/quit | Hit HH/CY to move, BD to activate, or SD to resume; keyboard arrows/`Enter`/`Esc` are fallback | Kit works once pause is open |
| Results | Continue back to songs | BD or SD | Kit-accessible |
| Search | Find a named song | Type title/artist characters and use `Backspace` to delete | Keyboard fallback is required |
| Import | Add an archive | Press `F6`, click files, or mouse-drag files onto the window | Keyboard/mouse fallback is required |
| Open settings | Change a routine setting | Press keyboard `F1`; after opening, use pad navigation for Gameplay/Audio/Drums/System | Opening requires fallback; four settings tabs are pad-operable |

### Returning practice journey

The player can start practice from the kit with floor tom. Setup, Settings,
Progress, preview transport, and Pause consume the shared pad-navigation verbs;
physical-kit behavior remains a manual research item.

| Need | Current path | Device switch |
|---|---|---|
| Open Setup | FT on difficulty selection | None |
| Set A/B loop | Navigate Setup controls, or use `[` / `]` or mouse while running | None in wired pad navigation; keyboard/mouse fallback remains |
| Change tempo | Navigate Setup/Settings controls, or press `-`/`=` while running | None in wired pad navigation; keyboard fallback remains |
| Restart section | `R` | Kit to keyboard |
| Inspect completed Progress | Navigate to Progress in Setup/Settings | None in wired pad navigation |
| Seek a section | Navigate preview transport; mouse timeline remains available | None for bar transport; mouse for direct dragging |
| Configure Off/Wait/Ramp | Navigate Trainer and Ramp rows in Setup/Settings | None in wired pad navigation |
| Resume exactly | Open Pause with a configured system Pause binding and choose Resume | No device switch after binding |
| Continue after editing | Choose Continue Practice in Settings | None in wired pad navigation |
| Exit practice | Open Pause and choose Exit to Song Select | No device switch after binding |

### Nearby-MIDI blockers and workarounds

| Issue | Classification | Workaround |
|---|---|---|
| System Pause is unbound by default | Setup requirement | Assign a kit note in the MIDI profile or press keyboard `Esc` |
| Physical-kit practice navigation is not live-verified | Research gap | Test with representative modules/interfaces |
| Search, sort, import, and opening settings are not pad workflows | Friction | Use nearby keyboard/mouse; four settings tabs accept pads after opening |
| Physical reconnect and latency behavior is not live-verified | Research gap | Test with representative modules/interfaces |

### Nearby-MIDI success criteria

- The player can tell whether a pad is connected, mapped, and above threshold.
- The player completes routine browse-play-results loops mostly from the kit.
- Reaching for keyboard/mouse is infrequent and occurs at natural stopping
  points rather than during performance.
- Calibration produces a result the player trusts.
- Practice device switching does not cause the player to lose the selected
  section or tempo context.

## 7. Player Type 3: Distant Computer, MIDI Kit in Reach

### Context

The player is seated at a drum kit while the computer display is visible but
the keyboard and mouse are out of reach. Reaching the computer may require
standing up, leaning away from the kit, or walking across the room.

This is not simply Player Type 2 with fewer conveniences. Physical distance
changes whether a missing pad command is minor friction or a task blocker.

Possible motivations:

- Use a television, projector, or distant monitor with a permanent drum setup.
- Avoid placing computer equipment beside moving sticks and pedals.
- Run long play sessions entirely from the drum throne.
- Use DTXManiaRS primarily as an electronic-drum practice system.

Possible concerns:

- Becoming trapped in a screen that needs keyboard/mouse.
- Being unable to pause quickly for a real-world interruption.
- Starting practice but being unable to define or adjust the exercise.
- Small text or subtle state changes at viewing distance.

### Core story

> As a drummer whose computer is out of reach, I want every routine session
> action available from the kit so I can remain seated and recover safely from
> interruptions or mistakes.

### First-time setup journey

First-time configuration cannot reasonably be completed from the distant kit
alone. The player must temporarily access the computer or receive help.

| Stage | Required behavior | Current reality | Classification |
|---|---|---|---|
| Add songs | Import or locate content | Press `F6` and click one or more archives, or mouse-drag archives onto the window | Computer-access setup task |
| Configure input | Establish working mappings/device | At the computer, open Controls, click MIDI, choose/rescan port, adjust threshold, capture pads, resolve shared/move conflicts, and save a user profile | Computer-access setup task; not operable from distant kit alone |
| Configure lanes | Establish readable order/width | At the computer, open Lanes, select or create a profile, use mouse preview/detail actions, and save | Computer-access setup task; mouse required |
| Calibrate | Start and accept calibration | Press `F1`, click `Calibrate`, hit pads 12 times, then press `Enter` to apply or `Esc` to cancel | Computer-access setup task |
| Verify distant readability | Read title, wheel, difficulty, HUD, pause, results | Not verified at real room distance | Research gap |
| Learn pad commands | Associate pads with menu verbs | Contextual legends appear when MIDI is connected | Potentially kit-accessible |
| Move to kit | Begin the no-computer-reach portion | All prerequisites must already be correct | Critical transition point |

### First-time acceptance stories

- **DM-F1:** As a distant-computer player, I want a clear readiness checkpoint
  before I sit at the kit so I do not discover configuration problems after the
  computer is out of reach.
- **DM-F2:** As a player viewing from a distance, I want connected state and
  navigation commands readable from the drum throne.
- **DM-F3:** As a first-time player, I want to know which workflows will still
  require the computer before I begin a kit-only session.
- **DM-F4:** As a drummer, I want to recover from a wrong selection using the
  snare Back action without leaving the kit.

### Returning normal-play journey

| Stage | Kit-only action | Current outcome | Reachability |
|---|---|---|---|
| Start from title | BD | Opens song selection | Reachable |
| Browse songs | HH/CY | Previous/next song | Reachable |
| Enter difficulties | BD | Changes navigation level | Reachable |
| Choose difficulty | HH/CY | Previous/next difficulty | Reachable |
| Correct wrong song | SD | Returns to song wheel | Reachable |
| Start normal play | BD | Performance begins | Reachable |
| Cancel loading | No pad action | Loading continues unless it fails or computer input intervenes | **Blocker** |
| Play | Mapped pads | Notes are judged | Reachable |
| Pause during play | Configured system Pause note | Performance freezes and Pause opens | Reachable after setup-time binding |
| Resume/restart/quit after pause is open | HH/CY/BD/SD | Pause menu works | Reachable |
| Leave results | BD or SD | Returns to songs and triggers a save attempt | Reachable; save failure is silent |
| Return to title | SD from song wheel | Title appears | Reachable |
| Quit application | No kit action on title | Requires keyboard `Esc` | **Blocker** |

### Interruption story

> As a drummer away from the computer, I want to pause immediately from a pad
> so an interruption does not force me to abandon the chart or allow the song
> to continue unattended.

Current sequence:

1. An interruption occurs during performance.
2. No pad navigation context exists while performance is running.
3. The player must leave the kit and press keyboard `Esc`, ask another person,
   or allow the song to continue.
4. Once paused, the kit can operate Resume, Restart Song, Practice This Section,
   Quick Settings, and Return to Song Select.

This is a current blocker for a self-contained distant-kit session.

### Failure and retry story

> As a distant-kit player, I want to retry or return after a failed chart so I
> can continue the session without walking to the computer.

Current behavior:

- Stage failure advances through the banner to Results.
- BD or SD leaves Results and triggers a save attempt.
- The player can select the song again and restart from the kit.
- There is no direct Retry action on Results.

The recovery is reachable but requires a full Results-to-song-selection cycle.

### Returning practice journey

| Practice need | Kit-only support | Reachability |
|---|---|---|
| Start practice | FT on difficulty level | Reachable |
| Configure Setup and start | Shared pad navigation | Wired; physical kit unverified |
| Play whole-song or A/B loop | Normal mapped pads | Reachable after Setup |
| Change loop, tempo, or Off/Wait/Ramp | Practice Setup/Settings navigation | Wired; physical kit unverified |
| Restart section | Bound system Restart or Pause menu | Reachable after binding |
| Inspect completed Progress | Progress tab navigation | Wired; physical kit unverified |
| Seek preview | Previous/next bar transport | Wired; direct timeline drag still needs mouse |
| Pause and resume exactly | Bound system Pause, then Resume | Reachable after binding |
| Exit practice | Bound system Pause, then Exit to Song Select | Reachable after binding |

Starting practice from floor tom now enters mandatory Setup rather than an
already-running whole-song loop. A complete distant-kit workflow depends on
configured system Pause/Restart bindings. Its source/test reachability is known,
but it still requires physical-kit and room-distance validation.

### Distant-MIDI blockers and workarounds

| Blocker | Workaround | Behavioral cost |
|---|---|---|
| Pause/Restart notes are unbound by default | Configure them before moving away from the computer | Requires setup-time device knowledge |
| Direct timeline dragging requires a mouse | Use pad previous/next-bar transport | Less precise than pointer seeking |
| Physical practice navigation is not live-verified | Keep wireless keyboard/mouse nearby during validation | Temporary fallback until representative-kit checks pass |
| Cannot quit from title with kit | Walk to computer or close application remotely | Session cannot be completed kit-only |
| Cannot search/sort/import from kit | Pre-plan library choices or use computer | Limits spontaneous song selection |
| Cannot visually verify distant readability from source review | Conduct room-distance testing | Unknown until observed |

### Distant-MIDI success criteria

- After one computer-access setup phase, a returning player completes an entire
  routine session from the kit.
- The player can pause immediately during active performance.
- The player can retry, quit a song, leave results, return to title, and exit
  the game without another person.
- Practice supports section selection, tempo adjustment, restart, feedback, and
  exit from the kit.
- Essential text and selection state remain readable at realistic room distance.

## 8. Cross-persona reachability matrix

Legend:

- **Yes**: directly reachable with the player's primary available input.
- **Fallback**: reachable using a secondary nearby input.
- **No**: not reachable in the stated physical setup.
- **Setup**: expected to be completed before the kit-only portion.

| Routine action | Keyboard/mouse | Nearby MIDI + computer | Distant MIDI |
|---|---|---|---|
| Advance title | Yes | Yes, kit | Yes, kit |
| Browse songs | Yes | Yes, kit | Yes, kit |
| Select difficulty | Yes | Yes, kit | Yes, kit |
| Search songs | Yes | Fallback | No |
| Change sort | Yes | Fallback | No |
| Import archives | Yes | Fallback | No |
| Start normal play | Yes | Yes, kit | Yes, kit |
| Start practice | Yes | Yes, kit | Yes, kit |
| Cancel loading | Yes, `Esc` | Keyboard fallback | No |
| Play drum notes | Yes | Yes, kit | Yes, kit |
| Open pause during play | Yes | Yes after system binding | Yes after system binding |
| Operate an open pause menu | Yes | Yes, kit | Yes, kit |
| Leave results and trigger save attempt | Yes | Yes, kit | Yes, kit |
| Define practice A/B | Yes | Yes, pad navigation | Yes, pad navigation |
| Change practice tempo | Yes | Yes, pad navigation | Yes, pad navigation |
| Inspect practice feedback | Yes | Yes, Progress navigation | Yes, Progress navigation |
| Exit practice normally | Yes | Yes after system binding | Yes after system binding |
| Calibrate | Yes | Fallback plus kit taps | Setup only |
| Change Gameplay/Audio/Drums/System after editor opens | Yes | Yes, pads or fallback | Setup only |
| Edit Keyboard/MIDI profiles and bindings | Yes, mouse-led | Computer fallback plus pad capture | Setup only |
| Edit lane profiles/order/width/channels | Yes, mouse required | Computer fallback | Setup only |
| Customize HUD | Yes | Fallback | Setup only |
| Skip clear/fail banner | Yes | Keyboard fallback | No, wait for auto-advance |
| Confirm result save success | No visible confirmation | No visible confirmation | No visible confirmation |
| Quit from title | Yes | Fallback | No |

### Exact player-action inventory

This inventory states what the player must physically do in each current
journey, including the redesigned Controls and Lanes workflows.

#### Controls and profile setup

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Open Controls | Press `F1`, then click Controls | No pad action to open/enter it |
| Choose source/profile | Click Keyboard or MIDI; click profile name and a dropdown item | No pad action |
| Inspect a binding | Click channel row or hover a shared chip | Hit an already mapped pad to select its channel |
| Add keyboard source | Click Keyboard and channel `+`; press key; use `Left`/`Right` or click shared/move choice; press `Enter` or click Confirm | No pad action |
| Add MIDI source | Click MIDI and channel `+`; hit pad; use keyboard/mouse for shared/move choice; press `Enter`/click Confirm | Hit candidate; hit same note again to confirm, or a different note to replace it |
| Cancel capture | Press `Esc` or click Cancel | No dedicated cancel hit |
| Remove one source claim | Click chip `x` | No pad action |
| Choose/rescan port | Click port arrows or Rescan | Strike pads only to verify input |
| Change threshold | Click the threshold previous/next buttons; each click changes it by one | Strike pads to observe velocity/below-threshold feedback |
| Reset bindings/device | Click Reset tab, then Confirm reset; this currently affects both source types and device fields | No pad action |
| Save current user profile | Click Save | No pad action |
| Create/rename profile | Click Save As or overflow Rename; type/edit name; press `Enter` or click OK | No pad action |
| Revert/delete user profile | Click overflow Revert/Delete and complete dialog | No pad action |
| Resolve dirty switch | Click Cancel, Discard, or Save in the guard | No pad action |

Controls is not a keyboard-only or pad-only surface: mouse clicks are required
to enter the custom panel controls. Keyboard participates in capture, choice,
confirmation, and profile-name entry; a MIDI pad participates only while MIDI
capture or input inspection is active.

#### Lane profiles and arrangement

| Intent | Mouse action | Keyboard/MIDI action |
|---|---|---|
| Open/select lane profile | Click Lanes, profile name, then a dropdown item | No wired keyboard/pad content navigation |
| Select lane | Click row or playfield preview pad | No wired action |
| Reorder lane | Drag preview pad body horizontally and release near target | No wired action |
| Resize lane | Drag preview pad edge or detail Width slider | No wired action |
| Merge channel | Click `+ add`, then click an available channel | No wired action |
| Split secondary | Click its `x` chip | No wired action |
| Hide/restore | Click Hide lane or a channel in Hidden | No wired action |
| Undo/redo edit | Not required | Press `Ctrl+Z` / `Ctrl+Y` |
| Save/manage profile | Click Save, Save As, selector, or overflow actions and complete dialogs | Keyboard can type/submit/cancel only after a mouse opens a dialog |

The Lanes profile and preview update immediately as a draft. A distant-kit
player must complete this entire workflow during computer-access setup.

#### Startup, title, and exit

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Wait for startup | No input; wait about half a second | No input |
| Enter song selection | Press `Enter` | Hit bass drum |
| Open Gameplay settings | Press `F1`; with no playable chart, nothing opens | No pad action |
| Open layout/Widgets editor | Press `F2`; with no playable chart, nothing opens | No pad action |
| Quit from title | Press `Esc` | No pad action |
| Finish quitting | No input; wait on `Thanks for playing` | No input |

The title itself is not mouse-clickable for Continue or Quit.

#### Song selection and import

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Previous/next song | Press `Up`/`Down` | At wheel level, hit hi-hat/cymbal or ride |
| Enter difficulty level | Not required; keyboard is flat | Hit bass drum on wheel level |
| Previous/next difficulty | Press `Left`/`Right` | At difficulty level, hit hi-hat/cymbal or ride |
| Start normal play | Press `Enter` | Hit bass drum at difficulty level |
| Start practice | Press `Shift+Enter` | Hit floor tom at difficulty level |
| Back from difficulty | Not applicable | Hit snare |
| Back to title | Press `Esc` | Hit snare at wheel level |
| Search | Type title/artist characters; press `Backspace` to delete | No pad action |
| Change sort | Press `Tab` | No pad action |
| Rescan library | Press `F5` | No pad action |
| Open settings for selected chart | Press `F1` | No pad action |
| Open archive picker | Press `F6`, then click/select one or more files | No pad action |
| Drop an archive/file | Mouse-drag file onto game window | No pad action |

The visible song wheel is not mouse-clickable for routine browse/start actions.

#### Loading

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Wait for loading | No input | No input |
| Cancel loading | Press `Esc` | No pad action |
| Recover from loading failure | No input; game returns automatically | No input |

#### Normal performance and banner

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Play a note | Press its configured drum key | Hit its mapped pad above threshold |
| Open pause | Press `Esc` | Hit the configured system Pause note; unbound by default |
| Increase/decrease scroll speed | Press `Up`/`Down` | No pad action |
| Adjust input offset | Press `Left`/`Right`; hold `Ctrl` for fine adjustment | No pad action |
| Adjust per-song BGM offset | Press `Shift+Up`/`Shift+Down`; add `Ctrl` for fine adjustment | No pad action |
| Toggle performance information | Press `F11` | No pad action |
| Skip clear/fail banner | Press `Enter` or `Space` | No documented pad action |
| Let banner continue | No input; wait about 1.6 seconds | No input |

#### Normal pause and Results

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Move pause selection | Press `Up`/`Down` | Hit hi-hat/cymbal or ride |
| Activate Resume/Restart/Settings/Exit actions | Press `Enter` or `Space` | Hit bass drum |
| Resume directly | Press `Esc` | Hit snare after pause is open |
| Leave Results | Press `Enter` or `Esc` | Hit bass drum or snare |
| Trigger result save attempt | Leave Results using an action above | Leave Results using an action above |

Main pause and Results rows are not mouse-clickable.

#### Practice while playing

| Intent | Keyboard/mouse action | MIDI action |
|---|---|---|
| Set loop A/B | Press `[` / `]` | No pad action |
| Clear loop | Press `Backspace` | No pad action |
| Tempo down/up | Press `-` / `=` | No pad action |
| Restart section | Press `R` | No pad action |
| Toggle ramp | Press `T` | No pad action |
| Open Practice Settings | Press `Tab` | Navigate through Practice Pause, or use keyboard fallback |
| Open Practice Pause | Press `Esc` | Hit the configured system Pause note |
| Continue playing notes | Press configured drum keys | Hit mapped pads |

#### Practice Setup, Settings, and Progress

| Intent | Keyboard action | Mouse action | MIDI action |
|---|---|---|---|
| Move setting selection | Press `Up`/`Down` | Click a setting row | Hit hi-hat/cymbal or ride |
| Change selected value | Press `Left`/`Right` | Click its adjustment control | Use decrement/increment navigation verbs |
| Activate selected action | Press `Enter` or `Space` | Click the action | Hit bass drum |
| Switch Setup/Progress/Preview | Press `Tab` or directional navigation according to layout | Click a tab | Use shared navigation verbs |
| Seek | Use Preview previous/next bar or the timeline | Click timeline | Focus previous/next-bar transport and activate |
| Create A/B span | Adjust loop start/end rows | Mouse-drag across timeline | Navigate loop rows and adjust |
| Play/pause non-judged preview | Focus Preview transport and activate | Click preview play/pause | Focus preview transport and activate |
| Start or continue from pre-roll | Activate Start Practice or Continue Practice | Click the primary action | Navigate to the primary action and confirm |
| Save/update/delete a loop | Activate the explicit preset rows; confirm Delete | Click the corresponding action | Navigate to the preset action and confirm |
| Return from Settings to Pause | Press `Esc` | Use the Back action | Hit snare/back |

#### Gameplay, Audio, Drums, and System settings

| Intent | Keyboard action | Mouse action | MIDI action after editor is open |
|---|---|---|---|
| Change tab | Press `PageUp`/`PageDown` or use tab arrows | Click tab | At tab bar, hit hi-hat/cymbal or ride |
| Enter settings rows | Press `Down` or `Enter` from tab bar | Click/drag a row control directly | Hit bass drum |
| Move row | Press `Up`/`Down` | Click desired row control | Hit hi-hat/cymbal or ride |
| Change value | Press `Left`/`Right`; hold `Shift` for coarse change | Click stepper/toggle or drag slider | Hit BD to enter adjust, HH/CY to change, BD to keep |
| Cancel current pad adjustment | Not applicable | Not applicable | Hit snare to restore saved value |
| Reset settings tab | No dedicated shortcut | Click `RESET TAB` | No pad content action |
| Start calibration | No keyboard activation action | Click `Calibrate` | Pad can provide the 12 taps only after the mouse click |
| Apply/cancel calibration | Press `Enter`/`Esc` | No apply/cancel mouse action documented | No pad apply/cancel action |
| Close settings | Press `Esc` | No mouse close action documented | From tab bar, hit snare |

#### Widgets editor

| Intent | Keyboard action | Mouse action | MIDI action |
|---|---|---|---|
| Select widget | No keyboard selection action | Click widget in list or on canvas | No pad action |
| Move widget | Press arrows; hold `Shift` for 8-pixel nudge | Left-drag widget | No pad action |
| Resize widget | No keyboard resize action documented | Drag resize corner or drag the Scale inspector control | No pad action |
| Cycle overlapping widgets | Hold `Alt` and click | `Alt`+click | No pad action |
| Set anchor/auto | No keyboard anchor/auto action | Click anchor cell or `auto` | No pad action |
| Change numeric/visibility values | No keyboard inspector-value action documented | Click steppers/toggles or drag controls | No pad action |
| Reset selected widget | No dedicated shortcut | Click `Reset Widget` | No pad action |
| Undo/redo | Press `Ctrl+Z`; `Ctrl+Y` or `Ctrl+Shift+Z` | No dedicated visible action documented | No pad action |
| Save layout | Press `Ctrl+S`; pressing `Esc` to close also triggers a save attempt | No independent mouse save/close action | No pad action |
| Deselect then close | Press `Esc` once to deselect, again to close | No mouse close action | No pad action |

## 9. Journey handoff costs

A handoff is a switch from the player's primary play device to another device.
The same software action has different cost by physical context.

| Handoff | Keyboard/mouse | Nearby MIDI | Distant MIDI |
|---|---|---|---|
| Configure a system Pause/Restart note | Not applicable | Setup-time interruption | Must be completed before the distant session |
| Kit to mouse for direct timeline dragging | Not applicable | Optional advanced action | Pad bar transport remains available |
| Kit to keyboard for search | Not applicable | Occasional fallback | Unavailable during seated session |
| Keyboard to mouse in Customize | Low cost | Setup-time cost | Requires leaving kit |
| Results to song wheel | Same device | Same kit grammar | Same kit grammar |

Research should measure handoff frequency and physical effort, not merely count
the number of available actions.

## 10. Behavioral hypotheses

These statements are not findings. They should be tested.

### H1: Main-menu mouse absence affects discovery more than efficiency

Returning keyboard users may navigate quickly after learning the keys, while
first-time users may repeatedly attempt to click visible song or menu elements.

### H2: Nearby MIDI players tolerate fallback at natural breaks

They may accept keyboard/mouse for import, device setup, and direct timeline
dragging, but reject reaching away during live performance.

### H3: Pad navigation is learnable when legends change with context

The HH/CY/BD/SD/FT grammar may become efficient after a few sessions, but BD's
meaning change between song wheel and difficulty selection must be observed.

### H4: Distant players interpret floor-tom Practice as a complete promise

Because practice can be started from the kit, players may expect Setup,
section, tempo, restart, Progress, and exit actions to remain understandable at
room distance. This needs physical-kit and readability testing.

### H5: Save-on-leaving-Results is not visible enough

Players may close the game while admiring or recording a result, losing the
play without knowing that leaving Results was required.

### H6: Calibration confidence differs by input type

Keyboard taps and drum-pad strikes may produce different sample spread. A
single median with no confidence indication may be trusted differently by the
two groups.

### H7: Viewing distance changes information usefulness

Song density, history, difficulty details, practice diagnosis, and HUD widgets
may be useful at a desk but unreadable or cognitively expensive from a drum
throne.

## 11. Research sessions

### Session A: First-time keyboard/mouse

Ask the participant to:

1. Launch the game, wait for the startup splash, then press `Enter` on title.
2. Press `F6`, mouse-select one valid archive, then repeat with one invalid or
   duplicate archive and respond to its notification.
3. Press `Up`/`Down` or type to find a named song, then press `Left`/`Right` for
   the requested difficulty.
4. Start loading the wrong chart and cancel with `Esc`.
5. Press `F1`, click `Calibrate`, press configured drum keys 12 times, then
   press `Enter` to apply.
6. Click the Audio and System tabs, click/drag one control in each, then press
   `Esc` to close Customize.
7. Press `Enter` to start and press configured drum keys to complete or fail one
   play.
8. Press `Enter` or `Esc` to leave Results, then inspect whether the result
   appears in history.
9. Press `Shift+Enter`, verify Setup preview is stopped, configure a section and
   Off/Wait/Ramp, start from pre-roll, open Settings with `Tab`, continue from
   pre-roll, then press `Esc` and choose Exit to Song Select.
10. Return to title with `Esc`, press `Esc` again, and wait for application exit.

Observe attempted mouse clicks, shortcut discovery, mapping comprehension,
offset language, result-saving assumptions, and practice discoverability.

### Session B: First-time nearby MIDI

Ask the participant to:

1. Begin from a declared ready state with the intended kit configured.
2. Press keyboard `F1`, click `Calibrate`, hit configured pads 12 times, then
   press `Enter` to apply.
3. Press `Esc` to close Customize, hit BD on title, hit HH/CY to move songs, BD
   to enter difficulty, HH/CY to move difficulties, and SD to return to songs.
4. Press keyboard `F1`; hit HH/CY on the tab bar to select Audio, hit BD to
   enter rows, HH/CY to select a volume, BD to adjust, HH/CY to change it, BD
   to keep it, SD to return to the tab bar, and SD again to close.
5. From title, hit BD, use HH/CY to choose the wrong song, hit BD to enter its
   difficulties, hit BD to start loading, then press keyboard `Esc` to cancel.
6. Use HH/CY to choose a song, hit BD to enter difficulty and BD again to start,
   press keyboard `Esc`, use HH/CY to select Restart Song, hit BD to activate it, then
   hit BD or SD to leave Results.
7. Use HH/CY to choose a song, hit BD to enter difficulty, hit FT to open
   Practice Setup, configure a two-bar section and 0.75x tempo with pad
   navigation, choose Off, Wait, or Ramp, then start from pre-roll.

Observe readiness assumptions, pad-command learning, computer handoffs,
calibration confidence, and whether practice remains kit-centered. Do not
evaluate the current or replacement Controls/Lanes configuration UI.

### Session C: Returning distant MIDI

Prepare the game and kit, place keyboard/mouse out of reach, then ask the
participant to:

1. Hit BD on title.
2. Hit HH/CY to choose the named chart, BD to enter difficulties, HH/CY to
   choose difficulty, and BD to start.
3. Start loading the wrong chart and attempt to cancel it; explicitly record
   that no pad action is available.
4. Respond to a simulated interruption with the configured system Pause note;
   verify Pause opens and Resume returns to the exact position.
5. After failure, hit BD or SD to leave Results, navigate back with HH/CY and
   BD, and hit BD to retry.
6. Hit FT to enter stopped Practice Setup, set the requested section and slower
   tempo using pads, start, then open Settings and inspect completed Progress.
7. Open Pause with the configured system binding and exit Practice using pads.
8. Return to title with SD from the song wheel, then attempt to quit using pads;
   record that no pad action exists.

Do not rescue the participant immediately. Record the first attempted pad,
time-to-action, whether they leave the throne, and whether system bindings,
Setup state, and preview state remain understandable.

### Session D: First-time distant readiness

Allow computer access initially, then move the participant to the kit. Ask the
participant to:

1. Press `F6` and mouse-select a provided archive, or mouse-drag it onto the
   window.
2. Reach a declared ready state without evaluating the Controls/Lanes UI.
3. Press `F1`, click `Calibrate`, hit 12 pad samples, and press `Enter` to apply.
4. Confirm title, song, difficulty, HUD, and navigation legends are readable
   from the intended drum-throne distance.
5. State which actions they believe will remain possible after the computer is
   moved out of reach.
6. Begin the kit-only journey and record the first incorrect expectation.

Observe the readiness handoff, what the player checks before leaving the
computer, distant readability, and mismatch between expected and actual kit
reachability.

### Session E: Returning-player efficiency

Run once with a returning keyboard/mouse player and once with a returning nearby
MIDI player. Ask each participant to:

1. Press the required title action (`Enter` or bass drum).
2. Use remembered selection, then type a different song title and press
   `Backspace` if correction is needed.
3. Press `Enter` or hit BD to start, press `Up`/`Down` once for scroll speed,
   press keyboard `Esc`, select Restart Song with arrows or HH/CY, and activate with
   `Enter`/BD.
4. Press `Enter`/`Esc` or hit BD/SD to leave Results, then inspect history.
5. Press `Shift+Enter` or hit FT, configure and start an A/B span, press `R`,
   press `Tab` to open Settings, continue from pre-roll, then press `Esc` and
   choose Exit to Song Select.
6. Press `F1`, click Audio, click/drag a BGM or drum volume, close with `Esc` or
   SD from the tab bar, and press `F11` during the next play.
7. Press `Esc` or hit SD to return to title, then press keyboard `Esc` to quit.

Measure time, incorrect actions, device handoffs, help requests, and whether
each handoff occurs during performance or at a natural break.

## 12. Interview questions

Ask after behavior has been observed:

1. Which actions did you expect to perform with your primary device?
2. When did you first feel unsure what the game was waiting for?
3. Which device switches felt natural, and which interrupted the session?
4. When did you believe your score had been saved?
5. What did you expect Practice to let you control?
6. Which song-selection information influenced your choice?
7. Which feedback could you read without leaning toward the display?
8. What would make you stop a session rather than continue?
9. Which recovery action must always be immediately available?
10. After one session, which pad-menu commands can you recall unaided?
11. What did you think happened when loading returned to the song wheel?
12. How would you know that a settings or result save failed?
13. What did ramp promotion, wait flow, and the second Exit confirmation mean
    to you?
14. Which exact key, click, or pad hit did you expect for the first blocked
    action?

## 13. Evidence needed before UX verdicts

The following evidence is still required before declaring the design good or
bad for any player type:

- Live screenshots and interaction recordings at desktop and room distance
- Physical tests with at least two MIDI drum modules
- Pad velocity, reconnect, and duplicate-trigger observations
- Audio/input synchronization at representative hardware latency
- First-time discovery tests without coaching
- Returning-player timing for common journeys
- Frequency and physical cost of device handoffs
- Practice completion and escape behavior from a distant kit
- Confirmation of save behavior under normal exit and forced application close
- OS-specific archive picker and drag/drop tests
- Loading cancel/failure tests for keyboard, nearby-kit, and distant-kit setups
- MIDI disconnect/reconnect during title, browsing, loading, play, pause, and
  Results
- Soft rolls, pedal double-triggers, hi-hat openness/controller behavior, and
  crosstalk on physical kits
- Keyboard rollover during realistic multi-limb chord patterns
- Corrupt/missing settings, score history, and unwritable save locations
- Audio interfaces with different buffer sizes and output-device changes
- Practice wait/ramp comprehension and pitch-change tolerance

## 14. Neutral baseline summary

The current product provides a complete routine play loop for keyboard/mouse
and a mostly kit-driven browse-play-results loop for MIDI. A nearby computer
turns missing kit actions into fallbacks. Physical distance turns several of
the same gaps into blockers, especially opening pause, controlling practice,
and quitting the application.

This is a reachability summary, not a usability verdict. The hypotheses and
research sessions above must be tested before redesign priorities are assigned.
