# Song Select and Song Ready Directional Navigation — Design

Unify keyboard navigation across Song Select and Song Ready around one spatial
rule: horizontal input chooses what is active, while vertical input changes the
active selection or value. This removes Song Ready's unnecessary keyboard Edit
step without changing the repository's shared navigation API or current pad
semantics.

## Goals

- Make Song Select and Song Ready teach the same directional model.
- Let keyboard players adjust Ready options directly without entering Edit.
- Make the visible arrows, focus treatment, and legends match the controls.
- Preserve the existing difficulty source of truth, config paths, mouse input,
  and limited pad interaction model.

## Non-goals

- No changes to `NavAction`, `NavVerb`, `NavSource`, shared input architecture,
  or physical pad mappings.
- No changes to Practice Setup or any practice gameplay implementation.
- No new Ready options, Song Select discovery actions, or config fields.
- No redesign of Song Select's information architecture.

## Unified control model

The keyboard rule on both screens is:

```text
Left / Right  choose the active region or card
Up / Down     change the active selection or value
Enter         perform the active primary action
Escape        return one layer
```

### Song Select

Song Select keeps its existing `Songs` and `Difficulty` focus regions.

- Left/Right moves between the two regions and never changes difficulty.
- Up/Down changes the song when `Songs` is focused.
- Up/Down changes `Selection.difficulty` when `Difficulty` is focused.
- Enter opens Song Ready from either region with the current difficulty.
- Shift+Enter opens Song Ready with Practice preselected from either region.
- Existing search-clear-before-back behavior remains unchanged.

Requiring the player to enter Difficulty before Ready is deliberately rejected:
Ready is already the final confirmation surface, so another confirmation step
would slow discovery without preventing an accidental launch.

### Song Ready

In keyboard Browse, Left/Right moves through the horizontal strip:

```text
Modifiers -> Mode -> Central Song -> Lane Speed -> Audio
```

Up/Down immediately changes the focused card:

- Modifiers: switch between None and No Fail.
- Mode: switch between Normal and Practice.
- Central Song: select the previous/next available difficulty, clamped.
- Lane Speed: adjust by 0.5 within the existing 0.5–9.0 range.
- Audio: adjust the active BGM or Drums row by 5%, clamped to 0–100%.

Enter on Audio toggles the active row between BGM and Drums. Enter on the
central card activates its primary action. Enter on Modifiers, Mode, or Lane
Speed does nothing because those values are already directly adjustable.

Keyboard never enters `SongReadyLayer::Edit`. The explicit Edit and
PrimaryDetail layers remain local to Song Ready for the current limited pad
model; no shared input redesign is introduced.

When Practice is active, keyboard and pad card traversal continue to skip the
disabled Modifiers card. Switching back to Normal reveals the previously
selected None/No Fail value.

## Mouse and pad behavior

Mouse behavior remains direct:

- Clicking a card focuses it without launching.
- Clicking visible value controls adjusts that value.
- Clicking an Audio row selects BGM or Drums.
- Clicking the central Start action launches or opens Practice Setup.
- Mouse wheel over the central card changes difficulty.

Value controls use vertical up/down affordances rather than left/right arrows,
so the presentation does not contradict horizontal card navigation.

The current pad two-level model remains unchanged because the pad API does not
provide full spatial Left/Right input:

- Browse Up/Down traverses cards; Confirm enters a card; Back closes Ready.
- In an entered option, Up/Down changes values; Confirm applies; Back cancels.
- PrimaryDetail Up/Down changes difficulty; Confirm launches; Back returns.

## State and persistence

Both screens continue to read and update only `Selection.difficulty`; Ready
does not gain duplicate difficulty state.

Keyboard and mouse value changes are applied immediately. Config-backed values
(fail mode, lane speed, BGM volume, and drum volume) are persisted through the
existing config save path after each discrete adjustment. A save failure keeps
the in-memory value visible and raises the existing notification rather than
closing Ready or launching.

Pad Edit retains its pre-edit snapshot and Confirm/Back transaction behavior.
Keyboard Escape closes Ready without reverting already applied changes. Mode
and Audio row focus are local Ready UI state and do not require persistence.

## Presentation cleanup

- Song Select and Ready legends use the shared wording
  `←→ SELECT · ↑↓ CHANGE` with screen-specific Enter/Escape actions appended.
- Both screens show focus through border thickness, scale/elevation, and a
  directional marker rather than color alone.
- Ready uses a stronger scrim so Song Select remains contextual but does not
  compete with the five-card strip.
- The central card preserves one horizontal strip while separating the jacket,
  difficulty rail, and metadata into non-overlapping columns. Secondary
  metadata truncates before primary labels shrink.
- Reduced Motion continues to remove large focus scales and slides.

## Error handling

- Difficulty, speed, and volume changes clamp at their existing bounds.
- Empty or missing chart selections ignore adjustment and launch requests and
  retain the existing warning behavior.
- Disabled Modifiers ignores adjustment while Practice is active.
- Config save errors surface through `NotificationQueue` and do not corrupt
  `Selection.difficulty` or unrelated config fields.
- Existing silenced entity-command cleanup remains intact during Ready
  open/close and Song Select wheel rebuilds.

## Testing

Unit tests cover:

- Song Select Left/Right region movement and Up/Down selection changes.
- Enter and Shift+Enter opening Ready from either Song Select region.
- Ready keyboard Left/Right traversal, including Practice's Modifiers skip.
- Immediate Up/Down changes for every Ready card and boundary clamping.
- Audio Enter toggling BGM/Drums and Up/Down adjusting only the active row.
- Keyboard never entering Edit, while pad Edit transactions still apply and
  cancel correctly.
- Config merge/save helpers preserving unrelated and newer selection fields.

Runtime tests exercise repeated Song Select entry, Ready open/close, immediate
adjustment, and launch transitions to catch Bevy query or deferred-command
conflicts. Package tests, warnings-denied Clippy, formatting, and the desktop
compile check are required before handoff.

## Acceptance criteria

1. Song Select and Ready consistently use horizontal input for focus and
   vertical input for selection/value changes on keyboard.
2. Enter opens Ready from either Song Select region; Ready still prevents a
   single Song Select click or selection action from launching gameplay.
3. Ready keyboard option changes require no Edit step and persist through
   existing config mechanisms.
4. Audio exposes an obvious active row; Enter toggles the row and Up/Down
   changes only that value.
5. Mouse remains fully operable, and current pad behavior remains compatible
   without shared navigation or mapping changes.
6. Ready's central content no longer overlaps, and background Song Select is
   visibly de-emphasized at 1280×720 and 1920×1080.
