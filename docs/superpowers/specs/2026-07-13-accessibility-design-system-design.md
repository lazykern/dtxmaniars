# Accessibility and Design-System Consolidation — Design

Date: 2026-07-13
Status: Approved
Program cycle: 6

## Goal

Make DTXManiaRS readable and predictable across desk and drum-kit viewing
distances while giving players independent control over motion, flashes, and
moving backgrounds. Consolidate the UI primitives needed to apply those
choices consistently. No gameplay timing, lane routing, or scoring formula
changes are part of this cycle.

Several findings from the 2026-07-12 audit have already been closed: gameplay
has a visible gauge, results has a real hierarchy and practice handoff, and
kit-reachable system binds have their own design. This cycle targets the
remaining accessibility and consistency gaps rather than reimplementing those
features.

## Product decisions

- Accessibility controls are independent settings, grouped together in
  Customize. There is no preset that silently changes several behaviors.
- Color is never the only signal for focus, selection, destructive actions,
  failure, or modifier state.
- Reduced effects preserve state and timing information. They may remove
  decoration, never feedback meaning.
- The existing full-motion song wheel, transitions, and hit feedback remain
  the default experience.
- No Fail is an assisted modifier. Its results remain visible but do not enter
  ordinary score/PB/skill history.
- Distant-kit control grammar is out of scope. It is owned by the approved
  system-bind design and must not be replaced by gestures here.

## Persisted configuration

Add a version-tolerant `AccessibilityConfig` to `dtx-config::Config`:

```rust
pub struct AccessibilityConfig {
    pub text_scale: TextScale,
    pub reduce_motion: bool,
    pub reduce_flashes: bool,
    pub background_motion: bool,
}

pub enum TextScale {
    Standard, // 1.00
    Large,    // 1.25
    XLarge,   // 1.50
}
```

Defaults reproduce current behavior: Standard text, full motion and flashes,
and moving backgrounds enabled. The entire section uses `#[serde(default)]`,
so existing configuration files migrate without a rewrite or prompt. Values
outside the supported scale set are rejected by deserialization and recover to
the default configuration through the existing config-load policy.

Customize adds an Accessibility group with four independently focusable rows.
Changes participate in the existing draft/save/discard transaction and preview
live. Closing with Discard restores the previous runtime policy.

## Runtime policies

`dtx-ui` owns a derived `AccessibilityPolicy` resource. Game-layer screens read
that policy; they do not repeatedly load configuration files.

### Text scale

Replace ad-hoc player-facing font sizes with semantic roles: Display, Title,
Heading, Body, Label, Hint, and HUD. Each role has a reference size and a
minimum readable size. `TextScale` multiplies those roles, with final sizes
clamped to keep overlays inside their safe regions.

The scale applies to menus, loading, pause, stage banners, results, practice,
legends, notifications, and HUD text. It does not scale notes, hit windows,
lane widths, density geometry, or other mechanics-bearing visuals. Editor
microcopy moves to the same roles, but dense spatial canvases may use Standard
scale when enlarging them would hide the object being edited; in that case the
focused description and error text must still honor the selected scale.

### Reduced motion

When enabled:

- entrance translations, parallax, beat pulses, rolling-number overshoot, and
  spring overshoot become steady placement or opacity-only feedback;
- screen transitions remain as 120 ms OutQuint opacity fades so state changes
  do not become visually abrupt;
- selection changes remain immediate and retain their shape/marker cue;
- timing-driven note scroll, progress, gauge movement, and practice playheads
  are unchanged because they communicate gameplay state.

Motion primitives consume the policy centrally. Individual screens must not
invent separate reduced-motion interpretations.

### Reduced flashes

When enabled, lane/key-cap hits, judgment emphasis, danger feedback, and stage
banners use a stable outline, marker, or low-contrast color hold for 120 ms.
They do not brighten the entire element or oscillate opacity. Fullscreen danger
pulses become a constant low-opacity border. Judgment labels and signed timing
text remain present.

The policy is applied at the shared flash/tween primitives. A regression test
must prove that each affected event still produces a visible state change.

### Background motion

When disabled, decorative parallax and animated menu backgrounds stop. Static
BGA image events continue, but movie playback and BGAPAN/AVIPAN movement are
suppressed. The effective movie capability is:

```text
system.movie_enabled && accessibility.background_motion
```

The existing BGA/movie enable switches remain authoritative media controls;
Background Motion is an accessibility override. Gameplay keeps the lane
backdrop and most recent static image so notes remain readable.

## Explicit No Fail modifier

The current `stage_failed_enabled` and legacy `DamageLevel::None` can both
produce a no-failure run. Customize presents one clear `Fail Mode` row:
Standard or No Fail. Damage severity remains a separate Small/Normal/High row
and is disabled visually while No Fail is active.

For backward compatibility, either `stage_failed_enabled == false` or a legacy
`DamageLevel::None` loads as No Fail. Saving writes the canonical form
(`stage_failed_enabled = false`) while retaining the last non-None damage
severity for later restoration. A legacy config that contains
`DamageLevel::None` but no recoverable prior severity migrates to Small as the
disabled retained severity, matching the current default.

Performance shows a persistent `NO FAIL` badge using text plus a shield marker.
The run context records `NoFail` before stage completion. Results explains
`Not saved: No Fail enabled`; the run does not update ordinary history, PB,
rank records, compatible score.ini data, or player skill. Practice keeps its
existing score-exclusion behavior and does not add a second No Fail badge.

## Semantic visual system

Extend `Theme` into named semantic tokens rather than screen-local colors:

- Focus: accent outline plus directional/focus marker.
- Selected: filled/tinted surface plus check/chevron marker.
- Error/destructive: red plus warning/destructive glyph or label.
- Success: green/cyan plus confirmation marker.
- Disabled: reduced contrast plus unavailable marker where action discovery
  matters.

Yellow may remain a musical/difficulty accent, but it is no longer an
unqualified synonym for selection. Editor-local blue/gold/red focus constants
are migrated to the semantic tokens. Lane and difficulty colors are preserved
because they identify domain data, not generic interaction state.

## Shared components

Add focused primitives in `dtx-ui`:

- `ActionButton`: default, selected, focused, disabled, and destructive states;
  keyboard, pad, and pointer activation all emit the same action.
- `ModalDialog`: focus trap, title/body/actions, default and destructive action,
  keyboard/pad/pointer parity, and explicit cancel behavior.
- `NotificationQueue`: Info, Success, Warning, and Error tones; bounded queue;
  readable lifetime; reduced-motion fade policy; no silent persistence errors.
- `Typography` and `Spacing` roles used by the three primitives and migrated
  player-critical surfaces.

Import and practice notifications migrate to `NotificationQueue`. Editor save
errors may continue to occupy the footer while editing, but they use the same
tone/content model. The cycle migrates Title, Song Select, Loading, Pause,
stage banners, Results, practice surfaces, and Customize dialogs. It does not
require rewriting every low-level widget merely to use a new constructor.

## Layout safety

Player-facing overlays use the 1280×720 reference-space transform already used
by gameplay and Song Select. Fixed screen-pixel practice/editor rails are
converted to reference-space constraints with:

- safe-area insets;
- maximum width/height;
- clipping or scrolling for overflow;
- wrapping at semantic word boundaries;
- a minimum 720p layout and explicit ultrawide behavior.

Custom HUD widgets are clamped so at least their focus handle and critical text
remain inside the safe area. A resize or text-scale change repairs only the
runtime presentation; persisted coordinates are rewritten only after the user
moves/saves the layout.

## Error handling

- Invalid accessibility config recovers to defaults and raises a nonblocking
  warning.
- A component that cannot fit at XL text uses its defined compact layout; it
  must not silently shrink below the role minimum.
- Failed config/layout/profile writes use the shared Error notification or the
  editor error footer and retain the dirty draft.
- Missing glyphs fall back to text markers (`>`, `!`, `[x]`) so shape cues are
  not font-dependent.

## Verification

Automated coverage includes:

- config defaulting, round-trip, legacy No Fail migration, and discard restore;
- semantic role scaling at 1.00/1.25/1.50 and minimum-size clamps;
- policy transformation for every motion/flash primitive;
- No Fail badge, run qualification, results explanation, and score exclusion;
- non-color focus/selection/error markers;
- shared button/dialog/notification reducers across keyboard, pad, and pointer;
- safe layout at 1280×720, 1920×1080, 2560×1080, and XL text;
- practice/editor overflow, wrapping, and widget recovery.

Visual acceptance is performed at desk distance and at 2.5–3.5 m with Standard
and XL text. Full-motion screenshots and reduced-effects screenshots are kept
as review artifacts, not golden pixel tests.

## Acceptance criteria

- Every independent accessibility setting is live, persisted, and reversible.
- Important state remains understandable in grayscale and with motion/flashes
  reduced.
- No Fail is visible during play and cannot enter ordinary records.
- Player-critical surfaces use shared semantic tokens and components.
- No supported viewport/text-scale combination hides the only available action
  or critical state.
