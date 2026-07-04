# Menu UI Redesign — GITADORA Layout × osu Smoothness

Date: 2026-07-05
Status: Approved

## Goal

Replace the bare menu UI (title, song select, settings, song loading) with a
GITADORA-style visual language — black stage, drifting multicolor streaks,
yellow selection, bold italic skill numbers — animated with osu!lazer-grade
motion. Everything rendered procedurally: no image or font assets are added.

Mockups from the brainstorm session live in `.superpowers/brainstorm/1333996-1783187662/content/`
(`song-select-layout-v2.html`, `other-screens.html` are the approved versions).

## Decisions

- **Style**: GITADORA Energy — black stage `#050505`, animated diagonal
  gradient streaks, yellow `#ffcc00` selection outline/highlight, full
  GITADORA palette. Per-difficulty colors: BASIC blue `#0088ff`, ADVANCED
  yellow `#ffcc00`, EXTREME red `#ff4444`, MASTER purple `#cc33ff`.
- **osu mash-ins**: ambient album-art background tint (crossfades on
  selection), type-to-search in song select, difficulty chips on the selected
  wheel row, settings screen with left section rail + description panel.
- **Assets**: procedural only. System font (existing FontSource::SansSerif),
  bold weight for numbers; italic only if the system font provides it.
- **Input**: keyboard only. No hover/mouse systems.
- **Screens in scope**: Title, Song Select, Settings (renamed from "Config"
  in all user-facing text), Song Loading, plus shared enter/exit
  choreography.

## Architecture

New reusable menu kit in `dtx-ui`, consumed by `game-menu`:

```
crates/dtx-ui/src/
├── theme.rs            extend: stage black, select yellow, streak colors,
│                       4 difficulty colors
├── easing.rs           existing, reuse
├── tween.rs            existing, reuse
├── motion.rs           NEW motion primitives
│     EnterChoreo { delay_ms, from: Offset/Scale/Alpha }   staggered enter
│     ExitChoreo                                           screen leave
│     SpringValue { target, vel, stiffness, damping }      scroll physics
│     RollingNumber { shown, target }                      digit roll
│     BeatPulse { bpm, phase }                             audio-reactive pulse
└── widget/
      stage_background.rs  fullscreen stage: black bg + drifting rotated
                           gradient streaks + ambient album-art layer
      stage_panel.rs       dark panel (#0d0d0dee, 1px #444 border);
                           yellow-glow selected variant
      song_wheel.rs        wheel container + rows: art thumb, skill number +
                           achievement bar, curve indent, selected expansion
      skill_badge.rs       bold number blocks (skill, BPM, length, level)
      density_graph.rs     GITADORA per-lane vertical bars + total notes
      difficulty_grid.rs   4-level grid with rank badge + achievement bar

crates/game-menu/src/
      title.rs         rebuild on kit
      song_select.rs   rebuild on kit
      config.rs        rebuild on kit; user-facing name "Settings"
      song_loading.rs  rebuild on kit
```

Kept as-is: `AppState` machine and 300ms screen fade (choreography layers on
top), preview audio system, folder grouping logic, scores.json persistence.

## Screen layouts

### Song Select (centerpiece)

1280×720 reference, three zones:

- **Top bar**: logo left; search field + `SORT: … ▾` chip right. No GROUP
  dropdown (deferred).
- **Left column (24%)**: album art / BGA preview (yellow-bordered when
  selected song has art), SKILL BY SONG box (big italic number), BPM + length
  box.
- **Center (21%)**: GITADORA density graph — vertical per-lane colored bars,
  START→END bottom-to-top, TOTAL NOTES beneath — beside the difficulty grid:
  four level panels (colored label bar, big level number, achievement % +
  bar + rank badge on played levels; dimmed "no play" otherwise). Selected
  level gets yellow border + glow.
- **Right (50%)**: song wheel. Rows ~11% screen height: art thumbnail,
  ⚡skill number + yellow achievement bar above the title line, artist inline,
  CLEAR badge. Rows indent toward the selection (arc). Selected row expands
  to ~17% height with 3px yellow border, glow, difficulty chips for all
  levels (current marked), achievement % + best score right-aligned.
  Root-level song folders render as gold category boxes (ENTER opens,
  BACKSPACE up); no new grouping modes.
- **Bottom bar**: key hints.
- **Background**: fullscreen ambient layer tinted by selected song art
  (see Technical constraints) under dark overlay, plus drifting streaks.

### Title

Boxed italic logo "DTXMANIARS" (white border, dark fill, glow), no tagline.
Pulsing yellow PRESS ENTER chip. Version bottom-left, ESC QUIT bottom-right.
Streaks drift continuously.

### Settings

Left rail with sections: SYSTEM / GAMEPLAY / AUDIO / DRUMS-KEYS, yellow
active item. Rows right: name left, value right (`◂ ON ▸` arrows, slider +
number for ranges). Selected row yellow border + glow. Description panel
below rows explains the focused setting. All user-facing text says
"Settings".

### Song Loading

Centered hero card: art, NOW LOADING label, title, artist + BPM + difficulty
chip, yellow progress bar bound to real load progress, status line
(e.g. "loading audio chips… 41/64").

## Motion spec

| Event | Animation |
|---|---|
| Screen enter | Staggered: left column slides from left (200ms OutQuint, 30ms stagger), wheel rows cascade from right, top/bottom bars drop/rise; rides on existing 300ms fade |
| Screen exit | Reverse, 150ms |
| Wheel scroll | SpringValue on selection index; rows spring to slots with slight overshoot; held key ramps repeat rate |
| Row select | Expansion lerp 180ms OutQuint; yellow glow pulses via BeatPulse |
| Difficulty ←→ | Grid highlight slides, chips update, numbers roll |
| Selection change | Art crossfade (existing tween), ambient re-tint 400ms, density bars re-grow staggered 20ms/bar, skill/BPM/notes roll |
| Title idle | Streaks drift and wrap; PRESS ENTER pulses ~60bpm, syncs to chart BPM when preview plays |
| Settings tab switch | Old rows exit left, new enter right, 20ms stagger |
| Loading | Card scales in 250ms OutQuint; bar lerps to real progress; on done, card zooms + fade to gameplay |

## Technical constraints

- **Streaks**: Bevy 0.19 `bevy_ui` gradients + `UiTransform` rotation.
  Verify the gradient+rotation combo renders correctly as a plan-stage spike;
  fallback is a sprite layer behind the UI camera.
- **Ambient background**: no backdrop-blur in bevy_ui. Approximate: album
  art fullscreen at low alpha under a dark overlay; downscaled texture with
  linear upscale reads as cheap blur.
- **Fonts**: system discovery only. Bold for numbers; skip italic if
  unavailable.
- **Skill number**: computed from existing scores.json best as
  achievement % × chart level; display-only, formula tunable later.
- **Performance**: streaks are a handful of animated nodes; all tweens
  frame-driven like the existing `ScalarTween`.

## Error handling

- Missing album art (`#PREIMAGE` absent): full black placeholder — no
  gradient fill. Ambient background stays black.
- Empty library: wheel replaced by panel "no songs found — put song folders
  in <dir>, press F5".
- scores.json missing or corrupt: everything renders in "no play" state
  (dashes), no crash.

## Testing

- Unit tests for motion math (spring, rolling number, choreography timing)
  in dtx-ui — pure functions, no Bevy runtime.
- Existing gameplay/mechanics integration tests untouched.
- Visual verification per screen via bevy-brp screenshots.

## Out of scope (deferred)

- GROUP dropdown / extra grouping modes
- Interactive loading screen (osu-style minigame) — noted future idea;
  questionable for drum players away from keyboard
- Skinnable image assets / user skin pipeline
- Mouse input in menus
- Per-song adaptive streak colors sampled from album art
